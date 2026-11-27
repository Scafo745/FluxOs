//! Processes from /proc, and signals to the user's own processes.

use std::fs;

use anyhow::{Result, bail};
use serde_json::{Value, json};

fn total_ticks() -> u64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines().next().map(|l| l.split_whitespace().skip(1).filter_map(|v| v.parse::<u64>().ok()).sum())
        })
        .unwrap_or(0)
}

/// One process, or None if it vanished while we read it.
fn process(pid: u32, names: &std::collections::HashMap<u32, String>, page: u64) -> Option<Value> {
    let dir = format!("/proc/{pid}");
    let stat = fs::read_to_string(format!("{dir}/stat")).ok()?;
    // comm may contain spaces and parentheses; the fields after the last ')' are fixed.
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = &stat[open + 1..close];
    let f: Vec<&str> = stat[close + 2..].split_whitespace().collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let status = fs::read_to_string(format!("{dir}/status")).unwrap_or_default();
    let uid = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let rss_pages = fs::read_to_string(format!("{dir}/statm"))
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()))
        .unwrap_or(0);
    let cmdline = fs::read(format!("{dir}/cmdline")).unwrap_or_default();
    let mut cmd: String = String::from_utf8_lossy(&cmdline).replace('\0', " ").trim().to_string();
    if cmd.len() > 300 {
        cmd.truncate(cmd.char_indices().nth(300).map(|(i, _)| i).unwrap_or(cmd.len()));
        cmd.push('…');
    }
    Some(json!({
        "pid": pid,
        "ppid": num(1),
        "name": comm,
        "state": f.first().copied().unwrap_or("?"),
        "uid": uid,
        "user": names.get(&uid).cloned().unwrap_or_else(|| uid.to_string()),
        "cpu_ticks": num(11) + num(12),
        "threads": num(17),
        "start_ticks": num(19),
        "rss": rss_pages * page,
        "cmd": if cmd.is_empty() { format!("[{comm}]") } else { cmd },
        "kernel": cmdline.is_empty(),
    }))
}

pub fn list() -> Value {
    let names = crate::user_names();
    // SAFETY: sysconf has no preconditions.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
    let total = total_ticks();
    let mut procs: Vec<Value> = fs::read_dir("/proc")
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()))
                .filter_map(|pid| process(pid, &names, page))
                .collect()
        })
        .unwrap_or_default();
    procs.sort_by_key(|p| p["pid"].as_u64().unwrap_or(0));
    // SAFETY: getuid has no preconditions.
    let me = unsafe { libc::getuid() };
    json!({
        "uid": me,
        "total_ticks": total,
        "ticks_per_second": hz,
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "processes": procs,
    })
}

/// Sends a signal. The kernel refuses other users' processes (we run unprivileged).
pub fn signal(pid: i64, name: &str) -> Result<String> {
    let sig = match name {
        "" | "TERM" => libc::SIGTERM,
        "KILL" => libc::SIGKILL,
        "INT" => libc::SIGINT,
        "HUP" => libc::SIGHUP,
        "STOP" => libc::SIGSTOP,
        "CONT" => libc::SIGCONT,
        other => bail!("unsupported signal {other}"),
    };
    if pid <= 1 || pid > i32::MAX as i64 {
        bail!("invalid pid {pid}");
    }
    // SAFETY: kill only takes plain integers.
    if unsafe { libc::kill(pid as i32, sig) } != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EPERM) {
            bail!("process {pid} belongs to another user; only your own processes can be stopped here");
        }
        bail!("signal to {pid} failed: {err}");
    }
    Ok(format!("sent SIG{} to {pid}", if name.is_empty() { "TERM" } else { name }))
}
