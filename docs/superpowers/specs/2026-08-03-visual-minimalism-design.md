# Visual minimalism pass — rank every reader-facing UX feature, cut below the line

**Date:** 2026-08-03
**Status:** design, approved in outline; not executed
**Owner rulings this session:** scope = chrome + constructs on two separate scales;
a11y rule = exempt what adds no pixels at rest; cutoff = delete T4 + T5.

## Why

The author's report: "I get overwhelmed by Taliesin … the placement of everything is
nice, and the UX is genuinely polished. I just feel like it can be a lot, and the main
goal is to let the reader focus on the text."

Both halves of that are true, and the reason they are compatible is structural.
**Taliesin has no quiet default.** A built book page persistently shows brand +
Chapters + search + gear + download in a sticky bar that follows the reader down the
page, and then at runtime injects a `#` on every heading, a copy button on every code
block, a hover card on every citation and cross-reference, a lightbox on every figure,
and a "Referenced by" line into referenced blocks. Each is individually well-built.
None is ever *off*. The reading column is never allowed to be just text.

So this is not a craft problem and not a feature-count problem. It is that every
capability was wired as unconditional chrome.

## What this pass is NOT about: bytes

Measured 2026-08-03, and it corrects an impression the preview gives.

| surface | size |
|---|---|
| Preview page (`using/writing.html`) | 1,094,772 B — **63% inline CSS, 33% inline JS, 4% document** |
| **Built** page, same document | **69,727 B** |
| Built shared `app.css` / `app.js` | 70,087 B / 89,148 B (cacheable) |
| Built page range across the guide | 46,518 – 132,040 B |

The 1.07 MB figure is **preview-only inlining for hot reload**, not what a reader
downloads. A built prose page costs a reader roughly 330 KB on first load and near
zero after. **Nothing in this pass is justified by weight.** Every cut below is
justified by attention.

## The scale

The question asked of each feature: **what breaks for a stranger reading a document if
this is gone?** (Audience = strangers is a standing ruling; adoption is evidence, never
a verdict.)

| Tier | Meaning |
|---|---|
| **T1 Load-bearing** | The document is unreadable or unnavigable |
| **T2 Structural** | A retained format loses its core job |
| **T3 Convenience** | Saves a real manual step a reader cannot easily do |
| **T4 Nicety** | Pleasant; a browser-native action replaces it |
| **T5 Speculative** | Solves a problem no stranger reported having |

**Cutoff: T4 and T5 are deleted. T3 and above survive.**

### The a11y exemption

Agreed rule: **if a feature renders nothing until a keyboard or AT user summons it, it
costs zero visual complexity and is exempt from the cut.** This protects conformance
already paid for (`notes/2026-07-25-ap7-accessibility-audit.md`,
`notes/2026-07-28-conformance-acr-audit.md`) without letting a WCAG label shield
permanently visible chrome.

Exempt regardless of tier: skip link (`06`), focus trap (`04`), scroll-a11y regions
(`16`), focus-visible rings, and keyboard shortcuts (`07`, 82 lines — `/` search, `?`
menu, arrows for prev/next chapter). Anything with a permanent visible affordance is
ranked on its merits.

---

## Scale 1 — always-on chrome

### Survives (T3 and above)

Reading typography and the measure; theme resolution + pre-paint bootstrap; the theme
picker; book chapter navigation; site navbar; prev/next chapter nav; in-page TOC +
scrollspy; footer; listing cards + grid; figure numbering + captions;
footnotes/bibliography; code copy buttons; Cmd-K search; section numbers.

### Deleted — T5, acts without the reader asking

