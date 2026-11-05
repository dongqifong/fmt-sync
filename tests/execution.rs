mod support;

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use support::{calls, install_tool, run};
use tempfile::TempDir;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn project(languages: &str) -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix("fmt sync caf\u{e9} ")
        .tempdir()
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fmt-sync"))
        .current_dir(dir.path())
        .args(["init", "--lang", languages])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    dir
}
fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn assert_code(out: &Output, code: i32) {
    assert_eq!(out.status.code(), Some(code), "{}", text(out));
}
fn operations(root: &Path, tool: &str) -> Vec<Vec<String>> {
    calls(root)
        .into_iter()
        .filter(|c| c["tool"] == tool)
        .map(|c| {
            c["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|a| {
            !a.iter().any(|s| s == "--version" || s == "-V")
                && a.first().is_none_or(|s| s != "version")
        })
        .collect()
}

#[test]
fn format_passes_only_owned_nonignored_files_to_both_languages() {
    let dir = project("python,typescript");
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python','typescript']\nignore = ['legacy/']\n",
    );
    for file in [
        "src/hello world.py",
        "src/caf\u{e9}.ts",
        "legacy/no.py",
        "legacy/no.ts",
        "legacy-copy/yes.py",
        "child/no.py",
    ] {
        write(dir.path(), file, "");
    }
    write(dir.path(), "child/fmt-sync.toml", "anything");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    install_tool(tools.path(), "biome");
    let out = run(dir.path(), &["format"], tools.path(), &[]);
    assert_code(&out, 0);
    let ruff = operations(dir.path(), "ruff");
    let biome = operations(dir.path(), "biome");
    assert!(!ruff.is_empty() && !biome.is_empty());
    let paths: Vec<_> = ruff.iter().chain(biome.iter()).flatten().collect();
    assert!(
        paths
            .iter()
            .any(|s| s.replace('\\', "/").ends_with("src/hello world.py"))
    );
    assert!(
        paths
            .iter()
            .any(|s| s.replace('\\', "/").ends_with("src/caf\u{e9}.ts"))
    );
    assert!(
        !paths
            .iter()
            .any(|s| s.contains("no.py") || s.contains("no.ts"))
    );
    assert!(!paths.iter().any(|s| s.as_str() == "."));
}

#[test]
fn local_ruff_is_preferred_over_path() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    let relative = if cfg!(windows) {
        ".venv/Scripts"
    } else {
        ".venv/bin"
    };
    install_tool(&dir.path().join(relative), "ruff");
    let tools = TempDir::new().unwrap();
    let out = run(dir.path(), &["check"], tools.path(), &[]);
    assert_code(&out, 0);
    let commands = operations(dir.path(), "ruff");
    assert!(
        commands
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "format") && a.iter().any(|s| s == "--check"))
    );
    assert!(
        commands
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "check"))
    );
}

#[test]
fn local_biome_native_executable_is_used_without_npx() {
    let dir = project("typescript");
    write(dir.path(), "main.ts", "");
    install_tool(&dir.path().join("node_modules/.bin"), "biome");
    let tools = TempDir::new().unwrap();
    let out = run(dir.path(), &["check"], tools.path(), &[]);
    assert_code(&out, 0);
    assert!(
        operations(dir.path(), "biome")
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "check"))
    );
}

#[test]
fn gofmt_output_is_a_finding_even_with_successful_exit() {
    let dir = project("go");
    write(dir.path(), "main.go", "package main\n");
    write(dir.path(), "go.mod", "module example.test/demo\ngo 1.23\n");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "gofmt");
    install_tool(tools.path(), "go");
    let out = run(
        dir.path(),
        &["check"],
        tools.path(),
        &[("FAKE_GOFMT_DIRTY", "1")],
    );
    assert_code(&out, 1);
    assert!(text(&out).contains("failed"));
    assert!(
        operations(dir.path(), "go")
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "vet"))
    );
}

#[test]
fn go_package_lint_is_not_disabled_by_gitignored_source_filenames() {
    let dir = project("go");
    write(dir.path(), "go.mod", "module example.test/demo\ngo 1.23\n");
    write(
        dir.path(),
        "main.go",
        "package main\nfunc main() { nonexistent() }\n",
    );
    write(dir.path(), ".gitignore", "main.go\n");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "go");
    let out = run(
        dir.path(),
        &["check"],
        tools.path(),
        &[("FAKE_GO_EXIT", "1")],
    );
    assert_code(&out, 1);
    assert!(
        operations(dir.path(), "go")
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "vet"))
    );
}

