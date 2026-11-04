//! Actual external tools, separately invoked with `cargo test --test smoke -- --ignored`.
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn command(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fmt-sync"));
    cmd.current_dir(root).args(args);
    if let Some(extra) = std::env::var_os("FMT_SYNC_SMOKE_TOOL_PATH") {
        let paths = std::env::split_paths(&extra)
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ))
            .collect::<Vec<_>>();
        cmd.env("PATH", std::env::join_paths(paths).unwrap());
    }
    cmd
}
fn run(root: &Path, args: &[&str]) -> Output {
    command(root, args).output().unwrap()
}
fn run_rust(root: &Path, args: &[&str]) -> Output {
    command(root, args)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .env_remove("CARGO_BUILD_BUILD_DIR")
        .output()
        .unwrap()
}
fn expect(out: Output, code: i32) {
    assert_eq!(
        out.status.code(),
        Some(code),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "requires actual Ruff, Go and Biome executables on PATH"]
fn real_multilingual_format_and_check_preserve_excluded_projects() {
    let dir = tempfile::Builder::new()
        .prefix("fmt smoke caf\u{e9} ")
        .tempdir()
        .unwrap();
    expect(
        run(dir.path(), &["init", "--lang", "python,typescript,go"]),
        0,
    );
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python','typescript','go']\nignore = ['legacy/']\n",
    );
    write(
        dir.path(),
        "app.py",
        "def greeting( name ):\n return 'hello '+name\n",
    );
    write(
        dir.path(),
        "app.ts",
        "export function increment(value:number){return value+1}\n",
    );
    write(
        dir.path(),
        "go.mod",
        "module example.test/smoke\n\ngo 1.23\n",
    );
    write(
        dir.path(),
        "main.go",
        "package main\nfunc main( ){println(\"ok\")}\n",
    );
    for file in [
        "legacy/bad.py",
        "legacy/bad.ts",
        "legacy/bad.go",
        "independent/bad.py",
    ] {
        write(dir.path(), file, "not valid source!!!\n");
    }
    write(
        dir.path(),
        "independent/fmt-sync.toml",
        "independent scope\n",
    );
    expect(run(dir.path(), &["check"]), 1);
    expect(run(dir.path(), &["format"]), 0);
    expect(run(dir.path(), &["check"]), 0);
    for file in [
        "legacy/bad.py",
        "legacy/bad.ts",
        "legacy/bad.go",
        "independent/bad.py",
    ] {
        assert_eq!(
            fs::read_to_string(dir.path().join(file)).unwrap(),
            "not valid source!!!\n"
        );
    }
    let first = fs::read(dir.path().join("app.py")).unwrap();
    expect(run(dir.path(), &["format"]), 0);
    assert_eq!(fs::read(dir.path().join("app.py")).unwrap(), first);
}

#[test]
#[ignore = "requires actual Cargo, rustfmt and Clippy"]
fn real_rust_package_does_not_format_other_workspace_members() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'smoke'\nversion = '0.1.0'\nedition = '2024'\n[workspace]\nmembers = ['.', 'legacy']\n",
    );
    write(
        dir.path(),
        "src/main.rs",
        "fn main( ){println!(\"hello\");}\n",
    );
    write(
        dir.path(),
        "legacy/Cargo.toml",
        "[package]\nname = 'legacy'\nversion = '0.1.0'\nedition = '2024'\n",
    );
    let untouched = "pub fn hello( ){println!(\"legacy\");}\n";
    write(dir.path(), "legacy/src/lib.rs", untouched);
    expect(run(dir.path(), &["init", "--lang", "rust"]), 0);
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['rust']\nignore = ['legacy/']\n",
    );
    expect(run(dir.path(), &["check"]), 1);
    expect(run(dir.path(), &["format"]), 0);
    expect(run(dir.path(), &["check"]), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("legacy/src/lib.rs")).unwrap(),
        untouched
    );
}

fn rust_scope_project(main: &str, module: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'scope_test'\nversion = '0.1.0'\nedition = '2024'\n",
    );
    write(dir.path(), "src/main.rs", main);
    write(
        dir.path(),
        &format!("src/{module}/mod.rs"),
        "pub fn hello( ){ }\n",
    );
    expect(run(dir.path(), &["init", "--lang", "rust"]), 0);
    write(
        dir.path(),
        "fmt-sync.toml",
        &format!("schema-version = 1\nlanguages = ['rust']\nignore = ['src/{module}/']\n"),
    );
    dir
}