| # | Feature | Files | Lines |
|---|---|---|---|
| 1 | Citation/xref hover previews | `12-link-preview.js`, `site/hover.rs`, + the `hover-index.js` build artifact | 244 + 199 |
| 2 | Reading-position resume + "Continue reading" pill + **TOC read checkmarks** | `15-reading-progress.js` | 141 |
| 3 | "Referenced by" backlinks | `site/backlinks.rs`, `site/sentences.rs` | 477 + 252 |
| 4 | Video hover-play | `18-media.js` | 118 |
| 5 | Mobile floating "Contents" pill | `web-client/toc-sheet.js`, `tali-toc-handle` | 184 |

**Rationale, per item:**

1. The single most "a lot" behaviour on the page: it fires on **passive mouse
   movement**, uninvited, over every citation and cross-reference. A reader who moves
   the pointer while reading gets popups they did not ask for.
2. Finishes a deletion already begun. `15-reading-progress.js:4` records that the
   ambient top progress bar was deleted 2026-08-02 "because it duplicates the native
   scrollbar". Resume-position is the same argument left unfinished; browsers restore
   scroll on reload and back-navigation natively (they do not restore across days,
   which is the residual loss and it is small).
   **Third sub-feature found while reading the manual, not in the initial inventory:**
   `reading.tmd:103–105` documents **read checkmarks in the TOC** — sections you have
   scrolled through gain a mark. This is the same file and dies with it. It is the
   clearest T5 case of the three: it animates the TOC in response to nothing the reader
   asked for.
3. Injects a reverse-reference line into the target block. Wiki-brain; a linear reader
   never asked for it. `sentences.rs`'s only consumer is `backlinks.rs` (verified), so
   the pair cuts cleanly.
4. Playing on passive `pointerenter` is motion the reader did not request — the exact
   territory of WCAG 2.2.2. The fragment already had to build three input paths to be
   fair across mouse/keyboard/touch, which is evidence the affordance was fighting itself.
5. Duplicates the topbar, which is already sticky and already carries Chapters.

### Deleted — T4, a browser-native action replaces it

| # | Feature | Files | Lines |
|---|---|---|---|
| 6 | Heading/figure `#` anchor links | `02-anchor-links.js` | 56 |
| 7 | Image/mermaid lightbox | `11-lightbox.js` | 292 |
| 8 | Chapter-outline disclosures in the drawer | `19-book-outline.js` | 249 |
| 9 | Book download button | `tali-book-download` chrome (`chrome.rs`, `site.css`) | — |
| 10 | Category filter chips + vocabulary linter | `10-category-filter.js`, `site/categories.rs` | 110 + 181 |
| 11 | Reader show/hide code toggle | `20-code-visibility.js` + its pre-paint API | 68 |

**Rationale, per item:**

6. The TOC already emits deep links. Its own header comment justifies it as
   complementing "the selection toolbar's text-fragment Share" — **that selection
   toolbar does not exist anywhere in the tree** (verified by grep). The comment points
   at a removed feature, so the stated justification is stale.
7. Browsers open images in a new tab and pinch-zoom natively. **Flagged loss:** complex
   mermaid diagrams on mobile lose their only comfortable inspection path. Accepted as
   part of the cutoff; revisit only if a real reader reports it.
