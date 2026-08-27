//! Server-side syntax highlighting (syntect), done at render time.
//!
//! Code is colored in Rust rather than by a client-side highlight.js pass. Three
//! wins, matching how KaTeX is already handled (render in Rust, ship offline):
//!
//!   - the exported HTML is self-contained — no CDN dependency for a built post's
//!     own highlighting,
//!   - the first paint (SSR) is already highlighted — no flash of plain code, and
//!     nothing to re-run on the client after each block update,
//!   - highlighting moves into the corpus-tested Rust half.
//!
//! We emit syntect's *scope classes* (each prefixed `tali-hl-`) rather than inline
//! colors, and map them to a palette in CSS with a `[data-theme=dark]` override,
//! so the light/dark toggle restyles code with no re-highlight.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Prefix on every emitted scope class, so they can't collide with page CSS and
/// are easy to target (`.tali-hl-keyword`, `.tali-hl-string`, …).
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "tali-hl-" };

/// syntect's bundled syntaxes. `_newlines` is the variant the line-based
/// `ClassedHTMLGenerator` expects.
fn bundled() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The `bat`-curated extras, consulted **only** when [`bundled`] has no syntax for
/// a token. This is not a superset of syntect's defaults: it is a separate curated
/// set whose definitions for shared languages (Rust, Python, JavaScript, JSON,
/// HTML, YAML) emit *different* scope spans. Preferring it wholesale would silently
/// re-highlight every existing code block in every document, so the bundled set
/// always wins and the extras supply only what it lacks: TypeScript and TOML.
///
/// Loaded lazily, so a document that uses neither never pays to deserialize it.
fn extras() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(two_face::syntax::extra_newlines)
}

/// Resolve a token to its syntax **and the set that owns it** (the generator must be
/// given the owning set, since a `SyntaxReference` indexes into it).
///
/// Order is load-bearing and is the same rule the `extras` comment states: the bundled
/// set always wins, so adding a later one can never re-highlight a language that already
/// resolved. A token neither set carries resolves to nothing and renders as plain escaped
/// text; there is no third, vendored tier (one was carried for PowerShell until
/// 2026-08-09 and nothing outside the corpus ever used it).
fn resolve(token: &str) -> Option<(&'static SyntaxReference, &'static SyntaxSet)> {
    if let Some(s) = bundled().find_syntax_by_token(token) {
        return Some((s, bundled()));
    }
    extras().find_syntax_by_token(token).map(|s| (s, extras()))
}

/// Force both syntax sets to deserialize now, on whatever thread calls this.
///
/// Exists for [`crate::prewarm`]: the `two-face` extras cost a measured 138.0 ms to
/// deserialize (2026-08-27, release) and [`resolve`] pulls them in the moment a document
/// uses a token the bundled set lacks — `ts` and `toml`, both of which `docs/` uses on its
/// first page. On the critical path that is a third of a whole `build docs/guide`.
pub fn load_syntax_sets() {
    bundled();
    extras();
}

/// Map a markdown language token to a token the syntax sets know.
fn alias(lang: &str) -> &str {
    match lang {
        "ojs" | "js" => "javascript",
        "ts" => "typescript",
        "sh" | "shell" | "zsh" => "bash",
        "py" => "python",
        "rs" => "rust",
        "yml" => "yaml",
        other => other,
    }
}

/// A `(code, language) -> highlighted HTML` memo, bounded by the BYTES it holds.
///
/// `math.rs` makes this exact argument for KaTeX and it applies here unchanged:
/// highlighting is a pure function of its inputs, and the dev server re-renders the
/// *whole* document on every save, so without a memo every keystroke re-runs syntect
/// over every code block in the document. Measured on
/// `corpus/tech-blog/posts/em-algorithm/index.tmd` (2026-08-27, release): 10.7 ms of a
/// 12.6 ms warm render, i.e. 85% of the edit loop, spent re-deriving identical HTML.
///
/// Bounded by bytes rather than by `math.rs`'s entry count, because the two values have
/// different shapes: a rendered math expression is small and bounded, a code block's HTML
/// is whatever the author pasted. An 8192-entry cap would be a memory leak with a
/// respectable name.
#[derive(Default)]
struct HighlightCache {
    map: HashMap<Key, Arc<str>>,
    order: VecDeque<Key>,
    bytes: usize,
}

