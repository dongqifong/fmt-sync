//! Real Git/Cargo updates against local releases, without network or user installs.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use semver::Version;
use tempfile::TempDir;

const OFFICIAL: &str = "https://github.com/dongqifong/fmt-sync.git";

struct Sandbox {
    directory: TempDir,
    repository: PathBuf,
    install_root: PathBuf,
    executable: PathBuf,
    git_config: PathBuf,
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn checked(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{command:?}\n{}", text(&output));
    output
}

fn file_url(path: &Path) -> String {
    let path = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let path = path
        .strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .replace('\\', "/");
    let encoded = path
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    format!(
        "file://{}{encoded}",
        if encoded.starts_with('/') { "" } else { "/" }
    )
}

impl Sandbox {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("fmt update real ")
            .tempdir()
            .unwrap();
        let repository = directory.path().join("releases");
        let install_root = directory.path().join("installation");
        fs::create_dir_all(repository.join("src")).unwrap();
        fs::create_dir_all(install_root.join("bin")).unwrap();
        let filename = format!("fmt-sync{}", std::env::consts::EXE_SUFFIX);
        let executable = install_root.join("bin").join(&filename);
        fs::copy(env!("CARGO_BIN_EXE_fmt-sync"), &executable).unwrap();
        fs::write(
            install_root.join(".crates.toml"),
            format!(
                "[v1]\n\"fmt-sync {} (path+file:///old-installation)\" = [\"{filename}\"]\n",
                env!("CARGO_PKG_VERSION")
            ),
        )
        .unwrap();
        let git_config = directory.path().join("gitconfig");
        fs::write(&git_config, "").unwrap();
        let sandbox = Self {
            directory,
            repository,
            install_root,
            executable,
            git_config,
        };
        checked(sandbox.git().args(["init", "--quiet"]));
        checked(
            sandbox
                .git()
                .args(["config", "user.name", "fmt-sync tests"]),
        );
        checked(
            sandbox
                .git()
                .args(["config", "user.email", "tests@example.invalid"]),
        );
        checked(sandbox.git().args(["config", "commit.gpgsign", "false"]));
        checked(sandbox.git().args(["config", "tag.gpgsign", "false"]));
        checked(
            sandbox
                .git()
                .arg("config")
                .arg("--file")
                .arg(&sandbox.git_config)
                .arg(format!("url.{}.insteadOf", file_url(&sandbox.repository)))
                .arg(OFFICIAL),
        );
        sandbox.release(env!("CARGO_PKG_VERSION"), false);
        sandbox
    }

    fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(&self.repository)
            .env("GIT_CONFIG_GLOBAL", &self.git_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ALLOW_PROTOCOL", "file")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("CARGO_HOME", self.directory.path().join("cargo-home"))
            .env("CARGO_TARGET_DIR", self.directory.path().join("build"))
            .env(
                "CARGO_BUILD_BUILD_DIR",
                self.directory.path().join("intermediates"),
            )
            .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
            .env(
                "CARGO_INSTALL_ROOT",
                self.directory.path().join("unused-install-root"),
            )
            .env("NO_COLOR", "1");
        command
    }

    fn git(&self) -> Command {
        self.command("git")
    }

    fn release(&self, version: &str, broken: bool) {
        fs::write(
            self.repository.join("Cargo.toml"),
            format!("[package]\nname = 'fmt-sync'\nversion = '{version}'\nedition = '2024'\n"),
        )
        .unwrap();
        fs::write(
            self.repository.join("src/main.rs"),
            if broken {
                "fn main() { missing_function(); }"
            } else {
                "fn main() { println!(\"fmt-sync {}\", env!(\"CARGO_PKG_VERSION\")); }"
            },
        )
        .unwrap();
        checked(self.command("cargo").arg("generate-lockfile"));
        checked(
            self.git()
                .args(["add", "Cargo.toml", "Cargo.lock", "src/main.rs"]),
        );
        checked(self.git().args(["commit", "--quiet", "-m", version]));
        checked(self.git().arg("tag").arg(format!("v{version}")));
    }

    fn update(&self) -> Output {
        self.command(&self.executable)
            .arg("update")
            .output()
            .unwrap()
    }

    fn installed_version(&self) -> String {
        let output = checked(self.command(&self.executable).arg("--version"));
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

fn next_version() -> String {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    format!("{}.{}.0", current.major, current.minor + 1)
}

#[test]
#[ignore = "requires actual Git and Cargo"]
fn real_update_compiles_a_local_release_and_updates_cargo_metadata() {
    let sandbox = Sandbox::new();
    let next = next_version();
    sandbox.release(&next, false);
    let output = sandbox.update();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(
        text(&output).contains("Update successful"),
        "{}",
        text(&output)
    );
    assert_eq!(sandbox.installed_version(), format!("fmt-sync {next}"));
    let metadata = fs::read_to_string(sandbox.install_root.join(".crates.toml")).unwrap();
    assert!(
        metadata.contains(&format!("fmt-sync {next} (git+{OFFICIAL}")),
        "{metadata}"
    );
    assert!(
        !sandbox
            .directory
            .path()
            .join("unused-install-root")
            .exists()
    );
    // This tiny release only reports its version; backup cleanup by a full
    // fmt-sync release is exercised separately in tests/update.rs.
}

#[test]
#[ignore = "requires actual Git and Cargo"]
fn real_update_skips_compilation_when_the_local_release_is_current() {
    let sandbox = Sandbox::new();
    let before = fs::read(&sandbox.executable).unwrap();
    let output = sandbox.update();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(
        text(&output).contains("Already up to date"),
        "{}",
        text(&output)
    );
    assert_eq!(fs::read(&sandbox.executable).unwrap(), before);
    assert!(!sandbox.directory.path().join("build").exists());
    assert!(!sandbox.directory.path().join("cargo-home/git").exists());
}

#[test]
#[ignore = "requires actual Git and Cargo"]
fn real_update_restores_the_old_executable_when_compilation_fails() {
    let sandbox = Sandbox::new();
    let before = fs::read(&sandbox.executable).unwrap();
    sandbox.release(&next_version(), true);
    let output = sandbox.update();
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert!(text(&output).contains("Update failed"), "{}", text(&output));
    assert!(
        text(&output).contains("missing_function"),
        "{}",
        text(&output)
    );
    assert_eq!(fs::read(&sandbox.executable).unwrap(), before);
    assert_eq!(
        sandbox.installed_version(),
        format!("fmt-sync {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        fs::read_dir(sandbox.install_root.join("bin"))
            .unwrap()
            .count(),
        1
    );
}
