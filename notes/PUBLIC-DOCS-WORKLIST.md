# Public docs worklist (CLOSED)

**Closed 2026-08-19.** Every open item from the pre-release audit has been resolved,
skipped with a stated reason, or handed back as a decision. Nothing here is outstanding.

The rules, the gate table and the structural decisions stay in
`notes/PUBLIC-DOCS-BACKLOG.md`.

## What happened to the 23 accuracy findings

**Already true when re-read (4).** The worklist listed a finding when its quoted text
still appeared in the file, which is a heuristic; these four had been fixed by the
implementation wave and the quote survived in rewritten prose.

| finding | why it was already closed |
|---|---|
| `using/code.tmd` savefig into a gitignored directory | the cell already imports `os` and calls `os.makedirs("figures", exist_ok=True)` before `savefig`, and the prose is already the short form |
| `internals/rendering.tmd` never mentions the two line coordinate systems | the chapter carries a "Two line coordinate systems" section |
| `internals/rendering.tmd` demonstrates no `{js}`, no `{{< input >}}` | the `{js}` section ends with a live `{{< input name="tau" >}}` slider driving a `{js}` cell |
| `site/_includes/three-scene.tmd` esm.sh vs "never phone home" | `features.tmd` (which carried the claim) is deleted; `index.tmd` says the scene loads Three.js from esm.sh, and the "Bundled, not fetched" box scopes the claim to what Taliesin itself emits |

**Fixed (18).** Listed in the backlog's Wave 5 table.

**Skipped with a reason (1).**

- `gallery/index.tmd` "hand-written collection page on a tool that ships `listing:`".
  **Not doing.** A listing card links to the `.tmd` page it crawled, and the three
  exhibits are not pages of this project: they are separate projects `tools/publish.sh`
  writes under the gallery's output, declared as `external-prefixes`. A `listing:` would
  therefore need three new stub pages, and the reader would click a card, land on a stub,
  and have to click again to reach the exhibit, which is worse than today's one click. It
  is also an ADD of three pages, three URLs, three sitemap and search-index entries, to
  dogfood a construct. Do not re-propose without a reason beyond dogfooding.

**Cut candidate (1): done.** `reference/frontmatter.tmd`'s "Page blocks (sites)" section
is deleted. It was a heading whose entire content said its content was elsewhere on the
same page, duplicating the "### Site page blocks" subsection eight screens above it. The
Recipes cross-link it carried moved onto the `listing:` row.

## The three handed-back items: all fixed 2026-08-19

1. **`site/assets/` held 2.27 MB that no page referenced. Deleted, all three.**
   `og-card.png` was stale branding, and that is why it was never wired up: it read
   "qmd-fast", "One **.qmd** file", and offered "Slide decks", a cut feature, in the brand
   blue the palette replaced. Setting `image:` to it would have unfurled a card for a
   differently named tool. `live-code-dark.mp4` and `live-edit-dark.mp4` carried the same
   old accent and logo mark, and were dark-only besides. `site/README.md` records what was
   there and why the recorder is kept. No page on any of the four sites sets `image:`, so
   every share unfurls as a plain `summary` card, deliberately, until there is a card worth
   shipping.

2. **A site page with a TOC rail gave code a third less width than a book page. Fixed in
   `base.css`, and it was a real defect rather than a tradeoff.** The margin-column rule
   was written to engage on
   `body:not(.has-toc), .tali-site-main:not(.has-toc), .tali-book-main`, with a comment
   saying a rail page "keeps the collapsed form it has always had". It never did:
   **`has-toc` lands on `body` only for a single document**, while a project puts it on the
   wrapper and leaves the body as `.tali-site`. So `body:not(.has-toc)` matched every site
   page ever rendered, set `--tali-note-w: 20rem` there, and let it inherit straight past
   the `.tali-site-main:not(.has-toc)` exclusion beside it, which was dead text for as long
   as it existed. Measured on `corpus/tech-blog/posts/a-star`: the grid reserved a 320px
   note column plus a 60px gap on a rail page, and because the bleed track is
   `minmax(0, …)` the band absorbed the whole loss, leaving 109px of bleed instead of 320
   and a `pre` at **749px against the 960px the design states fits 84 columns**. Nine site
   pages across the corpus were in that state and **not one of them used the margin
   column**, so the column that squeezed the code was reserved for nobody. The body arm now
   excludes the two project body classes, which is what makes it mean "the single-document
   container" as intended. Verified in Chrome across all five layout modes: the rail page
   goes 749px to 960px, and a book page, a site page without a rail, a single document
   without a rail and a single document with one are all byte-for-byte unchanged, floating
   sidenotes and `.column-margin` included.

   The `toc: false` added to `site/showcase.tmd` as a workaround was **reverted**: the page
   keeps its rail and gets 960px anyway. Leaving a surface removed on a rationale that had
   become false is the kind of thing this audit existed to clean up.

3. **`site/showcase.tmd`'s reprinted `{js}` cell now has a gate**:
   `crates/core/tests/reprinted_js_cells.rs`. First confirmed against the binary that the
   duplication cannot simply be cut: a `{js}` cell emits no source listing with
   `//| echo: true` any more than without it, so reprinting is a real workaround for a real
   gap. The gate pairs a display fence with a live cell **by their opening line** rather
   than by filename, because naming the file would be the hand-kept list
   `three_scene_theme.rs` already documents as having undercounted once. Measured over
   every `.tmd` in the tree, that pairs showcase's transcript and nothing else: the guide's
   five `//|`-opening display fences teach the option syntax and match no cell on their
   page. Proven to fail on a one-sided edit before landing, with anti-vacuity at 1 pair.

## Deliberately NOT done (do not re-propose)

- **`using/choosing.tmd` "What Taliesin claims, and who asked for it"** (23 lines). A
  grader called it defensive prose and marked it CUT. Overridden: it cites five real issue
  links showing the demand is not retrofitted. That is evidence, and it is the page's whole
  point.
- **The R half of `using/code.tmd`'s "Publishing a table"** is already gone; the section
  survives with its Python half, which is correct.
- **A `help` row in the CLI reference table.** `every_subcommand_has_a_row_in_the_cli_reference`
  filters `help` out by name, and says why in the code: "`help` is the usage page itself,
  not a row in the table it prints." The worklist asked for the row; the gate is right.
- **Live specimens throughout `reference/cheatsheet.tmd`.** The finding asked for a
  callout, an equation, a `{mermaid}` cell and a live `{{< input >}}` beside the six
  tables. Only the equation was taken (it proves the `$$…$$ {#eq-x}` row it sits under at
  a cost of three lines). A `{mermaid}` cell pulls the 3.5 MB runtime onto the one page a
  reader keeps open in a tab, and an `{{< input >}}` with no `{js}` cell to feed is a dead
  control. The page's value is density and Ctrl-F; four specimens spend that to make a
  point the rest of the guide already makes.
- See `notes/PUBLIC-DOCS-BACKLOG.md` for the three structural decisions (marketing
  collapse, gallery stays on its own domain, CDN claim scoped rather than vendored).
