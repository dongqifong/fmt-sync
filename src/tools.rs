use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::config::Config;
use anyhow::{Context, Result, bail, ensure};

#[derive(Clone)]
pub struct Tool {
    pub program: PathBuf,
    pub prefix: Vec<OsString>,
    pub label: String,
}

impl Tool {
    pub fn output(&self, root: &Path, args: &[OsString]) -> Result<Output> {
        Command::new(&self.program)
            .args(&self.prefix)
            .args(args)
            .current_dir(root)
            .output()
            .with_context(|| format!("Failed to start {}", self.program.display()))
    }

    pub fn versioned(
        mut self,
        root: &Path,
        args: &[&str],
        key: &str,
        config: &Config,
    ) -> Result<Self> {
        let output = self.output(root, &strings(args))?;
        let description = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(
            output.status.success(),
            "Failed to query the {key} version: {}\n{}",
            description.trim(),
            installation_hint(key)
        );
        if let Some(required) = config.tool_versions.get(key) {
            let actual = version_token(&description)
                .context("The tool did not report a recognizable version")?;
            ensure!(
                actual == *required,
                "{key} version mismatch: required {required}, found {actual}"
            );
        }
        self.label = format!("{} ({})", self.program.display(), description.trim());
        Ok(self)
    }
}

pub fn strings(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn version_token(description: &str) -> Option<String> {
    description.split_whitespace().find_map(|token| {
        let token = token
            .strip_prefix("go")
            .unwrap_or(token)
            .trim_start_matches('v');
        (token.as_bytes().first().is_some_and(u8::is_ascii_digit) && token.contains('.'))
            .then(|| token.to_string())
    })
}

fn executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn from_path(name: &str) -> Result<Tool> {
    let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        if directory.as_os_str().is_empty() {
            continue;
        }
        let candidate = directory.join(&filename);
        if executable(&candidate) {
            // Preserve argv[0]: rustup and other multicall shims dispatch by the symlink name.
            return Ok(Tool {
                program: std::path::absolute(candidate)?,
                prefix: Vec::new(),
                label: name.into(),
            });
        }
    }
    bail!("Cannot find {name}.\n{}", installation_hint(name))
}

