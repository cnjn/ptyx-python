//! ptyx - Cross-platform PTY/TTY management library
//!
//! This library provides a simple, cross-platform API for managing
//! pseudo-terminals (PTY) and terminal TTYs.

pub mod ansi;
pub mod console;
pub mod error;
pub mod mux;
pub mod session;

use console::{Console as RustConsole, ConsoleTrait, RawState as RustRawState};
use error::PtyxError;
use pyo3::exceptions::{PyOSError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use session::{spawn as rust_spawn, SessionTrait, SpawnOpts};
use std::path::PathBuf;

/// Raw terminal state (opaque handle)
#[pyclass(frozen)]
#[derive(Clone)]
struct RawState {
    inner: RustRawState,
}

/// Console - TTY control
#[pyclass(unsendable)]
struct Console {
    inner: Option<RustConsole>,
}

#[pymethods]
impl Console {
    /// Create a new Console
    #[new]
    fn new() -> PyResult<Self> {
        let inner = RustConsole::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Console { inner: Some(inner) })
    }

    /// Check if stdout is a TTY
    #[getter]
    fn is_tty(&self) -> PyResult<bool> {
        let inner = self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        Ok(inner.is_tty_out())
    }

    /// Check if stderr is a TTY
    #[getter]
    fn is_tty_err(&self) -> PyResult<bool> {
        let inner = self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        Ok(inner.is_tty_err())
    }

    /// Get terminal size as (cols, rows)
    #[getter]
    fn size(&self) -> PyResult<(u16, u16)> {
        let inner = self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        Ok(inner.size())
    }

    /// Enter raw mode, returns state to restore later
    fn make_raw(&mut self) -> PyResult<RawState> {
        let inner = self.inner.as_mut().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        let state = inner.make_raw().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(RawState { inner: state })
    }

    /// Restore terminal state from raw mode
    fn restore(&mut self, state: &RawState) -> PyResult<()> {
        let inner = self.inner.as_mut().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        inner.restore(state.inner.clone()).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Enable virtual terminal processing (ANSI support)
    fn enable_vt(&self) -> PyResult<()> {
        let inner = self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Console already closed")
        })?;
        inner.enable_vt();
        Ok(())
    }

    /// Close the console
    fn close(&mut self) -> PyResult<()> {
        self.inner = None;
        Ok(())
    }

    /// Context manager enter
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false) // Don't suppress exceptions
    }
}

/// Session - PTY process control
#[pyclass]
struct Session {
    #[cfg(unix)]
    inner: Option<session::unix::Session>,
    #[cfg(windows)]
    inner: Option<session::windows::Session>,
}

#[pymethods]
impl Session {
    /// Get process ID
    #[getter]
    fn pid(&self) -> PyResult<u32> {
        let inner = self.get_inner()?;
        Ok(inner.pid())
    }

    /// Check if process is still alive
    #[getter]
    fn is_alive(&self) -> PyResult<bool> {
        let inner = self.get_inner()?;
        Ok(inner.is_alive())
    }

    /// Read data from PTY (up to max_bytes)
    fn read(&mut self, max_bytes: usize, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let inner = self.get_inner_mut()?;
        let mut buf = vec![0u8; max_bytes];
        let n = inner.read(&mut buf).map_err(|e| PyOSError::new_err(e.to_string()))?;
        buf.truncate(n);
        Ok(PyBytes::new(py, &buf).into())
    }

    /// Read data from PTY with timeout (milliseconds). Returns empty bytes on timeout.
    #[cfg(unix)]
    #[pyo3(signature = (max_bytes, timeout_ms=100))]
    fn read_timeout(&mut self, max_bytes: usize, timeout_ms: i32, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let inner = self.get_inner_mut()?;
        let mut buf = vec![0u8; max_bytes];
        let n = inner.read_timeout(&mut buf, timeout_ms).map_err(|e| PyOSError::new_err(e.to_string()))?;
        buf.truncate(n);
        Ok(PyBytes::new(py, &buf).into())
    }

    /// Write data to PTY
    fn write(&mut self, data: &[u8]) -> PyResult<usize> {
        let inner = self.get_inner_mut()?;
        inner.write(data).map_err(|e| PyOSError::new_err(e.to_string()))
    }

