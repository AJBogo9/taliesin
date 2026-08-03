# Scope inventory — the pre-flip review sheet

**Built 2026-08-03**, against `main` = `9869fe32` plus the four fixes made in the same
session (below). Every number here was measured by running the tool or the tree, not
carried over from an earlier audit. Where it contradicts
[the final-scope audit](2026-08-02-final-scope-audit.md), the contradiction is called out.

**Nothing here is cut. This is a sheet to rule on.**

## Scale, for calibration

| | |
|---|---|
| Rust | 124,704 lines, of which **37,580 (30%) sit in `crates/*/tests/` integration suites** — inline `#[cfg(test)]` modules add more on top |
| Bundled JS + CSS | 17,163 lines |
| Binary, default features | **38.4 MB** (`headless-js` and `pyodide` both off) |
| Corpus documents | 133 |
| Manual pages (`docs/`) | 40, in two dogfooded books |
| Built public surface | **81 HTML files** across 8 projects (site + 2 books + 5 gallery exhibits): 69 content pages, 8 × `404.html`, 4 decks |

---

## 1. The 18 CLI commands

**17 of the 18 have a dedicated integration test that spawns them and asserts the result**;
`vocab` is the exception (its data is unit-tested, its CLI output is not — see below). No
command has zero documentation except `help`, which is its own documentation.

Legend: **tests** = dedicated integration test file; **manual** = mentions in
`docs/guide/reference/cli.tmd` unless noted.

### Author (2)

| command | purpose | tests | manual |
|---|---|---|---|
| `init` | Scaffold a starter site (`_site.yml` + `index.tmd`) you can preview immediately | `init_cli.rs`, `wizard_gate.rs` | 3 |
| `new` | Scaffold one document (`post`/`page`/`deck`/`paper`), correct on first save | `new_cli.rs`, `wizard_gate.rs` | 7 |

### Preview & build (5)

| command | purpose | tests | manual |
|---|---|---|---|
| `preview` | The live dev server: nav, hot reload, click-to-source. **The product.** | `preview_single_instance.rs`, `mount_serving_live.rs` | 5 |
| `build` | Render to self-contained HTML — a file, a portable folder, or a whole site | `build_stdout_cli.rs`, `build_jobs.rs`, `build_reproducibility.rs`, +30 more | 19 |
| `run` | Execute code cells in the terminal against the warm session, no browser | `run_session_discovery.rs` | 7 |
| `pdf` | Typeset paginated PDF rendered *from* the built HTML (needs Chrome) | `print_pdf.rs` | 4 |
| `publish` | Build + deploy to Cloudflare Pages behind a passcode | `publish.rs` | 9 |

### Inspect (5)

| command | purpose | tests | manual |
|---|---|---|---|
| `check` | List located diagnostics; non-zero exit if any | `check_cli.rs`, `hostile_input.rs`, +10 | 29 |
| `doctor` | Audit the environment for running code cells (interpreters, kernels) | `doctor_cli.rs` | 6 |
| `map` | Whole-project outline: pages, nav, xref graph | `map_cli.rs` | 9 |
| `read` | Project the document to plain text (agent-readable); `--run` executes | `read_cli.rs`, `read_book.rs`, `read_run.rs`, `read_run_js.rs` | 7 |
| `features` | What a document uses, and what no document uses | `features_cli.rs` | 5 |

### Editor & agent (5)

| command | purpose | tests | manual |
|---|---|---|---|
| `lsp` | The offline LSP server — **all** editor intelligence lives here | `lsp_stdio.rs` | 3 |
| `mcp` | stdio MCP server (check/read/map/vocab/build tools) | `mcp_stdio.rs` | 3 |
| `schema` | Emit JSON Schemas for `_site.yml` + front matter | `schema_cli.rs` | 0 in cli.tmd; **5 elsewhere in docs** |
| `vocab` | Emit editor autocomplete vocabulary as JSON | `help_cli.rs`, `mcp_stdio.rs` | 4 |
| `completions` | Print (or `--install`) a shell completion script | `complete_cli.rs` | 1 |

### Meta (1)

