# Spec: mermaid offline + visible failure, figure `height=`, video/embed query-strings

Date: 2026-06-27
Lane: `lane-d-render` (release-hardening pass)

Three independent render/asset fixes, each corpus-pinned. Owned files only:
`render/mod.rs` (the `MERMAID` const), `render/figure.rs`, `render/extension/mod.rs`,
`assets/js/mermaid.js`, `THIRD_PARTY.md`, plus a corpus doc.

## 1. Mermaid offline + non-silent failure

### Problem
`MERMAID` in `render/mod.rs` is a hard-coded jsdelivr CDN URL. `assets/js/mermaid.js`
lazy-loads it on the first `pre.mermaid`. Offline (or CDN-blocked), the diagram fails:
the existing `onerror` sets a bare `data-mermaid-error="1"` attribute but renders **no
visible message** — the reader just sees the raw mermaid source text with no indication
anything went wrong. And there is no way to point the loader at a local/self-hosted copy.

### Decision (and the tradeoff)
**Configurable URL, NOT bundled-by-`include_str!`.** The mermaid library is ~2.8 MB
minified; baking it into every `qmd-fast` binary (and into every built page's `<script>`
budget) to serve the minority of pages that use a diagram is the wrong tradeoff for an
HTML-only static-site tool. Instead:

- The diagram-library URL becomes overridable at render time via the
  **`QMD_FAST_MERMAID_URL`** environment variable, read once in `render/mod.rs`. Unset →
  the pinned jsdelivr CDN URL (unchanged default behavior). Set → that value is used
  verbatim (an author who wants a fully-offline build self-hosts `mermaid.min.js` next to
  their site and points the env var at the relative/absolute URL). This keeps the binary
  small, keeps the default working, and makes a network-free build *possible* without
  forcing 2.8 MB on every page.
- **Failure is always non-silent** (the real offline-promise fix): when the library
  cannot load, `mermaid.js` now injects a visible `[data-mermaid-error]` banner in the
  diagram's place (a styled message + the readable source kept below), instead of only
  flipping a silent attribute. This holds whether the URL is the CDN or a missing local
  file.

This satisfies the task's "configurable URL AND/OR copy a local asset" with the
configurable-URL arm (the lighter, no-binary-bloat arm), and makes the offline failure
mode loud rather than silent per requirement (a).

### Implementation
- `render/mod.rs`: replace the `const MERMAID: &str = "…jsdelivr…"` with a small accessor
  `fn mermaid_url() -> String` that returns `QMD_FAST_MERMAID_URL` when set and non-empty,
  else the pinned default const. Use it where `MERMAID` was substituted into `MERMAID_JS`.
  Update the surrounding doc comment so it no longer claims an unconditional CDN dependency.
- `assets/js/mermaid.js`: in the `s.onerror` handler, in addition to flagging
  `data-mermaid-error`, replace each unprocessed `pre.mermaid`'s rendering with a visible
  error block: a `<div class="mermaid-error" role="alert">` banner ("Diagram could not be
  loaded (offline or blocked).") followed by the original source in a `<pre>` so the
  content is never lost. Idempotent and retry-safe (re-running clears/re-applies). No CSS
  file edits (CSS is out of scope / owned elsewhere) — the banner uses inline-safe styling
  via a class plus minimal inline style so it is visible even with no stylesheet.

### Verification (browser, by coordinator)
- Online: a page with a `{mermaid}` block renders the SVG diagram (CDN reachable).
- Blocked: with the network blocked (or `QMD_FAST_MERMAID_URL` pointed at a 404), the
  same page shows the visible `[data-mermaid-error]` banner + the source text, no silent
  blank.

## 2. Figure `height=` honored

### Problem
`render/figure.rs::emit_figure` builds the inline `style` from `width=` only; a
`height=` attribute on a standalone-image figure is silently dropped.

### Fix
Emit both `width:` and `height:` into the style when present (each escaped with
`escape_attr`, like width). Width-only and height-only and both-present all produce a
valid `style="…"`; neither present → no style attribute (unchanged).

### Test (TDD, in `render/tests.rs`)
A standalone image figure with `{#fig-x width=50% height=200px}` emits
`style="width:50%;height:200px"` (assert both halves present). Write failing first.

## 3. Video / embed query-string in the path

### Problem
`embed_path` picks the first token that is *not* a `key=value` named arg via
`!a.contains('=') || a.starts_with('=')`. A path carrying a query string
(`clip.mp4?token=abc`) contains `=`, so it is wrongly classified as a named arg and
rejected → `embed_path` returns `None` → the whole `{{< video … >}}` ships as literal
braces.

### Fix
Classify a token as a *named argument* only when it actually looks like `key=value` with
a plain-identifier key: the substring before the first `=` matches `[A-Za-z][A-Za-z0-9_-]*`
and there is no `?` before that `=`. Everything else (including a path with a
`?query=string`) is positional. Concretely, `embed_path` keeps the first token that is
**not** a named arg by this stricter rule. This still rejects genuine named args
(`dark=clip.mp4`, `title=…`) and still treats a leading-`=` token as positional, but now
accepts `clip.mp4?token=abc`, `clip.mp4?a=1&b=2`, etc. The emitted `src` is `escape_attr`'d
so `&` in a query string becomes `&amp;` safely.

### Test (TDD, in `tests/extensions.rs`)
`{{< video clip.mp4?token=abc >}}` → body contains `src="clip.mp4?token=abc"` inside a
`class="qmd-video"` figure and contains **no** literal `{{<`. Write failing first.

## Corpus pin
New `corpus/render-fixes/index.qmd`: one `height=` (and width) figure, one
`{{< video clip.mp4?token=demo123 >}}`, one `{mermaid}` block. A render test
(`tests/extensions.rs` or `render/tests.rs`) asserts the height style + the intact video
src render from that doc. The mermaid block's offline banner is browser-verified.
