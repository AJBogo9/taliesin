# The Instrument theme: Taliesin's default visual system

**Date:** 2026-08-14
**Status:** design, awaiting review
**Author decisions taken before this was written:** scope is *everything a person sees*
(reading theme, marketing site, gallery, brand mark, preview dev UI); *anatomy is fair game*
(components may be restructured or deleted, not merely restyled); the tool owns *two* faces
(a serif for prose **and** headings, plus a bundled mono); the identity is *purely
typographic* — **no narrative anchor**; and the full ~3,000-line cut rides along in the same
pass.

> **The no-narrative rule is load-bearing.** Neither the Welsh bard nor Frank Lloyd Wright's
> Taliesin may be cited as a reason for any value in this document, and no future session may
> reintroduce one to justify a colour. Where a warm paper or a dark ink is specified, the
> reason given is typographic or perceptual, never a story. The research's own top-ranked
> direction was Wright-anchored and was **rejected on this ground**, not on its merits.

---

## 1. The thesis, in one sentence

**The mono is the machine's voice; the serif is the author's — and colour on a Taliesin page
means data.**

Everything the *tool* says (labels, figure and table numbers, table headers, callout kinds,
nav, TOC, cell timings) is set in a small tracked mono. Everything the *author* wrote is set
in the serif. Nothing in the furniture is coloured, so the only colour a reader meets is a
plot, a syntax token, or an error.

That single rule generates the whole system, is checkable on a rendered page, and is a
position the peer set has almost entirely refused to take: of fifteen documentation themes
surveyed, one ships without an accent colour.

**Why this rather than a warmer, accented direction.** Both were built and rendered on
identical content before choosing (`scratchpad/specimen/`). The accented variant is warmer
and yields a shape asset; it was not chosen. This document does not relitigate that.

---

## 2. What is wrong today, measured

Every number here was measured on a rendered page or computed from a font binary on
2026-08-14. Nothing is inherited from a report unchecked.

| Defect | Measurement |
|---|---|
| The line is too long | column 640→**736 px**, real-English advance 7.67 px ⇒ **96 characters** of capacity, 80–92 realized. WCAG 1.4.8 AAA caps at 80 |
| The body face fights the math | KaTeX renders at a fixed 1.21 em. Newsreader x-height 0.4531 em ⇒ math/body ratio **1.168**; ceiling is 1.08 |
| Two of three voices are not owned | `--tali-font-head: ui-sans-serif, system-ui` and `--tali-font-mono: ui-monospace` — headings and *all code* render differently per OS |
| The code palette belongs to GitHub | `base.css:251-259` + `dark.css:11-16` are GitHub Primer Light/Dark Default, wholesale |
| The brand mark is a Tailwind artifact | `site/favicon.svg`: `rx=14`, `#1e293b` (slate-800), `#e2e8f0` (slate-200), `#4c6ef5` (indigo-500) |
| …and the two copies disagree | `site/favicon.svg` `#4c6ef5` vs `web-client/favicon.svg` `#4c8dff` |
| The doctrine is enforced in 5 files and violated in 9 | `no_vendor_default_colours_remain_in_any_bundled_stylesheet` (`tests.rs:3502`) bans `#4c8dff` and `#4c6ef5` as "the single loudest *assembled from framework defaults* tell". Both ship — in both favicons, the VS Code icon, the dev UI (6×), and every demo plot and 3D scene. The test passes because it scans only the five stylesheets where the doctrine had already been applied |
| The preview dev UI is a second design system | `serve/mod.rs:380-467`: hardcoded `#d9a23a` `#3fb950` `#e5534b` `#4c8dff`, radii 999/9/6/4 px, its own shadows, none of it tokenized |
| Geometry advertises more than it has | `tokens.css` documents "three roundness tiers, three elevation shadows, two motion durations"; the sheets contain three radii, **one** shadow and **one** duration |
| Spacing has no scale | 39 distinct rem values, 17 off any plausible grid, with `.35rem` the second most common |

---

## 3. Invariants

These bind every surface: reading page, marketing site, book, preview chrome.

