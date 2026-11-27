// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! What the host's contract says of each hook event, the part a front needs:
//! which payload field a matcher is tested against, and whether the host reads
//! plain stdout as context. An event not named here is still run; it takes no
//! matcher, and its plain stdout is handed on as it was.

/// The payload field each event's matcher is tested against.
const MATCHED: &[(&str, &str)] = &[
    ("PreToolUse", "tool_name"),
    ("PostToolUse", "tool_name"),
    ("PostToolUseFailure", "tool_name"),
    ("PermissionRequest", "tool_name"),
    ("PermissionDenied", "tool_name"),
    ("SessionStart", "source"),
    ("Setup", "trigger"),
    ("PreCompact", "trigger"),
    ("PostCompact", "trigger"),
    ("Notification", "notification_type"),
    ("SubagentStart", "agent_type"),
    ("SubagentStop", "agent_type"),
    ("SessionEnd", "reason"),
    ("StopFailure", "error"),
    ("InstructionsLoaded", "load_reason"),
    ("UserPromptExpansion", "command_name"),
    ("PreModelSwitch", "to_model"),
    ("PostModelSwitch", "to_model"),
];

/// The events whose plain stdout the host adds to the session's context, or,
/// for `PreCompact`, reads as instructions for the summary.
const PLAIN_IS_CONTEXT: &[&str] = &["SessionStart", "UserPromptSubmit", "PreCompact"];

/// The field `event`'s matcher reads, or `None` where it takes no matcher.
pub fn matched_field(event: &str) -> Option<&'static str> {
    MATCHED.iter().find(|(e, _)| *e == event).map(|(_, f)| *f)
}

/// Whether the host reads `event`'s plain stdout as context.
pub fn plain_is_context(event: &str) -> bool {
    PLAIN_IS_CONTEXT.contains(&event)
}

/// An event name the host could send: letters only, as every one it has is.
pub fn plain_event(event: &str) -> bool {
    !event.is_empty() && event.chars().all(|c| c.is_ascii_alphabetic())
}
