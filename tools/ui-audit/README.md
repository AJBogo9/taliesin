# Taliesin UI-Audit Harness

Finds UI bugs across Taliesin's entire rendered surface by **capturing once,
cheaply, then analyzing wide**. The browser is a serial bottleneck, so instead
of fanning out browser-driving agents the work splits into three phases, and
only one of them touches the browser:

1. **Capture** (`capture-run.mjs`): a scripted headless Puppeteer loop builds
   every page and screenshots it across 3 viewports and 2 themes, dumping
   console/network/DOM logs. Deterministic, no agents, no tokens.
2. **Probe** (`probe-run.mjs`): drives a live `taliesin preview` to exercise the
   interactive features (deck nav, search, TOC
   scrollspy, click-to-source) on representative pages. No tokens.
3. **Analyze, dedup, verify, report** (`audit.workflow.js`): a Claude Code
   *Workflow* fans analysis across the captured artifacts, clusters findings by
   root cause, adversarially verifies the visual ones, and writes a ranked
   report. This is the parallel phase, and the only one that spends tokens.

See the design spec: `docs/superpowers/specs/2026-07-08-ui-audit-harness-design.md`.

## Requirements

- **Node** (works on v20; puppeteer-core declares v22+, so v20 prints a harmless
  `EBADENGINE` warning. Upgrade to node 22 LTS to silence it, optional).
- **Google Chrome** at `/usr/bin/google-chrome` (override with `CHROME_PATH`).
- A **taliesin** binary. Auto-resolved in this order: `--bin`, `$TALIESIN_BIN`,
  `target/release/taliesin`, then `taliesin` on `PATH`. Build a fresh release
  binary first for speed: `cargo build -p taliesin-server --release`.

```sh
cd tools/ui-audit
npm install          # puppeteer-core only (no Chromium download)
```

## Capture

```sh
# Everything (all 6 site projects + ~29 standalone corpus docs):
node capture-run.mjs

# A slice (glob/substring matched against unit slug or source; repeatable):
node capture-run.mjs --only 'corpus/deck.tmd' --only 'demo-book'

# Flags:
#   --only <glob>        select units (repeat for several); default = all
#   --viewports a,b,c    subset of: mobile,laptop,portrait
#   --themes light,dark  subset of themes
#   --scale N            deviceScaleFactor for screenshots (default 1). Use 0.5
#                        for half-resolution shots: ~4x fewer image pixels, so
#                        the workflow spends far fewer vision tokens, still
#                        plenty legible for layout bugs.
#   --jobs N             concurrent tabs per process (default = cores-4, clamped
#                        to 4..10). Capture is wait-bound, so more tabs help;
#                        a crashed tab auto-retries once, a dead browser
#                        relaunches, and a wedged tab is force-closed after 60s.
#   --parallel N         fan the run across N browser PROCESSES (each its own
#                        Chrome), units split by size-balanced LPT so the heavy
#                        multi-page books land on different shards. Breaks the
#                        single-browser CDP ceiling: one process tops out around
#                        5 cells/s no matter the --jobs, but N processes scale
#                        with the (otherwise idle) CPU. Auto-merges the shards.
#   --shard i/N          run only shard i of N by hand (advanced; --parallel
#                        does this for you). Share one --out across the N procs,
#                        then `--merge` to combine their manifest.shard-*.json.
#   --merge              merge manifest.shard-*.json in <out>/artifacts into
#                        manifest.json (run after hand-launched --shard procs).
#   --max-open N         cap how many built units are served ahead of capture
#                        (backpressure; default 6).
#   --no-build           reuse an existing build dir (skip rebuilding)
#   --no-cache           set TALIESIN_NO_CACHE=1 (force fresh cell execution)
#   --bin <path>         taliesin binary
#   --out <dir>          work dir (default ./.work)
```

Output:

- `.work/build/<unit>/...`: the static builds.
- `.work/artifacts/<unit>/<route>/<viewport>__<theme>.png` + `.json`: the
  screenshot and its per-cell log (console, network, DOM flags).
- `.work/artifacts/manifest.json`: one record per cell (paths + triage counts +
  the actual console/network errors).

The full surface is 35 units (6 site projects + 29 standalone corpus docs)
building to ~89 pages, times 6 matrix cells (~530 screenshots). With
`--parallel 3 --scale 0.5` on an 8-core/16-thread box this is roughly a
one-minute capture (plus builds); the pipeline overlaps builds with capture and
the whole thing is safe to leave in the background. Executed code-cell output
requires a Jupyter kernel (`TALIESIN_PYTHON` / `TALIESIN_R`); without one, cells
degrade to source and the build still succeeds.

