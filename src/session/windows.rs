//! Windows ConPTY session implementation

use crate::error::{PtyxError, Result};
use crate::session::{SessionTrait, SpawnOpts};
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, InitializeProcThreadAttributeList,
    TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
    CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    WAIT_OBJECT_0,
};

/// Windows ConPTY Session
pub struct Session {
    hpc: HPCON,
    in_write: HANDLE,
    out_read: HANDLE,
    process: HANDLE,
    thread: HANDLE,
    job: HANDLE,
    pid: u32,
    killed: AtomicBool,
    closed: AtomicBool,
}

/// Spawn a process in a ConPTY
pub fn spawn(opts: SpawnOpts) -> Result<Session> {
    if opts.prog.is_empty() {
        return Err(PtyxError::EmptyProgram);
    }

    let cols = if opts.cols > 0 { opts.cols } else { 80 };
    let rows = if opts.rows > 0 { opts.rows } else { 24 };

    unsafe {
        // Create pipes for PTY I/O
        let mut pty_in_read = HANDLE::default();
        let mut pty_in_write = HANDLE::default();
        let mut pty_out_read = HANDLE::default();
        let mut pty_out_write = HANDLE::default();

        CreatePipe(&mut pty_in_read, &mut pty_in_write, None, 0)?;
        CreatePipe(&mut pty_out_read, &mut pty_out_write, None, 0)?;

        // Create pseudo console
        let size = COORD {
            X: cols as i16,
            Y: rows as i16,
        };
        let mut hpc = HPCON::default();
        CreatePseudoConsole(size, pty_in_read, pty_out_write, 0, &mut hpc)?;

        // Close handles that are now owned by the pseudo console
        CloseHandle(pty_in_read)?;
        CloseHandle(pty_out_write)?;

        // Initialize proc thread attribute list
        let mut attr_list_size: usize = 0;
        let _ = InitializeProcThreadAttributeList(
            LPPROC_THREAD_ATTRIBUTE_LIST::default(),
            1,
            0,
            &mut attr_list_size,
        );

        let attr_list_buf = vec![0u8; attr_list_size];
        let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_list_buf.as_ptr() as *mut _);
        InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_list_size)?;

        // Update attribute with pseudo console
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc.0 as *const _),
            mem::size_of::<HPCON>(),
            None,
            None,
        )?;

        // Build command line
        let cmdline = build_cmdline(&opts.prog, &opts.args);
        let mut cmdline_wide: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();

        // Build environment block
        let env_block = build_env_block(opts.env.as_ref());

        // Setup startup info
        let mut si = STARTUPINFOEXW::default();
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.lpAttributeList = attr_list;

        let mut pi = PROCESS_INFORMATION::default();

        let flags = CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT | CREATE_NEW_PROCESS_GROUP;

        let dir: Option<Vec<u16>> = opts.dir.as_ref().map(|d| {
            d.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        });

        CreateProcessW(
            None,
            PCWSTR(cmdline_wide.as_mut_ptr()),
            None,
            None,
            false,
            flags,
            env_block.as_ref().map(|v| v.as_ptr() as *const _),
            dir.as_ref().map(|v| PCWSTR(v.as_ptr())),
            &si.StartupInfo,
            &mut pi,
        )?;

        // Create job object for cleanup
        let job = CreateJobObjectW(None, None)?;
        let mut ext_info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        ext_info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &ext_info as *const _ as *const _,
            mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;
        AssignProcessToJobObject(job, pi.hProcess)?;

        // Forget the attribute list buffer to prevent deallocation issues
        mem::forget(attr_list_buf);

        Ok(Session {
            hpc,
            in_write: pty_in_write,
            out_read: pty_out_read,
            process: pi.hProcess,
            thread: pi.hThread,
            job,
            pid: pi.dwProcessId,
            killed: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }
}

fn build_cmdline(prog: &str, args: &[String]) -> String {
    let mut cmdline = escape_arg(prog);
    for arg in args {
        cmdline.push(' ');
        cmdline.push_str(&escape_arg(arg));
    }
    cmdline
}

