use std::{fs, path::PathBuf, process::Command};

use anyhow::{Context, Result, ensure};
use clap::builder::styling::AnsiColor;
use semver::Version;

use crate::tools;

const REPOSITORY: &str = "https://github.com/dongqifong/fmt-sync.git";

pub fn run() -> i32 {
    match update() {
        Ok(()) => 0,
        Err(error) => {
            let red = AnsiColor::Red.on_default();
            anstream::eprintln!("{red}Update failed: {error:#}{red:#}");
            2
        }
    }
}

fn update() -> Result<()> {
    let git = tools::from_path("git")
        .context("Install Git and add it to PATH: https://git-scm.com/downloads")?;
    let work = tempfile::tempdir().context("Failed to create an update working directory")?;
    println!("Checking for updates from {REPOSITORY}...");
    let output = Command::new(&git.program)
        .args(["ls-remote", "--tags", "--refs", REPOSITORY])
        .env("GIT_TERMINAL_PROMPT", "0")
        .current_dir(work.path())
        .output()
        .context("Failed to query release tags with Git")?;
    ensure!(
        output.status.success(),
        "Failed to query release tags ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let (latest, tag) = String::from_utf8(output.stdout)
        .context("Git returned non-UTF-8 release tags")?
        .lines()
        .filter_map(|line| {
            let (_, reference) = line.split_once('\t')?;
            let tag = reference.strip_prefix("refs/tags/")?;
            let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
            version.pre.is_empty().then(|| (version, tag.to_owned()))
        })
        .max_by(|a, b| a.0.cmp_precedence(&b.0).then_with(|| a.1.cmp(&b.1)))
        .context("No stable version tags found in the official repository")?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    if !latest.cmp_precedence(&current).is_gt() {
        let cyan = AnsiColor::Cyan.on_default();
        anstream::println!(
            "{cyan}Already up to date: fmt-sync {current} (latest stable: {latest}){cyan:#}"
        );
        return Ok(());
    }
    let cargo = tools::from_path("cargo").context(
        "Install Rust and Cargo and add them to PATH: https://rust-lang.org/tools/install/",
    )?;
    let install_root = installation_root()?;
    if install_root.is_none() {
        println!(
            "This executable is not registered with Cargo. The new version will be installed to Cargo's configured destination; this copy will remain unchanged."
        );
    }
    println!("Updating fmt-sync {current} -> {latest} with Cargo (this may take a few minutes)...");
    let mut command = Command::new(&cargo.program);
    command
        .args([
            "install", "--git", REPOSITORY, "--tag", &tag, "--locked", "--force", "fmt-sync",
        ])
        .current_dir(work.path());
    if let Some(root) = &install_root {
        command.arg("--root").arg(root);
    }
    #[cfg(windows)]
    let mut backup = install_root
        .as_ref()
        .map(|_| RunningBackup::prepare())
        .transpose()?;
    let installed = command
        .status()
        .context("Failed to start Cargo")
        .and_then(|status| {
            ensure!(status.success(), "Cargo installation failed ({status})");
            Ok(())
        });
    if let Err(error) = installed {
        #[cfg(windows)]
        if let Some(backup) = &mut backup {
            backup.restore().with_context(|| {
                format!(
                    "{error:#}; failed to restore the original executable; backup: {}",
                    backup.saved.display()
                )
            })?;
        }
        return Err(error);
    }
    #[cfg(windows)]
    if let Some(backup) = &mut backup {
        backup.committed = true;
    }
    let green = AnsiColor::Green.on_default();
    anstream::println!("{green}Update successful: fmt-sync {current} -> {latest}{green:#}");
    Ok(())
}

// A bin directory alone is insufficient: only redirect Cargo to a root whose
// own installation metadata registers this exact executable for fmt-sync.
fn installation_root() -> Result<Option<PathBuf>> {
    let executable = std::env::current_exe().context("Failed to locate the current executable")?;
    let filename = format!("fmt-sync{}", std::env::consts::EXE_SUFFIX);
    if executable
        .file_name()
        .is_none_or(|name| name != filename.as_str())
    {
        return Ok(None);
    }
    let Some(bin) = executable
        .parent()
        .filter(|dir| dir.file_name().is_some_and(|name| name == "bin"))
    else {
        return Ok(None);
    };
    let Some(root) = bin.parent() else {
        return Ok(None);
    };
    let metadata = match fs::read_to_string(root.join(".crates.toml")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("Failed to read Cargo installation metadata"),
    };
    let metadata: toml::Value =
        toml::from_str(&metadata).context("Invalid Cargo installation metadata")?;
    let registered = metadata
        .get("v1")
        .and_then(toml::Value::as_table)
        .is_some_and(|packages| {
            packages.iter().any(|(package, bins)| {
                package.split_whitespace().next() == Some("fmt-sync")
                    && bins
                        .as_array()
                        .is_some_and(|bins| bins.iter().any(|bin| bin.as_str() == Some(&filename)))
            })
        });
    Ok(registered.then(|| root.to_path_buf()))
}

// Windows permits renaming the running image, but not overwriting it. Leave a
// replacement copy for Cargo and keep the image alive in its own backup directory.
#[cfg(windows)]
struct RunningBackup {
    original: PathBuf,
    saved: PathBuf,
    directory: PathBuf,
    committed: bool,
}

#[cfg(windows)]
impl RunningBackup {
    fn prepare() -> Result<Self> {
        let original = std::env::current_exe()?;
        let directory = tempfile::Builder::new()
            .prefix(".fmt-sync-update-")
            .tempdir_in(
                original
                    .parent()
                    .context("Executable has no parent directory")?,
            )
            .context("Failed to create an executable backup directory")?;
        let saved = directory.path().join("fmt-sync.exe");
        fs::rename(&original, &saved).context("Failed to move the running executable aside")?;
        let backup = Self {
            original,
            saved,
            directory: directory.keep(),
            committed: false,
        };
        // Keep the installed path runnable if compilation is interrupted.
        // Drop restores the original if copying or process startup fails.
        fs::copy(&backup.saved, &backup.original)
            .context("Failed to create Cargo's replacement copy")?;
        Ok(backup)
    }

    fn restore(&mut self) -> Result<()> {
        match fs::remove_file(&self.original) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        fs::rename(&self.saved, &self.original)?;
        self.committed = true;
        let _ = fs::remove_dir(&self.directory);
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for RunningBackup {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.restore();
        }
    }
}

pub fn cleanup() {
    #[cfg(windows)]
    {
        let Ok(Some(root)) = installation_root() else {
            return;
        };
        let Ok(entries) = fs::read_dir(root.join("bin")) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(suffix) = name.strip_prefix(".fmt-sync-update-") else {
                continue;
            };
            if suffix.len() != 6 || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                continue;
            }
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            // No recursive removal: only our known backup executable and its
            // empty directory. A currently running backup remains locked.
            if fs::remove_file(entry.path().join("fmt-sync.exe")).is_ok() {
                let _ = fs::remove_dir(entry.path());
            }
        }
    }
}
