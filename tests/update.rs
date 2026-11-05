mod support;

use std::{fs, process::Output};
use support::{calls, install_tool, run};
use tempfile::TempDir;

fn setup() -> (TempDir, TempDir) {
    let root = TempDir::new().unwrap();
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "git");
    install_tool(tools.path(), "cargo");
    (root, tools)
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn update_help_explains_the_command_without_project_configuration() {
    let (root, tools) = setup();
    let out = run(root.path(), &["update", "--help"], tools.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("Cargo"));
    assert!(calls(root.path()).is_empty());
}

#[test]
fn newest_stable_tag_is_compared_numerically_and_installed_with_cargo() {
    let (root, tools) = setup();
    fs::write(root.path().join("fmt-sync.toml"), "invalid config").unwrap();
    let out = run(
        root.path(),
        &["update"],
        tools.path(),
        &[
            (
                "FAKE_GIT_TAGS",
                "abc\trefs/tags/v1.9.0\ndef\trefs/tags/v1.10.0\nghi\trefs/tags/v9.0.0-rc.1\njkl\trefs/tags/nightly\nmno\trefs/tags/1.2.0",
            ),
            ("CLICOLOR_FORCE", "1"),
            ("NO_COLOR", ""),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let log = calls(root.path());
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(
        log[0]["args"],
        serde_json::json!([
            "ls-remote",
            "--tags",
            "--refs",
            "https://github.com/dongqifong/fmt-sync.git"
        ])
    );
    assert_eq!(
        log[1]["args"],
        serde_json::json!([
            "install",
            "--git",
            "https://github.com/dongqifong/fmt-sync.git",
            "--tag",
            "v1.10.0",
            "--locked",
            "--force",
            "fmt-sync"
        ])
    );
    assert!(text(&out).contains("Update successful"), "{}", text(&out));
    assert!(text(&out).contains("\x1b[32m"), "{}", text(&out));
}

#[test]
fn equal_or_older_tags_report_current_in_cyan_without_cargo() {
    let (root, tools) = setup();
    for tags in [
        "a\trefs/tags/v1.1.0",
        "a\trefs/tags/0.9.0",
        "a\trefs/tags/v1.1.0+build.7",
    ] {
        let _ = fs::remove_file(root.path().join("tool-log.jsonl"));
        let out = run(
            root.path(),
            &["update"],
            tools.path(),
            &[
                ("FAKE_GIT_TAGS", tags),
                ("CLICOLOR_FORCE", "1"),
                ("NO_COLOR", ""),
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", text(&out));
        assert!(text(&out).contains("Already up to date"), "{}", text(&out));
        assert!(text(&out).contains("\x1b[36m"), "{}", text(&out));
        assert_eq!(calls(root.path()).len(), 1);
    }
}

#[test]
fn cargo_failure_has_red_failure_status_and_nonzero_exit() {
    let (root, tools) = setup();
    let out = run(
        root.path(),
        &["update"],
        tools.path(),
        &[
            ("FAKE_GIT_TAGS", "a\trefs/tags/v1.2.0"),
            ("FAKE_CARGO_EXIT", "101"),
            ("CLICOLOR_FORCE", "1"),
            ("NO_COLOR", ""),
        ],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("Update failed"), "{}", text(&out));
    assert!(text(&out).contains("\x1b[31m"), "{}", text(&out));
    assert!(text(&out).contains("fixture diagnostic"));
    assert!(!text(&out).contains("Update successful"));
}

#[test]
fn failed_query_does_not_attempt_installation() {
    let (root, tools) = setup();
    let out = run(
        root.path(),
        &["update"],
        tools.path(),
        &[
            ("FAKE_GIT_TAGS", "a\trefs/tags/v1.2.0"),
            ("FAKE_GIT_EXIT", "128"),
        ],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("Update failed"));
    assert!(text(&out).contains("fixture diagnostic"));
    assert_eq!(calls(root.path()).len(), 1);
}

#[test]
fn absent_stable_tags_are_an_error_instead_of_success() {
    let (root, tools) = setup();
    for tags in [
        "",
        "a\trefs/tags/v2.0.0-beta.1\nb\trefs/tags/nightly",
        "garbage v2.0.0",
    ] {
        let out = run(
            root.path(),
            &["update"],
            tools.path(),
            &[("FAKE_GIT_TAGS", tags)],
        );
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(
            text(&out).contains("No stable version tags"),
            "{}",
            text(&out)
        );
        assert!(!text(&out).contains("Update successful"));
    }
    assert!(calls(root.path()).iter().all(|call| call["tool"] == "git"));
}

#[test]
fn missing_tools_report_how_to_install_them() {
    for missing in ["git", "cargo"] {
        let root = TempDir::new().unwrap();
        let tools = TempDir::new().unwrap();
        if missing == "git" {
            install_tool(tools.path(), "cargo");
        } else {
            install_tool(tools.path(), "git");
        }
        let out = run(
            root.path(),
            &["update"],
            tools.path(),
            &[("FAKE_GIT_TAGS", "a\trefs/tags/v1.2.0")],
        );
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(text(&out).contains("Update failed"));
        assert!(
            text(&out).contains(if missing == "git" {
                "git-scm.com"
            } else {
                "rust-lang.org"
            }),
            "{}",
            text(&out)
        );
        assert!(
            calls(root.path())
                .iter()
                .all(|call| call["tool"] != "cargo")
        );
    }
}

#[test]
fn redirected_and_no_color_update_statuses_have_no_escape_codes() {
    let (root, tools) = setup();
    for (tags, exit) in [
        ("a\trefs/tags/v1.1.0", "0"),
        ("a\trefs/tags/v1.2.0", "0"),
        ("a\trefs/tags/v1.2.0", "101"),
    ] {
        let out = run(
            root.path(),
            &["update"],
            tools.path(),
            &[
                ("FAKE_GIT_TAGS", tags),
                ("FAKE_CARGO_EXIT", exit),
                ("CLICOLOR_FORCE", ""),
                ("NO_COLOR", ""),
            ],
        );
        assert!(!text(&out).contains('\x1b'), "{}", text(&out));
        let out = run(
            root.path(),
            &["update"],
            tools.path(),
            &[
                ("FAKE_GIT_TAGS", tags),
                ("FAKE_CARGO_EXIT", exit),
                ("NO_COLOR", "1"),
                ("CLICOLOR_FORCE", "1"),
            ],
        );
        assert!(!text(&out).contains('\x1b'), "{}", text(&out));
    }
}

fn managed_copy(root: &std::path::Path) -> std::path::PathBuf {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let filename = format!("fmt-sync{}", std::env::consts::EXE_SUFFIX);
    let executable = bin.join(&filename);
    fs::copy(env!("CARGO_BIN_EXE_fmt-sync"), &executable).unwrap();
    fs::write(
        root.join(".crates.toml"),
        format!("[v1]\n\"fmt-sync 1.1.0 (path+file:///fixture)\" = [\"{filename}\"]\n"),
    )
    .unwrap();
    executable
}

fn run_managed(root: &std::path::Path, tools: &std::path::Path, fail: bool) -> Output {
    let executable = managed_copy(root);
    std::process::Command::new(&executable)
        .arg("update")
        .current_dir(root)
        .env("PATH", tools)
        .env("FAKE_LOG", root.join("tool-log.jsonl"))
        .env("FAKE_GIT_TAGS", "a\trefs/tags/v1.2.0")
        .env("FAKE_CARGO_EXIT", if fail { "101" } else { "0" })
        .env("FAKE_INSTALL_DEST", &executable)
        .env("FAKE_VERIFY_EXISTING_EXE", &executable)
        .env("FAKE_INSTALL_SOURCE", env!("CARGO_BIN_EXE_fmt-sync"))
        .env("FAKE_FORBIDDEN_CWD", root)
        .output()
        .unwrap()
}

#[test]
fn update_preserves_the_active_cargo_install_root_and_runs_outside_the_project() {
    let (root, tools) = setup();
    let out = run_managed(root.path(), tools.path(), false);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let log = calls(root.path());
    let args = log[1]["args"].as_array().unwrap();
    assert_eq!(args[args.len() - 2], "--root");
    assert_eq!(args[args.len() - 1], root.path().to_str().unwrap());
    let executable = root
        .path()
        .join("bin")
        .join(format!("fmt-sync{}", std::env::consts::EXE_SUFFIX));
    let version = std::process::Command::new(executable)
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success(), "{}", text(&version));
    assert_eq!(
        fs::read_dir(root.path().join("bin")).unwrap().count(),
        1,
        "stale update backup was not cleaned on the next invocation"
    );
}

#[test]
fn failed_install_restores_the_running_executable() {
    let (root, tools) = setup();
    let out = run_managed(root.path(), tools.path(), true);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("Update failed"));
    let executable = root
        .path()
        .join("bin")
        .join(format!("fmt-sync{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        fs::read(&executable).unwrap(),
        fs::read(env!("CARGO_BIN_EXE_fmt-sync")).unwrap()
    );
    let version = std::process::Command::new(executable)
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success(), "{}", text(&version));
    assert_eq!(fs::read_dir(root.path().join("bin")).unwrap().count(), 1);
}

#[test]
fn stable_tags_with_build_metadata_can_be_installed() {
    let (root, tools) = setup();
    let out = run(
        root.path(),
        &["update"],
        tools.path(),
        &[("FAKE_GIT_TAGS", "a\trefs/tags/v1.2.0+build.7")],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).contains("Update successful"));
    let log = calls(root.path());
    assert_eq!(log[1]["args"][4], "v1.2.0+build.7");
}
