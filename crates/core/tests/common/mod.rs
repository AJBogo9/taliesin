//! Shared helpers for the integration test crates. Each `tests/*.rs` file is its
//! own crate, so common code lives here and is pulled in with `mod common;`. Not
//! every crate uses every item, hence the module-wide `dead_code` allow.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The repository's `corpus/` directory (the spec documents), resolved relative
/// to the `taliesin-core` crate the test belongs to.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

/// A throwaway project directory built up in-test, so each case can express
/// exactly the manifest/config/files it needs without committed fixtures. The
/// directory is removed on drop.
pub struct TempProj(pub PathBuf);

impl TempProj {
    pub fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "tali-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        TempProj(p)
    }

    /// Write a file (creating parent dirs) relative to the project root.
    pub fn file(&self, rel: &str, content: &str) -> &Self {
        let f = self.0.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, content).unwrap();
        self
    }

    /// Install an extension `name` whose `_extension.yml` is `manifest`.
    pub fn ext(&self, name: &str, manifest: &str) -> &Self {
        self.file(&format!("_extensions/{name}/_extension.yml"), manifest)
    }
}

impl Default for TempProj {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempProj {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
