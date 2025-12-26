//! Console (TTY) management

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

use crate::error::Result;
use std::sync::mpsc::Receiver;

/// Saved terminal state for raw mode
#[derive(Clone)]
pub struct RawState {
    #[cfg(unix)]
    pub(crate) termios: libc::termios,
    #[cfg(windows)]
    pub(crate) mode: u32,
}

// Implement Send + Sync for RawState (libc::termios is plain data)
unsafe impl Send for RawState {}
unsafe impl Sync for RawState {}

/// Console/TTY interface
pub trait ConsoleTrait: Send {
    /// Check if stdout is a TTY
    fn is_tty_out(&self) -> bool;

    /// Check if stderr is a TTY
    fn is_tty_err(&self) -> bool;

    /// Get terminal size (cols, rows)
    fn size(&self) -> (u16, u16);

    /// Enter raw mode, returns saved state
    fn make_raw(&mut self) -> Result<RawState>;

    /// Restore terminal state
    fn restore(&mut self, state: RawState) -> Result<()>;

    /// Enable virtual terminal processing (ANSI support)
    fn enable_vt(&self);

    /// Get resize notification channel
    fn on_resize(&self) -> Option<Receiver<()>>;
}
