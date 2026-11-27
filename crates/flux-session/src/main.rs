//! flux-session: the FluxOS UI's door into the logged-in user's session.
//!
//! It runs as the console user (a systemd user service, see
//! `nix/module.nix`) and serves fluxd over a unix socket that only that user
//! and root can open: `$XDG_RUNTIME_DIR/fluxos/session.sock`. fluxd checks
//! the paired screen's token and relays requests from the web UI. Everything
//! here runs with the user's own permissions, so the terminal is that user's
//! shell, file listings follow that user's access rights, and only that
//! user's processes can be signalled.
//!
//! Protocol: one JSON request line per connection, one JSON response line.
//! `{"op":"term"}` turns the connection into a framed terminal stream:
//! `[kind: u8][length: u32 big-endian][payload]`, kind 0 = bytes, 1 = resize
//! (`{"cols":..,"rows":..}`, to the terminal), 2 = exit (`{"code":..}`, from it).

mod apps;
mod procs;
mod term;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde_json::{Value, json};

const MAX_REQUEST: usize = 64 * 1024;
const MAX_ENTRIES: usize = 5000;
const MAX_TEXT: u64 = 512 * 1024;
const MAX_IMAGE: u64 = 8 * 1024 * 1024;

/// The account this service runs as, from /etc/passwd.
#[derive(Clone)]
pub struct Me {
    pub uid: u32,
    pub name: String,
    pub home: PathBuf,
    pub shell: String,
}

impl Me {
    fn load() -> Result<Self> {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let passwd = fs::read_to_string("/etc/passwd").context("read /etc/passwd")?;
        for line in passwd.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() >= 7 && f[2].parse::<u32>().ok() == Some(uid) {
                let shell = if f[6].is_empty() || f[6].ends_with("nologin") { "/bin/sh" } else { f[6] };
                return Ok(Me { uid, name: f[0].into(), home: PathBuf::from(f[5]), shell: shell.into() });
            }
        }
        bail!("uid {uid} is not in /etc/passwd")
    }
}

/// uid -> name for every account, for process listings.
pub fn user_names() -> std::collections::HashMap<u32, String> {
    fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            Some((f.get(2)?.parse().ok()?, f.first()?.to_string()))
        })
        .collect()
}

fn socket_path() -> Result<PathBuf> {
    if let Some(p) = std::env::args().skip_while(|a| a != "--socket").nth(1) {
        return Ok(PathBuf::from(p));
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?;
    Ok(PathBuf::from(runtime).join("fluxos").join("session.sock"))
}

/// `~` and `~/x` relative to the home directory; everything else must be absolute.
fn resolve(path: &str, me: &Me) -> Result<PathBuf> {
    let p = if path.is_empty() || path == "~" {
        me.home.clone()
    } else if let Some(rest) = path.strip_prefix("~/") {
        me.home.join(rest)
    } else {
        PathBuf::from(path)
    };
    if !p.is_absolute() {
        bail!("path must be absolute");
    }
    // Lexical cleanup; access control is the kernel's job (we run as the user).
    let mut out = PathBuf::from("/");
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(n) => out.push(n),
            _ => {}
        }
    }
    Ok(out)
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> Option<u64> {
    t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

fn list_dir(path: &str, me: &Me) -> Result<Value> {
    let dir = resolve(path, me)?;
    let mut entries = Vec::new();
    let mut truncated = false;
    for (i, e) in fs::read_dir(&dir).with_context(|| format!("{}", dir.display()))?.enumerate() {
        if i >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        let Ok(e) = e else { continue };
        let Ok(md) = e.metadata() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        let target_dir = md.file_type().is_symlink() && fs::metadata(e.path()).map(|m| m.is_dir()).unwrap_or(false);
        let kind = if md.is_dir() || target_dir {
            "dir"
        } else if md.file_type().is_symlink() {
            "link"
        } else if md.is_file() {
            "file"
        } else {
            "other"
        };
        entries.push(json!({
            "name": name,
            "kind": kind,
            "symlink": md.file_type().is_symlink(),
            "size": md.len(),
            "modified": secs(md.modified()),
            "mode": format!("{:o}", md.permissions().mode() & 0o7777),
            "uid": md.uid(),
            "hidden": name.starts_with('.'),
        }));
    }
    entries.sort_by(|a, b| {
        let d = |v: &Value| v["kind"] != "dir";
        d(a).cmp(&d(b)).then_with(|| {
            a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
        })
    });
    Ok(json!({
        "path": dir,
        "parent": dir.parent(),
        "home": me.home,
        "entries": entries,
        "truncated": truncated,
    }))
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        Some("json") => "application/json",
        Some("md" | "markdown") => "text/markdown",
        Some("html" | "htm") => "text/html",
        _ => "text/plain",
    }
}

