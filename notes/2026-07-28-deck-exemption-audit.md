# R14 — the deck: the subsystem the checks cannot see

**Date:** 2026-07-28
**Round:** Wave 2 / R14 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question the round was given.** What is wrong with the deck engine, given that it is exempted from
two of the project's diagnostic families and that the backlog has deflected deck work eight times?

**This is not a re-run of the 2026-07-12 deck audit or the 2026-07-27 touch crossing.** Both audited
deck *behaviour*. This audits the deck's **exemption from the project's own quality machinery**.

---

## Headline

The spec was scoped on the premise that decks are exempt from **two** diagnostic families. That
premise is **too generous by an order of magnitude**, and the two documented exemptions are not where
the hole is.

**A deck that lives in a site is exempt from all thirteen static validator families, on all three
site surfaces, and both gates report success.** Measured, not inferred:

```
$ taliesin check .            # site containing an embedded deck with 6 defects
no problems found             # exit 0

$ taliesin build . --strict   # same tree
built … · 1 page · 1 deck · 44ms    # exit 0, and talk.html ships the broken image + failed math

$ taliesin check talk.tmd     # THE SAME FILE, checked directly
talk.tmd:8:  error[TAL-ASSET]:   local asset not found: `nope.png`
talk.tmd:12: error[TAL-ASSET]:   local asset not found: `does-not-exist.png`
talk.tmd:10: error[TAL-LINK]:    broken link: `broken-target.tmd`
talk.tmd:8:  warning[TAL-A11Y-ALT]:  image is missing alt text
talk.tmd:10: warning[TAL-A11Y-NAME]: link has no accessible name
talk.tmd:16: warning[TAL-MATH]:  math failed to render: KaTeX parse error
6 problems (3 errors, 3 warnings)
```

None of those six is deck-specific. None is covered by either documented exemption. They are the
ordinary rules — broken assets, broken links, missing alt text, unrenderable math — and the deck is
simply never handed to them.

This is live in the tree, not only in a probe: `taliesin check docs/guide` prints **"no problems
found"** and its JSON output mentions `demo.tmd` and `tour.tmd` **zero** times. Two real decks in the
dogfooded User Guide have never been linted by anything.

**Mechanism** (`crates/core/src/site/mod.rs:358-359`):

```rust
let decks = discover_decks(root, &pages, &mut warnings);
pages.retain(|p| !decks.iter().any(|d| d.url == p.url));
```

An `{{< embed >}}`-referenced deck is removed from `site.pages` so it stays out of nav and listings —
correct, and the reason it was written. But `check`'s site walk
(`crates/server/src/check.rs:295`, `for page in &site.pages`) and the site preview both iterate that
same set, so removing a deck from the *navigation* set silently removed it from the *validation* set.
The deck survives in `site.decks` (a public field, `site/mod.rs:178`), which nothing lints.

**The exemption register below is therefore mostly a record of exemptions that are correct.** The
defect is not in the two `DocFormat::Reveal` early-returns. It is that a deck never reaches the code
those early-returns live in.

---

## Findings

Item numbers start at **109**. Wave 1 used 79-108 and the concurrent `critique-pass-2026-07-27`
branch independently used 79-90 for different findings; see "Numbering collision" at the end.

### 109. An embedded deck is invisible to every static validator, on every site surface. (HIGH)

**Measured.** The transcript above. A throwaway site (`_site.yml`, an `index.tmd` that embeds
`talk.tmd`, and a `talk.tmd` deck carrying two missing assets, one broken link, one unnamed link, one
alt-less `<img>` and one malformed `$$`):

| Surface | Result |
|---|---|
| `taliesin check <dir>` | `no problems found`, exit 0 |
| `taliesin build <dir> --strict` | exit 0; `_site/talk.html` ships `nope.png` and one `katex-error` |
| `taliesin preview <dir>` | deck branch at `serve_site/mod.rs:585-604` returns `render_doc_to_page` directly — no `PageState`, no diagnostics panel |
| `taliesin check <deck.tmd>` | all 6 reported |

