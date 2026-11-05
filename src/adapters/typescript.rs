use super::{LanguageAdapter, RunContext};
use crate::{
    execution::{RunMode, StageResult},
    language::Language,
    native::{self, ConfigSource, NativeConfigSpec},
    runner::{self, OutcomePolicy, StageCommand},
    tools::{self, Tool, ToolSpec, VersionSpec, hints, os, strings},
};
use anyhow::{Result, bail, ensure};
use std::{env, path::Path};

pub struct Typescript;
const BIOME: ToolSpec = ToolSpec {
    name: "biome",
    version: Some(VersionSpec {
        key: "biome",
        args: &["--version"],
    }),
    install_hint: hints::BIOME,
    local: Some(local_biome),
};

impl LanguageAdapter for Typescript {
    fn tools(&self) -> &'static [ToolSpec] {
        &[BIOME]
    }
    fn native_config(&self) -> Option<NativeConfigSpec> {
        Some(NativeConfigSpec {
            project_files: &["biome.json", "biome.jsonc"],
            preset_files: &["biome.json", "biome.jsonc"],
            default: Some((
                "biome.json",
                "{\n  \"formatter\": { \"enabled\": true },\n  \"linter\": { \"enabled\": true }\n}\n",
            )),
            validate: validate_config,
        })
    }
    fn check_init_conflicts(&self, root: &Path) -> Result<()> {
        check_conflicts(root)
    }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode) -> Result<Vec<StageResult>> {
        let root = context.root();
        let scope = context.scope;
        let check = mode.is_check();
        let language = Language::Typescript;
        let paths =
            scope.files_with_extensions(&["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]);
        if paths.is_empty() {
            return Ok(vec![StageResult::skipped(language, mode.primary_stage())]);
        }
        ensure!(
            native::discover(root, self.native_config().as_ref())?.is_some(),
            "The current project has no Biome configuration; run fmt-sync init --lang typescript"
        );
        let tool = context.tool(&BIOME)?;
        let stage = if check { "check" } else { "format" };
        let mut args = strings(&[stage, "--no-errors-on-unmatched", "--config-path"]);
        args.push(os(root));
        if !check {
            args.push(os("--write"));
        }
        args.push(os("--"));
        Ok(vec![runner::run_batched(
            root,
            &StageCommand {
                language,
                stage: mode.primary_stage(),
                tool: &tool,
                args: &args,
                policy: OutcomePolicy::findings(&[1]),
            },
            &paths,
        )])
    }
}

fn validate_config(path: &Path, content: &str, source: ConfigSource) -> Result<bool> {
    let parsed = native::parse_json(content, path)?;
    ensure!(
        parsed.is_object(),
        "Biome configuration must be a JSON object: {}",
        path.display()
    );
    if source == ConfigSource::Preset {
        ensure!(
            parsed.get("extends").is_none(),
            "Presets do not support extends: {}",
            path.display()
        );
    }
    Ok(true)
}

fn local_biome(root: &Path) -> Result<Option<Tool>> {
    let filename = format!("biome{}", env::consts::EXE_SUFFIX);
    let mut local = Vec::new();
    local.push(root.join("node_modules/.bin").join(&filename));
    let arch = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        x => x,
    };
    let os = if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    local.push(root.join(format!("node_modules/@biomejs/cli-{os}-{arch}/{filename}")));
    if cfg!(target_os = "linux") && cfg!(target_env = "musl") {
        local.insert(
            0,
            root.join(format!(
                "node_modules/@biomejs/cli-linux-{arch}-musl/{filename}"
            )),
        );
    }

    for candidate in local {
        if let Some(tool) = tools::local_executable(candidate, "biome")? {
            return Ok(Some(tool));
        }
    }
    let entry = root.join("node_modules/@biomejs/biome/bin/biome");
    if entry.is_file() {
        let mut node = tools::from_path_with_hint("node", hints::NODE)?;
        node.prefix.push(entry.into_os_string());
        return Ok(Some(node));
    }
    Ok(None)
}

fn check_conflicts(root: &Path) -> Result<()> {
    let names = [
        ".eslintrc",
        ".eslintrc.json",
        ".eslintrc.js",
        ".eslintrc.cjs",
        ".eslintrc.yaml",
        ".eslintrc.yml",
        "eslint.config.js",
        "eslint.config.mjs",
        "eslint.config.cjs",
        "eslint.config.ts",
        "eslint.config.mts",
        "eslint.config.cts",
        ".prettierrc",
        ".prettierrc.json",
        ".prettierrc.json5",
        ".prettierrc.yaml",
        ".prettierrc.yml",
        ".prettierrc.js",
        ".prettierrc.cjs",
        ".prettierrc.mjs",
        ".prettierrc.toml",
        "prettier.config.js",
        "prettier.config.cjs",
        "prettier.config.mjs",
        "prettier.config.ts",
    ];
    let manifest = root.join("package.json");
    let package_conflict = if manifest.is_file() {
        let value = native::read_json(&manifest)?;
        value.get("eslintConfig").is_some() || value.get("prettier").is_some()
    } else {
        false
    };
    if package_conflict || names.iter().any(|name| root.join(name).exists()) {
        bail!(
            "Existing ESLint/Prettier configuration found; resolve overlapping tools before initializing Biome. Automatic migration is not supported"
        );
    }
    Ok(())
}