/// The memo key: the code, and the **alias-resolved** language token, so `js` and
/// `javascript` share one entry instead of holding two copies of identical HTML.
type Key = (String, String);

fn key(code: &str, token: &str) -> Key {
    (code.to_string(), token.to_string())
}

impl HighlightCache {
    /// Insert `key -> html`, evicting oldest-first (FIFO) until `budget` bytes fit.
    /// A no-op when the key is already present (so `order` never holds duplicates and a
    /// re-render neither reorders the queue nor double-counts its bytes) and when one
    /// entry alone exceeds the budget (which could otherwise evict the entire live set
    /// to make room for something that still would not fit).
    fn insert_bounded(&mut self, key: Key, html: Arc<str>, budget: usize) {
        if self.map.contains_key(&key) || html.len() > budget {
            return;
        }
        while self.bytes + html.len() > budget {
            match self.order.pop_front() {
                Some(old) => {
                    if let Some(v) = self.map.remove(&old) {
                        self.bytes -= v.len();
                    }
                }
                None => break,
            }
        }
        self.bytes += html.len();
        self.order.push_back(key.clone());
        self.map.insert(key, html);
    }
}

static CACHE: LazyLock<Mutex<HighlightCache>> =
    LazyLock::new(|| Mutex::new(HighlightCache::default()));

/// 16 MiB of rendered HTML. The whole corpus plus both docs books is far under this;
/// it exists so a pathological document cannot grow the cache without limit.
const CACHE_BUDGET_BYTES: usize = 16 * 1024 * 1024;

/// Highlight a fenced code block's `code` for `lang`, returning the inner HTML for
/// a `<code>` element: text is HTML-escaped and wrapped in `<span class="tali-hl-…">`
/// scope spans. An unknown/missing language (or any highlighter error) falls back
/// to plain escaped text, so output is always valid and never panics. Memoized: see
/// [`HighlightCache`].
///
/// The cache is consulted BEFORE [`resolve`], which is load-bearing rather than merely
/// tidy: resolving a token the bundled set lacks deserializes the `two-face` extras, a
/// measured 138.6 ms. Looking up the memo first means a warm re-render of a page with a
/// `ts` or `toml` block never touches that path at all.
pub fn highlight(code: &str, lang: Option<&str>) -> String {
    let Some(token) = lang.map(alias) else {
        return crate::render::html_escape(code);
    };
    let k = key(code, token);
    // The value is an `Arc<str>` so the clone taken under the lock is a refcount bump
    // and the string copy happens outside it. That matters because the whole-project
    // render loops (`Site::harvest_xref_numbers` and friends) run pages concurrently,
    // so this mutex is on several threads' hot path at once.
    let hit = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map
        .get(&k)
        .cloned();
    if let Some(html) = hit {
        return html.to_string();
    }
    let Some((syntax, ss)) = resolve(token) else {
        return crate::render::html_escape(code);
    };
    let html: Arc<str> = highlight_uncached(code, syntax, ss).into();
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert_bounded(k, Arc::clone(&html), CACHE_BUDGET_BYTES);
    html.to_string()
}