fn read_file(path: &str, me: &Me) -> Result<Value> {
    let p = resolve(path, me)?;
    let md = fs::metadata(&p).with_context(|| format!("{}", p.display()))?;
    if md.is_dir() {
        bail!("{} is a folder", p.display());
    }
    let m = mime(&p);
    let mut out = json!({ "path": p, "size": md.len(), "modified": secs(md.modified()), "mime": m });
    if m.starts_with("image/") && md.len() <= MAX_IMAGE {
        let bytes = fs::read(&p)?;
        // Shown with <img>, where even SVG cannot run scripts.
        out["data_url"] = json!(format!("data:{m};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
        return Ok(out);
    }
    let mut buf = Vec::new();
    fs::File::open(&p)?.take(MAX_TEXT).read_to_end(&mut buf)?;
    out["truncated"] = json!(md.len() > MAX_TEXT);
    match String::from_utf8(buf) {
        Ok(text) if !text.contains('\0') => out["text"] = json!(text),
        _ => out["binary"] = json!(true),
    }
    Ok(out)
}

fn info(me: &Me) -> Value {
    let read = |p: &str| fs::read_to_string(p).unwrap_or_default();
    let meminfo = read("/proc/meminfo");
    let mem = |key: &str| {
        meminfo
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let load: Vec<f64> = read("/proc/loadavg").split_whitespace().take(3).filter_map(|v| v.parse().ok()).collect();
    let uptime = read("/proc/uptime").split_whitespace().next().and_then(|v| v.parse::<f64>().ok());
    let mut disk = json!(null);
    if let Ok(c) = std::ffi::CString::new(me.home.as_os_str().as_encoded_bytes()) {
        // SAFETY: statvfs writes into the zeroed struct we pass; the path is NUL-terminated.
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0 {
            // The field types differ between 32- and 64-bit targets.
            #[allow(clippy::unnecessary_cast)]
            let (frsize, blocks, avail) = (s.f_frsize as u64, s.f_blocks as u64, s.f_bavail as u64);
            disk = json!({ "path": me.home, "total": blocks * frsize, "free": avail * frsize });
        }
    }
    json!({
        "user": me.name,
        "uid": me.uid,
        "home": me.home,
        "shell": me.shell,
        "hostname": read("/proc/sys/kernel/hostname").trim(),
        "kernel": read("/proc/sys/kernel/osrelease").trim(),
        "uptime_s": uptime,
        "load": load,
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "memory": { "total_kb": mem("MemTotal:"), "available_kb": mem("MemAvailable:") },
        "disk": disk,
        "wayland": term::wayland_display().is_some(),
    })
}

fn handle(stream: UnixStream, me: &Me) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.by_ref().take(MAX_REQUEST as u64).read_line(&mut line)?;
    let req: Value = serde_json::from_str(line.trim()).context("request is not JSON")?;
    let op = req["op"].as_str().unwrap_or_default();
    let s = |k: &str| req[k].as_str().unwrap_or_default().to_string();
    let result = match op {
        "info" => Ok(info(me)),
        "ls" => list_dir(&s("path"), me),
        "read" => read_file(&s("path"), me),
        "ps" => Ok(procs::list()),
        "signal" => procs::signal(req["pid"].as_i64().unwrap_or(0), &s("signal")).map(|m| json!({ "message": m })),
        "apps" => Ok(json!({ "apps": apps::list(me) })),
        "launch" => apps::launch(&s("id"), me).map(|pid| json!({ "pid": pid })),
        "term" => {
            let cols = req["cols"].as_u64().unwrap_or(80).clamp(10, 500) as u16;
            let rows = req["rows"].as_u64().unwrap_or(24).clamp(4, 300) as u16;
            return term::run(stream, me, cols, rows);
        }
        other => Err(anyhow!("unknown op {other:?}")),
    };
    let reply = match result {
        Ok(mut v) => {
            v["ok"] = json!(true);
            v
        }
        Err(e) => json!({ "ok": false, "error": format!("{e:#}") }),
    };
    let mut w = stream;
    writeln!(w, "{reply}")?;
    Ok(())
}

fn main() -> Result<()> {
    let me = Me::load()?;
    let path = socket_path()?;
    let dir = path.parent().context("socket path has no parent")?;
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    eprintln!("flux-session for {} listening on {}", me.name, path.display());
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        // Only root (fluxd) and this user may talk to us; the socket mode already says so.
        let me = me.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &me) {
                eprintln!("flux-session: {e:#}");
            }
        });
    }
    Ok(())
}
