//! Unix Console implementation

use crate::console::{ConsoleTrait, RawState};
use crate::error::{PtyxError, Result};
use nix::libc;
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use std::io::{self, Stdin, Stdout};
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, Once};

static RESIZE_INIT: Once = Once::new();
static RESIZE_FLAG: AtomicBool = AtomicBool::new(false);
static RESIZE_SENDER: Mutex<Option<Sender<()>>> = Mutex::new(None);

extern "C" fn sigwinch_handler(_: libc::c_int) {
    RESIZE_FLAG.store(true, Ordering::SeqCst);
    if let Ok(guard) = RESIZE_SENDER.lock() {
        if let Some(ref sender) = *guard {
            let _ = sender.send(());
        }
    }
}

fn is_tty(fd: RawFd) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

/// Unix Console implementation
pub struct Console {
    stdin: Stdin,
    stdout: Stdout,
    out_tty: bool,
    err_tty: bool,
    original_termios: Option<libc::termios>,
}

impl Console {
    /// Create a new Console
    pub fn new() -> Result<Self> {
        let stdin = io::stdin();
        let stdout = io::stdout();

        let out_tty = is_tty(libc::STDOUT_FILENO);
        let err_tty = is_tty(libc::STDERR_FILENO);

        Ok(Console {
            stdin,
            stdout,
            out_tty,
            err_tty,
            original_termios: None,
        })
    }

    /// Get stdin
    pub fn stdin(&mut self) -> &mut Stdin {
        &mut self.stdin
    }

    /// Get stdout
    pub fn stdout(&mut self) -> &mut Stdout {
        &mut self.stdout
    }
}

impl ConsoleTrait for Console {
    fn is_tty_out(&self) -> bool {
        self.out_tty
    }

    fn is_tty_err(&self) -> bool {
        self.err_tty
    }

    fn size(&self) -> (u16, u16) {
        if !self.out_tty {
            return (80, 24);
        }

        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 {
                (ws.ws_col, ws.ws_row)
            } else {
                (80, 24)
            }
        }
    }

    fn make_raw(&mut self) -> Result<RawState> {
        if !self.out_tty {
            return Err(PtyxError::NotAConsole);
        }

        // Use stdin for termios operations (STDIN_FILENO)
        let fd = libc::STDIN_FILENO;

        // Get current termios using libc directly
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::tcgetattr(fd, &mut original) };
        if ret < 0 {
            return Err(PtyxError::Io(std::io::Error::last_os_error()));
        }

        self.original_termios = Some(original.clone());

        // Create raw mode
        let mut raw = original.clone();
        unsafe {
            libc::cfmakeraw(&mut raw);
            let ret = libc::tcsetattr(fd, libc::TCSANOW, &raw);
            if ret < 0 {
                return Err(PtyxError::Io(std::io::Error::last_os_error()));
            }
        }

        Ok(RawState { termios: original })
    }

    fn restore(&mut self, state: RawState) -> Result<()> {
        if !self.out_tty {
            return Ok(());
        }

        let fd = libc::STDIN_FILENO;
        let ret = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &state.termios) };
        if ret < 0 {
            return Err(PtyxError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn enable_vt(&self) {
        // No-op on Unix, VT is always enabled
    }

    fn on_resize(&self) -> Option<Receiver<()>> {
        if self.out_tty {
            // Setup SIGWINCH handler if not already done
            RESIZE_INIT.call_once(|| {
                let sa = SigAction::new(
                    SigHandler::Handler(sigwinch_handler),
                    SaFlags::SA_RESTART,
                    SigSet::empty(),
                );
                unsafe {
                    let _ = sigaction(Signal::SIGWINCH, &sa);
                }
            });
            let (tx, rx) = mpsc::channel();
            *RESIZE_SENDER.lock().unwrap() = Some(tx);
            Some(rx)
        } else {
            None
        }
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        // Restore terminal if we have saved state
        if let Some(ref termios) = self.original_termios {
            let fd = libc::STDIN_FILENO;
            unsafe {
                libc::tcsetattr(fd, libc::TCSANOW, termios);
            }
        }
    }
}
