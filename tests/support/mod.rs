use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
};
use tempfile::TempDir;

static FIXTURE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();

pub fn install_tool(directory: &Path, name: &str) -> PathBuf {
    let (_, binary) = FIXTURE.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let binary = dir
            .path()
            .join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
        let result = Command::new("rustc")
            .arg("--edition=2024")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/tool.rs"))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        (dir, binary)
    });
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    fs::copy(binary, &path).unwrap();
    path
}

pub fn run(root: &Path, args: &[&str], tools: &Path, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fmt-sync"));
    cmd.current_dir(root)
        .args(args)
        .env("PATH", tools)
        .env("FAKE_LOG", root.join("tool-log.jsonl"));
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().unwrap()
}

pub fn calls(root: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(root.join("tool-log.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
