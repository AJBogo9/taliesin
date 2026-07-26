# Lenses L2, L3, L4, L5 — 2026-07-26

Four lenses off the menu in [backlog.md](backlog.md), run in one session after **L1 (path parity)**
closed. Release binary at `e534c73`. Browser work via the project's own `puppeteer-core` harness
(`tools/ui-audit/lib/browser.mjs`); the chrome-devtools MCP profile was held by a parallel session
throughout, the documented fallback. No repo file was modified by the probes.

**One-line verdicts.** L2: reader-side performance is **healthy on every ordinary page** and the single
outlier is the standalone-deck artifact, which opts out of the site build's shared assets. L3
(partial): the newest process-spawning subsystem is well-built, with one unbounded wait. L4: a project
written against last month's Taliesin **silently loses its whole configuration**, and removed
vocabulary is indistinguishable from a typo. L5: the two dogfooded books ship almost no metadata,
which is what leaves the derived reader surfaces empty.

---

## L2 — reader-side runtime performance

The lens AP1 and AP6 both left open: AP1 measured the **server** (8,000 blocks in 647 ms, 400 pages in
874 ms), AP6 measured browser **parity**, not speed. The only successful Lighthouse run is 2026-07-11,
desktop mode, and it predates the switch from per-page inlining to hashed `_assets/`.

**Ordinary pages are fast, including on a throttled phone.** 4× CPU throttle, 390×844:

| page | profile | LCP | long tasks | blocking | DOM ready |
|---|---|---|---|---|---|
| blog index | laptop | 128 ms | 1 | 0 ms | 107 ms |
| blog index | phone 4× | 148 ms | 1 | 21 ms | 109 ms |
| blog post (math) | phone 4× | 236 ms | 5 | 232 ms | 626 ms |
| book chapter | phone 4× | 132 ms | 1 | 21 ms | 109 ms |
| deck (4.6 MB) | phone 4× | 404 ms | 6 | 662 ms | 1,145 ms |
| mermaid + `{js}` probe | phone 4× | 1,156 ms | 4 | 667 ms | 1,130 ms |

Every LCP is inside the 2,500 ms "good" band with room to spare. The two heavy pages exceed the 200 ms
total-blocking-time "good" bar (662 / 667 ms), and both are mermaid-carrying.

### L2-1 (MEDIUM): a deck in a site build ignores `_assets/` and re-inlines the whole framework

Measured on a two-page site whose only deck draws one mermaid diagram:

| artifact | raw | gzipped |
|---|---|---|
| `talk.html` (the deck, inside the site build) | 4,583,261 | 1,375,317 |
| the same deck with its mermaid block removed | 1,011,028 | 396,685 |
| `index.html` (an ordinary page in the same build) | 24,718 | 9,469 |

The site build emits a shared, content-hashed `_assets/` (including `mermaid.*.js`, 3.5 MB), and the
ordinary page links it. **The deck page links `_assets/` zero times** and inlines its own copy of
everything, so in this one output tree the mermaid library ships **twice**, and a second deck would
ship a third copy. The fixed per-deck cost of framework the site already has externally is **~1 MB raw
/ ~390 KB gzipped**.

Loaded over Slow 3G with a 4× CPU throttle (uncompressed, as served):

```
blog index (365 KB)   unthrottled   204 ms      Slow 3G   10,735 ms
deck       (4.6 MB)   unthrottled 1,165 ms      Slow 3G   94,018 ms
```

**Name the trade-off, do not just "fix" it:** a deck built standalone *should* be self-contained (that
is the artifact you hand someone, and `site/mod.rs` deliberately treats an embedded deck as a
standalone document, not a page). The finding is narrower: inside a `build <dir>` the shared assets
already exist beside it, so the same decision costs the reader a megabyte per deck for nothing. A deck
page in a site build could take `AssetMode::External` like every other page in that build, leaving the
standalone path untouched.

### Measured healthy — do not re-scope

Ordinary page weight is small (9 KB gzipped for a blog index, 9 KB for a book chapter, on top of
shared cached assets); zero console errors on every page and profile except the fixture's own missing
image; the math-heavy post costs 232 ms of blocking on a 4× phone, which is fine.

### The instrument trap this lens paid for

**Raw CDP `Network.emulateNetworkConditions` silently does nothing**, with or without a preceding
`Network.enable`: a first pass reported the 4.6 MB deck loading in 1.2 s on "Fast 3G" and I nearly
filed "network weight is a non-issue". Puppeteer's own `page.emulateNetworkConditions(...)` works and
turned the same load into 94 s. **A throttled number that is not slower than the unthrottled one is a
broken instrument, not a fast page.** CPU throttling via `Emulation.setCPUThrottlingRate` did work.

---

## L3 (PARTIAL) — the subsystems that post-date every lens that would own them

Only the sharpest item was run: **`headless_js.rs`** (615 lines, landed 2026-07-22, five days *after*
the security round), the one subsystem that launches an external browser. It is well-built: a local
Chrome found on `$PATH` with the browser-download `fetcher` feature off, a unique throwaway profile
dir, `file://` loading with assets already inlined so no network is reached, `--disable-extensions`,
teardown regardless of outcome, `handler_task.abort()`, and the profile dir removed.

