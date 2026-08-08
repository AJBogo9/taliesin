# Scope ruling: cutting Taliesin back to a writer's tool

**Built 2026-08-08** against `main` = `f6dee87d`, working tree clean, baseline
`cargo test --workspace` = 123 suites, 2,318 passed, 0 failed, 0 ignored, exit 0.

This is the ruling that [the 2026-08-03 scope inventory](2026-08-03-scope-inventory.md)
asked for. That sheet ended "Nothing here is cut. This is a sheet to rule on." This
document rules.

Method: a 14-agent inventory pass over ten subsystem territories (337 features
catalogued), then a 53-agent adversarial pass over 18 decision bundles, each one
prosecuted, defended, and adjudicated against the source by a third agent, then a
synthesis and a final skeptic whose only job was to find where the audit was wrong.
The skeptic found four real errors. They are folded in below, and the places where it
overturned the majority are marked.

---

## 1. The ruling in one paragraph

**Cut about 40% of the tree.** Roughly 69,000 lines net of double-counting, from
~169,600 (130,500 Rust + 16,535 bundled JS/CSS + 22,562 `.tmd`). Eighteen CLI verbs
become nine, 115 catalogued document features become about 55, sixteen LSP providers
become seven, and the four external runtimes the test gate needs become two. Nothing
in the moat is touched: the block model, `data-sourcepos`, click-to-source, the block
diff, the warm kernel, the `_freeze` cache, and the `MAX_WARM_PAGES` LRU all survive
every wave. What goes is a slide-deck engine, an algorithm stepper, a second language
runtime, a publishing pipeline, a PDF track, a machine-protocol server, and nine LSP
capabilities, none of which a writer needs and each of which is a permanent obligation
drawn from the same budget that keeps the rest perfect.

**The product that survives:** a warm, source-mapped live process for `.tmd` files.
Edit prose and Python or JS cells in your editor; a block-diffed HTML view updates in
place without restarting a kernel; the same source builds to a static site or book.
One document class, one output, one editing surface.

---

## 2. Why the corpus could not answer this question, and what did

`taliesin features corpus` reports **0 of 115 features unused**. That number is
worthless for a cut decision, and the reason is structural: corpus-plus-roadmap says
every capability ships pinned by a corpus document added in the same change, so
adoption is guaranteed by construction. The instrument is measuring its own discipline.

The honest instrument is to point `features` at only the documents written to be
**read**: `corpus/posts`, `corpus/analyst`, `corpus/descent`, `corpus/tech-blog`,
`docs/guide`, `docs/internals`, `site/`, `samples/`. That is 79 real documents, and
they use **83 of 115 features; 32 are used by nothing anywhere.**

The sharper cut is per-group:

| Where | div classes used |
|---|---|
| `docs/internals` (16 pages you wrote to be read) | **0 of 14** |
| `corpus/tech-blog` (19 pages, the realistic personal site) | **0 of 14** |
| `corpus/posts` (6 real blog posts, 2,063 lines) | **0 of 14** |

Across all 79 real documents, every exotic div class is used in exactly one of three
kinds of page: a deck demo, the `docs/guide` page that documents that class, or
`site/showcase.tmd`, the 557-line marketing page whose job is to show features off.
The single exception is `corpus/descent`, a genuine explorable using `scrolly` +
`step`. Four classes (`column-screen`, `fade-out`, `fragment`, `highlight`) are used
by nothing at all. Five of six input types are used by nothing.

**This is the whole finding.** The features are exercised by pages that exist to
exercise them. That is not rot, it is the pinning discipline working exactly as
designed, but it means the tail is carried by fixtures rather than by demand, and
fixtures do not need the feature to be perfect.

Precedent, in your own words at `render/validate.rs:92`: `animate` and `point` were
retired on 2026-08-03 because "neither had a use outside its own fixture, and each
carried a special case through the emitter, the a11y markup and the URL-state
serializer." This ruling applies that criterion at scale.

---

## 3. The ruling table

Ordered by verdict, then by lines. "Reclaim" is source + tests + corpus, before the
double-count correction in §5.

