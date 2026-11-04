use crate::config::{CONFIG_FILE, Config, Language, is_link_like};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct Scope {
    pub files: Vec<PathBuf>,
    pub go_files: Vec<PathBuf>,
    pub go_packages: Vec<PathBuf>,
    pub excluded_projects: Vec<PathBuf>,
}

impl Scope {
    pub fn discover(root: &Path, config: &Config) -> Result<Self> {
        let root = root.canonicalize()?;
        let ignored = config.normalized_ignores(&root)?;
        let (files, excluded_projects) = collect(&root, &ignored, false)?;
        let go_files = if config.languages.contains(&Language::Go) {
            collect(&root, &ignored, true)?
                .0
                .into_iter()
                .filter(|p| p.extension().is_some_and(|ext| ext == "go"))
                .collect()
        } else {
            Vec::new()
        };
        let mut go_packages = std::collections::BTreeSet::new();
        for file in &files {
            if file.extension().is_some_and(|ext| ext == "go") {
                let parent = file.parent().unwrap_or(Path::new(""));
                let native_excluded = parent.components().any(|c| {
                    let text = c.as_os_str().to_string_lossy();
                    text.starts_with('.') || text.starts_with('_') || text == "testdata"
                });
                let filename = file.file_name().unwrap().to_string_lossy();
                if !native_excluded && !filename.starts_with(['.', '_']) {
                    go_packages.insert(if parent.as_os_str().is_empty() {
                        PathBuf::from(".")
                    } else {
                        parent.to_path_buf()
                    });
                }
            }
        }
        Ok(Self {
            files,
            go_files,
            go_packages: go_packages.into_iter().collect(),
            excluded_projects,
        })
    }
}

fn collect(
    root: &Path,
    ignored: &[PathBuf],
    respect_gitignore: bool,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
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
        let scope = Scope::discover(dir.path(), &Config::load(dir.path()).unwrap()).unwrap();
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
        let scope = Scope::discover(dir.path(), &Config::load(dir.path()).unwrap()).unwrap();
        assert!(scope.go_files.contains(&PathBuf::from("main.go")));
        assert!(!scope.go_files.contains(&PathBuf::from("generated/tool.go")));
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
        let scope = Scope::discover(dir.path(), &Config::load(dir.path()).unwrap()).unwrap();
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
        let scope = Scope::discover(dir.path(), &Config::load(dir.path()).unwrap()).unwrap();
        assert!(!scope.files.contains(&PathBuf::from("linked/secret.py")));
    }
}