**Speed.** Capture is wait-bound (per-cell settle + screenshot), not
compute-bound: a single browser leaves most of the CPU idle and tops out around
5 cells/s regardless of `--jobs`. `--parallel N` runs N browser processes over
that idle CPU and scales past the single-browser ceiling; `--parallel 3` splits
this corpus into a balanced 30/30/30 pages across shards (each of the three big
books on its own browser). Reliability came along for free: builds are async and
pipelined, a wedged tab is force-closed after 60s (no more whole-run hangs), and
a crashed browser relaunches mid-run.

## Probe (interactive features)

```sh
node probe-run.mjs                      # all features
node probe-run.mjs --only toc-scrollspy # one feature
```

Spawns a live `taliesin preview` per representative target and asserts each
feature's expected DOM/JS outcome (click-to-source needs the live preview's
`window.TALIESIN_DOC` + websocket, which a static build lacks). Writes
`.work/probe-results.json`.

## Analyze / verify / report (the Workflow)

`audit.workflow.js` is a Claude Code Workflow. Run it via the **Workflow tool**
(ask Claude to run the audit workflow), not with `node`.

A workflow script has **no filesystem access**, so it reads the manifest through
an agent. A full-corpus manifest (~530 cells) is ~66K tokens, past the 64K
agent-output ceiling, so a single reader agent cannot echo it back. Before a
**full** audit, shard the manifest first:

```sh
node split-manifest.mjs   # writes .work/artifacts/parts/<unit>.json + _index.json
```

then pass `manifestPartsDir` (below): the workflow reads the tiny index and fans
out one small reader per unit in parallel. `manifestPath` (single reader) is fine
only for a **small** `onlyUnits` slice that fits under the cap.

Args:

```js
args = {
  artifactsRoot:    "<abs>/tools/ui-audit/.work/artifacts",
  // full run (preferred): shard first with split-manifest.mjs, then:
  manifestPartsDir: "<abs>/tools/ui-audit/.work/artifacts/parts",
  // OR, for a small onlyUnits slice only, a single-agent read of the whole file:
  manifestPath:     "<abs>/tools/ui-audit/.work/artifacts/manifest.json",
  probeResultsPath: "<abs>/tools/ui-audit/.work/probe-results.json", // optional
  // cost controls (balanced defaults):
  model:      "sonnet",   // model for all agents; default 'sonnet' keeps the
                          // run off the scarce weekly-Opus budget
  fullMatrix: false,      // false (default): analyze one theme across all
                          // viewports + any flagged cell (3 vision reads/clean
                          // page). true: analyze all 6 cells/page (exhaustive,
                          // costlier)
  onlyUnits:  ["tech-blog"] // optional: restrict to these unit slugs, so one
                            // capture can be audited in batches across days
}
```

It returns `{ reportMarkdown, summary, confirmedVisual, consoleNetwork,
probeResults, buildFailures }`. Claude writes `reportMarkdown` to `.work/report.md`.

Epistemics baked in: console/network findings are treated as **facts** (harvested
mechanically from the logs, deduped + attributed, never at an agent's
discretion); visual findings are **judgments** (clustered by root cause, then
each cluster is adversarially verified: a skeptic tries to refute it against the
screenshot + the source CSS/emit code, and it only survives if it can't be
refuted).

## Cost and pacing

Capture and probe are free (local Chrome, no model calls); only the Workflow
spends tokens, dominated by vision over screenshots. On the balanced defaults
(Sonnet, half the cells per page) a full run is far cheaper than the exhaustive
default and stays off the weekly-Opus budget. To pace it against session/weekly
limits, capture once (free), then run the workflow in batches with `onlyUnits`
(one or a few unit slugs per invocation) across several sessions or days.

## Full-run recipe (balanced)

```sh
cargo build -p taliesin-server --release          # fresh binary
cd tools/ui-audit && npm install
node capture-run.mjs --scale 0.5 --parallel 3      # ~530 half-res shots, ~1 min (free, background)
# if capture deadlocked before writing manifest.json, salvage it from the metas:
#   node rebuild-manifest.mjs --scale 0.5
node split-manifest.mjs                            # shard manifest -> parts/ (needed for a full audit)
node probe-run.mjs                                 # interactive features (free)
# then, in a Claude session, run the whole surface in one workflow (manifestPartsDir),
# or batch by unit to pace token spend, e.g.:
#   "run the ui-audit workflow over manifestPartsDir"
#   "run the ui-audit workflow, onlyUnits ['tech-blog']"
#   ... one batch per site project, plus one for the standalone corpus docs.
```

Do **not** run `capture-run` and `probe-run` at the same time: two headless
Chromes plus preview servers contend and can crash the browser (the harness
relaunches and continues, but it is slower). Run them sequentially.

`.work/` and `node_modules/` are gitignored. Nothing here is in the cargo
workspace.
