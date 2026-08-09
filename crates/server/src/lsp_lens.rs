//! `textDocument/codeLens`: what a cell will do next time the document runs.
//!
//! **Labels, not buttons, since Wave 13 cut `taliesin run`.** This provider used to carry
//! ▶ Run Cell / Run Above, which named a command the CLI answered; with the verb gone a
//! button here would name nothing, so only the cache label remains. Execution reaches the
//! author through `taliesin preview`, which runs the edited cell and everything downstream
//! against the same warm kernel and writes the same `_freeze/`.
//!
//! **The ⚡ label is the 2026-07-18 DX audit's still-open "make caching legible" item**, and
//! it is the whole reason this method survives.
//! `freeze.rs` knows whether a cell's output can be restored without running it, and until now
//! nothing outside the browser said so — `decorations.ts` explicitly declined to half-build it
//! on the TypeScript side, because the answer belongs to the execution layer. It is computed
//! here from [`crate::exec::cell_cache_keys`], which is the executor's own key function, so
//! the lens cannot claim a hit the executor would miss on.
//!
//! **Still kernel-free.** The only subprocess is one `<interpreter> --version` per language
//! per session, memoized — the same probe the executor makes, and the thing the cache key is
//! seeded with. No kernel is started, nothing is executed, and a missing interpreter costs the
//! ⚡ label and nothing else.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Lenses for `blocks`, anchored on each executable cell's fence line.
///
/// `probe` carries the memoized interpreter identity and the page's `_freeze/` file; it is
/// threaded in rather than built here so a burst of lens requests (the client re-asks on every
/// edit) costs one probe and one cache read for the session, not one per keystroke.
pub(crate) fn code_lenses(
    blocks: &[taliesin_core::Block],
    probe: &mut CacheProbe,
) -> Vec<lsp_types::CodeLens> {
    // The 1-based fence line of each executable cell, in document order.
    let keys = crate::exec::cell_cache_keys(blocks, &mut |lang| probe.interp_id(lang));
    let lines: Vec<u32> = keys
        .iter()
        .map(|k| taliesin_core::render::sourcepos_start_line(&blocks[k.block_index].sourcepos))
        .collect();

    let mut out = Vec::new();
    for (i, cell) in keys.iter().enumerate() {
        let line = lines[i];
        // A generated block carries no position, so there is no fence to hang a label on.
        if line == 0 {
            continue;
        }
        let anchor = lsp_types::Range::new(
            lsp_types::Position::new(line - 1, 0),
            lsp_types::Position::new(line - 1, 0),
        );
        if let Some(title) = cache_label(cell, probe) {
            // A label, not a button: there is nothing to click, and a command here would
            // put a second meaning on a lens whose whole job is to say what will happen.
            out.push(lens(anchor, &title, "", None));
        }
    }
    out
}

fn lens(
    range: lsp_types::Range,
    title: &str,
    command: &str,
    arguments: Option<Vec<serde_json::Value>>,
) -> lsp_types::CodeLens {
    lsp_types::CodeLens {
        range,
        command: Some(lsp_types::Command {
            title: title.to_string(),
            command: command.to_string(),
            arguments,
        }),
        data: None,
    }
}

/// What this cell will do when the document next runs, or `None` when there is nothing worth
/// saying.
///
/// Silence is the default and is deliberate: "will run" above every cell of a fresh project is
/// noise on a line that already has two buttons. The two cases worth a word are the ones an
/// author would otherwise be surprised by — an output that restores without executing, and a
/// cell that re-executes however unchanged it is.
fn cache_label(cell: &crate::exec::CellCacheKey, probe: &mut CacheProbe) -> Option<String> {
    if !cell.cacheable {
        return Some("↻ always re-runs".to_string());
    }
    probe.is_cached(&cell.key).then(|| "⚡ cached".to_string())
}

/// The per-session state behind the ⚡ label: which interpreter seeds this page's cache keys,
/// and what is in its `_freeze/` file.
///
/// Both are memoized and both are re-validated cheaply, for the same reason
/// [`crate::lsp_project::SiteCache`] is: a code lens is re-requested on every edit, and neither
/// a `--version` fork nor a JSON read belongs on that path. The freeze file is validated by
/// `(mtime, len)` — the same stamp shape the project caches use — so a `build` or a preview run
/// that rewrites it is picked up, and a page that has not been executed since costs one `stat`.
pub(crate) struct CacheProbe {
    /// The page this probe is about, and the project it belongs to.
    page: PathBuf,
    root: Option<PathBuf>,
    python_pin: Option<String>,
    interp: HashMap<&'static str, String>,
    freeze: Option<(PathBuf, Option<std::time::SystemTime>, u64, Vec<String>)>,
}

