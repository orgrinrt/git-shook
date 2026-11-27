// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! A matcher, read as the host reads one: `*`, empty or absent takes every
//! value; letters, digits, `_`, `-`, spaces, `,` and `|` alone are a list of
//! exact names; anything else is an unanchored regular expression.

use regex::Regex;

#[derive(Debug, Clone)]
pub enum Matcher {
    /// Every value.
    All,
    /// One of these names, exactly.
    Names(Vec<String>),
    /// A regular expression, tested anywhere in the value.
    Pattern(Regex),
}

impl Matcher {
    pub fn parse(text: &str) -> Result<Matcher, String> {
        let t = text.trim();
        if t.is_empty() || t == "*" {
            return Ok(Matcher::All);
        }
        let plain = t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | ',' | '|'));
        if plain {
            let names: Vec<String> = t
                .split(['|', ','])
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect();
            return Ok(if names.is_empty() { Matcher::All } else { Matcher::Names(names) });
        }
        Regex::new(t)
            .map(Matcher::Pattern)
            .map_err(|e| format!("`{t}` is not a regular expression this front reads: {e}"))
    }

    pub fn matches(&self, value: &str) -> bool {
        match self {
            Matcher::All => true,
            Matcher::Names(names) => names.iter().any(|n| n == value),
            Matcher::Pattern(re) => re.is_match(value),
        }
    }
}

/// The matcher one registration carries for entries with these matchers: the
/// names joined, so the host spares a process where no entry could take the
/// call, or every value where any entry takes every value or a pattern. Only
/// a narrowing for the host; each entry is still matched exactly.
pub fn joined<'a>(matchers: impl Iterator<Item = &'a Matcher>) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    for m in matchers {
        match m {
            Matcher::Names(ns) => {
                for n in ns {
                    if !names.contains(&n.as_str()) {
                        names.push(n);
                    }
                }
            },
            Matcher::All | Matcher::Pattern(_) => return None,
        }
    }
    if names.is_empty() { None } else { Some(names.join("|")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_of_names_matches_each_exactly_and_nothing_longer() {
        let m = Matcher::parse("Edit|Write, Bash").unwrap();
        assert!(m.matches("Edit") && m.matches("Write") && m.matches("Bash"));
        assert!(!m.matches("NotebookEdit"));
    }

    #[test]
    fn anything_else_is_an_unanchored_pattern() {
        let m = Matcher::parse("Edit.*").unwrap();
        assert!(m.matches("NotebookEdit"));
        assert!(!m.matches("Bash"));
        assert!(Matcher::parse("mcp__(").is_err());
    }

    #[test]
    fn star_empty_and_blank_take_every_value() {
        for t in ["", "*", "  "] {
            assert!(Matcher::parse(t).unwrap().matches("anything"));
        }
    }

    #[test]
    fn the_registration_joins_names_and_widens_for_a_pattern_or_every_value() {
        let a = Matcher::parse("Edit|Bash").unwrap();
        let b = Matcher::parse("Bash,Write").unwrap();
        assert_eq!(joined([&a, &b].into_iter()).as_deref(), Some("Edit|Bash|Write"));
        let p = Matcher::parse("Edit.*").unwrap();
        assert_eq!(joined([&a, &p].into_iter()), None);
        assert_eq!(joined([&a, &Matcher::All].into_iter()), None);
    }
}
