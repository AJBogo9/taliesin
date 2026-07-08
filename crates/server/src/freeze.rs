//! Persistent execution cache (`_freeze/`): rendered cell outputs keyed by a
//! **cumulative content hash**, so an unchanged cell (with unchanged upstream and
//! the same interpreter) restores its output instead of re-executing — across
//! `build` invocations and preview restarts.
//!
//! ## Why a cumulative hash (and why invalidation is no longer "weird")
//!
//! A cell's key is `hash(interpreter-id → cell₀ code → cell₁ code → … → this
//! cell's code)`, computed left-to-right over the same-language cells in document
//! order (see [`cumulative_hashes`]). The key therefore encodes *every byte of
//! code the kernel ran to reach this cell's output*. Editing a cell — or anything
//! upstream of it, or swapping the interpreter (its `--version` seeds the chain) —
//! changes its key and the key of everything downstream, so a stale hit is
//! impossible *for the axes the key can see*: cell code, its upstream, and the
//! interpreter's own version. There are no mtime heuristics: the content *is* the key.
//!
//! The one axis the key does **not** capture is the interpreter's *installed
//! packages*. Upgrading a library in place (`pip install --upgrade …` / `install.packages()`
//! — same interpreter, same `--version`) leaves every key unchanged, so a cell that now
//! produces a different output can still restore the pre-upgrade one. This is the lone
//! by-design stale-hit path; there is deliberately no package-fingerprint knob. Force a
//! fresh run when a library upgrade matters: the dev-menu "Restart kernel"
//! (re-executes and rewrites the cache) or `TALIESIN_NO_CACHE` (bypasses it entirely).
//!
//! ## What is (and isn't) stored
//!
//! An entry is just the cell's inner output HTML (kernel outputs already render to
//! self-contained HTML — images are inline data URIs — so there are no sidecar
//! resource files). Outputs that are execution errors are never stored, so a
//! transient failure can't get baked in. `#| cache: false` cells are never stored
//! either (the [`crate::exec`] planner decides that; this module just persists what
//! it's told). Kernel *variable state* is deliberately not cached — that is what
//! makes the naive per-cell `cache` approach fragile, so a cold start can only skip work
//! when the whole document is unchanged (see [`crate::exec`] for the replay rule).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Bumped if the on-disk format or hashing scheme changes — or if the *bundled
/// output format* of a cached cell changes (the cell code is unchanged, so the
/// cumulative key wouldn't move on its own). A mismatch makes the loader treat the
/// file as empty (and the next save rewrites it fresh). v2: the Python bridge
/// emits `<script type="qmd-define">` (was `ojs-define`) for native `{js}` cells.
const FORMAT_VERSION: u32 = 2;

/// Per-page entry cap. Entries beyond the live set are kept (so toggling an edit
/// back and forth restores instantly instead of re-running) up to this bound, then
/// the least-recently-touched are evicted. Generous: a page rarely has more than a
/// few dozen cells, so this holds a deep edit history while staying small on disk.
const MAX_ENTRIES: usize = 1024;

/// The cache key uses the **same** 64-bit FNV-1a as the core's block-id scheme — one
/// shared definition in [`taliesin_core::hash`] (they must hash identically). The
/// cumulative chain below feeds each step's hex digest into the next, so the per-cell
/// keys are independent of any other document's cells.
pub use taliesin_core::hash::fnv1a;

/// Cumulative per-cell cache keys for one language's cells, in document order.
///
/// `interp` seeds the chain (so a different interpreter/version busts every cell,
/// and a `{python}` chain never collides with an `{r}` one). Each step folds the
/// previous digest and the cell's (options-stripped) code into the next, so cell
/// `i`'s key reflects all of `cells[0..=i]`.
pub fn cumulative_hashes(interp: &str, codes: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(codes.len());
    let mut acc = format!("{:016x}", fnv1a(interp));
    for code in codes {
        acc = format!("{:016x}", fnv1a(&format!("{acc}\n{code}")));
        out.push(acc.clone());
    }
    out
}

