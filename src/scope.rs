use crate::paths::{CONFIG_FILE, is_link_like};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy)]
pub enum ScanPolicy {
    AllFiles,
    RespectGitignore,
}

pub struct Scope {
    root: PathBuf,
    ignored: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub excluded_projects: Vec<PathBuf>,
}

impl Scope {
    pub fn discover(root: &Path, ignored: &[PathBuf]) -> Result<Self> {
        let root = root.canonicalize()?;
        let (files, excluded_projects) = collect(&root, ignored, ScanPolicy::AllFiles)?;
        Ok(Self {
            root,
            ignored: ignored.to_vec(),
            files,
            excluded_projects,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn scan(&self, policy: ScanPolicy) -> Result<Vec<PathBuf>> {
        collect(&self.root, &self.ignored, policy).map(|(files, _)| files)
    }

    pub fn files_with_extensions(&self, extensions: &[&str]) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|path| {
                path.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|extension| extensions.contains(&extension))
            })
            .cloned()
            .collect()
    }
}

fn collect(
    root: &Path,
    ignored: &[PathBuf],
    policy: ScanPolicy,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let respect_gitignore = matches!(policy, ScanPolicy::RespectGitignore);
    let ignored = ignored.to_vec();
    let excluded = Arc::new(Mutex::new(ignored.clone()));
    let collected = Arc::clone(&excluded);
    let base = root.to_path_buf();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(respect_gitignore)
        .git_ignore(respect_gitignore)
        .parents(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            let Some(kind) = entry.file_type() else {
                return false;
            };
            if kind.is_symlink() || is_link_like(entry.path()) {
                return false;
            }
            let Ok(relative) = entry.path().strip_prefix(&base) else {
                return false;
            };
            if ignored.iter().any(|p| relative.starts_with(p)) {
                return false;
            }
            if kind.is_dir() {
                let defaults = [
                    ".git",
                    "vendor",
                    "node_modules",
                    ".venv",
                    "venv",
                    "target",
                    "dist",
                    "build",
                    ".ruff_cache",
                    ".mypy_cache",
                    "__pycache__",
                ];
                let default_exclude = defaults.iter().any(|name| entry.file_name() == *name);
                if default_exclude || entry.path().join(CONFIG_FILE).exists() {
                    collected
                        .lock()
                        .expect("scope mutex")
                        .push(relative.to_path_buf());
                    return false;
                }
            }
            true
        });
    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.context("Failed to scan the project directory")?;
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            files.push(entry.path().strip_prefix(root)?.to_path_buf());
        }
    }
    files.sort();
    let mut excluded_projects = excluded.lock().expect("scope mutex").clone();
    excluded_projects.sort();
    excluded_projects.dedup();
    Ok((files, excluded_projects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, path: &str, content: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn scope_skips_ignored_and_independently_managed_projects() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "fmt-sync.toml",
            "schema-version = 1\nlanguages = ['python','typescript']\nignore = ['legacy/', 'future/']\n",
        );
        for file in [
            "src/main.py",
            "src/helper.ts",
            "legacy/old.py",
            "legacy-backup/keep.py",
            "independent/main.py",
            "node_modules/vendor.ts",
            ".venv/lib/tool.py",
        ] {
            write(dir.path(), file, "");
        }
        write(
            dir.path(),
            "independent/fmt-sync.toml",
            "invalid but irrelevant",
        );
        let scope = Scope::discover(
            dir.path(),
            &Config::load(dir.path())
                .unwrap()
                .normalized_ignores(dir.path())
                .unwrap(),
        )
        .unwrap();
        assert!(scope.files.contains(&PathBuf::from("src/main.py")));
        assert!(scope.files.contains(&PathBuf::from("src/helper.ts")));
        assert!(
            scope
                .files
                .contains(&PathBuf::from("legacy-backup/keep.py"))
        );
        for file in [
            "legacy/old.py",
            "independent/main.py",
            "node_modules/vendor.ts",
            ".venv/lib/tool.py",
        ] {
            assert!(!scope.files.contains(&PathBuf::from(file)), "leaked {file}");
        }
        assert!(
            scope
                .excluded_projects
                .contains(&PathBuf::from("independent"))
        );
    }

    #[test]
    fn gitignore_is_respected_without_a_git_repository() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "fmt-sync.toml",
            "schema-version = 1\nlanguages = ['go']\n",
        );
        write(dir.path(), ".gitignore", "generated/\n");
        write(dir.path(), "generated/tool.go", "package generated");
        write(dir.path(), "main.go", "package main");
        let scope = Scope::discover(
            dir.path(),
            &Config::load(dir.path())
                .unwrap()
                .normalized_ignores(dir.path())
                .unwrap(),
        )
        .unwrap();
        let files = scope.scan(ScanPolicy::RespectGitignore).unwrap();
        assert!(files.contains(&PathBuf::from("main.go")));
        assert!(!files.contains(&PathBuf::from("generated/tool.go")));
    }

    #[test]
    fn scope_handles_space_and_unicode_paths() {
        let dir = tempfile::Builder::new()
            .prefix("fmt sync caf\u{e9} ")
            .tempdir()
            .unwrap();
        write(
            dir.path(),
            "fmt-sync.toml",
            "schema-version = 1\nlanguages = ['python']\nignore = ['legacy caf\u{e9}/']\n",
        );
        write(dir.path(), "source caf\u{e9}/hello.py", "");
        write(dir.path(), "legacy caf\u{e9}/hello.py", "");
        let scope = Scope::discover(
            dir.path(),
            &Config::load(dir.path())
                .unwrap()
                .normalized_ignores(dir.path())
                .unwrap(),
        )
        .unwrap();
        assert!(
            scope
                .files
                .contains(&PathBuf::from("source caf\u{e9}/hello.py"))
        );
        assert!(
            !scope
                .files
                .contains(&PathBuf::from("legacy caf\u{e9}/hello.py"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_directories_are_not_traversed() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write(
            dir.path(),
            "fmt-sync.toml",
            "schema-version = 1\nlanguages = ['python']\n",
        );
        write(outside.path(), "secret.py", "");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked")).unwrap();
        let scope = Scope::discover(
            dir.path(),
            &Config::load(dir.path())
                .unwrap()
                .normalized_ignores(dir.path())
                .unwrap(),
        )
        .unwrap();
        assert!(!scope.files.contains(&PathBuf::from("linked/secret.py")));
    }
}
