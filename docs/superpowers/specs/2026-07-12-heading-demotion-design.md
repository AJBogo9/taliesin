# Heading demotion for title-blocked pages (backlog #11 / `#multiple-h1-per-post`)

**Status:** approved design, 2026-07-12. Target: an implementation plan (TDD).

## Problem

A post renders a visible title block (`<h1 class="title">…</h1>`) *and* its body
`#` sections each render as `<h1>` too. `corpus/tech-blog/posts/em-algorithm/index.tmd`
emits `<h1 class="title">The EM-algorithm</h1>` plus body `# Theory` and `# Code demo`
as `<h1>`s — three sibling `<h1>`s on one page (other posts have more). This is a
Quarto-migration a11y/SEO regression: a page should have exactly one `<h1>` (its title),
with sections nested beneath it (`<h2>`, `<h3>`, …). Screen-reader document outlines and
search engines both read the heading hierarchy; multiple `<h1>`s flatten it.

## Goal

When a page emits a title-block `<h1 class="title">`, demote every **body** markdown
heading by one level (`#` → `<h2>`, `##` → `<h3>`, …), so the page has exactly one `<h1>`
(the title) and a correct nested outline. Preserve every load-bearing invariant and touch
no Do-NOT-touch machinery.

## Scope — the trigger is the title-block h1

The demotion trigger is precisely **"this render emitted the title block."** That single
condition scopes the change with no dangerous blast radius (verified empirically
2026-07-12 by building each page type):

| Page type | Emits `<h1 class="title">`? | Demote? |
|---|---|---|
| Post (title + `#` sections) | yes | **yes — the fix** |
| Index / CV (titled; hero landing) | yes (index has exactly 1 `<h1>` today) | yes (harmless: few/no body headings) |
| `title-block-style: none` (blog / publications / projects) | no | no |
| Book chapter | no — renders `<h1 id="sec-…">` (numbered chapter heading), no title-block | **no — untouched** |
| Deck (`format: deck`) | no — builds its own title slide; `h1`/`h2` define slide breaks | **no — untouched** |

Because books and decks never emit the title-block h1, their two fragile subsystems —
book **section numbering** (`number_chapter_headings`, keyed on heading level) and deck
**slide grouping** (`deck.rs`, `block_heading_level` < `SLIDE_LEVEL` opens a stack) — are
never entered by this change. This was the decisive scoping insight.

Approved scope (2026-07-12): trigger on *any* page with a title-block h1 (posts + index +
cv), because "one `<h1>` per page" is a whole-site property, not a post-only concern.

## Mechanism

1. **Detect** at render time that the title block was emitted. The orchestrator already
   computes `hide_title_block` and inserts the `qmd-title-block` block only when
   `format == DocFormat::Html && !hide_title_block && title.is_some()`
   (`render/mod.rs`). Reuse exactly that condition as the demotion gate — a single
   `bool` (`demote_headings`) known before the per-block loop emits heading HTML.

2. **Demote the emitted `<hN>` tag only**, `N → min(N + 1, 6)` (clamp: a body `<h6>`
   stays `<h6>`; posts effectively never nest that deep). Do **not** change:
   - the heading **id / slug** — it is text-derived, so `#sec-…` anchors and
     `@sec-`/`@fig-`/theorem xrefs are unaffected;
   - **`data-sourcepos`** — unchanged;
   - the **logical heading level** used for any numbering / `@sec-` registration — keep
     the original level there (moot for demoted docs, which are never books, but keeps
     the change strictly additive to the visible tag).

   Implementation seam (to be finalized in the plan): demote the visible tag without
   disturbing id/sourcepos/registration. Candidate: after a heading block's HTML is
   built, rewrite its leading `<hN…>`/closing `</hN>` to `N+1` when `demote_headings`;
   alternative: bump the comrak `Heading.level` on the node immediately before `emit`,
   after id/number computation. The plan picks one and justifies it against the id-hash
   ordering (see invariants).

3. **`data-block-id`** is a content hash of the block HTML. Demotion changes the HTML, so
   a demoted heading's block id differs from its pre-change id. This is a **one-time,
   deterministic** shift (the same source always demotes the same way), so ids stay
   unique and stable per source — the corpus invariant holds. The id must be computed
   from the **post-demotion** HTML; the plan ensures the demotion happens before the hash.

4. **TOC follows automatically.** `toc_html` reads levels from the block HTML
   (`block_heading_level`), so demoted headings nest correctly. But its filter is an
   absolute `level <= 3`; after demotion (`base = 2`) that would drop the deepest shown
   level. Change it to **relative to the minimum level present** (`level - base <= 2`),
   which is identical to `level <= 3` for today's `base = 1` documents and correct for a
   demoted `base = 2` document. This is the one deliberate TOC change.

## Invariants preserved

- **Block model:** every heading block keeps `data-block-id` (recomputed from the demoted
  HTML) + `data-sourcepos`; included headings keep `data-source-file`. Ids stay unique +
  document-ordered. Enforced by the existing corpus invariant tests.
- **Single editing surface / HTML-only / Do-NOT-touch:** demotion is emission-only. It
  does not touch `divs.rs`, `cite.rs`, `includes.rs`, the numbering scanners, exec, or the
  deck engine. Decks and books are excluded by construction.
- **Reverse-sync:** sourcepos on demoted headings is unchanged, so click-to-source and the
  reverse cursor sync are unaffected.

## Testing (corpus-pinned)

- **Post (the fix):** render `posts/em-algorithm/index.tmd`; assert exactly **one** `<h1>`
  and that it is the `class="title"` block; assert the body `# Theory` / `# Code demo`
  now render as `<h2>` and their `##` children as `<h3>`; assert the section anchor ids
  (slugs) are unchanged from before; assert the TOC still lists those sections.
- **Regression guards for the excluded paths (mutation-checked):**
  - a **book chapter** (`demo-book`) heading structure is byte-identical to today (its
    `<h1 id="sec-…">` is not demoted);
  - a **deck** corpus doc's slide-break heading levels are unchanged (slides still split).
- **TOC-filter change:** a unit test that a demoted document surfaces three heading levels
  in the TOC (the relative filter), and that a non-demoted (`base = 1`) document's TOC is
  unchanged.
- Full `cargo test -p taliesin-core` + clippy clean; browser spot-check of a post
  (outline in devtools shows one h1 + nested h2/h3; TOC intact) at desktop + mobile.

## Out of scope / non-goals

- No change to `title-block-style: none` pages, books, or decks.
- No new front-matter knob — demotion is automatic and correct-by-default (matches the
  "perfect the default" convention). If a future page genuinely wants many `<h1>`s it can
  hide the title block, which already disables the trigger.
- No renumbering of book sections, no deck slide-level change.