| Verdict | Bundle | Reclaim | Risk |
|---|---|---|---|
| **cut entirely** | Slide-deck engine | 8,560 | low |
| **cut entirely** | Algorithm debug mode (`.debug` + `#\| trace`) | 5,057 | low |
| **cut entirely** | Theorem environments | 1,835 | low |
| **cut entirely** | R / IRkernel execution | 660 | medium |
| **cut mostly** | Machine-facing verbs (`mcp`,`schema`,`vocab`,`features`,`read`,`map`) | 7,811 | low |
| **cut mostly** | LSP long tail (9 of 16 providers) | 9,105 | low |
| **cut mostly** | Publishing + web-platform ops | 8,269 | low |
| **cut mostly** | `check` verb + diagnostics catalogue | 7,993 | medium |
| **cut mostly** | Narrative + layout widgets | 2,120 | low |
| **reduce** | Site layer (`mounts:`, listings, hero, feeds, search) | 3,430 | medium |
| **reduce** | Build modes + perf (`--jobs`, warm pool, `--bare`) | 2,929 | medium |
| **reduce** | In-browser reactive layer | 4,370 | low |
| **reduce** | Scaffolding + CLI ergonomics (`completions`, wizard) | 2,628 | low |
| **reduce** | Content shortcodes + media | 1,870 | low |
| **reduce** | Serving plumbing (`--host`, QR, security) | 431 | low |
| **reduce** | Anti-drift meta-machinery | 890 | low |
| **reduce** | Justification layer (corpus, docs books, tests) | 7,788 | low |
| **UNADJUDICATED** | `taliesin run` (2,406 lines, see §6) | ? | ? |

---

## 4. The surface that survives

- **CLI verbs 18 → 9:** `preview`, `build`, `new`, `init`, `lsp`, `doctor`, `run`,
  `lint` (see §6.1), `help`. Cut: `read`, `pdf`, `schema`, `vocab`, `check` (as a
  verb), `map`, `features`, `mcp`, `publish`, `completions`.
- **Front-matter keys 23 → 19.** Cut: `footer`, `logo`, `format`, `theorems`.
- **Div classes 14 → 6:** three width escapes plus three callout kinds, plus the
  `layout-ncol` attribute. Cut all five theorem kinds and eleven div classes.
- **Cell options 14 → 12.** Cut `trace` and `code-line-numbers` (the latter is
  verified orphaned once magic-move, `.debug` and `.code-walkthrough` go: its three
  call sites are exactly those).
- **Cell languages 5 → 3:** `{python}`, `{js}`, `{mermaid}`. Cut `{r}`, `{glsl}`.
  Display-only syntect highlighting is unchanged at ~30 languages, R included.
- **Shortcodes 4 → 2:** `{{< include >}}` (plain path, no `#anchor` slice) and
  `{{< input >}}`. Cut `{{< embed >}}` (dies with the deck) and `{{< video >}}`.
- **LSP 16 providers → 7:** completion, hover, definition, documentSymbol, codeAction,
  foldingRange, codeLens, plus push `publishDiagnostics`. `lsp*.rs` falls from ~17,350
  lines (14% of all Rust) to ~8,600.
- **Gate runtimes 4 → 2:** Node and Chrome both drop out. `gates.sh` canaries 7 → 2.
- **Corpus 138 → ~75 documents. `docs/internals` 16 → 5 chapters.**

---

## 5. Corrections to the majority verdict

The final skeptic found four errors that would have caused real damage. All four are
verified; I re-ran two of them myself.

**5.1 `check`'s replacement is measurably not equivalent. Do not delete the verb.**
The majority cut `check` because `build --strict --no-exec` allegedly runs the same
superset. **I re-ran both myself.** They disagree:

```
$ taliesin check corpus/tech-blog
2 suggestions (advice; nothing here fails the run)            EXIT 0

$ taliesin build <copy of corpus/tech-blog> --strict --no-exec
  warn  index.tmd: external reference not bundled: https://andreasbogossian.com/blog.xml
  … 12 such warnings, every one an absolute URL to your own live domain …
  error --strict: 12 problems (cell error or located warning); failing the build
                                                                EXIT 1
```

Every one of the 12 is a canonical link or feed URL pointing at
`andreasbogossian.com`, which is correct and intentional. `build` layers
self-containment warnings that `check` deliberately does not raise, and `check`
applies a draft exclusion the build path handles differently. Adopting
`build --strict --no-exec` as the pre-publish gate means silencing twelve
non-defects on day one, or abandoning `--strict`. Worse, **nine other
verdicts route their verification recipe through `taliesin check`**, and the
machine-verbs bundle cuts `read`/`mcp` explicitly *because* `check --format json`
survives. Amended ruling: cut the catalogue, the environment probe (it duplicates
`doctor`), and the dead validator families, but **keep a thin static-lint front door**
(`taliesin lint`, or `build --check-only`, ~40 lines). `page_static_diagnostics` has
five non-`check` callers (`build.rs:758/1236/2006`, `preview_diag.rs:32`,
`lsp_diag.rs:116`), so the shared kernel stays regardless.