#[test]
#[ignore = "requires actual Cargo and rustfmt"]
fn real_rust_scope_rejects_raw_identifier_modules_before_formatting() {
    let dir = rust_scope_project("mod r#type;\nfn main() {}\n", "type");
    expect(run(dir.path(), &["format"]), 2);
    assert_eq!(
        fs::read_to_string(dir.path().join("src/type/mod.rs")).unwrap(),
        "pub fn hello( ){ }\n"
    );
}

#[test]
#[ignore = "requires actual Cargo and rustfmt"]
fn real_rust_scope_rejects_module_bearing_macros_before_formatting() {
    let dir = rust_scope_project(
        "cfg_if::cfg_if! { if #[cfg(unix)] { mod child; } }\nfn main() {}\n",
        "child",
    );
    expect(run(dir.path(), &["format"]), 2);
    assert_eq!(
        fs::read_to_string(dir.path().join("src/child/mod.rs")).unwrap(),
        "pub fn hello( ){ }\n"
    );
}

#[test]
#[ignore = "requires actual Go"]
fn real_go_package_lint_includes_gitignored_source_files() {
    let dir = TempDir::new().unwrap();
    expect(run(dir.path(), &["init", "--lang", "go"]), 0);
    write(dir.path(), "go.mod", "module example.test/demo\ngo 1.23\n");
    write(
        dir.path(),
        "main.go",
        "package main\nfunc main() { nonexistent() }\n",
    );
    write(dir.path(), ".gitignore", "main.go\n");
    expect(run(dir.path(), &["check"]), 1);
}

#[test]
#[ignore = "requires actual Ruff"]
fn real_python_native_gitignore_override_is_respected() {
    let dir = TempDir::new().unwrap();
    expect(run(dir.path(), &["init", "--lang", "python"]), 0);
    write(dir.path(), "ruff.toml", "respect-gitignore = false\n");
    write(dir.path(), ".gitignore", "main.py\n");
    let original = "x=  1\n";
    write(dir.path(), "main.py", original);
    expect(run(dir.path(), &["format"]), 0);
    assert_ne!(
        fs::read_to_string(dir.path().join("main.py")).unwrap(),
        original
    );
}

#[test]
#[ignore = "requires actual Ruff"]
fn real_python_defaults_sort_imports_without_fixing_other_lints() {
    let dir = TempDir::new().unwrap();
    expect(run(dir.path(), &["init", "--lang", "python"]), 0);
    let original = "import sys\nimport os\n\nprint(sys.platform)\n";
    write(dir.path(), "main.py", original);
    let output = run(dir.path(), &["check"]);
    let detail = String::from_utf8_lossy(&output.stdout).into_owned();
    expect(output, 1);
    assert!(
        detail.contains("I001"),
        "Imports were not checked: {detail}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("main.py")).unwrap(),
        original
    );

    expect(run(dir.path(), &["format"]), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.py")).unwrap(),
        "import os\nimport sys\n\nprint(sys.platform)\n"
    );
    let output = run(dir.path(), &["check"]);
    let detail = String::from_utf8_lossy(&output.stdout).into_owned();
    expect(output, 1);
    assert!(
        detail.contains("F401"),
        "The unused import was removed: {detail}"
    );
    assert!(
        !detail.contains("I001"),
        "Imports are still unsorted: {detail}"
    );

    write(
        dir.path(),
        "main.py",
        "import os\nimport sys\n\nprint(os.name, sys.platform)\n",
    );
    expect(run(dir.path(), &["check"]), 0);
    assert!(!dir.path().join(".ruff_cache").exists());

    fs::remove_file(dir.path().join("ruff.toml")).unwrap();
    write(
        dir.path(),
        "main.py",
        "import sys\nimport os\n\nprint(os.name, sys.platform)\n",
    );
    expect(run(dir.path(), &["check"]), 1);
    expect(run(dir.path(), &["format"]), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("main.py")).unwrap(),
        "import os\nimport sys\n\nprint(os.name, sys.platform)\n"
    );
    expect(run(dir.path(), &["check"]), 0);
    assert!(!dir.path().join(".ruff_cache").exists());
}