| | Value | Why |
|---|---|---|
| Body | `1.25rem / 1.55` (20 px / 31 px) | comprehension peaks well below speed-optimal; 20 px keeps weight 400 legible without APCA's weight penalty |
| Measure | `--tali-measure: 32em` of the **body face** | **never `ch`** — `ch` is the advance of `0` and overshoots real lowercase by 12–55 %, so the same `65ch` is 73 characters in one face and 101 in another. Verified: 32 em of Literata at 20 px = 640 px = **67 characters** |
| Vertical unit | `U = 1.55 × 1.25rem = 1.9375rem (31px)` | derived from the line box, so the rhythm is the leading. This is also why the Tailwind 4 px lattice cannot be reintroduced piecemeal |
| Spacing scale | `{0.5U, U, 2U, 3U}` and nothing else | replaces 39 ad-hoc values |
| Space distribution | above a thing, not around it; heading `margin-top : margin-bottom` = 3:1 or 4:1 | binds a heading to the section it owns |
| Radius | one token, **2px**, on interactive objects only (copy button, search input, `kbd`, focus ring). `0` on `pre`, `table`, `figure`, callouts, cards, drawers, page frame | the 8–16 px ladder is the single widest measured gap between generated and authored interfaces. Flagged in §12 as the weakest-evidenced number here |
| Shadows | **none**, anywhere | also: `box-shadow` is forced to `none` under `forced-colors`, so anything drawn with one is already invisible to some readers |
| Backdrop blur | **none** | two verbatim `saturate(1.4) blur(9px)` rules go |
| Hover | may change an underline or a ground. **It may not move anything.** | |
| Motion | one duration, `--tali-dur: .1s` | `--tali-dur-slow` has zero consumers |
| Colour in chrome | **none** | §1 |
| Text dimming | never `opacity`; every text colour an explicit, scored hex | `opacity` is a colour nobody chose, and it fades inline links too |

---

## 4. Typography

### Faces — two, both owned

| Role | Face | Payload | Measured x-height | math/body |
|---|---|---|---|---|
| Prose **and headings** | **Literata** variable, roman + italic | 52.5 + 53.7 = **106 KB** | 0.5156 em | **1.027** |
| The machine's voice **and code** | **JetBrains Mono** variable, `calt` dropped at subset time | ~40 KB as fetched; ~14 KB once `calt` is dropped | 0.5625 em | — |

Measured by canvas `TextMetrics` at a 400 px em on 2026-08-14
(`scratchpad/specimen/xheight.html`); re-runnable.

- **This is a net reduction.** Literata's 106 KB replaces Newsreader's 121 KB, and buys a face
  that fits the math instead of fighting it.
- Rejected on measurement: Newsreader (1.168), Charis SIL (1.093, over the ceiling).
  Source Serif 4 (1.059) is the fallback if Literata's licence or rendering disappoints.
- **Mono is set at `0.92em`** so its x-height matches Literata's (0.5156 / 0.5625 = 0.917).
  This is derived, not chosen; it must be re-derived if either face changes.
- Dropping `calt` at subset time is deliberate: it halves the file **and** makes ligatures
  un-re-enableable by a stray rule. Code ligatures misrepresent the source characters.
- `--tali-font-head` is **deleted**, with all 18 consumers.

### The machine's voice

`0.78rem`, uppercase, `letter-spacing: .053em`, weight 400, in the mono. Applies to: `h4`,
callout kind labels, table headers, figure/table/equation *numbers*, the TOC, nav, footer,
the title-block meta line, cell timings, and the dev menu.

> **Correction, from the render.** Captions are **prose** and stay in the serif (italic,
> 0.92 rem). Only the `Figure 3` *number* takes the machine voice. The first render set whole
> captions in mono and they read as terminal output. Sidenotes and margin notes are likewise
> authored prose: serif.

### Scale

`h1` 2rem/1.1 w600 · `h2` 1.35rem w600 · `h3` 1.12rem w600 · `h4` = the machine voice.
Four levels, then stop. No `font-size` in `px` anywhere (seven survive today).

---

## 5. Colour

Every ratio below is WCAG 2.x, computed from the sRGB relative-luminance definition on
2026-08-14 (`scratchpad/palette.py`), and re-derivable. **APCA is deliberately not quoted:
it is a different model and a guessed Lc is worse than an absent one.** Where the research
supplied Lc figures they are treated as advisory, not as gates.

### Light — ground `#FBF9F5`

| Role | Hex | Ratio |
|---|---|---|
| ink (body) | `#22201A` | 15.49 |
| muted (machine voice) | `#5F5C54` | 6.35 |
| inline code | `#3A362E` | 11.43 |
| rule, decorative | `#D9D7D2` | 1.37 (non-text separator) |
| rule, control boundary | `#8B887F` | 3.37 (clears the 3:1 non-text floor) |
| code ground | `#F4F1EB` | ink 4 % over paper |