#[test]
fn ignoring_directory_alias_does_not_ignore_the_real_project() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    let alias = dir.path().join("alias");
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
    #[cfg(windows)]
    {
        let out = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", text(&out));
    }
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python']\nignore = ['alias/']\n",
    );
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    let out = run(dir.path(), &["format"], tools.path(), &[]);
    assert_code(&out, 0);
    assert!(
        operations(dir.path(), "ruff")
            .iter()
            .flatten()
            .any(|s| s.ends_with("main.py")),
        "{}",
        text(&out)
    );
}

#[test]
fn ignoring_directory_alias_does_not_ignore_its_real_source_target() {
    let dir = project("python");
    write(dir.path(), "src/main.py", "");
    let alias = dir.path().join("alias");
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("src"), &alias).unwrap();
    #[cfg(windows)]
    {
        let out = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(dir.path().join("src"))
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", text(&out));
    }
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python']\nignore = ['alias/']\n",
    );
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    let out = run(dir.path(), &["format"], tools.path(), &[]);
    assert_code(&out, 0);
    assert!(
        operations(dir.path(), "ruff")
            .iter()
            .flatten()
            .any(|s| s.ends_with("main.py")),
        "{}",
        text(&out)
    );
}

#[test]
fn missing_tool_takes_precedence_but_other_languages_still_run() {
    let dir = project("python,typescript");
    write(dir.path(), "main.py", "");
    write(dir.path(), "main.ts", "");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "biome");
    let out = run(
        dir.path(),
        &["check"],
        tools.path(),
        &[("FAKE_BIOME_EXIT", "1")],
    );
    assert_code(&out, 2);
    assert!(text(&out).contains("ruff") && text(&out).contains("biome"));
    assert!(!operations(dir.path(), "biome").is_empty());
}

#[test]
fn exact_version_requirement_prevents_tool_execution() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python']\n[tool-versions]\nruff = '1.2.3'\n",
    );
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    let out = run(
        dir.path(),
        &["format"],
        tools.path(),
        &[("FAKE_VERSION", "11.2.3")],
    );
    assert_code(&out, 2);
    assert!(text(&out).contains("1.2.3"));
    assert!(operations(dir.path(), "ruff").is_empty());
}

#[test]
fn empty_python_project_skips_instead_of_scanning_the_current_directory() {
    let dir = project("python");
    let tools = TempDir::new().unwrap();
    let out = run(dir.path(), &["check"], tools.path(), &[]);
    assert_code(&out, 0);
    assert!(text(&out).contains("skipped"));
    assert!(calls(dir.path()).is_empty());
}

#[test]
fn ruff_abnormal_exit_is_an_error_not_a_lint_finding() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    for code in ["2", "101"] {
        let out = run(
            dir.path(),
            &["check"],
            tools.path(),
            &[("FAKE_RUFF_EXIT", code)],
        );
        assert_code(&out, 2);
        assert!(text(&out).contains("error"));
    }
}

fn rust_metadata(root: &Path, source: &str) -> String {
    serde_json::json!({"packages":[{"name":"demo","id":"path+file:///demo#0.1.0","manifest_path":root.join("Cargo.toml"),"edition":"2024","targets":[{"src_path":root.join(source),"edition":"2024","kind":["bin"]}]}],"workspace_members":["path+file:///demo#0.1.0"],"workspace_root":root,"resolve":null}).to_string()
}

#[test]
fn rust_format_preflight_failure_does_not_suppress_clippy() {
    for invalid_native_config in [false, true] {
        let dir = project("rust");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = 'demo'\nversion = '0.1.0'\nedition = '2024'\n",
        );
        if invalid_native_config {
            write(dir.path(), "rustfmt.toml", "invalid = [");
            write(dir.path(), "src/main.rs", "fn main() {}\n");
        } else {
            write(
                dir.path(),
                "src/main.rs",
                "#[path = \"child.rs\"]\nmod child;\nfn main() {}\n",
            );
            write(dir.path(), "src/child.rs", "pub fn hello() {}\n");
        }
        let tools = TempDir::new().unwrap();
        install_tool(tools.path(), "cargo");
        let metadata = rust_metadata(dir.path(), "src/main.rs");
        let out = run(
            dir.path(),
            &["check"],
            tools.path(),
            &[("FAKE_METADATA", &metadata)],
        );
        assert_code(&out, 2);
        let commands = operations(dir.path(), "cargo");
        assert!(
            commands
                .iter()
                .any(|a| a.first().is_some_and(|s| s == "clippy")),
            "{}",
            text(&out)
        );
        assert!(
            !commands
                .iter()
                .any(|a| a.first().is_some_and(|s| s == "fmt"))
        );
        assert!(text(&out).contains("rust format [error]"));
        assert!(text(&out).contains("rust lint [passed]"));
    }
}

