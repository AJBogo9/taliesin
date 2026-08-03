# site/ — multi-page projects (websites + books)

A directory with a `_site.yml` is a `Site`. `chapters:` present ⇒ a **book** (one centred
reading column; the chapter list is an off-canvas drawer summoned from a slim sticky
`.tali-book-topbar`); otherwise a **website** (top navbar).

Module map:
- `mod.rs`        the `Site` type + its `impl` (discover, page render, listings, `hero:` blocks)
- `discovery.rs`  `.tmd` page + loose-deck filesystem walk (`website_pages`, `discover_decks`)
- `chapter.rs`    book-chapter section numbering (`number_chapter_headings`, `section_number`)
- `links.rs`      `.tmd`→`.html` href rewrite (`rewrite_tmd_links`, pub) + cross-page link validation
- `chrome.rs`     navbar / footer / post-nav / book sidebar HTML + social-icon glyphs
                  (a second `impl Site`, methods `pub(super)` so `page_chrome()` calls them)
- `frontmatter.rs` per-page `---` parsing (reuses `crate::frontmatter::front_matter_block`)
- `config/`       `_site.yml` → `SiteConfig` (flat native schema; the only path)
- `bibliography.rs` the project-wide `bibliography:` — resolved once at discovery against the
                  site root, laid **under** each page's own; plus the site-wide unused-entry +
                  duplicate-key lints (unused must be site-wide, or every shared entry a page
                  ignores would fire)
- `sentences.rs` reading-form text + sentence splitting (`plain`, `sentence_at`), read by
                  `backlinks.rs` to quote the sentence around a reference
- `book.rs` `meta.rs` `search.rs` `xref.rs`

Conventions:
- Submodules use `use super::*`; expose an item to `mod.rs`/siblings via a `pub(crate) use`
  re-export in `mod.rs` (parents can't see a child's private items).
- `mounts:` (config) serves another project (e.g. the docs book) under a URL prefix in
  `preview` — rendered on request via `serve_site`'s `MountedSite` — and `build` recurses
  into each one, writing it to `<out>/<at>/` **after** the parent (whose stale-output sweep
  would otherwise delete it).
- An `{{< embed >}}`-referenced deck is built/served but kept out of nav + search.
