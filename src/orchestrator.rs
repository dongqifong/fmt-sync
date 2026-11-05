use crate::{
    adapters::RunContext,
    config::Config,
    execution::{RunMode, StageKind, StageResult},
    scope::Scope,
};
use std::path::Path;

/// Discover one project boundary, then execute languages in configuration order.
pub fn execute(root: &Path, config: &Config, mode: RunMode) -> Vec<StageResult> {
    let scope = match config
        .normalized_ignores(root)
        .and_then(|ignored| Scope::discover(root, &ignored))
    {
        Ok(scope) => scope,
        Err(error) => {
            return vec![StageResult::error(
                config.languages[0],
                StageKind::Scope,
                format!("{error:#}"),
            )];
        }
    };
    let context = RunContext {
        scope: &scope,
        versions: &config.tool_versions,
    };
    let mut results = Vec::new();
    for language in &config.languages {
        match language.adapter().execute(&context, mode) {
            Ok(stages) => results.extend(stages),
            Err(error) => results.push(StageResult::error(
                *language,
                StageKind::Setup,
                format!("{error:#}"),
            )),
        }
    }
    results
}
