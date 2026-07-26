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
//! ## The axis the key cannot see: out-of-band input
//!
//! The key folds in *code and interpreter identity only*. So the whole class it cannot see
//! is **anything a cell reads that is not the code**:
//!
//! - a **data file** the cell opens (`pd.read_csv("data.csv")` — edit the CSV, the key is
//!   unchanged, the old numbers restore),
//! - an **environment variable** or a config file it reads,
//! - a **network resource** it fetches,
//! - the **wall clock** or anything else nondeterministic,
//! - the interpreter's **installed packages**: upgrading a library in place
//!   (`pip install --upgrade …` / `install.packages()`) is the same interpreter reporting
//!   the same `--version`, so every key is unchanged.
//!
//! This was previously written as "the lone by-design stale-hit path = packages", which
//! overclaims: packages are one member of the class, not the class. There is deliberately no
//! fingerprint knob for any of it — fingerprinting arbitrary out-of-band input is not
//! decidable from the source, and a knob that covered only some of it would be worse than an
//! honest boundary.
//!
//! **Mark a cell with an out-of-band input `#| cache: false`** and it re-runs every time,
//! along with everything downstream of it. To force one fresh run instead, use the dev-menu
//! "Restart kernel" (re-executes and rewrites the cache) or `TALIESIN_NO_CACHE` (bypasses it
//! entirely).
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
/// file as empty (and the next save rewrites it fresh). v2: the Python bridge stopped
/// emitting `ojs-define` for native `{js}` cells and switched to its own define-blob
/// script type. v3: a dual-theme figure switched to the `tali-fig-light` /
/// `tali-fig-dark` classes; entries cached before that rename still hold the old
/// classes, which no current CSS rule hides, so both variants would render stacked.
/// v4: the `{js}` runtime script types became `application/tali-js` / `tali-define`
/// and the cell target id became `tali-js-<block_id>`; entries cached before that
/// rename carry the old names, which the current runtime's exact-match selectors
/// never ingest, so `{js}` cells would silently receive no data.
///
/// (The v2/v3 notes deliberately describe the *change* rather than spelling the
/// retired prefix, which `crates/core/tests/retired_names.rs` keeps out of the tree.)
const FORMAT_VERSION: u32 = 4;

/// Per-page entry cap. Entries beyond the live set are kept (so toggling an edit
/// back and forth restores instantly instead of re-running) up to this bound, then
/// the least-recently-touched are evicted. Generous: a page rarely has more than a
/// few dozen cells, so this holds a deep edit history while staying small on disk.
const MAX_ENTRIES: usize = 1024;

/// Per-page byte budget, the bound [`MAX_ENTRIES`] cannot provide.
///
/// The count cap reasons about *cells* ("a page rarely has more than a few dozen"), but
/// what the cache stores is one entry per distinct cell **version**, and each entry holds
/// that cell's whole rendered output. For a text cell that is a few hundred bytes; for a
/// plot cell it is a base64 PNG. Measured on a warm preview session editing one
/// matplotlib cell: ~45 KB per entry, 150 edits, a 6.71 MB `_freeze/<page>.json` — growing
/// strictly linearly, because 150 is nowhere near 1024. Left to reach the count cap that
/// is ~45 MB resident per page and re-serialized on every save.
///
/// 16 MB because it has to clear two floors and stay under a ceiling: at least 2x
/// `kernel::MAX_RICH_BYTES` (8 MB), so one legitimately huge output can never fill the
/// budget by itself; several hundred ordinary rich outputs, which is more edit history
/// than a session revisits; and small enough that the `serve_site` warm set
/// (`MAX_WARM_PAGES` = 6) stays proportionate to the kernels it already holds
/// (~80-150 MB each) rather than becoming the dominant cost. Text-output pages are
/// unaffected: the entry cap still binds first for them.
const MAX_BYTES: usize = 16 * 1024 * 1024;

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
    /// Live total of `key.len() + value.len()` across `entries`, maintained on every
    /// insert and eviction so [`MAX_BYTES`] costs no walk of the map.
    bytes: usize,
    dirty: bool,
}

