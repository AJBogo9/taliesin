# Taliesin final scope

**Date:** 2026-08-02
**Status:** design, pending owner review
**Supersedes as the scope authority:** the tier tables in
[2026-08-01-feature-value-audit.md](../../../notes/2026-08-01-feature-value-audit.md)
(that round measured adoption only; three of its findings are corrected below).

## Why this document exists

The tool is being published. The owner's brief: *"I want a tool that only does what it is
supposed to do and nothing else."* This document defines what the finished product is, what
it is not, and the ordered work to get from here to there.

Three owner rulings fix the frame:

1. **Audience: a real product for strangers.** Value means value to a plausible new user,
   not to the author. Adoption is therefore evidence, never a verdict on its own: `author:`
   reads as pin-only here purely because the author's own sites set it in `_site.yml`, and a
   stranger publishing a first post needs it.
2. **Aggressiveness: scope B of three.** Remove the structural duplication and the surfaces
   with no user, keep all four document formats.
3. **Removal means deletion**, not a feature flag. With one standing exception recorded in
   "Preconditions" below, because for an *open* vocabulary plain deletion produces silence
   rather than a diagnostic.

## How the scope was decided

501 user-facing features were catalogued across nine disjoint surfaces (front-matter and
`_site.yml` vocabulary, fenced-div blocks, shortcodes and cell languages, CLI, LSP and
editor companion, site model, reader runtime, build/exec/publish, machine-facing
projections), each record carrying code locations, adoption, dependence, carry cost, origin
and byte cost. Four cross-cutting lenses then reasoned over the whole catalogue: the moat
lens, the inherited-vocabulary lens, a maximal-cut skeptic, and a keep-defender whose job
was to refute the cuts.

Adoption was measured with the tool's own `taliesin features --format json` across all 195
`.tmd` documents, split by authorship: **author writing** (`corpus/tech-blog` 19,
`corpus/tarn` 14, `site/` 7), **the dogfooded manual** (`docs/` 40), **pin fixtures**
(`corpus/` 105). Of 152 tracked authoring features: **63 used in real writing, 19
manual-only, 45 pin-only, 25 used by no document anywhere.**

### Three findings that reframed the question

**The moat is 13% of the tree.** The warm-process core plus both dev servers is ~17,000 of
~131,000 Rust lines. The other 87% competes with Quarto and Hugo on their terms. Cutting
inside the moat saves nothing and costs the product's identity; the entire budget is in the
generic mass.

**Maintenance attention is inverted against the thesis.** Verified by `git log`: the deck
engine has absorbed **178 commits** (`deck.js` 75, `deck.css` 70, `deck.rs` 33) against
**28 combined** for `diff.rs` (13) and `freeze.rs` (15), the two files that *are* the
competitive claim.

**The inherited-vocabulary hypothesis was refuted by measurement.** Inherited and chosen
features are cut or folded at an identical rate (17.4% vs 17.3%), because the shed-Quarto
ruling was genuinely applied, three times, in writing, with dates. The 71% of the surface
that was never inherited has never had an equivalent ruling and carries **3x the freeze
rate**. The bloat is in what was invented, not what was copied. This document is that
missing ruling.

---

## The product

> Taliesin renders `.tmd` files to HTML, live, with a warm kernel and a block-level
> incremental preview that never loses page state, and click-to-source back to the editor.
> It ships four formats from one dialect: a blog post, a slide deck, a book, a multi-page
> site.

Everything in scope serves that sentence. Everything out of scope is named below rather than
left to be re-litigated.

### In scope: the load-bearing core (never touch)

The content-hash block model (`data-block-id` + `data-sourcepos` on every block), the
block-level diff and incremental swap, click-to-source, the warm dev server, the warm Jupyter
kernel, and the cumulative-hash freeze cache. `MAX_WARM_PAGES` and the deterministic LRU in
`serve_site/exec_pool.rs` remain the one standing freeze.

### In scope: table stakes for the four formats

Named explicitly so a later round does not re-derive them as cut candidates. Every one is
generic, every one is measurably used: `title`/`date`/`description`/`categories` front
matter; listings and Atom feeds; nav and footer; Cmd-K search; TOC; figures, callouts and
code blocks with copy buttons; cross-references; theme toggle; 404; favicon; `.tmd` to
`.html` link rewriting; server-side syntax highlighting; KaTeX math; `{{< include >}}`;
citations and bibliography; `check` and the diagnostics engine; `doctor`.

