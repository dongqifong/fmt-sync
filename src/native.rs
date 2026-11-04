use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use jsonc_parser::ParseOptions;
use serde_json::Value;

use crate::config::Language;

pub struct NativeConfig {
    pub path: PathBuf,
    pub content: String,
}

pub fn read_json(path: &Path) -> Result<Value> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    parse_json(&content, path)
}

fn parse_json(content: &str, path: &Path) -> Result<Value> {
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

pub fn discover(root: &Path, lang: Language) -> Result<Option<NativeConfig>> {
    if lang == Language::Go {
        return Ok(None);
    }
    let names: &[&str] = match lang {
        Language::Python => &[".ruff.toml", "ruff.toml", "pyproject.toml"],
        Language::Rust => &["rustfmt.toml", ".rustfmt.toml"],
        Language::Typescript => &["biome.json", "biome.jsonc"],
        Language::Go => unreachable!(),
    };
    let mut chosen = None;
    for name in names {
        let path = root.join(name);
        if !path.try_exists()? {
            continue;
        }
        ensure!(
            path.is_file() && !path.is_symlink(),
            "Native configuration must be a regular file: {}",
            path.display()
        );
        let content = fs::read_to_string(&path)?;
        let relevant = validate(&path, &content, lang, false)?;
        if relevant && chosen.is_none() {
            chosen = Some(NativeConfig { path, content });
        }
    }
    Ok(chosen)
}

fn validate(path: &Path, content: &str, lang: Language, preset: bool) -> Result<bool> {
    if lang == Language::Typescript {
        let parsed = parse_json(content, path)?;
        ensure!(
            parsed.is_object(),
            "Biome configuration must be a JSON object: {}",
            path.display()
        );
        if preset {
            ensure!(
                parsed.get("extends").is_none(),
                "Presets do not support extends: {}",
                path.display()
            );
        }
        return Ok(true);
    }
    let parsed: toml::Value =
        toml::from_str(content).with_context(|| format!("Failed to parse {}", path.display()))?;
    if path.file_name().is_some_and(|x| x == "pyproject.toml") {
        let section = parsed.get("tool").and_then(|x| x.get("ruff"));
        if let Some(section) = section {
            ensure!(section.is_table(), "tool.ruff must be a table");
        }
        return Ok(section.is_some());
    }
    if preset {
        ensure!(
            parsed.get("extend").is_none() && parsed.get("extends").is_none(),
            "Presets do not support extend/extends: {}",
            path.display()
        );
    }
    Ok(true)
}

pub fn preset_config(root: &Path, lang: Language) -> Result<Option<NativeConfig>> {
    let names: &[&str] = match lang {
        Language::Python => &["ruff.toml", ".ruff.toml"],
        Language::Rust => &["rustfmt.toml", ".rustfmt.toml"],
        Language::Typescript => &["biome.json", "biome.jsonc"],
        Language::Go => &[],
    };
    let found: Vec<_> = names
        .iter()
        .map(|n| root.join(n))
        .filter(|p| p.exists())
        .collect();
    ensure!(
        found.len() <= 1,
        "The preset contains multiple configuration files for {lang}"
    );
    match found.into_iter().next() {
        Some(path) => {
            ensure!(
                path.is_file() && !path.is_symlink(),
                "Preset configuration must be a regular file: {}",
                path.display()
            );
            let content = fs::read_to_string(&path)?;
            validate(&path, &content, lang, true)?;
            Ok(Some(NativeConfig { path, content }))
        }
        None => Ok(None),
    }
}

pub fn defaults(lang: Language) -> Option<(&'static str, &'static str)> {
    match lang {
        Language::Python => Some((
            "ruff.toml",
            "line-length = 88\n\n[lint]\nselect = [\"E4\", \"E7\", \"E9\", \"F\", \"I\"]\n",
        )),
        Language::Rust => Some((
            "rustfmt.toml",
            "# Use rustfmt stable defaults; edit this file to customize.\n",
        )),
        Language::Typescript => Some((
            "biome.json",
            "{\n  \"formatter\": { \"enabled\": true },\n  \"linter\": { \"enabled\": true }\n}\n",
        )),
        Language::Go => None,
    }
}

pub fn check_typescript_conflicts(root: &Path) -> Result<()> {
    let names = [
        ".eslintrc",
        ".eslintrc.json",
        ".eslintrc.js",
        ".eslintrc.cjs",
        ".eslintrc.yaml",
        ".eslintrc.yml",
        "eslint.config.js",
        "eslint.config.mjs",
        "eslint.config.cjs",
        "eslint.config.ts",
        "eslint.config.mts",
        "eslint.config.cts",
        ".prettierrc",
        ".prettierrc.json",
        ".prettierrc.json5",
        ".prettierrc.yaml",
        ".prettierrc.yml",
        ".prettierrc.js",
        ".prettierrc.cjs",
        ".prettierrc.mjs",
        ".prettierrc.toml",
        "prettier.config.js",
        "prettier.config.cjs",
        "prettier.config.mjs",
        "prettier.config.ts",
    ];
    let manifest = root.join("package.json");
    let package_conflict = if manifest.is_file() {
        let value = read_json(&manifest)?;
        value.get("eslintConfig").is_some() || value.get("prettier").is_some()
    } else {
        false
    };
    if package_conflict || names.iter().any(|name| root.join(name).exists()) {
        bail!(
            "Existing ESLint/Prettier configuration found; resolve overlapping tools before initializing Biome. Automatic migration is not supported"
        );
    }
    Ok(())
}