### Dark — ground `#14130F`, designed independently

| Role | Hex | Ratio |
|---|---|---|
| ink | `#EAE7E0` | 15.05 |
| muted | `#D0CCC3` | 11.60 |
| inline code | `#DBD7CE` | 12.94 |
| rule, decorative | `#33312B` | 1.43 |
| rule, control boundary | `#7C7972` | 4.28 |
| code ground | `#1C1A15` | |

**The dark muted tier is nearly as bright as the body, and that is correct here.** A dark
muted dark enough to *look* secondary fails perceptual contrast. In this theme the secondary
register is carried by **face, size and tracking** (it is the mono voice), not by lightness —
so muted can stay bright and still read as secondary. This is a direct dividend of §1, and
it is the reason the usual dark-mode muted-grey trap does not apply.

Ground is `#14130F`, not `#000000`: pure black buys almost nothing and adds halation.

### The owned syntax palette

Four hues sharing one warm-anchored chroma envelope, replacing twelve borrowed GitHub Primer
hexes. Comments are italic, so hue is never the only cue.

| Scope | Light on `#F4F1EB` | Dark on `#1C1A15` |
|---|---|---|
| comment (italic) | `#6E6A60` 4.78 | `#8C877C` 4.86 |
| string | `#3F6152` 6.12 | `#8FBBA3` 8.12 |
| keyword / storage | `#7A3B52` 7.23 | `#D99BB0` 7.67 |
| constant / support | `#3A5578` 6.77 | `#9DB4DA` 8.26 |
| entity (fn, type) | `#6B4A2F` 7.04 | `#D0A67C` 7.80 |
| variable | `#22201A` 14.45 | `#EAE7E0` 14.08 |

All twelve clear AA. The palette a reader currently sees bottoms out at 4.63 (`#8250df`).

### Callout kinds

Three kinds, distinguished by a 2 px left rule and the mono kind-word. The five callout
tokens collapse to three; `--tali-callout-caution` (zero consumers) is deleted. The two
diagnostic surfaces (`stderr`, kernel error) keep their own named colours.

---

## 6. Layout

### The bleed grid — one definition, replacing three

Today the width-escape arithmetic exists in three near-identical copies
(`base.css:484-534`, `base.css:691-717`, `site.css:98-112`). Replace with one grid:

```
grid-template-columns: 1fr 1fr minmax(auto, var(--tali-measure)) 1fr 1fr;
article > * { grid-column: 3; }
```

with escape classes opting into wider tracks.

**This is what fixes the clipping visible in every render in this design's history:** a `pre`
must leave the prose measure rather than scroll inside it. A 640 px column at mono 0.92 em
fits ~58 columns against PEP 8's 79, and no single width serves both — so prose stays at 32 em
and code escapes. Prose never widens to accommodate code.

### The margin column

`16em` beside the `32em` measure with a `3em` gutter, engaging near 1022 px (today: 1168 px).

**Below the breakpoint the note renders inline with a back-link to its reference.** Today it
is `display: none` behind `:target` with no way back — the one component whose reduced form is
a defect rather than a simplification. This is a bug fix riding along with the redesign.

---

## 7. Brand identity — purely typographic

The favicon, the CLI banner glyph and the VS Code icon become **one mark**, and it is a
letterform, not a picture.

**Concretely:** a single `T` set in JetBrains Mono weight 500, ink (`#22201A`) on paper
(`#FBF9F5`), on a **square** 64×64 canvas with `rx="0"`, optically centred, cap-height
occupying ~62 % of the canvas. The dark variant swaps the two colours and nothing else. No
rounded rect, no bars, no third colour, no gradient. The `.tmd` file icon is the same mark;
only its canvas differs.

Deleted: `rx=14`, `#1e293b`, `#e2e8f0`, `#4c6ef5`, `#4c8dff` — every one of them from a
framework palette, and two of them already on the project's own banned list.

The distinctive asset is the mono machine-voice itself: the property that every non-authored
word on the page is set in one small tracked mono. It survives greyscale, dark ground and
16 px, and no peer tool does it.

---

## 8. The preview dev UI

`serve/mod.rs`'s inline CSS is rewritten against the token layer. Its four hardcoded status
hexes become **named, scored status tokens** (live / warming / warn / error) shared with the
diagnostic surfaces, its radii collapse to the one radius, its shadows go.

