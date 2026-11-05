use super::{LanguageAdapter, RunContext};
use crate::{
    execution::{RunMode, StageKind, StageResult},
    language::Language,
    native::{self, ConfigSource, NativeConfigSpec},
    runner::{self, OutcomePolicy, StageCommand},
    tools::{self, Tool, ToolSpec, VersionSpec, hints, os, strings},
};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub struct Python;
const RUFF: ToolSpec = ToolSpec {
    name: "ruff",
    version: Some(VersionSpec {
        key: "ruff",
        args: &["--version"],
    }),
    install_hint: hints::RUFF,
    local: Some(local_ruff),
};

impl LanguageAdapter for Python {
    fn tools(&self) -> &'static [ToolSpec] {
        &[RUFF]
    }
    fn native_config(&self) -> Option<NativeConfigSpec> {
        Some(NativeConfigSpec {
            project_files: &[".ruff.toml", "ruff.toml", "pyproject.toml"],
            preset_files: &["ruff.toml", ".ruff.toml"],
            default: Some((
                "ruff.toml",
                "line-length = 88\n\n[lint]\nselect = [\"E4\", \"E7\", \"E9\", \"F\", \"I\"]\n",
            )),
            validate: validate_config,
        })
    }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode) -> Result<Vec<StageResult>> {
        let root = context.root();
        let scope = context.scope;
        let check = mode.is_check();
        let language = Language::Python;
        let paths = scope.files_with_extensions(&["py", "pyi", "ipynb"]);
        if paths.is_empty() {
            return Ok(vec![StageResult::skipped(language, mode.primary_stage())]);
        }
        let native = native::discover(root, self.native_config().as_ref())?;
        let tool = context.tool(&RUFF)?;
        let mut results = Vec::new();
        if !check {
            let imports = (|| -> Result<StageResult> {
                let non_import_rules = ruff_non_import_rules(root, &tool)?;
                let mut args = strings(&[
                    "check",
                    "--fix-only",
                    "--no-unsafe-fixes",
                    "--force-exclude",
                    "--no-cache",
                    "--extend-ignore",
                ]);
                args.push(os(non_import_rules));
                if native.is_none() {
                    args.extend(strings(&["--isolated", "--extend-select", "I"]));
                }
                args.push(os("--"));
                Ok(runner::run_batched(
                    root,
                    &StageCommand {
                        language,
                        stage: StageKind::Imports,
                        tool: &tool,
                        args: &args,
                        policy: OutcomePolicy::findings(&[1]),
                    },
                    &paths,
                ))
            })();
            results.push(imports.unwrap_or_else(|e| {
                StageResult::error(language, StageKind::Imports, format!("{e:#}"))
            }));
        }
        let mut args = strings(&["format", "--force-exclude", "--no-cache"]);
        if native.is_none() {
            args.push(os("--isolated"));
        }
        if check {
            args.push(os("--check"));
        }
        args.push(os("--"));
        results.push(runner::run_batched(
            root,
            &StageCommand {
                language,
                stage: StageKind::Format,
                tool: &tool,
                args: &args,
                policy: OutcomePolicy::findings(&[1]),
            },
            &paths,
        ));
        if check {
            let mut args = strings(&[
                "check",
                "--no-fix",
                "--no-fix-only",
                "--force-exclude",
                "--no-cache",
            ]);
            if native.is_none() {
                args.extend(strings(&["--isolated", "--extend-select", "I"]));
            }
            args.push(os("--"));
            results.push(runner::run_batched(
                root,
                &StageCommand {
                    language,
                    stage: StageKind::Lint,
                    tool: &tool,
                    args: &args,
                    policy: OutcomePolicy::findings(&[1]),
                },
                &paths,
            ));
        }
        Ok(results)
    }
}

fn local_ruff(root: &Path) -> Result<Option<Tool>> {
    let path = root
        .join(if cfg!(windows) {
            ".venv/Scripts"
        } else {
            ".venv/bin"
        })
        .join(format!("ruff{}", std::env::consts::EXE_SUFFIX));
    tools::local_executable(path, "ruff")
}

fn validate_config(path: &Path, content: &str, source: ConfigSource) -> Result<bool> {
    let parsed: toml::Value =
        toml::from_str(content).with_context(|| format!("Failed to parse {}", path.display()))?;
    if path
        .file_name()
        .is_some_and(|name| name == "pyproject.toml")
    {
        let section = parsed.get("tool").and_then(|tool| tool.get("ruff"));
        if let Some(section) = section {
            ensure!(section.is_table(), "tool.ruff must be a table");
        }
        return Ok(section.is_some());
    }
    if source == ConfigSource::Preset {
        ensure!(
            parsed.get("extend").is_none() && parsed.get("extends").is_none(),
            "Presets do not support extend/extends: {}",
            path.display()
        );
    }
    Ok(true)
}

fn ruff_non_import_rules(root: &Path, tool: &Tool) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Linter {
        prefix: String,
        #[serde(default)]
        categories: Vec<Linter>,
    }

    // Filter other rules without overriding the user's fixable/unfixable settings.
    let output = tool.output(root, &strings(&["linter", "--output-format", "json"]))?;
    ensure!(
        output.status.success(),
        "Failed to list Ruff rule families: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut pending: Vec<Linter> = serde_json::from_slice(&output.stdout)
        .context("Ruff returned invalid rule-family metadata")?;
    let mut excluded = Vec::new();
    let mut found_imports = false;
    while let Some(linter) = pending.pop() {
        if linter.prefix.is_empty() {
            ensure!(
                !linter.categories.is_empty(),
                "Ruff returned a rule family without a prefix or categories"
            );
            pending.extend(linter.categories);
        } else if linter.prefix == "I" {
            found_imports = true;
        } else {
            excluded.push(linter.prefix);
        }
    }
    ensure!(
        found_imports && !excluded.is_empty(),
        "Ruff did not report the rule families needed for import-only formatting"
    );
    Ok(excluded.join(","))
}
