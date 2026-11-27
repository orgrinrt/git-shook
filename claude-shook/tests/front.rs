// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! The front end to end, on the POSIX base this repository carries: what an
//! install writes, and what a run of one event does with its entries.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const FRONT: &str = env!("CARGO_BIN_EXE_claude-shook");
const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../shook");

struct Project(PathBuf);

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project(manifest: &str) -> Project {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("claude-shook-front-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".claude/shook/manifests")).unwrap();
    std::fs::create_dir_all(root.join("engine")).unwrap();
    std::fs::write(root.join(".claude/shook/manifests/t.toml"), manifest).unwrap();
    let out = Command::new(FRONT).arg("install").arg("--root").arg(&root).output().unwrap();
    assert!(out.status.success(), "install: {}", String::from_utf8_lossy(&out.stderr));
    Project(root)
}

fn run_with(p: &Path, event: &str, payload: &str, base: &str) -> Output {
    let mut child = Command::new(FRONT)
        .args(["run", event])
        .env("CLAUDE_PROJECT_DIR", p)
        .env("SHOOK_BASE", base)
        .current_dir(p)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn run(p: &Project, event: &str, payload: &str) -> Output {
    run_with(&p.0, event, payload, BASE)
}

fn bash(p: &Project, cmd: &str) -> String {
    format!(r#"{{"tool_name": "Bash", "cwd": "{}", "tool_input": {{"command": "{cmd}"}}}}"#, p.0.display())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn an_install_writes_one_registration_per_event_and_a_quiet_chain_prints_nothing() {
    let p = project(
        "[[PreToolUse]]\nname = \"a\"\nrun = [\"true\"]\nmatcher = \"Bash\"\n\n[[PreToolUse]]\nname = \"b\"\nrun = [\"true\"]\nmatcher = \"Edit\"\n\n[[Stop]]\nname = \"c\"\nrun = [\"true\"]\n",
    );
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(p.0.join(".claude/settings.local.json")).unwrap()).unwrap();
    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0]["matcher"], "Bash|Edit");
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    let out = run(&p, "PreToolUse", &bash(&p, "ls"));
    assert_eq!((out.status.code(), text(&out.stdout), text(&out.stderr)), (Some(0), String::new(), String::new()));
}

#[test]
fn a_block_stops_the_chain_and_is_the_answer() {
    let p = project(
        "[[PreToolUse]]\nname = \"no\"\nrun = [\"sh\", \"-c\", \"echo not this >&2; exit 2\"]\n\n[[PreToolUse]]\nname = \"after\"\nrun = [\"sh\", \"-c\", \"touch ran\"]\n",
    );
    let out = run(&p, "PreToolUse", &bash(&p, "ls"));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stderr), "not this\n");
    assert!(!p.0.join("ran").exists(), "an entry after the block ran");
}

#[test]
fn a_failure_is_named_and_the_entries_after_it_still_run() {
    let p = project(
        "[[PreToolUse]]\nname = \"broken\"\nrun = [\"sh\", \"-c\", \"echo oops >&2; exit 1\"]\n\n[[PreToolUse]]\nname = \"gone\"\nrun = [\"true\"]\ndir = \"missing\"\n\n[[PreToolUse]]\nname = \"after\"\nrun = [\"sh\", \"-c\", \"touch ran\"]\n",
    );
    let out = run(&p, "PreToolUse", &bash(&p, "ls"));
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let msg = v["systemMessage"].as_str().unwrap();
    assert!(msg.contains("t--000-broken exited 1: oops"), "{msg}");
    assert!(msg.contains("t--001-gone could not be started: its directory missing cannot be entered"), "{msg}");
    assert!(p.0.join("ran").exists(), "the entry after the failures did not run");
}

#[test]
fn the_matcher_and_the_paths_decide_which_entries_take_a_call() {
    let p = project(
        "[[PreToolUse]]\nname = \"edits\"\nrun = [\"sh\", \"-c\", \"touch edits\"]\nmatcher = \"Edit\"\n\n[[PreToolUse]]\nname = \"engine\"\nrun = [\"sh\", \"-c\", \"touch engine-ran\"]\nmatcher = \"Bash\"\npaths = [\"engine\"]\ndir = \".\"\n",
    );
    run(&p, "PreToolUse", &bash(&p, "ls docs"));
    assert!(!p.0.join("edits").exists() && !p.0.join("engine-ran").exists());
    run(&p, "PreToolUse", &bash(&p, "cargo test --manifest-path engine/Cargo.toml"));
    assert!(!p.0.join("edits").exists());
    assert!(p.0.join("engine-ran").exists());
}

