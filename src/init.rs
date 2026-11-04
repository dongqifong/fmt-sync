use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::{Context, Result, ensure};
use toml_edit::{Array, DocumentMut, value};

use crate::{
    config::{CONFIG_FILE, Config, Language},
    native,
};

pub fn initialize(
    root: &Path,
    languages: &[Language],
    preset: Option<&Path>,
) -> Result<Vec<String>> {
    let preset = preset.map(|p| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        }
    });
    if let Some(path) = &preset {
        ensure!(
            path.is_dir(),
            "Preset directory does not exist: {}",
            path.display()
        );
    }
    let path = root.join(CONFIG_FILE);
    let existing = if path.try_exists()? {
        Some(Config::load(root)?)
    } else {
        None
    };
    ensure!(
        !path.is_symlink(),
        "Symlink configuration files are not supported: {}",
        path.display()
    );
    let mut selected = Vec::new();
    for lang in languages {
        if !selected.contains(lang) {
            selected.push(*lang);
        }
    }
    let mut messages = Vec::new();
    let mut writes = Vec::new();
    // Preflight every selected language before creating any files.
    for lang in &selected {
        if let Some(config) = native::discover(root, *lang)? {
            messages.push(format!("{lang}: keeping {}", config.path.display()));
            continue;
        }
        if *lang == Language::Typescript {
            native::check_typescript_conflicts(root)?;
        }
        let template = preset
            .as_ref()
            .map(|p| native::preset_config(p, *lang))
            .transpose()?
            .flatten();
        if let Some(template) = template {
            let name = template
                .path
                .file_name()
                .context("Preset file has no filename")?;
            writes.push((root.join(name), template.content));
            messages.push(format!("{lang}: using preset {}", template.path.display()));
        } else if let Some((name, content)) = native::defaults(*lang) {
            writes.push((root.join(name), content.to_string()));
            messages.push(format!("{lang}: using built-in defaults from {name}"));
        } else {
            messages.push(format!("{lang}: using standard toolchain rules"));
        }
    }
    let old_text = if existing.is_some() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let mut doc = old_text.parse::<DocumentMut>()?;
    if existing.is_none() {
        doc["schema-version"] = value(1);
        doc["languages"] = value(Array::new());
        doc["ignore"] = value(Array::new());
    }
    let present = existing
        .as_ref()
        .map(|c| c.languages.clone())
        .unwrap_or_default();
    let array = doc["languages"]
        .as_array_mut()
        .context("languages must be an array")?;
    let mut changed = existing.is_none();
    for lang in selected {
        if !present.contains(&lang) {
            array.push(lang.to_string());
            changed = true;
        }
    }
    let mut created = Vec::new();
    for (target, content) in writes {
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            created.push(target.display().to_string());
            file.write_all(content.as_bytes())?;
            Ok(())
        })();
        result.with_context(|| {
            format!(
                "Failed to write {}; files already created: {}. Remaining items are incomplete",
                target.display(),
                created.join(", ")
            )
        })?;
    }
    if changed {
        if existing.is_none() {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)?;
            file.write_all(doc.to_string().as_bytes())?;
        } else {
            fs::write(&path, doc.to_string()).with_context(|| {
                format!(
                    "Failed to update {}; files already created: {}",
                    path.display(),
                    created.join(", ")
                )
            })?;
        }
    }
    messages.push(format!("Initialized {}", path.display()));
    Ok(messages)
}
