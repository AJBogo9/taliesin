# site/ — multi-page projects (websites + books)

A directory with a `_site.yml` is a `Site`. `chapters:` present ⇒ a **book** (one centred
reading column; the chapter list is an off-canvas drawer summoned from a slim sticky
`.tali-book-topbar`); otherwise a **website** (top navbar).

Module map:
- `mod.rs`        the `Site` type + its `impl` (discover, page render, listings, `hero:` blocks)
- `discovery.rs`  `.tmd` page + loose-deck filesystem walk (`website_pages`, `discover_decks`)
- `chapter.rs`    book-chapter section numbering (`number_chapter_headings`, `section_number`)
- `links.rs`      `.tmd`→`.html` href rewrite (`rewrite_qmd_links`, pub) + cross-page link validation
- `chrome.rs`     navbar / footer / post-nav / book sidebar HTML + social-icon glyphs
                  (a second `impl Site`, methods `pub(super)` so `page_chrome()` calls them)
- `frontmatter.rs` per-page `---` parsing (reuses `crate::frontmatter::front_matter_block`)
- `config/`       `_site.yml` → `SiteConfig` (flat native schema; the only path)
- `book.rs` `meta.rs` `search.rs` `xref.rs`

Conventions:
- Submodules use `use super::*`; expose an item to `mod.rs`/siblings via a `pub(crate) use`
  re-export in `mod.rs` (parents can't see a child's private items).
- `mounts:` (config) serves another project (e.g. the docs book) under a URL prefix in
  `preview` — rendered on request via `serve_site`'s `MountedSite`.
- An `{{< embed >}}`-referenced deck is built/served but kept out of nav + search.
