// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! The registry this front keeps under a project's `.claude/shook/`:
//!
//! - `manifests/<name>.toml`, what tools put there to be registered;
//! - `entries/<Event>/<entry>`, each an entry in the base's own format, its
//!   first line the directory it runs in and each line after it one argument;
//! - `conditions/<Event>/<entry>`, what decides whether the entry takes a call:
//!   `matcher <text>`, `path <dir>` once per directory, `timeout <seconds>`, and
//!   `session-dir` where it runs in the session's directory.
//!
//! An entry is named `<manifest>--<nnn>-<row>`, so a chain runs the manifests
//! in the byte order of their names and each one's rows in the order written.

use std::path::{Path, PathBuf};

use crate::manifest::{self, Manifest};
use crate::matcher::Matcher;

/// Seconds an entry may run where it says nothing.
pub const DEFAULT_TIMEOUT: u64 = 60;

pub fn state(root: &Path) -> PathBuf {
    root.join(".claude").join("shook")
}

/// One registered entry and what decides whether it takes a call.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name:        String,
    /// The entry file, in the base's format.
    pub file:        PathBuf,
    pub matcher:     Matcher,
    pub paths:       Vec<String>,
    pub timeout:     u64,
    pub session_dir: bool,
}

/// Every manifest under `state`, in the order of their names.
pub fn manifests(state: &Path) -> Result<Vec<Manifest>, String> {
    let dir = state.join("manifests");
    let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    files.sort();
    let mut out: Vec<Manifest> = Vec::new();
    for f in &files {
        let m = manifest::read(f)?;
        if out.iter().any(|o| o.name == m.name) {
            return Err(format!("{}: a second manifest is named {}", f.display(), m.name));
        }
        out.push(m);
    }
    Ok(out)
}

/// Write `manifests` into the registry at `state` and under `root`, replacing
/// every entry and condition there. What is returned is each event with the
/// matchers and timeouts of its entries, for the registration.
pub fn write(root: &Path, state: &Path, manifests: &[Manifest]) -> Result<Vec<(String, Vec<Entry>)>, String> {
    let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    for sub in ["entries", "conditions"] {
        let d = state.join(sub);
        match std::fs::remove_dir_all(&d) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(io(&d, e)),
        }
    }
    for m in manifests {
        for (event, rows) in &m.events {
            let ed = state.join("entries").join(event);
            let cd = state.join("conditions").join(event);
            std::fs::create_dir_all(&ed).map_err(|e| io(&ed, e))?;
            std::fs::create_dir_all(&cd).map_err(|e| io(&cd, e))?;
            for (i, row) in rows.iter().enumerate() {
                let name = format!("{}--{:03}-{}", m.name, i, row.name);
                let mut entry = format!("{}\n", row.dir.as_deref().unwrap_or("."));
                for a in &row.run {
                    entry.push_str(a);
                    entry.push('\n');
                }
                let mut cond = String::new();
                if let Some(mt) = &row.matcher {
                    cond.push_str(&format!("matcher {mt}\n"));
                }
                for p in &row.paths {
                    cond.push_str(&format!("path {p}\n"));
                }
                cond.push_str(&format!("timeout {}\n", row.timeout.unwrap_or(DEFAULT_TIMEOUT)));
                if row.dir.is_none() {
                    cond.push_str("session-dir\n");
                }
                let ef = ed.join(&name);
                std::fs::write(&ef, entry).map_err(|e| io(&ef, e))?;
                let cf = cd.join(&name);
                std::fs::write(&cf, cond).map_err(|e| io(&cf, e))?;
            }
        }
    }
    let mut events: Vec<String> = manifests
        .iter()
        .flat_map(|m| m.events.iter().filter(|(_, r)| !r.is_empty()).map(|(e, _)| e.clone()))
        .collect();
    events.sort();
    events.dedup();
    events
        .into_iter()
        .map(|e| read(root, state, &e).map(|entries| (e, entries)))
        .collect()
}

/// The entries registered at `event`, in the order the chain runs them.
pub fn read(root: &Path, state: &Path, event: &str) -> Result<Vec<Entry>, String> {
    let ed = state.join("entries").join(event);
    let mut names: Vec<String> = match std::fs::read_dir(&ed) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", ed.display())),
    };
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let cf = state.join("conditions").join(event).join(&name);
        let text = match std::fs::read_to_string(&cf) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{}: {e}", cf.display())),
        };
        let mut entry = Entry {
            file: ed.join(&name),
            name,
            matcher: Matcher::All,
            paths: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            session_dir: false,
        };
        for line in text.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "matcher" => entry.matcher = Matcher::parse(value).map_err(|e| format!("{}: {e}", cf.display()))?,
                "path" => {
                    let p = Path::new(value);
                    let p = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
                    entry.paths.push(p.to_string_lossy().into_owned());
                },
                "timeout" => {
                    entry.timeout = value
                        .parse()
                        .map_err(|_| format!("{}: `{value}` is not a number of seconds", cf.display()))?;
                },
                "session-dir" => entry.session_dir = true,
                "" => {},
                other => return Err(format!("{}: `{other}` is not a condition this front reads", cf.display())),
            }
        }
        out.push(entry);
    }
    Ok(out)
}