Fix, not restyle: `serve/mod.rs:472` reads `--tali-mono`, **a token that has never existed**
(`tokens.css` defines it zero times — verified). It survives because
`every_tali_custom_property_read_is_defined_somewhere` explicitly exempts
`var(--x, fallback)` references, and this one is `var(--tali-mono, monospace)`. So the
dev-menu cell badge has silently never used the page's mono stack. The correct name is
`--tali-font-mono`.

Deleted from the dev menu: the section-annotations panel (three of its four columns are
duplicated three rows below in the same panel), the static "Cache" prose row, the empty
"Sections" label row, the client-side a11y scanner (duplicates the server, re-implements a
check wave 9 deliberately cut, double-counts its own badge, and renders unstyled because every
`.tali-diag` rule is scoped to a different id), and the canvas favicon dot.

Emoji (`⚡ ✗ ⚠ ♿ ⏳ ✓ ✕ ●`) leave the dev chrome with them; the machine voice replaces them.

---

## 9. The cut

Approved in full. Ranked by size; each lands in the same commit that restyles its neighbours.

| # | Cut | ~Lines |
|---|---|---|
| 1 | Structured `author:` — `author.rs`, byline/appendix/affiliations, `base.css:194-222`, `corpus/structured-authors/`, the reference page section. Scalar/list `author:` and the byline survive at ~20 lines | ~750 |
| 2 | Cmd-K runtime on **standalone** builds (kept on sites and books) | 1,039 shipped bytes/page |
| 3 | Knobs a better default already answers (§10) | ~350 |
| 4 | Dev-menu section-annotations panel | ~150 |
| 5 | Client-side a11y scanner | ~140 |
| 6 | Search-hit whole-page flash + `<mark>` fallback | ~137 |
| 7 | Affiliations + contributions appendix | ~87 + 29 CSS |
| 8 | `_extensions/<name>/theme.css` arm (also removes a silent-failure path) | ~75 |
| 9 | `column-screen` | ~70 |
| 10 | Brand home-url fallbacks + 4 tests | ~70 |
| 11 | Callout `appearance=`, `icon=`, the Octicon blobs | ~35 |
| 12 | Category chips, monogram placeholder, reading time, chapter word counts | ~92 |
| 13 | `07-keyboard.js` arrow-key nav (undiscoverable since its cheatsheet was cut; also hijacks arrow keys inside wide `<pre tabindex=0>` in books) | 28 |
| 14 | Canvas favicon dot | 26 |
| 15 | Residue: `data-tali-cell`, `align_class`, nav `data-label` reservation, dead tokens, shadow uses, blur rules | ~30 |

**Verified while writing this, not taken on report:**

- **`crates/core/src/author.rs` (284 lines) declares three consumers and two are dead.** Its
  own doc comment names the byline (`render/mod.rs`), JSON-LD `Person` (`site/meta.rs`) and
  the Atom feed (`site/feed.rs`). `meta.rs` contains **zero** JSON-LD (cut in wave 4), and
  `feed.rs:162` reads `self.config.authors`, a flat `Vec<String>` from `_site.yml`, not an
  `author::Author`. The only live consumer is the byline. Fix the doc comment in the same
  commit.
- **`data-tali-cell` passes the orphan gate by being a *prefix*.**
  `every_emitted_attribute_has_a_runtime_consumer` tests `sources.contains(a)` — a substring
  match over concatenated source — so the bare `data-tali-cell` emitted at `emit.rs:75` is
  "found" inside the genuinely-consumed `data-tali-cell-state`, and likewise
  `dataset.taliCell` inside `dataset.taliCellState`. Nothing reads the bare attribute. The
  gate cannot distinguish an attribute from a prefix of a longer one; note this when the
  attribute goes, because the same hole covers any future `data-tali-x` / `data-tali-x-y` pair.
- `base.css:860`'s print comment still names "the heading copy-links", deleted at `dc4f3fd0`.

### Retirement register entries required

Per CLAUDE.md, a withdrawn name costs one register line and **the parser must stop reading
it** (a register entry alone leaves the key live):

- `RETIRED_DIV_CLASSES`: `column-screen`
- `RETIRED_KEYS` (`config` scope): `python:`
- `RETIRED_KEYS` (callout scope): `appearance`, `icon`
- `RETIRED_FLAGS`: `--jobs`/`-j`, `--json` (both verbs), `doctor --format human`
- `TALIESIN_CELL_TIMEOUT` removal is env, not vocabulary — it needs a `doctor` note, not a register line

