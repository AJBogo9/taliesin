//! Parse `.tmd` source with comrak (sourcepos-aware) and emit our own HTML.
//!
//! We deliberately do not use comrak's built-in HTML formatter: every
//! top-level AST node is treated as a "block" and gets its own root element
//! carrying `data-block-id` and `data-sourcepos`, which the dev server later
//! keys off for incremental block-swap and click-to-source.

use crate::includes::LineOrigin;
use comrak::nodes::{AstNode, ListType, NodeList, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod model;
pub(crate) use model::CellRole;
pub use model::{
    AssetMode, Block, Cell, CellFigure, CellTable, ExternalAssets, JsOpts, OutputMode,
    PageIncludes, RenderedDoc, Severity, SiteDefaults, Warning,
};

fn parse_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    // Parse `$...$` (inline) and `$$...$$` (display) into Math nodes for KaTeX.
    options.extension.math_dollars = true;
    // `[^1]` references + `[^1]: …` definitions; comrak moves definitions to the
    // document end in reference order, which we gather into a footnotes section.
    options.extension.footnotes = true;
    // Smart typography (curly quotes, en/em dashes) to match Pandoc output.
    options.parse.smart = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
/// Does not resolve `{{< include >}}` (use [`render_document_with_includes`]).
mod doc_includes;
pub use doc_includes::includes_from_parts;
mod fm_extract;
pub(crate) use fm_extract::bibliography_paths;
pub(crate) use fm_extract::emits_title_block; // also used by site/xref.rs's numbering scan
use fm_extract::{detect_title_block_hidden, detect_toc, extract_field};
mod cell_extract;
pub use cell_extract::option_directive;
use cell_extract::{
    cell_flag_or, cell_option, code_fold, code_lang, detect_execute_cache, hidden_cell,
    is_executable_fence, parse_js_opts, slice_lines, strip_cell_options,
};
mod cell_numbered;
pub(crate) use cell_numbered::numbered_caption;
use cell_numbered::{FloatLabel, emit_client_cell, emit_client_figure, emit_code_listing};
mod client_lang;
pub use client_lang::{
    ClientLang, client_lang, client_lang_runnable, has_client_cells, has_client_cells_of,
};
// `pub(crate)` only so `frontmatter` can reach `extension::dataset::DATASET_KEYS`: the
// front-matter linter validates `datasets:` sub-keys against the same closed list the
// renderer reads, rather than a second copy that could drift from it.
mod divs;
pub(crate) mod extension;
mod validate;
pub(crate) use divs::parse_attrs;
pub use divs::{CELL_OUT_SLOT_ATTR, tokenize_attrs};
use divs::{group_divs, parse_pandoc_attrs, preprocess, scan_div_spans};

// Re-exported for the editor vocabulary (crate::vocab), which sources completion
// vocabulary from the SAME consts the validator enforces so the two cannot drift.
pub(crate) use validate::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES};
// The IMPLEMENTED div classes. The validator uses this directly; outside `render` its only
// reader is `vocab.rs`'s drift test, which pins the OFFERED subset (`vocab::DIV_CLASS_NAMES`,
// several classes shorter) as a subset of it so a class the editor suggests always gets a
// did-you-mean when it is typo'd. `#[cfg(test)]` because that is now the whole of it.
#[cfg(test)]
pub(crate) use validate::DIV_FEATURE_CLASSES;

mod emit;
use emit::emit;
// emit_children is re-exported so the sibling figure module reaches it via `super`.
pub(crate) use emit::emit_children;
pub(crate) use emit::safe_url;
mod figure;
use figure::{emit_figure, emit_mermaid_figure, figure_parts};
// Intrinsic `width`/`height` + loading hints on local raster images. A post-emission pass
// over each block's HTML (like `shift_heading_html` below), because neither `<img>` emitter
// can reach a `base_dir`.
mod image_meta;
use image_meta::ImageAnnotator;
// The reader's code download (C-READ-2): every cell's source, in the order it ran, as one
// `data:` URL per language. Reads the BLOCK MODEL rather than the emitted HTML, because a
// `#| echo: false` cell renders no listing and must still be in the script.
mod repro;
// The id four consumers must agree to skip when turning a page into text.
pub(crate) use repro::REPRO_BLOCK_ID;
// Text projection (`taliesin read`): a plain-text VIEW of the block model, not an output
// format. Crate-internal; reached via `RenderedDoc::body_text()`.
mod text;
// The search index's text extraction, shared with the `read`/TOC/slug path above rather
// than re-derived in `site/` (where a weaker copy silently indexed KaTeX three times).
pub(crate) use text::indexable_text;
mod theme;
// Used only by the page builders; kept crate-internal, not part of the public API.
pub(crate) use theme::theme_head;
mod page;
use page::page_from_doc;
pub use page::{
    PageParts, SiteCtx, assemble_html_page, favicon_link, html_page_from_doc_in_site,
    html_page_from_doc_in_site_external, render_doc_to_page, render_doc_to_page_external,
    title_with_site_suffix,
};
// Crate-internal: `Site::page_title` is the entry point for resolving a page's tab title.
pub(crate) use page::site_page_title;
use theme::{detect_theme, resolve_theme, theme_default_mode, theme_style};

/// Render a `.tmd` source string into the `RenderedDoc` block model: the parse
/// step only (no code execution, no page chrome). The dev server diffs these
/// block lists for incremental updates; the CLI wraps the result in a page.
///
/// `title` is the front-matter `title:` only. A leading `# H1` titles the *page* (see
/// [`render_doc_to_page`]) but is not folded in here, so a site can still prefer an
/// authored `_site.yml` chapter title over the heading text.
///
/// ```
/// let doc = taliesin_core::render_document("# Title\n\nHello *world*.\n");
/// assert_eq!(doc.title, None); // no front-matter title
/// assert_eq!(doc.blocks.len(), 2); // the heading + the paragraph
/// assert!(doc.blocks[0].html.contains("<h1"));
/// ```
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src.to_owned(), None, None, None, None, None)
}

/// Whether `--no-exec` / `TALIESIN_NO_EXEC` is in force: the user asked that this
/// document's code cells not run.
///
/// Read in `crates/core` — not only in the server's executor — because a `{js}` cell is a
/// **code cell whose runtime is the browser** rather than a kernel. Before this, `--no-exec`
/// stopped `{python}` and emitted `{js}` unchanged while
/// `docs/guide/reference/cli.tmd` called the flag a way to "preview untrusted docs safely"
/// (item 79, measured: `crates/core` contained zero references to the variable). A flag
/// named "no exec" that runs half the document's code is worse than no flag.
///
/// An env read in the render path follows the two already here (`TALIESIN_RENDER_TIMEOUT`,
/// `TALIESIN_MERMAID_URL`), and the flag is process-wide by construction — `cli.rs` sets the
/// variable — so there is nothing per-document to thread through the render entry points.
///
/// **Deliberately not extended** to raw `<script>` passthrough, `include-in-header` or
/// `css:`. Removing author-written HTML is a sanitizer, which this project ruled out
/// (2026-07-03 catalog: no CSP, no sanitizer, no cell sandbox). Those channels are
/// *documented* instead, in the same place this flag is. `mermaid` is likewise untouched: a
/// diagram is a declarative description, not the document's program.
pub fn no_exec_in_force() -> bool {
    std::env::var_os("TALIESIN_NO_EXEC").is_some()
}

/// The languages Taliesin executes against a warm kernel, whose *output block* can
/// therefore carry a figure/table anchor. This is the canonical set: the render pass
/// reserves a `@fig-`/`@tbl-` number only for a lang that will actually produce the
/// float, and `taliesin-server`'s `exec::kernel_lang` (which does the running) is
/// drift-locked to it by a test. A lang that is neither executed here nor emitted at
/// render time (mermaid/`{js}`) — `{bash}`, `{sql}`, `{julia}`, … — produces no float,
/// so labelling one as a figure/table must NOT burn a number or register a phantom
/// anchor.
pub fn executes_to_kernel(lang: &str) -> bool {
    lang == "python"
}

/// Like [`render_document`], but first expands `{{< include >}}` shortcodes
/// relative to `base_dir`, mapping each block back to its origin file, and
/// resolves citations/cross-references against the doc's bibliography.
pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    render_document_with_includes_scoped(src, base_dir, None)
}

/// Like [`render_document_with_includes`] but with an optional book chapter number, so a
/// numbered chapter renders "Figure 2.3" / "Table 2.1". Only the site book path passes
/// `Some(n)`; everything else is `None` (continuous numbering).
pub fn render_document_with_includes_scoped(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
) -> RenderedDoc {
    render_doc_with_includes_impl(src, base_dir, chapter, None, None)
}

/// Like [`render_document_with_includes_scoped`] but carrying what the page inherits from
/// its project's `_site.yml` ([`SiteDefaults`]): the project-wide `bibliography:` laid under
/// the page's own. Everything else passes `None` and is byte-identical to
/// [`render_document_with_includes_scoped`]. Public so the server's site build + live
/// preview render each page with the project's policies.
pub fn render_document_scoped_with_site(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
    site: Option<&SiteDefaults>,
) -> RenderedDoc {
    render_doc_with_includes_impl(src, base_dir, chapter, None, site)
}

/// Render one **invoked** document: `build`, `preview`, `check`, `read`, `map` or the LSP
/// handed a single `.tmd` rather than a project directory.
///
/// The one place the single-document containment root is decided (see
/// [`crate::includes::single_doc_root`]). It replaced a `..._rooted(src, base,
/// Some(base))` written out at twelve call sites, which is one policy copied twelve
/// times: the site build widened a page's root to its project and the single-file build
/// did not, so one source built into two different documents (PP-3, 2026-07-26). The
/// hand-rooted entry point was removed with them rather than left as a thirteenth way to
/// answer the question. Route a new single-document command through here.
pub fn render_single_doc(src: &str, base_dir: &Path) -> RenderedDoc {
    let root = crate::includes::single_doc_root(base_dir);
    // A page of a project inherits its project-wide `bibliography:` even when invoked on its
    // own, so `preview post.tmd` and `preview <dir>` render the same document (see
    // `site::shared_for_single_doc`). It is the only thing a page inherits from its
    // project, so there is no other project state to reproduce here.
    let site = SiteDefaults {
        bibliography: crate::site::shared_for_single_doc(&root),
    };
    render_doc_with_includes_impl(src, base_dir, None, Some(&root), Some(&site))
}

fn render_doc_with_includes_impl(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
    root: Option<&Path>,
    site: Option<&SiteDefaults>,
) -> RenderedDoc {
    let (expanded, origins, include_warnings) =
        crate::includes::resolve_warned_in(src, base_dir, root);
    // Declarative shortcodes (`{{< embed >}}` / `{{< video >}}` / `{{< input >}}`)
    // expand after includes, line-preserving so `origins` stays valid. A `{{< name >}}`
    // that no built-in declares is left verbatim but reported, so a typo'd shortcode
    // doesn't ship silently as literal text.
    let (expanded, shortcode_warnings) = extension::expand_shortcodes(&expanded);
    // Hands `expanded`/`origins` over rather than copying them: the watchdog needs owned
    // inputs, and this path already owns both.
    let mut doc = render_internal(
        expanded,
        Some(origins),
        Some(base_dir.to_path_buf()),
        root.map(Path::to_path_buf),
        chapter,
        site.cloned(),
    );
    // An include that couldn't be expanded (unsafe path, cycle, unreadable) leaves
    // its `{{< include … >}}` directive literal in the output; surface it as a
    // located, click-to-source diagnostic on the same channel as broken refs so it
    // shows in build/preview/`check` instead of shipping silently.
    doc.warnings.extend(include_warnings.into_iter().map(|iw| {
        // `iw.line` is always >= 1 (constructed as `idx + 1` in includes.rs), so the
        // warning is always located on the directive line.
        Warning::new(format!(
            "include not resolved ({}): {{{{< include {} >}}}}",
            iw.reason, iw.target
        ))
        .at(iw.file, iw.line as u32)
    }));
    doc.warnings.extend(shortcode_warnings);
    doc
}

/// The deepest block-container nesting Taliesin will parse ([`overlong_nesting`]).
///
/// A bigger stack only moves the cliff, it does not remove it: AP2 measured the 256 MB
/// render stack aborting between 65k and 70k `>` levels in debug and around 900k in release
/// (a ~900 KB line). Past the cliff the outcome is maximal and *uncatchable* — a stack
/// overflow `abort()`s the process, so it sails through every `catch_unwind`, including the
/// per-page site guard whose whole job is to stop one bad page killing a multi-page build.
/// Bounding the depth before the parse converts that abort into a located diagnostic.
///
/// 1000 is ~100x the deepest nesting in the corpus and 65x below the *lowest* measured
/// cliff, so it fires only on input that was never going to render.
pub const MAX_NESTING_DEPTH: usize = 1000;

/// The first line of `src` that opens more than [`MAX_NESTING_DEPTH`] block containers, as
/// (1-based line, measured depth). `None` when the document is within bounds.
///
/// Nesting is measured per line because a container is only ever *opened* by a marker run at
/// the start of a line, so the deepest line is the document's depth. Only the recursive
/// containers are counted: `>` blockquotes (AP2's worst case) and `-`/`*`/`+` list bullets,
/// which also nest one level per marker (`- - - x` is three nested lists). `:::` fenced divs
/// are deliberately NOT counted: their fences are stripped by a flat line-preserving pass
/// rather than parsed recursively, and AP2 measured 1M levels of them rendering fine.
fn overlong_nesting(src: &str) -> Option<(usize, usize)> {
    src.lines().enumerate().find_map(|(idx, line)| {
        let depth = leading_container_depth(line);
        (depth > MAX_NESTING_DEPTH).then_some((idx + 1, depth))
    })
}

/// How many nested block containers `line` opens. Walks the leading marker run, skipping the
/// whitespace CommonMark allows between markers. A bullet must be followed by a space to
/// count, which is what keeps a `---` thematic break or a `***` emphasis run from reading as
/// three nested lists.
fn leading_container_depth(line: &str) -> usize {
    let mut depth = 0usize;
    let mut rest = line.as_bytes();
    loop {
        let ws = rest
            .iter()
            .position(|b| !matches!(b, b' ' | b'\t'))
            .unwrap_or(rest.len());
        rest = &rest[ws..];
        match rest {
            [b'>', tail @ ..] => {
                depth += 1;
                rest = tail;
            }
            [b'-' | b'*' | b'+', b' ' | b'\t', tail @ ..] => {
                depth += 1;
                rest = tail;
            }
            _ => return depth,
        }
    }
}

/// How long a single render may take before it is abandoned, in seconds. Override with
/// `TALIESIN_RENDER_TIMEOUT`; `0` disables the watchdog.
///
/// Rendering is not execution: a cell may legitimately run for hours (which is why
/// execution is capped on silence, not wall-clock), but a render never does. The largest thing
/// AP1 ever measured is an 8000-block document at 647 ms in release, so 30 s is ~50x the
/// worst legitimate render and still turns AP2-2's multi-minute freeze into a bounded wait.
const DEFAULT_RENDER_TIMEOUT_SECS: u64 = 30;

/// The watchdog budget: `TALIESIN_RENDER_TIMEOUT` if it parses, else the default. `0` (or a
/// value that overflows the wait) means "no watchdog", which is what a caller measuring a
/// deliberately pathological document wants.
fn render_budget() -> Option<std::time::Duration> {
    let secs = match std::env::var("TALIESIN_RENDER_TIMEOUT") {
        Ok(v) => v.trim().parse().unwrap_or(DEFAULT_RENDER_TIMEOUT_SECS),
        Err(_) => DEFAULT_RENDER_TIMEOUT_SECS,
    };
    (secs > 0).then(|| std::time::Duration::from_secs(secs))
}

