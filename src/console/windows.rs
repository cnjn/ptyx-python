//! Windows Console implementation

use crate::console::{ConsoleTrait, RawState};
use crate::error::{PtyxError, Result};
use std::fs::File;
use std::io::{self, Stdin, Stdout};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    GetConsoleMode, GetConsoleScreenBufferInfo, GetStdHandle, SetConsoleMode,
    CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, ENABLE_ECHO_INPUT,
    ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

/// Windows Console implementation
pub struct Console {
    stdin: Stdin,
    stdout: Stdout,
    stdin_handle: HANDLE,
    stdout_handle: HANDLE,
    stderr_handle: HANDLE,
    out_tty: bool,
    err_tty: bool,
    original_mode: Option<u32>,
    resize_tx: Option<Sender<()>>,
    resize_rx: Option<Receiver<()>>,
    stop_resize: Arc<Mutex<bool>>,
}

impl Console {
    /// Create a new Console
    pub fn new() -> Result<Self> {
        let stdin = io::stdin();
        let stdout = io::stdout();

        let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE)? };
        let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE)? };
        let stderr_handle = unsafe { GetStdHandle(STD_ERROR_HANDLE)? };

        let out_tty = is_console_handle(stdout_handle);
        let err_tty = is_console_handle(stderr_handle);

        let (resize_tx, resize_rx, stop_resize) = if out_tty {
            let (tx, rx) = mpsc::channel();
            let stop = Arc::new(Mutex::new(false));
            let stop_clone = Arc::clone(&stop);
            let tx_clone = tx.clone();
            let handle = stdout_handle;

            // Start resize polling thread
            thread::spawn(move || {
                let mut last_size = get_console_size(handle);
                loop {
                    {
                        if *stop_clone.lock().unwrap() {
                            break;
                        }
                    }
                    thread::sleep(Duration::from_millis(200));
                    let new_size = get_console_size(handle);
                    if new_size != last_size {
                        last_size = new_size;
                        let _ = tx_clone.send(());
                    }
                }
            });

            (Some(tx), Some(rx), stop)
        } else {
            (None, None, Arc::new(Mutex::new(false)))
        };

        Ok(Console {
            stdin,
            stdout,
            stdin_handle,
            stdout_handle,
            stderr_handle,
            out_tty,
            err_tty,
            original_mode: None,
            resize_tx,
            resize_rx,
            stop_resize,
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

fn is_console_handle(handle: HANDLE) -> bool {
    let mut mode = CONSOLE_MODE::default();
    unsafe { GetConsoleMode(handle, &mut mode).is_ok() }
}

fn get_console_size(handle: HANDLE) -> (u16, u16) {
    let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
    unsafe {
        if GetConsoleScreenBufferInfo(handle, &mut info).is_ok() {
            let cols = (info.srWindow.Right - info.srWindow.Left + 1) as u16;
            let rows = (info.srWindow.Bottom - info.srWindow.Top + 1) as u16;
            (cols, rows)
        } else {
            (80, 24)
        }
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
        get_console_size(self.stdout_handle)
    }

    fn make_raw(&mut self) -> Result<RawState> {
        if !self.out_tty {
            return Err(PtyxError::NotAConsole);
        }

        let mut mode = CONSOLE_MODE::default();
        unsafe {
            GetConsoleMode(self.stdin_handle, &mut mode)?;
        }

        self.original_mode = Some(mode.0);

        // Disable line input, echo, and processed input
        let new_mode = CONSOLE_MODE(
            mode.0 & !(ENABLE_LINE_INPUT.0 | ENABLE_ECHO_INPUT.0 | ENABLE_PROCESSED_INPUT.0),
        );

        unsafe {
            SetConsoleMode(self.stdin_handle, new_mode)?;
        }

        Ok(RawState { mode: mode.0 })
    }

    fn restore(&mut self, state: RawState) -> Result<()> {
        if !self.out_tty {
            return Ok(());
        }

        unsafe {
            SetConsoleMode(self.stdin_handle, CONSOLE_MODE(state.mode))?;
        }
        Ok(())
    }

    fn enable_vt(&self) {
        if !self.out_tty {
            return;
        }

        let mut mode = CONSOLE_MODE::default();
        unsafe {
            if GetConsoleMode(self.stdout_handle, &mut mode).is_ok() {
                let new_mode = CONSOLE_MODE(mode.0 | ENABLE_VIRTUAL_TERMINAL_PROCESSING.0);
                let _ = SetConsoleMode(self.stdout_handle, new_mode);
            }
        }
    }

    fn on_resize(&self) -> Option<Receiver<()>> {
        // Return a clone of the receiver isn't possible, so we return None
        // The resize polling is internal
        None
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        // Stop resize polling thread
        *self.stop_resize.lock().unwrap() = true;

        // Restore original mode
        if let Some(mode) = self.original_mode {
            unsafe {
                let _ = SetConsoleMode(self.stdin_handle, CONSOLE_MODE(mode));
            }
        }
    }
}