Each entry is **one sentence**: the date, then the successor or an explicit "nothing". No
entry may be phrased as a did-you-mean.

---

## 10. Knobs a better default kills

| Knob | The default that answers it |
|---|---|
| `--jobs` / `-j` | `build_budget.rs` already weighs cores, free memory and cgroup headroom; the flag's only power is to *ignore* the memory budget |
| `TALIESIN_CELL_TIMEOUT` | `TALIESIN_CELL_SILENCE`, which resets on output. Its own comment says it exists to reproduce pre-175a behaviour for a tool with no external users |
| `preview [port]` positional | `--port <N>`, which the code already calls "the more deliberate spelling" |
| `build --json`, `doctor --json` | `--format json` |
| `doctor --format human` | `human` is the default; the value exists only to name it |
| `_site.yml python:` | `interpreter.rs` resolves through five precedence levels with an ancestor-`.venv` walk. Zero witnesses. Cutting it drops the stack to four |
| `theme:` bare-name arm | `theme: file.css` |
| Callout `appearance=` / `icon=` | one right default |
| `--tali-radius-md` / `-lg`, `--tali-shadow-*`, `--tali-dur-slow` | one radius, no shadows, one duration |

---

## 11. Additions — three, all approved 2026-08-14

The default is no. These three clear the bar and **all three were approved by the author**;
A3 was approved specifically in its no-opt-out form.

- **A1. Emit only the KaTeX faces a page's math actually references.** All 20 bundled woff2
  (253.7 KB, ~338 KB base64-inflated) ship on every page; a real guide page uses 5 of 22
  declared faces. Mechanically checkable from KaTeX's own output classes. A pure subtraction
  wearing an addition's clothes; no new vocabulary, flag or gate. **The largest byte win
  available.**
- **A2. Emit fonts as files for `--out <dir>` and site builds; inline `data:` only for the
  genuinely single-file `build <file.tmd>`.** A built page is 222,979 B of which 163,476
  (73.3 %) is base64 font inside a render-blocking `<style>`. The asset-copy path already
  exists. Caveat to handle in the same change: use `font-display: optional` + `preload`, not
  metric overrides — `ascent-override` is unsupported in Safari.
- **A3. The orphan-page diagnostic.** A `.tmd` in no `chapters:`, no `nav:`, no
  `listing: contents:` and linked from nowhere is built and sitemapped but reachable only by
  URL, and the author cannot see it because they are looking straight at it. Every input
  already exists; it is a set difference over data already computed; the drift-gate tax is
  zero. ~70–90 lines + one test. **Approved in the no-opt-out form.** If a future session finds
  it needs a `linked: false` key, that is the signal to *remove the diagnostic*, not to add the
  key: a knob in front of a default contradicts the project's own rule.

**Rejected:** a per-book accent spectrum (would ship 96 unscored colour pairs); heading
permalink anchors (deliberately deleted at `dc4f3fd0`; the real residue is a stale comment);
a reader theme picker (withdrawn 2026-08-13); a browser smoke test (decided against
2026-08-13). A missing in-chapter outline is real, but the fix is to *delete* the book special
case at `site/mod.rs:1025-1032` and let `page_toc` answer per chapter — a subtraction.

---

## 12. Gates, so this cannot rot

The redesign is worthless if the next session reintroduces what it removed. Three gates,
all cheap:

1. **Widen the vendor-hex ban beyond the five stylesheets.** The existing test's own comment
   states the intent; only its file list is too narrow. Extend it to every file that emits
   colour: `serve/mod.rs`, `client.js`, both favicons, the VS Code icon, and the demo `.tmd`
   sources. This single change would have caught today's violation.
2. **A rendered-page tell probe.** On the built docs guide, assert: exactly one non-zero
   border-radius ≤ 3 px; zero `box-shadow`; zero `backdrop-filter`; exactly two font families;
   zero text nodes with `opacity` < 1; zero elements painted in a chrome accent.
3. **Pin the measure in characters, not CSS.** Assert the realized characters-per-line on a
   built page is 62–72. A CSS assertion would pass while a font swap silently moved the layout
   by 21 %.

`docs/guide/using/theming.tmd` is **rewritten and gated against `tokens.css`**: it is wrong on
every colour row today, six of its hexes are on the project's own banned list, and it documents
a `--tali-scale` token that has never existed and a lightbox that was deleted.

---

## 13. Verification protocol