    /// Resize the PTY
    fn resize(&self, cols: u16, rows: u16) -> PyResult<()> {
        let inner = self.get_inner()?;
        inner.resize(cols, rows).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Wait for process to exit, returns exit code
    fn wait(&mut self) -> PyResult<i32> {
        let inner = self.get_inner_mut()?;
        match inner.wait() {
            Ok(code) => Ok(code),
            Err(PtyxError::ExitError { exit_code }) => Ok(exit_code),
            Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
        }
    }

    /// Kill the process
    fn kill(&mut self) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        inner.kill().map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Close stdin to signal EOF
    fn close_stdin(&mut self) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        inner.close_stdin().map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Close the session
    fn close(&mut self) -> PyResult<()> {
        self.inner = None;
        Ok(())
    }

    /// Context manager enter
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}

impl Session {
    #[cfg(unix)]
    fn get_inner(&self) -> PyResult<&session::unix::Session> {
        self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Session already closed")
        })
    }

    #[cfg(unix)]
    fn get_inner_mut(&mut self) -> PyResult<&mut session::unix::Session> {
        self.inner.as_mut().ok_or_else(|| {
            PyRuntimeError::new_err("Session already closed")
        })
    }

    #[cfg(windows)]
    fn get_inner(&self) -> PyResult<&session::windows::Session> {
        self.inner.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("Session already closed")
        })
    }

    #[cfg(windows)]
    fn get_inner_mut(&mut self) -> PyResult<&mut session::windows::Session> {
        self.inner.as_mut().ok_or_else(|| {
            PyRuntimeError::new_err("Session already closed")
        })
    }
}

/// Spawn a process in a PTY
#[pyfunction]
#[pyo3(signature = (prog, args=None, env=None, dir=None, cols=80, rows=24))]
fn spawn(
    prog: &str,
    args: Option<Vec<String>>,
    env: Option<Vec<(String, String)>>,
    dir: Option<&str>,
    cols: u16,
    rows: u16,
) -> PyResult<Session> {
    let opts = SpawnOpts {
        prog: prog.to_string(),
        args: args.unwrap_or_default(),
        env,
        dir: dir.map(PathBuf::from),
        cols,
        rows,
    };

    let session = rust_spawn(opts).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    Ok(Session {
        inner: Some(session),
    })
}

/// Run a command (non-interactive)
#[pyfunction]
#[pyo3(signature = (prog, args=None, env=None, dir=None))]
fn run(
    prog: &str,
    args: Option<Vec<String>>,
    env: Option<Vec<(String, String)>>,
    dir: Option<&str>,
) -> PyResult<i32> {
    let opts = SpawnOpts {
        prog: prog.to_string(),
        args: args.unwrap_or_default(),
        env,
        dir: dir.map(PathBuf::from),
        cols: 80,
        rows: 24,
    };

    let mut session = rust_spawn(opts).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    // Simple wait
    match session.wait() {
        Ok(code) => Ok(code),
        Err(PtyxError::ExitError { exit_code }) => Ok(exit_code),
        Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
    }
}

/// Run a command interactively (with console I/O bridging)
#[pyfunction]
#[pyo3(signature = (prog, args=None, env=None, dir=None))]
fn run_interactive(
    prog: &str,
    args: Option<Vec<String>>,
    env: Option<Vec<(String, String)>>,
    dir: Option<&str>,
) -> PyResult<i32> {
    let console = RustConsole::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    let (cols, rows) = console.size();

    let opts = SpawnOpts {
        prog: prog.to_string(),
        args: args.unwrap_or_default(),
        env,
        dir: dir.map(PathBuf::from),
        cols,
        rows,
    };

    let session = rust_spawn(opts).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    match mux::run_interactive(console, session) {
        Ok(code) => Ok(code),
        Err(PtyxError::ExitError { exit_code }) => Ok(exit_code),
        Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
    }
}

/// ANSI escape sequence helpers
#[pyfunction]
fn csi(seq: &str) -> String {
    ansi::csi(seq)
}

#[pyfunction]
fn sgr(codes: Vec<u32>) -> String {
    ansi::sgr(&codes)
}

#[pyfunction]
fn clear_screen() -> &'static str {
    ansi::clear_screen()
}

#[pyfunction]
fn cursor_home() -> &'static str {
    ansi::cursor_home()
}

#[pyfunction]
fn cursor_to(row: u16, col: u16) -> String {
    ansi::cursor_to(row, col)
}

#[pyfunction]
fn cursor_hide() -> &'static str {
    ansi::cursor_hide()
}

#[pyfunction]
fn cursor_show() -> &'static str {
    ansi::cursor_show()
}

/// Python module
#[pymodule]
fn ptyx(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<Console>()?;
    m.add_class::<Session>()?;
    m.add_class::<RawState>()?;

    // Functions
    m.add_function(wrap_pyfunction!(spawn, m)?)?;
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(run_interactive, m)?)?;

    // ANSI helpers
    m.add_function(wrap_pyfunction!(csi, m)?)?;
    m.add_function(wrap_pyfunction!(sgr, m)?)?;
    m.add_function(wrap_pyfunction!(clear_screen, m)?)?;
    m.add_function(wrap_pyfunction!(cursor_home, m)?)?;
    m.add_function(wrap_pyfunction!(cursor_to, m)?)?;
    m.add_function(wrap_pyfunction!(cursor_hide, m)?)?;
    m.add_function(wrap_pyfunction!(cursor_show, m)?)?;

    Ok(())
}