/// A well-formed but empty document carrying one located diagnostic. This is what a
/// *refused* render returns: the failure surfaces on the same click-to-source warning
/// channel as a broken ref, so the preview shows it, `check` exits non-zero, and a site
/// build loses one page instead of the whole run.
fn refused_render(warning: Warning) -> RenderedDoc {
    let mut doc = render_internal_impl("", None, None, None, None, None);
    doc.warnings.push(warning);
    doc
}

/// Core render. Runs the actual work on a worker thread with a large stack, under a
/// watchdog.
///
/// **The stack:** deeply nested input (blockquotes / lists) drives deep recursion in the
/// Markdown parser and block emission, which on the default ~8 MB stack overflows and
/// **aborts the whole process** at ~3000 levels. The big stack raises that ceiling but does
/// not remove it, so [`MAX_NESTING_DEPTH`] bounds the input below the remaining cliff.
///
/// **The watchdog:** AP2-2 measured comrak 0.52's inline reference-link matcher going
/// quadratic on balanced nested brackets (4.27 s at 128k in release, minutes by ~500k).
/// That is neither a panic nor an abort — just unbounded CPU — so no `catch_unwind` and no
/// depth guard can see it, and the warm preview loop simply freezes with no diagnostic. The
/// worker is therefore *detached* rather than scoped, so a render that blows the budget can
/// be abandoned and the caller answered with a located error. The abandoned thread keeps
/// running to completion (there is no safe way to kill a thread mid-parse); it is a bounded
/// leak on a document that was already unrenderable, and the diagnostic tells the author
/// which one. A panic is still propagated to the caller unchanged.
///
/// Takes its inputs by value because a detached thread needs `'static`. This costs one
/// `String` copy of the source on the [`render_document`] path; the include path already
/// owns both its expanded source and its origins, so it hands them over rather than cloning.
fn render_internal(
    src: String,
    origins: Option<Vec<LineOrigin>>,
    base_dir: Option<PathBuf>,
    include_root: Option<PathBuf>,
    chapter: Option<u32>,
    site: Option<SiteDefaults>,
) -> RenderedDoc {
    // Behind an `Arc` so the worker and the spawn-failure fallback can both reach it: a
    // failed `Builder::spawn` drops its closure rather than handing it back, so the inputs
    // cannot simply be moved in.
    let input = std::sync::Arc::new(RenderInput {
        src,
        origins,
        base_dir,
        include_root,
        chapter,
        site,
    });
    let big_stack = || std::thread::Builder::new().stack_size(256 * 1024 * 1024);

    let Some(budget) = render_budget() else {
        // Watchdog disabled: keep the worker *scoped*, so nothing can outlive this call.
        return std::thread::scope(|scope| {
            match big_stack().spawn_scoped(scope, || input.render()) {
                Ok(handle) => match handle.join() {
                    Ok(doc) => doc,
                    Err(payload) => std::panic::resume_unwind(payload),
                },
                Err(_) => input.render(),
            }
        });
    };

    // `sync_channel(1)` so an abandoned worker never blocks forever on its send.
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let worker = std::sync::Arc::clone(&input);
    match big_stack().spawn(move || {
        // Catch here rather than letting the thread unwind, so a panic reaches the caller
        // as a payload to re-raise instead of being misreported as a timeout.
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.render()));
        let _ = tx.send(out);
    }) {
        Ok(_detached) => match rx.recv_timeout(budget) {
            Ok(Ok(doc)) => doc,
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(_) => refused_render(Warning::new(format!(
                "render exceeded {}s and was abandoned (is this document pathological? \
                 deeply nested brackets render quadratically); \
                 raise or disable the limit with TALIESIN_RENDER_TIMEOUT",
                budget.as_secs()
            ))),
        },
        // Spawning the big-stack worker can fail under a strict address-space limit
        // (e.g. `ulimit -v`). Render inline on the current thread rather than panicking.
        Err(_) => input.render(),
    }
}

/// The owned inputs to one render, so the worker thread can be `'static` (and therefore
/// abandonable when it blows the watchdog budget).
struct RenderInput {
    src: String,
    origins: Option<Vec<LineOrigin>>,
    base_dir: Option<PathBuf>,
    include_root: Option<PathBuf>,
    chapter: Option<u32>,
    site: Option<SiteDefaults>,
}

impl RenderInput {
    fn render(&self) -> RenderedDoc {
        render_internal_impl(
            &self.src,
            self.origins.as_deref(),
            self.base_dir.as_deref(),
            self.include_root.as_deref(),
            self.chapter,
            self.site.as_ref(),
        )
    }
}

