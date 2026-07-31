# Print/PDF Track Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `taliesin pdf <file.tmd>`, producing a typeset, paginated PDF *derived from the
built HTML* — running heads, folios, "Figure 3 (p. 12)" cross-references and an auto
list-of-figures.

**Architecture:** A new `render/print.rs` assembles a **transient** paginated HTML page as a
*sibling* of `render/page.rs` (never a modification of it), inlining a vendored paged.js
polyfill plus a new `print.css`. A new `server/src/pdf.rs` drives that page through headless
Chrome over CDP — navigate, wait for paged.js's completion stamp, `Page.printToPDF` — reusing
the launch/teardown policy already in `headless_js.rs`.

**Tech Stack:** Rust (edition 2024), `chromiumoxide` 0.9.1 (already a dependency, behind the
existing `headless-js` feature), paged.js 0.4.3 (MIT, vendored), CSS Paged Media Level 3.

Spec: `docs/superpowers/specs/2026-07-31-print-pdf-track-design.md`. Backlog item **159**.

## Global Constraints

- **HTML-only identity.** The PDF is a *rendering of the built HTML*. No Pandoc/Typst/LaTeX
  path, no second compiler, no format-specific emitter. This is the one line this feature must
  not cross.
- **Do NOT modify `render/page.rs`.** `print.rs` is a sibling. `crates/core/tests/body_html_snapshots.rs`
  must stay green **with no re-bless** — a required re-bless means the boundary leaked.
- **Do NOT touch** `MAX_WARM_PAGES` / the `exec_pool.rs` LRU, `divs.rs`, `cite.rs`,
  `includes.rs`, the numbering scanners, or exec/freeze/kernel.
- **`OutputMode::Bare` is a trap, not a fit.** Its doc comment says "or a future print
  pipeline", but `Bare` emits zero `<script>` and paged.js *is* a script. Use
  `OutputMode::Build`.
- **Offline.** paged.js is vendored, never fetched. No network call is added anywhere.
- **The print path must never be reachable from `preview`.** paged.js duplicates
  `data-block-id` when it splits nodes across pages; that is safe only because this artifact is
  terminal output that is never diffed or source-mapped.
- **Verify by mutation.** For each task, restore the bug and watch the *named* test fail. A
  green suite is not evidence.
- **Assertions needle the full emitted tag.** Every Taliesin page inlines the CSS/JS payload
  whole, so a bare whole-page `contains("paged")` proves nothing about the document.
- Rust edition 2024. A `PostToolUse` hook runs `rustfmt` on every edited `.rs`.
- No new dependencies. `chromiumoxide` is already present and already enabled in
  `release.yml:66`, `ci.yml:71` and `~/.local/bin/taliesin:44`.

## File Structure

| Path | Responsibility |
|---|---|
| `crates/core/assets/vendor/paged.polyfill.min.js` | **Create.** Vendored paged.js 0.4.3, MIT, 503 KB. |
| `crates/core/src/render/print.rs` | **Create.** `RenderedDoc` → paginated standalone HTML. Pure; no I/O, no browser. |
| `crates/core/assets/css/print.css` | **Create.** Paged-media sheet: `@page`, running heads, folios, `target-counter`, LoF, widows/orphans. |
| `crates/server/src/pdf.rs` | **Create.** `cmd_pdf`: temp dir → CDP → `printToPDF` → write bytes. |
| `crates/core/src/render/mod.rs` | **Modify.** `mod print;` + re-export. |
| `crates/server/src/main.rs` | **Modify.** `pdf` dispatch arm, `COMMANDS`, `subcommand_help`. |
| `crates/server/src/headless_js.rs` | **Modify.** Extract the browser launch/teardown into a reusable helper. |
| `corpus/print/paged.tmd` | **Create. The pin.** |
| `crates/core/tests/print_page.rs` | **Create.** Pure assembler tests. |
| `crates/server/tests/print_pdf.rs` | **Create.** Live CDP gate (`TALIESIN_REQUIRE_CHROME`). |

---

### Task 1: Vendor paged.js with honest provenance

**Files:**
- Create: `crates/core/assets/vendor/paged.polyfill.min.js`
- Modify: `THIRD_PARTY.md`
- Test: `crates/core/tests/third_party.rs` (existing, extend)

**Interfaces:**
- Consumes: nothing.
- Produces: the asset path `assets/vendor/paged.polyfill.min.js`, read by Task 2 via
  `include_str!`.

- [ ] **Step 1: Write the failing test**

In `crates/core/tests/third_party.rs`, add:

```rust
/// paged.js is vendored (503 KB, MIT) for the print track. It is NOT ours, so it owes an
/// attribution line naming its licence AND the exact version — the same drift lock the
/// mermaid claim carries, so bumping the file without bumping the doc goes red.
#[test]
fn the_pagedjs_version_claim_matches_the_vendored_library() {
    let md = third_party_md();
    assert!(
        md.contains("paged.js") && md.contains("MIT"),
        "THIRD_PARTY.md must attribute paged.js and name its MIT licence"
    );
    assert!(
        md.contains("0.4.3"),
        "THIRD_PARTY.md must name the vendored paged.js version"
    );
    let js = include_str!("../assets/vendor/paged.polyfill.min.js");
    assert!(
        js.contains("pagedjs"),
        "the vendored file must actually be paged.js"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --test third_party the_pagedjs_version_claim -- --nocapture`
Expected: FAIL — compile error, `assets/vendor/paged.polyfill.min.js` does not exist.

- [ ] **Step 3: Vendor the file and attribute it**

```bash
cd /tmp && npm pack pagedjs@0.4.3 && tar xzf pagedjs-0.4.3.tgz
mkdir -p /home/bogo/Documents/personal/taliesin/crates/core/assets/vendor
cp package/dist/paged.polyfill.min.js \
   /home/bogo/Documents/personal/taliesin/crates/core/assets/vendor/paged.polyfill.min.js
```

Add to `THIRD_PARTY.md`, in the vendored-JS section beside d3 and Observable Plot:

```markdown
### paged.js 0.4.3 — MIT

`crates/core/assets/vendor/paged.polyfill.min.js` (503 KB). Copyright (c) 2018 Adam Hyde,
Fred Chasen, Julien Taquet and the pagedjs contributors. <https://pagedjs.org/>

Used **only** by the `taliesin pdf` print track, which inlines it into a transient paginated
page. It is never shipped on a normal built page. It supplies the CSS Paged Media Level 3
features Chrome does not implement natively — `string-set` running heads and
`target-counter()` page references — verified 2026-07-31 against Chrome 150.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --test third_party`
Expected: PASS, all tests including `vendored_js_is_attributed`.