| command | purpose | tests | manual |
|---|---|---|---|
| `help` | Usage / `--version` | `help_cli.rs` | n/a (is the docs) |

**Observations for your ruling, not cuts:**

- `vocab` is the thinnest command on the board, precisely: its **data** is well gated
  (10 unit tests in `crates/core/src/vocab.rs`, including the `descriptions_present`
  drift gate), but **no integration test spawns `taliesin vocab` and asserts its JSON** —
  unlike `schema`, which has `schema_cli.rs`. `help_cli.rs` only checks it has a help page;
  `mcp_stdio.rs` exercises the MCP *tool* named `vocab`, not the subcommand. It is also
  the one vocabulary `CLAUDE.md` warns is the *offered-completions subset*, not the
  implemented set.
- `schema` and `vocab` overlap in job (both feed editor autocomplete) and both exist
  because the VS Code companion predates `lsp`. Now that **all** editor intelligence is
  supposed to live in `taliesin lsp`, whether two more emit-a-blob commands earn their
  place is a real question — but note `editor/vscode/schema/tali-site.schema.json` is a
  bundled *copy* of `schema`'s output, so they are not dead.
- `mcp` and `read`/`map` overlap similarly: `mcp` wraps commands that already exist as
  verbs. That is a deliberate protocol adapter, not duplication, but it is surface.

---

## 2. What `taliesin features` reports

Measured with the tool itself (`taliesin features . --format json`) across **all 190 `.tmd`
documents in the repository**: `corpus/` 133, `docs/` 40, `site/` 7, `editor/` 5, `tools/` 3,
`samples/` 2.

> **I fixed two false negatives in this instrument before trusting it.** The report is the
> input to a cut decision, and it was under-counting. See §4.

| group | known | used | unused |
|---|---|---|---|
| front-matter keys | 23 | 21 | 2 |
| front-matter sub-keys | 14 | 11 | 3 |
| div classes | 16 | 14 | 2 |
| div attributes | 8 | 8 | 0 |
| callout kinds | 5 | 5 | 0 |
| theorem kinds | 8 | 8 | 0 |
| cell languages | 10 | 8 | 2 |
| cell options | 13 | **13** | **0** |
| shortcodes | 4 | 4 | 0 |
| shortcode arguments | 7 | **6** | **1** |
| input types | 6 | 5 | 1 |
| cross-reference kinds | 12 | 9 | 3 |
| **total** | **126** | **112** | **14** |

### The 14 constructs no document uses — and why 5 of them are not dead surface

**Read this table before treating "unused" as "cuttable".** Five entries are unused *by
design*, and cutting them would remove working behavior.