**Why it matters more than it looks.** `check` is the pass an agent runs first on an unknown project
(the due-diligence round names it as such), `--strict` is the
only build-side gate, and the deck is the artifact most likely to be *presented to an audience*. A
broken image on a slide is seen by a room, not by one reader scrolling past.

**Scope, enumerated rather than assumed.** All 13 families in `page_static_diagnostics`
(`check.rs:206-227`) are skipped: duplicate heading ids, internal anchors, local assets, local media,
local links, the `{js}` reactive graph, a11y, link-text collisions, document shape, math, code
languages, and both citation rules. Two of those (a11y, shape) are *additionally* deck-exempt from
inside; the other eleven have no deck opinion at all.

**Fix.** `collect_site_diagnostics` (`check.rs:280`) should walk `site.decks` after `site.pages`,
rendering each with `render_document_with_includes` against the deck's own parent dir (the deck is a
standalone document, so `Scope::Standalone` is the correct scope — it is not subject to the
site-link rewrite that `Scope::InSite` exists to accommodate). The build path needs the same, since
`--strict` reads the same superset by design.

**Refuted if** a `check <dir>` on the probe tree reports the deck's defects, or if `site.decks` turns
out to be reachable from the page loop by another route.

**Pin it with** a test that plants exactly one deck-only defect in an embedded deck and asserts
`check <dir>` is non-empty. There is currently **no test anywhere** that runs any validator against a
deck inside a site: `crates/core/tests/check_superset.rs` and `crates/server/tests/check_cli.rs`
contain zero occurrences of `deck` or `Reveal` (measured).

### 110. A `draft:` page is unlinted by `check` and `--strict` for the same structural reason. (MEDIUM)

Found while enumerating item 109's shape, per contract rule 3 ("a finding that names one instance has
not enumerated the shape"). Same `retain`-shaped mechanism, different set.

**Measured.** A site with a published `index.tmd` and a `draft: true` `wip.tmd` carrying a missing
asset and an alt-less `<img>`:

```
$ taliesin check .          →  no problems found        (exit 0)
$ taliesin check wip.tmd    →  3 problems (2 errors, 1 warning)
```

`discovery.rs:30` drops `draft: true` pages in `DraftMode::Exclude`, which is what `check` and
`build` use.

**Severity is genuinely lower than 109, and for a reason worth recording rather than a hunch.** The
*preview* uses `DraftMode::Include`, so a draft page is fully linted in the live loop where the author
actually writes it; and a draft is never built, so nothing ships. The cost is that defects accumulate
silently and arrive in one batch the moment `draft:` is removed — and that the kernel-free gate an
agent or a pre-publish script runs cannot see the page at all.

**This is arguably correct behaviour** — "don't fail a build over an unfinished page" is a defensible
ruling. It is filed because the ruling is nowhere stated, and because the deck case proves the same
mechanism can hide a page that *does* ship. If the ruling is affirmed, the deliverable is one comment
at `discovery.rs:30` and a line in the `check` docs, not code.

**Refuted if** a draft page's defects appear in `check <dir>` output.

### 111. The deck exemption makes an existing a11y test structurally vacuous on the two decks it walks. (MEDIUM)

`crates/core/tests/a11y_outline.rs:162-198` walks every `.tmd` under `docs/guide`, `docs/internals`
and `corpus/tarn`, calls `validate_a11y(&doc.blocks, doc.format)`, filters for `"heading level
skips"`, and asserts the result is empty across `pages >= 40`.

`docs/guide/demo.tmd` and `docs/guide/tour.tmd` are decks. `validate_a11y` early-returns out of the
heading rule for `DocFormat::Reveal` (`a11y.rs:228`), so for those two files the filtered list is
empty **by construction**. They count toward the `pages >= 40` floor — which is the assertion that
exists to prove the walk was live — while being incapable of contributing a finding.

