use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fmt-sync"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn write(root: &Path, path: &str, content: &str) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, content).unwrap();
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn ok(output: Output) {
    assert!(output.status.success(), "{}", text(&output));
}

#[test]
fn init_without_preset_creates_usable_python_defaults() {
    let dir = TempDir::new().unwrap();
    ok(cli(dir.path(), &["init", "--lang", "python"]));
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(dir.path().join("fmt-sync.toml")).unwrap()).unwrap();
    assert_eq!(
        config["languages"].as_array().unwrap()[0].as_str(),
        Some("python")
    );
    let ruff: toml::Value =
        toml::from_str(&fs::read_to_string(dir.path().join("ruff.toml")).unwrap()).unwrap();
    assert_eq!(ruff["line-length"].as_integer(), Some(88));
    assert!(!dir.path().join("pyproject.toml").exists());
}

#[test]
fn init_supports_all_four_languages_without_installing_tools() {
    let dir = TempDir::new().unwrap();
    ok(cli(
        dir.path(),
        &["init", "--lang", "python,go,rust,typescript"],
    ));
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(dir.path().join("fmt-sync.toml")).unwrap()).unwrap();
    assert_eq!(config["languages"].as_array().unwrap().len(), 4);
    assert!(dir.path().join("ruff.toml").is_file());
    assert!(dir.path().join("rustfmt.toml").is_file());
    assert!(dir.path().join("biome.json").is_file());
    assert!(!dir.path().join("go.mod").exists());
    assert!(!dir.path().join("Cargo.toml").exists());
    assert!(!dir.path().join("package.json").exists());
}

#[test]
fn init_preserves_pyproject_ruff_settings() {
    let dir = TempDir::new().unwrap();
    let original = "[project]\nname = 'demo'\n[tool.ruff]\nline-length = 100\n";
    write(dir.path(), "pyproject.toml", original);
    ok(cli(dir.path(), &["init", "--lang", "python"]));
    assert_eq!(
        fs::read_to_string(dir.path().join("pyproject.toml")).unwrap(),
        original
    );
    assert!(!dir.path().join("ruff.toml").exists());
}

#[test]
fn pyproject_without_ruff_does_not_prevent_default_config() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "pyproject.toml", "[project]\nname = 'demo'\n");
    ok(cli(dir.path(), &["init", "--lang", "py"]));
    assert!(dir.path().join("ruff.toml").is_file());
}

#[test]
fn repeated_init_adds_languages_and_keeps_comments_and_ignore() {
    let dir = TempDir::new().unwrap();
    let config = "# personal\nschema-version = 1\nlanguages = ['python'] # keep\nignore = ['legacy/']\ncustom = 'untouched'\n";
    write(dir.path(), "fmt-sync.toml", config);
    write(dir.path(), "ruff.toml", "line-length = 99\n");
    ok(cli(dir.path(), &["init", "--lang", "python,ts"]));
    let updated = fs::read_to_string(dir.path().join("fmt-sync.toml")).unwrap();
    assert!(updated.contains("# personal") && updated.contains("# keep"));
    let parsed: toml::Value = toml::from_str(&updated).unwrap();
    assert_eq!(parsed["ignore"][0].as_str(), Some("legacy/"));
    assert_eq!(parsed["custom"].as_str(), Some("untouched"));
    assert_eq!(parsed["languages"].as_array().unwrap().len(), 2);
    assert_eq!(
        fs::read_to_string(dir.path().join("ruff.toml")).unwrap(),
        "line-length = 99\n"
    );
    ok(cli(dir.path(), &["init", "--lang", "python,typescript"]));
    assert_eq!(
        fs::read_to_string(dir.path().join("fmt-sync.toml")).unwrap(),
        updated
    );
}

#[test]
fn preset_is_copied_and_existing_native_config_wins() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "preset/ruff.toml", "line-length = 120\n");
    write(
        dir.path(),
        "preset/biome.jsonc",
        "{ // preserved\n\"formatter\": {\"enabled\": true},\n}\n",
    );
    write(dir.path(), "ruff.toml", "line-length = 99\n");
    ok(cli(
        dir.path(),
        &[
            "init",
            "--lang",
            "python,typescript,rust",
            "--preset",
            "preset",
        ],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("ruff.toml")).unwrap(),
        "line-length = 99\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("biome.jsonc")).unwrap(),
        fs::read_to_string(dir.path().join("preset/biome.jsonc")).unwrap()
    );
    assert!(dir.path().join("rustfmt.toml").is_file());
}

