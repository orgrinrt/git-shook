// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! The one registration per event in the host's settings file.
//!
//! A registration is this front's when its command is the one this front
//! writes; every other hook, and every other key of the file, is kept as it
//! was. An install removes each of its own and writes one for every event that
//! has entries, so an event whose entries are gone loses its registration.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::matcher;
use crate::registry::Entry;

const GUARD: &str = "command -v claude-shook >/dev/null 2>&1 || { echo ";

/// The command registered at `event`. Where claude-shook is not on the host's
/// `PATH` it says so in one line and exits 1, which the host shows the person
/// and goes ahead past: an absent front passes, since refusing every call
/// would leave a session no way to install it.
pub fn command(event: &str) -> String {
    format!(
        "{GUARD}'claude-shook is not on PATH, so the {event} hooks did not run; install it, then run claude-shook install' >&2; exit 1; }}; exec claude-shook run {event}"
    )
}

/// Whether `cmd` is a registration this front wrote.
pub fn ours(cmd: &str) -> bool {
    cmd.starts_with(GUARD) && cmd.contains("; exec claude-shook run ")
}

/// Seconds the host waits for the whole chain: the entries run one after
/// another, so it is their timeouts added, and a margin for the front itself.
pub fn timeout(entries: &[Entry]) -> u64 {
    entries.iter().map(|e| e.timeout).sum::<u64>() + 10
}

/// `settings` with this front's registrations replaced by one per event in
/// `events`.
pub fn registered(mut settings: Value, events: &[(String, Vec<Entry>)]) -> Result<Value, String> {
    let top = settings.as_object_mut().ok_or("the settings file does not hold an object")?;
    let hooks = top.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().ok_or("`hooks` is not an object")?;
    let names: Vec<String> = hooks.keys().cloned().collect();
    for name in names {
        let Some(groups) = hooks.get_mut(&name).and_then(Value::as_array_mut) else { continue };
        let before = groups.len();
        let mut removed = false;
        for g in groups.iter_mut() {
            if let Some(hs) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                let n = hs.len();
                hs.retain(|h| !h.get("command").and_then(Value::as_str).is_some_and(ours));
                removed |= hs.len() != n;
            }
        }
        if removed {
            groups.retain(|g| g.get("hooks").and_then(Value::as_array).is_none_or(|h| !h.is_empty()));
            if groups.is_empty() && before > 0 {
                hooks.remove(&name);
            }
        }
    }
    for (event, entries) in events {
        if entries.is_empty() {
            continue;
        }
        let mut group = Map::new();
        if crate::event::matched_field(event).is_some() {
            if let Some(m) = matcher::joined(entries.iter().map(|e| &e.matcher)) {
                group.insert("matcher".into(), Value::String(m));
            }
        }
        group.insert(
            "hooks".into(),
            json!([{"type": "command", "command": command(event), "timeout": timeout(entries)}]),
        );
        let list = hooks.entry(event.clone()).or_insert_with(|| Value::Array(Vec::new()));
        list.as_array_mut()
            .ok_or_else(|| format!("`hooks.{event}` is not a list"))?
            .push(Value::Object(group));
    }
    Ok(settings)
}

/// The settings file at `path`, an absent one read as empty.
pub fn read(path: &Path) -> Result<Value, String> {
    let shown = path.display();
    match std::fs::read_to_string(path) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{shown}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(e) => Err(format!("{shown}: {e}")),
    }
}

/// `settings`, read from `path`, with `events` registered, written back.
pub fn write(path: &Path, settings: Value, events: &[(String, Vec<Entry>)]) -> Result<(), String> {
    let shown = path.display();
    let out = registered(settings, events).map_err(|e| format!("{shown}: {e}"))?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(path, format!("{text}\n")).map_err(|e| format!("{shown}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::Matcher;

    fn entry(m: &str, t: u64) -> Entry {
        Entry {
            name: "x".into(),
            file: "x".into(),
            matcher: Matcher::parse(m).unwrap(),
            paths: Vec::new(),
            timeout: t,
            session_dir: true,
        }
    }

    #[test]
    fn one_registration_per_event_and_every_other_hook_kept() {
        let mine = json!({"type": "command", "command": "my-guard"});
        let start = json!({"permissions": {"allow": ["Bash(ls)"]}, "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [mine]}]}});
        let events = vec![("PreToolUse".to_string(), vec![entry("Bash", 5), entry("Edit", 7)]), ("Stop".to_string(), vec![entry("", 60)])];
        let once = registered(start.clone(), &events).unwrap();
        let twice = registered(once.clone(), &events).unwrap();
        assert_eq!(once, twice);
        assert_eq!(once["permissions"], start["permissions"]);
        let pre = once["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0]["hooks"][0]["command"], "my-guard");
        assert_eq!(pre[1]["matcher"], "Bash|Edit");
        assert_eq!(pre[1]["hooks"][0]["timeout"], 22);
        assert!(ours(pre[1]["hooks"][0]["command"].as_str().unwrap()));
        assert!(once["hooks"]["Stop"][0].get("matcher").is_none());
    }

    #[test]
    fn an_event_whose_entries_are_gone_loses_its_registration_and_nothing_else() {
        let events = vec![("Stop".to_string(), vec![entry("", 1)])];
        let with = registered(json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "kept"}]}]}}), &events).unwrap();
        let without = registered(with, &[]).unwrap();
        assert_eq!(without, json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "kept"}]}]}}));
        let alone = registered(registered(json!({}), &events).unwrap(), &[]).unwrap();
        assert_eq!(alone, json!({"hooks": {}}));
    }

    #[test]
    fn a_command_merely_naming_claude_shook_is_not_ours() {
        assert!(!ours("claude-shook run Stop"));
        assert!(!ours("echo exec claude-shook run Stop"));
        assert!(ours(&command("Stop")));
    }
}