impl CacheProbe {
    /// A probe for the page at `path`, given its enclosing project (`None` for a standalone
    /// document, which keeps its `_freeze/` beside itself exactly as `build` does).
    pub(crate) fn new(path: &Path, site: Option<&taliesin_core::Site>) -> Self {
        let root = site.map(|s| s.root.clone());
        CacheProbe {
            page: path.to_path_buf(),
            python_pin: site.and_then(|s| s.config.python.clone()),
            root,
            interp: HashMap::new(),
            freeze: None,
        }
    }

    /// The identity string that seeds this page's cumulative hash chain, memoized per language.
    ///
    /// Resolved through [`crate::interpreter`] and formatted by
    /// [`crate::exec::interp_identity`], because both halves have to match the executor
    /// byte for byte or every key differs and the lens reports "not cached" for a fully
    /// cached page. A language with no interpreter answers its own name, exactly as
    /// `compute_outputs` does for a language it has no spec for.
    fn interp_id(&mut self, lang: &'static str) -> String {
        if let Some(id) = self.interp.get(lang) {
            return id.clone();
        }
        let dir = self
            .root
            .clone()
            .or_else(|| self.page.parent().map(Path::to_path_buf));
        let id = match (dir, lang) {
            (Some(dir), "python") => {
                let r = crate::interpreter::resolve_python(self.python_pin.as_deref(), &dir);
                probe_identity(lang, &r.path)
            }
            _ => lang.to_string(),
        };
        self.interp.insert(lang, id.clone());
        id
    }

    /// Is there a stored output for `key`?
    fn is_cached(&mut self, key: &str) -> bool {
        self.freeze_keys().iter().any(|k| k == key)
    }

    /// This page's `_freeze/` keys, re-read when the file's `(mtime, len)` moved.
    fn freeze_keys(&mut self) -> &[String] {
        let path = self.freeze_path();
        let meta = std::fs::metadata(&path).ok();
        let mtime = meta.as_ref().and_then(|m| m.modified().ok());
        let len = meta.map(|m| m.len()).unwrap_or(0);
        let fresh = self
            .freeze
            .as_ref()
            .is_some_and(|(p, t, l, _)| *p == path && *t == mtime && *l == len);
        if !fresh {
            self.freeze = Some((path, mtime, len, read_freeze_keys(&self.page, &self.root)));
        }
        match &self.freeze {
            Some((_, _, _, keys)) => keys,
            None => &[],
        }
    }

    /// `<project>/_freeze/<rel>.json`, or `<dir>/_freeze/<stem>.json` for a standalone page.
    /// The same two shapes `serve_site` and `query` build, so the lens reads the file a run
    /// actually writes.
    fn freeze_path(&self) -> PathBuf {
        match &self.root {
            Some(root) => {
                let rel = self
                    .page
                    .strip_prefix(root)
                    .unwrap_or(&self.page)
                    .to_string_lossy()
                    .into_owned();
                crate::freeze::page_path(&root.join("_freeze"), &rel)
            }
            None => {
                let dir = self.page.parent().unwrap_or(Path::new("."));
                let stem = self
                    .page
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "doc".to_string());
                crate::freeze::page_path(&dir.join("_freeze"), &stem)
            }
        }
    }
}