#[test]
fn malformed_second_language_does_not_create_first_language_files() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "biome.json", "{invalid");
    let out = cli(dir.path(), &["init", "--lang", "python,typescript"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(!dir.path().join("ruff.toml").exists());
    assert!(!dir.path().join("fmt-sync.toml").exists());
}

#[test]
fn preset_external_references_are_rejected_before_writing() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "preset/ruff.toml",
        "extend = '../shared.toml'\n",
    );
    let out = cli(
        dir.path(),
        &["init", "--lang", "python", "--preset", "preset"],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("extend"));
    assert!(!dir.path().join("fmt-sync.toml").exists());
}

#[test]
fn missing_preset_is_not_silently_replaced_by_defaults() {
    let dir = TempDir::new().unwrap();
    let out = cli(dir.path(), &["init", "--lang", "go", "--preset", "missing"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).to_ascii_lowercase().contains("preset"));
}

#[test]
fn runtime_commands_do_not_inherit_parent_config() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['go']\n",
    );
    fs::create_dir(dir.path().join("child")).unwrap();
    for action in ["format", "check"] {
        let out = cli(&dir.path().join("child"), &[action]);
        assert_eq!(out.status.code(), Some(2));
        assert!(text(&out).contains("not initialized"), "{}", text(&out));
    }
}

#[test]
fn runtime_commands_reject_language_flags() {
    let dir = TempDir::new().unwrap();
    for action in ["format", "check"] {
        let out = cli(dir.path(), &[action, "--lang", "python"]);
        assert_eq!(out.status.code(), Some(2));
        assert!(text(&out).contains("--lang"));
    }
}

#[test]
fn eslint_project_is_not_silently_migrated() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "eslint.config.js", "export default [];\n");
    let out = cli(dir.path(), &["init", "--lang", "typescript"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("ESLint"));
    assert!(!dir.path().join("biome.json").exists());
}

#[test]
fn invalid_schema_and_empty_languages_are_rejected() {
    let dir = TempDir::new().unwrap();
    for config in [
        "schema-version = 99\nlanguages = ['go']\n",
        "schema-version = 1\nlanguages = []\n",
    ] {
        write(dir.path(), "fmt-sync.toml", config);
        let out = cli(dir.path(), &["check"]);
        assert_eq!(out.status.code(), Some(2));
        assert!(!text(&out).contains("not implemented"));
    }
}

#[test]
fn tool_version_keys_come_from_registered_tools() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "fmt-sync.toml",
        "schema-version = 1\nlanguages = ['python']\n[tool-versions]\nruff = '1.2.3'\ngo = '1.2.3'\ncargo = '1.2.3'\nrustfmt = '1.2.3'\nclippy = '1.2.3'\nbiome = '1.2.3'\n",
    );
    // With no sources, known requirements are validated without launching tools.
    ok(cli(dir.path(), &["check"]));
    for entry in ["unknown = '1.2.3'", "ruff = ' '"] {
        write(
            dir.path(),
            "fmt-sync.toml",
            &format!("schema-version = 1\nlanguages = ['python']\n[tool-versions]\n{entry}\n"),
        );
        let out = cli(dir.path(), &["check"]);
        assert_eq!(out.status.code(), Some(2));
    }
}

#[test]
fn ignore_rejects_files_absolute_paths_and_escaping_paths() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "main.py", "print('hello')\n");
    for ignored in [
        "main.py",
        "../other",
        "/tmp/other",
        "C:/other",
        "legacy/*",
        ".",
    ] {
        write(
            dir.path(),
            "fmt-sync.toml",
            &format!("schema-version = 1\nlanguages = ['python']\nignore = ['{ignored}']\n"),
        );
        let out = cli(dir.path(), &["init", "--lang", "python"]);
        assert_eq!(
            out.status.code(),
            Some(2),
            "accepted {ignored}: {}",
            text(&out)
        );
        assert!(text(&out).contains("ignore"), "{}", text(&out));
        assert!(!dir.path().join("ruff.toml").exists());
    }
}
