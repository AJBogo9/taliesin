# render/ — parse → block model → HTML emission

Module map:
- `mod.rs`    the render pipeline (`render_internal`), bundled-asset consts/accessors, small helpers
- `model.rs`  data types: `Cell`, `Block`, `RenderedDoc`, `PageIncludes`, `DocFormat`
- `page.rs`   full HTML-page assembly (the `PAGE_TEMPLATE` shell, `SiteCtx` wiring, favicon)
- `emit.rs`   per-block HTML (server-side highlight, code line-wrapping)
- `divs.rs`   `:::` fenced divs (callouts, columns, magic-move)
- `figure.rs` numbered figures + captions
- `deck.rs` the slide-deck engine (bundles `deck.css`/`deck.js`; native `.qmd-deck` + `window.QmdDeck`)
- `theme.rs`  `--qmd-*` themes (light/dark), `theme_head`
- `extension/` format extensions + shortcode expansion (`{{< embed >}}`, `{{< video >}}`)

Conventions:
- Submodules use `use super::*` and **can see mod.rs's private items** (a child sees its
  parent's privates). The reverse is not true: to use a submodule's item from mod.rs or a
  sibling, `pub use` / `pub(crate) use` it in `mod.rs`.
- Every emitted block carries `data-block-id` (content hash) + `data-sourcepos`; preserve
  them — the incremental block-swap, click-to-source, and corpus invariants depend on it
  (`tests.rs` + `crates/core/tests/corpus.rs`).
- Bundled assets (`KATEX_CSS`, `BASE_CSS`, `code-enhance.js`, …) are `include_str!`'d in `mod.rs`.
