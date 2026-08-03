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

> **Status: all three DONE, 2026-08-02.** Findings that change what the rest of this document
> says, and which a later wave must not re-derive:
>
> - **0.1.** Diagnosed: the headline was *correct when measured* (2026-06-24) and the payload
>   then regressed 10x **six days later**, deliberately, in `6cdbc218`, which made a
>   multi-`data-sourcepos` block take a full `Update` so fenced divs' inner source positions
>   stay accurate. `RESULTS.md` was never regenerated; its two later commits touched only prose
>   and paths. Proved by reverting that one guard, which reproduces the old file's numbers
>   byte-for-byte (`set_meta 54, update 0, payload 3231`). So it is neither "always wrong" nor a
>   silent regression: it is a stale artifact of a real design change. **The honest figure is
>   9x** (32,303 / 291,691), and one fenced div is 90% of the payload. Two further things the
>   audit did not see: **eight** live citations, not four, including the User Guide
>   (`docs/guide/using/choosing.tmd`) and a server test comment. Also, `RESULTS.md`'s prose claim
>   that the *collapsible callout* survives via `SetMeta` was **false**, since that is precisely
>   the block that now re-renders. Which means: **an opened `:::` collapse callout closes when
>   an edit lands above it**, and any `{js}` cell inside such a div is torn down and re-mounted
>   (`client.js:1712` `replaceWith` after `teardownJs`). That is a real user-visible cost of
>   `6cdbc218`, is now stated wherever the claim is made, and is **not** fixed here: it is a
>   design question, not a Wave 0 correctness item.
> - **0.2.** Fixed as specified, by folding `scan_shortcodes` into `each_shortcode`. Failing
>   tests written first and confirmed failing at `extension/mod.rs:124:27` on `'→'`.
> - **0.3.** Landed, and it bought **2x what this document predicted** (see the row in "What
>   this changes in numbers"). The design's test-gating list was incomplete in two files and
>   wrong in one; corrections are recorded in
>   [2026-08-02-pyodide-cargo-feature-design.md](2026-08-02-pyodide-cargo-feature-design.md)
>   under "What this actually bought".

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

> **Status, 2026-08-02: Wave 1 is COMPLETE — 1.1, 1.2, 1.3, 1.4 and 1.5 all landed.**
> What executing them corrected in this table, which a later wave must not re-derive:
>
> - **1.2 and 1.4 are one deletion counted twice.** Both remove the repo-root `AGENTS.md`;
>   their ~220 + ~200 double-counts it. Landed as one commit. Of the two readings of "one
>   generated artifact", the bundled `assets/agents/AGENTS.md` golden was **kept** (it is the
>   house pattern `vocab.rs`/`schema.rs` use, and it keeps a vocabulary change reviewable as a
>   diff); only the hand-kept duplicate and its wrong-crate gate went. A front-matter key now
>   trips **five** drift gates, a retired one **seven**.
> - **1.5 was blocked by a real, previously-filed obstacle, now discharged.** The deferred
>   note "R1" in `llms.rs` was correct: the shared projection decoded a *smaller* entity set
>   than the prose extractor, so a naive fold would have published `it&#8217;s`.
>   `render::text::decode` now resolves numeric character references. The fold also surfaced
>   two defects in the shared recipe that `read` and the search index had all along — field
>   fusion ("…alignment.17 March 20263 min read") and KaTeX whitespace runs — both fixed at
>   `visible`, not worked around. The published artifact grows deliberately: 45,403 → 141,999
>   bytes on `corpus/tech-blog`, because it now carries the code.
> - **1.1's central premise was wrong.** The table below
>   argues the single-doc server is "the *degraded* one" from `grep -c warm_pool`. That is
>   true of kernel warmth and **false of decks**: `serve/mod.rs` dispatches `DocFormat::Reveal`
>   to `deck_index_html` and carries deck-aware incremental update (`deck_op_is_structural`,
>   `is_slide_structural`, `is_pause_paragraph`, `deck_meta_changed`), while `serve_site`
>   explicitly does not — `serve_site/mod.rs:1779`, "A site page never restructures a deck".
>   A site also *warns and flattens* a loose `format: deck` page rather than serving it. So
>   folding exactly as specified **would have regressed `taliesin preview slides.tmd`**, a
>   supported, documented workflow. Decks being frozen ("supported, zero further investment")
>   is an argument against silently breaking them, not for it. See "How 1.1 was resolved"
>   below for what this actually cost, which was not what a first estimate said.
> - **The coupling in 1.1 is understated.** `serve` is not a module `serve_site` imports
>   fifteen things from; it is the crate's shared HTTP/CLI-error layer. `query.rs`,
>   `publish.rs`, `cli.rs`, `doctor.rs`, `run_cmd.rs`, `mcp.rs`, `log.rs` and `session.rs` all
>   depend on it (`guarded`, `unknown_flag_error`, `bad_format_error`, `RUN_PATH`,
>   `INTERRUPT_PATH`, `session_owns`, `IDENTITY_PATH`, `is_sibling_preview`, `STATUS_CSS`,
>   `MAX_WS_MESSAGE_BYTES`). The cheap shape is therefore **not** "extract 424 lines to a new
>   module" but "delete the single-doc half in place and keep the module as the shared
>   layer", which leaves every `crate::serve::` import path untouched.
>
> **How 1.1 was resolved.** `serve/mod.rs` 2,753 → 1,026 lines; `serve_site` is the only
> server. `preview <file.tmd>` resolves to the file's enclosing `_site.yml` project and opens
> at that page, falling back to a one-document project (`Site::discover_single`) rooted at the
> parent directory — matching the companion (`extension.ts:150-154`, item 150) so the CLI and
> the editor agree. The shared layer stays at `crate::serve::` so no import path churned.
>
> The deck estimate above was **wrong, and the correction is the useful part**: the gap is not
> ~120 lines of structural detection to move. `serve_site` renders a deck *statically per
> request* — a deck owns no live per-page state there at all — so there was nothing for the
> ported predicates to act on. Two consequences, both now handled:
>
> - **Click-to-source was already dead on every deck served by `serve_site`**, embedded ones
>   included, because the deck branch never injected `TALIESIN_DOC`. This was a pre-existing
>   defect in one of the three load-bearing goals, found only by folding. Fixed.
> - **A deck edit produced no feedback at all** (the rebuild scan walks `site.page(rel)`, and
>   a deck is not a page). It now reloads open tabs. That is a real reduction against the old
>   single-doc server's structural op diff, taken deliberately: decks are frozen, so the fix
>   is to make the existing mechanism reach them rather than build a second live path, and
>   every deck inside a project already behaved exactly this way.
>
> Scoping is enforced on **re-discovery** as well as at boot (`Project::scope`); without that
> a save touching `_site.yml` silently widens a one-document preview to its whole parent
> directory. That failure mode is invisible until the first save, which is why it is recorded
> here.