fn render_internal_impl(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
    include_root: Option<&Path>,
    chapter: Option<u32>,
    site: Option<&SiteDefaults>,
) -> RenderedDoc {
    // Bound nesting BEFORE the parse. Past the measured cliff the recursive descent
    // overflows even this thread's 256 MB stack and *aborts the process* — uncatchable, and
    // on a site build every other page dies with it. Refusing the one document with a
    // located, click-to-source diagnostic keeps the failure proportional: the rest of the
    // build survives, and the author is pointed at the line. The empty-source re-render
    // supplies a well-formed doc to hang the warning on (`""` cannot recurse: no nesting).
    if let Some((line, depth)) = overlong_nesting(src) {
        let (file, mapped) = map_origin(origins, line);
        let mut doc = render_internal_impl("", None, base_dir, include_root, chapter, site);
        doc.warnings.push(
            Warning::new(format!(
                "document nests {depth} levels deep at this line, over the {MAX_NESTING_DEPTH}-level limit; \
                 not rendered (deeper nesting overflows the render stack and would abort the build)"
            ))
            .at(file, mapped as u32),
        );
        return doc;
    }
    let arena = Arena::new();
    let options = parse_options();
    // fenced divs (`:::`) aren't CommonMark. Record their spans first,
    // then strip the fence markers in a line-preserving pass so sourcepos line
    // numbers stay exact and the inner content parses as normal blocks. The
    // recorded spans are used afterwards to wrap blocks back up as callouts etc.
    let (spans, unclosed_fences) = scan_div_spans(src);
    let processed = preprocess(src);
    let root = parse_document(&arena, &processed, &options);

    let lines: Vec<&str> = processed.lines().collect();
    let mut title: Option<String> = None;
    let mut subtitle: Option<String> = None;
    let mut date: Option<String> = None;
    let mut authors: Vec<crate::author::Author> = Vec::new();
    let mut description: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut toc_explicit: Option<bool> = None;
    // `title-block-style: none` keeps `title` (drives `<title>`, OpenGraph, nav)
    // but skips the visible `<h1>` header (nav landing pages don't need it).
    let mut hide_title_block = false;
    let mut theme: Option<String> = None;
    let mut bib_paths: Vec<String> = Vec::new();
    // Populated only by a project's `_site.yml head:` (merged in by `site::page_chrome`) and
    // by the chrome's own draft banner; a document's front matter has had no include keys
    // since the raw-injection family was retired on 2026-08-02.
    let includes = PageIncludes::default();
    // Non-fatal render warnings (a missing `bibliography:`/`theme:` file, …),
    // collected through the whole render and surfaced in the dev menu / build log.
    let mut warnings: Vec<Warning> = Vec::new();
    // Validate the document's front matter against taliesin's vocabulary (top-level
    // keys + the nested execute/listing/about/hero children); located warnings flow to
    // the dev panel as click-to-source diagnostics, the same channel as broken refs.
    warnings.extend(crate::frontmatter::validate_front_matter(src));
    // An unterminated `:::` fence drops its wrapper silently (the callout/columns never
    // form and the content renders unfenced) — surface it as a click-to-source warning.
    for open in unclosed_fences {
        let (file, mapped) = map_origin(origins, open);
        warnings.push(
            Warning::new(
                "unterminated `:::` fenced div: add a closing `:::` \u{2014} the block is \
                 rendered without its wrapper",
            )
            .at(file, mapped as u32),
        );
    }
    // The document-level `execute: cache:` default; a cell's own `#| cache` overrides it.
    // `echo`/`include` have no document-level form since 2026-08-02 — they are per-cell
    // (`#| echo:`), which is where every real document already said them.
    let mut exec_cache = true;
    // Whether this render will emit a visible title block — and therefore demote every
    // body heading one level. Read from the front matter up front (not from the in-loop
    // `title`/`format`/`hide_title_block`, which are only set once the walk has passed
    // the front-matter node) because the section-numbering base below needs it BEFORE
    // the first heading. The demotion site further down uses this same value, so the
    // two cannot drift.
    let emits_title_block_here =
        emits_title_block(crate::frontmatter::front_matter_block(src).unwrap_or(""));
    let mut flat: Vec<FlatBlock> = Vec::new();
    // Footnote definitions, resolved BEFORE the walk. comrak moves every definition to
    // the document end, so by the time the walk reaches a `[^a]` reference in an early
    // paragraph the definition node has not been visited yet — and the note has to be
    // spliced in at that reference, because that is the only place CSS can float it into
    // the margin from (owner ruling 2026-08-01: margin placement is the default). A
    // pre-pass over the same node set the walk covers is the cheapest way to have both.
    let footnote_defs: HashMap<String, FootnoteDef> = root
        .children()
        .filter_map(|node| {
            let data = node.data.borrow();
            let NodeValue::FootnoteDefinition(fd) = &data.value else {
                return None;
            };
            // Keep the definition's OWN source range: comrak has moved the node to the
            // document end, but its sourcepos still points at where the author wrote it,
            // which is the line click-to-source must land on.
            let sp = data.sourcepos;
            let (file, start_line) = map_origin(origins, sp.start.line);
            let (_, end_line) = map_origin(origins, sp.end.line);
            Some((
                fd.name.clone(),
                FootnoteDef {
                    node,
                    sourcepos: format!(
                        "{}:{}-{}:{}",
                        start_line, sp.start.column, end_line, sp.end.column
                    ),
                    source_file: file,
                    src: slice_lines(&lines, sp.start.line, sp.end.line),
                    line: start_line as u32,
                },
            ))
        })
        .collect();
    let mut id_counts: HashMap<String, u32> = HashMap::new();
    // Heading anchor slugs (deduped) and the cross-reference number registry
    // (figures + equations), both used for `@sec-x`/`@fig-x`/`@eq-x` and the TOC.
    let mut heading_slugs: HashMap<String, u32> = HashMap::new();
    let mut fig_count: usize = 0;
    let mut eq_count: usize = 0;
    // Per-DOCUMENT, not per-block: the LCP rule needs to know which image is the first one
    // on the page, so the annotator is threaded across the whole walk.
    let mut image_annotator = ImageAnnotator::new();
    let mut lst_count: usize = 0;
    let mut sec_count: usize = 0;
    // Section numbering for a book chapter, advanced over EVERY heading in document
    // order so a `{#sec-x}` registers the same number the heading visibly shows via
    // `number_chapter_headings` (they share `ChapterNumbering`). Its base is the
    // shallowest heading below the chapter's own, so the whole heading shape must be
    // known before the walk reaches the first heading: pre-scan the top-level nodes,
    // exactly the set the walk below numbers.
    let chapter_heading_levels: Vec<usize> = root
        .children()
        .filter_map(|n| match &n.data.borrow().value {
            NodeValue::Heading(h) => Some(h.level as usize),
            _ => None,
        })
        .collect();
    let mut sec_numbering = chapter.map(|ch| {
        crate::site::ChapterNumbering::new(ch, &chapter_heading_levels, emits_title_block_here)
    });
    // How far every body heading moves so the page keeps exactly one `<h1>` (the title
    // block's) with no gap under it. Shares `chapter_heading_levels` with the numbering
    // above, which is the same set the walk demotes, so the two cannot disagree about the
    // page's shape. `None` when this render emits no title block: then the document's own
    // `#` is its `<h1>` and nothing shifts.
    let heading_shift = emits_title_block_here
        .then(|| heading_shift_for(&chapter_heading_levels))
        .flatten();
    let mut xref_registry: HashMap<String, String> = HashMap::new();

    for node in root.children() {
        // A definition renders at its reference, never in place (the pre-pass above
        // already holds it). comrak has moved them all to the document end.
        if matches!(node.data.borrow().value, NodeValue::FootnoteDefinition(_)) {
            continue;
        }
        // Which notes this block displays: every `[^a]` reference under it whose
        // `ref_num` is 1. A repeat reference to the same note keeps its `<sup>` but
        // carries no content — two copies would duplicate `id="fn-a"` in the DOM and
        // silently break the anchor every other reference points at.
        let notes: Vec<(String, u32)> = node
            .descendants()
            .filter_map(|d| match &d.data.borrow().value {
                NodeValue::FootnoteReference(r) if r.ref_num == 1 => Some((r.name.clone(), r.ix)),
                _ => None,
            })
            .filter(|(name, _)| footnote_defs.contains_key(name))
            .collect();
        let (
            buf_start,
            sourcepos,
            source_file,
            block_src,
            is_paragraph,
            heading_level,
            mut cell,
            cell_role,
        ) = {
            let data = node.data.borrow();
            if let NodeValue::FrontMatter(fm) = &data.value {
                title = extract_field(fm, "title");
                subtitle = extract_field(fm, "subtitle");
                date = extract_field(fm, "date");
                // The byline reads the front matter as YAML, not with `extract_field`'s
                // line scan: a structured `author:` puts the name on an INDENTED
                // sub-line, which the scan skips, so it would return None and the byline
                // would silently vanish. The scan stays the fallback for a block YAML
                // cannot parse at all, where a scalar `author: Name` is still recoverable.
                // `fm` is comrak's literal, `---` fences included, and a trailing `---`
                // opens a SECOND YAML document that `from_str::<Value>` refuses outright.
                // Parse the stripped block instead.
                let block = crate::frontmatter::front_matter_block(src).unwrap_or(fm);
                match serde_yaml::from_str::<serde_yaml::Value>(block) {
                    Ok(v) => {
                        let (list, msgs) = crate::author::parse(v.get("author"));
                        // Front matter is the head of the primary document by
                        // definition, so line 1 is the right anchor and the only one in
                        // scope here (the per-block `file`/`start_line` are bound below).
                        warnings.extend(msgs.into_iter().map(|m| Warning::new(m).at(None, 1)));
                        authors = list;
                    }
                    Err(_) => {
                        // YAML the parser refuses at all: recover what a line scan can, so a
                        // scalar `author: Name` still produces a byline.
                        authors = extract_field(fm, "author")
                            .map(crate::author::Author::named)
                            .into_iter()
                            .collect();
                    }
                }
                description = extract_field(fm, "description");
                lang = extract_field(fm, "lang");
                bib_paths = bibliography_paths(fm);
                toc_explicit = detect_toc(fm);
                hide_title_block = detect_title_block_hidden(fm);
                theme = detect_theme(fm);
                exec_cache = detect_execute_cache(fm);
                continue;
            }
            let sp = data.sourcepos;
            // Translate the buffer line range back to the originating file/line.
            let (file, start_line) = map_origin(origins, sp.start.line);
            let (_, end_line) = map_origin(origins, sp.end.line);
            let sourcepos = format!(
                "{}:{}-{}:{}",
                start_line, sp.start.column, end_line, sp.end.column
            );
            let is_paragraph = matches!(data.value, NodeValue::Paragraph);
            let heading_level = match &data.value {
                NodeValue::Heading(h) => Some(h.level),
                _ => None,
            };
            // Executable code cell: ```{lang} ... ``` (lang detected, options stripped).
            // A LEADING DOT (```{.python}) is the documented display-only form, so it is
            // NOT a cell — see `is_executable_fence`.
            let cell = match &data.value {
                NodeValue::CodeBlock(cb) if is_executable_fence(&cb.info) => code_lang(&cb.info)
                    .map(|lang| {
                        let js = parse_js_opts(&cb.literal, &lang);
                        Cell {
                            lang,
                            code: strip_cell_options(&cb.literal),
                            figure: None,
                            table: None,
                            echo: cell_flag_or(&cb.literal, "echo", true),
                            include: cell_flag_or(&cb.literal, "include", true),
                            cache: cell_flag_or(&cb.literal, "cache", exec_cache),
                            js,
                        }
                    }),
                _ => None,
            };
            // A code cell labelled/captioned as a figure (`label: fig-x` / `fig-cap`)
            // or a listing (`label: lst-x` / `lst-cap`) becomes a numbered, anchored
            // `<figure>`/listing so `@fig-x` / `@lst-x` resolve.
            let cell_role = match &data.value {
                NodeValue::CodeBlock(cb) if cell.is_some() => {
                    let label = cell_option(&cb.literal, "label");
                    let fig_cap = cell_option(&cb.literal, "fig-cap");
                    let lst_cap = cell_option(&cb.literal, "lst-cap");
                    let tbl_cap = cell_option(&cb.literal, "tbl-cap");
                    let is_fig = label.is_some_and(|l| l.starts_with("fig-")) || fig_cap.is_some();
                    let is_lst = label.is_some_and(|l| l.starts_with("lst-")) || lst_cap.is_some();
                    let is_tbl = label.is_some_and(|l| l.starts_with("tbl-")) || tbl_cap.is_some();
                    if is_fig {
                        Some(CellRole::Figure {
                            anchor: label.filter(|l| l.starts_with("fig-")).map(str::to_string),
                            caption: fig_cap.map(str::to_string),
                        })
                    } else if is_lst {
                        Some(CellRole::Listing {
                            anchor: label.filter(|l| l.starts_with("lst-")).map(str::to_string),
                            caption: lst_cap.map(str::to_string),
                            fold: code_fold(&cb.literal),
                        })
                    } else if is_tbl {
                        Some(CellRole::Table {
                            anchor: label.filter(|l| l.starts_with("tbl-")).map(str::to_string),
                            caption: tbl_cap.map(str::to_string),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            // Validate this code cell's `#|` options against taliesin's vocabulary
            // (a typo or a legacy key becomes a located, click-to-source warning;
            // the cell still renders unchanged).
            if cell.is_some()
                && let NodeValue::CodeBlock(cb) = &data.value
            {
                warnings.extend(validate::validate_cell_options(
                    &cb.literal,
                    start_line,
                    file.clone(),
                ));
            }
            (
                sp.start.line,
                sourcepos,
                file,
                slice_lines(&lines, sp.start.line, sp.end.line),
                is_paragraph,
                heading_level,
                cell,
                cell_role,
            )
        };

        // A block id is hashed from the block's SOURCE, and a note's source lives
        // somewhere else entirely (`[^a]: …`, anywhere in the document) while its text
        // renders inside THIS block. Fold the definitions this block displays into the
        // hash input, or editing a note leaves every id identical, the diff emits no op,
        // and the live preview keeps showing the old note. Only the notes this block
        // actually displays are folded in, so an edit to some other note does not churn
        // this block's id and reset the live state of a paragraph nothing changed in.
        let id = if notes.is_empty() {
            make_id(&block_src, &mut id_counts)
        } else {
            let mut keyed = block_src.clone();
            for (name, _) in &notes {
                keyed.push('\u{0}');
                keyed.push_str(&footnote_defs[name].src);
            }
            make_id(&keyed, &mut id_counts)
        };
        let file_attr = source_file_attr(source_file.as_deref());
        // A heading gets a stable, deduped anchor id.
        // A heading may carry a Pandoc attribute (`## Title {#sec-x}`): use
        // an explicit `#id` as the anchor (else a slug of the cleaned text), and
        // strip the attribute from the rendered heading below.
        let h_attr = heading_level.and_then(|_| parse_heading_attr(&block_src));
        // Advance the hierarchical section counters over EVERY heading (in a book
        // chapter), so a labelled `{#sec-x}` registers the same number its heading
        // will visibly show — even when earlier, unlabelled headings sit between them.
        // Outside a chapter there is no hierarchy: keep the flat sequential counter.
        let hierarchical_number = heading_level.and_then(|level| {
            sec_numbering
                .as_mut()
                .map(|numbering| numbering.next(level as usize))
        });
        // A heading labelled `{#sec-x}` is numbered so `@sec-x` resolves to "Section N":
        // the chapter-hierarchical number ("2.2") in a book, else a flat sequential one.
        if let Some((_, Some(id))) = &h_attr
            && id.starts_with("sec-")
        {
            let number = match &hierarchical_number {
                Some(n) => n.clone(),
                None => {
                    sec_count += 1;
                    sec_count.to_string()
                }
            };
            register_xref(
                &mut xref_registry,
                &mut warnings,
                id,
                number,
                source_file.as_deref(),
                buf_start as u32,
            );
        }
        let id_attr = match heading_level {
            Some(_) => {
                let id = match &h_attr {
                    // An explicit `{#id}` must be deduped too: two same explicit ids
                    // (or an explicit id colliding with an autoslug) would otherwise
                    // emit DUPLICATE element ids, silently breaking in-page anchors
                    // and `@sec-` refs. Route it through the SAME `heading_slugs` map
                    // the autoslug path uses (so explicit-vs-autoslug collisions are
                    // caught), and warn on the duplicate.
                    Some((_, Some(id))) => {
                        let deduped = dedup_with_suffix(id.clone(), &mut heading_slugs);
                        if &deduped != id {
                            warnings.push(
                                Warning::new(format!(
                                    "duplicate heading id \u{201c}{id}\u{201d} (using \u{201c}{deduped}\u{201d}; in-page links to \u{201c}{id}\u{201d} may not resolve)"
                                ))
                                .at(source_file.clone(), buf_start as u32)
                                .severity(Severity::Error),
                            );
                        }
                        deduped
                    }
                    Some((clean, None)) => {
                        dedup_slug(&strip_math_for_slug(clean), &mut heading_slugs)
                    }
                    None => dedup_slug(&strip_math_for_slug(&block_src), &mut heading_slugs),
                };
                format!(" id=\"{}\"", escape_attr(&id))
            }
            _ => String::new(),
        };
        // `sourcepos` MUST stay in `L:C-L:C` form: reverse cursor-sync (client.js
        // `highlightAtLine`) matches it against `^(\d+):\d+-(\d+):\d+$` and silently
        // skips any block it can't parse. Corpus-enforced by
        // `reverse_sync_sourcepos_is_total` (tests/corpus.rs). A generated block with no
        // source position uses an EMPTY sourcepos (omitted), never a degenerate one.
        let attrs =
            format!("{id_attr} data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");
        let mut html = String::new();
        // Pandoc treats a bare `\begin{env}...\end{env}` block as display
        // math even without `$$`; comrak doesn't, so detect and render it here.
        if let Some(env) = is_paragraph.then(|| bare_math_env(&block_src)).flatten() {
            html.push_str(&format!("<div{attrs} class=\"tali-math-block\">"));
            html.push_str(&crate::math::render(env, true));
            html.push_str("</div>");
        } else if let Some((latex, anchor)) = is_paragraph
            .then(|| labelled_display_eq(&block_src))
            .flatten()
        {
            // `$$ ... $$ {#eq-x}` -> a numbered display equation; register the
            // `#eq-` id so `@eq-x` cross-references resolve to "Equation N".
            eq_count += 1;
            let eq_num = float_number(chapter, eq_count);
            register_xref(
                &mut xref_registry,
                &mut warnings,
                &anchor,
                eq_num.clone(),
                source_file.as_deref(),
                buf_start as u32,
            );
            html.push_str(&emit_equation(&latex, &anchor, &attrs, &eq_num));
        } else if let Some(fig) = is_paragraph.then(|| figure_parts(node)).flatten() {
            // Standalone image -> a numbered `<figure>`; register `#fig-` ids so
            // `@fig-x` cross-references resolve to the number.
            fig_count += 1;
            let fig_num = float_number(chapter, fig_count);
            if let Some(fid) = fig.attrs.id.as_deref().filter(|i| i.starts_with("fig-")) {
                register_xref(
                    &mut xref_registry,
                    &mut warnings,
                    fid,
                    fig_num.clone(),
                    source_file.as_deref(),
                    buf_start as u32,
                );
            }
            html.push_str(&emit_figure(&fig, &attrs, &fig_num));
        } else if let Some(role) = &cell_role {
            // A labelled/captioned code cell -> a numbered, anchored figure/listing.
            let lang = cell.as_ref().map(|c| c.lang.clone()).unwrap_or_default();
            let code = cell.as_ref().map(|c| c.code.clone()).unwrap_or_default();
            match role {
                CellRole::Figure { anchor, caption } => {
                    // Register from what will EXIST, like the `Listing` arm below — not
                    // from what the label declares. A figure materializes only when it is
                    // emitted here at render time (mermaid/`{js}`) OR the cell executes
                    // against a kernel (python/r) with its output kept (`include: true`);
                    // `include: false` drops that output block outright (`exec.rs`).
                    //
                    // Any OTHER lang — `{bash}`, `{sql}`, `{julia}`, … — is neither
                    // emitted here nor executed, so it has no figure for ANY value of
                    // `include`. Registering an anchor + burning a number for one would
                    // point `@fig-x` at a "Figure N" no element carries and shift every
                    // later figure down by one. `executes_to_kernel` is the canonical
                    // executable set (`exec::kernel_lang` is drift-locked to it).
                    let include = cell.as_ref().is_none_or(|c| c.include);
                    // Under `--no-exec` a client-side figure (`{js}`, `{glsl}`) no longer
                    // materializes, so it must not burn a figure number or register an
                    // anchor — the same reasoning the comment above gives for
                    // `{bash}`/`{sql}`, reached for the same reason (nothing will emit the
                    // float). It falls through to the keeps-its-source arm below and warns
                    // like any other non-executing labelled cell.
                    // `client_lang_runnable` is the same reasoning one step further: a
                    // client language whose runtime is unavailable in this build also
                    // materializes nothing, so it must not burn a figure number either.
                    let emitted_at_render_time = lang == "mermaid"
                        || (client_lang(&lang).is_some()
                            && client_lang_runnable(&lang)
                            && !no_exec_in_force());
                    if !(emitted_at_render_time || (executes_to_kernel(&lang) && include)) {
                        if let Some(a) = anchor {
                            warnings.push(if include {
                                unreferenceable_nonexec_label(
                                    "figure",
                                    a,
                                    &lang,
                                    source_file.clone(),
                                    buf_start,
                                )
                            } else {
                                unreferenceable_hidden_label(
                                    "figure",
                                    a,
                                    source_file.clone(),
                                    buf_start,
                                )
                            });
                        }
                        // A visible non-executable cell keeps its source (the author wants
                        // to show the code), just not wrapped as a numbered figure. An
                        // `include: false` cell still RUNS (if executable) but its output
                        // is dropped, so hide the source.
                        if include {
                            emit(node, &attrs, &mut html);
                        } else {
                            html.push_str(&hidden_cell(&attrs));
                        }
                    } else {
                        fig_count += 1;
                        let fig_num = float_number(chapter, fig_count);
                        if let Some(a) = anchor {
                            register_xref(
                                &mut xref_registry,
                                &mut warnings,
                                a,
                                fig_num.clone(),
                                source_file.as_deref(),
                                buf_start as u32,
                            );
                        }
                        match lang.as_str() {
                            // Client-rendered outputs are known now, so wrap them here.
                            "mermaid" => html.push_str(&emit_mermaid_figure(
                                &code,
                                anchor.as_deref(),
                                caption.as_deref(),
                                &attrs,
                                &fig_num,
                            )),
                            l if client_lang(l).is_some() => html.push_str(&emit_client_figure(
                                client_lang(l).expect("guarded by the match arm"),
                                &code,
                                &id,
                                cell.as_ref().map(|c| &c.js),
                                &attrs,
                                &FloatLabel {
                                    anchor: anchor.as_deref(),
                                    caption: caption.as_deref(),
                                    num: &fig_num,
                                },
                            )),
                            // Python/R: the source renders now; tag the cell so the
                            // executor wraps the (later) output in the numbered figure.
                            _ => {
                                if let Some(c) = cell.as_mut() {
                                    c.figure = Some(CellFigure {
                                        anchor: anchor.clone(),
                                        caption: caption.clone(),
                                        number: fig_num.clone(),
                                    });
                                }
                                // `echo: false` hides the code but keeps the figure
                                // tagging, so the executed output still becomes Figure N.
                                // (`include: false` never reaches here — it is handled
                                // above, where the figure is known not to materialize.)
                                if cell.as_ref().is_some_and(|c| !c.echo) {
                                    html.push_str(&hidden_cell(&attrs));
                                } else {
                                    emit(node, &attrs, &mut html);
                                }
                            }
                        }
                    }
                }
                CellRole::Listing {
                    anchor,
                    caption,
                    fold,
                } => {
                    // A listing exists to show source, so only `include: false`
                    // (hide everything) suppresses it.
                    if cell.as_ref().is_some_and(|c| !c.include) {
                        html.push_str(&hidden_cell(&attrs));
                    } else {
                        lst_count += 1;
                        let lst_num = float_number(chapter, lst_count);
                        if let Some(a) = anchor {
                            register_xref(
                                &mut xref_registry,
                                &mut warnings,
                                a,
                                lst_num.clone(),
                                source_file.as_deref(),
                                buf_start as u32,
                            );
                        }
                        html.push_str(&emit_code_listing(
                            &code,
                            &lang,
                            anchor.as_deref(),
                            caption.as_deref(),
                            fold.as_ref(),
                            &attrs,
                            &lst_num,
                        ));
                    }
                }
                CellRole::Table { anchor, caption } => {
                    // The table is the cell's executed output (e.g. a pandas
                    // DataFrame), so tag the cell and let the executor inject the
                    // caption/id; the number is assigned in document order by
                    // `apply_table_captions` below. The source renders now (or is
                    // hidden by `echo: false`), like a figure cell.
                    //
                    // A table has no render-time emission path at all — unlike a figure,
                    // which mermaid/`{js}` can produce here — so it materializes ONLY when
                    // the cell executes (python/r) with its output kept. `include: false`
                    // or a non-executable lang (`{bash}`, `{sql}`, …) means no `<table>`
                    // and no anchor; leaving `c.table` unset is what keeps
                    // `apply_table_captions` from numbering and registering a phantom.
                    let include = cell.as_ref().is_none_or(|c| c.include);
                    if executes_to_kernel(&lang) && include {
                        if let Some(c) = cell.as_mut() {
                            c.table = Some(CellTable {
                                anchor: anchor.clone(),
                                caption: caption.clone(),
                                // Filled in document order by `apply_table_captions`.
                                number: String::new(),
                            });
                        }
                        if cell.as_ref().is_some_and(|c| !c.echo) {
                            html.push_str(&hidden_cell(&attrs));
                        } else {
                            emit(node, &attrs, &mut html);
                        }
                    } else {
                        if let Some(a) = anchor {
                            warnings.push(if include {
                                unreferenceable_nonexec_label(
                                    "table",
                                    a,
                                    &lang,
                                    source_file.clone(),
                                    buf_start,
                                )
                            } else {
                                unreferenceable_hidden_label(
                                    "table",
                                    a,
                                    source_file.clone(),
                                    buf_start,
                                )
                            });
                        }
                        // A visible non-executable cell keeps its source; an
                        // `include: false` cell hides it (the executor drops the output).
                        if include {
                            emit(node, &attrs, &mut html);
                        } else {
                            html.push_str(&hidden_cell(&attrs));
                        }
                    }
                }
            }
        } else if let Some((c, spec)) = cell
            .as_ref()
            .and_then(|c| client_lang(&c.lang).map(|spec| (c, spec)))
        {
            if no_exec_in_force() || !client_lang_runnable(&c.lang) {
                // `--no-exec`: a client-side cell is a code cell whose kernel is the
                // browser, so it renders as source like a `{python}` cell with no kernel
                // does (item 79). `emit` keeps the highlighted source and the block's
                // id/sourcepos, so click-to-source and the incremental swap are unaffected.
                //
                // A language whose runtime is unavailable in this build takes the identical
                // arm, for the identical reason: nothing will run it, so emitting the live
                // wrapper would leave a husk. Doing it here rather than as a post-pass over
                // finished HTML also means the wrapper is never emitted, so no later stage
                // has to recover the author's source back out of a `<script>` element.
                emit(node, &attrs, &mut html);
            } else {
                // Native interactive client-side cell (`{js}`, `{glsl}`): the matching
                // enhancer runs it in the reader's browser (no Observable runtime).
                html.push_str(&emit_client_cell(spec, &c.code, &id, &c.js, &attrs));
            }
        } else if cell.as_ref().is_some_and(|c| !c.echo || !c.include) {
            // `echo: false` / `include: false`: keep the block so the executor still
            // runs it, but hide its source.
            html.push_str(&hidden_cell(&attrs));
        } else if h_attr.is_some() {
            // Heading with a Pandoc attribute: render it, then drop the trailing
            // `{#id ...}` text comrak left behind (it isn't CommonMark).
            emit(node, &attrs, &mut html);
            html = strip_heading_attr(&html);
        } else {
            emit(node, &attrs, &mut html);
            // Apply Pandoc attribute blocks trailing a link (`[t](u){.btn}`) onto
            // the `<a>`, dropping the literal `{...}` comrak left as text.
            html = apply_link_attrs(&html);
            // Drop a stray trailing `\` (a hard break at the end of a block): strict
            // CommonMark leaves it literal, but Pandoc drops it. Match Pandoc.
            html = strip_trailing_hardbreak(&html);
        }
        // One <h1> per page: when this render emits a visible title block, shift every body
        // heading so its sections nest directly beneath the title. `emits_title_block` is
        // the title-block insertion condition (not hidden, titled), computed once before
        // the walk and shared with the section numbering above, so a shifted heading and
        // its `@sec-` number can never disagree.
        if let Some(level) = heading_level
            && let Some(shift) = heading_shift
        {
            html = shift_heading_html(&html, level, shift);
        }
        // Reserve each local raster image's box before its bytes arrive, so loading one does
        // not shove the text below it down the page. Relative to `base_dir` like every other
        // asset reference the build resolves (`copy_local_assets`), which is also what an
        // `{{< include >}}`d block's paths already resolve against.
        if let Some(base) = base_dir {
            html = image_annotator.annotate(&html, base);
        }
        // Splice each note in immediately after its own `<sup>`. Last, so the note's
        // markup is not itself rewritten by any of the passes above (the annotator would
        // otherwise size an image inside a note against the wrong base, and the heading
        // shift would walk into it).
        for (name, ix) in &notes {
            let def = &footnote_defs[name];
            let (sidenote, flattened) = emit::footnote_sidenote(
                def.node,
                name,
                *ix,
                &def.sourcepos,
                def.source_file.as_deref(),
            );
            if flattened {
                warnings.push(
                    Warning::new(format!(
                        "footnote `[^{name}]` contains block content (a list, quote or code \
                         block); a note renders beside its reference as a margin sidenote, \
                         which can only hold inline content, so it was flattened to its text"
                    ))
                    .at(def.source_file.clone(), def.line),
                );
            }
            let anchor = emit::footnote_ref_markup(name, 1, *ix);
            html = html.replacen(&anchor, &format!("{anchor}{sidenote}"), 1);
        }
        flat.push(FlatBlock {
            buf_start,
            block: Block {
                id,
                sourcepos,
                source_file,
                html,
                cell,
                nested: Vec::new(),
            },
        });
    }

    let mut blocks = group_divs(flat, &spans, origins, &mut id_counts, &mut warnings);
    // Pandoc table captions (`: caption {#tbl-x}` after a table) are numbered and
    // folded into the table's `<caption>`; registers `tbl-x` for `@tbl-` refs.
    apply_table_captions(&mut blocks, &mut xref_registry, &mut warnings, chapter);
    let bib_line = crate::frontmatter::bibliography_line(src);
    let bib = load_bibliography(
        &bib_paths,
        site.map(|s| s.bibliography.as_slice()).unwrap_or(&[]),
        base_dir,
        include_root,
        bib_line,
        &mut warnings,
    );
    warnings.extend(crate::cite::process(
        &mut blocks,
        &bib,
        &xref_registry,
        bib_line,
    ));
    // No gathered endnote section: each note renders beside its own reference (see the
    // splice in the walk above). Keeping a trailing list as well would put every note's
    // text in the DOM twice, which Ctrl-F and all four text projections (`taliesin read`,
    // `skim.rs`, the search index, `llms-full.txt`) would each report twice.
    // A visible title block. It is a generated block (no sourcepos), so it rides the
    // block model + diff like the References section.
    // A dated document is an article (a post), which gates both the reading-time estimate and
    // the standalone `og:type`. An undated page is a generic `website`, not an `article`.
    let is_article = date.as_deref().is_some_and(|d| !d.is_empty());
    // Reading-time estimate, shown only for a dated post. Prose words / 200 wpm, rounded to
    // whole minutes (min 1), matching the client's live count; `src` is include-expanded, so
    // an included file's prose is included.
    let read_time = is_article.then(|| {
        let mins = ((crate::prose::word_count(src) + 100) / 200).max(1);
        format!("{mins} min read")
    });
    if !hide_title_block
        && let Some(tb) = title_block_html(
            title.as_deref(),
            subtitle.as_deref(),
            &authors,
            date.as_deref(),
            description.as_deref(),
            read_time.as_deref(),
        )
    {
        blocks.insert(
            0,
            Block {
                id: "tali-title-block".to_string(),
                sourcepos: String::new(),
                source_file: None,
                html: tb,
                cell: None,
                nested: Vec::new(),
            },
        );
    } else if hide_title_block
        && let Some(t) = title.as_deref().filter(|t| !t.is_empty())
        && !blocks.iter().any(|b| b.html.contains("<h1"))
    {
        // `title-block-style: none` suppresses the *visible* title, but a listing/section/
        // landing page still needs one `<h1>` for SEO + heading-nav (PA-H2): without it the
        // outline opens at an H2/H3 card with no page context. Inject a visually-hidden `<h1>`
        // — but only when the body carries no `<h1>` of its own, so a `hero:` landing (which
        // renders its own `<h1>`) never gets a duplicate.
        let mut h1 = String::from("<h1 class=\"tali-sr-only\" data-block-id=\"tali-sr-title\">");
        escape_html(t, &mut h1);
        h1.push_str("</h1>");
        blocks.insert(
            0,
            Block {
                id: "tali-sr-title".to_string(),
                sourcepos: String::new(),
                source_file: None,
                html: h1,
                cell: None,
                nested: Vec::new(),
            },
        );
    }
    // The appendix (author contributions), after the References a reader scrolls past and
    // before the code download, which is chrome rather than content.
    let ap = appendix_html(&authors);
    if !ap.is_empty() {
        blocks.push(Block {
            id: APPENDIX_BLOCK_ID.to_string(),
            sourcepos: String::new(),
            source_file: None,
            html: ap,
            cell: None,
            nested: Vec::new(),
        });
    }
    // The reader's code download (C-READ-2), appended last of the generated blocks so it
    // sits after the References/footnotes a reader scrolls past. `mark_section_extents`
    // below must still see the final block list.
    if let Some(b) = repro::repro_block(&blocks) {
        blocks.push(b);
    }
    // LAST over the block list: every block that will ever be in the document is in it
    // by now (References, footnotes, the title block), so a section's end is final.
    mark_section_extents(&mut blocks);
    let theme_css = resolve_theme(theme.as_deref(), base_dir, include_root, &mut warnings);
    let theme_default = theme_default_mode(theme.as_deref()).to_string();
    let theme_is_custom = !theme_css.trim().is_empty();
    RenderedDoc {
        title,
        subtitle,
        lang,
        description,
        is_article,
        // Standalone default: a TOC only when the page asked for one. The site
        // path overrides this via `page_toc` using `toc_explicit`.
        toc: toc_explicit.unwrap_or(false),
        toc_explicit,
        theme_css,
        theme_default,
        theme_is_custom,
        includes,
        warnings,
        xref_numbers: xref_registry,
        blocks,
    }
}

/// OpenGraph / Twitter-card / SEO `<meta>` for a standalone document, from its own
/// front matter. A single file has no site URL, so there's no canonical/og:url or
/// absolute image — just the text tags that make a shared link or search result
/// meaningful. (Site pages get the richer, URL-aware set from `site::meta`.)
fn social_meta_head(title: Option<&str>, description: Option<&str>, is_article: bool) -> String {
    let meta = |attr: &str, key: &str, val: &str| {
        format!("\n<meta {attr}=\"{key}\" content=\"{}\">", escape_attr(val))
    };
    let mut h = String::new();
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        h.push_str(&meta("name", "description", d));
        h.push_str(&meta("property", "og:description", d));
        h.push_str(&meta("name", "twitter:description", d));
    }
    h.push_str(&meta(
        "property",
        "og:type",
        if is_article { "article" } else { "website" },
    ));
    if let Some(t) = title.filter(|s| !s.is_empty()) {
        h.push_str(&meta("property", "og:title", t));
        h.push_str(&meta("name", "twitter:title", t));
    }
    h.push_str(&meta("name", "twitter:card", "summary"));
    h
}

/// Humanize an ISO `YYYY-MM-DD` date for display ("2026-04-14" → "14 April 2026"),
/// day un-padded. Any value that isn't a valid plain `YYYY-MM-DD` (a free-form date the
/// author wrote, or one carrying a time) is returned unchanged, so nothing an author
/// typed is ever mangled. Shared by the post title block and the listing cards; the
/// machine-readable ISO form is emitted separately (JSON-LD, `citation_*`, the feed).
pub(crate) fn humanize_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    // A date carrying a time prints verbatim: humanizing it would silently drop the time
    // the author wrote. That `T` rule is this function's own — `calendar_date` answers only
    // "which day is this", which the feed and sitemap ask of the same value.
    if !date.contains('T')
        && let Some((y, month, day)) = crate::frontmatter::calendar_date(date)
    {
        return format!("{day} {} {y}", MONTHS[month as usize - 1]);
    }
    date.to_string()
}

/// A date rendered as a `<time datetime="…">` element (PA-M1): a machine-readable ISO value
/// in the attribute, the humanized form as the visible text. A plain calendar date gets a
/// normalized `YYYY-MM-DD` `datetime`; a value carrying a time (or one we can't parse) is
/// passed through verbatim. `class` is optional (empty ⇒ no class attribute).
pub fn time_html(raw: &str, class: &str) -> String {
    let iso = match crate::frontmatter::calendar_date(raw) {
        Some((y, m, d)) if !raw.contains('T') => format!("{y:04}-{m:02}-{d:02}"),
        _ => raw.to_string(),
    };
    let cls = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", escape_attr(class))
    };
    format!(
        "<time{cls} datetime=\"{}\">{}</time>",
        escape_attr(&iso),
        html_escape(&humanize_date(raw))
    )
}

/// Build the visible title-block header from front-matter metadata (title +
/// optional subtitle/description and an author · date meta line). Returns `None`
/// without a title. Carries `data-block-id` so it lives in the block model.
fn title_block_html(
    title: Option<&str>,
    subtitle: Option<&str>,
    authors: &[crate::author::Author],
    date: Option<&str>,
    description: Option<&str>,
    read_time: Option<&str>,
) -> Option<String> {
    let title = title?;
    let mut h = String::from(
        "<header class=\"tali-title-block\" data-block-id=\"tali-title-block\"><h1 class=\"title\">",
    );
    h.push_str(&html_escape(title));
    h.push_str("</h1>");
    if let Some(s) = subtitle.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"subtitle\">{}</p>", html_escape(s)));
    }
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"description\">{}</p>", html_escape(d)));
    }
    let author_span = byline_html(authors);
    // The date is humanized for display ("2026-04-14" → "14 April 2026") inside a
    // `<time datetime>` so it stays machine-readable; a value that isn't a plain ISO date is
    // shown verbatim (never mangled).
    let date_span = date.filter(|s| !s.is_empty()).map(|s| time_html(s, ""));
    // A subtle reading-time estimate ("N min read"), only when the caller supplies one
    // (a dated post). Rides the same muted meta line as author · date.
    let read_span = read_time
        .filter(|s| !s.is_empty())
        .map(|s| format!("<span class=\"tali-read-time\">{}</span>", html_escape(s)));
    let meta: Vec<String> = [author_span, date_span, read_span]
        .into_iter()
        .flatten()
        .collect();
    if !meta.is_empty() {
        h.push_str(&format!(
            "<div class=\"tali-title-meta\">{}</div>",
            meta.join("")
        ));
    }
    h.push_str(&affiliations_html(authors));
    h.push_str("</header>");
    Some(h)
}

