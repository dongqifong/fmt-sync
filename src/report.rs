use crate::config::Language;
use clap::builder::styling::AnsiColor;

#[derive(Clone, Copy, PartialEq, Eq)]
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
    fn label(self) -> &'static str {
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
    pub stage: &'static str,
    pub status: Status,
    pub tool: String,
    pub detail: String,
}

impl StageResult {
    pub fn error(language: Language, stage: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            language,
            stage,
            status: Status::Error,
            tool: String::new(),
            detail: error.to_string(),
        }
    }
    pub fn skipped(language: Language, stage: &'static str) -> Self {
        Self {
            language,
            stage,
            status: Status::Skipped,
            tool: String::new(),
            detail: "No source files in scope".into(),
        }
    }
}

pub fn print(results: &[StageResult]) -> i32 {
    let mut code = 0;
    for result in results {
        println!(
            "{} {} [{}] {}",
            result.language,
            result.stage,
            result.status.label(),
            result.tool
        );
        if !result.detail.trim().is_empty() {
            println!("{}", result.detail.trim_end());
        }
        code = code.max(result.status.code());
    }
    if code == 0 {
        let green = AnsiColor::Green.on_default();
        anstream::println!("fmt-sync: {green}passed (exit 0){green:#}");
    } else {
        let yellow = AnsiColor::Yellow.on_default();
        anstream::println!("fmt-sync: {yellow}see results above (exit {code}){yellow:#}");
    }
    code
}
