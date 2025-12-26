//! Platform-specific PTY opening functions

use crate::error::{PtyxError, Result};
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

/// Open a PTY pair (master, slave)
#[cfg(target_os = "linux")]
pub fn open_pty() -> Result<(File, File)> {
    use nix::libc;

    // Open /dev/ptmx
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;

    let fd = master.as_raw_fd();

    // Get pts number
    let mut pts_num: libc::c_int = 0;
    let ret = unsafe { libc::ioctl(fd, libc::TIOCGPTN, &mut pts_num) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    // Unlock pts
    let mut unlock: libc::c_int = 0;
    let ret = unsafe { libc::ioctl(fd, libc::TIOCSPTLCK, &mut unlock) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    // Open slave
    let slave_path = format!("/dev/pts/{}", pts_num);
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_path)?;

    Ok((master, slave))
}

#[cfg(target_os = "macos")]
pub fn open_pty() -> Result<(File, File)> {
    use nix::libc;

    // Open /dev/ptmx
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;

    let fd = master.as_raw_fd();

    // Grant access
    let ret = unsafe { libc::ioctl(fd, libc::TIOCPTYGRANT as _) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    // Unlock
    let ret = unsafe { libc::ioctl(fd, libc::TIOCPTYUNLK as _) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    // Get slave name
    let mut name_buf = [0u8; 128];
    let ret = unsafe { libc::ioctl(fd, libc::TIOCPTYGNAME as _, name_buf.as_mut_ptr()) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    let slave_name = unsafe {
        CStr::from_ptr(name_buf.as_ptr() as *const _)
            .to_string_lossy()
            .into_owned()
    };

    // Open slave
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_name)?;

    Ok((master, slave))
}

#[cfg(target_os = "freebsd")]
pub fn open_pty() -> Result<(File, File)> {
    use nix::libc;
    use std::fs;

    // Open /dev/ptmx
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;

    let fd = master.as_raw_fd();

    // Get slave name via readlink on /dev/fd/<fd>
    let fd_path = format!("/dev/fd/{}", fd);
    let slave_name = fs::read_link(&fd_path)?;

    // Open slave
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_name)?;

    Ok((master, slave))
}

#[cfg(target_os = "netbsd")]
pub fn open_pty() -> Result<(File, File)> {
    use nix::libc;
    use nix::pty::ptsname_r;

    // Open /dev/ptmx
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;

    let fd = master.as_raw_fd();

    // grantpt and unlockpt
    let ret = unsafe { libc::grantpt(fd) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    let ret = unsafe { libc::unlockpt(fd) };
    if ret < 0 {
        return Err(PtyxError::Io(std::io::Error::last_os_error()));
    }

    // Get slave name via ptsname
    let slave_name = unsafe {
        let name = libc::ptsname(fd);
        if name.is_null() {
            return Err(PtyxError::Io(std::io::Error::last_os_error()));
        }
        CStr::from_ptr(name).to_string_lossy().into_owned()
    };

    // Open slave
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_name)?;

    Ok((master, slave))
}

#[cfg(target_os = "openbsd")]
pub fn open_pty() -> Result<(File, File)> {
    use nix::libc;

    // OpenBSD uses legacy pty devices
    // Try /dev/ptyXY where X is [p-za-e] and Y is [0-9a-f]
    let pty_chars = "pqrstuvwxyzabcde";
    let tty_nums = "0123456789abcdef";

    for p in pty_chars.chars() {
        for n in tty_nums.chars() {
            let pty_name = format!("/dev/pty{}{}", p, n);
            let tty_name = format!("/dev/tty{}{}", p, n);

            if let Ok(master) = OpenOptions::new().read(true).write(true).open(&pty_name) {
                if let Ok(slave) = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NOCTTY)
                    .open(&tty_name)
                {
                    return Ok((master, slave));
                }
            }
        }
    }

    Err(PtyxError::Pty("no available pty".into()))
}

#[cfg(target_os = "dragonfly")]
pub fn open_pty() -> Result<(File, File)> {
    // DragonFlyBSD is similar to FreeBSD
    open_pty_freebsd_style()
}

#[cfg(target_os = "dragonfly")]
fn open_pty_freebsd_style() -> Result<(File, File)> {
    use nix::libc;
    use std::fs;

    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;

    let fd = master.as_raw_fd();
    let fd_path = format!("/dev/fd/{}", fd);
    let slave_name = fs::read_link(&fd_path)?;

    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_name)?;

    Ok((master, slave))
}