| construct | verdict | reading |
|---|---|---|
| `.aside`, `.marginnote` | **deliberate alias** | `docs/guide/using/writing.tmd:248`: "`.column-margin` is the canonical name; `.sidenote`, `.marginnote`, and `.aside` are accepted aliases for the identical block (Tufte/Distill/Quarto muscle memory)". Unused is the *expected* state for an alias. |
| `range` (input type) | **deliberate alias** | `extension/mod.rs:656` — `"slider" \| "range" =>` is one match arm. Documented as aliases in `shortcodes.tmd:231`. |
| `logo` (front matter) | **live elsewhere** | Supplied site-wide in two `_site.yml` files. Two mechanisms for one job — a real redundancy, but not a dead feature. |
| `video.poster` | **correctly unused** | Its only appearance is inside a ```markdown example fence, which the scanner rightly excludes. |
| `csl` (front matter) | genuinely idle | Citation-style override. Real feature, zero demand, cheap to keep. |
| `hero.actions.href` / `.primary` / `.text` | genuinely idle | The hero call-to-action triple. The marketing site uses `hero:` but never `actions:` — **and `hero:` is the marketing site's own feature**, so this is the author's own unused knob. |
| `{julia}`, `{sql}` | genuinely idle | Cell languages with no kernel wired anywhere in the tree. Pure catalogue surface. |
| `exm`, `prp`, `rem` | genuinely idle | 3 of the 8 theorem kinds are never cross-referenced by any document. |

So the honest count is **9 idle constructs, not 14** — and of those 9, the three theorem
xref kinds and the two cell languages cost catalogue entries rather than machinery.

**The one real redundancy worth a ruling:** margin content has **four** accepted spellings
(`.column-margin` canonical, plus `.sidenote` / `.marginnote` / `.aside`), and only two are
ever written — `.column-margin` in the corpus, `.sidenote` once in `samples/paper.tmd`. The
aliases were a deliberate welcome mat for people arriving from Tufte/Distill/Quarto. Whether
a tool that has otherwise **shed** its Quarto vocabulary should still carry three of them is
a positioning question, not a code-size one. Your call; I have not touched it.

### One feature is exercised only outside the corpus

`.sidenote` is used by exactly one document in the tree, `samples/paper.tmd` — **not by any
`corpus/` document**. It has unit coverage (`render/tests.rs`, `corpus.rs` names it), so it
is not untested, but `samples/` is not the regression net. Under the corpus-plus-roadmap
rule ("every capability ships pinned by a target corpus document"), that is the one visible
gap in the pinning discipline.

### The authorship split — the number that actually answers "is the MVP the right size"

Adoption counted per authorship bucket, not per document:

| bucket | documents | features used |
|---|---|---|
| The **manual** (`docs/`) | 40 | 48 |
| The **marketing site** (`site/`) | 5 | 29 |
| **Union of all real writing** | 45 | **66 of 126** |
| The **corpus** (pin fixtures) | 133 | 108 of 126 |
| Pin-only (corpus exercises it, no real writing does) | — | **45** |
| Used by nothing anywhere | — | 14 |

(The marketing site counts 5, not the 7 `.tmd` on disk: walked as a *project*, `features`
visits pages, so the leading-underscore `_includes/` is not one.)

**The shape this reveals:** writing 45 real pages needed **66 constructs**. The other 45
exist because a corpus document was written *to pin them* — which is exactly what
corpus-plus-roadmap prescribes, so this is the discipline working, not rot. But it does mean
**a stranger's first month would plausibly touch about half the vocabulary**, and the tail
is carried by fixtures rather than by demand.

That is the honest frame for the MVP-size question: the tool is not carrying 126 features
because 126 were wanted. It is carrying 66 that got used and 45 that were built with their
pin, in lockstep, on purpose.

### The instrument's blind spot

**`taliesin features` does not scan `_site.yml` at all.** Its 12 groups are front matter,
divs, callouts, theorems, cells, shortcodes, inputs and xrefs — the 21 validated `_site.yml`
config keys are outside it entirely. So "what nothing uses" is currently answered for the
*document* vocabulary only. If you want the config surface in the same table, that is a
feature request against `features.rs`, not a scope ruling.

---

## 3. Vestigial / half-finished / flagged

Ordered by how strongly I would ask you to look, not by size.

### (a) `site/_site.yml` declares `url: https://taliesin.dev`, which does not resolve

```
$ curl -L https://taliesin.dev
curl: (6) Could not resolve host: taliesin.dev
```

That URL is the canonical origin: it feeds Atom feeds, `llms.txt`, SEO canonical tags and
the social-card metadata. The build already warns about it five times (once per marketing
page). **Publishing the repo does not break anything, but the built site advertises a
domain that does not exist.** Owner call: register it, or change `url:` to wherever it will
actually live. Not something I should pick.

### (b) Seven pages are not self-contained — **including the marketing homepage**

The build says so itself, 17 times:

```
warn  index.tmd:1: external reference not bundled: https://esm.sh/three@0.163.0
      — the build will fetch it at view time, so the output is not self-contained
```

Every page carrying a Three.js scene loads it from **esm.sh at view time**:

| page | what it is |
|---|---|
| `index.html` | **the marketing site's front page** (the 3D hero blob) |
| `showcase.html` | "See it live" |
| `gallery/graphics3d/{cad,lorenz,molecules}.html` | 3 gallery exhibits |
| `docs/guide/using/code.html`, `.../interactive.html` | 2 User Guide pages |