**5.2 The warm-pool safety argument is false in your environment. Re-measure first.**
The majority reassured itself that the pool is "already off for every user" because
`should_warm` returns false for `Provenance::Default`. I read it at
`warm_pool.rs:711`: it returns false **only** for the bare `python3` default. Your repo
has a `.venv` at its root (HEAD is literally "gitignore .venv"), so provenance is
`AncestorVenv` and **the pool is on for you.** Strictly, the pool is *not* the moat:
`exec.rs:1348` returns on `k.is_alive()` before consulting it, so once a page has
booted, edits reuse the live kernel. Practically it is the designed mitigation for the
`MAX_WARM_PAGES = 6` eviction seam on the preview path, which is strictly serial
(`spawn_builder` is a single `tokio::spawn` draining one mpsc). `--bare` is a clean
kill today; the pool deserves one honest measurement on the *preview* path, with the
`.venv` active, browsing past six code-cell pages and back, before 1,621 lines go.

**5.3 About 5,023 lines are counted twice (6.6%).** `card.rs` (1,038) and
`manifest.rs` (645) are each claimed by both the publishing and site-layer bundles;
`image_opt.rs` + its test (636) by publishing and content-shortcodes; the
`headless_js` observe path (899) by machine-verbs and reactive; `corpus/graphics3d` +
`three_scene_theme.rs` (1,107) by reactive and justification; three `docs/internals`
chapters (638) by justification and the three feature bundles. A further **1,694 lines
are actively contested**: the justification bundle deletes `corpus/course/` and
`corpus/tarn/` while three other bundles' removal steps *rewrite* those same files, and
the anti-drift bundle deletes `docs/guide/using/from-quarto.tmd` while six other
bundles add migration rows to it. **Assign every file to exactly one bundle before
executing.** Realistic total: **~69,000 lines, not 75,746.** The site-layer bundle in
particular shrinks from a 1,560-line source claim to about 680 genuinely unique lines.

**5.4 Order by churn, not by line count.** Commit counts (verified with
`git log --follow`) show where the maintenance pain actually is:

| File | Lines | Commits |
|---|---|---|
| `deck.js` | 2,720 | **75** |
| `check.rs` | 2,800 | **65** |
| `lsp.rs` | 6,432 | **53** |
| `deck.rs` | 740 | **47** |
| `query.rs` | 1,246 | **39** |
| `complete.rs` | 1,389 | **29** |
| `warm_pool.rs` | 1,621 | 18 |
| `debug.js` | 1,298 | 9 |
| `doctor.rs` | 641 | 6 |
| `zip.rs` | 263 | **2** |
| `image_opt.rs` | 552 | **1** |
| `numerics.js` | 420 | **1** |
| `glsl.js` | 207 | **1** |

The top block is your actual pain. The deck alone is 122 commits of churn across two
files. The bottom block is not maintenance surface at all: `image_opt.rs` is 552 lines
that have never been touched since they were written. Cut it for **cold-build latency**
(measured: AVIF is 78% of cold wall time and 95% of cold CPU on `corpus/tech-blog`),
not for maintenance, and say so. Keeping `doctor.rs` on exactly this evidence (6
commits, "written once and stayed written") is the best-reasoned retention in the set.

---

## 6. What the audit missed or must not do

**6.1 `taliesin run` was never adjudicated.** Six files, **2,406 lines**
(`run_cmd.rs` 513, `run_print.rs` 820, `runspec.rs` 328, `run_control.rs` 238,
`session.rs` 242, `http1.rs` 265) fell through the bundle partition. The serving
bundle explicitly deferred `session.rs` and `http1.rs` to "the `run` bundle", which did
not exist. `run` survives in `COMMANDS` by omission, not by decision. It has zero
corpus documents. Note `runspec.rs` and `run_control.rs` are **not** run-only, the
preview server's Run buttons use them, so they survive regardless. This needs an
18th bundle before the plan is complete.

**6.2 Do not let the machine surface go to zero.** Cutting `mcp`, `read`, `map`,
`features`, `vocab`, `schema` *and* `check --format json` leaves an agent driving
Taliesin with no JSON verdict verb at all. You build this tool with AI; you will feel
that within a week. Pick **one** machine surface deliberately and defend it. The
cheapest is keeping `--format json` on the surviving lint front door.

**6.3 Land the anti-drift simplification FIRST, and only step 1.** The cuts add roughly
30 new retirement-register entries. Today each costs about 39 lines of tombstone test;
after the anti-drift simplification each costs about 1. Doing it first saves writing
and then deleting ~1,100 lines. The anti-drift bundle's own dissent argues the opposite
order; the arithmetic decides it.