/// The keys in a page's freeze file, or an empty list when there is none.
///
/// Reads the file directly rather than through [`crate::freeze::FreezeCache`] because the
/// *values* are whole rendered cell outputs — megabytes for a plot-heavy page — and the only
/// question here is which keys exist.
fn read_freeze_keys(page: &Path, root: &Option<PathBuf>) -> Vec<String> {
    let probe = CacheProbe {
        page: page.to_path_buf(),
        root: root.clone(),
        python_pin: None,
        interp: HashMap::new(),
        freeze: None,
    };
    let Ok(bytes) = std::fs::read(probe.freeze_path()) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    value["entries"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| e["k"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// `<program> --version`, synchronously and memoized process-wide.
///
/// Synchronous because the language server has no async runtime, and one fork per interpreter
/// per process is what the executor pays too. Bounded by the OS rather than a timer: this is
/// the same `--version` call `doctor` makes, and an interpreter that hangs on it has already
/// broken every other path in the tool.
fn probe_identity(lang: &'static str, program: &Path) -> String {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let key = format!("{lang}\u{0}{}", program.display());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return id;
    }
    let Ok(out) = std::process::Command::new(program)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
    else {
        // We could not *ask*. Not memoized, for the reason `probe_interp_id` gives: a
        // transient failure must not poison the answer for the process lifetime.
        return crate::exec::interp_identity(lang, program, "");
    };
    let id = crate::exec::interp_identity(
        lang,
        program,
        &crate::exec::version_line(&out.stdout, &out.stderr),
    );
    if let Ok(mut c) = cache.lock() {
        c.insert(key, id.clone());
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(src: &str) -> taliesin_core::RenderedDoc {
        taliesin_core::render_single_doc(src, Path::new("."))
    }

    const DOC: &str = "---\ntitle: T\n---\n\n# A\n\n```{python}\nx = 1\n```\n\ntext\n\n```{python}\nprint(x)\n```\n\n```bash\nls\n```\n";

    fn lenses_for(src: &str, page: &Path) -> Vec<lsp_types::CodeLens> {
        let doc = render(src);
        let mut probe = CacheProbe::new(page, None);
        code_lenses(&doc.blocks, &mut probe)
    }

    /// A `{bash}` fence is code and is not runnable. A lens over one is the exact drift
    /// `taliesin/cellRegions` was created to prevent, and it must not come back through a
    /// second scanner here.
    ///
    /// Driven with `#| cache: false` cells rather than with `DOC`, because a fresh cacheable
    /// cell is deliberately silent: counting lenses on `DOC` would return zero and pass for
    /// the wrong reason. `↻ always re-runs` is the one label that fires unconditionally.
    #[test]
    fn a_non_kernel_fence_gets_no_lens() {
        let page = std::env::temp_dir().join(format!("tali-lens-{}-b.tmd", std::process::id()));
        let src = "# A\n\n```{python}\n#| cache: false\nx = 1\n```\n\n```bash\nls\n```\n\n```{python}\n#| cache: false\nprint(x)\n```\n";
        let at: Vec<u32> = lenses_for(src, &page)
            .iter()
            .map(|l| l.range.start.line)
            .collect();
        // Computed from the source rather than written out, so the expectation cannot drift
        // away from the document it is about.
        let want: Vec<u32> = src
            .lines()
            .enumerate()
            .filter(|(_, l)| l.starts_with("```{python}"))
            .map(|(i, _)| i as u32)
            .collect();
        assert_eq!(want.len(), 2, "the fixture must have two kernel cells");
        assert_eq!(
            at, want,
            "a lens belongs on each `{{python}}` fence and on no other fence"
        );
    }

    /// `#| cache: false` says this cell re-runs however unchanged it is. That is exactly the
    /// thing an author is surprised by, so it is the one state worth a label besides ⚡.
    #[test]
    fn an_uncacheable_cell_says_so() {
        let page = std::env::temp_dir().join(format!("tali-lens-{}-d.tmd", std::process::id()));
        let src = "# A\n\n```{python}\n#| cache: false\nimport time\n```\n";
        let titles: Vec<String> = lenses_for(src, &page)
            .iter()
            .filter_map(|l| l.command.as_ref().map(|c| c.title.clone()))
            .collect();
        assert!(
            titles.iter().any(|t| t == "↻ always re-runs"),
            "expected the uncacheable label: {titles:?}"
        );
    }

    /// A page with no `_freeze/` must claim nothing. The ⚡ label is a promise that a run
    /// restores rather than executes, and a lens that showed it unconditionally would be a
    /// confident lie above every cell.
    #[test]
    fn nothing_is_claimed_cached_without_a_freeze_file() {
        let page = std::env::temp_dir().join(format!("tali-lens-{}-e.tmd", std::process::id()));
        let titles: Vec<String> = lenses_for(DOC, &page)
            .iter()
            .filter_map(|l| l.command.as_ref().map(|c| c.title.clone()))
            .collect();
        assert!(
            !titles.iter().any(|t| t.contains("cached")),
            "no freeze file, so nothing is cached: {titles:?}"
        );
    }

    /// And the positive axis, which is what makes the test above mean something: an entry
    /// under the executor's own key lights the label.
    #[test]
    fn a_stored_output_lights_the_cached_label() {
        let dir = std::env::temp_dir().join(format!("tali-lens-cached-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let page = dir.join("a.tmd");
        std::fs::write(&page, DOC).unwrap();

        // The key the EXECUTOR would look for, computed through its own function.
        let doc = render(DOC);
        let mut probe = CacheProbe::new(&page, None);
        let keys = crate::exec::cell_cache_keys(&doc.blocks, &mut |lang| probe.interp_id(lang));
        assert_eq!(keys.len(), 2, "two executable cells");
        std::fs::create_dir_all(dir.join("_freeze")).unwrap();
        std::fs::write(
            dir.join("_freeze/a.json"),
            serde_json::json!({
                "version": 4,
                "entries": [{ "k": keys[0].key, "v": "<pre>1</pre>" }],
            })
            .to_string(),
        )
        .unwrap();

        let titles: Vec<(u32, String)> = lenses_for(DOC, &page)
            .iter()
            .filter_map(|l| {
                l.command
                    .as_ref()
                    .map(|c| (l.range.start.line, c.title.clone()))
            })
            .collect();
        assert!(
            titles.contains(&(6, "⚡ cached".to_string())),
            "the first cell has a stored output: {titles:?}"
        );
        assert!(
            !titles.contains(&(12, "⚡ cached".to_string())),
            "the second does not, and must not borrow the first's answer: {titles:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