#[test]
#[ignore = "requires actual Ruff"]
fn real_python_import_sorting_respects_native_configuration() {
    for (configuration, sorted) in [
        (
            "[lint]\nselect = ['F', 'I']\nextend-fixable = ['F']\n",
            true,
        ),
        ("[lint]\nselect = ['F']\n", false),
        (
            "[lint]\nselect = ['F401', 'I001']\nextend-fixable = ['F']\n",
            true,
        ),
        ("[lint]\nselect = ['F', 'I']\nignore = ['I']\n", false),
        ("[lint]\nselect = ['F', 'I']\nunfixable = ['I']\n", false),
        ("[lint]\nselect = ['F', 'I']\nfixable = []\n", false),
        (
            "fix = true\nfix-only = true\n[lint]\nselect = ['F', 'I']\n",
            true,
        ),
    ] {
        let dir = TempDir::new().unwrap();
        expect(run(dir.path(), &["init", "--lang", "python"]), 0);
        write(dir.path(), "ruff.toml", configuration);
        let original = "import sys\nimport os\n";
        write(dir.path(), "main.py", original);
        expect(run(dir.path(), &["check"]), 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("main.py")).unwrap(),
            original
        );
        expect(run(dir.path(), &["format"]), 0);
        assert_eq!(
            fs::read_to_string(dir.path().join("main.py")).unwrap(),
            if sorted {
                "import os\nimport sys\n"
            } else {
                original
            },
            "Unexpected import changes with configuration: {configuration}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("ruff.toml")).unwrap(),
            configuration
        );
        assert!(!dir.path().join(".ruff_cache").exists());
    }
}

#[test]
#[ignore = "requires actual Ruff"]
fn real_python_commands_do_not_create_cache() {
    let dir = TempDir::new().unwrap();
    expect(run(dir.path(), &["init", "--lang", "python"]), 0);
    write(dir.path(), "main.py", "import os\nvalue= 1\n");

    for (command, code) in [("check", 1), ("format", 0), ("check", 1)] {
        expect(run(dir.path(), &[command]), code);
        assert!(
            !dir.path().join(".ruff_cache").exists(),
            "{command} created a Ruff cache"
        );
    }

    write(dir.path(), "main.py", "value = 1\n");
    expect(run(dir.path(), &["check"]), 0);
    assert!(!dir.path().join(".ruff_cache").exists());

    write(dir.path(), "ruff.toml", "cache-dir = 'custom-cache'\n");
    for command in ["format", "check"] {
        expect(run(dir.path(), &[command]), 0);
        assert!(!dir.path().join("custom-cache").exists());
        assert!(!dir.path().join(".ruff_cache").exists());
    }
}

#[test]
#[ignore = "requires actual Ruff"]
fn real_python_commands_preserve_existing_cache() {
    let dir = TempDir::new().unwrap();
    expect(run(dir.path(), &["init", "--lang", "python"]), 0);
    write(dir.path(), "main.py", "value= 1\n");
    write(dir.path(), ".ruff_cache/keep.txt", "existing user cache\n");

    for command in ["format", "check"] {
        expect(run(dir.path(), &[command]), 0);
        let entries = fs::read_dir(dir.path().join(".ruff_cache"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, ["keep.txt"]);
        assert_eq!(
            fs::read_to_string(dir.path().join(".ruff_cache/keep.txt")).unwrap(),
            "existing user cache\n"
        );
    }
}

#[test]
#[ignore = "requires actual Cargo, rustfmt and Clippy"]
fn real_rust_commands_retain_and_reuse_project_build_artifacts() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'cache_probe'\nversion = '0.1.0'\nedition = '2024'\n",
    );
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn answer() -> u8 {\n    42\n}\n",
    );
    write(
        dir.path(),
        "build.rs",
        concat!(
            "fn main() {\n",
            "    let output = std::env::var(\"OUT_DIR\").unwrap();\n",
            "    let marker = std::path::Path::new(&output).join(\"build-count.txt\");\n",
            "    let count = std::fs::read_to_string(&marker).unwrap_or_default();\n",
            "    let count = count.parse::<u32>().unwrap_or(0) + 1;\n",
            "    std::fs::write(marker, count.to_string()).unwrap();\n",
            "    println!(\"cargo:rerun-if-changed=build.rs\");\n",
            "    println!(\"cargo:warning=fmt-sync-test-output={output}\");\n",
            "}\n"
        ),
    );
    expect(run(dir.path(), &["init", "--lang", "rust"]), 0);
    expect(run_rust(dir.path(), &["format"]), 0);
    let output = run_rust(dir.path(), &["check"]);
    let detail = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    expect(output, 0);
    let target = dir.path().join("target");
    assert!(
        target.is_dir(),
        "Rust check did not retain target: {detail}"
    );
    let build_output = detail
        .lines()
        .find_map(|line| line.split_once("fmt-sync-test-output="))
        .map(|(_, path)| Path::new(path.trim()).to_path_buf())
        .expect("The build script did not report its output");
    assert!(
        build_output
            .canonicalize()
            .unwrap()
            .starts_with(target.canonicalize().unwrap())
    );
    let marker = build_output.join("build-count.txt");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "1");
    expect(run_rust(dir.path(), &["check"]), 0);
    expect(run_rust(dir.path(), &["format"]), 0);
    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        "1",
        "The unchanged build script was rebuilt"
    );
    for source in [
        "pub fn count(values: &[u8]) -> usize {\n    values.iter().count()\n}\n",
        "pub fn answer() -> u8 {\n    missing_value\n}\n",
    ] {
        write(dir.path(), "src/lib.rs", source);
        expect(run_rust(dir.path(), &["check"]), 1);
        assert!(target.is_dir(), "A failed check removed target");
        assert_eq!(fs::read_to_string(&marker).unwrap(), "1");
    }
}

