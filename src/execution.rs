use std::fmt;

use crate::language::Language;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Format,
    Check,
}

impl RunMode {
    pub fn is_check(self) -> bool {
        self == Self::Check
    }

    pub fn primary_stage(self) -> StageKind {
        match self {
            Self::Format => StageKind::Format,
            Self::Check => StageKind::Check,
        }
    }
}

#[derive(Clone, Copy)]
pub enum StageKind {
    Scope,
    Setup,
    Imports,
    Format,
    Lint,
    Check,
}

impl fmt::Display for StageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scope => "scope",
            Self::Setup => "setup",
            Self::Imports => "imports",
            Self::Format => "format",
            Self::Lint => "lint",
            Self::Check => "check",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Passed,
    Failed,
    Error,
    Skipped,
}

impl Status {
    pub fn code(self) -> i32 {
        match self {
            Self::Passed | Self::Skipped => 0,
            Self::Failed => 1,
            Self::Error => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

pub struct StageResult {
    pub language: Language,
    pub stage: StageKind,
    pub status: Status,
    pub tool: String,
    pub detail: String,
}

impl StageResult {
    pub fn error(language: Language, stage: StageKind, error: impl fmt::Display) -> Self {
        Self {
            language,
            stage,
            status: Status::Error,
            tool: String::new(),
            detail: error.to_string(),
        }
    }

    pub fn skipped(language: Language, stage: StageKind) -> Self {
        Self {
            language,
            stage,
            status: Status::Skipped,
            tool: String::new(),
            detail: "No source files in scope".into(),
        }
    }
}
