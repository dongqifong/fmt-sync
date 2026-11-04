use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use proc_macro2::{TokenStream, TokenTree};
use serde::Deserialize;
use syn::ext::IdentExt;
use syn::{Item, Meta};

use crate::{
    scope::Scope,
    tools::{Tool, strings},
};

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}
#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<Target>,
}
#[derive(Deserialize)]
struct Target {
    src_path: PathBuf,
}

pub fn prepare(root: &Path, scope: &Scope, cargo: &Tool) -> Result<String> {
    let root = root.canonicalize()?;
    let manifest = root.join("Cargo.toml");
    ensure!(
        manifest.is_file(),
        "The current project has no Cargo.toml; create a Rust package first"
    );
    let value: toml::Value = toml::from_str(&fs::read_to_string(&manifest)?)?;
    ensure!(
        value.get("package").is_some(),
        "The current directory is a workspace; run this command in each Rust package directory"
    );
    let output = cargo.output(
        &root,
        &strings(&["metadata", "--no-deps", "--format-version", "1"]),
    )?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let meta: Metadata =
        serde_json::from_slice(&output.stdout).context("cargo metadata returned invalid JSON")?;
    let package = meta
        .packages
        .into_iter()
        .find(|p| p.manifest_path.canonicalize().is_ok_and(|p| p == manifest))
        .context("The current package was not found in metadata")?;
    let mut visited = HashSet::new();
    for target in &package.targets {
        // Lexical parent components are rejected before canonicalization for a clear scope error.
        ensure!(
            !target
                .src_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
            "Cargo target is outside the current project scope: {}",
            target.src_path.display()
        );
        validate_file(&root, &target.src_path, scope)?;
        visit_file(
            &root,
            &target.src_path,
            target
                .src_path
                .parent()
                .context("Target has no parent directory")?,
            scope,
            &mut visited,
        )?;
    }
    Ok(package.name)
}

fn validate_file(root: &Path, path: &Path, scope: &Scope) -> Result<()> {
    let path = path
        .canonicalize()
        .with_context(|| format!("Failed to verify module scope: {}", path.display()))?;
    let relative = path
        .strip_prefix(root)
        .context("Rust module is outside the current project scope")?;
    ensure!(
        scope.files.iter().any(|p| p == relative),
        "Rust module is outside the managed scope: {}; ignore only supports independent subprojects",
        relative.display()
    );
    ensure!(
        !scope
            .excluded_projects
            .iter()
            .any(|p| relative.starts_with(p)),
        "Rust module belongs to another subproject scope: {}",
        relative.display()
    );
    Ok(())
}

fn visit_file(
    root: &Path,
    path: &Path,
    directory: &Path,
    scope: &Scope,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    validate_file(root, path, scope)?;
    let key = path.canonicalize()?;
    if !visited.insert(key) {
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    let file = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse Rust module: {}", path.display()))?;
    visit_items(root, &file.items, directory, scope, visited)
}

fn visit_items(
    root: &Path,
    items: &[Item],
    directory: &Path,
    scope: &Scope,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    for item in items {
        if let Item::Macro(item) = item {
            ensure!(
                !has_module(item.mac.tokens.clone()),
                "This version cannot verify the formatting scope of modules declared in macros; use a standard module directory layout"
            );
        }
        let Item::Mod(module) = item else {
            continue;
        };
        for attr in &module.attrs {
            // #[path] and cfg_attr(path=...) require a separate scope proof. Fail closed for v1.
            let explicit_path = attr.path().is_ident("path")
                || (attr.path().is_ident("cfg_attr")
                    && matches!(&attr.meta, Meta::List(list) if list.tokens.to_string().contains("path")));
            ensure!(
                !explicit_path,
                "This version cannot verify the formatting scope of #[path] modules; use a standard module directory layout"
            );
        }
        let name = module.ident.unraw().to_string();
        let module_dir = directory.join(&name);
        if let Some((_, content)) = &module.content {
            visit_items(root, content, &module_dir, scope, visited)?;
        } else {
            for candidate in [
                directory.join(format!("{name}.rs")),
                module_dir.join("mod.rs"),
            ] {
                if candidate.exists() {
                    visit_file(root, &candidate, &module_dir, scope, visited)?;
                }
            }
        }
    }
    Ok(())
}

fn has_module(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident == "mod",
        TokenTree::Group(group) => has_module(group.stream()),
        _ => false,
    })
}
