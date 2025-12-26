//! Error types for ptyx

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::fmt;
use thiserror::Error;

/// Main error type for ptyx operations
#[derive(Error, Debug)]
pub enum PtyxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not a console/TTY")]
    NotAConsole,

    #[error("Empty program name")]
    EmptyProgram,

    #[error("PTY error: {0}")]
    Pty(String),

    #[error("Process exited with code {exit_code}")]
    ExitError { exit_code: i32 },

    #[error("Mux already started")]
    MuxAlreadyStarted,

    #[error("Session closed")]
    SessionClosed,

    #[cfg(unix)]
    #[error("Unix error: {0}")]
    Unix(#[from] nix::Error),

    #[cfg(windows)]
    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),
}

impl From<PtyxError> for PyErr {
    fn from(err: PtyxError) -> PyErr {
        PyRuntimeError::new_err(err.to_string())
    }
}

/// Exit error with exit code
#[derive(Debug, Clone)]
pub struct ExitError {
    pub exit_code: i32,
}

impl fmt::Display for ExitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "process exited with code {}", self.exit_code)
    }
}

impl std::error::Error for ExitError {}

pub type Result<T> = std::result::Result<T, PtyxError>;
