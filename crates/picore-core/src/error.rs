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
    #[error("config: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("ambiguous: {0:?}")]
    Ambiguous(Vec<String>),
}

pub type Result<T> = std::result::Result<T, Error>;
