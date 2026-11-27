// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! Where a call lands, read from the host's payload, so an entry with `paths`
//! sees only the calls landing under one of them.
//!
//! A tool writing a file lands on that file. A shell call lands in the
//! directory the session is in, and on every path its command names, read
//! relative to that directory. A command is not parsed as a shell would: it is
//! split on everything that cannot be part of a path, which takes quotes,
//! separators, redirections and the `=` of `--flag=path` apart, and each piece
//! is read as a path. A directory's absolute path anywhere in the command
//! counts, which catches one with a space in it. A file or a piece climbing with
//! `..` lands everywhere, since a guard skipped costs what the guard is for. Not
//! seen: a relative path with a space in it, a path spelled through a variable,
//! and what a script the command runs does next.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Landing {
    file: Option<String>,
    cwd:  String,
    cmd:  Option<String>,
    home: String,
}

fn text(v: &Value, path: &[&str]) -> Option<String> {
    let mut at = v;
    for p in path {
        at = at.get(p)?;
    }
    at.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

impl Landing {
    /// Read from `payload`, with `here` standing in where the host names no
    /// directory.
    pub fn of(payload: &Value, here: &str, home: &str) -> Landing {
        let file = text(payload, &["tool_input", "file_path"])
            .or_else(|| text(payload, &["tool_input", "notebook_path"]))
            .or_else(|| text(payload, &["tool_input", "path"]));
        let cwd = text(payload, &["cwd"])
            .or_else(|| text(payload, &["tool_input", "cwd"]))
            .unwrap_or_else(|| here.to_string());
        Landing {
            file,
            cwd,
            cmd: text(payload, &["tool_input", "command"]),
            home: home.to_string(),
        }
    }

    /// Whether the call lands anywhere under `root`, an absolute directory.
    pub fn lands_in(&self, root: &str) -> bool {
        let root = root.trim_end_matches('/');
        if let Some(file) = &self.file {
            if climbs(file) {
                return true;
            }
            let at = if file.starts_with('/') { file.clone() } else { format!("{}/{file}", self.cwd) };
            return under(root, &tidy(&at));
        }
        if under(root, &tidy(&self.cwd)) {
            return true;
        }
        let Some(cmd) = &self.cmd else { return false };
        if names_root(cmd, root) {
            return true;
        }
        cmd.split(|c: char| !path_char(c))
            .filter(|w| !w.is_empty())
            .any(|w| {
                let w = if let Some(rest) = w.strip_prefix("~/") {
                    format!("{}/{rest}", self.home)
                } else if w.starts_with('/') {
                    w.to_string()
                } else {
                    format!("{}/{w}", self.cwd)
                };
                let w = tidy(&w);
                climbs(&w) || under(root, &w)
            })
    }
}

fn path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_./~@%+,:-".contains(c)
}

/// `root` written out in `cmd`, ended by its end or anything a name cannot
/// carry on with, so a neighbour whose name only starts the same is not it.
fn names_root(cmd: &str, root: &str) -> bool {
    cmd.match_indices(root).any(|(at, _)| {
        cmd[at + root.len()..]
            .chars()
            .next()
            .is_none_or(|c| !path_char(c) || c == '/')
    })
}

fn climbs(p: &str) -> bool {
    p.split('/').any(|c| c == "..")
}

/// A `.` component names the directory it sits in, so `./engine` is `engine`.
fn tidy(p: &str) -> String {
    let mut out = p.replace("/./", "/");
    while out.contains("/./") {
        out = out.replace("/./", "/");
    }
    if let Some(s) = out.strip_suffix("/.") {
        out = s.to_string();
    }
    out
}

fn under(root: &str, p: &str) -> bool {
    p == root || p.strip_prefix(root).is_some_and(|r| r.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shell(cwd: &str, cmd: &str) -> Landing {
        Landing::of(&json!({"cwd": cwd, "tool_input": {"command": cmd}}), "/elsewhere", "/home/u")
    }

    #[test]
    fn a_file_tool_lands_on_its_file_and_a_relative_one_from_the_session_directory() {
        let l = Landing::of(&json!({"cwd": "/w", "tool_input": {"file_path": "/w/engine/a.rs"}}), "/x", "/h");
        assert!(l.lands_in("/w/engine"));
        assert!(!l.lands_in("/w/engine-two"));
        let r = Landing::of(&json!({"cwd": "/w", "tool_input": {"file_path": "engine/a.rs"}}), "/x", "/h");
        assert!(r.lands_in("/w/engine"));
        assert!(!r.lands_in("/w/docs"));
    }

    #[test]
    fn a_shell_call_lands_in_its_directory_and_on_every_path_it_names() {
        assert!(shell("/w/engine", "ls").lands_in("/w/engine"));
        assert!(shell("/w", "cargo test --manifest-path=engine/Cargo.toml").lands_in("/w/engine"));
        assert!(shell("/w", "cat ./engine/x").lands_in("/w/engine"));
        assert!(shell("/w", "cat '~/'x; ls ~/engine").lands_in("/home/u/engine"));
        assert!(!shell("/w", "ls docs && echo ok").lands_in("/w/engine"));
    }

    #[test]
    fn a_neighbour_with_a_longer_name_is_not_the_directory() {
        assert!(!shell("/w", "ls /w/engine-two/x").lands_in("/w/engine"));
        assert!(shell("/w", "ls \"/w/engine/a b\"").lands_in("/w/engine"));
        assert!(shell("/x", "ls /w/engine").lands_in("/w/engine"));
    }

    #[test]
    fn climbing_with_dot_dot_lands_everywhere() {
        assert!(shell("/w/docs", "cat ../x").lands_in("/w/engine"));
        let l = Landing::of(&json!({"tool_input": {"file_path": "../a"}}), "/x", "/h");
        assert!(l.lands_in("/anything"));
    }

    #[test]
    fn where_the_host_names_no_directory_the_process_one_stands_in() {
        let l = Landing::of(&json!({"tool_input": {"command": "true"}}), "/w/engine", "/h");
        assert!(l.lands_in("/w/engine"));
        let none = Landing::of(&json!({}), "/w", "/h");
        assert!(!none.lands_in("/w/engine"));
    }
}
