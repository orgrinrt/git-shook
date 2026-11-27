// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! claude-shook: the Claude Code front of shook. One hook per event calls it,
//! and it runs that event's entries as a chain on the shook base and answers
//! the host once, as the host would have folded the answers of hooks
//! registered side by side.

mod base;
mod event;
mod fold;
mod landing;
mod manifest;
mod matcher;
mod registry;
mod run;
mod scratch;
mod settings;
mod step;

use std::io::Write;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "claude-shook: the Claude Code front of shook.

USAGE
    claude-shook install [--root <dir>] [--settings <file>]
    claude-shook run <Event> [--root <dir>]
    claude-shook version

install reads every manifest under <root>/.claude/shook/manifests/, writes
their entries beside them and one registration per event into the settings
file, <root>/.claude/settings.local.json unless --settings names another.
run is what that registration calls: it reads the host's payload on stdin,
runs the entries that take the call, one after another, and prints the one
answer. The root is the current directory for install and the host's
CLAUDE_PROJECT_DIR, else the current directory, for run.
";

fn usage(why: &str) -> i32 {
    eprintln!("claude-shook: {why}\n\n{USAGE}");
    64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => run_cmd(&args[1..]),
        Some("step") => step_cmd(&args[1..]),
        Some("install") => install_cmd(&args[1..]),
        Some("version" | "--version") => {
            println!("claude-shook {VERSION}");
            0
        },
        Some("help" | "--help" | "-h") => {
            print!("{USAGE}");
            0
        },
        Some(other) => usage(&format!("`{other}` is not a verb of this front")),
        None => usage("a verb is needed"),
    };
    std::process::exit(code);
}

/// `--root <dir>` taken out of `args`, which keeps the rest.
fn take_root(args: &[String]) -> Result<(Option<PathBuf>, Vec<String>), String> {
    let mut root = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--root" {
            root = Some(PathBuf::from(it.next().ok_or("--root names no directory")?));
        } else {
            rest.push(a.clone());
        }
    }
    Ok((root, rest))
}

fn run_cmd(args: &[String]) -> i32 {
    let (root, rest) = match take_root(args) {
        Ok(r) => r,
        Err(e) => return usage(&e),
    };
    let [event] = rest.as_slice() else { return usage("run takes one event") };
    if !event::plain_event(event) {
        return usage(&format!("`{event}` is not a hook event"));
    }
    let root = root
        .or_else(|| std::env::var_os("CLAUDE_PROJECT_DIR").filter(|d| !d.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let a = run::run(&root, event);
    let _ = std::io::stdout().write_all(a.stdout.as_bytes());
    let _ = std::io::stderr().write_all(a.stderr.as_bytes());
    a.exit
}

fn step_cmd(args: &[String]) -> i32 {
    let mut record = None;
    let mut timeout = registry::DEFAULT_TIMEOUT;
    let mut session_dir = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--record" => record = it.next().map(PathBuf::from),
            "--timeout" => match it.next().and_then(|t| t.parse().ok()) {
                Some(t) => timeout = t,
                None => return usage("--timeout takes a number of seconds"),
            },
            "--session-dir" => session_dir = true,
            "--" => break,
            other => return usage(&format!("step does not take `{other}`")),
        }
    }
    let argv: Vec<String> = it.cloned().collect();
    let Some(record) = record else { return usage("step needs --record") };
    if argv.is_empty() {
        return usage("step names no program after --");
    }
    step::run(&step::Step { record, timeout, session_dir, argv })
}

fn install_cmd(args: &[String]) -> i32 {
    let (root, rest) = match take_root(args) {
        Ok(r) => r,
        Err(e) => return usage(&e),
    };
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let mut settings_file = root.join(".claude").join("settings.local.json");
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--settings" => match it.next() {
                Some(f) => settings_file = PathBuf::from(f),
                None => return usage("--settings names no file"),
            },
            other => return usage(&format!("install does not take `{other}`")),
        }
    }
    let state = registry::state(&root);
    // every file is read, and refused if it must be, before anything is written
    let installed = registry::manifests(&state).and_then(|ms| {
        let current = settings::read(&settings_file)?;
        let events = registry::write(&root, &state, &ms)?;
        settings::write(&settings_file, current, &events).map(|()| events)
    });
    match installed {
        Ok(events) => {
            for (event, entries) in &events {
                println!("{event}: {} entries", entries.len());
            }
            0
        },
        Err(e) => {
            eprintln!("claude-shook: {e}");
            1
        },
    }
}