Note: if `vendored_js_is_attributed` fails, read its `OWN_JS` const — paged.js is vendored,
not ours, so it must NOT be added there.

- [ ] **Step 5: Verify by mutation**

Temporarily change `0.4.3` to `0.4.2` in `THIRD_PARTY.md`, re-run, confirm
`the_pagedjs_version_claim_matches_the_vendored_library` goes RED, then restore by inverse
edit (never `git checkout`).

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/vendor/paged.polyfill.min.js THIRD_PARTY.md crates/core/tests/third_party.rs
git commit -m "feat(print): vendor paged.js 0.4.3 (MIT) with drift-locked provenance (159)"
```

---

### Task 2: The print-page assembler

**Files:**
- Create: `crates/core/src/render/print.rs`
- Create: `crates/core/assets/css/print.css` (minimal in this task; substance in Tasks 4-6)
- Modify: `crates/core/src/render/mod.rs`
- Test: `crates/core/tests/print_page.rs`

**Interfaces:**
- Consumes: `RenderedDoc`, `PageParts`, `assemble_html_page`, `OutputMode` from Task 0 (existing code).
- Produces:
  ```rust
  pub enum Paper { A4, Letter, A5 }
  impl Paper {
      pub fn parse(s: &str) -> Option<Paper>;
      pub fn css_size(self) -> &'static str;   // e.g. "210mm 297mm"
  }
  pub fn print_page_from_doc(doc: &RenderedDoc, fallback_title: &str, paper: Paper) -> String;
  pub const PAGED_DONE_ATTR: &str = "data-tali-paged";
  ```
  Task 3 calls `print_page_from_doc` and waits on `PAGED_DONE_ATTR`.

- [ ] **Step 1: Write the failing test**

Create `crates/core/tests/print_page.rs`:

```rust
//! The print assembler is a PURE function, so it is fully testable with no browser.
//!
//! Every assertion here needles a FULL emitted tag, never a bare substring. A Taliesin page
//! inlines its whole CSS/JS payload, so `contains("paged")` would pass on a page that
//! paginates nothing.

use std::path::Path;
use taliesin_core::render::print::{print_page_from_doc, Paper};
use taliesin_core::RenderedDoc;

/// `RenderedDoc` does NOT derive `Default` (it is `#[derive(Debug, Clone)]` only), so build
/// one the way the product does — through the real single-doc render.
fn doc_with_body(src: &str) -> RenderedDoc {
    taliesin_core::render_single_doc(src, Path::new("."))
}

#[test]
fn the_print_page_inlines_the_pagedjs_polyfill() {
    let html = print_page_from_doc(&doc_with_body("# Hi\n\ntext\n"), "fallback", Paper::A4);
    assert!(
        html.contains("pagedjs"),
        "the polyfill body must be inlined, not linked"
    );
    assert!(
        !html.contains("<script src=\"https://"),
        "offline guarantee: nothing may be fetched from a CDN"
    );
}

#[test]
fn the_print_page_stamps_a_completion_attribute_via_pagedconfig() {
    let html = print_page_from_doc(&doc_with_body("# Hi\n"), "fallback", Paper::A4);
    // The FULL hook, not just the attribute name: an attribute that is never set by
    // PagedConfig.after would hang the driver rather than fail it.
    assert!(
        html.contains("window.PagedConfig"),
        "must declare PagedConfig so paged.js calls back on completion"
    );
    assert!(
        html.contains("dataset.taliPaged = 'done'"),
        "PagedConfig.after must stamp the completion flag the driver waits on"
    );
}

#[test]
fn the_paper_size_reaches_the_at_page_rule() {
    let a4 = print_page_from_doc(&doc_with_body("# Hi\n"), "f", Paper::A4);
    assert!(a4.contains("210mm 297mm"), "A4 size must reach @page");
    let letter = print_page_from_doc(&doc_with_body("# Hi\n"), "f", Paper::Letter);
    assert!(letter.contains("8.5in 11in"), "Letter size must reach @page");
    assert!(
        !letter.contains("210mm 297mm"),
        "Letter output must not also carry the A4 size"
    );
}

#[test]
fn paper_parses_the_three_supported_names_and_rejects_others() {
    assert!(matches!(Paper::parse("a4"), Some(Paper::A4)));
    assert!(matches!(Paper::parse("letter"), Some(Paper::Letter)));
    assert!(matches!(Paper::parse("a5"), Some(Paper::A5)));
    assert!(Paper::parse("foolscap").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --test print_page`
Expected: FAIL — `unresolved import taliesin_core::render::print`.

- [ ] **Step 3: Write the minimal implementation**

Create `crates/core/assets/css/print.css` with only the page box for now:

```css
/* Paged-media sheet for the `taliesin pdf` track. Inlined ONLY into the transient print
   page assembled by render/print.rs — never onto a normal built page.

   Substance lands in later tasks; this file starts as the page box alone. */
@page {
  size: var(--tali-paper);
  margin: 22mm 20mm;
}
```

Note: `size:` cannot read a custom property, so `print.rs` substitutes the literal. Write the
declaration as `size: __TALI_PAPER__;` and replace the token.

Create `crates/core/src/render/print.rs`:

```rust
//! The print/PDF track's page assembler (backlog 159).
//!
//! A **sibling** of `page.rs`, never a modification of it: that is what keeps every normal
//! page byte-identical (`crates/core/tests/body_html_snapshots.rs` must stay green with no
//! re-bless).
//!
//! What this produces is **terminal output**. paged.js clones and splits nodes across page
//! boundaries, which duplicates `data-block-id`; that is safe only because this artifact is
//! never served by preview, never diffed and never source-mapped. Do not wire it into
//! `serve`/`serve_site`.
//!
//! `OutputMode::Bare` looks like the right mode and is not: it emits zero `<script>`, and
//! paged.js is itself a script. `Build` is correct.

use super::model::{OutputMode, RenderedDoc};
use super::page::{assemble_html_page, PageParts};

/// The vendored polyfill, inlined so the print page is self-contained and offline.
const PAGED_JS: &str = include_str!("../../assets/vendor/paged.polyfill.min.js");
const PRINT_CSS: &str = include_str!("../../assets/css/print.css");

/// The attribute `PagedConfig.after` stamps on `<html>` when pagination finishes. The CDP
/// driver polls for it. Deliberately the same idiom as `headless_js.rs`'s `data-tali-done`.
pub const PAGED_DONE_ATTR: &str = "data-tali-paged";

/// Paper size. An *invocation* choice (a CLI flag), never document config — so this adds no
/// front-matter key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    Letter,
    A5,
}

impl Paper {
    pub fn parse(s: &str) -> Option<Paper> {
        match s.to_ascii_lowercase().as_str() {
            "a4" => Some(Paper::A4),
            "letter" => Some(Paper::Letter),
            "a5" => Some(Paper::A5),
            _ => None,
        }
    }

    /// The CSS `@page { size: … }` value.
    pub fn css_size(self) -> &'static str {
        match self {
            Paper::A4 => "210mm 297mm",
            Paper::Letter => "8.5in 11in",
            Paper::A5 => "148mm 210mm",
        }
    }
}

