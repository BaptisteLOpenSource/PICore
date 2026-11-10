use std::io;

use thiserror::Error as ThisError;

use crate::process::ProcError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error(transparent)]
    Proc(#[from] ProcError),
    #[error(transparent)]
    Store(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("gh: {0}")]
    Gh(String),
    #[error("config: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("ambiguous: {0:?}")]
    Ambiguous(Vec<String>),
}

impl Error {
    /// Stable machine code, shared by CLI `--json` and MCP errors.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Proc(_) => "git",
            Error::Gh(_) => "gh",
            Error::Config(_) => "config",
            Error::NotFound(_) => "not_found",
            Error::Ambiguous(_) => "ambiguous",
            Error::Store(_) | Error::Io(_) => "io",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
