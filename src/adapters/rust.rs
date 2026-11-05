use super::{LanguageAdapter, RunContext};
use crate::{
    execution::{RunMode, StageKind, StageResult},
    language::Language,
    native::{self, ConfigSource, NativeConfigSpec},
    runner::{self, OutcomePolicy, StageCommand},
    tools::{ToolSpec, VersionSpec, hints, strings},
};
use anyhow::{Context, Result, ensure};
use std::path::Path;
mod project;

pub struct Rust;
const CARGO: ToolSpec = ToolSpec {
    name: "cargo",
    version: Some(VersionSpec {
        key: "cargo",
        args: &["--version"],
    }),
    install_hint: hints::CARGO,
    local: None,
};
const RUSTFMT: ToolSpec = ToolSpec {
    name: "cargo",
    version: Some(VersionSpec {
        key: "rustfmt",
        args: &["fmt", "--version"],
    }),
    install_hint: hints::RUSTFMT,
    local: None,
};
const CLIPPY: ToolSpec = ToolSpec {
    name: "cargo",
    version: Some(VersionSpec {
        key: "clippy",
        args: &["clippy", "--version"],
    }),
    install_hint: hints::CLIPPY,
    local: None,
};

impl LanguageAdapter for Rust {
    fn tools(&self) -> &'static [ToolSpec] {
        &[CARGO, RUSTFMT, CLIPPY]
    }
    fn native_config(&self) -> Option<NativeConfigSpec> {
        Some(NativeConfigSpec {
            project_files: &["rustfmt.toml", ".rustfmt.toml"],
            preset_files: &["rustfmt.toml", ".rustfmt.toml"],
            default: Some((
                "rustfmt.toml",
                "# Use rustfmt stable defaults; edit this file to customize.\n",
            )),
            validate: validate_config,
        })
    }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode) -> Result<Vec<StageResult>> {
        let root = context.root();
        let scope = context.scope;
        let check = mode.is_check();
        let language = Language::Rust;
        let cargo = context.tool(&CARGO)?;
        let package = project::discover(root, &cargo)?;
        let mut results = Vec::new();
        let format = (|| -> Result<StageResult> {
            native::discover(root, self.native_config().as_ref())?;
            project::validate_format_scope(root, scope, &package)?;
            let tool = context.versioned(cargo.clone(), &RUSTFMT)?;
            let mut args = strings(&["fmt", "--package", &package.name]);
            if check {
                args.extend(strings(&["--", "--check"]));
            }
            Ok(runner::run(
                root,
                &StageCommand {
                    language,
                    stage: StageKind::Format,
                    tool: &tool,
                    args: &args,
                    policy: OutcomePolicy::findings(&[1, 101]),
                },
            ))
        })();
        results.push(
            format.unwrap_or_else(|e| {
                StageResult::error(language, StageKind::Format, format!("{e:#}"))
            }),
        );
        if check {
            let lint = (|| -> Result<StageResult> {
                let tool = context.versioned(cargo.clone(), &CLIPPY)?;
                let args = strings(&[
                    "clippy",
                    "--package",
                    &package.name,
                    "--no-deps",
                    "--",
                    "-D",
                    "warnings",
                ]);
                // Clippy exits 101 for both denied lints and compiler failures. Keep its native diagnostics;
                // report a failed stage without claiming the underlying cause was precisely classified.
                Ok(runner::run(
                    root,
                    &StageCommand {
                        language,
                        stage: StageKind::Lint,
                        tool: &tool,
                        args: &args,
                        policy: OutcomePolicy::findings(&[1, 101]),
                    },
                ))
            })();
            results.push(lint.unwrap_or_else(|e| {
                StageResult::error(language, StageKind::Lint, format!("{e:#}"))
            }));
        }
        Ok(results)
    }
}

fn validate_config(path: &Path, content: &str, source: ConfigSource) -> Result<bool> {
    let parsed: toml::Value =
        toml::from_str(content).with_context(|| format!("Failed to parse {}", path.display()))?;
    if source == ConfigSource::Preset {
        ensure!(
            parsed.get("extend").is_none() && parsed.get("extends").is_none(),
            "Presets do not support extend/extends: {}",
            path.display()
        );
    }
    Ok(true)
}