/// Resolve `_freeze/<rel-without-ext>.json` for a page. `rel` is the page's path
/// relative to the project (e.g. `posts/x.tmd`), or a bare stem for a single doc;
/// either way the extension is replaced with `.json` and sub-directories are
/// preserved, so the layout mirrors the source tree and is easy to inspect.
pub fn page_path(freeze_dir: &Path, rel: &str) -> PathBuf {
    freeze_dir.join(rel).with_extension("json")
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    /// Oldest-first, so eviction drops from the front; rewritten on every save.
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    /// Cumulative cache key (hex digest).
    k: String,
    /// The cell's inner output HTML.
    v: String,
}

/// One page's disk-backed output cache. A `None` path means disabled — every
/// lookup misses and nothing is persisted (used when caching is turned off, or for
/// a doc with no on-disk home).
pub struct FreezeCache {
    path: Option<PathBuf>,
    entries: HashMap<String, String>,
    /// Keys oldest-first, kept in sync with `entries`, for bounded LRU-ish eviction.
    order: Vec<String>,
    dirty: bool,
}

impl FreezeCache {
    /// A disabled cache: always misses, never writes.
    pub fn disabled() -> Self {
        FreezeCache {
            path: None,
            entries: HashMap::new(),
            order: Vec::new(),
            dirty: false,
        }
    }

