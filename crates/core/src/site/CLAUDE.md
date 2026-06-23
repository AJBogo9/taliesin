# site/ — multi-page projects (websites + books)

A directory with a `_quarto.yml` is a `Site`. `chapters:` present ⇒ a **book** (left
chapter sidebar); otherwise a **website** (top navbar).

Module map:
- `mod.rs`        discovery, page render, listings, `about:`/`hero:` blocks, link rewrite, helpers
- `chrome.rs`     navbar / footer / post-nav / book sidebar HTML + social-icon glyphs
                  (a second `impl Site`, methods `pub(super)` so `page_chrome()` calls them)
- `frontmatter.rs` per-page `---` parsing (reuses `crate::frontmatter::front_matter_block`)
- `config/`       `_quarto.yml` → `SiteConfig` (flat native schema; the only path)
- `book.rs` `feed.rs` `meta.rs` `search.rs` `xref.rs`

Conventions:
- Submodules use `use super::*`; expose an item to `mod.rs`/siblings via a `pub(crate) use`
  re-export in `mod.rs` (parents can't see a child's private items).
- `mounts:` (config) serves another project (e.g. the docs book) under a URL prefix in
  `preview` — rendered on request via `serve_site`'s `MountedSite`.
- An `{{< embed >}}`-referenced deck is built/served but kept out of nav + search.
