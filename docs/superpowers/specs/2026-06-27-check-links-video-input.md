# `qmd-fast check`: broken links, local video, reactive-input/cycle checks

Date: 2026-06-27
Lane: release-hardening "Lane A" — extend the static `qmd-fast check` diagnostics channel.

## Problem

`qmd-fast check` is the kernel-free preflight: it renders a file/site in memory and
lists every *located* diagnostic (message + optional file/line) on the same
click-to-source `Warning` channel the live servers use. A green `check` is supposed to
mean "publishable". Three classes of silent breakage are not yet caught:

1. **Broken internal/relative links.** A Markdown link `[..](path)` /
   `[..](path#anchor)` whose local target file is missing, or whose `#anchor` resolves
   to no heading/id on the target. Today only same-page `[..](#anchor)` jumps are
   checked (`validate_internal_anchors`); a link to another *file* that does not exist
   ships silently.
2. **Local video paths.** `validate_local_assets` checks `<img src>` only; a
   `{{< video clip.mp4 >}}` / raw `<video src>` whose local file is missing is not
   flagged.
3. **Dangling `//| input` names + reactive cycles in `{js}` cells.** `qmd-js.js`'s
   `buildGraph` wires `//| name`/`//| viewof` *defines* to `//| input` *consumers* and
   diagnoses cycles at runtime in the browser. `check` should mirror this statically:
   flag a `//| input: x` that nothing defines, and a dependency cycle.

## Approach

All new analysis is **read-only static** and lives additively in
`crates/core/src/diagnostics.rs`, called from `collect_diagnostics` in the server
(`crates/server/src/main.rs`). It reuses the existing `Warning` located-diagnostic
struct and the `frontmatter::closest()` did-you-mean helper. No edits to the reactive
runtime, exec, includes, cite, or the `:::` machine.

### 1. Broken links

New `pub` fn `validate_local_links(blocks, base)` for a **single doc**. Scans
`<a href>` for *local* refs (reusing `is_local_ref`). For a `path` (or `path#frag`)
component that is non-empty, not absolute (`/…`), and not a bare `#frag` (already
handled by `validate_internal_anchors`): if `base.join(path)` is not a file, flag
"broken link: `<path>` (no such file under the document directory)". A `.qmd`→`.html`
rewrite is a *site* concern, so for a standalone doc the file is resolved as authored
on disk. Anchor-only (`#frag`) links and the fragment *within* a cross-file link are
left to `validate_internal_anchors` / the site path to avoid double-reporting.

For a **site**, `Site` gains a small additive helper `page_ids()` exposing
`(url -> set<id>)` over every page's rendered headings/anchors. `collect_site_diagnostics`
calls a new `validate_site_links` once per page: it resolves each local `<a href>`
against the registry (the set of page `url`s, with `.qmd`→`.html` applied via the
existing `qmd_to_html`) plus that page's id set; it flags an href whose target page is
unknown, and an href `page.html#frag` whose `frag` is not an id on the target page.

**External links** (`http(s)://`, `//`, `mailto:`, `tel:`, …): **skipped** — never
fetched over the network (offline-by-design tool; a network probe would make `check`
nondeterministic and slow). Documented in `cli.qmd`.

Cells caveat: like `validate_internal_anchors`, the **anchor** existence half is
suppressed for a target page that contains executable cells (a cell can emit an id at
runtime), so a green check stays a no-false-positive promise. The *file-existence* half
is always safe to run.

### 2. Local video

`validate_local_media(blocks, base)` mirrors `validate_local_assets` but scans
`<video …>`/`<source …>` `src=`/`poster=`. Only *local* refs (`is_local_ref`, not
absolute) are flagged when missing. (`{{< video >}}` renders to raw `<video src>`, so
scanning the emitted HTML catches both the shortcode and hand-written `<video>`.)

### 3. Dangling `//| input` + cycles

`validate_js_reactive_graph(blocks)` builds the same edge model as `buildGraph`, from
the **block model** (`b.cell` where `cell.lang == "js"`, reading `cell.js.name` /
`cell.js.viewof` / `cell.js.inputs`) plus the **static define sources** that are not js
cells:

- `//| name: N` and `//| viewof: V` → define `N` / `V`.
- declarative `{{< input name="k" >}}` → defines `k` (found via `data-qmd-input="k"`
  in any block's HTML).

Then:
- **Dangling input:** a `//| input: x` whose `x` is in no static define set →
  "unknown reactive input `x`" (+ did-you-mean over the known define names).
- **Cycle:** Kahn's topological sort over `define -> consumer` edges (exactly
  `buildGraph`); any cell not drained is in a cycle → "reactive dependency cycle
  involving `<name>`".

**Runtime-define suppression:** Python `ojs_define` publishes names at *runtime* via a
`<script type="qmd-define">` blob that static analysis cannot enumerate. So if the doc
contains **any** non-js executable cell (`{python}`/`{r}`/…), the *dangling-input*
check is suppressed (a name could be defined at runtime). The **cycle** check stays on
(a cycle among `{js}` cells is a structural fact independent of runtime defines).

## Invariant note

- Pure additive read-only analysis on the existing `Warning` channel; every emitted
  diagnostic is located (file + line from `sourcepos`) so it stays click-to-source.
- No write path, no edits to the single-editing-surface bridge, the `:::` machine,
  cite, includes, exec/freeze/kernel, or any `assets/js`.
- No-false-positive discipline: anything that could be satisfied at runtime
  (cell-emitted ids, `ojs_define` names) is conservatively suppressed, mirroring
  `validate_internal_anchors`.
- Pinned by `corpus/diagnostics/links.qmd` (exempt dir) + `collect_diagnostics_*`
  unit tests asserting exact located warnings, and guarded against corpus false
  positives by the existing `check_superset_has_no_false_positives_across_corpus`
  test (extended with the new message needles).
