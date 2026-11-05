pub mod hints;

use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Clone, Copy)]
pub struct VersionSpec {
    pub key: &'static str,
    pub args: &'static [&'static str],
}

pub type LocalResolver = fn(&Path) -> Result<Option<Tool>>;

pub struct ToolSpec {
    pub name: &'static str,
    pub version: Option<VersionSpec>,
    pub install_hint: &'static str,
    pub local: Option<LocalResolver>,
}

impl ToolSpec {
    pub fn resolve(&self, root: &Path) -> Result<Tool> {
        if let Some(resolve) = self.local
            && let Some(tool) = resolve(root)?
        {
            return Ok(tool);
        }
        from_path_with_hint(self.name, self.install_hint)
    }
}

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
        spec: &ToolSpec,
        required: Option<&str>,
    ) -> Result<Self> {
        let Some(version) = spec.version else {
            return Ok(self);
        };
        let key = version.key;
        let output = self.output(root, &strings(version.args))?;
        let description = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(
            output.status.success(),
            "Failed to query the {key} version: {}\n{}",
            description.trim(),
            spec.install_hint
        );
        if let Some(required) = required {
            let actual = version_token(&description)
                .context("The tool did not report a recognizable version")?;
            ensure!(
                actual == required,
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

pub fn executable(path: &Path) -> bool {
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
    from_path_with_hint(
        name,
        "Install the tool and add its executable directory to PATH.",
    )
}

pub fn from_path_with_hint(name: &str, hint: &str) -> Result<Tool> {
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
    bail!("Cannot find {name}.\n{}", hint)
}

pub fn append(base: &[OsString], paths: &[OsString]) -> Vec<OsString> {
    base.iter().chain(paths).cloned().collect()
}

pub fn os(value: impl AsRef<OsStr>) -> OsString {
    value.as_ref().to_os_string()
}

pub fn local_executable(path: PathBuf, name: &str) -> Result<Option<Tool>> {
    if !executable(&path) {
        return Ok(None);
    }
    Ok(Some(Tool {
        program: std::path::absolute(path)?,
        prefix: Vec::new(),
        label: name.into(),
    }))
}