fn escape_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }

    let needs_quote = arg.contains(' ') || arg.contains('\t') || arg.contains('"');
    if !needs_quote {
        return arg.to_string();
    }

    let mut result = String::with_capacity(arg.len() + 2);
    result.push('"');

    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else if c == '"' {
            for _ in 0..backslashes {
                result.push('\\');
            }
            result.push('\\');
            result.push('"');
            backslashes = 0;
        } else {
            for _ in 0..backslashes {
                result.push('\\');
            }
            result.push(c);
            backslashes = 0;
        }
    }

    for _ in 0..backslashes {
        result.push('\\');
    }
    result.push('"');
    result
}

fn build_env_block(env: Option<&Vec<(String, String)>>) -> Option<Vec<u16>> {
    let env = env?;
    let mut block = String::new();
    for (k, v) in env {
        block.push_str(k);
        block.push('=');
        block.push_str(v);
        block.push('\0');
    }
    block.push('\0');
    Some(block.encode_utf16().collect())
}

impl SessionTrait for Session {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        use windows::Win32::Storage::FileSystem::ReadFile;

        let mut bytes_read = 0u32;
        unsafe {
            ReadFile(self.out_read, Some(buf), Some(&mut bytes_read), None)?;
        }
        Ok(bytes_read as usize)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        use windows::Win32::Storage::FileSystem::WriteFile;

        let mut bytes_written = 0u32;
        unsafe {
            WriteFile(self.in_write, Some(buf), Some(&mut bytes_written), None)?;
        }
        Ok(bytes_written as usize)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let size = COORD {
            X: cols as i16,
            Y: rows as i16,
        };
        unsafe {
            ResizePseudoConsole(self.hpc, size)?;
        }
        Ok(())
    }

    fn wait(&mut self) -> Result<i32> {
        unsafe {
            let result = WaitForSingleObject(self.process, INFINITE);
            if result != WAIT_OBJECT_0 {
                return Err(PtyxError::Pty("wait failed".into()));
            }

            let mut exit_code = 0u32;
            GetExitCodeProcess(self.process, &mut exit_code)?;

            if self.killed.load(Ordering::SeqCst) {
                return Err(PtyxError::ExitError {
                    exit_code: exit_code as i32,
                });
            }

            if exit_code != 0 {
                return Err(PtyxError::ExitError {
                    exit_code: exit_code as i32,
                });
            }

            Ok(0)
        }
    }

    fn kill(&mut self) -> Result<()> {
        self.killed.store(true, Ordering::SeqCst);
        unsafe {
            if self.job != HANDLE::default() {
                let _ = CloseHandle(self.job);
                self.job = HANDLE::default();
            }
            let _ = TerminateProcess(self.process, 1);
        }
        Ok(())
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn close_stdin(&mut self) -> Result<()> {
        unsafe {
            if self.in_write != HANDLE::default() {
                let _ = CloseHandle(self.in_write);
                self.in_write = HANDLE::default();
            }
        }
        Ok(())
    }

    fn is_alive(&self) -> bool {
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }
        unsafe {
            let result = WaitForSingleObject(self.process, 0);
            result != WAIT_OBJECT_0
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        unsafe {
            if self.job != HANDLE::default() {
                let _ = CloseHandle(self.job);
            }
            ClosePseudoConsole(self.hpc);
            if self.in_write != HANDLE::default() {
                let _ = CloseHandle(self.in_write);
            }
            if self.out_read != HANDLE::default() {
                let _ = CloseHandle(self.out_read);
            }
            if self.process != HANDLE::default() {
                let _ = CloseHandle(self.process);
            }
            if self.thread != HANDLE::default() {
                let _ = CloseHandle(self.thread);
            }
        }
    }
}

// Mark as Send (ConPTY handles are safe to send between threads)
unsafe impl Send for Session {}
