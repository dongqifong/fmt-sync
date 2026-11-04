use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Output,
};

use anyhow::{Context, Result, ensure};

use crate::{
    config::{Config, Language},
    native,
    report::{StageResult, Status},
    rust_project,
    scope::Scope,
    tools::{self, Tool, append, os, strings},
};

pub fn execute(root: &Path, config: &Config, check: bool) -> Vec<StageResult> {
    let scope = match Scope::discover(root, config) {
        Ok(scope) => scope,
        Err(error) => {
            return vec![StageResult::error(
                config.languages[0],
                "scope",
                format!("{error:#}"),
            )];
        }
    };
    let mut results = Vec::new();
    for language in &config.languages {
        let outcome = match language {
            Language::Python => python(root, config, &scope, check),
            Language::Typescript => typescript(root, config, &scope, check),
            Language::Go => go(root, config, &scope, check),
            Language::Rust => rust(root, config, &scope, check),
        };
        match outcome {
            Ok(stages) => results.extend(stages),
            Err(error) => {
                results.push(StageResult::error(*language, "setup", format!("{error:#}")))
            }
        }
    }
    results
}

fn files(scope: &Scope, extensions: &[&str]) -> Vec<PathBuf> {
    scope
        .files
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| extensions.contains(&s))
        })
        .cloned()
        .collect()
}

