// Copyright (c) 2026                          orgrinrt                          orgrinrt@ikiuni.dev
// SPDX-License-Identifier: MPL-2.0        https://mozilla.org/MPL/2.0        contact@hiisi.digital

//! A directory of this process's own under the system's temporary directory.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A new, empty directory, removed by whoever asked for it.
pub fn dir(what: &str) -> PathBuf {
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("claude-shook-{what}-{}-{n}", std::process::id()));
        if std::fs::create_dir(&d).is_ok() {
            return d;
        }
    }
}