#[test]
#[ignore = "requires actual Cargo, rustfmt and Clippy"]
fn real_rust_commands_use_configured_build_directories() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname = 'cache_preservation'\nversion = '0.1.0'\nedition = '2024'\n",
    );
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn answer() -> u8 {\n    42\n}\n",
    );
    write(
        dir.path(),
        ".cargo/config.toml",
        "[build]\ntarget-dir = 'configured-target'\nbuild-dir = 'configured-build'\n",
    );
    let directories = [
        "target",
        "configured-target",
        "configured-build",
        "environment-target",
        "environment-build",
    ];
    for directory in directories {
        write(
            dir.path(),
            &format!("{directory}/keep.txt"),
            "existing user build output\n",
        );
    }
    expect(run(dir.path(), &["init", "--lang", "rust"]), 0);
    expect(run_rust(dir.path(), &["format"]), 0);
    expect(run_rust(dir.path(), &["check"]), 0);
    assert!(dir.path().join("configured-target/debug").is_dir());
    assert!(dir.path().join("configured-build/debug").is_dir());
    let output = command(dir.path(), &["check"])
        .env("CARGO_TARGET_DIR", dir.path().join("environment-target"))
        .env(
            "CARGO_BUILD_BUILD_DIR",
            dir.path().join("environment-build"),
        )
        .output()
        .unwrap();
    expect(output, 0);
    assert!(dir.path().join("environment-target/debug").is_dir());
    assert!(dir.path().join("environment-build/debug").is_dir());
    for directory in directories {
        assert_eq!(
            fs::read_to_string(dir.path().join(directory).join("keep.txt")).unwrap(),
            "existing user build output\n"
        );
    }
}

#[test]
#[ignore = "requires actual Go and Biome"]
fn real_go_and_typescript_commands_do_not_leave_project_cache() {
    for (language, file, broken, fixed) in [
        (
            "go",
            "main.go",
            "package main\nimport \"fmt\"\nfunc main(){fmt.Printf(\"%d\", \"wrong type\")}\n",
            "package main\n\nimport \"fmt\"\n\nfunc main() { fmt.Printf(\"%d\", 42) }\n",
        ),
        (
            "typescript",
            "main.ts",
            "export function matches(value:number){return value==1}\n",
            "export function matches(value: number) { return value === 1; }\n",
        ),
    ] {
        let dir = TempDir::new().unwrap();
        expect(run(dir.path(), &["init", "--lang", language]), 0);
        if language == "go" {
            write(dir.path(), "go.mod", "module example.test/cache\ngo 1.23\n");
        }
        write(dir.path(), file, broken);
        write(
            dir.path(),
            "existing-cache/keep.txt",
            "existing user cache\n",
        );
        let entries = || {
            fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let before = entries();
        for (command, code) in [("check", 1), ("format", 0), ("check", 1)] {
            expect(run(dir.path(), &[command]), code);
            assert_eq!(entries(), before, "{language} {command} left new files");
        }
        write(dir.path(), file, fixed);
        expect(run(dir.path(), &["format"]), 0);
        expect(run(dir.path(), &["check"]), 0);
        assert_eq!(entries(), before, "{language} left new files");
        assert_eq!(
            fs::read_to_string(dir.path().join("existing-cache/keep.txt")).unwrap(),
            "existing user cache\n"
        );
    }
}
