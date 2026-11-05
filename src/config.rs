use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::language::Language;
use crate::paths::CONFIG_FILE;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;

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
                Language::all().iter().any(|language| language
                    .adapter()
                    .tools()
                    .iter()
                    .any(|spec| spec.version.is_some_and(|version| version.key == tool))),
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
        crate::paths::normalize_ignores(root, &self.ignore)
    }
}
