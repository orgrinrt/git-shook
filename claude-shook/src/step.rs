// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! One entry, run as a step of the chain: the command the entry names, under
//! its timeout, with what it said and how it ended written to a record the
//! front folds once the chain is done. The step prints nothing itself.
//!
//! It exits 2 where the command blocked, so the base stops the chain there, and
//! 0 for every other ending, since the host lets a call go ahead past a hook
//! that failed any other way and so does the chain.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// What one step left for the fold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub exit:      i32,
    pub stdout:    String,
    pub stderr:    String,
    pub timed_out: bool,
}

/// The exit status standing for a command that could not be started.
pub const NOT_STARTED: i32 = 127;

pub struct Step {
    pub record:      PathBuf,
    pub timeout:     u64,
    pub session_dir: bool,
    pub argv:        Vec<String>,
}

fn session_dir(payload: &[u8]) -> Option<PathBuf> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let d = PathBuf::from(v.get("cwd")?.as_str()?);
    d.is_dir().then_some(d)
}

/// Run the step; its own exit status.
pub fn run(step: &Step) -> i32 {
    let mut payload = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut payload);
    let record = execute(step, &payload);
    let blocked = record.exit == 2 && !record.timed_out;
    if let Err(e) = write_record(&step.record, &record) {
        eprintln!("claude-shook: could not keep what {} said: {e}", step.argv[0]);
    }
    if blocked { 2 } else { 0 }
}

fn write_record(path: &Path, record: &Record) -> std::io::Result<()> {
    let text = serde_json::to_string(record).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

fn execute(step: &Step, payload: &[u8]) -> Record {
    let mut cmd = Command::new(&step.argv[0]);
    cmd.args(&step.argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if step.session_dir {
        if let Some(d) = session_dir(payload) {
            cmd.current_dir(d);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Record {
                exit:      NOT_STARTED,
                stdout:    String::new(),
                stderr:    format!("{} could not be started: {e}", step.argv[0]),
                timed_out: false,
            };
        },
    };
    let mut stdin = child.stdin.take();
    let input = payload.to_vec();
    let feeder = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(&input);
        }
    });
    let out = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = s.read_to_end(&mut b);
            b
        })
    });
    let err = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = s.read_to_end(&mut b);
            b
        })
    });
    let deadline = Instant::now() + Duration::from_secs(step.timeout);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                stop(&mut child);
                break child.wait().ok();
            },
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => break None,
        }
    };
    let _ = feeder.join();
    let text = |h: Option<std::thread::JoinHandle<Vec<u8>>>| {
        h.and_then(|h| h.join().ok())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    };
    Record {
        exit: status.map(code_of).unwrap_or(1),
        stdout: text(out),
        stderr: text(err),
        timed_out,
    }
}

fn code_of(s: std::process::ExitStatus) -> i32 {
    if let Some(c) = s.code() {
        return c;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = s.signal() {
            return 128 + sig;
        }
    }
    1
}

/// Stop the command and everything it started, which share its process group.
fn stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        if let Ok(pid) = i32::try_from(child.id()) {
            // SAFETY: a signal to the process group the child leads, which
            // `process_group(0)` made, and to nothing else.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            return;
        }
    }
    let _ = child.kill();
}