Confirmed live in the browser: loading `showcase.html` issues real requests to
`esm.sh` (HTTP 200, two of them). So **the first page a stranger opens makes a
third-party request** — which is a privacy, availability and offline-guarantee question
at once, and it sits against the repository's stated offline guarantee. It is a defensible
trade (vendoring Three.js is heavy), but it is currently *undeclared* anywhere a reader
sees: only the build log mentions it, and the build still exits 0.

### (c) The deck engine — measured, because the prior audit's number was wrong

The 2026-08-02 skeptic lens said decks have "**zero author documents and zero invocations
ever**". Measured today:

```
$ grep -rln "^format: deck" --include='*.tmd' .
corpus/course/lecture.tmd      docs/guide/demo.tmd
corpus/deck-marginalia.tmd     docs/guide/tour.tmd
corpus/deck.tmd                docs/guide/using/recipes.tmd
corpus/embed/talk.tmd          samples/deck.tmd
corpus/scaffold/my-talk.tmd    site/demo.tmd
```

**10 documents**, of which 4 are in the manual and the marketing site — including
`site/demo.tmd`, which falls in the audit's own "author writing" bucket. The 60-second tour
deck is the *first thing* the User Guide shows a new reader, and it renders correctly at all
three viewports. Cost is 4,621 lines (772 Rust + 2,720 JS + 1,129 CSS).

You already ruled **frozen, not cut**. Nothing here reopens that; it corrects the evidence
under it, so the ruling does not rest on a false premise.

### (d) `notes/backlog.md` listed a shipped item as open — **fixed**

P1 named **205** (get pyodide out of the binary) as unranked-open and "the obvious next
pick". It shipped: `crates/server/Cargo.toml:29` makes `pyodide` a non-default feature, and
`tools/gates.sh` has two canaries specifically because of it. Both stale references removed.

### (e) A local tag that is not a release

`git tag` shows one tag, `interpreter-resolution-fix`, pointing at `c3624909`. It is
**local only** (`git ls-remote --tags origin` is empty), so it will not appear publicly —
but it is also not a `v*` release tag, and there are zero releases. Housekeeping, not a
blocker. I did not delete it; deleting tags is yours.

### (f) Things I looked at and decided were NOT defects

Recording these so they are not re-flagged:

- The floating **"Contents" pill** on mobile overlays body prose while scrolling. At the
  true document end it covers nothing (7 px reserved). Correct floating-action-button
  behavior.
- The lightbox's `<img alt="">` with **no `src` attribute** — a placeholder populated on
  open. Issues no request.
- `video.poster` reported unused — correct; its only appearance is inside an example fence,
  which the scanner deliberately excludes.

---

## 4. Fixes made while building this sheet

All four are pinned by tests written before the fix.

1. **`taliesin features` under-reported adoption two ways** (`crates/core/src/features.rs`).
   Bare-flag shortcode arguments (`{{< video tour.mp4 controls >}}`) were dropped because
   the scan read only `key=value`, so `video.controls` and `video.audio` could **never** be
   reported used by any document — a vacuous "unused" in the exact column a cut decision
   reads. And a cell option written as a **fence attribute**
   (`{.python code-line-numbers="1|2-3"}`) was invisible, though the renderer reads both
   spellings — and the fence attribute is the *only* spelling `corpus/deck.tmd` and
   `docs/guide/demo.tmd` use. The unused count was **17; it is really 14.**

2. **Every marketing-site page 404'd on `icon-192.png`** (`crates/core/src/site/manifest.rs`).
   `manifest_head_at` emitted `<link rel="apple-touch-icon">` unconditionally, but the PNG
   is only written when `ships_bundled()` — which is false exactly when a project declares
   `favicon:`. `site/` and `corpus/tech-blog/` are the two such projects. Now the link is
   emitted only when the asset is really shipped. Verified: **81 pages, 0 broken refs.**

3. **`taliesin preview-site .` now suggests `preview`** (`crates/server/src/main.rs`).
   Added an extends/abbreviates rule consulted *after* edit distance declines, so no
   existing suggestion changes and retired verbs keep their notes.

4. **`notes/backlog.md`** — item 205, above.