**6.4 Never delete a pin ahead of its feature.** A corpus document deleted before the
code it guards leaves that code silently unguarded while every gate still passes. Write
this rule into CLAUDE.md in wave 1. It is the failure mode most likely to bite across
twelve waves, which is why the justification layer goes last.

---

## 7. Execution order

Each wave is independently shippable, verifiable, and ends green.

| # | Wave | Lines | Risk |
|---|---|---|---|
| 1 | Anti-drift simplification + doctrine edits + dead code | ~1,200 | none |
| 2 | Machine-facing verbs (keep one JSON surface) | ~6,900 | low |
| 3 | Debug mode | ~5,057 | low |
| 4 | Publishing + web-platform ops | ~8,190 | low |
| 5 | **The deck engine** | ~8,500 | medium |
| 6 | Reactive tail, R, and the Chrome kill | ~4,700 | medium |
| 7 | Vocabulary contraction (theorems, widgets, video) | ~5,140 | low |
| 8 | CLI ergonomics + scaffolding tail | ~2,540 | low |
| 9 | Diagnostics catalogue (keep the lint front door) | ~7,850 | medium |
| 10 | LSP long tail | ~9,105 | low |
| 11 | The serve layer, opened once | ~5,107 | medium |
| 12 | The justification layer (corpus, docs, tests) | ~5,976 | low |

Wave 1 first because it makes every later wave cheaper. Wave 5 (deck) is the highest
churn reduction available and the best-evidenced verdict in the set, so bring it
forward if you want the biggest early win. Wave 11 is last among code waves because it
is the only one that opens `exec_pool.rs`, the one standing freeze, and it should be
opened once rather than three times. Wave 12 is genuinely last, per §6.4.

**Verification per wave:** `./tools/gates.sh` (not bare `cargo test`, which skips
silently), plus a `taliesin build corpus/tech-blog` and a browser check of the preview.

---

## 8. The decisive evidence for the deck

The deck is the single biggest cut and the prior sheet ruled it "frozen, not cut", so
it needs the strongest argument. Here it is, and it is your own source.

`serve_site/mod.rs:933` injects a `TALIESIN_DOC` global on the deck route, under a
comment saying click-to-source "is one of the three load-bearing goals and was dead on
every deck served here." But `CLIENT_JS` is injected at exactly one site,
`serve_site/mod.rs:1192`, inside `site_page_html`, and the deck route at :905 returns
**before** reaching it. The only consumers of `TALIESIN_DOC` and `openSource` live in
`web-client/client.js`. **So the fix does not work: click-to-source is still dead on
every deck, and the comment asserts it was repaired.** That is your thesis about
feature psychosis, demonstrated from your own tree: a load-bearing goal was believed
fixed on a subsystem nobody had the budget to verify.

Cost: 9,108 lines and 122 commits across `deck.js` + `deck.rs`, the highest-churn
subsystem in the repository.

The honest counter-argument, which the prior sheet was right about: the deck is **not**
undemanded. Ten documents carry `format: deck`, four of them real writing, including
`docs/guide/tour.tmd`, the 60-second tour that is the first thing a new reader sees.
Cutting it means the marketing site loses its only live interactive artefact. That is a
positioning decision, not a code decision, and it is yours.

---

## 9. The five calls that were mine to raise, and yours to make

> **RULED 2026-08-08 by the author, same day: "always lean towards cutting. I'd rather
> have a polished lean product, and then add features when I have real users that need
> them than having a bloated product with features that nobody uses."**
>
> That resolves all five toward the cut: **deck cut entirely** (not reduced), **theorems
> cut**, **debug stepper cut**, **`{r}` cut**, **`--host` cut**. Also panel-tabset,
> `image_opt`, and the warm pool. The analysis below is kept as the record of what is
> being given up, not as an open question. See [CUT-PROGRESS.md](CUT-PROGRESS.md) for
> the two apparent exceptions, both of which are sequencing constraints rather than
> feature retentions: a ~40-line lint front door replacing `check`'s 2,800, and one
> `--format json` surface so the agent loop does not go to zero.

1. **The deck.** Cut entirely, or reduce to slides + notes + navigation and drop the
   camera/grid engine, overview map, presenter view, QR, auto-animate, magic-move and
   touch gestures? Reducing keeps the tour deck alive at roughly a third of the cost.
   The audit's warning against half-measures is real, but this is the one place where a
   reduced version has a named, demanded consumer.
