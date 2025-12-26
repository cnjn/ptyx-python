//! I/O Multiplexer - bridges Console and Session

use crate::console::Console;
use crate::error::{PtyxError, Result};
use crate::session::SessionTrait;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

const MUX_INIT: u8 = 0;
const MUX_RUNNING: u8 = 1;

#[allow(dead_code)]
const MUX_STOPPED: u8 = 2;

/// I/O Multiplexer that bridges Console and Session
pub struct Mux {
    state: Arc<AtomicU8>,
    #[allow(dead_code)]
    stop_flag: Arc<AtomicBool>,
}

impl Mux {
    /// Create a new Mux
    pub fn new() -> Self {
        Mux {
            state: Arc::new(AtomicU8::new(MUX_INIT)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if mux is running
    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::SeqCst) == MUX_RUNNING
    }
}

impl Default for Mux {
    fn default() -> Self {
        Self::new()
    }
}

/// Run an interactive session (blocking)
/// This is a convenience function that sets up raw mode, multiplexes I/O,
/// and handles cleanup
pub fn run_interactive<S: SessionTrait + 'static>(
    mut console: Console,
    mut session: S,
) -> Result<i32> {
    use crate::console::ConsoleTrait;

    // Check if we're in a TTY
    if !console.is_tty_out() {
        // Not a TTY, just run without raw mode
        return run_simple(&mut session);
    }

    // Enable VT processing
    console.enable_vt();

    // Enter raw mode
    let raw_state = console.make_raw()?;

    // Get initial size and resize session
    let (cols, rows) = console.size();
    let _ = session.resize(cols, rows);

    // Run I/O copy
    let result = run_copy(&mut session);

    // Restore terminal
    let _ = console.restore(raw_state);

    result
}

fn run_simple<S: SessionTrait>(session: &mut S) -> Result<i32> {
    run_copy(session)
}

fn run_copy<S: SessionTrait>(session: &mut S) -> Result<i32> {
    let mut buf = [0u8; 4096];
    let stdout = std::io::stdout();

    // Simple copy loop: read from session, write to stdout
    loop {
        match session.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut stdout = stdout.lock();
                if stdout.write_all(&buf[..n]).is_err() {
                    break;
                }
                let _ = stdout.flush();
            }
            Err(PtyxError::ExitError { exit_code }) => {
                return Ok(exit_code);
            }
            Err(_) => {
                // Check if session is still alive
                if !session.is_alive() {
                    break;
                }
            }
        }
    }

    session.wait()
}
