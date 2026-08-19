# site/ — multi-page projects (websites + books)

A directory with a `_site.yml` is a `Site`. `chapters:` present ⇒ a **book** (one centred
reading column; the chapter list is an off-canvas drawer summoned from a slim sticky
`.tali-book-topbar`); otherwise a **website** (top navbar).

Module map:
- `mod.rs`        the `Site` type + its `impl` (discover, page render, listings, `hero:` blocks)
- `discovery.rs`  the `.tmd` page filesystem walk (`website_pages`)
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
- `book.rs` `meta.rs` `search.rs` `xref.rs`

Conventions:
- Submodules use `use super::*`; expose an item to `mod.rs`/siblings via a `pub(crate) use`
  re-export in `mod.rs` (parents can't see a child's private items).
- One project per `Site`, and one project per deploy: this repo's four sites (marketing,
  the two docs books, the gallery) build and deploy alone and link to each other by
  absolute URL (`tools/publish.sh`). Nothing is composed into another project's output.
  The gallery is a flat, self-contained project of one-page demos, not a parent that
  writes others under its own output.
