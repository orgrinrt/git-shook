// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! A manifest: the entries one tool registers, an array per hook event named
//! as the host names it.
//!
//! ```toml
//! name = "guards"
//!
//! [[PreToolUse]]
//! name    = "keep-the-build-off"
//! run     = ["scripts/keep-the-build-off"]
//! matcher = "Bash"
//! paths   = ["engine"]
//! timeout = 30
//! ```

use std::path::Path;

use serde::Deserialize;

use crate::event;
use crate::matcher::Matcher;

/// One entry of a manifest, in the order it was written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// What names it in the chain, a plain name.
    pub name:    String,
    /// The argument list it runs, its program first.
    pub run:     Vec<String>,
    /// The directory it runs in, relative to the root; the session's own
    /// directory where it is absent, as the host runs a hook.
    pub dir:     Option<String>,
    pub matcher: Option<String>,
    /// Directories, relative to the root or absolute, a call must land under.
    #[serde(default)]
    pub paths:   Vec<String>,
    /// Seconds it may run before it is stopped.
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name:   String,
    pub events: Vec<(String, Vec<Row>)>,
}

/// A name the chain directory can carry as a file name and nothing escapes by.
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn line_safe(s: &str) -> bool {
    !s.contains('\n') && !s.contains('\r')
}

/// Read the manifest at `path`, refusing anything this front does not know.
pub fn read(path: &Path) -> Result<Manifest, String> {
    let shown = path.display();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    let table: toml::Table = text.parse().map_err(|e| format!("{shown}: {e}"))?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match table.get("name") {
        None => stem,
        Some(toml::Value::String(n)) => n.clone(),
        Some(_) => return Err(format!("{shown}: `name` is not a string")),
    };
    if !plain_name(&name) {
        return Err(format!("{shown}: `{name}` is not a name a chain can carry"));
    }
    let mut events = Vec::new();
    for (key, value) in &table {
        if key == "name" {
            continue;
        }
        if !event::plain_event(key) {
            return Err(format!("{shown}: `{key}` is not a hook event"));
        }
        let rows: Vec<Row> = value
            .clone()
            .try_into()
            .map_err(|e| format!("{shown}: [[{key}]]: {e}"))?;
        for row in &rows {
            check(key, row).map_err(|e| format!("{shown}: [[{key}]] {}: {e}", row.name))?;
        }
        let mut seen: Vec<&str> = Vec::new();
        for row in &rows {
            if seen.contains(&row.name.as_str()) {
                return Err(format!("{shown}: [[{key}]] names {} twice", row.name));
            }
            seen.push(&row.name);
        }
        events.push((key.clone(), rows));
    }
    Ok(Manifest { name, events })
}

fn check(event: &str, row: &Row) -> Result<(), String> {
    if !plain_name(&row.name) {
        return Err("`name` is not a name a chain can carry".into());
    }
    if row.run.is_empty() || row.run[0].is_empty() {
        return Err("`run` names no program".into());
    }
    let every = row.run.iter().chain(&row.paths).chain(row.dir.iter()).chain(row.matcher.iter());
    if !every.into_iter().all(|s| line_safe(s)) {
        return Err("a value holds a line break, which an entry cannot carry".into());
    }
    if let Some(m) = &row.matcher {
        let parsed = Matcher::parse(m)?;
        if !matches!(parsed, Matcher::All) && event::matched_field(event).is_none() {
            return Err(format!("{event} takes no matcher"));
        }
    }
    if row.timeout == Some(0) {
        return Err("`timeout` is 0, which never lets it run".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_text(name: &str, text: &str) -> Result<Manifest, String> {
        let dir = crate::scratch::dir("manifest");
        let path = dir.join(format!("{name}.toml"));
        std::fs::write(&path, text).unwrap();
        let out = read(&path);
        let _ = std::fs::remove_dir_all(dir);
        out
    }

    #[test]
    fn rows_keep_their_order_and_the_name_defaults_to_the_file() {
        let m = read_text("guards", "[[PreToolUse]]\nname = \"b\"\nrun = [\"x\"]\n\n[[PreToolUse]]\nname = \"a\"\nrun = [\"y\"]\n").unwrap();
        assert_eq!(m.name, "guards");
        let names: Vec<&str> = m.events[0].1.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn what_the_front_does_not_know_is_refused() {
        assert!(read_text("m", "[[PreToolUse]]\nname = \"a\"\nrun = [\"x\"]\nasync = true\n").unwrap_err().contains("async"));
        assert!(read_text("m", "[[pre-commit]]\nname = \"a\"\nrun = [\"x\"]\n").unwrap_err().contains("not a hook event"));
        assert!(read_text("m", "[[Stop]]\nname = \"a\"\nrun = [\"x\"]\nmatcher = \"Bash\"\n").unwrap_err().contains("takes no matcher"));
        assert!(read_text("m", "[[Stop]]\nname = \"a/b\"\nrun = [\"x\"]\n").is_err());
        assert!(read_text("m", "[[Stop]]\nname = \"a\"\nrun = []\n").is_err());
        assert!(read_text("m", "[[Stop]]\nname = \"a\"\nrun = [\"x\\ny\"]\n").unwrap_err().contains("line break"));
        assert!(read_text("m", "[[Stop]]\nname = \"a\"\nrun = [\"x\"]\n[[Stop]]\nname = \"a\"\nrun = [\"y\"]\n").unwrap_err().contains("twice"));
    }
}
