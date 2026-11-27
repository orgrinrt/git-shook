// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

use serde_json::{Value, json};

use super::*;

fn said(exit: i32, stdout: &str, stderr: &str) -> Record {
    Record { exit, stdout: stdout.into(), stderr: stderr.into(), timed_out: false }
}

fn named(rs: Vec<Record>) -> Vec<(String, Record)> {
    rs.into_iter().enumerate().map(|(i, r)| (format!("e{i}"), r)).collect()
}

fn json_of(a: &Answer) -> Value {
    serde_json::from_str(&a.stdout).unwrap()
}

fn decision(d: &str, why: &str) -> String {
    json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": d, "permissionDecisionReason": why}}).to_string()
}

#[test]
fn nothing_said_prints_nothing() {
    let a = fold("PreToolUse", &named(vec![said(0, "", ""), said(0, "\n", "")]));
    assert_eq!(a, Answer { stdout: String::new(), stderr: String::new(), exit: 0 });
}

#[test]
fn a_block_is_the_answer_with_the_context_before_it_kept() {
    let ctx = json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": "mind the build"}}).to_string();
    let a = fold("PreToolUse", &named(vec![said(0, &ctx, ""), said(2, "", "not here\n")]));
    assert_eq!(a.exit, 2);
    assert_eq!(a.stdout, "");
    assert_eq!(a.stderr, "not here\n\nmind the build\n");
}

#[test]
fn a_deny_beats_a_defer_an_ask_and_an_allow_whatever_the_order() {
    let rs = vec![
        said(0, &decision("allow", "fine"), ""),
        said(0, &decision("ask", "sure?"), ""),
        said(0, &decision("deny", "no"), ""),
        said(0, &decision("defer", "later"), ""),
    ];
    let v = json_of(&fold("PreToolUse", &named(rs)));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(v["hookSpecificOutput"]["permissionDecisionReason"], "no");
    let v = json_of(&fold("PreToolUse", &named(vec![said(0, &decision("allow", "a"), ""), said(0, &decision("ask", "b"), "")])));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask");
}

#[test]
fn every_context_and_message_is_kept() {
    let a = json!({"systemMessage": "one", "hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "A"}}).to_string();
    let b = json!({"systemMessage": "two", "hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "B"}}).to_string();
    let v = json_of(&fold("PostToolUse", &named(vec![said(0, &a, ""), said(0, &b, "")])));
    assert_eq!(v["systemMessage"], "one\ntwo");
    assert_eq!(v["hookSpecificOutput"]["additionalContext"], "A\n\nB");
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PostToolUse");
}

#[test]
fn a_continue_false_and_a_block_decision_win() {
    let a = json!({"continue": true}).to_string();
    let b = json!({"continue": false, "stopReason": "done", "decision": "block", "reason": "keep going"}).to_string();
    let v = json_of(&fold("Stop", &named(vec![said(0, &a, ""), said(0, &b, "")])));
    assert_eq!(v["continue"], false);
    assert_eq!(v["decision"], "block");
    assert_eq!(v["reason"], "keep going");
}

#[test]
fn plain_text_alone_is_handed_on_and_beside_json_joins_the_context_where_it_is_context() {
    let a = fold("SessionStart", &named(vec![said(0, "first\n", ""), said(0, "second", "")]));
    assert_eq!(a.stdout, "first\n\nsecond\n");
    let ctx = json!({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "json"}}).to_string();
    let v = json_of(&fold("SessionStart", &named(vec![said(0, "plain", ""), said(0, &ctx, "")])));
    assert_eq!(v["hookSpecificOutput"]["additionalContext"], "json\n\nplain");
    let v = json_of(&fold("PreToolUse", &named(vec![said(0, "plain", ""), said(0, &decision("allow", ""), "")])));
    assert_eq!(v["systemMessage"], "plain");
}

#[test]
fn a_failure_is_named_to_the_person_and_the_call_goes_ahead() {
    let slow = Record { exit: 137, stdout: String::new(), stderr: String::new(), timed_out: true };
    let a = fold("PreToolUse", &named(vec![said(1, "", "broke\nmore\n"), slow, said(NOT_STARTED, "", "")]));
    assert_eq!(a.exit, 0);
    let v = json_of(&a);
    assert_eq!(
        v["systemMessage"],
        "claude-shook: e0 exited 1: broke\nclaude-shook: e1 did not finish in time and was stopped\nclaude-shook: e2 could not be started"
    );
}

#[test]
fn a_step_that_timed_out_with_two_is_a_failure_not_a_block() {
    let slow = Record { exit: 2, stdout: String::new(), stderr: "x".into(), timed_out: true };
    let a = fold("PreToolUse", &named(vec![slow]));
    assert_eq!(a.exit, 0);
}
