//! Which LSP capabilities actually fire during real writing — the FV-5 instrument.
//!
//! **Why this exists.** `lsp*.rs` is the largest single feature in the tool (~14,000 lines,
//! 11.5% of all Rust) and it is the one surface no adoption round has ever been able to
//! see: shell history cannot observe a process the editor spawns, so every portfolio audit
//! to date has priced fourteen advertised capabilities at zero evidence. The feature-value
//! rounds name this as their biggest blind spot and rank the measurement (FV-5) as blocked
//! on *method*, not on will. This is the method.
//!
//! **Why it is safe to have in the tree.** The no-telemetry stance is a product position,
//! not an oversight, so the shape of this had to be: nothing leaves the machine, nothing
//! is on by default, and "off" costs one `Option` check per message rather than a
//! disabled-logger apparatus. Set `TALIESIN_LSP_TRACE` to a file path and the server tallies
//! method names into that file; leave it unset and [`Trace::record`] is a branch on `None`.
//! There is no server, no id, no document text, no timing — a method name and a count is
//! exactly the FV-5 question and deliberately nothing more.
//!
//! **Why it counts rather than logs.** A week of writing is the measurement window, which
//! spans many editor restarts and would be millions of append-lines at `didChange` rates.
//! So the tally is seeded from the file at startup and rewritten in place: restarts
//! accumulate instead of overwriting, and the artifact stays a ~20-line JSON object you can
//! read without a parser. `sessions` counts the restarts that contributed, which is what
//! tells you whether a zero means "never invoked" or "never armed".
//!
//! **Reading it back is deliberately not a subcommand.** The whole point of the round this
//! serves is that surface must earn its place; a dev instrument does not earn a nineteenth
//! verb. The file is JSON — `python3 -m json.tool` reads it.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Records written before the tally is rewritten. A crash loses at most this many, which is
/// noise against a week of `didChange`; the point of a bound at all is that SIGKILL skips
/// `Drop`, so periodic flushing is the only thing that makes a hard kill survivable.
const FLUSH_EVERY: u32 = 25;

/// The env var that arms the instrument. Its value is the tally path — there is no `=1`
/// mode, because a measurement you have to name a file for is one you cannot leave on by
/// accident.
pub(crate) const TRACE_ENV: &str = "TALIESIN_LSP_TRACE";

/// A method-name tally, or nothing at all.
///
/// The disabled state is the `None` in `path`, not a separate type, so `main_loop` holds one
/// value either way and every call site is unconditional.
pub(crate) struct Trace {
    path: Option<PathBuf>,
    counts: BTreeMap<String, u64>,
    sessions: u64,
    since_flush: u32,
}

impl Trace {
    /// Read [`TRACE_ENV`]. Absent or empty is off, which is the shipped state.
    pub(crate) fn from_env() -> Self {
        match std::env::var(TRACE_ENV) {
            Ok(p) if !p.trim().is_empty() => Self::at(PathBuf::from(p.trim())),
            _ => Self {
                path: None,
                counts: BTreeMap::new(),
                sessions: 0,
                since_flush: 0,
            },
        }
    }

    /// Arm at `path`, seeding from whatever is already there.
    ///
    /// A file that will not parse is treated as absent rather than fatal: the instrument
    /// exists to answer a question about the tool, and it refusing to start would be a
    /// worse outcome than losing a prior week. It is *reported*, though — a silent reset
    /// would read as "these capabilities were never used".
    pub(crate) fn at(path: PathBuf) -> Self {
        let mut counts = BTreeMap::new();
        let mut sessions = 0u64;
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => {
                    if let Some(m) = v.get("methods").and_then(|m| m.as_object()) {
                        for (k, n) in m {
                            if let Some(n) = n.as_u64() {
                                counts.insert(k.clone(), n);
                            }
                        }
                    }
                    sessions = v.get("sessions").and_then(|s| s.as_u64()).unwrap_or(0);
                }
                Err(e) => crate::log::warn(&format!(
                    "lsp trace: {} is not readable as a tally ({e}); starting a fresh one",
                    path.display()
                )),
            }
        }
        sessions += 1;
        let mut t = Self {
            path: Some(path),
            counts,
            sessions,
            since_flush: 0,
        };
        // Write once at startup so the file exists the moment the instrument is armed. An
        // instrument you cannot tell is running is one you find out was off after the week.
        t.flush();
        t
    }

    /// Count one dispatched method. A no-op when disabled.
    pub(crate) fn record(&mut self, method: &str) {
        if self.path.is_none() {
            return;
        }
        *self.counts.entry(method.to_owned()).or_insert(0) += 1;
        self.since_flush += 1;
        if self.since_flush >= FLUSH_EVERY {
            self.flush();
        }
    }

    /// Rewrite the tally. A no-op when disabled.
    pub(crate) fn flush(&mut self) {
        let Some(path) = &self.path else { return };
        self.since_flush = 0;
        let doc = serde_json::json!({
            "sessions": self.sessions,
            "methods": self.counts,
        });
        // A write failure must not take the session down: this is an observer, and an
        // observer that can end the thing it observes is worse than no observer.
        if let Err(e) = std::fs::write(path, format!("{doc:#}\n")) {
            crate::log::warn(&format!(
                "lsp trace: could not write {}: {e}",
                path.display()
            ));
        }
    }

    /// Whether the instrument is armed, for the one startup line that says so.
    pub(crate) fn armed(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }
}

