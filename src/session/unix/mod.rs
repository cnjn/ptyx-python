//! Unix PTY session implementation

mod pty;

use crate::error::{PtyxError, Result};
use crate::session::{SessionTrait, SpawnOpts};
use nix::libc;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

/// Unix PTY Session
pub struct Session {
    master: File,
    child: Option<Child>,
    pid: u32,
    closed: AtomicBool,
}

impl Session {
    /// Set master fd to non-blocking mode
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        let fd = self.master.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 {
                return Err(PtyxError::Io(std::io::Error::last_os_error()));
            }
            let new_flags = if nonblocking {
                flags | libc::O_NONBLOCK
            } else {
                flags & !libc::O_NONBLOCK
            };
            if libc::fcntl(fd, libc::F_SETFL, new_flags) < 0 {
                return Err(PtyxError::Io(std::io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    /// Read with timeout (milliseconds). Returns Ok(0) on timeout.
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let fd = self.master.as_raw_fd();

        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };

        if ret < 0 {
            return Err(PtyxError::Io(std::io::Error::last_os_error()));
        }

        if ret == 0 {
            // Timeout
            return Ok(0);
        }

        if pfd.revents & libc::POLLIN != 0 {
            Ok(self.master.read(buf)?)
        } else if pfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            // Connection closed or error
            Ok(0)
        } else {
            Ok(0)
        }
    }
}

/// Spawn a process in a PTY
pub fn spawn(opts: SpawnOpts) -> Result<Session> {
    if opts.prog.is_empty() {
        return Err(PtyxError::EmptyProgram);
    }

    // Open PTY pair
    let (master, slave) = pty::open_pty()?;

    // Set window size
    if opts.cols > 0 && opts.rows > 0 {
        set_winsize(master.as_raw_fd(), opts.cols, opts.rows)?;
    }

    // Build command
    let mut cmd = Command::new(&opts.prog);
    cmd.args(&opts.args);

    if let Some(ref env) = opts.env {
        cmd.env_clear();
        for (k, v) in env {
            cmd.env(k, v);
        }
    }

    if let Some(ref dir) = opts.dir {
        cmd.current_dir(dir);
    }

    // Convert slave to raw fd and use it for stdio
    // This transfers ownership to Stdio, so slave won't be double-closed
    let slave_fd = slave.into_raw_fd();
    unsafe {
        // Dup the fd for stdout and stderr since Stdio takes ownership
        let stdout_fd = libc::dup(slave_fd);
        let stderr_fd = libc::dup(slave_fd);

        cmd.stdin(Stdio::from_raw_fd(slave_fd));
        cmd.stdout(Stdio::from_raw_fd(stdout_fd));
        cmd.stderr(Stdio::from_raw_fd(stderr_fd));
    }

    // Create new session and set controlling terminal
    unsafe {
        cmd.pre_exec(move || {
            // Create new session
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Set controlling terminal
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    // Spawn the process
    let child = cmd.spawn()?;
    let pid = child.id();

    Ok(Session {
        master,
        child: Some(child),
        pid,
        closed: AtomicBool::new(false),
    })
}

fn set_winsize(fd: RawFd, cols: u16, rows: u16) -> Result<()> {
    let ws = libc::winsize {
        ws_col: cols,
        ws_row: rows,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ret = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
    if ret < 0 {
        Err(PtyxError::Io(std::io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

impl SessionTrait for Session {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.master.read(buf)?)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(self.master.write(buf)?)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        set_winsize(self.master.as_raw_fd(), cols, rows)
    }

    fn wait(&mut self) -> Result<i32> {
        if let Some(ref mut child) = self.child {
            let status = child.wait()?;
            if let Some(code) = status.code() {
                if code != 0 {
                    return Err(PtyxError::ExitError { exit_code: code });
                }
                Ok(code)
            } else {
                // Killed by signal
                Err(PtyxError::ExitError { exit_code: -1 })
            }
        } else {
            Ok(0)
        }
    }

    fn kill(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            child.kill()?;
        }
        Ok(())
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn close_stdin(&mut self) -> Result<()> {
        // On Unix, closing master effectively closes stdin
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn is_alive(&self) -> bool {
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }
        if let Some(ref child) = self.child {
            // Use kill with signal 0 to check if process exists
            // This doesn't actually send a signal, just checks if process exists
            unsafe {
                libc::kill(child.id() as i32, 0) == 0
            }
        } else {
            false
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Try to kill process if still running
        if self.is_alive() {
            let _ = self.kill();
        }
    }
}
