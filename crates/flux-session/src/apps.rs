//! Installed desktop applications (XDG .desktop files) and launching them in
//! the user's graphical session.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::Me;

fn data_dirs(me: &Me) -> Vec<PathBuf> {
    let mut dirs = vec![me.home.join(".local/share")];
    if let Some(v) = std::env::var_os("XDG_DATA_DIRS") {
        dirs.extend(std::env::split_paths(&v));
    }
    for d in [
        format!("/etc/profiles/per-user/{}/share", me.name),
        "/run/current-system/sw/share".into(),
        "/var/lib/flatpak/exports/share".into(),
    ] {
        dirs.push(PathBuf::from(d));
    }
    dirs.push(me.home.join(".local/share/flatpak/exports/share"));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

struct Entry {
    id: String,
    fields: BTreeMap<String, String>,
}

fn parse(id: String, text: &str) -> Option<Entry> {
    let mut fields = BTreeMap::new();
    let mut in_main = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            // Only the untranslated keys.
            if !k.contains('[') {
                fields.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    let shown = fields.get("Type").map(String::as_str) == Some("Application")
        && fields.get("NoDisplay").map(String::as_str) != Some("true")
        && fields.get("Hidden").map(String::as_str) != Some("true")
        && fields.contains_key("Exec");
    shown.then_some(Entry { id, fields })
}

fn entries(me: &Me) -> Vec<Entry> {
    let mut by_id: BTreeMap<String, Entry> = BTreeMap::new();
    for dir in data_dirs(me) {
        let Ok(rd) = fs::read_dir(dir.join("applications")) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".desktop") else { continue };
            if by_id.contains_key(id) {
                continue; // earlier data dirs win, as in the XDG spec
            }
            if let Ok(text) = fs::read_to_string(e.path())
                && let Some(entry) = parse(id.to_string(), &text)
            {
                by_id.insert(id.to_string(), entry);
            }
        }
    }
    by_id.into_values().collect()
}

pub fn list(me: &Me) -> Vec<Value> {
    let mut apps: Vec<Value> = entries(me)
        .into_iter()
        .map(|e| {
            let f = |k: &str| e.fields.get(k).cloned();
            json!({
                "id": e.id,
                "name": f("Name").unwrap_or_else(|| e.id.clone()),
                "generic": f("GenericName"),
                "comment": f("Comment"),
                "icon": f("Icon"),
                "categories": f("Categories").map(|c| c.split(';').filter(|s| !s.is_empty()).map(String::from).collect::<Vec<_>>()).unwrap_or_default(),
                "terminal": f("Terminal").as_deref() == Some("true"),
            })
        })
        .collect();
    apps.sort_by_key(|a| a["name"].as_str().unwrap_or("").to_lowercase());
    apps
}

/// Splits an Exec value into arguments and drops the field codes (%f, %U, ...).
fn exec_args(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if quoted => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ' ' | '\t' if !quoted => {
                if any || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            // Field codes (%f, %U, ...) are dropped; %% is a literal percent sign.
            '%' => {
                if chars.next() == Some('%') {
                    cur.push('%');
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        args.push(cur);
    }
    args.retain(|a| !a.is_empty());
    args
}

/// Starts an application in the user's Wayland session, detached from us.
pub fn launch(id: &str, me: &Me) -> Result<u32> {
    if id.is_empty() || id.contains('/') {
        bail!("invalid application id");
    }
    let entry = entries(me).into_iter().find(|e| e.id == id).with_context(|| format!("no application {id}"))?;
    if entry.fields.get("Terminal").map(String::as_str) == Some("true") {
        bail!("{id} runs in a terminal; start it from the Terminal app");
    }
    let args = exec_args(entry.fields.get("Exec").map(String::as_str).unwrap_or_default());
    let (program, rest) = args.split_first().context("empty Exec line")?;
    let mut cmd = Command::new(program);
    cmd.args(rest).current_dir(&me.home).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(display) = crate::term::wayland_display() {
        cmd.env("WAYLAND_DISPLAY", display).env("XDG_SESSION_TYPE", "wayland");
    }
    cmd.env("NIXOS_OZONE_WL", "1").env("MOZ_ENABLE_WAYLAND", "1");
    // SAFETY: setsid is async-signal-safe; the app gets its own session.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().with_context(|| format!("start {program}"))?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::exec_args;

    #[test]
    fn exec_lines() {
        assert_eq!(exec_args("firefox %u"), vec!["firefox"]);
        assert_eq!(exec_args("env A=1 \"/opt/My App/app\" --x %F"), vec!["env", "A=1", "/opt/My App/app", "--x"]);
        assert_eq!(exec_args("printf 100%%"), vec!["printf", "100%"]);
    }
}