/// The byline: every declared author, each carrying the superscript markers that tie it
/// to the affiliation list below. `None` when nobody is named, so the meta line collapses
/// exactly as it did when the byline was one optional string.
///
/// Names are joined with a comma rather than an "and" before the last: an author list is
/// data here, and the "A, B and C" form has to know about locale and about a two-author
/// list to read correctly, which is a lot of machinery for a separator.
fn byline_html(authors: &[crate::author::Author]) -> Option<String> {
    let named: Vec<&crate::author::Author> = authors
        .iter()
        .filter(|a| !a.name.trim().is_empty())
        .collect();
    if named.is_empty() {
        return None;
    }
    let index = crate::author::affiliation_index(authors);
    let mut out = String::from("<span class=\"tali-byline\">");
    for (i, a) in named.iter().enumerate() {
        if i > 0 {
            out.push_str("<span class=\"tali-byline-sep\">, </span>");
        }
        out.push_str("<span class=\"tali-author\">");
        // A `url:` makes the name a link to that person; without one it is plain text.
        // Deliberately not `mailto:` from `email:` — an address published as a link is
        // what address harvesters read, and `email:` exists for the metadata consumers.
        match a.url.as_deref() {
            Some(u) => out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape_attr(safe_url(u, false)),
                html_escape(&a.name)
            )),
            None => out.push_str(&html_escape(&a.name)),
        }
        let mut marks: Vec<String> = crate::author::marks(a, &index)
            .iter()
            .map(|m| m.to_string())
            .collect();
        if a.equal {
            marks.push("*".to_string());
        }
        if !marks.is_empty() {
            out.push_str(&format!(
                "<sup class=\"tali-author-mark\">{}</sup>",
                html_escape(&marks.join(","))
            ));
        }
        out.push_str("</span>");
    }
    out.push_str("</span>");
    Some(out)
}

/// The block id of the generated appendix, so the projections and the incremental client
/// can name it without matching on markup.
pub const APPENDIX_BLOCK_ID: &str = "tali-appendix";

/// The appendix: **Author Contributions**, **Acknowledgments**, and the DOI, in that order.
///
/// Distill's `d-appendix` made author-contributions a first-class *section* rather than a
/// paragraph someone remembered to write, and that is the whole idea here: the information
/// exists on nearly every multi-author paper, and leaving it to prose is what makes it
/// inconsistent and easy to omit.
///
/// **Deterministic by construction** — nothing here reads a clock. A generated block that
/// embedded an "accessed" date or a build timestamp would change on every build, which
/// breaks the byte-identical build *and* invalidates the freeze cache on every run; the
/// same rule `cite_this` documents.
///
/// Empty string when the page declares none of the three, so an ordinary post gains no
/// trailing furniture.
fn appendix_html(authors: &[crate::author::Author]) -> String {
    let contributors: Vec<&crate::author::Author> = authors
        .iter()
        .filter(|a| {
            a.contribution
                .as_deref()
                .is_some_and(|c| !c.trim().is_empty())
        })
        .collect();
    if contributors.is_empty() {
        return String::new();
    }
    let mut h = format!("<div class=\"tali-appendix\" data-block-id=\"{APPENDIX_BLOCK_ID}\">");
    if !contributors.is_empty() {
        h.push_str(
            "<section class=\"tali-appendix-part\"><h2>Author Contributions</h2><dl class=\"tali-contributions\">",
        );
        for a in contributors {
            h.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                html_escape(&a.name),
                html_escape(a.contribution.as_deref().unwrap_or("").trim())
            ));
        }
        h.push_str("</dl></section>");
    }
    h.push_str("</div>");
    h
}