impl FreezeCache {
    /// A disabled cache: always misses, never writes.
    pub fn disabled() -> Self {
        FreezeCache {
            path: None,
            entries: HashMap::new(),
            order: Vec::new(),
            bytes: 0,
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
                let map: HashMap<String, String> =
                    d.entries.into_iter().map(|e| (e.k, e.v)).collect();
                (map, order)
            })
            .unwrap_or_default();
        let bytes = entries
            .iter()
            .map(|(k, v): (&String, &String)| k.len() + v.len())
            .sum();
        FreezeCache {
            path: Some(path),
            entries,
            order,
            bytes,
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
        // Re-putting a key replaces its value, so drop the old weight before adding the
        // new one or the running total drifts up forever on a re-run of the same cell.
        if let Some(prev) = self.entries.remove(&key) {
            self.bytes -= key.len() + prev.len();
        }
        self.order.retain(|k| k != &key);
        self.bytes += key.len() + output.len();
        self.order.push(key.clone());
        self.entries.insert(key, output);
        // `len() > 1` keeps the entry just inserted: a single output may legitimately be
        // larger than the whole budget (`kernel::MAX_RICH_BYTES` allows 8 MB), and a cache
        // that evicted its own newest entry would re-run that cell on every edit forever.
        while self.order.len() > 1 && (self.order.len() > MAX_ENTRIES || self.bytes > MAX_BYTES) {
            let evicted = self.order.remove(0);
            if let Some(v) = self.entries.remove(&evicted) {
                self.bytes -= evicted.len() + v.len();
            }
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
        // Unique per writer, not a fixed `<page>.json.tmp`. Two processes building the same
        // page (two previews, a build beside a preview, a parallel CI job) otherwise share
        // one temp path, so one can interleave a partial write into the other's rename. The
        // rename itself is atomic, so this is not a stale-hit risk — a corrupt read starts
        // empty — but it silently loses a whole cache generation. Same `<pid>_<uuid>` shape
        // the kernel/warm-pool runtime dirs already use.
        let tmp = path.with_extension(format!(
            "json.{}_{}.tmp",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        if let Err(e) = std::fs::write(&tmp, &json) {
            crate::log::warn(&format!("cannot write cache {}: {e}", tmp.display()));
            // The name is unique per writer, so a failed write leaves litter behind rather
            // than being overwritten by the next attempt. Clean it up here.
            let _ = std::fs::remove_file(&tmp);
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

    /// The count cap alone does not bound the cache, because an entry is a whole
    /// rendered cell output and a rich one is ~45 KB, not ~45 bytes.
    ///
    /// Measured (AP1-residual round, 2026-07-26): 150 edits to a single matplotlib cell
    /// in ONE warm preview session wrote a **6.71 MB** `_freeze/<page>.json` holding 151
    /// entries — linear, no eviction, with [`MAX_ENTRIES`] (1024) nowhere near binding.
    /// Extrapolated to the count cap that is ~45 MB held in RAM per page **and** rewritten
    /// to disk on every save. The cache is byte-bound in practice and was capped only by
    /// count, so [`MAX_BYTES`] is the bound that actually applies to rich output.
    #[test]
    fn eviction_bounds_total_bytes_not_only_entry_count() {
        let path = tmp();
        let mut c = FreezeCache::for_page(path.clone());
        let big = "x".repeat(1024 * 1024); // 1 MB, ~20x a real plot output
        for i in 0..(MAX_BYTES / (1024 * 1024) + 8) {
            c.put(format!("k{i}"), big.clone());
        }
        assert!(
            c.bytes <= MAX_BYTES,
            "cache holds {} bytes, over the {MAX_BYTES}-byte cap",
            c.bytes
        );
        assert!(
            c.order.len() < MAX_ENTRIES,
            "the BYTE cap must bind long before the count cap for rich output ({} entries)",
            c.order.len()
        );
        assert_eq!(
            c.get("k0"),
            None,
            "oldest entry evicted to stay under the cap"
        );
        let newest = format!("k{}", MAX_BYTES / (1024 * 1024) + 7);
        assert!(c.get(&newest).is_some(), "newest entry retained");
        let _ = std::fs::remove_file(&path);
    }

    /// A single output bigger than the whole budget must still be cached, not evict
    /// itself into a permanent miss. `MAX_RICH_BYTES` lets one output reach 8 MB, so this
    /// is reachable, and a cache that refuses its own newest entry would re-run that cell
    /// on every single edit.
    #[test]
    fn one_output_larger_than_the_budget_is_still_kept() {
        let path = tmp();
        let mut c = FreezeCache::for_page(path.clone());
        c.put("small".into(), "v".into());
        let huge = "x".repeat(MAX_BYTES + 1024);
        c.put("huge".into(), huge.clone());
        assert_eq!(c.get("huge"), Some(huge.as_str()), "newest entry survives");
        assert_eq!(c.order.len(), 1, "everything else was evicted for it");
        let _ = std::fs::remove_file(&path);
    }

    /// Byte accounting must survive a reload, or the cap silently stops applying to a
    /// cache that was loaded from disk rather than filled in this process.
    #[test]
    fn byte_accounting_is_restored_on_load() {
        let path = tmp();
        let mut c = FreezeCache::for_page(path.clone());
        c.put("k1".into(), "a".repeat(1000));
        c.put("k2".into(), "b".repeat(2000));
        c.save();
        let reloaded = FreezeCache::for_page(path.clone());
        assert_eq!(reloaded.bytes, c.bytes, "reload recomputes the byte total");
        assert_eq!(reloaded.bytes, 2 + 1000 + 2 + 2000);
        let _ = std::fs::remove_file(&path);
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

    /// Tokens that appear inside a CACHED cell's rendered output.
    ///
    /// The cache key hashes a cell's SOURCE, never its output, so changing the
    /// vocabulary that output carries busts nothing on its own: old entries replay
    /// verbatim and the current browser runtime no longer recognises them.
    /// `FORMAT_VERSION` is the only lever, and it has to be turned by hand.
    ///
    /// `d0b1ffa` is the bug this exists to prevent: the dual-theme figure-class
    /// rename shipped without a bump and needed a follow-up fix commit, because
    /// every test runs against a clean tree and no test has a stale `_freeze/`.
    const CACHED_OUTPUT_TOKENS: &[&str] = &[
        "application/tali-js",
        "tali-define",
        "tali-fig-light",
        "tali-fig-dark",
    ];

    #[test]
    fn cached_output_vocabulary_is_tied_to_format_version() {
        let digest = format!("{:016x}", fnv1a(&CACHED_OUTPUT_TOKENS.join("\u{1f}")));
        assert_eq!(
            (digest.as_str(), FORMAT_VERSION),
            ("71f1fe21dc878fcd", 4),
            "the cached-output token vocabulary changed. Bump FORMAT_VERSION, then \
             update BOTH values here. Skipping the bump makes every existing _freeze/ \
             entry replay markup the current runtime cannot read."
        );
    }
}