### L3-1 (LOW/MEDIUM): the headless observation is not bounded end to end

`tokio::time::timeout` bounds the **eval** (`headless_js.rs:312`), but `Browser::launch`, navigation,
`browser.close()` and `browser.wait()` are unbounded, and the only call site runs the whole thing on a
bare `rt.block_on(...)` with no outer timeout (`query.rs:371`). The module's own contract says "Never
errors to the caller: any launch/navigation/eval failure degrades the whole set to `Skipped(reason)`"
— a **hang** is exactly the failure that contract does not cover, so a wedged Chrome hangs
`taliesin read --run-js` with no diagnostic. The project already has the pattern for this class
(`TALIESIN_CELL_TIMEOUT` bounds a runaway kernel cell). Fix: wrap `observe_inner` in a timeout of
`settle_timeout()` plus a launch margin and degrade to `Skipped("browser timed out")`.

### L3-2 (LOW): `.no_sandbox()` is unconditional with no recorded justification

`headless_js.rs:260`. Defensible (the author's own content, `file://`, no network, and the sandbox
routinely fails in containers) and probably correct, but every comparable decision in this tree
carries its reasoning next to it. One comment, not a code change.

**Not run:** `lsp.rs` (1,922 lines), `complete.rs` (1,157), `skim.rs` (647), `manifest.rs` (303). L3
stays open.

---

## L4 — deprecation / migration UX

"What does the tool say to a project written against last month's build?" Probed with a project
carrying the pre-rename config filename, the removed `about:` key, and a `format:` sub-key.

### L4-1 (MEDIUM): a pre-rename `_quarto.yml` is invisible, and `check` says "no problems found"

`_quarto.yml` was renamed to `_site.yml` on 2026-06-24 (the drop-Quarto execution). A project still
carrying the old filename gets **no diagnostic at all**: `check .` exits clean, and the site builds
with the config silently defaulted (the `title: A legacy project` in that file is dropped; the built
page keeps only its own front-matter title). Everything downstream is healthy — rename the file and
the config linter immediately reports `unknown config key 'project'` as an **error** — so the entire
gap is that nothing looks for the old filename. Fix: an existence check beside the config load, "found
`_quarto.yml`; the project config is now `_site.yml`".

### L4-2 (LOW/MEDIUM): removed vocabulary is indistinguishable from a typo

`about:` (removed 2026-07-17, superseded by `hero:`) and `number-within:` (removed when theorem
numbering scoped to chapters) both report:

```
warning[TAL-FM-KEY]: unknown front-matter key `about`
```

which is the same message a misspelling gets, with no pointer to the replacement. Grepping the tree
for a retired-key registry (`RETIRED`, `REMOVED_KEYS`) returns **nothing**: the removals live only in
source comments and tests. The omission is conspicuous precisely because this tool's did-you-mean
culture is otherwise thorough (it will suggest `titel` → `title`, and `check --explain` exists for
every code). Fix: a `RETIRED_KEYS: &[(&str, Option<&str>)]` consulted before the unknown-key warning,
emitting "`about` was removed; use `hero:`".

### Observation, not a finding

Directory discovery is `.tmd`-only (`preview <dir>` on a directory of `.qmd` files says "no .tmd pages
found"), but naming a file explicitly builds **any** extension: `build stray.qmd` succeeds. That is
not a silent degradation (you get exactly the file you named), so it is recorded rather than filed, so
that nobody "fixes" it into a regression for `build README.md`-style uses.

---

## L5 — the content layer of the dogfooded books

The skimmability round's third-order finding, measured directly (source `.tmd`, `wc -w`, 2026-07-26):

| book | pages | `description:` | xrefs | `{.definition}` | words |
|---|---|---|---|---|---|
| `docs/guide` | 22 | **3** | 15 | 1 | 33,718 |
| `docs/internals` | 15 | **0** | 16 | 0 | 31,256 |
| `corpus/tech-blog` (contrast) | 19 | **12** | — | — | — |

### L5-1: the tool's own manual ships without the metadata the tool derives its reader surfaces from

**3 of 37 dogfood pages set `description:`, against 12 of 19 in the blog corpus.** The books are what a
prospective user reads, and they are the pages with no meta description, no og:description and the
weakest search-result text; the corpus, which nobody reads, is the well-tagged one. Likewise 31 xrefs
across 37 chapters and **one** `{.definition}` block in 65,000 words, which is why a glossary or
term-index surface renders empty on the only books in the repo.

This is an **authoring pass, not code**, and it is the cheapest lever on how the tool presents itself.
Note for whoever runs it: the skimmability round recorded "0 of 37 pages set `description:`" and the
measured figure today is 3 of 37, so re-measure rather than trusting either number.

---

## Not measured

**L6** (import a real external document, the FL-weather Quarto book) needs a repository that is not on
this machine. The **mutation-testing re-run** scoped to files changed since 2026-07-18 is a long
compute job, not a read, and was not started. The **deck-audit re-run crossed with touch** and the
**website-audit re-run** are partly subsumed by the mobile round and by L2 above, but neither was run
as its own pass. Within L2: real devices, real networks (only emulated), and the preview path's
runtime cost. Within L4: the `_freeze/` cache format across versions, and `.taliesin/` schema drift.