/// The numbered affiliation list under the byline, plus the equal-contribution note when
/// any author claims one. Empty when no author declared either, so a page with a plain
/// `author: Name` emits exactly the title block it always did.
///
/// An `<ol>` for the semantics, but the number is written into the markup rather than
/// left to the list marker. Measured in a browser: laying the entries out inline (so two
/// institutions read as one quiet line instead of a stacked block) needs `display: inline`
/// on the `<li>`, and an inline list item **has no marker at all** — the numbers simply
/// vanished, leaving the `1,2` superscripts beside the names pointing at nothing. The
/// numbers are the content here, not decoration, so they are emitted as content.
fn affiliations_html(authors: &[crate::author::Author]) -> String {
    let index = crate::author::affiliation_index(authors);
    let any_equal = authors.iter().any(|a| a.equal);
    if index.is_empty() && !any_equal {
        return String::new();
    }
    let mut out = String::from("<div class=\"tali-affiliations\">");
    if !index.is_empty() {
        out.push_str("<ol class=\"tali-affiliation-list\">");
        for (i, aff) in index.iter().enumerate() {
            out.push_str(&format!(
                "<li><sup class=\"tali-affiliation-num\">{}</sup>{}</li>",
                i + 1,
                html_escape(aff)
            ));
        }
        out.push_str("</ol>");
    }
    if any_equal {
        out.push_str("<p class=\"tali-equal-note\">* Equal contribution</p>");
    }
    out.push_str("</div>");
    out
}

/// The built-in dark theme, scoped to `html[data-theme="dark"]` so it can be
/// flipped at runtime (the toggle / OS preference set the attribute). Always
/// shipped alongside the light `:root` base. The `:root` light vars plus this
/// block are the reference template for a community theme.
const DARK_CSS: &str = include_str!("../../assets/css/dark.css");

/// Load and merge the bibliography file(s) named in the front matter, resolved
/// relative to `base_dir`, laid **over** the project-wide `shared` files (already resolved
/// to absolute paths by `Site::discover`). Returns an empty bibliography when none is found
/// (citations still de-leak; cross-references still resolve).
///
/// Layer order is the feature: `shared` is read first so a page's own entry with the same
/// key wins, which is how a post corrects a shared reference without editing the shared
/// file. `shared`'s own diagnostics (unreadable path, duplicate key, dead entry) are NOT
/// raised here — they belong to `_site.yml`, and raising them per page would print one
/// project-level mistake once per page (`Site::validate_shared_bibliography`).
fn load_bibliography(
    paths: &[String],
    shared: &[PathBuf],
    base_dir: Option<&Path>,
    root: Option<&Path>,
    bib_line: Option<u32>,
    warnings: &mut Vec<Warning>,
) -> crate::cite::Bibliography {
    let mut shared_text = String::new();
    for p in shared {
        if let Ok(content) = std::fs::read_to_string(p) {
            shared_text.push_str(&content);
            shared_text.push('\n');
        }
    }
    let Some(base) = base_dir else {
        return crate::cite::parse_bib(&shared_text);
    };
    // Point every `.bib` diagnostic at the front-matter `bibliography:` line (the
    // .bib is an external file with no in-doc position of its own), so it is
    // click-to-source rather than an unlocated warning.
    let locate = |w: Warning| match bib_line {
        Some(l) => w.at(None, l),
        None => w,
    };
    let mut text = String::new();
    for path in paths {
        let path = path.trim();
        // Only `.bib` is supported; a differently-suffixed path (a stray token or an
        // unsupported CSL-JSON/YAML) is skipped rather than mis-read — but warn, since it
        // would otherwise silently fail to resolve any of its citations.
        if !path.ends_with(".bib") {
            warnings.push(locate(Warning::new(format!(
                "bibliography `{path}` ignored: only BibTeX (`.bib`) is supported"
            ))));
            continue;
        }
        // An explicitly named `.bib` that can't be read is worth flagging: citations
        // would otherwise silently fail to resolve, rendering as bare keys. A path that
        // was *refused* is reported as such rather than as "not found", so an author
        // whose file plainly exists is not sent hunting for a typo.
        match crate::includes::try_join_in(base, path, root) {
            Ok(p) => match std::fs::read_to_string(&p) {
                Ok(content) => {
                    text.push_str(&content);
                    text.push('\n');
                }
                Err(_) => warnings.push(locate(Warning::new(format!(
                    "bibliography file not found: {path}"
                )))),
            },
            Err(crate::includes::Refused::OutsideRoot) => warnings.push(locate(Warning::new(
                format!("bibliography `{path}` is outside the project root and was not read"),
            ))),
            Err(crate::includes::Refused::SymlinkOutsideRepo) => {
                warnings.push(locate(Warning::new(format!(
                    "bibliography `{path}` is a symlink whose target is outside the project \
                     repository and was not read"
                ))))
            }
        }
    }
    let (page_bib, bib_warnings) = crate::cite::parse_bib_warned(&text);
    warnings.extend(bib_warnings.into_iter().map(|m| locate(Warning::new(m))));
    let mut bib = crate::cite::parse_bib(&shared_text);
    bib.overlay(page_bib);
    bib
}

/// A top-level block plus its line in the (post-include, post-blank) buffer,
/// used to group blocks back into fenced-div containers.
struct FlatBlock {
    buf_start: usize,
    block: Block,
}

/// One `[^name]: …` definition, resolved before the walk so a reference met earlier in
/// the document can carry its note. comrak moves every definition to the document end,
/// so without this pre-pass the walk would reach a reference before its definition.
struct FootnoteDef<'a> {
    /// The definition's AST node; rendered at the reference, which is what supplies the
    /// visible number (`ix` lives on the reference, not here).
    node: &'a AstNode<'a>,
    /// The definition's OWN source range — where the author wrote it, not where comrak
    /// moved it — so a Ctrl-click on the note lands on the right line.
    sourcepos: String,
    source_file: Option<String>,
    /// The definition's source lines, folded into the hash of every block that displays
    /// it so editing a note re-keys that block. Without this the diff sees no change.
    src: String,
    /// Start line, for the flattening warning.
    line: u32,
}

/// The ` data-source-file="…"` attribute for a block from an included file (empty
/// for a primary-document block). Preserves the click-to-source invariant for both
/// leaf blocks and fenced-div containers.
fn source_file_attr(file: Option<&str>) -> String {
    match file {
        Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
        None => String::new(),
    }
}

/// The ` id="…"` attribute for an optional anchor (empty when `None`). Shared by the
/// figure/listing/div emitters that put a `#fig-`/`#lst-`/`#id` anchor on a wrapper.
pub(crate) fn id_attr(id: Option<&str>) -> String {
    match id {
        Some(i) => format!(" id=\"{}\"", escape_attr(i)),
        None => String::new(),
    }
}

/// Map a 1-based buffer line to its (origin file, origin line). Without a
/// source map, the file is the primary document and the line is unchanged.
fn map_origin(origins: Option<&[LineOrigin]>, buffer_line: usize) -> (Option<String>, usize) {
    match origins.and_then(|o| o.get(buffer_line.saturating_sub(1))) {
        Some(origin) => (origin.file.clone(), origin.line),
        None => (None, buffer_line),
    }
}

/// Render a complete, viewable HTML page (used by the one-shot CLI). The
/// front-matter `title:` becomes the document `<title>`; `fallback_title` is
/// used when the source declares none.
///
/// ```
/// let html = taliesin_core::render_html_page("---\ntitle: Demo\n---\n\nHi.\n", "fallback");
/// assert!(html.contains("<title>Demo</title>"));
/// assert!(html.contains("Hi."));
/// ```
pub fn render_html_page(src: &str, fallback_title: &str) -> String {
    // The in-process full-page API ships everything (like a preview); the static
    // `build`/`render` CLI opts into content-gating via `render_doc_to_page`.
    page_from_doc(&render_document(src), fallback_title, OutputMode::Preview)
}

/// Like [`render_html_page`], resolving `{{< include >}}` relative to `base_dir`.
pub fn render_html_page_with_includes(src: &str, base_dir: &Path, fallback_title: &str) -> String {
    page_from_doc(
        &render_document_with_includes(src, base_dir),
        fallback_title,
        OutputMode::Preview,
    )
}

/// Self-contained KaTeX stylesheet (fonts inlined as data URIs at build time).
const KATEX_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/katex-inlined.css"));

/// The owned body typeface: Newsreader `@font-face` rules with the two variable woff2
/// faces (roman + italic) inlined as data URIs at build time (see `build.rs`). Emitted
/// ahead of the base stylesheet so `--tali-font-body` resolves to the loaded face.
pub(crate) const FONTS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/fonts-inlined.css"));

/// The same `@font-face` rules with the faces left as `url(fonts/<name>.woff2)` refs
/// instead of inlined base64 — the pre-`build.rs` source, for a target that can ship the
/// faces as their own files. See [`FONT_FILES`] and [`shared_site_css_linked_fonts`].
const FONTS_CSS_LINKED: &str = include_str!("../../assets/css/fonts.css");

/// The body typeface's two variable faces as `(source filename, bytes)`, so a multi-page
/// build can content-hash and write them beside the rest of `_assets/`.
///
/// Why they leave the stylesheet at all (item 150): base64 inflates ~33% and gzips poorly,
/// and these two were **160 KB inside a render-blocking `<link>` on every page of a site** —
/// the only weight a reader pays on all of them. As files they are fetched once, cached
/// across every page, and no longer block first paint.
///
/// This is a **per-target** choice, not a global one. `build <file.tmd>` promises ONE
/// self-contained file, so it keeps [`FONTS_CSS`]; only a target that already emits a
/// sidecar `_assets/` directory uses these. Either way the face is self-hosted and offline:
/// no CDN, no network at render time.
pub const FONT_FILES: &[(&str, &[u8])] = &[
    (
        "newsreader-latin-wght-normal.woff2",
        include_bytes!("../../assets/fonts/newsreader-latin-wght-normal.woff2"),
    ),
    (
        "newsreader-latin-wght-italic.woff2",
        include_bytes!("../../assets/fonts/newsreader-latin-wght-italic.woff2"),
    ),
];

/// [`FONTS_CSS_LINKED`] with each face's `url(fonts/<name>.woff2)` rewritten to the href
/// the caller shipped it at.
///
/// The href a caller passes must be **relative to the stylesheet**, not to the page: a
/// `url()` inside a sheet resolves against the sheet's own URL. Both the sheet and the
/// faces live in `_assets/`, so a bare hashed filename is correct at every page depth,
/// and a `../` climb here would be a bug that only shows up on nested pages.
fn fonts_css_linked(hrefs: &[(&str, String)]) -> String {
    let mut css = FONTS_CSS_LINKED.to_string();
    for (name, href) in hrefs {
        css = css.replace(&format!("url(fonts/{name})"), &format!("url({href})"));
    }
    css
}

/// The owned design tokens (the light palette, fonts, geometry, motion),
/// `include_str!`'d ahead of `base.css` so
/// the palette is declared exactly once. See `tokens.css`. The dark palette override
/// is `TOKENS_DARK_CSS` (kept separate so the dark palette override is one layer).
pub(crate) const TOKENS_CSS: &str = include_str!("../../assets/css/tokens.css");
/// The dark palette override, keyed on `html[data-theme="dark"]`. See `TOKENS_CSS`.
pub(crate) const TOKENS_DARK_CSS: &str = include_str!("../../assets/css/tokens-dark.css");

/// Base document styling (typography, tables, callouts, references, block
/// highlight). Emitted by the page builders in `page.rs`; KaTeX rides
/// along when the page has (or, in a live preview, may gain) math.
const BASE_CSS: &str = include_str!("../../assets/css/base.css");

/// A human-readable ASCII-art banner emitted as the first thing inside `<head>`. The
/// machine-readable `<meta name="generator">` already ships; this is its view-source
/// sibling, so a developer who opens the page (view-source / dev tools) can find the
/// tool that built it. Kept INSIDE `<head>`, never before the doctype, so the first
/// byte stays `<!DOCTYPE`. Assembled at compile time (`concat!`) so `VERSION` is baked
/// in. NB: an HTML comment body must not contain a `--`, so separators stay single-dash.
pub(crate) const GENERATOR_BANNER: &str = concat!(
    r##"<!--
  mmmmmmm        ""#      "                    "
     #     mmm     #    mmm     mmm    mmm   mmm    m mm
     #    "   #    #      #    #"  #  #   "    #    #"  #
     #    m"""#    #      #    #""""   """m    #    #   #
     #    "mm"#    "mm  mm#mm  "#mm"  "mmm"  mm#mm  #   #

  Taliesin v"##,
    env!("CARGO_PKG_VERSION"),
    r##"  -  https://taliesin.sh
  Rendered from .tmd source, not a batch compiler: a warm, source-mapped,
  block-modeled live HTML process.  https://github.com/AJBogo9/taliesin
-->
"##,
);

// mermaid (pinned) is loaded as a separate script rather than bundled: the library is
// large (~2.8 MB) and only needed when a diagram is actually present, so it's lazy-loaded
// by `mermaid.js` (the self-registering enhancer) the first time a `{mermaid}` block
// appears. It's a client-side presentation layer, so it never affects the block model or
// the diff. (Syntax highlighting is server-side; KaTeX is bundled
// offline.)
//
// OFFLINE: a static Build with a diagram INLINES the vendored library (`MERMAID_MIN_JS`,
// ~2.5 MB, content-gated to pages that actually have a `pre.mermaid`), so a `--out` doc /
// book renders diagrams with zero network. The live Preview points the lazy loader at
// [`PREVIEW_MERMAID_PATH`], a same-origin route both dev servers serve from that same
// vendored copy — so preview is offline too, without inlining 2.5 MB into the page shell.
// `TALIESIN_MERMAID_URL` overrides the loader URL in either mode (e.g. to a self-hosted
// copy) and is also the loader's never-reached Build fallback if the inlined global somehow
// isn't present. Either way a load failure is *visible* (a `[data-mermaid-error]` banner),
// never a silent blank.
//
// This CDN default is now reached only by a caller that renders in Build mode without
// inlining (nothing in-tree does) — kept as a last-resort fallback, not a normal path.
const MERMAID_DEFAULT: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.16.0/dist/mermaid.min.js";

/// Same-origin path the live preview serves the vendored mermaid library from. Both dev
/// servers route it (see `serve`/`serve_site`), which is what makes a diagram render in
/// preview with **zero network** — previously the loader went to the CDN even though the
/// library was already compiled into the binary, so the one load-bearing offline guarantee
/// had a hole in exactly the mode the author spends all day in (OFF-2/AP12).
///
/// A path rather than an inline blob because the page shell is re-served on every
/// navigation: inlining would add 2.5 MB to each, while a route is fetched once and then
/// sits in the browser cache. It also keeps working when a document *gains* its first
/// diagram mid-session, which content-gated inlining could not.
pub const PREVIEW_MERMAID_PATH: &str = "/_taliesin/mermaid.min.js";

/// The vendored mermaid library, for the dev servers to serve at [`PREVIEW_MERMAID_PATH`].
pub fn mermaid_min_js() -> &'static str {
    MERMAID_MIN_JS
}

