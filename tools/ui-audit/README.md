# Taliesin UI-Audit Harness

Finds UI bugs across Taliesin's entire rendered surface by **capturing once,
cheaply, then analyzing wide**. The browser is a serial bottleneck, so instead
of fanning out browser-driving agents the work splits into three phases, and
only one of them touches the browser:

1. **Capture** (`capture-run.mjs`): a scripted headless Puppeteer loop builds
   every page and screenshots it across 3 viewports and 2 themes, dumping
   console/network/DOM logs. Deterministic, no agents, no tokens.
2. **Probe** (`probe-run.mjs`): drives a live `taliesin preview` to exercise the
   interactive features (deck nav, search, lightbox, hover-preview, TOC
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
#   --jobs N             concurrent tabs (default 3; raise cautiously: large
#                        pages can crash a tab under high concurrency, though a
#                        crashed cell auto-retries once and a dead browser
#                        relaunches)
#   --no-build           reuse an existing .work/build (skip rebuilding)
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

The full surface is ~214 source docs building to roughly 150-200 pages, times 6
matrix cells (~1,000+ screenshots). Expect a long, unattended run; it is safe to
leave in the background. Executed code-cell output requires a Jupyter kernel
(`TALIESIN_PYTHON` / `TALIESIN_R`); without one, cells degrade to source and the
build still succeeds.

## Probe (interactive features)

```sh
node probe-run.mjs                 # all features
node probe-run.mjs --only lightbox # one feature
```

Spawns a live `taliesin preview` per representative target and asserts each
feature's expected DOM/JS outcome (click-to-source needs the live preview's
`window.TALIESIN_DOC` + websocket, which a static build lacks). Writes
`.work/probe-results.json`.

## Analyze / verify / report (the Workflow)

`audit.workflow.js` is a Claude Code Workflow. Run it via the **Workflow tool**
(ask Claude to run the audit workflow), not with `node`. Args:

```js
args = {
  artifactsRoot:    "<abs>/tools/ui-audit/.work/artifacts",
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
node capture-run.mjs --scale 0.5                   # ~1,000+ half-res shots (free, background)
node probe-run.mjs                                 # interactive features (free)
# then, in a Claude session, batch by unit to pace token spend, e.g.:
#   "run the ui-audit workflow, onlyUnits ['tech-blog']"
#   "run the ui-audit workflow, onlyUnits ['docs-guide']"
#   ... one batch per site project, plus one for the standalone corpus docs.
```

Do **not** run `capture-run` and `probe-run` at the same time: two headless
Chromes plus preview servers contend and can crash the browser (the harness
relaunches and continues, but it is slower). Run them sequentially.

`.work/` and `node_modules/` are gitignored. Nothing here is in the cargo
workspace.