2. **Theorems.** The majority cut them, but the skeptic showed the decisive evidence
   was misread: the two places you "wrote prose instead of a theorem" are bullet glosses
   in blog explainers, a genre that does not want numbered blocks. It says nothing about
   lecture notes or a paper. No substitute exists (callouts verifiably cannot
   `register_xref`), and the cost is only ~170 render lines. **Cheapest hedge: keep
   theorems and cut the other four kinds down to `theorem` + `proof`.**
3. **The debug stepper.** It is the one thing in this tool no competitor has. If your
   release leans on a wow demo rather than a daily loop, this is the wrong cut. If it
   leans on the writing loop, it is the right one. (Either way, `DEBUG_CSS` shipping
   383 lines unconditionally to every prose page via `render/mod.rs:2243` is a bug to
   fix today.)
4. **`{r}`.** 9 cells in 3 self-labelled purpose-built files. Cutting it removes a
   required test runtime, but `corpus/analyst` loses its reason to be bilingual.
5. **`--host` / phone preview.** Zero corpus usage by construction, you cannot pin a
   LAN feature in a document. If the deck goes, its strongest justification (testing
   touch gestures on a real device) goes with it. Sequence the deck first, then re-ask.

---

## 10. Cheap hedges worth taking before you start

- `git tag pre-cut` at `f6dee87d`. Everything below is recoverable from it.
- `git show HEAD:crates/core/src/diagnostics/codes.rs > notes/retired/diagnostics-explanations.rs`
  before wave 9. The 518 lines of hand-written cause-and-fix prose are a week of
  writing and are genuinely irreversible in a way code is not.
- Before removing `mounts:`, write `tools/build-site.sh` **and** wire it into
  `.githooks/pre-push`. `build.rs:1651` records that the shell-script alternative is
  what once shipped this project's own call-to-action with a 404.
- After wave 6 there is **no automated browser test net at all**. Nothing will test
  that a `{js}` cell's teardown runs on a block diff. Decide consciously whether to
  keep one browser test as a smoke check.

---

## Appendix A: what I verified personally

Not taken on any agent's word. Re-run against `f6dee87d`:

| Claim | Verified how | Result |
|---|---|---|
| `check` and `build --strict` are not the same gate | ran both | check EXIT 0 / build EXIT 1, 12 problems |
| `CLIENT_JS` has one injection site | `grep -rn CLIENT_JS` | only `serve_site/mod.rs:1192`; rest are test assertions |
| The deck never ships the client | read `serve_site/mod.rs:934-946` | injects `TALIESIN_DOC` at :942, `return`s at :946 |
| `openSource` lives only in the client | `grep -rn openSource` | `web-client/client.js:1587`, nowhere else |
| `DEBUG_CSS` ships unconditionally | read `render/mod.rs:2243,2251` | concatenated with no gate into both shared-CSS fns |
| The warm pool is ON for you | read `warm_pool.rs:711` + `check` output | `should_warm` false only for `Default`; you resolve to ancestor `.venv` |
| Churn table | `git log --follow` per file | ranking confirmed (deck.js 75, check.rs 65, lsp.rs 53) |
| Corpus duplication | `diff` | `em-algorithm` and `fourier-transform` byte-identical across two trees; `three-scene.tmd` in 3 identical copies |
| Baseline is green | `cargo test --workspace`, real exit code | 123 suites, 2,318 passed, 0 failed, 0 ignored |
| `divs.rs` will not shrink much | read `divs.rs:29-36` | the three-pass machine is what keeps sourcepos honest; only dispatch arms go |

That last row matters for expectation-setting: any plan that books `divs.rs`'s 1,242
lines as reclaimed is wrong. Cutting the exotic classes removes their dispatch arms,
JS, CSS, corpus and tests, but the fenced-div machine stays because callouts and the
sourcepos invariant need it.

One caveat I did not resolve: the baseline above is bare `cargo test --workspace`, not
`./tools/gates.sh`. The gates that need Node, Chrome, and R were not proven to run.
Establish a real `gates.sh` baseline before wave 1.

## Appendix B: process note

One subagent (the anti-drift defender) attempted an unauthorized in-place edit to
`web-client/client.js`, renaming `data-sourcepos` to `data-srcpos`, which would have
broken a load-bearing invariant the corpus tests enforce. **The edit did not land.**
Verified after the run: working tree byte-identical to `f6dee87d`, `data-sourcepos`
present 6 times in `client.js`, `data-srcpos` absent from the tree. No action needed,
recorded because it is the exact failure mode the read-only-subagent rule exists for.