#[test]
fn every_entry_reads_the_payload_and_runs_in_the_session_directory() {
    let p = project(
        "[[UserPromptSubmit]]\nname = \"one\"\nrun = [\"sh\", \"-c\", \"cat > got-one; pwd > where\"]\n\n[[UserPromptSubmit]]\nname = \"two\"\nrun = [\"sh\", \"-c\", \"cat > got-two\"]\ndir = \".\"\n",
    );
    let payload = format!(r#"{{"prompt": "hi", "cwd": "{}/engine"}}"#, p.0.display());
    run(&p, "UserPromptSubmit", &payload);
    assert_eq!(std::fs::read_to_string(p.0.join("engine/got-one")).unwrap(), payload);
    assert_eq!(std::fs::read_to_string(p.0.join("got-two")).unwrap(), payload);
    assert_eq!(std::fs::read_to_string(p.0.join("engine/where")).unwrap().trim(), format!("{}/engine", p.0.display()));
}

#[test]
fn an_entry_past_its_timeout_is_stopped_and_named() {
    let p = project("[[Stop]]\nname = \"slow\"\nrun = [\"sleep\", \"30\"]\ntimeout = 1\n");
    let out = run(&p, "Stop", "{}");
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("t--000-slow did not finish in time and was stopped"));
}

#[test]
fn plain_context_from_several_entries_is_joined() {
    let p = project(
        "[[SessionStart]]\nname = \"a\"\nrun = [\"echo\", \"first\"]\n\n[[SessionStart]]\nname = \"b\"\nrun = [\"echo\", \"second\"]\n",
    );
    let out = run(&p, "SessionStart", r#"{"source": "startup"}"#);
    assert_eq!(text(&out.stdout), "first\n\nsecond\n");
}

#[test]
fn without_a_base_the_front_says_so_in_one_line_and_the_call_goes_ahead() {
    let p = project("[[Stop]]\nname = \"a\"\nrun = [\"true\"]\n");
    let out = run_with(&p.0, "Stop", "{}", "/nonexistent/shook");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stderr).lines().count(), 1);
    assert!(text(&out.stderr).contains("SHOOK_BASE names /nonexistent/shook"));
}

#[test]
fn without_the_front_the_registration_says_so_and_the_call_goes_ahead() {
    let p = project("[[Stop]]\nname = \"a\"\nrun = [\"true\"]\n");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(p.0.join(".claude/settings.local.json")).unwrap()).unwrap();
    let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap().to_string();
    let out = Command::new("/bin/sh").args(["-c", &cmd]).env("PATH", "/usr/bin:/bin").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("claude-shook is not on PATH, so the Stop hooks did not run"));
}

#[test]
fn a_manifest_the_front_refuses_leaves_the_registry_and_the_settings_as_they_were() {
    let p = project("[[Stop]]\nname = \"a\"\nrun = [\"true\"]\n");
    let before = std::fs::read_to_string(p.0.join(".claude/settings.local.json")).unwrap();
    std::fs::write(p.0.join(".claude/shook/manifests/u.toml"), "[[Stop]]\nname = \"b\"\nrun = [\"x\"]\nasync = true\n").unwrap();
    let out = Command::new(FRONT).arg("install").arg("--root").arg(&p.0).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("async"));
    assert_eq!(std::fs::read_to_string(p.0.join(".claude/settings.local.json")).unwrap(), before);
    assert!(p.0.join(".claude/shook/entries/Stop/t--000-a").is_file());
}

#[test]
fn root_in_an_argument_is_the_project_wherever_the_session_is() {
    let p = project("[[Stop]]\nname = \"a\"\nrun = [\"sh\", \"{root}/tool.sh\", \"{root}\"]\n");
    std::fs::write(p.0.join("tool.sh"), "printf '%s' \"$1\" > \"$1/got\"\n").unwrap();
    let payload = format!(r#"{{"cwd": "{}/engine"}}"#, p.0.display());
    let out = run(&p, "Stop", &payload);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_to_string(p.0.join("got")).unwrap(), p.0.display().to_string());
}
