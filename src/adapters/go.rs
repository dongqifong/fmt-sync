use super::{LanguageAdapter, RunContext};
use crate::{
    execution::{RunMode, StageKind, StageResult},
    language::Language,
    runner::{self, OutcomePolicy, StageCommand, StdoutPolicy},
    scope::{ScanPolicy, Scope},
    tools::{Tool, ToolSpec, VersionSpec, hints, strings},
};
use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

pub struct Go;
const GO: ToolSpec = ToolSpec {
    name: "go",
    version: Some(VersionSpec {
        key: "go",
        args: &["version"],
    }),
    install_hint: hints::GO,
    local: None,
};
const GOFMT: ToolSpec = ToolSpec {
    name: "gofmt",
    version: None,
    install_hint: hints::GO,
    local: None,
};

impl LanguageAdapter for Go {
    fn tools(&self) -> &'static [ToolSpec] {
        &[GO, GOFMT]
    }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode) -> Result<Vec<StageResult>> {
        let root = context.root();
        let scope = context.scope;
        let check = mode.is_check();
        let language = Language::Go;
        let targets = GoTargets::discover(scope)?;
        let paths = targets.files;
        let mut results = Vec::new();
        if paths.is_empty() {
            results.push(StageResult::skipped(language, StageKind::Format));
        } else {
            match (|| -> Result<Tool> {
                let go = context.tool(&GO)?;
                let mut fmt = GOFMT.resolve(root)?;
                fmt.label = format!("{} ({})", fmt.program.display(), go.label);
                Ok(fmt)
            })() {
                Ok(tool) => results.push(runner::run_batched(
                    root,
                    &StageCommand {
                        language,
                        stage: StageKind::Format,
                        tool: &tool,
                        args: &strings(&[if check { "-l" } else { "-w" }]),
                        policy: OutcomePolicy {
                            finding_codes: &[],
                            stdout: if check {
                                StdoutPolicy::Findings
                            } else {
                                StdoutPolicy::Diagnostics
                            },
                        },
                    },
                    &paths,
                )),
                Err(error) => results.push(StageResult::error(
                    language,
                    StageKind::Format,
                    format!("{error:#}"),
                )),
            }
        }
        if check {
            let lint = (|| -> Result<StageResult> {
                ensure!(
                    root.join("go.mod").is_file(),
                    "The current project has no go.mod; run go mod init first"
                );
                let tool = context.tool(&GO)?;
                let packages = &targets.packages;
                if packages.is_empty() {
                    return Ok(StageResult::skipped(language, StageKind::Lint));
                }
                // Explicit package paths avoid recursively linting ignored or independently managed projects.
                Ok(runner::run_batched(
                    root,
                    &StageCommand {
                        language,
                        stage: StageKind::Lint,
                        tool: &tool,
                        args: &strings(&["vet"]),
                        policy: OutcomePolicy::findings(&[1]),
                    },
                    packages,
                ))
            })();
            results.push(lint.unwrap_or_else(|e| {
                StageResult::error(language, StageKind::Lint, format!("{e:#}"))
            }));
        }
        Ok(results)
    }
}

struct GoTargets {
    files: Vec<PathBuf>,
    packages: Vec<PathBuf>,
}
impl GoTargets {
    fn discover(scope: &Scope) -> Result<Self> {
        // gofmt works on files; vet needs complete packages even when individual
        // source files are gitignored. Both scans retain fmt-sync boundaries.
        let files = scope
            .scan(ScanPolicy::RespectGitignore)?
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "go"))
            .collect();
        let mut go_packages = std::collections::BTreeSet::new();
        for file in &scope.files {
            if file.extension().is_some_and(|ext| ext == "go") {
                let parent = file.parent().unwrap_or(Path::new(""));
                let native_excluded = parent.components().any(|c| {
                    let text = c.as_os_str().to_string_lossy();
                    text.starts_with('.') || text.starts_with('_') || text == "testdata"
                });
                let filename = file.file_name().unwrap().to_string_lossy();
                if !native_excluded && !filename.starts_with(['.', '_']) {
                    go_packages.insert(if parent.as_os_str().is_empty() {
                        PathBuf::from(".")
                    } else {
                        parent.to_path_buf()
                    });
                }
            }
        }
        Ok(Self {
            files,
            packages: go_packages.into_iter().collect(),
        })
    }
}