/// The URL the lazy mermaid loader fetches the diagram library from. `TALIESIN_MERMAID_URL`
/// wins when set and non-empty; otherwise Preview uses the same-origin
/// [`PREVIEW_MERMAID_PATH`] and everything else falls back to the pinned CDN default.
fn mermaid_url_for(mode: OutputMode) -> String {
    match std::env::var("TALIESIN_MERMAID_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ if mode == OutputMode::Preview => PREVIEW_MERMAID_PATH.to_string(),
        _ => MERMAID_DEFAULT.to_string(),
    }
}

/// The client enhancers: the `window.taliEnhancers` registry + built-ins (copy
/// buttons) in code-enhance.js, then the
/// self-registering mermaid module (which lazy-loads the mermaid library on first
/// use). Emitted after the registry so it is defined when mermaid registers.
/// Syntax highlighting arrives already done from the server. Callers invoke
/// `window.taliEnhanceCode(root)` after (re)mounting; it is idempotent.
pub fn code_scripts() -> String {
    code_scripts_for("", OutputMode::Preview)
}

/// The client enhancer scripts, content-gated by [`OutputMode`]. `code-enhance.js`
/// (copy buttons + the whole reader menu + skip-link and
/// keyboard a11y) rides on every page, since every page benefits. The
/// DOM-specific enhancers (mermaid, `{js}`) ship
/// unconditionally in [`OutputMode::Preview`] (a doc can gain any construct on an
/// edit, same reasoning as the always-on KaTeX/d3 in preview) but only when their
/// target DOM is present in a static [`OutputMode::Build`].
pub fn code_scripts_for(body: &str, mode: OutputMode) -> String {
    let mermaid_present = body.contains("class=\"mermaid\"");
    // A static Build inlines the vendored mermaid library (it sets `globalThis.mermaid`,
    // which the loader below short-circuits on) so a diagram renders FULLY OFFLINE — no
    // CDN, no external request. Preview keeps just the lean lazy loader (dev-time network
    // is fine, and inlining 2.5 MB on every save would bloat the payload). The loader's
    // `{{MERMAID}}` CDN URL stays as a never-reached fallback (window.mermaid is already
    // set), so a stripped/edited inline still degrades gracefully rather than blank.
    let mermaid_lib = if mode == OutputMode::Build && mermaid_present {
        format!("\n<script>{MERMAID_MIN_JS}</script>")
    } else {
        String::new()
    };
    let mermaid = format!(
        "{mermaid_lib}\n<script>{}</script>",
        MERMAID_JS.replace("{{MERMAID}}", &mermaid_url_for(mode))
    );
    // In Preview every gate is open; in Build a gate opens only when the rendered
    // body carries that enhancer's DOM marker.
    let gate = |present: bool, script: &str| -> String {
        if mode == OutputMode::Preview || present {
            format!("\n<script>{script}</script>")
        } else {
            String::new()
        }
    };
    format!(
        "<script>{CODE_ENHANCE_JS}</script>{mermaid_s}{talijs_s}",
        mermaid_s = if mode == OutputMode::Preview || mermaid_present {
            mermaid.clone()
        } else {
            String::new()
        },
        talijs_s = gate(has_client_cells(body), TALIESIN_JS),
    )
}

/// The canonical TOC scrollspy (highlights the section under the navbar). Shared
/// so the static build and the live preview behave identically: the static build
/// inlines this once (it auto-inits on load); the preview also ships it and calls
/// `window.taliInitTocSpy()` after each TOC rebuild. Emitted only on TOC pages.
pub const TOC_SPY_JS: &str = include_str!("../../../../web-client/toc-spy.js");

/// The scripts that only make sense with an on-page TOC: the scrollspy.
/// **Does not include [`search_scripts`]** — Cmd-K is a whole-book affordance
/// and its button renders on every page, so gating it on this page's heading count made
/// the palette advertise itself and then come up empty on any chapter under
/// `MIN_TOC_HEADINGS` (the preview injects unconditionally, so the author never saw it).
pub fn toc_scripts() -> String {
    format!("<script>{TOC_SPY_JS}</script>")
}

/// The Cmd-K palette runtime. Ships wherever the palette's button ships, independently
/// of whether this page earned a table of contents.
pub fn search_scripts() -> String {
    format!("<script>{SEARCH_JS}</script>")
}

/// Cmd/Ctrl-K command palette: searches the whole book (via the cross-page index) or
/// the current document's headings. Its trigger is part of the page chrome, so this
/// ships on every page of a site, TOC or not.
pub const SEARCH_JS: &str = include_str!("../../../../web-client/search.js");

// Native interactive `{js}` cells: vendored d3 + Observable Plot (UMD globals) the
// cells draw with. The small enhancer (`tali-js.js`) ships unconditionally in
// `code_scripts()` (it registers and no-ops without cells, like mermaid); these heavy libs
// are gated on `has_js_cells` in a static BUILD only, and ride unconditionally in a
// preview — see `page::needs_js_libs`.
const D3_JS: &str = include_str!("../../assets/js/d3.min.js");
const PLOT_JS: &str = include_str!("../../assets/js/plot.umd.min.js");
const TALIESIN_JS: &str = include_str!("../../assets/js/tali-js.js");

/// `<head>` assets for native `{js}` cells: vendored d3 + Observable Plot. The enhancer
/// itself rides in [`code_scripts`].
///
/// **When to emit is not this function's decision** — `page::needs_js_libs` owns it:
/// unconditional in a preview (a doc can gain its first `{js}` cell on any edit, and the
/// head cannot be swapped under a live page), content-gated on [`has_js_cells`] in a static
/// build. This doc comment used to say "emit only when a page actually has `{js}` cells",
/// which described the build and silently mis-described the preview it was breaking.
pub(crate) fn js_cell_head() -> String {
    format!("<script>{D3_JS}</script>\n<script>{PLOT_JS}</script>")
}

/// True if a rendered body contains native `{js}` cells (gates the Plot/d3 libs).
pub fn has_js_cells(body: &str) -> bool {
    has_client_cells_of(body, "js")
}

// `code-enhance.js` is authored as ordered per-feature fragments under
// `assets/js/code-enhance/` and concatenated (in filename order) into one inline
// `<script>`. The numeric prefix IS the load order: the registry IIFE (`01`) must
// define `window.taliEnhancers` before the registration block (`09`) runs, and every
// built-in's `function` declaration is hoisted within this single concatenated
// script, so `09` can forward-reference features defined in later fragments. The
// `code_enhance_bundle_matches_fragments_in_order` test (tests.rs) guards that this
// list stays complete + in load order (no separators — the fragments tile exactly).
const CODE_ENHANCE_JS: &str = concat!(
    include_str!("../../assets/js/code-enhance/01-registry.js"),
    include_str!("../../assets/js/code-enhance/04-focus-trap.js"),
    include_str!("../../assets/js/code-enhance/06-skip-link.js"),
    include_str!("../../assets/js/code-enhance/07-keyboard.js"),
    include_str!("../../assets/js/code-enhance/08-copy-buttons.js"),
    include_str!("../../assets/js/code-enhance/09-register.js"),
    include_str!("../../assets/js/code-enhance/13-reader-menu.js"),
    include_str!("../../assets/js/code-enhance/14-reader-prefs.js"),
    include_str!("../../assets/js/code-enhance/16-scroll-a11y.js"),
);
/// The enhancer registry (`window.taliEnhancers` + `taliEnhanceCode`) on its own, so the
/// External build path can emit it INLINE at parse (before any `include-after-body`
/// extension script that calls `taliEnhancers.register`), while the shared app.js stays
/// deferred. This double-includes `01-registry.js` at compile time (once here, once inside
/// `CODE_ENHANCE_JS`'s `concat!` above); keep BOTH copies, since dropping it from
/// `CODE_ENHANCE_JS` would drift the Inline path's byte output. The IIFE is idempotent
/// (`if (window.taliEnhancers) return;`), so app.js's bundled copy no-ops on its later run.
const REGISTRY_JS: &str = include_str!("../../assets/js/code-enhance/01-registry.js");
const MERMAID_JS: &str = include_str!("../../assets/js/mermaid.js");
/// The vendored Mermaid library (pinned mermaid@11.16.0, ~3.5 MB; sets `globalThis.mermaid`).
/// Inlined into a static Build page that has a diagram so it renders with no CDN; the
/// live Preview keeps the lazy loader instead (see `code_scripts_for`).
const MERMAID_MIN_JS: &str = include_str!("../../assets/js/mermaid.min.js");

/// The raw framework CSS a non-bare site page inlines in its main `<style>` (fonts +
/// tokens + base + dark + site chrome). Exposed so the multi-page build can externalize it
/// into one content-hashed `_assets/app.<hash>.css` instead of inlining a copy per page.
pub fn shared_site_css() -> String {
    format!("{FONTS_CSS}{TOKENS_CSS}{TOKENS_DARK_CSS}{BASE_CSS}{DARK_CSS}{SITE_CSS}")
}

/// [`shared_site_css`] with the body typeface **linked** rather than inlined: the same
/// sheet minus 160 KB of base64, referencing the faces `hrefs` names. For a build that
/// writes [`FONT_FILES`] beside it in `_assets/` (item 150).
pub fn shared_site_css_linked_fonts(hrefs: &[(&str, String)]) -> String {
    format!(
        "{}{TOKENS_CSS}{TOKENS_DARK_CSS}{BASE_CSS}{DARK_CSS}{SITE_CSS}",
        fonts_css_linked(hrefs)
    )
}

/// The KaTeX stylesheet (base64 fonts inlined), for the externalized `katex.<hash>.css`.
pub fn katex_css() -> &'static str {
    KATEX_CSS
}

/// The base framework stylesheet (layout + reader chrome), for tests that need to
/// assert a retired feature's CSS is gone without reaching into a full page render.
pub fn base_css() -> &'static str {
    BASE_CSS
}

/// The site-chrome stylesheet (nav, book chrome, listings, TOC), for tests that need
/// to assert a retired feature's CSS is gone without reaching into a full page render.
pub fn site_css() -> &'static str {
    SITE_CSS
}

/// All of Taliesin's OWN page JS, concatenated for the always-on `app.<hash>.js`. Each
/// piece is separated by a bare `;` on its own line so concatenation is ASI-safe. The
/// big vendored libs (mermaid, d3, Plot) are deliberately excluded (their own files), and
/// so is the `{js}`-cell runtime (`TALIESIN_JS`): it runs each cell via `new
/// AsyncFunction(..., src)`, whose dynamic `import()` resolves the specifier relative to
/// the SCRIPT that called the constructor. Folded into the shared `/_assets/app.js`, a
/// cell's `import("./helper.js")` would wrongly resolve against `/_assets/` (a 404), so the
/// External page keeps that runtime INLINE instead (see `assemble_html_page`), anchoring
/// the resolution base to the page itself.
pub fn core_enhance_js() -> String {
    [CODE_ENHANCE_JS, TOC_SPY_JS, SEARCH_JS].join("\n;\n")
}

/// The vendored mermaid library plus its loader (CDN placeholder already resolved), for
/// the conditional `mermaid.<hash>.js`. Ships only on pages that have a diagram, so the
/// loader's never-reached CDN fallback stays off prose pages.
pub fn mermaid_bundle_js() -> String {
    format!(
        "{MERMAID_MIN_JS}\n;\n{}",
        MERMAID_JS.replace("{{MERMAID}}", &mermaid_url_for(OutputMode::Build))
    )
}

/// The `{js}` drawing globals for the conditional `jslibs.<hash>.js` (ships only on pages
/// with `{js}` cells): vendored d3 + Observable Plot. **Must stay the External-mode twin of
/// [`js_cell_head`]** — a global present on one path and absent on the other is a cell that
/// works in preview and is `undefined` in the built site, which no render test would see.
pub fn js_cell_libs_js() -> String {
    format!("{D3_JS}\n;\n{PLOT_JS}")
}

/// True if a rendered body contains a mermaid diagram (gates the mermaid file link).
pub fn has_mermaid(body: &str) -> bool {
    body.contains("class=\"mermaid\"")
}

/// Heading level (1–6) for a block whose root element is `<hN ...>`/`<hN>`.
pub(crate) fn block_heading_level(html: &str) -> Option<u8> {
    let b = html.as_bytes();
    if b.len() >= 4 && b[0] == b'<' && b[1] == b'h' && b[2].is_ascii_digit() {
        let lvl = b[2] - b'0';
        if (1..=6).contains(&lvl) && matches!(b[3], b' ' | b'>') {
            return Some(lvl);
        }
    }
    None
}

/// GitHub-style slug for a heading's visible text, used as the `<section>` id.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if pending_dash {
                out.push('-');
                pending_dash = false;
            }
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() {
            pending_dash = true;
        }
    }
    out
}