This is the coverage-illusion shape LESSONS.md already names: the guard that proves the walk ran is
satisfied by files the walk cannot fail on.

**Refuted if** `validate_a11y` can return a heading warning for a deck (it cannot; the branch is
unconditional on format).

**Fix is one line of test intent, not of product code:** either exclude decks from the walk and say
why, or assert the deck count separately so the floor cannot be met by files the rule skips.

### 112. The repo's browser automation has never been pointed at the artifact that needs it most. (MEDIUM)

`chromiumoxide` appears in exactly two places: `crates/server/Cargo.toml` and
`crates/server/src/headless_js.rs`. `crates/server/tests/read_run_js.rs` — the one test behind the
fourth hand-run gate (`TALIESIN_REQUIRE_CHROME=1`) — contains **zero** occurrences of `deck` or
`reveal` (measured).

So the project owns headless-browser machinery, gates it in CI-equivalent form, and has never aimed
it at a 2,690-line JavaScript subsystem whose entire product value is runtime behaviour.

**What the existing deck tests actually assert**, so this is not overstated:

- `deck_key_sheet.rs` reads `deck.js` **as text** and pins that the key-binding strings and the
  in-product key sheet agree. A string pin, not a behaviour test — but a good one: it exists because
  the sheet drifted twice.
- `deck_marginalia.rs` pins shipped CSS tokens plus the emitted `data-level="1"` hook.
- `deck_offline_build.rs`, `deck_social_card.rs`, `loose_deck.rs` assert on emitted HTML from
  `deck.rs`.

Every one of them tests `deck.rs`'s *emission*. None executes `deck.js`.

**Smallest honest first step**, given the round must not become a wishlist: one headless test that
loads a built `corpus/deck.tmd`, presses `ArrowRight` N times, and asserts the slide index and the
`#/slug` hash agree. That single assertion covers the navigation core, the fragment stepper and the
hash writer at once, and the harness already exists.

**Refuted if** any test drives `deck.js` in a browser.

### 113. `corpus/deck.tmd` is the deck regression net and it contains no math, no kernel cell, and eleven other shapes the corpus has nowhere. (MEDIUM)

Per LESSONS.md, a shape the corpus lacks is invisible to a green suite, and that rule has already
hidden three real bugs. Measured across **all nine** decks in the tree (probe sanity-checked against a
hand-verified positive first — an all-empty first run was a broken zsh word-split, not a result):

| Construct | Decks containing it |
|---|---|
| display / inline math | `docs/guide/demo.tmd`, `site/demo.tmd`, `docs/guide/tour.tmd` — **no corpus deck** |
| `{python}` cell | `docs/guide/tour.tmd` — **no corpus deck** |
| `{js}` cell, mermaid, image, `@sec-`, notes, auto-animate, backgrounds | `corpus/deck.tmd` ✓ |
| callout | `docs/guide/tour.tmd` only |
| columns | `corpus/scaffold/deck-tour.tmd` only |
| **table, footnote, citation `[@key]`, `{r}` cell, theorem/definition/proof, tabset, `@fig-` + captioned figure, `{{< include >}}`, `{{< video >}}`, `logo:`, `theme:`, `lang:`, `css:`/`include-in-header:`** | **NONE — no deck in the tree** |

Two of these are load-bearing rather than exotic:

- **Math on a slide has no corpus coverage**, yet the deck assembler makes a deliberate decision about
  it (`serve/mod.rs:1060`, `ship_katex: true`, "a live deck can gain math at any edit"). The only
  math-bearing decks are dogfood and marketing files. `corpus.rs` walks `corpus_dir()` only; `docs/`
  is reached by four targeted tests, none of which renders a deck's math.
- **No corpus deck runs a kernel cell.** The deck × execution crossing — the exact *crossed dimension*
  the demand-probe programme concluded is where defects live — is unpinned in the regression net.

