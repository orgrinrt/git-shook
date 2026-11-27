// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! The entries' answers folded into the one answer the host reads, the way the
//! host folds the answers of hooks registered side by side:
//!
//! - An entry that blocked, by exiting 2, is the answer: the front exits 2 with
//!   its stderr, and the context earlier entries gave follows it.
//! - Otherwise each entry's stdout is JSON or plain text. JSON objects fold by
//!   key: `continue` is false where any says false, a `deny` beats a `defer`
//!   beats an `ask` beats an `allow`, a `block` decision beats none, every
//!   `additionalContext`, `systemMessage`, `stopReason` and reason is kept, and
//!   any other key is the first entry's that gives it.
//! - Plain text alone is handed on as it was. Beside JSON, it joins the
//!   context at an event whose plain stdout is context, and the message the
//!   person sees at any other.
//! - An entry that failed another way, or ran out of time, is named in the
//!   message the person sees, and the call goes ahead.
//! - Where nothing was said, nothing is printed.

use serde_json::{Map, Value};

use crate::event;
use crate::step::{NOT_STARTED, Record};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub stdout: String,
    pub stderr: String,
    pub exit:   i32,
}

const DECISIONS: [&str; 4] = ["deny", "defer", "ask", "allow"];

fn rank(d: &str) -> usize {
    DECISIONS.iter().position(|x| *x == d).unwrap_or(DECISIONS.len())
}

fn join(into: &mut Map<String, Value>, key: &str, text: &str, sep: &str) {
    if text.is_empty() {
        return;
    }
    let joined = match into.get(key).and_then(Value::as_str) {
        Some(had) if !had.is_empty() => format!("{had}{sep}{text}"),
        _ => text.to_string(),
    };
    into.insert(key.to_string(), Value::String(joined));
}

fn failure(name: &str, r: &Record) -> String {
    let said = r.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let how = if r.timed_out {
        "did not finish in time and was stopped".to_string()
    } else if r.exit == NOT_STARTED {
        "could not be started".to_string()
    } else {
        format!("exited {}", r.exit)
    };
    if said.is_empty() { format!("{name} {how}") } else { format!("{name} {how}: {said}") }
}

/// Fold the records of `event`'s entries, in the order they ran.
pub fn fold(event: &str, records: &[(String, Record)]) -> Answer {
    let mut contexts: Vec<String> = Vec::new();
    let mut plain: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut objects: Vec<Map<String, Value>> = Vec::new();
    for (name, r) in records {
        if r.exit == 2 && !r.timed_out {
            let mut reason = r.stderr.trim_end().to_string();
            for c in contexts.iter().chain(plain.iter()) {
                reason.push_str("\n\n");
                reason.push_str(c);
            }
            return Answer { stdout: String::new(), stderr: format!("{reason}\n"), exit: 2 };
        }
        if r.exit != 0 || r.timed_out {
            failures.push(failure(name, r));
            continue;
        }
        let out = r.stdout.trim();
        if out.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(out) {
            Ok(Value::Object(o)) => {
                if let Some(c) = o
                    .get("hookSpecificOutput")
                    .and_then(|h| h.get("additionalContext"))
                    .and_then(Value::as_str)
                {
                    contexts.push(c.to_string());
                }
                objects.push(o);
            },
            _ => plain.push(out.to_string()),
        }
    }
    if objects.is_empty() && failures.is_empty() {
        let stdout = if plain.is_empty() { String::new() } else { format!("{}\n", plain.join("\n\n")) };
        return Answer { stdout, stderr: String::new(), exit: 0 };
    }
    let mut top = Map::new();
    let mut specific = Map::new();
    let mut decision: Option<(usize, String)> = None;
    let mut decision_reasons: Vec<String> = Vec::new();
    for o in &objects {
        for (k, v) in o {
            match k.as_str() {
                "continue" => {
                    let now = top.get("continue").and_then(Value::as_bool).unwrap_or(true);
                    top.insert(k.clone(), Value::Bool(now && v.as_bool().unwrap_or(true)));
                },
                "suppressOutput" => {
                    let now = top.get(k).and_then(Value::as_bool).unwrap_or(false);
                    top.insert(k.clone(), Value::Bool(now || v.as_bool().unwrap_or(false)));
                },
                "stopReason" | "systemMessage" | "reason" => join(&mut top, k, v.as_str().unwrap_or(""), "\n"),
                "decision" => {
                    if v.as_str() == Some("block") {
                        top.insert(k.clone(), v.clone());
                    } else {
                        top.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                },
                "hookSpecificOutput" => {
                    let Some(h) = v.as_object() else { continue };
                    for (hk, hv) in h {
                        match hk.as_str() {
                            "additionalContext" => join(&mut specific, hk, hv.as_str().unwrap_or(""), "\n\n"),
                            "permissionDecision" => {
                                let d = hv.as_str().unwrap_or("").to_string();
                                let r = rank(&d);
                                let reason = h.get("permissionDecisionReason").and_then(Value::as_str).unwrap_or("");
                                match &decision {
                                    Some((had, _)) if *had < r => {},
                                    Some((had, _)) if *had == r => decision_reasons.push(reason.to_string()),
                                    _ => {
                                        decision = Some((r, d));
                                        decision_reasons = vec![reason.to_string()];
                                    },
                                }
                            },
                            "permissionDecisionReason" => {},
                            _ => {
                                specific.entry(hk.clone()).or_insert_with(|| hv.clone());
                            },
                        }
                    }
                },
                _ => {
                    top.entry(k.clone()).or_insert_with(|| v.clone());
                },
            }
        }
    }
    if let Some((_, d)) = decision {
        specific.insert("permissionDecision".into(), Value::String(d));
        let reasons: Vec<&str> = decision_reasons.iter().map(String::as_str).filter(|r| !r.is_empty()).collect();
        if !reasons.is_empty() {
            specific.insert("permissionDecisionReason".into(), Value::String(reasons.join("\n")));
        }
    }
    let plain_text = plain.join("\n\n");
    if event::plain_is_context(event) {
        join(&mut specific, "additionalContext", &plain_text, "\n\n");
    } else {
        join(&mut top, "systemMessage", &plain_text, "\n");
    }
    for f in &failures {
        join(&mut top, "systemMessage", &format!("claude-shook: {f}"), "\n");
    }
    if !specific.is_empty() {
        specific.insert("hookEventName".into(), Value::String(event.to_string()));
        top.insert("hookSpecificOutput".into(), Value::Object(specific));
    }
    Answer {
        stdout: format!("{}\n", Value::Object(top)),
        stderr: String::new(),
        exit:   0,
    }
}

#[cfg(test)]
mod tests;
