// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! `claude-shook run <Event>`: the one hook the host calls at an event.
//!
//! The entries whose matcher and paths take the call are written as a chain of
//! steps, each the entry run through `claude-shook step`, and the base runs
//! that chain with the payload replayed to every step. The steps' records are
//! then folded into the one answer the host reads.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::fold::{self, Answer};
use crate::landing::Landing;
use crate::registry::{self, Entry};
use crate::step::Record;
use crate::{base, event, scratch};

/// The entries of `entries` that take a call with `payload`.
pub fn selected<'a>(event: &str, entries: &'a [Entry], payload: &Value, here: &str, home: &str) -> Vec<&'a Entry> {
    let value = event::matched_field(event)
        .and_then(|f| payload.get(f))
        .and_then(Value::as_str)
        .unwrap_or("");
    let landing = Landing::of(payload, here, home);
    entries
        .iter()
        .filter(|e| e.matcher.matches(value))
        .filter(|e| e.paths.is_empty() || e.paths.iter().any(|p| landing.lands_in(p)))
        .collect()
}

/// Run `event` for the project at `root`; the answer to print and exit with.
pub fn run(root: &Path, event: &str) -> Answer {
    let mut payload = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut payload);
    let state = registry::state(root);
    let entries = match registry::read(root, &state, event) {
        Ok(e) => e,
        Err(e) => return said(format!("claude-shook: the {event} entries cannot be read: {e}")),
    };
    let parsed: Value = serde_json::from_slice(&payload).unwrap_or(Value::Null);
    let here = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let chosen = selected(event, &entries, &parsed, &here, &home);
    if chosen.is_empty() {
        return Answer { stdout: String::new(), stderr: String::new(), exit: 0 };
    }
    let base = match base::find() {
        Ok(b) => b,
        Err(e) => return said(format!("claude-shook: the {event} hooks did not run: {e}")),
    };
    let tmp = scratch::dir("run");
    let out = chain(root, &base, &tmp, event, &chosen, &payload);
    let _ = std::fs::remove_dir_all(&tmp);
    match out {
        Ok(records) => fold::fold(event, &records),
        Err(e) => said(format!("claude-shook: the {event} hooks did not run: {e}")),
    }
}

/// A line on stderr and a non-blocking failure, which the host shows the
/// person and goes ahead past.
fn said(line: String) -> Answer {
    Answer { stdout: String::new(), stderr: format!("{line}\n"), exit: 1 }
}

/// Run `chosen` as steps on `base`, and the record each one left, in order.
///
/// The base stops at the first step that does not exit 0. A step exits 2 only
/// where its entry blocked, which ends the call; any other stop is the base's
/// own, an entry whose directory cannot be entered, and that entry is named
/// failed and the chain goes on with the ones after it, as the host goes on
/// past a hook that failed.
fn chain(
    root: &Path,
    base: &Path,
    tmp: &Path,
    event: &str,
    chosen: &[&Entry],
    payload: &[u8],
) -> Result<Vec<(String, Record)>, String> {
    let me = std::env::current_exe().map_err(|e| format!("this program's own path is not known: {e}"))?;
    let rec = tmp.join("rec");
    std::fs::create_dir_all(&rec).map_err(|e| e.to_string())?;
    let mut pending: Vec<&Entry> = chosen.to_vec();
    let mut records: Vec<(String, Record)> = Vec::new();
    let mut round = 0usize;
    while !pending.is_empty() {
        round += 1;
        let dir = tmp.join(format!("chain-{round}"));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for e in &pending {
            std::fs::write(dir.join(&e.name), step_entry(root, &me, &rec, e)?).map_err(|e| e.to_string())?;
        }
        let (status, stderr) = run_base(root, base, &dir, payload)?;
        let mut rest: Vec<&Entry> = Vec::new();
        let mut stopped = false;
        for e in &pending {
            if stopped {
                rest.push(e);
                continue;
            }
            match std::fs::read_to_string(rec.join(&e.name)) {
                Ok(text) => {
                    let r: Record = serde_json::from_str(&text).map_err(|x| format!("{}: {x}", e.name))?;
                    records.push((e.name.clone(), r));
                },
                Err(_) if status == 0 || status == 2 => {},
                Err(_) => {
                    stopped = true;
                    // the base says nothing of a directory it cannot enter, so
                    // the front, which knows the entry, says which one
                    let mut said = stderr.trim().to_string();
                    if said.is_empty() && status == crate::step::NOT_STARTED {
                        let dir = std::fs::read_to_string(&e.file).ok().and_then(|t| t.lines().next().map(str::to_string));
                        said = format!("its directory {} cannot be entered", dir.unwrap_or_default());
                    }
                    records.push((e.name.clone(), Record { exit: status, stdout: String::new(), stderr: said, timed_out: false }));
                },
            }
        }
        if status == 0 || status == 2 || !stopped {
            if status != 0 && status != 2 {
                return Err(format!("the base exited {status} for {event}: {}", stderr.trim()));
            }
            break;
        }
        pending = rest;
    }
    Ok(records)
}

/// The entry `e` as a step: its own directory, then this program's `step`
/// verb in front of its argument list, with `{root}` in any argument read as
/// the project root, so a manifest naming a program under the project stays
/// right wherever the project is moved.
fn step_entry(root: &Path, me: &Path, rec: &Path, e: &Entry) -> Result<String, String> {
    let text = std::fs::read_to_string(&e.file).map_err(|x| format!("{}: {x}", e.file.display()))?;
    let mut lines = text.lines();
    let dir = lines.next().unwrap_or(".");
    let mut out = format!("{dir}\n{}\nstep\n--record\n{}\n--timeout\n{}\n", me.display(), rec.join(&e.name).display(), e.timeout);
    if e.session_dir {
        out.push_str("--session-dir\n");
    }
    out.push_str("--\n");
    for a in lines {
        out.push_str(&a.replace("{root}", &root.to_string_lossy()));
        out.push('\n');
    }
    Ok(out)
}

fn run_base(root: &Path, base: &Path, chain: &PathBuf, payload: &[u8]) -> Result<(i32, String), String> {
    let mut child = Command::new(base)
        .arg("run")
        .arg("--root")
        .arg(root)
        .args(["--stdin", "replay"])
        .arg(chain)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} could not be started: {e}", base.display()))?;
    if let Some(mut s) = child.stdin.take() {
        let _ = s.write_all(payload);
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok((out.status.code().unwrap_or(1), String::from_utf8_lossy(&out.stderr).into_owned()))
}
