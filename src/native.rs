use anyhow::{Context, Result, ensure};
use jsonc_parser::ParseOptions;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Project,
    Preset,
}

pub struct NativeConfigSpec {
    pub project_files: &'static [&'static str],
    pub preset_files: &'static [&'static str],
    pub default: Option<(&'static str, &'static str)>,
    pub validate: fn(&Path, &str, ConfigSource) -> Result<bool>,
}

pub struct NativeConfig {
    pub path: PathBuf,
    pub content: String,
}

pub fn read_json(path: &Path) -> Result<Value> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    parse_json(&content, path)
}

pub fn parse_json(content: &str, path: &Path) -> Result<Value> {
    let comments = path.extension().is_some_and(|x| x == "jsonc");
    let options = ParseOptions {
        allow_comments: comments,
        allow_trailing_commas: comments,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    jsonc_parser::parse_to_serde_value(content.trim_start_matches('\u{feff}'), &options)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

pub fn discover(root: &Path, spec: Option<&NativeConfigSpec>) -> Result<Option<NativeConfig>> {
    let Some(spec) = spec else {
        return Ok(None);
    };
    let mut chosen = None;
    for name in spec.project_files {
        let path = root.join(name);
        if !path.try_exists()? {
            continue;
        }
        let config = read_config(path, spec, ConfigSource::Project)?;
        if chosen.is_none() {
            chosen = config;
        }
    }
    Ok(chosen)
}

pub fn preset_config(
    root: &Path,
    language: impl std::fmt::Display,
    spec: Option<&NativeConfigSpec>,
) -> Result<Option<NativeConfig>> {
    let Some(spec) = spec else {
        return Ok(None);
    };
    let found: Vec<_> = spec
        .preset_files
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect();
    ensure!(
        found.len() <= 1,
        "The preset contains multiple configuration files for {language}"
    );
    found
        .into_iter()
        .next()
        .map(|path| read_config(path, spec, ConfigSource::Preset))
        .transpose()
        .map(Option::flatten)
}

fn read_config(
    path: PathBuf,
    spec: &NativeConfigSpec,
    source: ConfigSource,
) -> Result<Option<NativeConfig>> {
    ensure!(
        path.is_file() && !path.is_symlink(),
        "Native configuration must be a regular file: {}",
        path.display()
    );
    let content = fs::read_to_string(&path)?;
    Ok((spec.validate)(&path, &content, source)?.then_some(NativeConfig { path, content }))
}
