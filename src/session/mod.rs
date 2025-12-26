//! Session (PTY) management

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::spawn;
#[cfg(windows)]
pub use windows::spawn;

use crate::error::Result;
use std::path::PathBuf;

/// Options for spawning a process
#[derive(Debug, Clone, Default)]
pub struct SpawnOpts {
    /// Program to execute
    pub prog: String,
    /// Arguments to pass
    pub args: Vec<String>,
    /// Environment variables (None = inherit)
    pub env: Option<Vec<(String, String)>>,
    /// Working directory
    pub dir: Option<PathBuf>,
    /// Terminal columns
    pub cols: u16,
    /// Terminal rows
    pub rows: u16,
}

impl SpawnOpts {
    pub fn new(prog: impl Into<String>) -> Self {
        SpawnOpts {
            prog: prog.into(),
            args: Vec::new(),
            env: None,
            dir: None,
            cols: 80,
            rows: 24,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn env<I, K, V>(mut self, env: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.env = Some(env.into_iter().map(|(k, v)| (k.into(), v.into())).collect());
        self
    }

    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// Session trait for PTY process control
pub trait SessionTrait: Send {
    /// Read from PTY output
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Write to PTY input
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Resize the PTY
    fn resize(&self, cols: u16, rows: u16) -> Result<()>;

    /// Wait for process to exit, returns exit code
    fn wait(&mut self) -> Result<i32>;

    /// Kill the process
    fn kill(&mut self) -> Result<()>;

    /// Get process ID
    fn pid(&self) -> u32;

    /// Close stdin to signal EOF
    fn close_stdin(&mut self) -> Result<()>;

    /// Check if session is still alive
    fn is_alive(&self) -> bool;
}