fn highlight_uncached(code: &str, syntax: &SyntaxReference, ss: &SyntaxSet) -> String {
    let mut hl = ClassedHTMLGenerator::new_with_class_style(syntax, ss, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        if hl.parse_html_for_line_which_includes_newline(line).is_err() {
            return crate::render::html_escape(code);
        }
    }
    hl.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_language_emits_scope_classes() {
        let html = highlight("def f():\n    return 1\n", Some("python"));
        assert!(
            html.contains("tali-hl-"),
            "no scope classes emitted: {html}"
        );
        // the keyword `def` should be wrapped as a keyword scope
        assert!(html.contains("tali-hl-keyword"), "keyword not highlighted");
    }

    #[test]
    fn unknown_language_is_plain_escaped() {
        let html = highlight("a < b && c", Some("no-such-lang"));
        assert_eq!(html, "a &lt; b &amp;&amp; c");
        assert!(!html.contains("tali-hl-"));
    }

    #[test]
    fn no_language_is_plain_escaped() {
        assert_eq!(highlight("x < y", None), "x &lt; y");
    }

    /// FIFO eviction under a BYTE budget, not an entry count: a code block's rendered
    /// HTML is unbounded in a way a math expression's is not (one pasted file is worth
    /// thousands of `$x$`), so the cap that matters is bytes held, and `math.rs`'s
    /// entry-count cap would let 8192 large blocks sit in memory.
    #[test]
    fn cache_evicts_oldest_first_and_stays_within_its_byte_budget() {
        let mut c = HighlightCache::default();
        for i in 0..3 {
            c.insert_bounded(key(&i.to_string(), "rust"), "0123456789".into(), 30);
        }
        assert_eq!(c.map.len(), 3, "three 10-byte entries fit a 30-byte budget");
        c.insert_bounded(key("3", "rust"), "0123456789".into(), 30);
        assert_eq!(c.map.len(), 3, "stays bounded");
        assert!(!c.map.contains_key(&key("0", "rust")), "oldest evicted");
        assert!(c.map.contains_key(&key("3", "rust")), "newest kept");
        assert!(c.map.contains_key(&key("2", "rust")), "recent kept");
        // Re-inserting an existing key is a no-op: it must not reorder the queue or
        // double-count its bytes (which would evict live entries on every re-render).
        c.insert_bounded(key("2", "rust"), "different".into(), 30);
        assert_eq!(&*c.map[&key("2", "rust")], "0123456789");
        assert_eq!(c.order.len(), 3, "no duplicate in the eviction queue");
        assert_eq!(c.bytes, 30, "bytes not double-counted");
    }

    /// One entry larger than the whole budget must not spin the eviction loop forever
    /// nor evict everything to make room for something that can never fit.
    #[test]
    fn an_entry_larger_than_the_budget_is_not_cached_and_evicts_nothing() {
        let mut c = HighlightCache::default();
        c.insert_bounded(key("keep", "rust"), "0123456789".into(), 30);
        c.insert_bounded(key("huge", "rust"), "x".repeat(31).into(), 30);
        assert!(
            c.map.contains_key(&key("keep", "rust")),
            "live entry survives"
        );
        assert!(
            !c.map.contains_key(&key("huge", "rust")),
            "oversized not cached"
        );
        assert_eq!(c.bytes, 10);
    }

    /// The memo must be transparent (same input -> identical output) and keyed on the
    /// RESOLVED token, so `js` and `javascript` share one entry rather than aliasing
    /// to different ones — and so `python` never serves a `rust` block's HTML.
    #[test]
    fn memoized_highlight_is_stable_and_keyed_on_the_resolved_language() {
        let code = "let x = 1;\n";
        let first = highlight(code, Some("js"));
        let second = highlight(code, Some("js"));
        assert_eq!(first, second, "memoized highlight must be stable");
        assert_eq!(
            highlight(code, Some("javascript")),
            first,
            "`js` aliases to `javascript`: one cache entry, not two"
        );
        assert_ne!(
            highlight(code, Some("rust")),
            first,
            "a different language must be a distinct cache entry"
        );
    }

    /// The point of the memo: a repeat call does no syntect work. Asserted through the
    /// cache's own state rather than a wall clock, so it cannot flake on a slow machine.
    #[test]
    fn a_repeated_highlight_is_served_from_the_cache() {
        let code = "fn unique_to_this_test() -> u8 { 7 }\n";
        let k = key(code, "rust");
        let _ = highlight(code, Some("rust"));
        let cached = {
            let c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            c.map.get(&k).cloned()
        };
        let cached = cached.expect("first highlight populates the cache");
        assert_eq!(
            highlight(code, Some("rust")),
            *cached,
            "second call is the cached html"
        );
    }

    /// The docs use `ts` (22 blocks) and `toml` (8). syntect's bundled set carries
    /// neither, so both degraded to plain text until the `two-face` extras landed.
    #[test]
    fn typescript_and_toml_highlight() {
        let ts = highlight("const x: number = 1;\n", Some("ts"));
        assert!(ts.contains("tali-hl-"), "ts not highlighted: {ts}");
        let tsx = highlight("const x: number = 1;\n", Some("typescript"));
        assert!(
            tsx.contains("tali-hl-"),
            "typescript not highlighted: {tsx}"
        );
        let toml = highlight("[deps]\nx = 1\n", Some("toml"));
        assert!(toml.contains("tali-hl-"), "toml not highlighted: {toml}");
    }

    /// Adding the extras must not re-highlight a language syntect already knew.
    ///
    /// Comparing `SyntaxReference::name` is **not** enough: the extras carry their own
    /// "Rust"/"Python"/… definitions under identical names but different contexts, so a
    /// name check passes while the emitted bytes drift. Assert instead that every
    /// established token resolves *into the bundled set itself*.
    #[test]
    fn established_languages_still_come_from_the_bundled_set() {
        for token in [
            "rust", "rs", "bash", "sh", "zsh", "yaml", "yml", "js", "ojs", "markdown", "python",
            "py", "json", "css", "html", "r", "bibtex", "diff", "sql", "c",
        ] {
            let (_, set) = resolve(alias(token)).unwrap_or_else(|| panic!("`{token}` unresolved"));
            assert!(
                std::ptr::eq(set, bundled()),
                "`{token}` now resolves into the extras set; its highlighting would drift"
            );
        }
    }

    /// The bytes, not just the provenance: highlighting must match what the bundled
    /// set alone produces. These six are the languages whose extras definition differs.
    #[test]
    fn established_languages_emit_unchanged_bytes() {
        let ss = bundled();
        for (token, code) in [
            ("rust", "pub fn f(x: u32) -> u32 { x + 1 } // c\n"),
            ("python", "def f(x):\n    return 'a' # c\n"),
            ("js", "const a = 1; // c\n"),
            ("json", "{\"a\": 1}\n"),
            ("html", "<p class=\"x\">hi</p>\n"),
            ("yaml", "a: 1 # c\n"),
        ] {
            let syntax = ss.find_syntax_by_token(alias(token)).unwrap();
            let mut hl = ClassedHTMLGenerator::new_with_class_style(syntax, ss, CLASS_STYLE);
            for line in LinesWithEndings::from(code) {
                hl.parse_html_for_line_which_includes_newline(line).unwrap();
            }
            assert_eq!(
                highlight(code, Some(token)),
                hl.finalize(),
                "`{token}` highlighting drifted from the bundled set"
            );
        }
    }

    /// The extras are the only place TypeScript and TOML can come from.
    #[test]
    fn typescript_and_toml_come_from_the_extras() {
        for token in ["ts", "typescript", "toml"] {
            assert!(bundled().find_syntax_by_token(alias(token)).is_none());
            let (_, set) = resolve(alias(token)).expect("resolves via extras");
            assert!(std::ptr::eq(set, extras()));
        }
    }

    /// A token nothing carries a syntax for renders as plain escaped text rather than
    /// failing: `text` is the deliberate spelling of that, and a typo lands in the same
    /// place. There is no `known_language` query any more — the fence-language lint that
    /// asked it was cut, and it had no caller but its own test.
    #[test]
    fn a_language_with_no_syntax_renders_plain_and_escaped() {
        for plain in ["text", "console", "output", "none", "pyton", "no-such-lang"] {
            assert_eq!(highlight("a < b", Some(plain)), "a &lt; b", "{plain}");
        }
    }
}