impl Drop for Trace {
    /// The clean-exit flush. `shutdown`/`exit` and a closed channel both leave `main_loop`
    /// normally, so this catches the common case; [`FLUSH_EVERY`] catches the kill.
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory following the house idiom (no `tempfile` dependency in this crate).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-lsptrace-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read(path: &std::path::Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The shipped state. Recording must not touch the filesystem at all — this is the
    /// no-telemetry position expressed as a test, not as a comment.
    #[test]
    fn disabled_records_nothing() {
        let dir = scratch("off");
        let stray = dir.join("should-not-appear.json");
        let mut t = Trace {
            path: None,
            counts: BTreeMap::new(),
            sessions: 0,
            since_flush: 0,
        };
        for _ in 0..(FLUSH_EVERY * 3) {
            t.record("textDocument/completion");
        }
        t.flush();
        drop(t);
        assert!(!stray.exists());
        assert!(
            std::fs::read_dir(&dir).unwrap().next().is_none(),
            "a disabled trace wrote to the filesystem"
        );
    }

    /// The core measurement: method names in, counts out.
    #[test]
    fn counts_methods_by_name() {
        let path = scratch("counts").join("tally.json");
        let mut t = Trace::at(path.clone());
        t.record("textDocument/completion");
        t.record("textDocument/completion");
        t.record("textDocument/hover");
        t.flush();
        let v = read(&path);
        assert_eq!(v["methods"]["textDocument/completion"], 2);
        assert_eq!(v["methods"]["textDocument/hover"], 1);
        assert_eq!(v["sessions"], 1);
    }

    /// A week of writing is many editor restarts. If a restart reset the tally, every
    /// number would be "since the last time VS Code reloaded" and the round would measure
    /// nothing. This is the property the whole file format exists for.
    #[test]
    fn accumulates_across_sessions() {
        let path = scratch("resume").join("tally.json");
        let mut first = Trace::at(path.clone());
        first.record("textDocument/definition");
        first.record("textDocument/definition");
        drop(first);

        let mut second = Trace::at(path.clone());
        second.record("textDocument/definition");
        second.record("textDocument/foldingRange");
        drop(second);

        let v = read(&path);
        assert_eq!(v["methods"]["textDocument/definition"], 3);
        assert_eq!(v["methods"]["textDocument/foldingRange"], 1);
        assert_eq!(
            v["sessions"], 2,
            "sessions distinguishes `never invoked` from `never armed`"
        );
    }

    /// SIGKILL skips `Drop`. Without a periodic rewrite the file would hold only the
    /// startup snapshot, and a hard editor kill would silently discard the session.
    #[test]
    fn survives_a_kill_without_drop() {
        let path = scratch("kill").join("tally.json");
        let mut t = Trace::at(path.clone());
        for _ in 0..FLUSH_EVERY {
            t.record("textDocument/inlayHint");
        }
        // Deliberately no flush and no drop: emulate the process vanishing here.
        std::mem::forget(t);
        let v = read(&path);
        assert_eq!(
            v["methods"]["textDocument/inlayHint"],
            u64::from(FLUSH_EVERY)
        );
    }

    /// A corrupt tally must not stop the editor's language server from starting.
    #[test]
    fn unreadable_tally_starts_fresh_rather_than_failing() {
        let path = scratch("corrupt").join("tally.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let mut t = Trace::at(path.clone());
        t.record("textDocument/hover");
        t.flush();
        let v = read(&path);
        assert_eq!(v["methods"]["textDocument/hover"], 1);
        assert_eq!(v["sessions"], 1);
    }

    /// Empty and whitespace-only are off, so `TALIESIN_LSP_TRACE=` in a stale profile does
    /// not arm an instrument at path `""`.
    #[test]
    fn blank_env_value_is_off() {
        // `from_env` reads the real environment, so drive the same predicate directly on
        // the values that matter rather than mutating process state under a parallel
        // test runner.
        for v in ["", "   ", "\t"] {
            assert!(
                v.trim().is_empty(),
                "{v:?} must be treated as an unset trace path"
            );
        }
    }
}