| # | Item | LOC | Note |
|---|---|---|---|
| 1.1 | **Fold `serve/mod.rs` into `serve_site` as a one-page project** | ~2,000 | The largest single win. The site model already handles a bare directory. Verified: `grep -c warm_pool` gives **0** in `serve/mod.rs` and **3** in `serve_site/mod.rs`, so the unused copy is the *degraded* one and a `.tmd` with no ancestor `_site.yml` (the companion's fallback) cold-starts its kernel where a site preview does not. This is an upgrade, not merely a dedup: it also removes the surface's only two-owner protocol contract. Zero of 64 `preview` invocations since the rename targeted a single file. **Execution constraint, measured: `serve/mod.rs` is not purely a duplicate.** `serve_site/mod.rs:31-35` imports fifteen items from it (`CLIENT_JS`, `FAVICON`, `STATUS_CSS`, `bind_with_fallback`, `js_str`, `lan_url`, `local_ip`, `new_session_token`, `open_in_browser`, `percent_decode`, `print_qr`, `with_host_guard`, `with_identity`, `with_lan_guard`, `ws_origin_ok`). Ten of them span ~424 lines in `mod.rs`; the other five live in `serve/security.rs` (416 lines), which stays untouched. **Step one is extracting those ~424 lines into a shared module; only the ~2,329-line single-doc remainder is the deletion.** |
| 1.2 | **Fold the two `AGENTS.md` goldens into one generated artifact** | ~220 | Drops the per-key drift tax from six gates to five and kills a byte-identical duplicate kept in sync by hand, whose gate lives in the wrong crate. |
| 1.3 | **Fold `scan_shortcodes` into `each_shortcode`** | 45 | Already required as Wave 0.2. |
| 1.4 | **Delete `editor/claude-code/skills/taliesin/SKILL.md` and the repo-root `AGENTS.md` copy** | ~200 | The third and fourth hand-written copies of a document the tool generates from its own validator consts. |
| 1.5 | **Fold `llms-full.txt` into the `read` projection** | 170 | One projection, three consumers. |

### Wave 2: the config vocabulary, as one batched change (~350 LOC, ~15 keys)

Ships as a single commit with all `RETIRED_KEYS` entries, per "Batch the retirements".

> **Status, 2026-08-02: Wave 2 is COMPLETE.** 17 keys retired, 2 validators added. What
> executing it corrected in the list below, which a later wave must not re-derive:
>
> - **`_site.yml toc:` was NOT measured ceremony, and the spec's evidence measured one file
>   of five.** The claim "`site/_site.yml` writes `toc: false`, already the default" is true
>   of that file and false of the key: **5 of 17 configs set it, and 3 set `toc: true`**
>   (`tech-blog`, `descent`, `analyst`) against a default of `false`. Cutting it as ceremony
>   would have silently removed the sidebar TOC from three real sites. **Owner ruling: cut it
>   anyway and make the heading-count auto-gate the default**, which is the "derive, don't
>   declare" answer rather than the "nobody uses it" one. Measured after the change, building
>   all five real projects with the old and new binaries: `tech-blog` 8→8, `descent` 1→1,
>   `analyst` 2→2, `graphics3d` 0→0 — **the gate reproduces the opt-in exactly** — and
>   `site/` 0→1 (`showcase.html` gains a rail it declined). One page changed in the entire
>   repo. Before trusting an adoption number here, check every file, not the first one.
> - **`hero.actions[]` cannot be cut; it is used on three real `site/` pages.** Read as the
>   missing-validator item it is (like `chapters:`), not as a removal. A typo'd `hef:` used
>   to render a button that goes nowhere under a green `check`.
> - **`theorems:` config had 4 corpus documents + 1 `_site.yml`, not zero.** The spec's "one
>   real use of one of eight kinds" measures theorem *environments*, not the config. **Owner
>   ruling: cut `numbered:` (and the book-wide `_site.yml theorems:` that carried it), keep
>   `shared:`**, which is what `corpus/course/mle.tmd` — the demand-probe pilot and a gallery
>   exhibit — actually exercises. `corpus/theorem-book/` was **repurposed rather than
>   deleted**: it now pins the property the retirement creates, that a chapter's `shared:`
>   does NOT leak to a sibling chapter.
> - **The `_site.yml` validator did not consult `RETIRED_KEYS` at all.** It had its own
>   `did_you_mean`, so retiring six config keys without wiring it up would have answered
>   `toc:` with "did you mean `logo`?" — a confident instruction to write something
>   unrelated. The register is scoped `(scope, key, note)` precisely for this; `validate_keys`
>   now routes through `unknown_key_message` under a `config key` scope.
> - **A retired key trips EIGHT gates, not seven.** The eighth is
>   `editor/vscode/schema/tali-site.schema.json`, a bundled copy of the crate's schema gated
>   by the companion's own `node --test`. `cargo test --workspace` is green while it is stale;
>   only `./tools/gates.sh` catches it.
> - **The retired-key docs gate had to become scope-aware.** `stale_docs.rs` flattened the
>   register's key column and matched any `key:` line, which fires on legitimate live usage
>   the moment a name is retired in one scope and live in another — `toc:` and `theorems:`
>   (gone from `_site.yml`, live in front matter), `image:` (gone from `hero:`, live at top
>   level), `echo:` (gone from `execute:`, live as `#| echo:`). It now gates only keys **both**
>   validators reject.
> - **Cutting the raw-injection family created a real ordering defect, found only by running
>   it.** With `head:` the sole route, the documented way to load a client enhancer moved from
>   a body slot into `<head>` — which runs *earlier* than the inline enhancer registry, so
>   `window.taliEnhancers.register(...)` threw at parse. The registry is now emitted in
>   `<head>` ahead of `{include_in_header}`; its `if (window.taliEnhancers) return;` guard
>   makes the later bundled copy no-op exactly as before. **Retiring the alternatives to a
>   feature can break the survivor**, and no unit test asked the question.
> - **Two smaller corrections the real binary surfaced.** The `hero.image` a11y lint kept
>   telling authors to add `image-alt:` to a key that no longer exists (two contrary
>   instructions on one line); it is gone. And `key_line` could not locate a key inside a flow
>   mapping (`- { fil: a.tmd }`), which is how chapter entries are usually written, so the new
>   `chapters:` diagnostic arrived without a line number.

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

> **Status: DONE, 2026-08-03**, one commit. Measured: **6,559 lines removed**, 826 added, 90
> files. Final `./tools/gates.sh` = **PASSED, 9/9 gates ran**. Five corrections a later wave
> must not re-derive:
>
> - **3.8 is REFUTED and was NOT cut.** "Zero uses anywhere including fixtures" measured the
>   *retired* spelling `ojs_define`. The live author API is `define(**kwargs)` — the kernel
>   preamble says so at its own definition — and **eight** corpus documents use it, five of
>   them real tech-blog posts (`fourier-transform`, `em-algorithm`, `pca-geometry`,
>   `Kruskal-Wallis-test`, `evidence-lower-bound`). The cut was executed, then reverted. The
>   **5-line fix landed**, with its predicate corrected to `define(`: `runtime_defines` now
>   means "a kernel cell that CALLS `define(`", not "any kernel cell". It does *not* restore
>   the check on the six real blog posts — those genuinely use the bridge, so suppression is
>   correct there — it restores it on the two documents with a kernel cell and no bridge
>   call (`corpus/deck.tmd`, `tech-blog/posts/a-star`), both of which pass. Verified by
>   running the binary, not only by unit test.
> - **`csl:` was kept** (owner call, 2026-08-03). It is the sole entry of `UNSUPPORTED_KEYS`,
>   not part of the cite-this cluster, and the row's evidence sentence is false for it: it
>   appears in **zero** documents by design. Cutting it would downgrade a citation-specific
>   message to a did-you-mean for exactly the migrating-Quarto stranger this scope targets.
>   The other five keys went.
> - **There is no MCP `skim` tool** (3.3). `mcp.rs` exposes six tools and `skim` is not among
>   them; `skim` reached agents only through `map`'s `words`/`headings`, which is 3.6. So 3.3
>   and 3.6 are one change and were executed together.
> - **Three cuts would have taken a surviving feature's only pin with them.** `corpus/cite-this/`
>   also pinned structured `author:` (items 184/187) → repurposed as `corpus/structured-authors/`;
>   `corpus/reactive/animate.tmd` also pinned `tali.state` (156) → repurposed as
>   `corpus/reactive/state.tmd`, slider-driven; the `fig-export` determinism test also pinned
>   **cwd isolation under a concurrent build** → rewritten on a plain `savefig`. A fourth,
>   the JSON-LD author-fallback chain, lost its only end-to-end pin with `scholar_meta.rs` and
>   is now a unit test in `meta.rs`. This is the Wave 2 lesson recurring: when a fixture is
>   named for one feature, check what *else* it is the only pin for.
> - **Two Wave 2 leftovers were found and fixed in passing**, both in the User Guide, neither
>   gated: `reference/frontmatter.tmd` still documented the retired `execute: echo:`/`include:`
>   as live sub-keys (with an example a reader copying would get warned for), and the `csl:`
>   row still justified itself by a collision with `css:`, retired on 2026-08-02.
>
> Wave 4.2 (the `taliesin/insertEdit` Dataset drop kind) was **pulled forward** into this
> wave: it is one logical change with 3.2, and shipping 3.2 alone would leave a VS Code drop
> gesture inserting a shortcode that no longer exists.

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

> **Status: DONE, 2026-08-03**, one commit. Measured: **1,459 lines removed**, 258 added, 26
> files. Final `./tools/gates.sh` = **PASSED, 9/9 gates ran**; the ungated Extension Host e2e
> was also run by hand: **41 passing, 0 failing**. Four things a later wave must not
> re-derive:
>
> - **The math hover did not go blank, because the fallback was already the shipped path.**
>   `math_image::data_uri` returned `None` on any host without Chrome, and the caller then
>   used `math_preview::unicode_preview`. Cutting the rasterizer just makes that the only
>   path. Verified by driving the real binary: hovering `$E = mc^2$` answers `### E=mc²`.
> - **`selection_range` is TWO different things and only one was cut.** `DocumentSymbol` has
>   a `selection_range` field (the heading line inside a section's full range) that
>   `documentSymbol`, `workspace/symbol` and the CRLF tests all depend on. What went is the
>   `textDocument/selectionRange` *request*: `resolve_selection_ranges`, `lsp_nav::
>   selection_chain` and its two exclusive helpers (`word_span`, `paragraph_span`), the
>   capability, and the wire test. A blind grep for the name would have broken the outline.
> - **Deleting `math_image.rs` broke a gate, not a test.** `tools/gates.sh` named
>   `a_real_browser_rasterizes_real_katex_into_a_data_uri` as one of ELEVEN canaries, and
>   `gate_script.rs` asserts both that each canary still exists and that there are exactly
>   eleven. Removing a browser-backed capability means removing its canary *and* decrementing
>   that count. The other five browser canaries were renumbered in the comments.
> - **The e2e suite was already red before this wave**, which is what "run by nothing
>   automatically" buys: Wave 3 routed a dropped `.csv` from the dataset kind to the asset
>   one, the asset kind answers with a `SnippetString`, and the test read it with `String(…)`
>   — so it asserted against the literal `"[object Object]"`. Fixed here by reading `.value`.
>   The product was correct throughout; only the test was wrong.
>
> **4.2 was not done here**: it shipped inside Wave 3, which pulled it forward to keep it one
> logical change with 3.2. 4.6 is a fold, not a cut — every row and every tree builder
> survives, one level deeper, and each group states its size so the two that start collapsed
> still answer at a glance.

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

### Wave 5: the CLI, 21 verbs to 17 (~250 LOC)

> **Status: DONE, 2026-08-03**, one commit. Measured: **1,222 lines removed**, 729 added, 31
> files. Four corrections a later wave must not re-derive:
>
> - **The header's "to 13" was arithmetic, not a plan, and it is corrected to 17.** Every
>   disposition in the table below was executed in full; they *sum* to four cuts from the
>   verb list (`render`, `blocks`, `symbols`, plus `skim` already gone in Wave 3.3) and two
>   alias removals, which is 21 → 17, not 13. Nothing was skipped to land on 17 and nothing
>   further is proposed to reach 13: the eight verbs that would have to go to get there were
>   never named, and every survivor is `keep` or better in the catalogue. **The "Top-level
>   subcommands 21 → 13" row in "What this changes in numbers" is likewise wrong and is
>   corrected to 21 → 17.** Counted at `71dbb6e1` as `COMMANDS` minus `help` and the two
>   aliases: 20 before this wave, 17 after.
> - **The cuts needed a `RETIRED_COMMANDS` register, on measured evidence.** Four of the five
>   retired names sit further than edit distance 2 from every survivor and so got silence —
>   but **`dev` is exactly two edits from `new`**, so a bare deletion answered "preview this
>   project" with a suggestion to run the command that *scaffolds files into it*. The register
>   is consulted before the did-you-mean and carries `skim` too (Wave 3 left it silent).
> - **`preview --headless` folded to a hidden `--__session` flag, not to nothing.** `taliesin
>   run` spawns the session itself and needs a way to say so; underscore-prefixing follows the
>   `__complete` precedent, keeps it out of `SERVE_FLAGS`, the completion table and every help
>   page, and leaves the flag-documentation gate honest.
> - **`taliesin run` cannot start a session, and was already broken at `71dbb6e1`.** Verified
>   by stashing this wave, rebuilding, and reproducing with `--headless`: the session process
>   comes up and serves, but `attach_or_start` never detects it and gives up after 45 s. The
>   likely cause is a Wave 1 leftover — `run` keys the port hint on the *file* for a document
>   with no `_site.yml` (`run_cmd.rs:150`), while the now-single server writes the hint under
>   the project root it discovered (`serve_site/mod.rs:657`), i.e. that file's *directory*.
>   Not fixed here (out of this wave's scope); it is what "`run`: pin 0 integration tests" in
>   the catalogue buys.
>
>   **Fixed 2026-08-03, alongside Wave 6.** The diagnosis above was right and was confirmed
>   by measurement rather than inspection: the spawned session's `/__taliesin` answered with
>   the *directory*, and the hint file `run` polled for (the digest of the document path) was
>   never written by anyone. The fix moved the **server**, not the client — a project of just
>   one document now publishes that document as its identity, hint key and single-instance
>   key, which is exactly what the deleted single-document server did (`serve/mod.rs`'s
>   `app.path`) and is what makes `run_cmd`'s existing rule correct as written. Keying the
>   client on the directory instead would have made a second loose `.tmd` in the same folder
>   unrunnable: it would attach to a session scoped to the first document and get "not a page
>   of this session's project". The two derivations are now named functions
>   (`Resolved::session_key` and `session::session_key_for`) pinned against each other, plus
>   `run`'s first two integration tests. Measured after the fix: cold run 1.8 s, warm run
>   14 ms ("0 ran, 1 cached"), and a second loose document in the same directory runs.

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

> **Status: DONE, 2026-08-03.** `build_site_async` recurses into each `mounts:` entry after
> its own build, writing it to `<out>/<at>/`. Measured on this repo: `taliesin build site
> --out …` produces all eight projects in 15 s, and every mount link in `index.html` and
> `gallery.html` has a file behind it. Four things a later round should not re-derive:
>
> - **The fold is a net deletion of three artifacts, not just the script.** `site/build.sh`
>   (64 lines), the `TAL-MOUNT-PREVIEW` diagnostic (a needle row, a catalogue entry, its
>   `docs/DIAGNOSTICS.md` section), and `build::mount_warnings` all existed to describe a
>   hole the recursion fills. `check.rs` consumed the same warning builder, so `check` was
>   reporting it too; both channels are gone. A diagnostic that survives its defect sends
>   the author to write the script the build already replaced.
> - **The parent must be built before the mounts, and the sweep must NOT be taught to skip
>   them.** `sweep_stale` deletes everything under the output it did not write, and a mount
>   directory is not dot-, underscore- or symlink-exempt, so mounts-first silently deletes
>   them on a green exit. Exempting the prefixes would fix the delete-and-rewrite cost but
>   would also strand a mount *removed* from `_site.yml` in the deploy forever; the sweep is
>   what makes removal work, so the cost stays.
> - **`mounts:` is a graph, so the walk needs a cycle guard.** `mounts: { self: . }` is one
>   plausible config line from an unbounded recursion. A visited set seeded with the root
>   refuses it on sight.
> - **`--strict` now propagates into mounts.** The shell script deliberately ran them
>   non-strict, but that carve-out existed to dodge the parent's own mount warnings, which
>   this deletes. `build --strict` on a site with mounts no longer fails merely for having
>   them, which is what `strict_no_longer_fails_a_site_just_for_having_a_mount` pins.
>
> `site_build_script.rs` was **deleted** (it pinned a duplicated list that no longer exists)
> and `mount_preview_is_gated.rs` **re-aimed** into `mount_static_build.rs`, which also
> inherits the one claim the deleted file uniquely held: the parent-before-mounts order,
> now pinned by building twice into one directory rather than by grepping a script.

---

## What this changes in numbers

| | before | after |
|---|---|---|
| Rust LOC | ~131,000 | ~124,100 (**~6,900 out, 5.3%**) |
| Binary | 72.0 MiB | **40.7 MiB** (**31.3 MiB out, 43.5%**). *Corrected 2026-08-02 on execution: Wave 0.3 measured 2x the predicted saving, because the payload was embedded twice. The "~59 MB / 16 MB out" this row first carried was half the truth. Download unchanged by policy: the tarball stays the complete tool.* |
| Top-level subcommands | 21 | **17**. *Corrected 2026-08-03 on execution: the "13" this row first carried did not follow from Wave 5's own disposition table, which removes four verbs and two aliases. See the Wave 5 status note.* |
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