**This is a coverage statement, not a defect.** Filing it as a defect would be the "grow the corpus
wholesale" error the spec warns against. The proposal is narrow: add math and one `{python}` cell to
`corpus/deck.tmd` (both already render correctly in the dogfood decks, so this pins working
behaviour), and leave the other eleven listed but unbuilt until something needs them.

**Refuted if** any corpus deck contains `$$` or a kernel cell.

---

## The exemption register

The round's durable deliverable: every branch in the tree that treats a deck differently, what it
skips, whether the rationale still holds, and **what replacement check exists**. An exemption with no
replacement is a hole, and the register makes holes countable.

| # | Site | What it skips | Rationale | Holds? | Replacement check |
|---|---|---|---|---|---|
| E1 | `site/mod.rs:359` | **All 13 validator families**, all site surfaces | none — an unintended consequence of removing decks from nav | **NO** | **none** → item 109 |
| E2 | `shape.rs:97` | the entire `TAL-SHAPE-*` family (`DUP`, `EMPTY`, `HOLLOW`, `ECHO`, `CAPTION`) | every rule reasons about *document* structure; a deck has no TOC and each rule inverts on a slide | **YES — verified** | **none** (see below) |
| E3 | `a11y.rs:228` | heading-level-skip detection only (rules 2 and 3 still run) | slides are slide-structured, not one outline | partly | the **preview** client runs the rule on decks — a divergence, not a replacement (see below) |
| E4 | `build.rs:620` | `--bare` refuses a deck outright | bare output is JS-free; deck navigation is JavaScript | **YES** | n/a — a refusal, not a skip |
| E5 | `site/mod.rs:388-396` | — | warns when a deck is a *loose* site page | **YES** | this is itself a check; `loose_deck.rs` pins it |
| E6 | `site/mod.rs:374-380` | — | warns that `draft:` is ignored on an embedded deck | **YES** | pinned by `embed_warning.rs` |
| E7 | `page.rs:13`, `serve/mod.rs:92/780` | separate deck page assembler | a deck is not an article shell | **YES** | the 2026-07-26 path-parity round measured all four deck paths identical |
| E8 | `serve/mod.rs:1237/1605/1614` | flat block-op diffing; deck re-mounts on structural or title change | `<section>`-grouped slides can't be restructured by flat ops | **YES** | `incremental.rs` |

**E2 is correct, and measured to be correct.** Running the duplicate-heading rule over all nine decks
produces exactly three hits: `"One idea, refined"` ×2 in `corpus/deck.tmd` and `"One source, many
views"` ×2 in both `demo.tmd`s. All three are deliberate `auto-animate` magic-move pairs. A naive
rule would be 100% false positives today. **The finding is not "remove E2" — it is that the
deck-appropriate replacement was never written**: a duplicate slide title is fine *when the pair is a
magic-move pair* and suspect otherwise. That rule does not exist, so a genuinely duplicated slide
title is unreportable.

Two related things came back **healthy** and are recorded so they are not re-derived:

- **Slide ids do not collide** on a magic-move pair: `id="one-idea-refined"` and
  `id="one-idea-refined-1"`. The dedup works.
- **No deck in the tree skips a heading level.** Measured sequences are `h1 h2 h2 …` throughout, with
  `h1` re-entering only at a vertical-stack lead. So E3's divergence between `check` (exempt) and the
  preview client (not exempt, `client.js:231-238`) is **latent, not live** — no deck currently trips
  it. It is filed in the register rather than as an item because nothing is presently wrong; the note
  exists so that a future deck that *does* skip is understood to be reported in one surface and not
  the other, by design in neither.

---

## Measured healthy

A confirmation is a valid result, and three of these actively refute plausible-sounding claims this
round could otherwise have filed.

