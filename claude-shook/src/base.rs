// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! Which shook base runs the chain: `SHOOK_BASE` where it is set, else a
//! `shook` beside this program, else the first `shook` on `PATH`. Every base is
//! held to the one suite, so which is found decides nothing of what a chain
//! does; the order only makes it predictable which one ran.

use std::path::{Path, PathBuf};

fn runnable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

pub fn find() -> Result<PathBuf, String> {
    if let Some(set) = std::env::var_os("SHOOK_BASE").filter(|s| !s.is_empty()) {
        let p = PathBuf::from(set);
        return if runnable(&p) {
            Ok(p)
        } else {
            Err(format!("SHOOK_BASE names {}, which is not a program", p.display()))
        };
    }
    if let Some(beside) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("shook"))) {
        if runnable(&beside) {
            return Ok(beside);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join("shook");
            if runnable(&p) {
                return Ok(p);
            }
        }
    }
    Err("no shook base was found beside claude-shook or on PATH; install git-shook, which carries it, or set SHOOK_BASE".into())
}