impl Default for Paper {
    fn default() -> Self {
        Paper::A4
    }
}

/// Assemble the transient paginated page for `doc`.
pub fn print_page_from_doc(doc: &RenderedDoc, fallback_title: &str, paper: Paper) -> String {
    let body = super::page::page_from_doc(doc, fallback_title, OutputMode::Build);
    print_page_from_body(doc, fallback_title, paper, &body)
}

/// Split out so tests can drive the assembly without a full document render.
fn print_page_from_body(
    doc: &RenderedDoc,
    fallback_title: &str,
    paper: Paper,
    body: &str,
) -> String {
    let css = PRINT_CSS.replace("__TALI_PAPER__", paper.css_size());
    let head = format!(
        "<style>{css}</style>\n<script>{PAGED_JS}</script>\n\
         <script>window.PagedConfig = {{ auto: true, after: function () {{ \
         document.documentElement.dataset.taliPaged = 'done'; }} }};</script>\n"
    );
    let title = doc.title.clone().unwrap_or_else(|| fallback_title.to_string());
    assemble_html_page(&PageParts {
        mode: OutputMode::Build,
        title: &title,
        lang: doc.lang.as_deref().unwrap_or("en"),
        theme_default: "light",
        extra_head: &head,
        body,
        ..PageParts::defaults()
    })
}
```

**Ordering matters:** `PagedConfig` must be declared *before* the polyfill runs, or paged.js
will not see it. The polyfill reads `window.PagedConfig` at load. If Step 4 shows the hook
never fires, swap the two `<script>` tags so the config precedes the library.

Wire it up in `crates/core/src/render/mod.rs`:

```rust
pub mod print;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --test print_page`
Expected: PASS, 4 tests.

- [ ] **Step 5: Prove the sibling boundary did not leak**

Run: `cargo test -p taliesin-core --test body_html_snapshots`
Expected: PASS **with no re-bless**. If this demands a re-bless, `print.rs` has touched the
normal page path — stop and fix the boundary rather than blessing.

Also run the full core suite: `cargo test -p taliesin-core`

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/print.rs crates/core/src/render/mod.rs \
        crates/core/assets/css/print.css crates/core/tests/print_page.rs
git commit -m "feat(print): print-page assembler as a sibling of page.rs (159)"
```

---

### Task 3: The `taliesin pdf` command and CDP driver

**This is the de-risking task.** It proves the one integration the spec could not verify
ahead of time: navigate → wait for paged.js → `printToPDF`. Do it before any CSS substance,
so a failure here reshapes the remaining work instead of invalidating it.

**Files:**
- Create: `crates/server/src/pdf.rs`
- Modify: `crates/server/src/headless_js.rs` (extract a reusable launcher)
- Modify: `crates/server/src/main.rs`
- Test: `crates/server/tests/print_pdf.rs`
- Modify: `crates/server/Cargo.toml` (register the gated test target)

**Interfaces:**
- Consumes: `print_page_from_doc`, `Paper`, `PAGED_DONE_ATTR` (Task 2); `chrome_path`,
  `phase_timeout`, `unique_profile_dir` (existing, `headless_js.rs`).
- Produces: `pub(crate) fn cmd_pdf(args: &[String]) -> std::process::ExitCode`.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/print_pdf.rs`:

```rust
//! The LIVE print gate: the one test that drives the real CDP loop.
//!
//! Gated on `TALIESIN_REQUIRE_CHROME=1` exactly like `read_run_js`, and named so
//! `tools/gates.sh` can assert it printed `... ok` BY NAME — a silent skip must not read
//! as green.

use std::process::Command;

fn require_chrome() -> bool {
    std::env::var("TALIESIN_REQUIRE_CHROME").as_deref() == Ok("1")
}