### In scope: the demand-bearing differentiators

`{python}` / `{r}` / `{js}` cells and the freeze cache behind them. The `{js}` reactive
graph, which is the most-adopted novel construct in the tool (21 of 38 documents in the
author's three real external projects) and which replaced 440 KB of vendored Observable
runtime with 110 lines. `{{< input >}}`, `{{< embed >}}`, `{{< video >}}`. The LSP and the
VS Code companion, minus the long tail named in Wave 4. `publish`, which carries the only
successful real-use trace found anywhere in this audit (`--init` at 2026-07-31, then a real
deploy against an external project 27 minutes later).

### Frozen: supported, documented, no further investment

The deck engine. It stays a first-class advertised format and keeps working, but acquires no
new transitions, chrome, presenter tooling or parity features. Roughly 100 lines of it are
genuine moat (live per-slide reconciliation in `client.js:1652` and the slide-transformed
projection at `deck.rs:469`, which together keep `{js}`/WebGL/video state alive across an
edit, something reveal.js cannot do); the remaining ~4,500 are reveal.js parity. Freezing
stops the 6.4x churn inversion without withdrawing a format from a tool about to be
published.

The freeze absorbs one item the skeptic proposed cutting. The hand-rolled in-page QR encoder
(323 LOC) is `qrSvg` at `assets/js/deck.js:2206`, i.e. it is *inside* the frozen engine, so
carving it out would contradict the freeze. It stays until decks are re-judged. Note this is
a different implementation from the terminal QR that `preview --host` prints, which is the
`qrcode` crate at `serve/mod.rs:738` via `print_qr`, is unrelated to decks, and stays
regardless (see the Wave 1.1 constraint, which must preserve it).

Also frozen: the 2026-07-29 explorable extras (`tali.tex`, `tali.table`, `num`, cross-run
state) and `{glsl}`. They are cheap when unused and an API break on a runtime with 21 real
documents is not worth the bytes. Re-judge on evidence of demand, not on a date.

### Out of scope: standing guardrails

Unchanged from `ROADMAP.md` and restated so they are one list:

- New output **formats**. HTML is the identity. PDF stays a paged rendering of the built
  HTML, never a parallel compiler path.
- Preview write-back, WYSIWYG, drag gestures. The `.tmd` is the only editing surface.
- Rewriting Do-NOT-touch machinery. New capability rides the supported seams.
- i18n / RTL, RSS, Julia / knitr engines.
- A hosted service, a reader-side backend, a CDN, telemetry of any kind.

**New, added by this round:** no second mechanism for a job that already has one, and no
capability whose only consumer is its own fixture. Both are what produced the cut list.

---

## Preconditions and standing rules

**A retired register is required for open vocabularies.** Div classes are an *open*
vocabulary: mutation testing measured that with `RETIRED_DIV_CLASSES` not consulted, a
leftover `::: {.columns}` produced **zero** diagnostics, because nothing survives within
edit distance 2. Every div-class cut below therefore ships its `RETIRED_DIV_CLASSES` entry
in the same change. `.aside` most of all: it is both an HTML5 element name and Quarto's own
spelling, so a migrating author will type it. Front-matter keys have `RETIRED_KEYS` for the
same job. This is the one place where "delete the code" is qualified, and it costs one line
per item.

**Batch the retirements.** A new front-matter key trips six drift gates; a retired one trips
**eight**, two of which live in the server crate so `cargo test -p taliesin-core` is green
while they are stale. A dozen separate key removals is ~96 gate edits. All front-matter and
`_site.yml` retirements land as **one** change or the tax dominates the saving.

**Derive, don't declare.** Promoted here from a batch note to a standing constraint. Every
proposed key must answer: what on the page already implies this? Measured ceremony to remove
under this rule: `listing.sort:` is written `"date desc"` on 4 of 4 real listings, which is
what the parser already defaults to; both uses of `_site.yml output:` write `_site`, already
the default; `site/_site.yml` writes `toc: false`, already the default.

---

## Wave 0: before anything is cut

Not scope work. These are correctness items that must not ship as they stand.

| # | Item | Why it is Wave 0 |
|---|---|---|
| 0.1 | **Re-run the live-edit bench and fix the published number** | `RESULTS.md` claims "83x smaller"; `RESULTS.json` on the same machine gives `32,303 / 287,195` = **8.89x**, with `update_count 1` where the committed file says 0. `RESULTS.json` is gitignored (`.gitignore:58`) so nothing regenerates or gates it, and the regression test asserts only a 5x floor. The 83x figure is cited as fact in `ROADMAP.md:125`, `backlog.md:372`, `AUDITS.md:913` and the launch critique. Either the payload regressed 10x (an `Update` where a `SetMeta` used to be, exactly the property being sold) or the headline was always wrong. **Publishing a 10x-overstated benchmark is the single biggest launch risk in this document.** |
| 0.2 | **Fix the `taliesin features` panic** | `render/extension/mod.rs:135` advances a byte cursor with `i += 1`, and `:124` then slices `line[i..]`, so any line containing both `{{<` and a non-ASCII character panics mid-codepoint. It aborts on 3 of 25 `docs/guide` pages. Ordinary prose triggers it. Fix by folding `scan_shortcodes` into `each_shortcode` (`extension/mod.rs:967`), which already does the identical job correctly: deletes the third hand-copied scanner and the bug with it. |
| 0.3 | **Land item 205 (pyodide behind a cargo feature + `exclude`)** | 16,458,446 B, **21.8% of the 75.6 MB binary**, for one fixture. The design exists and is unimplemented (`crates/core/Cargo.toml` has no `[features]` section). `degrade_pyodide_cells` is already compiled unconditionally (`page.rs:601`, `query.rs:77`), so a feature-off build is correct by a contract that already exists. This is a fold, not a cut: full capability retained, 16 MB off every build. |

---

## The cut list, in dependency order

LOC figures are the catalogue's, de-duplicated where two surfaces recorded the same feature.
Totals are approximate and should be re-measured at execution.

### Wave 1: structural duplication (~2,700 LOC)

The highest-value work in the document. Each item removes a *second mechanism for one job*.

| # | Item | LOC | Note |
|---|---|---|---|
| 1.1 | **Fold `serve/mod.rs` into `serve_site` as a one-page project** | ~2,000 | The largest single win. The site model already handles a bare directory. Verified: `grep -c warm_pool` gives **0** in `serve/mod.rs` and **3** in `serve_site/mod.rs`, so the unused copy is the *degraded* one and a `.tmd` with no ancestor `_site.yml` (the companion's fallback) cold-starts its kernel where a site preview does not. This is an upgrade, not merely a dedup: it also removes the surface's only two-owner protocol contract. Zero of 64 `preview` invocations since the rename targeted a single file. **Execution constraint, measured: `serve/mod.rs` is not purely a duplicate.** `serve_site/mod.rs:31-35` imports fifteen items from it (`CLIENT_JS`, `FAVICON`, `STATUS_CSS`, `bind_with_fallback`, `js_str`, `lan_url`, `local_ip`, `new_session_token`, `open_in_browser`, `percent_decode`, `print_qr`, `with_host_guard`, `with_identity`, `with_lan_guard`, `ws_origin_ok`). Ten of them span ~424 lines in `mod.rs`; the other five live in `serve/security.rs` (416 lines), which stays untouched. **Step one is extracting those ~424 lines into a shared module; only the ~2,329-line single-doc remainder is the deletion.** |
| 1.2 | **Fold the two `AGENTS.md` goldens into one generated artifact** | ~220 | Drops the per-key drift tax from six gates to five and kills a byte-identical duplicate kept in sync by hand, whose gate lives in the wrong crate. |
| 1.3 | **Fold `scan_shortcodes` into `each_shortcode`** | 45 | Already required as Wave 0.2. |
| 1.4 | **Delete `editor/claude-code/skills/taliesin/SKILL.md` and the repo-root `AGENTS.md` copy** | ~200 | The third and fourth hand-written copies of a document the tool generates from its own validator consts. |
| 1.5 | **Fold `llms-full.txt` into the `read` projection** | 170 | One projection, three consumers. |

### Wave 2: the config vocabulary, as one batched change (~350 LOC, ~15 keys)

Ships as a single commit with all `RETIRED_KEYS` entries, per "Batch the retirements".

- **Raw-injection family, keep exactly one.** Eight keys at measured zero adoption across 218
  documents and 17 `_site.yml` files: `css`, `include-in-header`, `include-before-body`,
  `include-after-body` (front matter) and `css`, `head`, `body-start`, `body-end`
  (`_site.yml`). **Keep `_site.yml head:`** as the analytics / search-console / custom-CSS
  escape hatch a published tool needs. Cut the other seven. All resolve through one shared
  helper (`doc_includes.rs`), so the marginal carry cost of the family is one function call,
  not eight implementations, and the case for cutting is surface area, not code.
- **Dead sub-keys:** `execute.echo`, `execute.include` (the per-cell `#| echo:` is what every
  real document uses), `hero.image`, `hero.image-alt`, `prose-lint.banned`.
- **Measured ceremony:** `listing.sort:`, `_site.yml output:`, `_site.yml toc:` (fold into the
  heading-count auto-gate that already exists at `site/mod.rs:1237`).
- **`prose-lint:` itself** (~190 LOC): opt-in, never opted into, by the person who writes
  daily. If the capability is wanted later it belongs behind `check --prose`.
- **`theorems:` config** (~140 LOC): a five-file inheritance chain and a public type carrying
  configuration for a family with **one** real use of **one** of its eight kinds
  (`definition`, in `corpus/tarn/grouping.tmd`). The kinds stay; the config goes.
- **Nested `mount` item keys** (`at` / `path` duplication) and the `hero.actions[]` item
  vocabulary, which is unvalidated today.
- **Add the missing `chapters:` entry validator.** Not a cut: a typo'd `fil:` currently
  produces an empty part header that `site/book.rs:167` silently pops, dropping the chapter
  with zero diagnostics. For a published tool this is the worst failure shape in the config
  surface.

### Wave 3: surfaces with no user (~2,250 LOC)

| # | Item | LOC | Evidence |
|---|---|---|---|
| 3.1 | **Academic-publishing cluster**: cite-this box, Google Scholar `citation_*` meta, and the `doi`/`venue`/`award`/`links`/`acknowledgements`/`csl` keys | ~600 | Every key appears in exactly two files: its pin fixture and the page documenting it. Measured **zero** emissions on all four built projects. Six keys at six gates each. |
| 3.2 | **`{{< dataset >}}` provenance card** | 500 | Its only user is the document written to test it. |
| 3.3 | **`skim`: the CLI verb, the layer-cake projection, and the MCP `skim` tool** | ~600 | Value chain empty end to end: zero invocations, zero consumers. **Corrected from the skeptic's 1,400:** verified that `backlinks.rs:93,96` consumes `skim::plain` and `skim::sentence_at`, and `query.rs:967,1238` consumes `skim::PageSkim`. Keep the shared text helpers where backlinks reads them; retire the verb, the projection and the agent tool. |
| 3.4 | **Session revision digest** | 162 | Answers a question git answers better and with history. |
| 3.5 | **`input type="point"` and `type="animate"`** | 250 | Zero real adoption plus a special case in the URL-state serializer. The standard form controls (`slider`, `number`, `select`, `checkbox`, `text`) stay: a stranger needs those. |
| 3.6 | **`map`'s `words` + `headings` per page** | 30 | Zero consumers on both sides; removing them turns `map` from a whole-site render into a cheap read. |
| 3.7 | **`#| fig-export:`** | 40 | Shipped as a LaTeX-workflow affordance for a tool with no LaTeX path. |
| 3.8 | **`ojs_define` bridge** | 65 | Zero uses anywhere including fixtures. **Do the 5-line fix regardless:** narrow `reactive.rs:86` `runtime_defines` from "any kernel cell" to "a kernel cell whose literal contains `ojs_define`", which restores the dangling-input diagnostic on all six real blog posts. |

### Wave 4: the LSP and companion long tail (~1,400 LOC)

The largest single investment in the tool (~11,300 LOC) and the surface no prior audit could
see. Completion, hover, diagnostics, definition and document links are the editing
experience and stay untouched. What goes:

| # | Item | LOC |
|---|---|---|
| 4.1 | **Rasterized math hover** (headless Chrome screenshot + content-hash disk cache) and its `taliesin/colorScheme` notification and `TALIESIN_MATH_IMAGE_TIMEOUT` | ~450 |
| 4.2 | **`taliesin/insertEdit` Dataset drop kind** (follows 3.2 out) | 264 |
| 4.3 | **The 5 VS Code language-model tools**, folded into the MCP provider 20 lines below them in the same file | ~240 |
| 4.4 | **`selectionRange`** | 156 |
| 4.5 | **Status bar item** | 104 |
| 4.6 | **Sidebar: fold three views into one** | ~200 net |

Launching a browser and maintaining a disk cache to put a picture in a tooltip, for an
expression the live preview already renders continuously, is the clearest single overreach
found anywhere in the audit.

### Wave 5: the CLI, 21 verbs to 13 (~250 LOC)

| Verb | Disposition |
|---|---|
| `render` | Fold into `build --stdout`. It is `build --no-exec` writing to stdout. |
| `blocks` | Cut. A maintainer debug dump, zero documentation, superseded by the LSP. |
| `symbols` | Fold into `map`, taught to accept a single file. The library function keeps its MCP consumer. |
| `skim` | Cut (Wave 3.3). |
| `dev`, `serve` aliases | Cut. `serve` has zero hand-invocations in 6,274 lines, `dev` has two. Three spellings already failed to prevent the author reaching for a fourth (`tali view`); more aliases is not the fix. |
| `new deck --tour` | Cut the flag. |
| `check --stdin`, `pdf --keep-html`, `preview --headless` | Cut / fold. |
| `TALIESIN_OPEN`, `TALIESIN_HOST` | Cut. Duplicate two flags; `TALIESIN_HOST` silently changes network exposure. |

### Wave 6: a fix, not a cut

**`mounts:` is preview-only; the static build needs a shell script.** Fold the script into
`build` so a mount builds. Filed here because the audit found it while measuring, and a
published tool whose config key works in preview but not in `build` is a support burden.

---

## What this changes in numbers

| | before | after |
|---|---|---|
| Rust LOC | ~131,000 | ~124,100 (**~6,900 out, 5.3%**) |
| Binary | 75.6 MB | ~59 MB (**16 MB out**, download 32 MiB to ~23 MiB) |
| Top-level subcommands | 21 | **13** |
| Front-matter + `_site.yml` keys | 73 | **~58** |
| Dev servers | 2 | **1** |

All four formats survive. No load-bearing invariant is touched.

---

## Rejected cuts, and why

Recorded so a later round does not re-derive them. Each was proposed by the skeptic lens and
refuted by evidence.

- **Social-card generator (551 LOC).** Its proposed replacement (fill `og:image` from the
  page's own `image:`) is forbidden by a dedicated regression test written after that exact
  bug shipped (`site/mod.rs`, `page_image_absolute_or_relative_never_leaks_into_og_image`),
  and `card_url` is also the sole source of the JSON-LD image. Cutting it means every shared
  link at launch renders as a bare text card.
- **`publish` (761 LOC + `--init` + passcode gate).** The only feature in the audit with a
  recent, purposeful, successful real invocation. The shared-passcode gate has no `wrangler`
  equivalent, and private-by-default draft sharing is the loop the tool exists to close.
- **`tali.tex` / `tali.table`.** The cut rested on a byte claim wrong by ~4x (about 120 lines
  of `tali-js.js`'s 1,028, not 533) and two agents disagreed by 9x on the same feature.
  Frozen instead.
- **Companion terminal links (211 LOC).** The "already duplicated by problem matchers"
  rationale is factually wrong: matchers attach only to Tasks, and `runcell.ts:60,145`
  deliberately uses a plain terminal, which no matcher can reach.
- **`taliesin/projectRefs`.** Its cut rationale claimed LSP diagnostics already cover it;
  `lsp.rs:2196` publishes per-URI for buffers the editor sent, so a dangling `@sec-` on an
  unopened chapter is not already a squiggle. May still be cut later, but not for that
  reason.
- **Pyodide, cut outright.** Fold (Wave 0.3) is strictly dominant.
- **The deck engine.** Owner ruling: frozen, not cut.

## Corrections to the record

- The 2026-08-01 audit's claim that theorem environments have "2 real uses including a
  genuine blog post" is **false**. `corpus/tech-blog/posts/em-algorithm/` merely contains the
  phrase "Bayes' theorem" in prose. The true figure is one document using one of eight kinds.
- The **101 MB binary** that justified item 205 is rot. Measured 2026-08-02: 72.1 MiB on
  disk; releases ship `tar czf`, so the actual download is **32 MiB**, of which pyodide is
  27.6%.
- `logo:` and `footer:` are not dead front-matter keys; both are supplied site-wide in
  `_site.yml`. Two mechanisms for one job, not an unused feature.

## What this audit did not measure

- **Reader-side value.** Every adoption number here is author adoption. Whether readers use
  the reader menu, link previews, Cmd-K or the lightbox is unmeasured and, under the
  no-telemetry stance, unmeasurable without asking a person.
- **Whether any cut is safe to execute.** Each is a recommendation with its evidence. None
  has been implemented and each needs its retired-register work before it lands.
- **`docs/guide` front matter via the tool's own instrument**, because of the Wave 0.2 panic.
  Covered by an independent scanner instead.
