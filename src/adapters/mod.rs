pub(crate) mod go;
pub(crate) mod python;
pub(crate) mod rust;
pub(crate) mod typescript;

use crate::{
    execution::{RunMode, StageResult},
    native::NativeConfigSpec,
    scope::Scope,
    tools::{Tool, ToolSpec},
};
use anyhow::Result;
use std::{collections::BTreeMap, path::Path};

/// A language owns its native settings, tool requirements and execution stages.
/// Shared infrastructure must not dispatch on individual languages.
pub trait LanguageAdapter: Sync {
    fn native_config(&self) -> Option<NativeConfigSpec> {
        None
    }
    fn tools(&self) -> &'static [ToolSpec];
    fn check_init_conflicts(&self, _root: &Path) -> Result<()> {
        Ok(())
    }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode) -> Result<Vec<StageResult>>;
}

pub struct RunContext<'a> {
    pub scope: &'a Scope,
    pub versions: &'a BTreeMap<String, String>,
}

impl RunContext<'_> {
    pub fn root(&self) -> &Path {
        self.scope.root()
    }
    pub fn tool(&self, spec: &ToolSpec) -> Result<Tool> {
        self.versioned(spec.resolve(self.root())?, spec)
    }
    pub fn versioned(&self, tool: Tool, spec: &ToolSpec) -> Result<Tool> {
        let required = spec
            .version
            .and_then(|version| self.versions.get(version.key))
            .map(String::as_str);
        tool.versioned(self.root(), spec, required)
    }
}
