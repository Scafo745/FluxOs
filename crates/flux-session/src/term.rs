//! A login shell on a pseudo-terminal, streamed over the connection in
//! frames: `[kind: u8][length: u32 BE][payload]`.

use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::Me;

pub const DATA: u8 = 0;
pub const RESIZE: u8 = 1;
pub const EXIT: u8 = 2;
const MAX_FRAME: usize = 1 << 20;

/// The Wayland socket of the user's session (cage creates `wayland-0`).
pub fn wayland_display() -> Option<String> {
    if let Ok(d) = std::env::var("WAYLAND_DISPLAY") {
        return Some(d);
    }
    let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("wayland-") && !n.ends_with(".lock"))
        .collect();
    names.sort();
    names.into_iter().next()
}

pub fn write_frame(w: &mut impl Write, kind: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut head = [0u8; 5];
    head[0] = kind;
    head[1..].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    w.write_all(&head)?;
    w.write_all(payload)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read) -> std::io::Result<Option<(u8, Vec<u8>)>> {
    let mut head = [0u8; 5];
    match r.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::other("frame too large"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(Some((head[0], buf)))
}

fn set_size(fd: i32, cols: u16, rows: u16) {
    let ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
    // SAFETY: TIOCSWINSZ reads a winsize from the pointer we pass.
    unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
}

/// Opens a pseudo-terminal pair: (master, slave).
fn open_pty() -> Result<(File, File)> {
    // SAFETY: plain libc calls; every return value is checked and the fds are owned by Files.
    unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC);
        if master < 0 {
            bail!("posix_openpt: {}", std::io::Error::last_os_error());
        }
        let master_file = File::from_raw_fd(master);
        if libc::grantpt(master) != 0 || libc::unlockpt(master) != 0 {
            bail!("grantpt/unlockpt: {}", std::io::Error::last_os_error());
        }
        let mut name = [0 as libc::c_char; 128];
        if libc::ptsname_r(master, name.as_mut_ptr(), name.len()) != 0 {
            bail!("ptsname: {}", std::io::Error::last_os_error());
        }
        let slave = libc::open(CStr::from_ptr(name.as_ptr()).as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
        if slave < 0 {
            bail!("open pty slave: {}", std::io::Error::last_os_error());
        }
        Ok((master_file, File::from_raw_fd(slave)))
    }
}

pub fn run(stream: UnixStream, me: &Me, cols: u16, rows: u16) -> Result<()> {
    let (master, slave) = open_pty()?;
    set_size(master.as_raw_fd(), cols, rows);
    let shell = me.shell.clone();
    let login = format!("-{}", shell.rsplit('/').next().unwrap_or("sh"));
    // The Command owns our copies of the slave side; dropping it at the end of
    // this block closes them, so the master reports EIO once the shell is gone.
    let mut child = {
        let mut cmd = Command::new(&shell);
        cmd.arg0(login)
            .current_dir(&me.home)
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .env("HOME", &me.home)
            .env("USER", &me.name)
            .env("LOGNAME", &me.name)
            .env("SHELL", &shell)
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));
        if let Some(display) = wayland_display() {
            cmd.env("WAYLAND_DISPLAY", display);
        }
        // SAFETY: only async-signal-safe calls: a new session with the pty as controlling terminal.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd.spawn().with_context(|| format!("start {shell}"))?
    };
    let pid = child.id() as i32;

    let mut out = stream.try_clone()?;
    writeln!(out, "{}", json!({ "ok": true, "pid": pid, "shell": shell }))?;

    // Terminal output to the connection.
    let mut reader = master.try_clone()?;
    let mut sock_out = stream.try_clone()?;
    let pump = std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break, // EIO once the shell and its children are gone
                Ok(n) => {
                    if write_frame(&mut sock_out, DATA, &buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Input and resizes from the connection.
    let mut writer = master.try_clone()?;
    let mut sock_in = stream.try_clone()?;
    let fd = master.as_raw_fd();
    let input = std::thread::spawn(move || {
        while let Ok(Some((kind, payload))) = read_frame(&mut sock_in) {
            match kind {
                DATA => {
                    if writer.write_all(&payload).is_err() {
                        break;
                    }
                }
                RESIZE => {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&payload) {
                        let c = v["cols"].as_u64().unwrap_or(80).clamp(10, 500) as u16;
                        let r = v["rows"].as_u64().unwrap_or(24).clamp(4, 300) as u16;
                        set_size(fd, c, r);
                    }
                }
                _ => {}
            }
        }
        // The browser went away: hang up the shell's session.
        // SAFETY: signalling our own child's process group.
        unsafe { libc::kill(-pid, libc::SIGHUP) };
    });

    let status = child.wait()?;
    drop(master);
    let _ = pump.join();
    let code = status.code().unwrap_or(128 + std::os::unix::process::ExitStatusExt::signal(&status).unwrap_or(0));
    let _ = write_frame(&mut out, EXIT, json!({ "code": code }).to_string().as_bytes());
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = input.join();
    Ok(())
}
