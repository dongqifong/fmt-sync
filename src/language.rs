use crate::adapters::{self, LanguageAdapter};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[value(alias = "py")]
    #[serde(alias = "py")]
    Python,
    Go,
    #[value(alias = "rs")]
    #[serde(alias = "rs")]
    Rust,
    #[value(alias = "ts")]
    #[serde(alias = "ts")]
    Typescript,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Typescript => "typescript",
        })
    }
}

impl Language {
    pub fn all() -> &'static [Self] {
        Self::value_variants()
    }

    pub fn adapter(self) -> &'static dyn LanguageAdapter {
        match self {
            Self::Python => &adapters::python::Python,
            Self::Go => &adapters::go::Go,
            Self::Rust => &adapters::rust::Rust,
            Self::Typescript => &adapters::typescript::Typescript,
        }
    }
}
