# B2 — Book landing-page auto-TOC

Date: 2026-07-18. Backlog item **B2** (PMF audit). Branch `b2-book-landing-toc`.

## Goal

Render a whole-book **Contents** list on a book's landing page (the hardcover pattern), so
a reader can jump straight to any chapter. Today the book landing is just the `index.tmd`
preface prose; `toc: true` only drives the per-page heading scrollspy and the off-canvas
chapter drawer. This is confirmed absent and is the owner's own idea.

## Data source (reuse)

- `Site.book: Option<Book>`; `Book.entries: Vec<BookEntry>` — the same ordered list the
  drawer (`sidebar_html`) and prev/next (`book_nav_html`) already iterate.
- `BookEntry { part: Option<String>, number: Option<u32>, title, rel, url, draft }` — a
  `part: Some` row is a divider (no `url`/`number`); a chapter row has `url` + optional
  `number`.
- Chapter **blurb**: `BookEntry` carries no description, but for a book `book_pages` parses
  every chapter's front matter onto its `Page.description`. Join `BookEntry` → `Page` by
  `rel` (`self.pages.iter().find(|p| p.rel == entry.rel)`), the pattern `Site::page()` uses.

## Detection + hook

Add `attach_book_toc(&self, page, blocks)` as a new step in `finish_blocks` (after
`expand_page`, before `attach_cite_this`), mirroring `attach_backlinks`/`attach_cite_this`.
It is a no-op unless **this page is the book landing**: `self.book.is_some()` and
`self.is_home(page)` (`page.url == "index.html"`) — both already reachable there. It pushes
one generated `Block { id: "qmd-book-toc", sourcepos: "", source_file: None, cell: None }`
at the end of the landing's content (after the preface prose, before `post_nav_html`).

## Rendering

Iterate `book.entries` exactly as the drawer does (so drafts behave identically: shown with
a badge in preview, excluded from a build), but:

- **Skip the landing's own entry** (`entry.url == page.url`) — no self-link on the preface.
- **Distinct class prefix** `tali-btoc-*` (NOT the drawer's `.tali-book-chapter`, which
  carries `[data-qmd-drawer-close]`/`BOOK_DRAWER_SCRIPT` semantics).
- Render nothing (no block) if no linkable chapter remains.

Markup:

```html
<nav class="tali-book-landing-toc" aria-labelledby="tali-btoc-h" data-block-id="qmd-book-toc">
  <h2 id="tali-btoc-h" class="tali-btoc-title">Contents</h2>
  <ul class="tali-btoc-list">
    <li class="tali-btoc-part">Core</li>                       <!-- a part divider -->
    <li class="tali-btoc-item">
      <a class="tali-btoc-link" href="methods.html">
        <span class="tali-btoc-num">2</span>
        <span class="tali-btoc-chap">Methodology</span>
      </a>
      <p class="tali-btoc-desc">A one-line blurb from the chapter's description:.</p>
    </li>
    <li class="tali-btoc-item">                                <!-- unnumbered chapter -->
      <a class="tali-btoc-link" href="preface2.html"><span class="tali-btoc-chap">Foreword</span></a>
    </li>
    <li class="tali-btoc-item tali-btoc-draft">…<span class="tali-draft-badge">Draft</span></li>
  </ul>
</nav>
```

- Number span omitted when `entry.number` is `None` (unnumbered chapter), matching the
  drawer's `label` idiom.
- Blurb `<p>` omitted when the chapter has no `description:` (the no-blurb path).
- Draft entries reuse the existing `.tali-draft-badge` (preview only; a build has no drafts).
- All text escaped (`escape_attr`). `<h2>Contents</h2>` is genuine landing structure; it is
  NOT in the scrollspy TOC because `page_toc` is computed *before* `finish_blocks` appends
  this block.

## Styling

New `.tali-book-landing-toc` / `.tali-btoc-*` block in `site.css`, `--tali-*` tokens,
theme-aware, `forced-colors` fallback. Reuse the drawer's visual conventions (muted number,
link color on the title) without reusing its classes. A prominent, readable contents list
(bigger than the drawer's compact rail), sitting in the reading column.

## Pin (corpus) + tests

- **`corpus/demo-book/`**: add `description:` front matter to **one** chapter (exercise the
  blurb path) and leave **another** without (the no-blurb path). Do not change any chapter
  title/number/part (protect `book_discovers_chapters_with_parts_numbering_and_chrome`).
- **New test** `crates/core/tests/book_landing_toc.rs`:
  - the landing (`index.tmd`) renders a `qmd-book-toc` nav listing the numbered chapters +
    the part divider "Core", in order, each linking its `.html`;
  - the chapter with a `description:` shows its blurb; the one without shows none;
  - the landing does **not** list itself (no self-link to `index.html`);
  - a **chapter page** (e.g. `methods.html`) renders **no** `qmd-book-toc` (landing only);
  - a **website** (non-book) landing renders no `qmd-book-toc`.
  - Each assertion mutation-checked (mutate the builder → the named test fails).
- **Regression**: the full `taliesin-core` + `taliesin-server` suites stay green (the big
  `book_discovers_…` chrome pin, publish/parallel-build determinism fixtures on demo-book).
- **Browser verify** (chrome-devtools): the landing shows Contents at three viewports,
  light + dark; links navigate; a chapter page has no landing-TOC; no console errors.

## Out of scope

- The per-page heading scrollspy TOC and the chapter drawer are unchanged (the 2026-07-06
  "keep both nav surfaces" decision; this is additive).
- No new config knob: the landing TOC is default-on for a book (minimal-config; it is the
  hardcover affordance the owner asked for). Revisit an opt-out only on demand.

## Invariants honored

All offline; no preview write-back; no new output format; `--tali-*` tokens only; generated
block carries `data-block-id`, no sourcepos (matches `attach_backlinks`/`attach_cite_this`);
deterministic output (byte-identical builds preserved); the drawer/postnav chrome strings
the corpus test pins are untouched.