#[test]
fn python_import_metadata_failure_still_runs_safe_formatter() {
    let dir = project("python");
    write(dir.path(), "main.py", "import sys\n");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    let out = run(
        dir.path(),
        &["format"],
        tools.path(),
        &[("FAKE_RUFF_BAD_METADATA", "1")],
    );
    assert_code(&out, 2);
    let commands = operations(dir.path(), "ruff");
    assert!(
        commands
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "format")),
        "{}",
        text(&out)
    );
    assert!(
        !commands
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "check"))
    );
    assert!(text(&out).contains("python imports [error]"));
    assert!(text(&out).contains("python format [passed]"));
}

#[test]
fn cargo_commands_select_only_the_current_package() {
    let dir = project("rust");
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'demo'\nversion = '0.1.0'\nedition = '2024'\n",
    );
    write(dir.path(), "src/main.rs", "fn main() {}\n");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "cargo");
    install_tool(tools.path(), "rustfmt");
    let meta = rust_metadata(dir.path(), "src/main.rs");
    let out = run(
        dir.path(),
        &["check"],
        tools.path(),
        &[("FAKE_METADATA", &meta)],
    );
    assert_code(&out, 0);
    let commands = operations(dir.path(), "cargo");
    for name in ["fmt", "clippy"] {
        let command = commands
            .iter()
            .find(|a| a.first().is_some_and(|s| s == name))
            .unwrap();
        assert!(command.windows(2).any(|a| a == ["--package", "demo"]));
        assert!(!command.iter().any(|a| a == "--all" || a == "--workspace"));
    }
}

#[test]
fn cargo_formatter_rejects_ignored_modules_before_any_write() {
    let dir = project("rust");
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['rust']\nignore = ['src/legacy/']\n",
    );
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'demo'\nversion = '0.1.0'\n",
    );
    write(dir.path(), "src/main.rs", "mod legacy;\nfn main() {}\n");
    write(dir.path(), "src/legacy/mod.rs", "pub fn hello( ){}\n");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "cargo");
    install_tool(tools.path(), "rustfmt");
    let meta = rust_metadata(dir.path(), "src/main.rs");
    let out = run(
        dir.path(),
        &["format"],
        tools.path(),
        &[("FAKE_METADATA", &meta)],
    );
    assert_code(&out, 2);
    assert!(text(&out).contains("module") || text(&out).contains("scope"));
    assert!(
        !operations(dir.path(), "cargo")
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "fmt"))
    );
}

#[test]
fn cargo_formatter_rejects_targets_outside_current_project() {
    let dir = project("rust");
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'demo'\nversion = '0.1.0'\n",
    );
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "cargo");
    let meta = rust_metadata(dir.path(), "../outside.rs");
    let out = run(
        dir.path(),
        &["format"],
        tools.path(),
        &[("FAKE_METADATA", &meta)],
    );
    assert_code(&out, 2);
    assert!(text(&out).contains("scope"));
    assert!(
        !operations(dir.path(), "cargo")
            .iter()
            .any(|a| a.first().is_some_and(|s| s == "fmt"))
    );
}

#[test]
fn large_go_file_sets_are_batched_without_losing_targets() {
    let dir = project("go");
    for i in 0..140 {
        write(
            dir.path(),
            &format!("src/long_directory_name_{i:04}/file_with_long_name_{i:04}.go"),
            "package demo\n",
        );
    }
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "gofmt");
    install_tool(tools.path(), "go");
    let out = run(dir.path(), &["format"], tools.path(), &[]);
    assert_code(&out, 0);
    let commands = operations(dir.path(), "gofmt");
    assert!(commands.len() > 1);
    assert_eq!(
        commands
            .iter()
            .flatten()
            .filter(|s| s.ends_with(".go"))
            .count(),
        140
    );
}

#[test]
fn later_successful_batches_do_not_erase_earlier_errors() {
    let dir = project("go");
    write(dir.path(), "000_error.go", "package demo\n");
    for i in 0..140 {
        write(
            dir.path(),
            &format!("src/long_directory_name_{i:04}/file_with_long_name_{i:04}.go"),
            "package demo\n",
        );
    }
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "gofmt");
    install_tool(tools.path(), "go");
    let out = run(
        dir.path(),
        &["format"],
        tools.path(),
        &[("FAKE_FAILURE_PATH", "000_error.go")],
    );
    assert_code(&out, 2);
    let commands = operations(dir.path(), "gofmt");
    assert!(commands.len() > 1);
    assert!(commands[0].iter().any(|arg| arg.ends_with("000_error.go")));
    assert!(
        !commands
            .last()
            .unwrap()
            .iter()
            .any(|arg| arg.ends_with("000_error.go"))
    );
    assert!(text(&out).contains("go format [error]"));
    assert!(text(&out).contains("gofmt: fixture diagnostic"));
}