Run against `docs/guide`, `docs/internals`, the composed deploy, `corpus/analyst/index.tmd`,
one `corpus/tech-blog` post, and the specimen page. Chrome via the devtools MCP.

1. **Contrast** — enumerate every distinct (text, ground) pair; all text ≥ 4.5:1, body ≥ 7:1.
2. **Measure** — realized characters-per-line 62–72, via block-height ÷ line-height (**not**
   `getClientRects().length`, which returns one rect for a block and silently reports one line).
3. **Tell probe** — the §12.2 assertions.
4. **Dark** — re-run 1 and 3 under `prefers-color-scheme: dark`; assert it is not an inversion.
5. **Print** — light palette forced; every `<details>` open; TOC expanded; `#tali-progress`
   hidden (it currently prints); `http` links show their URL and `#` links do not.
6. **Focus** — `:focus-visible` only; an `outline`, not a `box-shadow`; re-check under
   `forced-colors: active`, where a shadow-drawn ring vanishes entirely.
7. **Reduced motion** — all durations ≤ 0.01 ms; no view-transition crossfade.
8. **Text spacing (WCAG 1.4.12)** — inject the required overrides; no clipping. Most at risk:
   the cell-output box's `max-height`, drawer rows, callout titles.
9. **The margin column** at 1080 / 900 / 375 px, including the new inline-with-back-link form.
10. **Code and math** — a `pre` escapes the measure and fits ≥ 58 mono columns; `.katex-display`
    scrolls rather than overflows.

Before/after screenshots exist for the specimen (`scratchpad/specimen/`), so the redesign has
a real baseline.

---

## 14. Risks and what is *not* established

- **The radius value is the weakest-evidenced number here.** No experiment has ever tested
  border-radius. The curvature literature manipulates object silhouettes at 85 ms and does not
  reach a 6 px corner. 2 px is inference from measured category convention among admired
  interfaces, not from evidence.
- **"Whitespace improves comprehension 20 % (Lin 2004)" is fabricated** — traced and denied by
  its supposed author. Nothing in this document rests on it. The real finding (Chaparro) is a
  tradeoff: margins made reading slower and comprehension better.
- **The warm-ground argument is brand and preference, never legibility.** The most recent
  controlled test found no reading-performance effect at all in English. It must never be
  defended with blue light, which 17 RCTs say does nothing.
- **Serif vs sans has no reliable legibility difference.** The serif is chosen for register
  and for its x-height against KaTeX, not because it "reads better".
- **A face swap moves layout ~21 %.** The measure must be re-derived from the binary, and the
  §12.3 gate re-run, whenever a face changes.
- **Cutting structured `author:` deletes a corpus project.** Per the ordering rule, the corpus
  document and its pins die in the *same commit* as the feature, never before — a document
  removed early leaves its code unguarded while every gate still passes.
- **Literata's licence must be confirmed OFL and free of a Reserved Font Name** before it is
  vendored, since the build subsets and renames nothing.

---

## 15. Decisions taken 2026-08-14

All four resolved by the author; recorded here so no future session reopens them.

1. **A1, A2, A3 — all three approved**, A3 in its no-opt-out form (§11).
2. **The marketing landing page becomes an editorial masthead and prose.** No centred hero, no
   letterspaced all-caps eyebrow, no three-card feature grid, no repeated bottom CTA. The
   feature grid becomes a definition list or ruled sections. This is a rewrite of
   `site/index.tmd`, not only a CSS change.
3. **`corpus/structured-authors/` is deleted** by cut #1, together with `crates/core/src/author.rs`
   and the byline/appendix/affiliation render paths. Scalar and list `author:` survive.
   **The ordering rule binds:** the corpus project and its pins die in the *same commit* as the
   code they guard, never in an earlier one.

### Licensing — checked 2026-08-14, both clear

Both faces are SIL OFL 1.1 and **neither carries a Reserved Font Name**, so both may be
subset and vendored without renaming — the same footing as the Newsreader they replace.

| Face | Copyright line | RFN |
|---|---|---|
| Literata | `Copyright 2017 The Literata Project Authors` | none |
| JetBrains Mono | `Copyright 2020 The JetBrains Mono Project Authors` | none |

(In each `OFL.txt` the phrase "Reserved Font Name" appears only in the licence's own
definitions section, never appended to the copyright notice — which is where an RFN is
declared.) `THIRD_PARTY.md` gains both entries and loses Newsreader in the same commit.

**Nothing else is open.**
