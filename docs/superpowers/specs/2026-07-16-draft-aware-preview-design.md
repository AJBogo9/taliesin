# Draft-aware preview — design

**Date:** 2026-07-16
**Backlog:** §A item 7 (`notes/backlog.md`) — *[ruled: preview shows, build hides]*.
**Owner decision this session:** drafts surface **inline / in-context** in preview (in
nav, listings, and prev/next, each badged), not out-of-band.

## Problem

`draft: true` in a page's front matter currently drops the page **at discovery**
([`site/discovery.rs:21-23`](../../../crates/core/src/site/discovery.rs#L21)), and
`Site::discover` is the single mode-agnostic seam shared by preview, build, publish, and
the read-only `check`/`map`/query tools. So a draft is invisible **everywhere**, including
the author's own `preview` loop: you cannot see a draft render until you flip it live, and
book chapters cannot be drafted at all ([`book.rs:173`](../../../crates/core/src/site/book.rs#L173)
`book_pages` never reads `fm.draft`).

The goal: **preview shows drafts (in context, badged); build/publish exclude them and
report how many were held back.**

## Approach: mode-threaded discovery, "excluded" is the zero-arg default

Because the owner chose *inline*, a previewed draft must flow through the whole discovery
pipeline (cross-page `@fig-`/`@thm-` numbering, listings, prev/next, nav) so its card and
neighbours render correctly. That rules out "discover published-only, staple drafts on the
side" — the draft has to be a first-class member of the page set in preview. The split
therefore lives at **discovery mode**.

### Data model

- **`Page.draft: bool`** (new field, [`site/mod.rs:36`](../../../crates/core/src/site/mod.rs#L36)).
  `false` for every published page; `true` only for a draft surfaced in `Include` mode.
  Two `Page { … }` literal sites set it: `website_pages` (discovery.rs) and `book_pages`
  (book.rs).
- **`Site.excluded_drafts: Vec<String>`** (new field, [`site/mod.rs:140`](../../../crates/core/src/site/mod.rs#L140)).
  Rel paths of drafts dropped in `Exclude` mode (empty in `Include` mode). Drives build's
  "N drafts not published" report.

### Discovery modes

```rust
pub enum DraftMode { Exclude, Include }

impl Site {
    pub fn discover(root: &Path) -> Site {          // unchanged signature
        Self::discover_with(root, DraftMode::Exclude)
    }
    pub fn discover_with(root: &Path, drafts: DraftMode) -> Site { … }
}
```

- `Site::discover(root)` stays the **published view**: drops drafts from `pages`, records
  their rels in `excluded_drafts`. **build / publish / check / map / query call this and
  keep the exact same page set as today** — provably zero behaviour change (their call
  sites don't even change, the signature is identical). Build additionally reads
  `excluded_drafts` to log the report.
- `Site::discover_with(root, DraftMode::Include)` is used **only by the 4 preview call
  sites** in `serve_site/mod.rs` (`:119`, `:145` mounted, `:1096`, `:1119` hot-reload).
  Drafts flow through numbering/listings/prev-next/nav tagged `draft: true`.
- `website_pages` + `book_pages` take the mode. `Exclude` drops drafts (collecting their
  rels for `excluded_drafts`); `Include` builds the `Page` with `draft: true`. This is
  what makes **book chapters draftable**: a draft chapter renumbers away in `Exclude`,
  renders in-context in `Include`.

**Why "excluded" is the zero-arg default (not the reverse):** the worst failure mode is a
draft leaking into a **build**. Making the plain `discover(root)` exclude drafts means any
consumer — including a future one — is safe unless it *explicitly* opts into `Include`.
Only preview opts in. The alternative ("discover total always, every consumer filters
`!draft`") spreads a parallel filter across build/publish/check/map/query where a single
missed filter ships a draft; rejected. An env-var/global mode flag hides the coupling and
resists testing; rejected.

### Book consistency requirement

A book's chapter **drawer, section numbering, and prev/next must agree with the rendered
page set for the active mode.** In `Exclude`, a draft chapter is absent from all three
(so the drawer never links a page that wasn't built); in `Include`, it appears in all
three, badged. The draft chapter is placed **last** in the corpus pin (an appendix) so an
`Exclude` build of `corpus/demo-book` stays byte-identical to today (no mid-list
renumber). Implementation must trace the drawer seam (`build_book` / `book_pages` /
`chrome.rs` book sidebar) and drop/keep the draft consistently across all of them per
mode.

### Badge + dev menu (preview-only, driven by `page.draft`)

A built page always has `draft == false`, so every affordance below is structurally inert
in a build — no build-time guard needed, the data carries the gate:

- A quiet `DRAFT` badge in the page title area (site chrome, `render_page_doc`), on
  listing cards (`tali-card` builder, [`mod.rs:~1105`](../../../crates/core/src/site/mod.rs#L1105)),
  and a marker on nav / prev-next labels.
- The floating **dev menu** ([`serve/mod.rs:511`](../../../crates/server/src/serve/mod.rs#L511))
  gains an `N drafts` count that expands to a click-to-open list of draft URLs.

### Build report

After `Site::discover` (Exclude) in `run_site_build` ([`build.rs:1182`](../../../crates/server/src/build.rs#L1182)),
when `!site.excluded_drafts.is_empty()`, log `N drafts not published: <rel>, <rel>, …`.
Same for `publish` (it routes through `run_site_build`, so one implementation covers both).

## Boundary (explicitly out of scope)

`check` / `map` / `query` stay the published view (they call `discover` = Exclude), so
drafts never affect the publish gate or the agent tooling — consistent with build/publish.
**Draft-aware `map`** (surfacing drafts to an agent) is the separate Tier-2 AI-native
backlog item and is not built here.

## Invariants held

- **HTML-only**, no new output format. No CDN. No preview write-back (drafts are read like
  any page; the badge/dev-menu are read-only view affordances).
- **Block-model** untouched (`data-block-id`/`data-sourcepos` unchanged; the change is
  page *membership* + one bool, not block emission).
- **Do-NOT-touch** machinery untouched: `:::` scanner, `cite.rs`, `includes.rs`,
  numbering scanners, exec/freeze/kernel. Discovery mode only gates which pages enter the
  existing pipeline.
- **Minimal blast radius:** the published path (`discover`) is byte-identical to today;
  drafts are strictly additive on the preview path.

## The pin (corpus)

- **Website:** a `draft: true` post under `corpus/tech-blog/posts/draft-example/index.tmd`
  (title + date + a category, so it exercises a listing card badge). Excluded from the
  tech-blog build → existing tech-blog build snapshots/counts unchanged.
- **Book:** a draft appendix chapter appended to `corpus/demo-book` (`chapters:` gains a
  final `- appendix.tmd`; the file sets `draft: true`). Last position → `Exclude` build of
  demo-book byte-identical to today.

## Testing

- **Unit** (`site/mod.rs` tests): `discover` vs `discover_with(Include)` — page-set
  membership (draft absent/present), `excluded_drafts` contents, `Page.draft` tagging,
  across both website + book; book `Exclude` renumbering closes the gap (draft appendix
  absent from numbering) and `Include` numbers it in context.
- **Corpus** (`corpus.rs` / `tech_blog.rs`): the draft is present in Include-mode
  discovery and absent from a build's output tree (no `draft-example/index.html`); the
  `DRAFT` badge HTML is present on the Include-mode listing card and absent from the
  Exclude-mode listing.
- **Browser** (chrome-devtools): `preview corpus/tech-blog` → the draft appears in the
  blog listing with a `DRAFT` badge and the dev menu shows the count/list; `build
  corpus/tech-blog` → `draft-example/index.html` is absent and the log reports "1 draft
  not published".

## Files touched (anticipated)

- `crates/core/src/site/mod.rs` — `Page.draft`, `Site.excluded_drafts`, `DraftMode`,
  `discover_with`, listing-card badge, `render_page_doc` title badge.
- `crates/core/src/site/discovery.rs` — `website_pages(mode)` tagging + exclusion.
- `crates/core/src/site/book.rs` — `book_pages(mode)` reads `fm.draft`; drawer consistency.
- `crates/core/src/site/chrome.rs` — nav / prev-next draft marker; book drawer consistency.
- `crates/server/src/serve_site/mod.rs` — 4 call sites → `discover_with(Include)`.
- `crates/server/src/serve/mod.rs` — dev-menu draft count/list (preview-only).
- `crates/server/src/build.rs` — "N drafts not published" report.
- `corpus/tech-blog/posts/draft-example/index.tmd`, `corpus/demo-book/appendix.tmd`,
  `corpus/demo-book/_site.yml` — the pins.