#[test]
fn missing_tools_include_install_commands_or_official_installation_pages() {
    for (language, source, hint) in [
        ("python", "main.py", "docs.astral.sh/ruff/installation/"),
        ("typescript", "main.ts", "biomejs.dev/installation/"),
        ("go", "main.go", "go.dev/doc/install"),
        ("rust", "src/main.rs", "rust-lang.org/tools/install"),
    ] {
        let dir = project(language);
        write(dir.path(), source, "");
        let tools = TempDir::new().unwrap();
        let out = run(dir.path(), &["format"], tools.path(), &[]);
        assert_code(&out, 2);
        assert!(
            text(&out).contains(hint),
            "missing {language} installation instructions: {}",
            text(&out)
        );
    }
}

#[test]
fn missing_rust_components_include_their_specific_install_commands() {
    for (missing, command) in [
        ("fmt", "rustup component add rustfmt"),
        ("clippy", "rustup component add clippy"),
    ] {
        let dir = project("rust");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = 'demo'\nversion = '0.1.0'\nedition = '2024'\n",
        );
        write(dir.path(), "src/main.rs", "fn main() {}\n");
        let tools = TempDir::new().unwrap();
        install_tool(tools.path(), "cargo");
        let meta = rust_metadata(dir.path(), "src/main.rs");
        let out = run(
            dir.path(),
            &["check"],
            tools.path(),
            &[
                ("FAKE_METADATA", &meta),
                ("FAKE_MISSING_COMPONENT", missing),
            ],
        );
        assert_code(&out, 2);
        assert!(text(&out).contains(command), "{}", text(&out));
    }
}

#[test]
fn commands_use_green_for_success_and_yellow_for_failure() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    for command in ["format", "check"] {
        let out = run(
            dir.path(),
            &[command],
            tools.path(),
            &[("NO_COLOR", ""), ("CLICOLOR_FORCE", "1")],
        );
        assert_code(&out, 0);
        assert!(
            text(&out).contains("fmt-sync: \u{1b}[32mpassed (exit 0)\u{1b}[0m"),
            "Success summary is not green: {:?}",
            text(&out)
        );
    }
    let out = run(
        dir.path(),
        &["check"],
        tools.path(),
        &[
            ("NO_COLOR", ""),
            ("CLICOLOR_FORCE", "1"),
            ("FAKE_RUFF_EXIT", "1"),
        ],
    );
    assert_code(&out, 1);
    assert!(
        text(&out).contains("fmt-sync: \u{1b}[33msee results above (exit 1)\u{1b}[0m"),
        "Failure summary is not yellow: {:?}",
        text(&out)
    );
    let tools_missing = TempDir::new().unwrap();
    let out = run(
        dir.path(),
        &["check"],
        tools_missing.path(),
        &[("NO_COLOR", ""), ("CLICOLOR_FORCE", "1")],
    );
    assert_code(&out, 2);
    assert!(
        text(&out).contains("fmt-sync: \u{1b}[33msee results above (exit 2)\u{1b}[0m"),
        "Error summary is not yellow: {:?}",
        text(&out)
    );
    let unconfigured = TempDir::new().unwrap();
    for command in ["format", "check"] {
        let out = run(
            unconfigured.path(),
            &[command],
            tools.path(),
            &[("NO_COLOR", ""), ("CLICOLOR_FORCE", "1")],
        );
        assert_code(&out, 2);
        assert!(
            String::from_utf8_lossy(&out.stderr).starts_with("\u{1b}[33merror: "),
            "Configuration error is not yellow: {:?}",
            text(&out)
        );
    }
}

#[test]
fn redirected_success_output_and_no_color_output_have_no_ansi_codes() {
    let dir = project("python");
    write(dir.path(), "main.py", "");
    let tools = TempDir::new().unwrap();
    install_tool(tools.path(), "ruff");
    for (no_color, force_color) in [("", ""), ("1", "1")] {
        for command in ["format", "check"] {
            let out = run(
                dir.path(),
                &[command],
                tools.path(),
                &[("NO_COLOR", no_color), ("CLICOLOR_FORCE", force_color)],
            );
            assert_code(&out, 0);
            assert!(text(&out).contains("fmt-sync: passed (exit 0)"));
            assert!(!text(&out).contains('\u{1b}'), "{:?}", text(&out));
            let out = run(
                dir.path(),
                &[command],
                tools.path(),
                &[
                    ("NO_COLOR", no_color),
                    ("CLICOLOR_FORCE", force_color),
                    ("FAKE_RUFF_EXIT", "1"),
                ],
            );
            assert_code(&out, 1);
            assert!(text(&out).contains("fmt-sync: see results above (exit 1)"));
            assert!(!text(&out).contains('\u{1b}'), "{:?}", text(&out));
        }
    }
}