1. **The QR encoder is the best-tested code in the deck subsystem, possibly in the tree.**
   `deck_qr_golden.rs` extracts the *actual bundled* encoder from `deck.js` and runs it through
   `node` against six golden fingerprints spanning versions 1-10, byte-mode UTF-8, `file://` and
   multi-block symbols, each confirmed scannable by opencv decode. "2,690 lines of JS with no tests"
   would have been a false finding: the one region with real algorithmic surface has a genuine
   regression net. The untested part is DOM behaviour, which is what item 112 says.
2. **`deck_key_sheet.rs` is a good pin with a real history** — it exists because the key sheet drifted
   from the bindings twice.
3. **Slide-id dedup, heading sequences, and the E2 rationale** — all three verified above.
4. **The four documented deck exemptions other than E1** (E2, E4, E7, E8) are each correct on their
   stated rationale, and E7/E8 have live replacement coverage.

---

## Not measured

Recorded so this round is not later mistaken for full deck coverage.

- **`deck.js` was not read line-by-line at audit depth.** Its structure was mapped (18 sections:
  grid/camera, camera clock, backgrounds, auto-animate, fragments, magic-move morph, fit/refit,
  navigation, overview, zoom, presenter + cross-window sync, hash, slide number, keyboard + touch,
  mobile feed, plugin API, QR, chrome, lifecycle) and its coverage quantified, but the read the spec
  asked for — 2,690 lines against the standard the rest of the tree is held to — was not done. The
  presenter cross-window sync (`:1277-1532`) is the largest untested region with cross-context state
  and is the place to start.
- **No browser session was run this round.** Every result above is from source or from the CLI.
- **`deck.css` (1,100 lines) was not audited.**
- **Whether the site preview hot-reloads an edited embedded deck** — the deck branch registers no
  `PageState`, which is suspicious, but it was not tested.

---

## Numbering collision — needs an author ruling before any of this is merged

`notes/backlog.md`'s RESUME block states that the concurrent `critique-pass-2026-07-27` branch "does
NOT touch `notes/backlog.md`". That was true when written and is **now false**: its commit `324f2cb`
("notes: file the 2026-07-28 critique round as backlog items 79-90") rewrites 442 lines of that file
and issues **items 79-90 for entirely different findings** than Wave 1's items 79-108.

Two live branches therefore both claim 79-90:

| Number | On `book-drawer-section-highlight` (Wave 1) | On `critique-pass-2026-07-27` |
|---|---|---|
| 79 | `--no-exec` does not stop browser-side code | a multi-root block is only half-mounted |
| 80 | `mounts:` resolves an unbounded path | `textDocument/rename` corrupts source |
| 81 | `check` spawns a project-chosen interpreter | three web-manifest defects |
| … | … through 108 | … through 90 |

This round numbered from **109** to sit clear of both, but that is a workaround, not a resolution.
Merging the two branches will need one renumbering pass, and the "item numbers are stable and never
reused" rule means whichever set moves has to be updated in its findings docs too.

---

## Round bookkeeping

Per the slate's guardrails, this round wrote only this file. `notes/backlog.md`, `notes/AUDITS.md`
and `notes/LESSONS.md` are untouched and are the coordinating session's to update.

**One method lesson for LESSONS.md**, if the author wants it kept: *a probe whose every cell is
negative is a broken probe until proven otherwise.* The shape-gap inventory's first run returned
`NONE` for all 27 constructs because zsh does not word-split `$DECKS` in a `for` loop. It was caught
only because one row (`auto-animate`) had been measured by hand minutes earlier and was known to be
non-empty. **Every table-shaped probe should carry a known-positive row as its own sanity check.**

**Remaining slate:** Wave 2's R6 (ATAM) and R7 (FMEA) — R7 should now score the deck's Detection axis
against the register above, and item 109 means the honest score for the whole deck surface is 10
("would not be caught"). Wave 3 (R2, R8, R9, R11) needs the tree to itself. R12 (real-device mobile)
and R10/R13 are independent.