/// Remove `$…$` / `$$…$$` math spans from a heading's markdown text before slugging, so
/// KaTeX/LaTeX (`$H_0$`) doesn't leak into the anchor id (`…-h-0`). Mirrors comrak's
/// `math_dollars` rule closely enough for a slug: an opening `$` is not followed by
/// whitespace and its matching closing `$` is not preceded by whitespace, `\$` is a
/// literal dollar (not a delimiter), and a span never crosses a newline. So a real math
/// span drops out while a lone/currency `$` (e.g. `$5 and $10`, which comrak also leaves
/// as text) stays put.
fn strip_math_for_slug(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        // A backslash escape (`\$`, `\\`, …) is literal: copy the pair, never a delimiter.
        if chars[i] == '\\' {
            out.push(chars[i]);
            if let Some(&next) = chars.get(i + 1) {
                out.push(next);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if chars[i] == '$' {
            let display = chars.get(i + 1) == Some(&'$');
            let open_len = if display { 2 } else { 1 };
            let body = i + open_len;
            // A span needs a body; inline `$…$` also forbids whitespace right after the
            // opening `$` (display `$$…$$` allows it, matching comrak). If a valid close
            // exists, drop the whole span; otherwise the `$` is literal (fall through).
            let open_ok = body < chars.len() && (display || !chars[body].is_whitespace());
            if open_ok && let Some(close) = math_close(&chars, body, display) {
                i = close + open_len; // skip delimiters + body
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Index of the matching closing `$` (inline) or first `$` of the closing `$$` (display)
/// for a math span whose body starts at `start`; `None` if the opening `$` isn't a math
/// delimiter after all. Mirrors comrak's inline scan: the close is the FIRST unescaped
/// `$`, and if that `$` is preceded by whitespace or followed by an ASCII digit the span
/// is abandoned (the opening `$` stays literal) rather than reaching for a later `$` —
/// else a lone/currency `$` would greedily swallow the text up to some later real span.
fn math_close(chars: &[char], start: usize, display: bool) -> Option<usize> {
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '\n' => return None,
            // `\` escapes the next char (`\$` is a literal dollar, never a delimiter).
            '\\' => {
                j += 2;
                continue;
            }
            // Display close: the first `$$` (comrak is lenient about adjacency here).
            '$' if display => {
                if chars.get(j + 1) == Some(&'$') {
                    return Some(j);
                }
            }
            // Inline close: THIS first unescaped `$` is the only candidate. `j > start`
            // (the caller guarantees `chars[start]` is neither whitespace nor `$`), so
            // `j - 1` is in range.
            '$' => {
                let ok = !chars[j - 1].is_whitespace()
                    && !chars.get(j + 1).is_some_and(|c| c.is_ascii_digit());
                return ok.then_some(j);
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// A deduped heading anchor slug; a repeated slug gets a `-N` suffix.
/// `block_src` is the heading's markdown line; `slugify` ignores the leading
/// `#`s and markup, yielding the visible-text slug.
fn dedup_slug(block_src: &str, counts: &mut HashMap<String, u32>) -> String {
    let base = slugify(block_src);
    let base = if base.is_empty() {
        "section".to_string()
    } else {
        base
    };
    dedup_with_suffix(base, counts)
}

/// Make `base` unique within `counts`: the first occurrence is `base`, each repeat
/// gets a `-N` suffix (`base`, `base-1`, `base-2`, …). Shared by heading-slug and
/// block-id deduplication.
fn dedup_with_suffix(base: String, counts: &mut HashMap<String, u32>) -> String {
    let n = counts.entry(base.clone()).or_insert(0);
    let out = if *n == 0 {
        base.clone()
    } else {
        format!("{base}-{n}")
    };
    *n += 1;
    out
}

/// The line of a heading block that carries a trailing `{...}` attribute. For an
/// ATX heading (`## Title {#id}`) that's the whole line; for a setext heading
/// (`Title {#id}` above a `===`/`---` rule) the attribute sits on the text line, so
/// return that, not the underline. Only called when the block is a heading.
fn heading_attr_line(block_src: &str) -> &str {
    let trimmed = block_src.trim_end();
    if let Some((above, rule)) = trimmed.rsplit_once('\n') {
        let r = rule.trim();
        if !r.is_empty() && (r.bytes().all(|b| b == b'=') || r.bytes().all(|b| b == b'-')) {
            // Setext underline: the attribute is on the last text line above it.
            return above.trim_end().lines().last().unwrap_or(above).trim_end();
        }
    }
    trimmed
}

/// A trailing Pandoc attribute on a heading line (`## Title {#id .class}`).
/// Returns `(text_without_attr, explicit_id)`, or `None` when there is no attr.
pub(crate) fn parse_heading_attr(block_src: &str) -> Option<(String, Option<String>)> {
    let line = heading_attr_line(block_src);
    let open = line.rfind('{')?;
    if !line.ends_with('}') {
        return None;
    }
    let inner = &line[open + 1..line.len() - 1];
    // Require an id or class so we don't strip a heading that merely ends in `}`.
    if !(inner.starts_with('#') || inner.starts_with('.')) {
        return None;
    }
    let id = inner
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix('#'))
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    Some((line[..open].trim_end().to_string(), id))
}

/// Remove the trailing `{...}` attribute comrak leaves as literal text inside a
/// rendered heading (heading attributes aren't CommonMark, so it doesn't consume
/// them). Only called when [`parse_heading_attr`] found one.
fn strip_heading_attr(html: &str) -> String {
    let Some(close) = html.rfind("</h") else {
        return html.to_string();
    };
    let inner = &html[..close];
    if inner.trim_end().ends_with('}')
        && let Some(open) = inner.rfind('{')
    {
        return format!("{}{}", inner[..open].trim_end(), &html[close..]);
    }
    html.to_string()
}

/// Apply a Pandoc attribute block trailing a link — `<a ...>text</a>{.btn #id}`
/// — onto the `<a>` (merging classes + setting an id) and drop the literal `{...}`
/// comrak leaves as text. The inline analogue of [`strip_heading_attr`]; only
/// `.class`/`#id` blocks are consumed (anything else, e.g. `{x}`, is left as-is).
fn apply_link_attrs(html: &str) -> String {
    if !html.contains("</a>") {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("</a>") {
        let end = pos + "</a>".len();
        out.push_str(&rest[..end]);
        let after = &rest[end..];
        let trimmed = after.trim_start();
        if let Some(body) = trimmed.strip_prefix('{')
            && let Some(close) = body.find('}')
            && let Some((classes, id)) = parse_pandoc_attrs(&body[..close])
        {
            inject_attrs_into_last_tag(&mut out, "<a ", &classes, id.as_deref());
            rest = &body[close + 1..];
            continue;
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Drop a literal backslash left right before a block-closing tag — a trailing
/// `\` hard break that comrak keeps (CommonMark) but Pandoc drops (e.g. a
/// CV line ending `2025–2027 \`). Scoped to block closers so inline `\` is untouched.
fn strip_trailing_hardbreak(html: &str) -> String {
    const TAGS: [&str; 11] = [
        "</p>",
        "</li>",
        "</h1>",
        "</h2>",
        "</h3>",
        "</h4>",
        "</h5>",
        "</h6>",
        "</blockquote>",
        "</td>",
        "</dd>",
    ];
    let trimmed = html.trim_end();
    for tag in TAGS {
        let Some(prefix) = trimmed.strip_suffix(tag) else {
            continue;
        };
        // A trailing hard-break backslash sits right before the block's closing tag
        // (`text\</p>`): CommonMark keeps it literal, Pandoc drops it. Anchored
        // to the block end, so raw-HTML content containing `\</p>` mid-block (not a
        // hardbreak) is left untouched — the previous global replace could corrupt it.
        let stripped = prefix
            .strip_suffix('\\')
            .or_else(|| prefix.strip_suffix("\\ "));
        return match stripped {
            Some(p) => format!("{p}{tag}{}", &html[trimmed.len()..]),
            None => html.to_string(),
        };
    }
    html.to_string()
}

/// Insert `class`/`id` attributes into the last opening `tag` already written to
/// `out` (e.g. the `<a ` that precedes a just-emitted `</a>`).
fn inject_attrs_into_last_tag(out: &mut String, tag: &str, classes: &[String], id: Option<&str>) {
    let Some(start) = out.rfind(tag) else {
        return;
    };
    let Some(rel_gt) = out[start..].find('>') else {
        return;
    };
    let gt = start + rel_gt;
    let mut ins = String::new();
    if !classes.is_empty() {
        ins.push_str(&format!(" class=\"{}\"", escape_attr(&classes.join(" "))));
    }
    if let Some(i) = id {
        ins.push_str(&format!(" id=\"{}\"", escape_attr(i)));
    }
    out.insert_str(gt, &ins);
}

/// Fold Pandoc table captions into their tables. A `: caption {#tbl-x}` paragraph
/// directly after a table becomes the table's numbered `<caption>` ("Table N"),
/// the table gains the `#tbl-x` id, and `tbl-x` is registered so `@tbl-x` resolves.
/// A float's displayed number: chapter-scoped ("2.3") inside a numbered book chapter,
/// else the flat count ("3"). Figures/tables/equations/listings each keep their own
/// counter and scope it to the chapter, so two chapters no longer both open with a
/// "Figure 1" and a cross-chapter `@fig-` ref is unambiguous.
///
/// There is no knob: outside a numbered chapter there is simply no chapter to scope to,
/// so numbering stays flat — the same rule `section_number` already follows. Every float
/// shares this one helper, so a chapter cannot show "Figure 2.3" beside "Table 5".
fn float_number(chapter: Option<u32>, n: usize) -> String {
    match chapter {
        Some(ch) => format!("{ch}.{n}"),
        None => n.to_string(),
    }
}

/// Register a cross-reference anchor → number, keeping the **first** definition and
/// warning on a duplicate label. Otherwise a repeated `{#fig-x}`/`{#sec-x}` silently
/// took the *last* number while the `#fig-x` anchor pointed at the *first* element —
/// so `@fig-x` and the link target disagreed, with no diagnostic.
fn register_xref(
    reg: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    anchor: &str,
    number: String,
    file: Option<&str>,
    line: u32,
) {
    if reg.contains_key(anchor) {
        // Locate the DUPLICATE (this second definition), like the "duplicate heading id"
        // warning beside it — an unlocated duplicate-label warning half-reproduces the
        // Quarto flaw D53 critiques.
        warnings.push(
            Warning::new(format!(
                "duplicate cross-reference label \u{201c}{anchor}\u{201d} (using the first definition)"
            ))
            .at(file.map(str::to_string), line)
            .severity(Severity::Error),
        );
    } else {
        reg.insert(anchor.to_string(), number);
    }
}

/// The 1-based start line of a `L:C-L:C` sourcepos, or 0 when it carries none (a generated
/// block with an empty sourcepos — not click-to-source anyway, and `locatable()` requires
/// a `[1-9]` line, so 0 reads as "no location").
pub fn sourcepos_start_line(sp: &str) -> u32 {
    sp.split(':')
        .next()
        .and_then(|l| l.parse().ok())
        .unwrap_or(0)
}

/// A labelled cell whose output the executor will never emit (`#| include: false`) has
/// nothing to carry its anchor, so the label is unreachable. Warn at the cell rather than
/// at the reference site: its own "broken cross-reference: @fig-x"
/// reads as a lie to an author looking straight at the `label: fig-x` they wrote, and an
/// unreferenced one would otherwise die silently. `kind` names the construct ("figure",
/// "table") so the message says which label it means.
fn unreferenceable_hidden_label(
    kind: &str,
    anchor: &str,
    file: Option<String>,
    line: usize,
) -> Warning {
    Warning::new(format!(
        "{kind} label \u{201c}{anchor}\u{201d} cannot be cross-referenced: `include: false` drops \
         the cell's output, so nothing carries the anchor and `@{anchor}` won't resolve"
    ))
    .at(file, line as u32)
}

/// A labelled figure/table cell whose language Taliesin never runs (`{bash}`, `{sql}`,
/// `{julia}`, …) can't produce the output the label points at. Unlike the `include:
/// false` case, the source still shows; the label just cannot resolve, so say why (a
/// listing label `lst-*` would work, since a listing IS the source).
fn unreferenceable_nonexec_label(
    kind: &str,
    anchor: &str,
    lang: &str,
    file: Option<String>,
    line: usize,
) -> Warning {
    Warning::new(format!(
        "{kind} label \u{201c}{anchor}\u{201d} cannot be cross-referenced: `{{{lang}}}` is not \
         executed, so it produces no {kind} output and `@{anchor}` won't resolve"
    ))
    .at(file, line as u32)
}

fn apply_table_captions(
    blocks: &mut Vec<Block>,
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    chapter: Option<u32>,
) {
    let mut tbl_count = 0usize;
    let mut i = 0;
    while i < blocks.len() {
        // The current block's location, captured before any mutable borrow of it, so a
        // duplicate-label warning can point at it (click-to-source).
        let bfile = blocks[i].source_file.clone();
        let bline = sourcepos_start_line(&blocks[i].sourcepos);
        // A code cell whose executed output is a numbered table (`#| label: tbl-x`):
        // assign its number in document order (so it interleaves correctly with
        // Markdown tables) and register the xref. The executor injects the matching
        // caption/id into the output using `cell.table.number`.
        if let Some(t) = blocks[i].cell.as_mut().and_then(|c| c.table.as_mut()) {
            tbl_count += 1;
            let num = float_number(chapter, tbl_count);
            t.number = num.clone();
            if let Some(a) = &t.anchor {
                register_xref(xrefs, warnings, a, num, bfile.as_deref(), bline);
            }
            i += 1;
            continue;
        }
        // A Markdown table directly followed by a `: caption {#tbl-x}` paragraph.
        if i + 1 < blocks.len()
            && blocks[i].html.starts_with("<table")
            && let Some((caption_html, id)) = parse_table_caption(&blocks[i + 1].html)
        {
            tbl_count += 1;
            let tbl_num = float_number(chapter, tbl_count);
            if let Some(id) = &id {
                // The `{#tbl-x}` label is authored on the caption paragraph (blocks[i+1]),
                // so locate the warning there.
                register_xref(
                    xrefs,
                    warnings,
                    id,
                    tbl_num.clone(),
                    blocks[i + 1].source_file.as_deref(),
                    sourcepos_start_line(&blocks[i + 1].sourcepos),
                );
            }
            let sep = if caption_html.is_empty() { "" } else { ": " };
            let id_attr = id_attr(id.as_deref());
            // Insert the `id` on the <table> and a <caption> as its first child.
            let table = &blocks[i].html;
            let gt = table.find('>').unwrap_or(0) + 1;
            let open = table[..gt].replacen("<table", &format!("<table{id_attr}"), 1);
            blocks[i].html = format!(
                "{open}<caption>Table&nbsp;{tbl_num}{sep}{caption_html}</caption>{}",
                &table[gt..],
            );
            blocks.remove(i + 1); // the caption paragraph is now folded in
        }
        i += 1;
    }
}

/// Parse a table-caption paragraph (`<p ...>: caption {#tbl-x}</p>`): returns the
/// caption's inner HTML (markers stripped) and an explicit `#id`, or `None`.
fn parse_table_caption(p_html: &str) -> Option<(String, Option<String>)> {
    if !(p_html.starts_with("<p ") || p_html.starts_with("<p>")) {
        return None; // a paragraph (not <pre>, etc.)
    }
    let gt = p_html.find('>')?;
    let body = p_html[gt + 1..].strip_suffix("</p>")?.trim_start();
    let body = body.strip_prefix(':')?.trim(); // the `: caption` marker
    Some(match parse_heading_attr(body) {
        Some((clean, id)) => (clean.trim().to_string(), id),
        None => (body.to_string(), None),
    })
}

// --- figures -------------------------------------------------------------

// --- table of contents ---------------------------------------------------

/// Build a `<nav id="TOC">` from the document's heading blocks (levels 1–3),
/// linking to their anchor ids. Empty when the doc has no anchored headings.
/// The headings the on-page TOC shows: those carrying an anchor `id`, within two levels
/// of the shallowest heading present (`level - base <= 2`). `toc_entry_count` and
/// `toc_html` share this so their filters cannot drift, and so a title-demoted page
/// (whose sections start at `<h2>`) still surfaces three levels instead of two.
fn toc_items(blocks: &[Block]) -> Vec<(u8, String, String)> {
    let all: Vec<(u8, String, String)> = blocks
        .iter()
        .filter_map(|b| {
            Some((
                block_heading_level(&b.html)?,
                extract_attr(&b.html, "id")?,
                strip_tags(&b.html),
            ))
        })
        .collect();
    let Some(base) = all.iter().map(|(l, _, _)| *l).min() else {
        return Vec::new();
    };
    // `base` is the minimum, so `*l >= base` always: the subtraction never underflows.
    all.into_iter().filter(|(l, _, _)| *l - base <= 2).collect()
}

/// How many entries the table of contents would list (exactly the set [`toc_html`]
/// renders). The site auto-gates the "on this page" TOC on this count — a short /
/// sequential page reads as one column; only long, chunkable pages earn the sidebar
/// TOC (NN/g).
pub(crate) fn toc_entry_count(blocks: &[Block]) -> usize {
    toc_items(blocks).len()
}
fn toc_html(blocks: &[Block]) -> String {
    let items = toc_items(blocks);
    if items.is_empty() {
        return String::new();
    }
    let base = items.iter().map(|(l, _, _)| *l).min().unwrap();
    let mut out = String::from("<nav id=\"TOC\" class=\"tali-toc\" role=\"doc-toc\"><ul>");
    let mut level = base;
    let mut open_li = false;
    for (lvl, id, text) in &items {
        let lvl = (*lvl).max(base);
        if lvl > level {
            // Descend: nest a <ul> inside the open parent <li>. When heading levels
            // are skipped (e.g. h1 -> h3, or the first heading is deeper than the
            // base) there is no <li> to hold the next <ul>, so emit a filler <li> —
            // a <ul> may only contain <li>, never another <ul> directly.
            while level < lvl {
                if !open_li {
                    out.push_str("<li>");
                }
                out.push_str("<ul>");
                level += 1;
                open_li = false; // the freshly opened <ul> has no <li> yet
            }
        } else {
            if open_li {
                out.push_str("</li>");
            }
            while level > lvl {
                out.push_str("</ul></li>");
                level -= 1;
            }
        }
        out.push_str(&format!(
            // NEITHER half may be escaped again. `text` is `strip_tags` output and `id`
            // is `extract_attr` output: both are read back out of already-escaped heading
            // HTML, so a second pass turns `&amp;` into `&amp;amp;`. On `text` that is a
            // visible typo; on `id` it is a DEAD LINK in the published build, because the
            // href stops matching the anchor the heading actually carries
            // (`## R&D notes {#r&d-notes}` -> anchor `r&amp;d-notes`, href
            // `#r&amp;amp;d-notes`). `escape_attr_from_html` is the attribute-context
            // counterpart: it escapes `"` and leaves existing entities alone.
            "<li><a href=\"#{}\">{text}</a>",
            escape_attr_from_html(id),
        ));
        open_li = true;
    }
    if open_li {
        out.push_str("</li>");
    }
    while level > base {
        out.push_str("</ul></li>");
        level -= 1;
    }
    out.push_str("</ul></nav>");
    out
}

/// Read the value of an HTML attribute from a start tag (e.g. `id="..."`).
fn extract_attr(html: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = html.find(&needle)? + needle.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

// --- emitter -------------------------------------------------------------

// --- block ids -----------------------------------------------------------

/// Build a stable block id from its source text, with a positional tiebreak
/// so duplicate-content blocks still get distinct ids.
fn make_id(block_src: &str, counts: &mut HashMap<String, u32>) -> String {
    // Block ids and the freeze cache key share one hashing scheme; see `crate::hash`.
    let hex = format!("{:016x}", crate::hash::fnv1a(block_src.trim()));
    let base = format!("b-{}", &hex[..12]);
    dedup_with_suffix(base, counts)
}

/// How far a titled page's body headings move so the shallowest one emits as `<h2>`:
/// directly under the title block's `<h1>`, with no gap. `None` for a page with no
/// headings, or when the shift is zero.
///
/// **Relative to the page, not an absolute `+1`** (AP7-1). An absolute `+1` is correct
/// only for a `#`-rooted page. The house style of both dogfood books is `##`-rooted — a
/// `#` would just restate the front-matter `title:` — so `+1` put their first section at
/// `<h3>` under an `<h1>`, and a page opening at `###` landed at `<h4>`: 37 of 51 book
/// pages emitted an outline with a hole in it, which is exactly what heading-level
/// navigation walks. The on-page TOC already windowed relative to the shallowest heading
/// present, so the two disagreed about the page's shape.
///
/// The shift can be negative (a `###`-rooted page promotes to `<h2>`); since every level
/// is at least `base`, nothing can land above `<h2>` and collide with the title.
fn heading_shift_for(levels: &[usize]) -> Option<i8> {
    let base = levels.iter().copied().min()? as i8;
    (base != 2).then_some(2 - base)
}

/// Record where each heading's section **ends**, as `data-section-end="<block-id>"` on
/// the heading block: the id of the last block the section covers, the heading itself
/// included. The DOM otherwise has no idea where a section stops — blocks are flat
/// siblings of one root, and nothing wraps a heading-to-next-heading run — so anything
/// wanting to *enumerate* a section (per-section length, section-scoped read state or
/// change marks, a JS-driven fold) had to re-derive the boundaries from tag names.
///
/// **Extents nest.** A section ends at the next heading of the same level or shallower,
/// so an `##` section contains its `###` subsections. That direction keeps information:
/// the flat heading-to-next-heading run is recoverable from the next heading, the
/// nesting is not.
///
/// A heading always covers at least itself, so an empty section (a heading immediately
/// followed by a sibling heading, or one ending the document) points at its own id and
/// no consumer needs a missing-value case.
///
/// **Generated trailing blocks belong to no section.** References and the footnotes
/// block are appended after the body and carry no sourcepos; the last section would
/// otherwise swallow them, claiming document furniture as its own content.
///
/// One consequence worth stating rather than discovering: this makes a heading block's
/// HTML depend on the id of the last block of its section, so editing that last block
/// re-emits its enclosing headings as `Update` ops. That is a handful of extra ops on
/// edits at a section boundary, and it is the cheaper of the two couplings available —
/// marking each *body* block with its heading instead would re-emit an entire section
/// every time its heading's text changed.
fn mark_section_extents(blocks: &mut [Block]) {
    let heads: Vec<(usize, u8)> = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| block_heading_level(&b.html).map(|l| (i, l)))
        .collect();
    if heads.is_empty() {
        return;
    }
    let body_end = blocks
        .iter()
        .rposition(|b| !b.sourcepos.is_empty())
        .unwrap_or(blocks.len() - 1);
    for (n, &(i, level)) in heads.iter().enumerate() {
        let end = heads[n + 1..]
            .iter()
            .find(|&&(_, l)| l <= level)
            .map_or(blocks.len() - 1, |&(j, _)| j - 1)
            .min(body_end)
            // A floor, not a behaviour: no input reaches it today (a heading parsed from
            // source carries a sourcepos, so `body_end >= i` for every real heading, and
            // the next-heading branch cannot land below `i` either). Kept, and marked as
            // unreachable rather than pinned by a test that could only pass vacuously, so
            // that a future *generated* heading block appended past the body produces a
            // degenerate self-extent instead of a silently backwards one.
            .max(i);
        let id = blocks[end].id.clone();
        let html = &mut blocks[i].html;
        // Append to the opening tag rather than inserting after `<hN`: `id`,
        // `data-block-id` and `data-sourcepos` lead a heading tag in a fixed order that
        // tests and the client both read, and a new attribute has no business splitting
        // it. `open_tag_end` is quote-aware, so a `>` inside an authored attribute value
        // cannot be mistaken for the end of the tag.
        if let Some(at) = open_tag_end(html) {
            html.insert_str(at, &format!(" data-section-end=\"{}\"", escape_attr(&id)));
        }
    }
}

/// The byte offset of the `>` closing an element's opening tag, skipping any `>` inside
/// a quoted attribute value. `None` if the tag never closes.
fn open_tag_end(html: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, b) in html.bytes().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// Move a heading block's visible tag by `shift` levels (`<hN>` -> `<h{N+shift}>`, clamped
/// to 1..=6), leaving its attributes, `id`, `data-block-id`, `data-sourcepos` and text
/// untouched. Used when a page renders a title-block `<h1 class="title">` so its body
/// sections nest beneath the single page title: one `<h1>` per page (a11y + SEO).
fn shift_heading_html(html: &str, level: u8, shift: i8) -> String {
    let to = (level as i8 + shift).clamp(1, 6) as u8;
    if to == level {
        return html.to_string();
    }
    // `html` is `<hN...>...</hN>`: rewrite only the opening tag name (at index 0) and the
    // lone closing tag. Heading text has its `<`/`>` escaped to entities, so the literal
    // `</hN>` appears exactly once (the real closing tag).
    html.replacen(&format!("<h{level}"), &format!("<h{to}"), 1)
        .replacen(&format!("</h{level}>"), &format!("</h{to}>"), 1)
}

// --- helpers -------------------------------------------------------------

/// Parsed fenced-div attributes. Kept here (rather than in the `divs` submodule)
/// because the sibling `figure` module reads its fields via descendant access.
#[derive(Default)]
pub(crate) struct DivAttrs {
    classes: Vec<String>,
    id: Option<String>,
    kv: Vec<(String, String)>,
}

impl DivAttrs {
    /// The `#id` parsed from the attribute block, if any. `pub(crate)` so the
    /// cross-page xref scanner (`site::xref`) reuses this one quote-aware parser
    /// instead of re-implementing brace scanning and drifting from the renderer.
    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn get(&self, key: &str) -> Option<&str> {
        self.kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    fn callout_kind(&self) -> Option<&str> {
        self.classes.iter().find_map(|c| c.strip_prefix("callout-"))
    }
}

fn is_heading(html: &str) -> bool {
    html.starts_with("<h") && html.as_bytes().get(2).is_some_and(u8::is_ascii_digit)
}

/// Index of the `>` that closes an element's opening tag, skipping any `>` inside
/// a quoted attribute value (so `<a title="a>b">` returns the *final* `>`, not the
/// one in the title). `None` if the tag is unterminated. Used by the string-surgery
/// helpers that splice a class/attribute into an already-emitted opening tag — a
/// naive `find('>')` would split inside an attribute value.
pub(crate) fn tag_end(html: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, &b) in html.as_bytes().iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// Strip HTML tags, returning the visible text (callout/tabset titles, TOC entries,
/// figure alt-text, heading slugs). Quote-aware, like [`tag_end`]: a `>` inside a quoted
/// attribute value (e.g. KaTeX's `<span title="a>b">`) does NOT end the tag, so the
/// visible text isn't truncated mid-attribute.
///
/// KaTeX-aware: with the default `htmlAndMathml` output, KaTeX renders inline math
/// three times — the MathML semantic text, a raw-TeX `<annotation>`, and the visible
/// `katex-html` glyphs. Emitting all three triples a heading's TOC label / slug and
/// leaks LaTeX (`$H_0$` → `H0H_0H0`). So the whole `<math>…</math>` subtree is dropped,
/// leaving only the visible `katex-html` glyphs (`H0`).
fn strip_tags(html: &str) -> String {
    strip_tags_inner(html, Separate::Never)
}

/// [`strip_tags`], but with a space at every tag boundary, so text from *adjacent
/// blocks* stays word-separated when a run of block HTML is read as one string (the
/// search index's case: `<p>First.</p><p>Second.</p>` must not fuse into
/// "First.Second."). The TOC/slug path must NOT do this — a space there would split
/// `<em>Fig</em>ure` into two words and change every slug.
fn strip_tags_separated(html: &str) -> String {
    strip_tags_inner(html, Separate::EveryTag)
}

/// Where [`strip_tags_inner`] leaves a word boundary behind.
#[derive(Clone, Copy, PartialEq)]
enum Separate {
    /// Never — the TOC/slug path, where a space changes every slug.
    Never,
    /// At every tag — a run of blocks read as one string (the search index).
    EveryTag,
}

fn strip_tags_inner(html: &str, separate: Separate) -> String {
    let mut out = String::new();
    let mut skip_math = 0usize; // depth of `<math>` subtrees whose text is dropped
    let mut chars = html.chars();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Consume the tag body up to the closing `>` (quote-aware: a `>` inside a
            // quoted attribute value does not end the tag).
            let mut tag = String::new();
            let mut quote: Option<char> = None;
            for c in chars.by_ref() {
                match quote {
                    Some(q) => {
                        if c == q {
                            quote = None;
                        }
                        tag.push(c);
                    }
                    None => match c {
                        '"' | '\'' => {
                            quote = Some(c);
                            tag.push(c);
                        }
                        '>' => break,
                        _ => tag.push(c),
                    },
                }
            }
            // Enter/exit the KaTeX `<math>` MathML subtree (depth-tracked for safety).
            let body = tag.trim_start();
            let is_close = body.starts_with('/');
            let name: String = body
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect();
            // Decided from the tag NAME, so it has to follow the parse above rather than
            // precede it. Nothing else is pushed in between, so the space still lands
            // exactly where the tag was.
            let boundary = match separate {
                Separate::Never => false,
                Separate::EveryTag => true,
            };
            // Never double a boundary that is already there. `</span> <span>` carries a
            // real space of its own, and pushing a second one publishes "models.  14 April".
            if boundary && !out.ends_with(char::is_whitespace) {
                out.push(' ');
            }
            if name == "math" {
                if is_close {
                    skip_math = skip_math.saturating_sub(1);
                } else if !tag.trim_end().ends_with('/') {
                    skip_math += 1;
                }
            }
        } else if skip_math == 0 {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// The plain text of a document's leading `# H1`, when that H1 is the document's *first*
/// block. A later or deeper heading is a section, not the document's name, so only the
/// first block qualifies.
///
/// The returned text is decoded, not HTML: `<title>` escapes its input, so a heading
/// carrying `&amp;` would otherwise ship as `&amp;amp;`.
fn leading_h1_text(blocks: &[Block]) -> Option<String> {
    let first = blocks.first()?;
    (block_heading_level(&first.html)? == 1)
        .then(|| unescape_html(&strip_tags(&first.html)))
        .filter(|t| !t.is_empty())
}

/// Reverse [`escape_html`]: decode the entities the renderer itself emits (`&amp;`,
/// `&lt;`, `&gt;`, `&quot;`, `&#39;`). Not a general HTML entity decoder — it exists so
/// text lifted back out of emitted HTML can be re-escaped exactly once.
fn unescape_html(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = [
            ("&amp;", '&'),
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&quot;", '"'),
            ("&#39;", '\''),
        ]
        .into_iter()
        .find(|(ent, _)| tail.starts_with(ent));
        match decoded {
            Some((ent, ch)) => {
                out.push(ch);
                rest = &tail[ent.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Escape a string for HTML *text* content (`&`, `<`, `>`). For attribute values
/// (which also need `"`), use [`escape_attr`]. Shared with the server crate's
/// executor/kernel output rendering so escaping is defined once.
pub fn html_escape(s: &str) -> String {
    let mut out = String::new();
    escape_html(s, &mut out);
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// If a block's source is entirely a LaTeX math environment
/// (`\begin{env} ... \end{env}`), return it for display-math rendering.
fn bare_math_env(block_src: &str) -> Option<&str> {
    let t = block_src.trim();
    (t.starts_with("\\begin{") && t.contains("\\end{") && t.ends_with('}')).then_some(t)
}

/// A labelled display equation: `$$ ... $$ {#eq-x}`. Returns the LaTeX
/// body and the `eq-x` anchor. Only `#eq-`-prefixed labels qualify (other
/// attribute blocks after `$$` are left to the normal math path).
fn labelled_display_eq(block_src: &str) -> Option<(String, String)> {
    let t = block_src.trim();
    let body = t.strip_prefix("$$")?;
    let close = body.rfind("$$")?;
    let (latex, after) = body.split_at(close);
    let attr = after.strip_prefix("$$")?.trim();
    let inner = attr.strip_prefix('{')?.strip_suffix('}')?;
    let anchor = inner
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix('#'))?;
    anchor
        .starts_with("eq-")
        .then(|| (latex.trim().to_string(), anchor.to_string()))
}

/// Render a numbered display equation: the KaTeX body plus a right-aligned
/// `(N)` number, carrying the `#eq-` id so `@eq-x` cross-refs link to it.
fn emit_equation(latex: &str, anchor: &str, block_attrs: &str, num: &str) -> String {
    format!(
        "<div id=\"{anchor}\"{block_attrs} class=\"tali-eqn\">\
         <span class=\"tali-eqn-body\">{}</span>\
         <span class=\"tali-eqn-number\">({num})</span></div>",
        crate::math::render(latex, true)
    )
}

/// The raw output format of a Pandoc passthrough fence: `{=html}` -> "html".
fn raw_block_format(info: &str) -> Option<String> {
    info.trim()
        .strip_prefix("{=")
        .and_then(|s| s.strip_suffix('}'))
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| !f.is_empty())
}

/// Minimal standard-alphabet base64 (mirrors `build.rs`); used to inline the
/// favicon as a `data:` URI (see [`page::favicon_link`]).
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(T[(n >> 18 & 63) as usize] as char);
        s.push(T[(n >> 12 & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

fn escape_html(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// Make already-entity-escaped HTML *text* (e.g. a rendered caption with its tags
/// stripped via [`strip_tags`]) safe inside a double-quoted attribute. Existing
/// entities are valid in an attribute value, so only the `"` needs escaping —
/// running [`escape_attr`] here would double-escape `&` (`&amp;` -> `&amp;amp;`).
pub(crate) fn escape_attr_from_html(s: &str) -> String {
    s.replace('"', "&quot;")
}

/// Escape a string for an HTML *attribute* value (`&`, `<`, `>`, `"`). For text
/// content, use [`html_escape`].
pub fn escape_attr(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Multi-page site chrome: a sticky theme-aware navbar, a slim footer, and post
/// prev/next nav. Only shipped when a page renders inside a site (see
/// [`html_page_from_doc_in_site`]); all of it is driven by `--tali-*` vars so a
/// theme extension restyles it for free. Deliberately leaner than a full
/// Bootstrap chrome (no banner, no search bar, no feed).
const SITE_CSS: &str = include_str!("../../assets/css/site.css");

#[cfg(test)]
mod tests;
