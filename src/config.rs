use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = "fmt-sync.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[value(alias = "py")]
    #[serde(alias = "py")]
    Python,
    Go,
    #[value(alias = "rs")]
    #[serde(alias = "rs")]
    Rust,
    #[value(alias = "ts")]
    #[serde(alias = "ts")]
    Typescript,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Typescript => "typescript",
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub schema_version: u32,
    pub languages: Vec<Language>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub tool_versions: BTreeMap<String, String>,
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILE);
        ensure!(
            path.is_file(),
            "The current directory is not initialized; run fmt-sync init --lang <language> first"
        );
        ensure!(
            !path.is_symlink(),
            "Symlink configuration files are not supported: {}",
            path.display()
        );
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: Self = toml::from_str(&text).context("Failed to parse fmt-sync.toml")?;
        ensure!(
            config.schema_version == 1,
            "Unsupported schema-version = {}",
            config.schema_version
        );
        ensure!(!config.languages.is_empty(), "languages must not be empty");
        for (index, lang) in config.languages.iter().enumerate() {
            ensure!(
                !config.languages[..index].contains(lang),
                "Duplicate language: {lang}"
            );
        }
        for (tool, version) in &config.tool_versions {
            ensure!(
                ["ruff", "go", "cargo", "rustfmt", "clippy", "biome"].contains(&tool.as_str()),
                "Unsupported tool version key: {tool}"
            );
            ensure!(
                !version.trim().is_empty(),
                "The {tool} version must not be empty"
            );
        }
        config.normalized_ignores(root)?;
        Ok(config)
    }

    pub fn normalized_ignores(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for raw in &self.ignore {
            let text = raw.replace('\\', "/");
            ensure!(
                !text.starts_with('/') && !text.contains([':', '*', '?', '[', ']', '!', '\0']),
                "ignore entries must be relative subproject directories, not absolute paths or globs: {raw}"
            );
            let mut components = Vec::new();
            for part in text.split('/') {
                match part {
                    "" | "." => {}
                    ".." => {
                        ensure!(
                            components.pop().is_some(),
                            "ignore must stay within the project: {raw}"
                        );
                    }
                    _ => components.push(part),
                }
            }
            ensure!(
                !components.is_empty(),
                "ignore must not refer to the project root: {raw}"
            );
            let mut path: PathBuf = components.iter().collect();
            let full = root.join(&path);
            if full.try_exists()? {
                ensure!(
                    full.is_dir(),
                    "ignore only accepts subproject directories, not files: {raw}"
                );
                // Correct existing Windows casing only for real directories; never turn an
                // ignored alias into an exclusion of its target (or of the entire root).
                let mut ancestor = root.to_path_buf();
                let linked = path.components().any(|part| {
                    ancestor.push(part);
                    is_link_like(&ancestor)
                });
                if !linked {
                    let canonical_root = root.canonicalize()?;
                    let canonical = full.canonicalize()?;
                    path = canonical
                        .strip_prefix(&canonical_root)
                        .with_context(|| format!("ignore must stay within the project: {raw}"))?
                        .to_path_buf();
                    ensure!(
                        !path.as_os_str().is_empty(),
                        "ignore must not refer to the project root: {raw}"
                    );
                }
            }
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        Ok(paths)
    }
}

pub fn is_link_like(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
    }
    #[cfg(not(windows))]
    {
        false
    }
}
