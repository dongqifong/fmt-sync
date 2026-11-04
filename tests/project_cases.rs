//! Persistent project fixtures tested with the actual native tools.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

static RUN_ID: OnceLock<String> = OnceLock::new();

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), files);
            } else {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn run(root: &Path, stage: &str, args: &[&str], expected: i32) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fmt-sync"));
    command.current_dir(root).args(args);
    if let Some(extra) = std::env::var_os("FMT_SYNC_SMOKE_TOOL_PATH") {
        let paths = std::env::split_paths(&extra)
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ))
            .collect::<Vec<_>>();
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let output: Output = command.output().unwrap();
    let text = format!(
        "Command: fmt-sync {}\nExit: {:?}\n\nSTDOUT\n{}\nSTDERR\n{}",
        args.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::create_dir_all(root.join("_logs")).unwrap();
    fs::write(root.join("_logs").join(format!("{stage}.log")), &text).unwrap();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{}\n{stage}: {text}",
        root.display()
    );
    text
}

fn exercise(name: &str, languages: &str, sources: &[&str], diagnostics: &[&str]) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = repository.join("tests/projects").join(name);
    let fixes = repository.join("tests/project_fixes").join(name);
    let root = repository
        .join("tests/.runs")
        .join(RUN_ID.get_or_init(|| {
            format!(
                "{}-{}-{}",
                std::env::consts::OS,
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )
        }))
        .join(name);
    assert!(
        !root.exists(),
        "Project work directory already exists: {}",
        root.display()
    );
    copy_tree(&fixture, &root);
    let fixture_before = snapshot(&fixture);
    let ignored = snapshot(&root.join("legacy"));
    let original = sources
        .iter()
        .map(|path| (path, fs::read(root.join(path)).unwrap()))
        .collect::<Vec<_>>();
    let config = fs::read(root.join("fmt-sync.toml")).unwrap();

    run(&root, "01-init", &["init", "--lang", languages], 0);
    run(&root, "02-check-broken", &["check"], 1);
    run(&root, "03-format", &["format"], 0);
    for (path, before) in &original {
        assert_ne!(
            &fs::read(root.join(path)).unwrap(),
            before,
            "Formatter did not change {path}"
        );
    }
    let lint = run(&root, "04-check-formatted", &["check"], 1);
    assert!(
        !lint.contains("DEPRECATED"),
        "Freshly generated native configuration is deprecated: {lint}"
    );
    for diagnostic in diagnostics {
        assert!(
            lint.contains(diagnostic),
            "Expected actual lint diagnostic {diagnostic}: {lint}"
        );
    }
    assert!(
        !lint.contains("[skipped]"),
        "A native tool was unexpectedly skipped: {lint}"
    );
    let lint_sources = sources
        .iter()
        .map(|path| {
            (
                (*path).to_owned(),
                fs::read_to_string(root.join(path)).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    copy_tree(&fixes, &root);
    run(&root, "05-format-fixed", &["format"], 0);
    run(&root, "06-check-fixed", &["check"], 0);
    let formatted = sources
        .iter()
        .map(|path| (path, fs::read(root.join(path)).unwrap()))
        .collect::<Vec<_>>();
    run(&root, "07-format-again", &["format"], 0);
    for (path, before) in formatted {
        assert_eq!(
            fs::read(root.join(path)).unwrap(),
            before,
            "Formatting is not idempotent for {path}"
        );
    }
    assert_eq!(
        snapshot(&root.join("legacy")),
        ignored,
        "An excluded project was modified"
    );
    assert_eq!(
        fs::read(root.join("fmt-sync.toml")).unwrap(),
        config,
        "Project configuration changed"
    );
    assert_eq!(
        snapshot(&fixture.join("legacy")),
        ignored,
        "Original fixtures changed"
    );
    assert_eq!(
        snapshot(&fixture),
        fixture_before,
        "Original project sources changed"
    );
    fs::write(
        root.join("_logs/result.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "project": name,
            "platform": std::env::consts::OS,
            "languages": languages,
            "work_directory": root,
            "expected_exit_codes": [0, 1, 0, 1, 0, 0, 0],
            "observed_lint_diagnostics": diagnostics,
            "formatted_sources_still_containing_lint": lint_sources,
            "excluded_project_unchanged": true,
            "idempotent": true,
            "passed": true
        }))
        .unwrap(),
    )
    .unwrap();
    println!(
        "{name}: PASS — actual tools; logs: {}",
        root.join("_logs").display()
    );
}

#[test]
#[ignore = "requires actual Ruff"]
fn python_project_reports_real_lint_and_accepts_fixed_code() {
    exercise("python", "python", &["app.py"], &["F401", "F821"]);
}

#[test]
#[ignore = "requires actual Go"]
fn go_project_reports_real_vet_and_accepts_fixed_code() {
    exercise("go", "go", &["main.go"], &["Printf", "wrong type"]);
}

#[test]
#[ignore = "requires actual Cargo, rustfmt and Clippy"]
fn rust_project_reports_real_clippy_and_accepts_fixed_code() {
    exercise("rust", "rust", &["src/lib.rs"], &["clippy::iter_count"]);
}

#[test]
#[ignore = "requires actual Biome"]
fn typescript_project_reports_real_lint_and_accepts_fixed_code() {
    exercise("typescript", "typescript", &["app.ts"], &["noDoubleEquals"]);
}

#[test]
#[ignore = "requires actual Ruff and Biome"]
fn backend_project_checks_both_languages_and_accepts_fixed_code() {
    exercise(
        "backend",
        "python,typescript",
        &["service.py", "client.ts"],
        &["F401", "noDoubleEquals"],
    );
}
