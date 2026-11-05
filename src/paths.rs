use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE: &str = "fmt-sync.toml";

pub fn normalize_ignores(root: &Path, ignore: &[String]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for raw in ignore {
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