    /// Load (or start fresh for) the cache file at `path`. Honours
    /// `TALIESIN_NO_CACHE`: when set, returns a disabled cache so a run neither
    /// reads nor writes `_freeze/`. A missing/corrupt/version-mismatched file is
    /// not an error — it just starts empty and the next save rewrites it.
    pub fn for_page(path: PathBuf) -> Self {
        if std::env::var_os("TALIESIN_NO_CACHE").is_some() {
            return Self::disabled();
        }
        let (entries, order) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<OnDisk>(&bytes).ok())
            .filter(|d| d.version == FORMAT_VERSION)
            .map(|d| {
                let order: Vec<String> = d.entries.iter().map(|e| e.k.clone()).collect();
                let map = d.entries.into_iter().map(|e| (e.k, e.v)).collect();
                (map, order)
            })
            .unwrap_or_default();
        FreezeCache {
            path: Some(path),
            entries,
            order,
            dirty: false,
        }
    }

    /// The cached output for `key`, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Store (or refresh) `key`'s output and mark it most-recently-used. A no-op on
    /// a disabled cache. Eviction drops the oldest entries past [`MAX_ENTRIES`].
    pub fn put(&mut self, key: String, output: String) {
        if self.path.is_none() {
            return;
        }
        self.order.retain(|k| k != &key);
        self.order.push(key.clone());
        self.entries.insert(key, output);
        while self.order.len() > MAX_ENTRIES {
            let evicted = self.order.remove(0);
            self.entries.remove(&evicted);
        }
        self.dirty = true;
    }

    /// Persist to disk if anything changed since load. Writes to a sibling temp
    /// file and renames it into place, so a crash mid-write can't corrupt an
    /// existing cache. Creates `_freeze/` (and any sub-dirs) on demand. Failures
    /// are reported but non-fatal — a build/preview still works, just uncached.
    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        let on_disk = OnDisk {
            version: FORMAT_VERSION,
            entries: self
                .order
                .iter()
                .filter_map(|k| {
                    self.entries.get(k).map(|v| Entry {
                        k: k.clone(),
                        v: v.clone(),
                    })
                })
                .collect(),
        };
        let Ok(json) = serde_json::to_vec(&on_disk) else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            crate::log::warn(&format!("cannot create {}: {e}", parent.display()));
            return;
        }
        let tmp = path.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, &json) {
            crate::log::warn(&format!("cannot write cache {}: {e}", tmp.display()));
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            crate::log::warn(&format!("cannot commit cache {}: {e}", path.display()));
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "tali-freeze-{}-{}.json",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn cumulative_hash_busts_self_upstream_and_interpreter() {
        let a = cumulative_hashes("py3.11", &["x = 1", "print(x)"]);
        assert_eq!(a.len(), 2);
        assert_ne!(a[0], a[1], "distinct cells get distinct keys");

        // Same inputs -> identical keys (deterministic, the basis of a hit).
        assert_eq!(cumulative_hashes("py3.11", &["x = 1", "print(x)"]), a);

        // Editing the FIRST cell changes BOTH keys (upstream invalidation): the
        // single property that makes manual cache-clearing unnecessary.
        let edit_up = cumulative_hashes("py3.11", &["x = 2", "print(x)"]);
        assert_ne!(edit_up[0], a[0]);
        assert_ne!(
            edit_up[1], a[1],
            "downstream key must move when upstream changes"
        );

        // Editing only the LAST cell leaves the first key stable, moves the last.
        let edit_down = cumulative_hashes("py3.11", &["x = 1", "print(x * 2)"]);
        assert_eq!(edit_down[0], a[0]);
        assert_ne!(edit_down[1], a[1]);

        // A different interpreter busts every key (a Python upgrade can't serve
        // outputs computed by the old one).
        let other_interp = cumulative_hashes("py3.12", &["x = 1", "print(x)"]);
        assert_ne!(other_interp[0], a[0]);
        assert_ne!(other_interp[1], a[1]);
    }

    #[test]
    fn put_get_survive_a_save_load_round_trip() {
        let path = tmp();
        let mut c = FreezeCache::for_page(path.clone());
        assert_eq!(c.get("k1"), None);
        c.put("k1".into(), "<pre>1</pre>".into());
        c.put("k2".into(), String::new()); // empty output is a legitimate cached result
        c.save();

        let reloaded = FreezeCache::for_page(path.clone());
        assert_eq!(reloaded.get("k1"), Some("<pre>1</pre>"));
        assert_eq!(reloaded.get("k2"), Some(""));
        assert_eq!(reloaded.get("absent"), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn disabled_cache_never_stores() {
        let mut c = FreezeCache::disabled();
        c.put("k".into(), "v".into());
        assert_eq!(c.get("k"), None);
        c.save(); // no path -> no-op, no panic
    }

    #[test]
    fn eviction_drops_oldest_past_the_cap() {
        let path = tmp();
        let mut c = FreezeCache::for_page(path.clone());
        for i in 0..MAX_ENTRIES + 5 {
            c.put(format!("k{i}"), format!("v{i}"));
        }
        assert_eq!(c.order.len(), MAX_ENTRIES, "live set is capped");
        assert_eq!(c.get("k0"), None, "oldest entry evicted");
        assert_eq!(
            c.get(&format!("k{}", MAX_ENTRIES + 4)),
            Some(format!("v{}", MAX_ENTRIES + 4).as_str()),
            "newest entry retained"
        );

        // Touching an existing key moves it to most-recent so it survives eviction.
        c.put(format!("k{}", MAX_ENTRIES), "touched".into());
        c.put("fresh".into(), "x".into());
        assert_eq!(
            c.get(&format!("k{}", MAX_ENTRIES)),
            Some("touched"),
            "re-touched entry survived"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn version_mismatch_starts_empty_without_error() {
        let path = tmp();
        std::fs::write(&path, br#"{"version":999,"entries":[{"k":"a","v":"b"}]}"#).unwrap();
        let c = FreezeCache::for_page(path.clone());
        assert_eq!(c.get("a"), None, "a future format version is ignored");

        // Corrupt JSON is likewise tolerated (starts empty).
        std::fs::write(&path, b"not json at all").unwrap();
        assert_eq!(FreezeCache::for_page(path.clone()).get("a"), None);
        let _ = std::fs::remove_file(&path);
    }
}