8. A second navigation layer inside a drawer that is already a navigation layer. Its
   header comment justifies it against a 60-chapter book ("at 12 chapters that is
   fine"); the largest real book in the tree is `docs/guide` at 25 chapters.
9. One more permanent button in the topbar for an action a reader rarely wants.
10. Pays off only on a blog with many posts *and* disciplined category vocabulary — the
    linter exists precisely because that discipline does not hold by default.
11. The author already decides per cell with `echo:`. A reader override of an author's
    presentation decision is a real but rare need, and it costs a permanent row in the
    Settings menu.

**Total deleted: 2,571 lines** (measured, not estimated), plus the `hover-index.js`
build artifact and the associated CSS.

### Kept despite ranking T4 — the exemption applies

`07-keyboard.js` (82) renders nothing at rest, so it survives. **It must be pruned, not
deleted:** its `?` cheatsheet enumerates shortcuts, and some of those shortcuts belong
to deleted features. `04-focus-trap.js` (40) is still required by the surviving reader
menu (verified: the lightbox does not use it, so cutting #7 does not orphan it).

### The exemption's premise partly fails here — an owner decision

`reading.tmd:151–160` documents a **Shortcuts on/off control** sitting in the Settings
menu. It exists because **WCAG 2.1.4 requires character-key shortcuts to be switchable
off** (a bare letter is easy to fire by accident, especially with speech input).

So `07-keyboard.js` is *not* actually zero-pixel: keeping the `?` and `/` character
shortcuts **forces a permanently available visible control** into the Settings menu.
The exemption admitted it on a premise that only half holds. Two consistent ways out:

- **(a) Keep both.** Shortcuts stay, and their mandatory off-switch stays as a Settings
  row. Status quo; conformant; costs one row in the menu.
- **(b) Delete the two character-key shortcuts (`?`, `/`) and their off-switch.** `Esc`
  and `←`/`→` are **not** character keys, so WCAG 2.1.4 does not apply to them and they
  stay live with no control needed. This removes a Settings row *and* stays conformant,
  and it is the option consistent with this pass's direction.

The cost of (b) is that `/`-to-search disappears; the gear and the search button remain
the way in. **Recommend (b). Not ruled — flagged for the owner.**

---

## Scale 2 — author opt-in constructs (gentler cut)

These cost a reader nothing until an author writes them, so the bar is lower. Cut only
redundancy and never-referenced surface.

| Change | From → to | Measured usage |
|---|---|---|
| Callout kinds | 5 → 3 (`note`, `warning`, `tip`) | `important`/`caution` appear in 7 documents, all of them the manual or the `corpus/callouts/kinds.tmd` pin. Readers cannot decode *important* vs *warning* vs *caution* visually. |
| Margin-content spellings | 4 → 1 (`column-margin`) | `.aside` and `.marginnote`: **zero** uses. `.sidenote`: one, `samples/paper.tmd:62`. The aliases were a Quarto/Tufte/Distill welcome mat for a tool that has otherwise shed its Quarto vocabulary. |
| Theorem kinds | 8 → 5 | `exm`/`prp`/`rem` are never cross-referenced; used in 2 documents (`docs/guide/using/theorems.tmd`, `corpus/refs/theorems.tmd`). |

**Explicitly NOT cut:** `scrolly`, `code-walkthrough`, `{glsl}`, `numerics`,
`magic-move`. These shipped as deliberate roadmap items each with a corpus pin (the
explorable cluster, items 153–157). Cutting them reopens a ruling rather than trimming
fat, and is out of scope for a visual pass.

---

## Execution cost — the part that is easy to underestimate

This is not a delete-the-files change. Each removal has a gate tail.

### Drift gates

- **A retired front-matter key trips EIGHT gates**, two of them outside `taliesin-core`
  (`crates/server/tests/agents_md_cli.rs` and `editor/vscode/schema/tali-site.schema.json`).
  `cargo test --workspace` can be green while both are stale — **only `./tools/gates.sh`
  catches them.**
- **`RETIRED_KEYS` is scoped `(scope, key, note)`.** The category cut retires
  `listing.categories` only; page-level `categories:` front matter and the card badges
  survive. Do not flatten the register.
- **A withdrawn div class REQUIRES a `RETIRED_DIV_CLASSES` entry.** Div classes are an
  open vocabulary: without one, a leftover `.sidenote` gets **silence**, not a
  did-you-mean, and the page quietly loses its layout.

### The browser canary — the wave-4 trap

`tools/gates.sh:88` defines a **fifth browser-backed canary specifically for the
figure lightbox**:

```
CANARY_LIGHTBOX="clicking_the_enlarged_image_closes_the_lightbox"
```

Deleting the lightbox therefore requires removing that canary and its
`TALIESIN_REQUIRE_*` wiring, and `crates/core/tests/gate_script.rs` pins the canary
names by parsing `CANARY_` prefixes out of the script — so it must be updated in the
same commit. `crates/server/tests/reader_chrome_browser.rs` (5 tests) exercises
lightbox + TOC handle + reading progress, i.e. three cut features, and is deleted whole.

### Tests to delete or amend

`crates/core/tests/hover.rs`, `backlinks_are_exercised.rs`, `xref_backlinks.rs`,
`crates/server/tests/reader_chrome_browser.rs` (delete); `corpus.rs`,
`retired_names.rs`, `gate_script.rs`, `tech_blog.rs`, `book_has_no_rail_toc.rs`,
`build_reproducibility.rs` (amend).

### Corpus — the regression net

`corpus/reader/hovercards.tmd` is a dedicated pin for a deleted feature and is deleted
with it. `corpus/media/screencast.tmd` needs its video play path re-pinned to native
controls. `corpus/callouts/kinds.tmd` and `corpus/refs/theorems.tmd` shrink to the
surviving vocabulary. `corpus/tech-blog/blog.tmd` and `projects.tmd` drop
`listing: { categories: true }`.

### Docs

`docs/guide/using/reading.tmd` (190 lines, 9 sections) loses three whole sections —
"Reading progress and resume" (92–105), "Hover cross-reference cards" (107–113),
"Anchor copy-links" (115–120) — plus the "Code" section (49–67) and the image-viewer
clause in the a11y section (171–172). Its `description:` front matter (line 3) names
three deleted features and must be rewritten.

**Correction to an earlier claim in this spec's own drafting: the page does NOT become a
stub.** Measured against the file, five sections survive substantially — The Settings
menu, Theme, Running it yourself, No focus mode and no fullscreen, and Keyboard and
accessibility — and they carry the majority of the page's words. The deletions total
roughly 50 of 190 lines. **Keep the page and trim it; do not fold it.**

**Editorial recommendation.** This page already works partly as a register of what was
built and then deliberately removed, and why: the sepia theme (37–42), focus mode and
fullscreen (122–136), the right-rail TOC (130–132), the progress bar (95–97), and the
text-size knob (44–47). The features cut by this pass should be *added to that record*
rather than silently erased. That is in character for the page and it stops a future
session re-proposing them.

Also affected: `using/writing.tmd` (margin aliases), `using/formats.tmd:373,386`
(`categories: true`), `using/theorems.tmd`, `reference/shortcodes.tmd`,
`docs/internals/client.tmd`, and `docs/guide/using/from-quarto.tmd` (a retired key must
tell a migrating reader what to do).

### One behavioural dependency that must land in the same change

Cutting **both** the lightbox (#7) and video hover-play (#4) removes *every* play path
for `{{< video >}}` — click-to-lightbox was the touch path. The `{{< video >}}`
shortcode must fall back to native `<video controls>` in the same commit, or the
shortcode breaks.

---

## Verification

The pre-push hook does **not** run browser suites, so a green push is not a green
`gates.sh`. This change touches a browser canary, so it must be verified with
`./tools/gates.sh` specifically, not `cargo test --workspace`.

Visual verification needs the chrome-devtools MCP at the three-viewport matrix (mobile
~390×844, laptop landscape ~1440×900, laptop portrait ~900×1440). **Blocked at time of
writing:** an orphaned headless Chrome (PID 1211877) holds
`~/.cache/chrome-devtools-mcp/chrome-profile` with `--remote-debugging-pipe`, so it can
be neither attached to nor shared. It must be cleared before the visual check.

## Open question for the owner

**One, and it is above:** keep the `?`/`/` character-key shortcuts plus their
WCAG-2.1.4-mandated off-switch (a), or delete both and keep only the non-character keys
`Esc` / `←` / `→` (b). Recommend (b).

The `reading.tmd` question raised during drafting is **resolved**: the page does not
become a stub, so it is trimmed in place, not folded.