fn python(root: &Path, config: &Config, scope: &Scope, check: bool) -> Result<Vec<StageResult>> {
    let language = Language::Python;
    let paths = files(scope, &["py", "pyi", "ipynb"]);
    if paths.is_empty() {
        return Ok(vec![StageResult::skipped(
            language,
            if check { "check" } else { "format" },
        )]);
    }
    let native = native::discover(root, language)?;
    let tool = tools::resolve(root, "ruff")?.versioned(root, &["--version"], "ruff", config)?;
    let mut results = Vec::new();
    if !check {
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
        results.push(batched(
            root, language, "imports", &tool, &args, &paths, false, true,
        ));
    }
    let mut args = strings(&["format", "--force-exclude", "--no-cache"]);
    if native.is_none() {
        args.push(os("--isolated"));
    }
    if check {
        args.push(os("--check"));
    }
    args.push(os("--"));
    results.push(batched(
        root, language, "format", &tool, &args, &paths, false, true,
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
        results.push(batched(
            root, language, "lint", &tool, &args, &paths, false, true,
        ));
    }
    Ok(results)
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

fn typescript(
    root: &Path,
    config: &Config,
    scope: &Scope,
    check: bool,
) -> Result<Vec<StageResult>> {
    let language = Language::Typescript;
    let paths = files(
        scope,
        &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"],
    );
    if paths.is_empty() {
        return Ok(vec![StageResult::skipped(
            language,
            if check { "check" } else { "format" },
        )]);
    }
    ensure!(
        native::discover(root, language)?.is_some(),
        "The current project has no Biome configuration; run fmt-sync init --lang typescript"
    );
    let tool = tools::resolve(root, "biome")?.versioned(root, &["--version"], "biome", config)?;
    let stage = if check { "check" } else { "format" };
    let mut args = strings(&[stage, "--no-errors-on-unmatched", "--config-path"]);
    args.push(os(root));
    if !check {
        args.push(os("--write"));
    }
    args.push(os("--"));
    Ok(vec![batched(
        root, language, stage, &tool, &args, &paths, false, true,
    )])
}

fn go(root: &Path, config: &Config, scope: &Scope, check: bool) -> Result<Vec<StageResult>> {
    let language = Language::Go;
    let paths = scope.go_files.clone();
    let mut results = Vec::new();
    if paths.is_empty() {
        results.push(StageResult::skipped(language, "format"));
    } else {
        match (|| -> Result<Tool> {
            let go = tools::resolve(root, "go")?.versioned(root, &["version"], "go", config)?;
            let mut fmt = tools::resolve(root, "gofmt")?;
            fmt.label = format!("{} ({})", fmt.program.display(), go.label);
            Ok(fmt)
        })() {
            Ok(tool) => results.push(batched(
                root,
                language,
                "format",
                &tool,
                &strings(&[if check { "-l" } else { "-w" }]),
                &paths,
                check,
                false,
            )),
            Err(error) => {
                results.push(StageResult::error(language, "format", format!("{error:#}")))
            }
        }
    }
    if check {
        let lint = (|| -> Result<StageResult> {
            ensure!(
                root.join("go.mod").is_file(),
                "The current project has no go.mod; run go mod init first"
            );
            let tool = tools::resolve(root, "go")?.versioned(root, &["version"], "go", config)?;
            let packages = &scope.go_packages;
            if packages.is_empty() {
                return Ok(StageResult::skipped(language, "lint"));
            }
            // Explicit package paths avoid recursively linting ignored or independently managed projects.
            Ok(batched(
                root,
                language,
                "lint",
                &tool,
                &strings(&["vet"]),
                packages,
                false,
                true,
            ))
        })();
        results
            .push(lint.unwrap_or_else(|e| StageResult::error(language, "lint", format!("{e:#}"))));
    }
    Ok(results)
}

fn rust(root: &Path, config: &Config, scope: &Scope, check: bool) -> Result<Vec<StageResult>> {
    let language = Language::Rust;
    native::discover(root, language)?;
    let cargo = tools::resolve(root, "cargo")?.versioned(root, &["--version"], "cargo", config)?;
    let package = rust_project::prepare(root, scope, &cargo)?;
    let mut results = Vec::new();
    let format = (|| -> Result<StageResult> {
        let tool = cargo
            .clone()
            .versioned(root, &["fmt", "--version"], "rustfmt", config)?;
        let mut args = strings(&["fmt", "--package", &package]);
        if check {
            args.extend(strings(&["--", "--check"]));
        }
        Ok(single(root, language, "format", &tool, &args, true))
    })();
    results
        .push(format.unwrap_or_else(|e| StageResult::error(language, "format", format!("{e:#}"))));
    if check {
        let lint = (|| -> Result<StageResult> {
            let tool = cargo
                .clone()
                .versioned(root, &["clippy", "--version"], "clippy", config)?;
            let args = strings(&[
                "clippy",
                "--package",
                &package,
                "--no-deps",
                "--",
                "-D",
                "warnings",
            ]);
            // Clippy exits 101 for both denied lints and compiler failures. Keep its native diagnostics;
            // report a failed stage without claiming the underlying cause was precisely classified.
            Ok(single(root, language, "lint", &tool, &args, true))
        })();
        results
            .push(lint.unwrap_or_else(|e| StageResult::error(language, "lint", format!("{e:#}"))));
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn batched(
    root: &Path,
    language: Language,
    stage: &'static str,
    tool: &Tool,
    args: &[OsString],
    paths: &[PathBuf],
    dirty_stdout: bool,
    findings: bool,
) -> StageResult {
    let mut status = Status::Passed;
    let mut details = String::new();
    for paths in tools::batches(paths) {
        let output = tool.output(root, &append(args, &paths));
        let (next, detail) = evaluate(output, dirty_stdout, findings, false);
        if next.code() > status.code() {
            status = next;
        }
        details.push_str(&detail);
    }
    StageResult {
        language,
        stage,
        status,
        tool: tool.label.clone(),
        detail: details,
    }
}

fn single(
    root: &Path,
    language: Language,
    stage: &'static str,
    tool: &Tool,
    args: &[OsString],
    findings: bool,
) -> StageResult {
    let (status, detail) = evaluate(tool.output(root, args), false, findings, true);
    StageResult {
        language,
        stage,
        status,
        tool: tool.label.clone(),
        detail,
    }
}

fn evaluate(
    output: Result<Output>,
    dirty_stdout: bool,
    findings: bool,
    cargo: bool,
) -> (Status, String) {
    let output = match output {
        Ok(output) => output,
        Err(error) => return (Status::Error, format!("{error:#}\n")),
    };
    let code = output.status.code();
    let status = if output.status.success() {
        if dirty_stdout && !output.stdout.is_empty() {
            Status::Failed
        } else {
            Status::Passed
        }
    } else if findings && (code == Some(1) || (cargo && code == Some(101))) {
        Status::Failed
    } else {
        Status::Error
    };
    let mut detail = String::from_utf8_lossy(&output.stdout).to_string();
    detail.push_str(&String::from_utf8_lossy(&output.stderr));
    if status != Status::Passed && detail.trim().is_empty() {
        detail.push_str(&format!("Tool exit code: {code:?}\n"));
    }
    (status, detail)
}
