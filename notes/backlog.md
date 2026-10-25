# Taliesin backlog

Open tasks only: delete an item when it lands. What must not be rebuilt or re-filed is in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md). Item numbers are stable and never reused. Older notes,
audits and the roadmap are in git (`git show 30b8c1cd:notes/<file>`).

## Open items

100. **Confirm `AJBogo9/taliesin-old` is gone, then delete this item.** It held the un-rewritten
     history that ruling D-8 forbids keeping on GitHub. On 2026-09-24 `gh repo view
     AJBogo9/taliesin-old` could not resolve it and `gh repo list AJBogo9 --visibility private`
     listed only `taliesin-private-archive`, so it looks deleted; the author confirms.

228. **Tables default to the text column; the wide band becomes opt-in** (owner request,
     2026-10-10). `base.css` lists `table` among the band occupants, so every top-level table is
     fitted up to the band and spills past the prose on both sides. The owner wants the measure
     as the default. Touch points: drop `table` from the two `:is()` occupant lists (the
     `grid-column: var(--tali-band-span)` rule and the `justify-self: var(--tali-band-anchor)`
     rule), rewrite the "A `table` is fitted" paragraph of the FILLED versus FITTED comment, and
     drop `("table", "a data table")` from the occupant assertion in
     `crates/core/tests/layout_escapes.rs` (its `--tali-band-anchor` message names a table too).
     The opt-in should already exist: a table inside `::: {.column-page}` is a child of a filled
     band div, so it fits up to the band; check where it sits (it may hug the band's left edge
     rather than centre). The comment's cost measurement (36 of 39 tables squeezed at the
     measure, 2026-08-19) was taken on the four taliesin.sh sites, which are gone; the blog is
     what this change affects now. After it lands, delete the two bare `::: {}` wrappers in
     tech-blog's `site/projects/taliesin/index.tmd`, which work around this today.

## Code cells: kept by review, not built yet

From the 2026-09-24 code-cell DX brainstorm, whose parked ideas (each with a revive trigger)
are in `git show 30b8c1cd:notes/2026-09-24-code-cell-dx-brainstorm.md`. Each of these is small,
and each problem was confirmed in the code on 2026-09-24:

- **Stop at the first failing Python cell.** Later cells get a NOT_RUN "the cell at X raised"
  placeholder. Measured: one typo gave 3 tracebacks. Two corrections from review: keep the
  failed cell in `ran`, and do not count a truncation marker as a failure. The audit session
  declined it as a behaviour change, so it is the author's call.
- **Name the exception in the cell-error line** (`NameError: name 'grid' is not defined`), and
  in `--format json`. The file and line are there now; the text is not.
- **`//# sourceURL=` per `{js}` cell**, named after `//| name` or its ordinal, with no padding,
  so DevTools stack frames and breakpoints name the cell.
- **Dev-panel cell-error rows open the cell in the editor** (`openSource`, `client.js`), the
  same way diagnostic rows do.
- **Delete `tali.container` and `tali.invalidation` from `makeApi`.** They duplicate the cell's
  parameters and have zero uses.
- **A `TaliCell` typedef on `makeApi`**, checked by the assets `tsc` gate and served to the
  shadow, so `tali.` completes members. Today `tali` is a parameter typed `any`.
- **A sibling `.js` module a `{js}` cell imports becomes a page dependency**, and saving it
  reloads the tab. Hoist `build.rs`'s `relative_specifiers` into one resolver.
- **Grammar:** map markdown-basics' embedded languages (Ctrl+/ in a ```` ```sh ```` fence gives
  `#`, not `<!--`), and remove the alias rot (`javascript|ojs` painted as a live `{js}` cell,
  `dot` as mermaid, `\.?`) along with the dotted `{.lang}` read.
- **`doctor`'s install line spells `uv pip install --python ...`** when the venv is a uv venv.
- **Blog:** replace the six hand-built slider/select `viewof` cells with `{{< input >}}`, and
  add one line to the blog's CLAUDE.md so the pattern does not come back.
- **An author-side Claude Code hook** in the blog's settings that runs
  `taliesin build <file> --check-only --format json` on every `.tmd` an agent edits
  (about 0.25 s).
