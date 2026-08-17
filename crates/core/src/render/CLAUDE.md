# render/ — parse → block model → HTML emission

Module map:
- `mod.rs`    the render pipeline (`render_internal_impl` orchestrator), bundled-asset consts/accessors, the shared-state mutators (id/slug/xref dedup), small helpers
- `model.rs`  data types: `Cell`, `Block`, `RenderedDoc`, `PageIncludes`
- `doc_includes.rs` front-matter `include-*`/`css` resolution + isolated file I/O
- `fm_extract.rs`   front-matter FIELD scans (toc/title-block detect, `extract_field`)
- `cell_extract.rs` cell-option parsing (`#|`/`//|`/`%%|` directive + leaf parsers)
- `cell_numbered.rs` numbered `{js}`/figure/listing emitters + `numbered_caption` (interpolate the orchestrator's `attrs`; never build data-attrs)
- `page.rs`   full HTML-page assembly (the `PAGE_TEMPLATE` shell, `SiteCtx` wiring, favicon)
- `emit.rs`   per-block HTML (server-side highlight, code line-wrapping)
- `divs.rs`   `:::` fenced divs (callouts, the `layout-ncol` grid, width escapes)
- `figure.rs` numbered figures + captions
- `theme.rs`  `--tali-*` themes (light/dark), `theme_head`
- `extension/` shortcode expansion (`{{< input >}}`)

Conventions:
- Submodules use `use super::*` and **can see mod.rs's private items** (a child sees its
  parent's privates). The reverse is not true: to use a submodule's item from mod.rs or a
  sibling, `pub use` / `pub(crate) use` it in `mod.rs`.
- Every emitted block carries `data-block-id` (content hash) + `data-sourcepos`; preserve
  them — the incremental block-swap, click-to-source, and corpus invariants depend on it
  (`tests.rs` + `crates/core/tests/corpus.rs`).
- **Two line coordinate systems, and the TYPE keeps them apart.** A post-include BUFFER
  line is a `BufLine` (`model.rs`) — comrak's sourcepos, the `:::` span scan, `buf_start`,
  what `group_divs` matches spans in. `map_origin`/`map_span` are the way out: they return
  the author's own file and a plain line number, and past that point there is one
  coordinate system, so the source side stays a bare `usize`. `BufLine` has no `Display`
  and no conversion, so it cannot reach a `data-sourcepos` or a `Warning::at` without
  `.get()` unwrapping it by hand. A block's `data-sourcepos` range must also stay inside
  one file's numbering — `map_span` clamps a block that comrak merged across an include
  boundary back to the file it starts in.
- `dedup_element_ids` runs LAST of the id-assigning passes and is the only thing standing
  between a repeated author-written `{#id}` and two elements sharing it. It renames the
  repeat; the first keeps the author's spelling.
- Bundled assets (`KATEX_CSS`, `BASE_CSS`, `code-enhance.js`, …) are `include_str!`'d in `mod.rs`.