fn installation_hint(name: &str) -> &'static str {
    match name {
        "ruff" => {
            concat!(
                "Install Ruff using ONE of these options:\n\n",
                "Project dependency (run in the project directory):\n",
                "  uv add --dev ruff\n\n",
                "Shared user tool (available across projects via PATH):\n",
                "  uv tool install ruff\n\n",
                "Current Python environment:\n",
                "  python -m pip install ruff\n",
                "  Uses the active virtual environment, if any; otherwise that Python's environment.\n\n",
                "fmt-sync searches the project's .venv first, then PATH.\n",
                "Installation guide: https://docs.astral.sh/ruff/installation/"
            )
        }
        "biome" => {
            concat!(
                "Install Biome using ONE of these options:\n\n",
                "Project dev dependency (recommended; run in the project directory):\n",
                "  npm install --save-dev --save-exact @biomejs/biome\n",
                "  Requires Node.js/npm; installs into the project's node_modules.\n\n",
                "Shared native executable (Node.js not required):\n",
                "  Download Biome for your OS/architecture and add its directory to PATH.\n",
                "  Use the executable name biome.exe on Windows or biome on Linux.\n\n",
                "fmt-sync searches the project's node_modules first, then PATH.\n",
                "Installation guide: https://biomejs.dev/installation/quick-start/"
            )
        }
        "cargo" => {
            concat!(
                "Rust toolchain (shared per user):\n",
                "  Install Rust with rustup and add Cargo's bin directory to PATH.\n",
                "  Installation guide: https://rust-lang.org/tools/install/\n\n",
                "Toolchain components (run in the project directory):\n",
                "  rustup component add rustfmt clippy\n",
                "  Installs into the active toolchain, shared by projects using it.\n",
                "  These are toolchain components, not Cargo.toml dependencies.\n\n",
                "fmt-sync finds cargo on PATH; Cargo/rustup select the project's active toolchain."
            )
        }
        "rustfmt" => {
            concat!(
                "Rustfmt component (run in the project directory):\n",
                "  rustup component add rustfmt\n\n",
                "Installation scope:\n",
                "  Installs into the active Rust toolchain, shared by projects using it.\n",
                "  If the project pins a toolchain, the component must be installed for it.\n",
                "  This is a toolchain component, not a Cargo.toml dependency.\n\n",
                "fmt-sync runs cargo fmt with the project's active toolchain.\n",
                "Installation guide: https://github.com/rust-lang/rustfmt#quick-start"
            )
        }
        "clippy" => {
            concat!(
                "Clippy component (run in the project directory):\n",
                "  rustup component add clippy\n\n",
                "Installation scope:\n",
                "  Installs into the active Rust toolchain, shared by projects using it.\n",
                "  If the project pins a toolchain, the component must be installed for it.\n",
                "  This is a toolchain component, not a Cargo.toml dependency.\n\n",
                "fmt-sync runs cargo clippy with the project's active toolchain.\n",
                "Installation guide: https://doc.rust-lang.org/clippy/installation.html"
            )
        }
        "go" | "gofmt" => {
            concat!(
                "Go toolchain (shared across projects):\n",
                "  Install Go using the official instructions for your OS.\n",
                "  Includes gofmt for formatting and go vet for lint; no separate install needed.\n\n",
                "PATH setup:\n",
                "  Add the Go installation's bin directory containing go and gofmt to PATH.\n",
                "  Windows default: C:\\Program Files\\Go\\bin\n",
                "  Linux typical: /usr/local/go/bin\n",
                "  Reopen the terminal after changing PATH.\n\n",
                "fmt-sync finds go and gofmt on PATH; they are not project dependencies.\n",
                "Installation guide: https://go.dev/doc/install"
            )
        }
        "node" => {
            concat!(
                "Shared Node.js runtime (required by this local Biome installation):\n",
                "  Install Node.js and add its executable directory to PATH.\n",
                "  Official download: https://nodejs.org/en/download\n\n",
                "Project Biome dependency (after installing Node.js/npm):\n",
                "  npm install --save-dev --save-exact @biomejs/biome\n",
                "  Run in the project directory; installs into node_modules.\n\n",
                "fmt-sync prefers local native Biome, which does not require Node.js.\n",
                "The local JavaScript entry point requires node on PATH.\n",
                "Biome installation guide: https://biomejs.dev/installation/quick-start/"
            )
        }
        _ => "Install the tool and add its executable directory to PATH.",
    }
}

pub fn resolve(root: &Path, name: &str) -> Result<Tool> {
    let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
    let mut local = Vec::new();
    if name == "ruff" {
        local.push(
            root.join(if cfg!(windows) {
                ".venv/Scripts"
            } else {
                ".venv/bin"
            })
            .join(&filename),
        );
    }
    if name == "biome" {
        local.push(root.join("node_modules/.bin").join(&filename));
        let arch = match env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            x => x,
        };
        let os = if cfg!(windows) {
            "win32"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        local.push(root.join(format!("node_modules/@biomejs/cli-{os}-{arch}/{filename}")));
        if cfg!(target_os = "linux") && cfg!(target_env = "musl") {
            local.insert(
                0,
                root.join(format!(
                    "node_modules/@biomejs/cli-linux-{arch}-musl/{filename}"
                )),
            );
        }
    }
    for candidate in local {
        if executable(&candidate) {
            return Ok(Tool {
                program: std::path::absolute(candidate)?,
                prefix: Vec::new(),
                label: name.into(),
            });
        }
    }
    if name == "biome" {
        let entry = root.join("node_modules/@biomejs/biome/bin/biome");
        if entry.is_file() {
            let mut node = from_path("node")?;
            node.prefix.push(entry.into_os_string());
            return Ok(node);
        }
    }
    from_path(name)
}

// A conservative bound leaves room for UTF-16 expansion, quoting, and the executable path.
pub fn batches(items: &[PathBuf]) -> Vec<Vec<OsString>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut size = 0;
    for item in items {
        let arg = Path::new(".").join(item).into_os_string();
        let cost = arg.to_string_lossy().encode_utf16().count() * 2 + 4;
        if size + cost > 6000 && !current.is_empty() {
            batches.push(std::mem::take(&mut current));
            size = 0;
        }
        size += cost;
        current.push(arg);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

pub fn append(base: &[OsString], paths: &[OsString]) -> Vec<OsString> {
    base.iter().chain(paths).cloned().collect()
}

pub fn os(value: impl AsRef<OsStr>) -> OsString {
    value.as_ref().to_os_string()
}