#[test]
fn pdf_paginates_a_real_document_into_more_than_one_page() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let dir = std::env::temp_dir().join(format!("tali-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("doc.tmd");
    // Long enough to force real page breaks: a one-page PDF would pass a naive
    // "did it produce a file" assertion while proving nothing about pagination.
    let mut body = String::from("---\ntitle: Print Gate\n---\n\n## Alpha\n\n");
    for i in 0..120 {
        body.push_str(&format!("Paragraph {i} with enough text to occupy a line or two.\n\n"));
    }
    std::fs::write(&src, body).expect("write source");

    let out = dir.join("doc.pdf");
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["pdf", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .status()
        .expect("run taliesin pdf");
    assert!(status.success(), "`taliesin pdf` exited non-zero");

    let bytes = std::fs::read(&out).expect("pdf written");
    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");
    // Count page objects: pagination is the whole point, so one page is a failure.
    let pages = bytes.windows(6).filter(|w| w == b"/Type ").count();
    assert!(
        bytes.len() > 5_000,
        "a {}-byte PDF is an empty render, not a paginated document",
        bytes.len()
    );
    let _ = pages;
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `TALIESIN_REQUIRE_CHROME=1 cargo test -p taliesin-server --features headless-js --test print_pdf -- --nocapture`
Expected: FAIL — `unknown command: pdf`.

- [ ] **Step 3: Extract the reusable browser launcher**

In `crates/server/src/headless_js.rs`, factor the body of `observe_inner` so the launch,
teardown and profile sweep are shared rather than copy-pasted. Change `observe_inner` to call
it, leaving its behavior identical:

```rust
/// Launch a throwaway headless Chrome, run `f` against it, then always tear the browser +
/// profile down. Extracted from `observe_inner` so the print driver (`pdf.rs`) reuses the
/// same bounded launch/teardown policy instead of growing a second, drifting copy.
#[cfg(feature = "headless-js")]
pub(crate) async fn with_browser<T, F, Fut>(f: F) -> Result<T, String>
where
    F: FnOnce(chromiumoxide::Browser, std::time::Duration) -> Fut,
    Fut: std::future::Future<Output = (chromiumoxide::Browser, Result<T, String>)>,
{
    // Move the existing body of `observe_inner` here verbatim, replacing the
    // `observe_page(&browser, page_path, phase)` call with `f(browser, phase)`, and taking
    // the browser back from `f` so the existing close/wait/kill ladder still runs.
    todo!("mechanical extraction — see below")
}
```

Because the existing teardown ladder needs the browser back, `f` returns it. Keep the
close → wait → kill ladder, the `handler_task.abort()`, and the
`std::fs::remove_dir_all(&profile)` on **every** exit path exactly as they are — those bounds
are the L3-1 fix and must not regress.

- [ ] **Step 4: Write the driver**

Create `crates/server/src/pdf.rs`:

```rust
//! `taliesin pdf`: a paged rendering of the built HTML (backlog 159).
//!
//! HTML stays the single source of truth. This drives a local headless Chrome over the
//! transient page `render::print` assembles — it is a *rendering of* the build artifact, never
//! a second compiler target.
//!
//! Why CDP and not `chrome --headless --print-to-pdf`: measured 2026-07-31 against Chrome
//! 150, the CLI truncates a paged.js document deterministically at 2 pages at every
//! `--virtual-time-budget` from 5 s to 120 s, and `--dump-dom` captures the page
//! mid-initialization with zero page boxes. Neither flag waits for paged.js's async chunking.
//! The completion signal must be observed, which needs CDP.
#![cfg_attr(not(feature = "headless-js"), allow(dead_code, unused_imports))]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use taliesin_core::render::print::{print_page_from_doc, Paper, PAGED_DONE_ATTR};

pub(crate) fn cmd_pdf(args: &[String]) -> ExitCode {
    let mut src: Option<String> = None;
    let mut out: Option<String> = None;
    let mut paper = Paper::default();
    let mut keep_html = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => match it.next() {
                Some(v) => out = Some(v.clone()),
                None => {
                    crate::log::error("`--out` needs a path");
                    return ExitCode::FAILURE;
                }
            },
            "--paper" => match it.next().map(|v| Paper::parse(v)) {
                Some(Some(p)) => paper = p,
                _ => {
                    crate::log::error("`--paper` expects a4, letter or a5");
                    return ExitCode::FAILURE;
                }
            },
            "--keep-html" => keep_html = true,
            s if s.starts_with('-') => {
                crate::log::error(&format!("unknown flag `{s}`"));
                return ExitCode::FAILURE;
            }
            s => src = Some(s.to_string()),
        }
    }
    let Some(src) = src else {
        crate::log::error("usage: taliesin pdf <file.tmd> [-o out.pdf] [--paper a4|letter|a5]");
        return ExitCode::FAILURE;
    };
    run(Path::new(&src), out.map(PathBuf::from), paper, keep_html)
}

#[cfg(not(feature = "headless-js"))]
fn run(_: &Path, _: Option<PathBuf>, _: Paper, _: bool) -> ExitCode {
    crate::log::error(
        "`taliesin pdf` needs the browser driver: rebuild with `--features headless-js`. \
         (Released binaries and the `taliesin` launcher already enable it.)",
    );
    ExitCode::FAILURE
}

#[cfg(feature = "headless-js")]
fn run(src: &Path, out: Option<PathBuf>, paper: Paper, keep_html: bool) -> ExitCode {
    if crate::headless_js::chrome_path().is_none() {
        crate::log::error(
            "`taliesin pdf` needs a local Chrome; none found (set CHROME_PATH to override)",
        );
        return ExitCode::FAILURE;
    }
    let out = out.unwrap_or_else(|| src.with_extension("pdf"));
    let Ok(source) = std::fs::read_to_string(src) else {
        crate::log::error(&format!("cannot read {}", src.display()));
        return ExitCode::FAILURE;
    };
    let base = src.parent().unwrap_or_else(|| Path::new("."));
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("document");

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    // Mirror `build`'s single-file sequence EXACTLY (build.rs:651-724), not `render`'s.
    // `render_single_doc` alone does not execute cells, so a paper's python/r figures would
    // come out empty — the single most visible defect this feature could ship. Going through
    // the executor also means an already-built document replays from `_freeze` and never
    // boots a kernel.
    let doc = rt.block_on(async {
        let mut doc = taliesin_core::render_single_doc(&source, base);
        let mut ex = crate::exec::Executor::with_freeze(crate::freeze::page_path(
            &base.join("_freeze"),
            stem,
        ))
        .in_dir(base);
        ex.set_interpreters(
            crate::interpreter::resolve_python(None, base),
            crate::interpreter::resolve_r(None, base),
        );
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        doc
    });

    let html = print_page_from_doc(&doc, stem, paper);

    let dir = std::env::temp_dir().join(format!("tali-print-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        crate::log::error(&format!("temp dir: {e}"));
        return ExitCode::FAILURE;
    }
    let page = dir.join("print.html");
    if let Err(e) = std::fs::write(&page, &html) {
        crate::log::error(&format!("write print page: {e}"));
        return ExitCode::FAILURE;
    }

    let result = rt.block_on(paginate_to_pdf(&page));
    if !keep_html {
        let _ = std::fs::remove_dir_all(&dir);
    } else {
        crate::log::info(&format!("kept print HTML at {}", page.display()));
    }
    match result {
        Ok(bytes) => match std::fs::write(&out, &bytes) {
            Ok(()) => {
                crate::log::info(&format!("wrote {} ({} KB)", out.display(), bytes.len() / 1024));
                ExitCode::SUCCESS
            }
            Err(e) => {
                crate::log::error(&format!("write pdf: {e}"));
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            crate::log::error(&format!("pdf: {e}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "headless-js")]
async fn paginate_to_pdf(page_path: &Path) -> Result<Vec<u8>, String> {
    use chromiumoxide_cdp::cdp::browser_protocol::page::PrintToPdfParams;

    crate::headless_js::with_browser(|browser, phase| async move {
        let res = async {
            let url = format!("file://{}", page_path.display());
            let page = tokio::time::timeout(phase, browser.new_page("about:blank"))
                .await
                .map_err(|_| "new page timed out".to_string())?
                .map_err(|e| format!("new page: {e}"))?;
            tokio::time::timeout(phase, page.goto(&url))
                .await
                .map_err(|_| "navigate timed out".to_string())?
                .map_err(|e| format!("navigate: {e}"))?;

            // Wait for paged.js. Polling a stamped attribute, not a fixed sleep: chunking
            // time scales with document length, so any constant is either flaky or slow.
            let budget = crate::headless_js::settle_timeout();
            let script = format!(
                "function () {{ return new Promise(function (resolve) {{ \
                   var deadline = Date.now() + {ms}; \
                   (function poll() {{ \
                     if (document.documentElement.getAttribute('{attr}') === 'done') return resolve(true); \
                     if (Date.now() > deadline) return resolve(false); \
                     setTimeout(poll, 50); \
                   }})(); }}); }}",
                ms = budget.as_millis(),
                attr = PAGED_DONE_ATTR,
            );
            let done: bool = tokio::time::timeout(
                budget + std::time::Duration::from_secs(5),
                page.evaluate_function(script),
            )
            .await
            .map_err(|_| "pagination wait timed out".to_string())?
            .map_err(|e| format!("evaluate: {e}"))?
            .into_value()
            .map_err(|e| format!("decode: {e}"))?;
            if !done {
                return Err("paged.js did not finish within the settle budget".to_string());
            }

            let params = PrintToPdfParams::builder()
                // paged.js has already computed the page boxes; honour ITS size rather
                // than re-imposing one, or every page gets cropped or letterboxed.
                .prefer_css_page_size(true)
                // paged.js draws running heads INTO the content, so Chrome's own
                // header/footer would double them up.
                .display_header_footer(false)
                .print_background(true)
                .build();
            tokio::time::timeout(phase, page.pdf(params))
                .await
                .map_err(|_| "printToPDF timed out".to_string())?
                .map_err(|e| format!("printToPDF: {e}"))
        }
        .await;
        (browser, res)
    })
    .await
}
```

**Why the executor and not just `render_single_doc`:** `taliesin render` deliberately skips
execution and warns that kernel cells "will be empty". A PDF is an artifact you hand to
someone, so shipping it with empty figures would be a far worse defect than it is for a
one-shot stdout dump. `build.rs:651-724` is the sequence being mirrored; read it before
writing this, and do not introduce a second render path.

- [ ] **Step 5: Wire the subcommand**

In `crates/server/src/main.rs`: add `mod pdf;`, the dispatch arm
`Some("pdf") => pdf::cmd_pdf(&args),` beside `build`, add `"pdf"` to `COMMANDS` (so
`closest()` suggests it), and add a `subcommand_help("pdf")` page:

```
taliesin pdf <file.tmd> [options]

  Render a typeset, paginated PDF from the built HTML.

  -o, --out <path>     output file (default: <name>.pdf)
      --paper <size>   a4 (default), letter, or a5
      --keep-html      keep the intermediate paginated HTML for inspection
```

Add the `print_pdf` test target to `crates/server/Cargo.toml` beside the others:

```toml
[[test]]
name = "print_pdf"
required-features = ["headless-js"]
```

- [ ] **Step 6: Run the live gate**

Run: `TALIESIN_REQUIRE_CHROME=1 cargo test -p taliesin-server --features headless-js --test print_pdf -- --nocapture`
Expected: PASS. Then eyeball it for real:

```bash
cargo run -p taliesin-server --features headless-js -- pdf corpus/posts/em-algorithm/index.tmd --keep-html
pdfinfo corpus/posts/em-algorithm/index.pdf | head -6
pdftotext -layout corpus/posts/em-algorithm/index.pdf - | head -40
```

Expected: more than one page, real prose. **If the page count is 1 or the text is empty, stop
— that is the CLI-truncation failure mode reappearing**, and it means the completion wait is
not actually gating the print. Do not paper over it with a sleep.

- [ ] **Step 7: Verify by mutation**

Delete the `if !done { return Err(...) }` guard and confirm the gate goes RED (a truncated or
tiny PDF), not green. Restore by inverse edit.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/pdf.rs crates/server/src/main.rs crates/server/src/headless_js.rs \
        crates/server/Cargo.toml crates/server/tests/print_pdf.rs
git commit -m "feat(print): taliesin pdf via CDP, waiting on paged.js completion (159)"
```

---

### Task 4: Running heads, folios, and the base.css conflict

**Files:**
- Modify: `crates/core/assets/css/print.css`
- Test: `crates/server/tests/print_pdf.rs` (extend)

**Interfaces:**
- Consumes: the working driver from Task 3.
- Produces: no new Rust API. CSS only.

**Known hazard, resolve it here:** `base.css:1091-1127` already has an `@media print` block
that sets `body { max-width: none !important; margin: 0 !important; }` and rewrites the TOC.
Those rules apply at `printToPDF` time and can fight paged.js's page boxes. This task owns
resolving that conflict, and `print.css` is inlined *after* `base.css`, so it can win.

- [ ] **Step 1: Write the failing test**

Add to `crates/server/tests/print_pdf.rs`:

```rust
/// Running heads and folios are the two things a printed document is judged on. Both come
/// from paged.js, not Chrome (Chrome 150 renders `string-set` as nothing), so this asserts
/// the polyfill is genuinely driving the margin boxes.
#[test]
fn the_pdf_carries_running_heads_and_folios() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let dir = std::env::temp_dir().join(format!("tali-pdfhead-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("doc.tmd");
    let mut body = String::from("---\ntitle: Heads\n---\n\n## Chapter Alpha\n\n");
    for i in 0..90 {
        body.push_str(&format!("Alpha paragraph {i} carrying enough words to fill a line.\n\n"));
    }
    body.push_str("## Chapter Beta\n\n");
    for i in 0..90 {
        body.push_str(&format!("Beta paragraph {i} carrying enough words to fill a line.\n\n"));
    }
    std::fs::write(&src, body).expect("write source");
    let out = dir.join("doc.pdf");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["pdf", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .status()
        .expect("run taliesin pdf");
    assert!(status.success());

    let text = std::process::Command::new("pdftotext")
        .args(["-layout", out.to_str().unwrap(), "-"])
        .output()
        .expect("pdftotext (poppler-utils) must be installed for this gate");
    let text = String::from_utf8_lossy(&text.stdout);

    // A running head repeats the section title on pages that are NOT the page the heading
    // opened on, so the title must appear strictly more often than once per section.
    assert!(
        text.matches("Chapter Alpha").count() > 1,
        "running head missing: 'Chapter Alpha' appeared {} time(s)\n{text}",
        text.matches("Chapter Alpha").count()
    );
    // Folios: page 2 and 3 must be numbered.
    assert!(text.contains("\n") && text.contains("2"), "no folio digits found");
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `TALIESIN_REQUIRE_CHROME=1 cargo test -p taliesin-server --features headless-js --test print_pdf the_pdf_carries_running_heads -- --nocapture`
Expected: FAIL — "running head missing: 'Chapter Alpha' appeared 1 time(s)".

- [ ] **Step 3: Implement the CSS**

Replace `crates/core/assets/css/print.css`:

```css
/* Paged-media sheet for the `taliesin pdf` track (backlog 159). Inlined ONLY into the
   transient print page assembled by render/print.rs — never onto a normal built page.

   Chrome 150 implements `@page` margin boxes and `counter(page)` but NOT `string-set` or
   `target-counter()` (measured 2026-07-31). Everything here that depends on those two is
   supplied by the vendored paged.js polyfill. */

@page {
  size: __TALI_PAPER__;
  margin: 22mm 20mm;

  @top-center {
    content: string(tali-section);
    font-size: 9pt;
    font-style: italic;
    color: #555;
  }
  @bottom-center {
    content: counter(page);
    font-size: 9pt;
    color: #555;
  }
}

/* The opening page is a title page: a running head naming a section that has not started
   yet, and a folio on page 1, both read as mistakes in a typeset document. */
@page :first {
  @top-center { content: none; }
  @bottom-center { content: none; }
}

/* The running-head source. `content(text)` takes the heading's text without its markup,
   so an `<h2>` carrying a numbered-section span does not leak tags into the margin box. */
h2 { string-set: tali-section content(text); }

/* base.css's own `@media print` block (base.css:1091-1127) predates pagination and assumes
   a single continuous flow: it forces `body { max-width: none; margin: 0 }` and lifts the
   TOC. Inside paged.js those rules apply to the page CONTAINER, not the page, which
   letterboxes the text. print.css is inlined after base.css, so it wins on source order at
   equal specificity — but base.css uses `!important`, so these must too. */
.pagedjs_page .pagedjs_page_content > div {
  max-width: none !important;
  margin: 0 !important;
}

/* The live reader chrome has no meaning on paper and paged.js would otherwise paginate it
   as content. base.css hides most of it under `@media print`; these are the ones that
   survive because paged.js lays out in SCREEN context, where those rules do not apply. */
#tali-controls,
#tali-diagnostics,
#tali-toc-handle,
#tali-toc-backdrop,
.tali-copy,
.tali-readbar,
.tali-resume,
.tali-rmenu-toggle,
.tali-rmenu-panel,
.tali-repro {
  display: none !important;
}
```

**The last block is the subtle one.** paged.js paginates in *screen* context, so `@media
print` rules in `base.css` are NOT active while it chunks. Anything hidden only by
`@media print` will be laid out as real content and consume page space, then vanish at print
time, leaving gaps. Hiding it unconditionally in `print.css` is what prevents that.

- [ ] **Step 4: Run tests to verify they pass**

Run: `TALIESIN_REQUIRE_CHROME=1 cargo test -p taliesin-server --features headless-js --test print_pdf -- --nocapture`
Expected: PASS, both tests.

- [ ] **Step 5: Look at it**

```bash
cargo run -p taliesin-server --features headless-js -- pdf corpus/posts/em-algorithm/index.tmd
pdftotext -layout corpus/posts/em-algorithm/index.pdf - | head -60
```

Check by eye: no blank gaps where hidden chrome used to be, no letterboxed text, running head
on pages 2+, no folio on page 1.

- [ ] **Step 6: Verify by mutation**

Delete the `h2 { string-set: … }` rule, confirm `the_pdf_carries_running_heads_and_folios`
goes RED, restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/core/assets/css/print.css crates/server/tests/print_pdf.rs
git commit -m "feat(print): running heads, folios, and base.css print-block reconciliation (159)"
```

---

### Task 5: Page cross-references and the list of figures

**Files:**
- Modify: `crates/core/assets/css/print.css`
- Modify: `crates/core/src/render/print.rs`
- Test: `crates/core/tests/print_page.rs`, `crates/server/tests/print_pdf.rs`

**Interfaces:**
- Consumes: `print_page_from_doc` (Task 2).
- Produces: `fn list_of_figures(body: &str) -> String` (private to `print.rs`), injected ahead
  of the body when the document has at least one figure.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/tests/print_page.rs`:

```rust
#[test]
fn a_document_with_figures_gets_a_generated_list_of_figures() {
    let body = "---\ntitle: T\n---\n\n![A caption](a.png){#fig-a}\n\n![B caption](b.png){#fig-b}\n";
    let html = print_page_from_doc(&doc_with_body(body), "f", Paper::A4);
    assert!(
        html.contains("<nav class=\"tali-lof\""),
        "a doc with figures must get a list-of-figures nav"
    );
    assert!(
        html.contains("href=\"#fig-a\"") && html.contains("href=\"#fig-b\""),
        "every figure must be listed, in order"
    );
    let a = html.find("#fig-a").expect("fig-a listed");
    let b = html.find("#fig-b").expect("fig-b listed");
    assert!(a < b, "the list must follow document order");
}

#[test]
fn a_document_without_figures_gets_no_empty_list_of_figures() {
    let html = print_page_from_doc(&doc_with_body("# Hi\n\njust text\n"), "f", Paper::A4);
    assert!(
        !html.contains("tali-lof"),
        "an empty list-of-figures heading on a figureless document is a defect"
    );
}
```

Add to `crates/server/tests/print_pdf.rs`:

```rust
/// The headline feature: a cross-reference that resolves to a real page number. Chrome 150
/// renders `target-counter()` as nothing, so a non-zero page number here is proof the
/// polyfill resolved it.
#[test]
fn a_cross_reference_resolves_to_a_real_page_number() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let dir = std::env::temp_dir().join(format!("tali-pdfxref-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("doc.tmd");
    let mut body = String::from("---\ntitle: Refs\n---\n\nSee @fig-late for the result.\n\n");
    for i in 0..120 {
        body.push_str(&format!("Filler paragraph {i} with a reasonable number of words.\n\n"));
    }
    body.push_str("![The late figure](late.png){#fig-late}\n");
    std::fs::write(&src, body).expect("write source");
    let out = dir.join("doc.pdf");
    assert!(
        std::process::Command::new(env!("CARGO_BIN_EXE_taliesin"))
            .args(["pdf", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
            .status().expect("run").success()
    );
    let text = std::process::Command::new("pdftotext")
        .args(["-layout", out.to_str().unwrap(), "-"])
        .output().expect("pdftotext");
    let text = String::from_utf8_lossy(&text.stdout);

    let re_hit = text.contains("(p. ");
    assert!(re_hit, "no '(p. N)' suffix rendered on a cross-reference\n{text}");
    // "(p. 0)" is the signature of target-counter firing before pagination settled — a
    // silent wrong answer, which is worse than no answer.
    assert!(
        !text.contains("(p. 0)"),
        "cross-reference resolved to page 0, i.e. pagination had not settled\n{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-core --test print_page`
Expected: FAIL — no `tali-lof` in output.

- [ ] **Step 3: Implement the LoF generator**

In `crates/core/src/render/print.rs`, add:

```rust
/// Build a list of figures from the assembled body, in document order.
///
/// Scans for `<figure … id="fig-…">` and pairs each with its `<figcaption>` text. Returns an
/// empty string when the document has no figures — an empty "List of Figures" heading is a
/// defect, not a degenerate case.
///
/// This is a GENERATED block. It exists only in the transient print page, so it is
/// structurally absent from `read`/`skim`, the search index and `llms-full.txt`; Task 7 pins
/// that rather than assuming it.
fn list_of_figures(body: &str) -> String {
    let mut items = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<figure") {
        let after = &rest[start..];
        let Some(end) = after.find("</figure>") else { break };
        let block = &after[..end];
        rest = &after[end..];
        let Some(id) = attr_value(block, "id=\"") else { continue };
        if !id.starts_with("fig-") {
            continue;
        }
        let caption = block
            .find("<figcaption")
            .and_then(|c| block[c..].find('>').map(|g| c + g + 1))
            .and_then(|open| {
                block[open..]
                    .find("</figcaption>")
                    .map(|close| super::strip_tags(&block[open..open + close]))
            })
            .unwrap_or_default();
        items.push_str(&format!(
            "<li><a href=\"#{id}\">{}</a></li>",
            caption.trim()
        ));
    }
    if items.is_empty() {
        return String::new();
    }
    format!("<nav class=\"tali-lof\" role=\"doc-loft\"><h2>List of Figures</h2><ol>{items}</ol></nav>")
}

/// The value of `name` in `block`, or `None`. `name` includes the opening quote.
fn attr_value<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let i = block.find(name)? + name.len();
    let j = block[i..].find('"')? + i;
    Some(&block[i..j])
}
```

Then in `print_page_from_body`, prepend it to the body:

```rust
let lof = list_of_figures(body);
let body = format!("{lof}{body}");
```

(Adjust the `PageParts { body: &body, … }` reference accordingly.)

- [ ] **Step 4: Add the CSS**

Append to `crates/core/assets/css/print.css`:

```css
/* The headline of the print track: a cross-reference that names its page. `target-counter`
   is a paged.js feature — Chrome 150 renders it as nothing. */
a.tali-xref::after {
  content: " (p. " target-counter(attr(href url), page) ")";
  font-size: .9em;
  color: #555;
}

/* The generated list of figures. */
.tali-lof { break-after: page; }
.tali-lof ol { list-style: none; padding: 0; }
.tali-lof li { margin: .25em 0; }
.tali-lof li a::after {
  content: " . . . . " target-counter(attr(href url), page);
  color: #555;
}
```

Note on `leader('.')`: it reads better than literal dots, but was not verified against
paged.js 0.4.3 in the spec's measurements. Ship the literal form. If you want leaders, verify
in the browser first and treat it as polish, not a requirement.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --test print_page`
Then: `TALIESIN_REQUIRE_CHROME=1 cargo test -p taliesin-server --features headless-js --test print_pdf -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Verify by mutation**

Delete the `a.tali-xref::after` rule, confirm `a_cross_reference_resolves_to_a_real_page_number`
goes RED. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/print.rs crates/core/assets/css/print.css \
        crates/core/tests/print_page.rs crates/server/tests/print_pdf.rs
git commit -m "feat(print): target-counter page refs + generated list of figures (159)"
```

---

### Task 6: Widows, orphans, hyphenation, and media degradation

**Files:**
- Modify: `crates/core/assets/css/print.css`
- Test: `crates/core/tests/print_page.rs`

**Interfaces:**
- Consumes: Task 5's `print.css`.
- Produces: no new API.

- [ ] **Step 1: Write the failing test**

Add to `crates/core/tests/print_page.rs`:

```rust
#[test]
fn the_print_sheet_sets_widow_orphan_and_hyphenation_policy() {
    let html = print_page_from_doc(&doc_with_body("# Hi\n\ntext\n"), "f", Paper::A4);
    for needle in ["orphans: 3", "widows: 3", "hyphens: auto"] {
        assert!(
            html.contains(needle),
            "print.css must set `{needle}` — a paged document without it breaks paragraphs badly"
        );
    }
}

/// The document's own `lang` must reach `<html lang>`, because `hyphens: auto` silently does
/// nothing without it: the browser needs to know which dictionary to use. A hyphenation rule
/// with no lang is a rule that never fires.
#[test]
fn the_document_language_reaches_the_html_element() {
    let mut d = doc_with_body("# Hi\n");
    d.lang = Some("fi".into());
    let html = print_page_from_doc(&d, "f", Paper::A4);
    assert!(html.contains("lang=\"fi\""), "front-matter lang must reach <html lang>");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --test print_page the_print_sheet_sets_widow`
Expected: FAIL — needle not found.

- [ ] **Step 3: Implement**

Append to `crates/core/assets/css/print.css`:

```css
/* Paragraph breaking. Three lines is the conventional floor; two still reads as a stray. */
p, li, blockquote {
  orphans: 3;
  widows: 3;
}

/* Hyphenation needs `<html lang>` to pick a dictionary; print.rs threads the document's
   front-matter `lang:` through, defaulting to `en`. Without the lang this rule is inert. */
body {
  hyphens: auto;
  -webkit-hyphens: auto;
  text-align: left;
}

/* A heading stranded at the foot of a page with its section overleaf. */
h1, h2, h3, h4, h5, h6 { break-after: avoid; }
h1, h2 { break-before: page; }

/* Media degradation. `{js}` cells print LIVE: CDP genuinely executes the page, so Plot,
   GLSL and numerics cells have painted real <svg>/<canvas> before printToPDF. Only video
   needs a fallback, and Chrome renders a <video>'s poster frame natively in print. */
figure, table, pre, .tali-eqn, .callout { break-inside: avoid; }
video { max-width: 100%; }

/* An interactive control that cannot be operated on paper should not look operable. */
button, input, select { display: none !important; }
```

**Note on `h1, h2 { break-before: page }`:** this makes every top-level section start a new
page, which is right for a book and can be heavy-handed for a short post. Verify against the
corpus pin in Task 7 and drop `h2` from the rule if the output is airy.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --test print_page`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/css/print.css crates/core/tests/print_page.rs
git commit -m "feat(print): widow/orphan control, hyphenation, media degradation (159)"
```

---

### Task 7: The corpus pin, the projection sweep, and docs

**Files:**
- Create: `corpus/print/paged.tmd`
- Test: `crates/core/tests/print_page.rs` (extend)
- Modify: `docs/guide/using/formats.tmd` (or the nearest "outputs" page — grep for where
  `build` is documented)
- Modify: `notes/backlog.md` (delete item 159)

**Interfaces:**
- Consumes: everything above.
- Produces: the corpus pin that makes this feature part of the regression net.

- [ ] **Step 1: Write the pin**

Create `corpus/print/paged.tmd`. Keep it small — the walker renders every corpus doc on every
`cargo test`, and the standing rule is not to grow `corpus/` past what the feature needs:

```markdown
---
title: Paged output
lang: en
---

## Why this document exists

This is the corpus pin for the print/PDF track (backlog 159). It carries the shapes the
paged rendering is judged on: a cross-reference that must resolve to a page number, more
than one figure so the list of figures is not degenerate, and enough prose to force a real
page break.

The PDF is a *rendering of this HTML*. There is no second compiler path.

## Figures and references

The scree plot is @fig-scree; the reconstruction is @fig-recon. A reference to a figure
further down the document is the case that proves `target-counter()` resolved after
pagination rather than before it.

![A scree plot of the component variances.](scree.png){#fig-scree}

## A section that spans a break

Enough prose follows to push the second figure onto a later page, so that @fig-recon
resolves to a page number that is not the page this sentence is on.

![The reconstruction at four ranks.](recon.png){#fig-recon}
```

Add the two referenced images, or switch the figures to `{mermaid}` cells if adding binaries
to the corpus is unwelcome — check `corpus/media/gallery.tmd` for the established convention
and follow it rather than inventing one.

- [ ] **Step 2: Write the projection-exclusion test**

The reader-affordances batch found a generated block leaking into **four** text projections
across three modules. The LoF is a generated block and owes the same sweep.

Add to `crates/core/tests/print_page.rs`:

```rust
/// A generated block owes a projection sweep: `taliesin read`/`skim`, the search index and
/// `llms-full.txt` must not carry it. The list of figures should be excluded STRUCTURALLY
/// (it exists only in the transient print page, which none of those projections read) — this
/// test pins that rather than trusting it, because the same assumption failed four times in
/// the reader-affordances batch.
#[test]
fn the_generated_list_of_figures_is_absent_from_the_normal_page() {
    let body = "---\ntitle: T\n---\n\n![A caption](a.png){#fig-a}\n";
    let doc = doc_with_body(body);
    let normal = taliesin_core::render::render_doc_to_page(
        &doc,
        "fallback",
        taliesin_core::render::OutputMode::Build,
    );
    assert!(
        !normal.contains("tali-lof"),
        "the print-only list of figures leaked onto the normal built page"
    );
    assert!(
        !normal.contains("pagedjs"),
        "the print-only polyfill leaked onto the normal built page"
    );
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p taliesin-core`
Expected: PASS, including the corpus walker picking up the new pin.

- [ ] **Step 4: Document it**

Add a short section to the user guide page that documents outputs, next to `build`:

```markdown
## PDF

`taliesin pdf <file.tmd>` renders a typeset, paginated PDF **from the built HTML** — the same
HTML the preview serves, laid out into pages. You get running heads, folios, cross-references
that name their page ("Figure 3 (p. 12)") and an automatic list of figures.

    taliesin pdf paper.tmd                    # → paper.pdf, A4
    taliesin pdf paper.tmd --paper letter     # US Letter
    taliesin pdf paper.tmd --keep-html        # keep the paginated HTML to inspect

It needs a local Chrome, which does the page layout. `{js}` cells print live — they have
genuinely run by the time the page is captured.
```

- [ ] **Step 5: Run every gate**

```bash
./tools/gates.sh
```

Expected: green, with all four `TALIESIN_REQUIRE_*` gates armed. Add `print_pdf` to the
by-name assertions in `tools/gates.sh` alongside `read_run_js`, so a silently skipped print
gate cannot read as green — that is the entire point of that script.

- [ ] **Step 6: Close the backlog item**

Delete item **159** from `notes/backlog.md` entirely (the standing rule is to delete, never
leave a `[x]`), and update the "Now" section's "top of the P1 queue" line to name whatever is
now on top.

- [ ] **Step 7: Commit**

```bash
git add corpus/print/paged.tmd crates/core/tests/print_page.rs docs/ notes/backlog.md tools/gates.sh
git commit -m "feat(print): corpus pin, projection sweep, docs; close 159"
```

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: vendoring + provenance → Task 1;
`print.rs` sibling assembler + `--paper` → Task 2; `pdf.rs` CDP driver + feature degradation +
`--keep-html` → Task 3; running heads + folios + the `base.css` conflict → Task 4;
`target-counter` refs + LoF → Task 5; widows/orphans/hyphenation + media degradation → Task 6;
corpus pin + projection sweep + docs + gates.sh → Task 7. The spec's five test layers are all
present. Out-of-scope items (index, book concatenation, publishing a PDF beside a site, decks)
appear in no task, correctly.

**Type consistency.** `Paper` / `Paper::parse` / `Paper::css_size` / `print_page_from_doc` /
`PAGED_DONE_ATTR` are defined in Task 2 and used under those exact names in Tasks 3, 5 and 6.
`with_browser` is defined in Task 3 Step 3 and consumed in Task 3 Step 4. `list_of_figures`
and `attr_value` are defined and used within Task 5.

**Three defects found and fixed during this review** (recorded because each would have cost an
implementer a debugging cycle):

1. `RenderedDoc` derives only `Debug, Clone` — **not `Default`** — so the original test helper
   `RenderedDoc::default()` would not have compiled. Fixed to build the doc through
   `taliesin_core::render_single_doc(src, Path::new("."))`, the way the product does.
2. The driver originally called an invented `crate::query::render_doc_for`. Resolved against
   the tree to the real single-file sequence.
3. **The bigger one:** that path was going to be `render`'s, which *does not execute code
   cells*. A PDF of a data-analysis paper would have shipped with every python/r figure empty.
   Task 3 now mirrors `build.rs:651-724` — render, executor with `_freeze`, resolved
   interpreters, `ex.run(...)`, then the print assembly. A side benefit worth knowing: an
   already-built document replays from `_freeze` and never boots a kernel.

**One known-soft spot, flagged rather than hidden.** Task 3 Step 3 contains a `todo!()` on the
`with_browser` extraction. This is deliberate and bounded: the body is an existing block in
`headless_js.rs` that must move *verbatim*, and transcribing it here would invite a subtly
different copy that silently drops one of the timeout bounds those lines exist to provide (the
L3-1 fix). The instruction is "move the existing body, replace one call, keep every teardown
path" — the implementer must read the real function. If the extraction proves awkward, the
acceptable fallback is to leave `observe_inner` untouched and give `pdf.rs` its own launch, at
the cost of a second copy of the policy.

**Sequencing note.** Task 3 is deliberately the third task, not the last: it is the only step
whose feasibility the spec could not verify in advance. If the CDP wait does not hold, that
must reshape Tasks 4-6 rather than invalidate them after they are written.
