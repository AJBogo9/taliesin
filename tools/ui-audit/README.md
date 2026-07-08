# Taliesin UI-Audit Harness

Finds UI bugs across Taliesin's entire rendered surface by **capturing once,
cheaply, then analyzing wide**. The browser is a serial bottleneck, so instead
of fanning out browser-driving agents we split the work into three phases and
only one of them touches the browser:

1. **Capture** (`capture-run.mjs`) — a scripted headless Puppeteer loop builds
   every page and screenshots it across **3 viewports × 2 themes**, dumping
   console/network/DOM logs. Deterministic, no agents.
2. **Probe** (`probe-run.mjs`) — drives a live `taliesin preview` to exercise the
   interactive features (deck nav, search, lightbox, hover-preview, TOC
   scrollspy, click-to-source) on representative pages.
3. **Analyze → dedup → verify → report** (`audit.workflow.js`) — a Claude Code
   *Workflow* fans analysis across the captured artifacts, clusters findings by
   root cause, adversarially verifies the visual ones, and writes a ranked
   report. This is the parallel phase.

See the design spec: `docs/superpowers/specs/2026-07-08-ui-audit-harness-design.md`.

## Requirements

- **Node** (works on v20; puppeteer-core declares v22+, so v20 prints a harmless
  `EBADENGINE` warning — upgrade to node 22 LTS to silence it, optional).
- **Google Chrome** at `/usr/bin/google-chrome` (override with `CHROME_PATH`).
- A **taliesin** binary. Auto-resolved in this order: `--bin`,
  `$TALIESIN_BIN`, `target/release/taliesin`, then `taliesin` on `PATH`. Build a
  fresh release binary first for speed: `cargo build -p taliesin-server --release`.

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
#   --jobs N             concurrent tabs (default 3; raise cautiously — large
#                        pages can crash a tab under high concurrency, though a
#                        crashed cell auto-retries once)
#   --no-build           reuse an existing .work/build (skip rebuilding)
#   --no-cache           set TALIESIN_NO_CACHE=1 (force fresh cell execution)
#   --bin <path>         taliesin binary
#   --out <dir>          work dir (default ./.work)
```

Output:

- `.work/build/<unit>/…` — the static builds.
- `.work/artifacts/<unit>/<route>/<viewport>__<theme>.png` + `.json` — the
  screenshot and its per-cell log (console, network, DOM flags).
- `.work/artifacts/manifest.json` — one record per cell (paths + triage counts).

The full surface is ~214 source docs → many pages × 6 matrix cells (~1,200+
screenshots). Expect a long, unattended run; it is safe to leave in the
background. Executed code-cell output requires a Jupyter kernel
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

`audit.workflow.js` is a Claude Code Workflow — run it via the **Workflow tool**
(ask Claude to run the audit workflow), not with `node`. Pass:

```js
args = {
  artifactsRoot: "<abs>/tools/ui-audit/.work/artifacts",
  manifestPath:  "<abs>/tools/ui-audit/.work/artifacts/manifest.json",
  probeResultsPath: "<abs>/tools/ui-audit/.work/probe-results.json" // optional
}
```

It returns `{ reportMarkdown, summary, confirmedVisual, consoleNetwork,
probeResults, buildFailures }`. Claude writes `reportMarkdown` to
`.work/report.md`.

Epistemics baked in: console/network findings are treated as **facts** (deduped
+ attributed, no refutation); visual findings are **judgments** (clustered by
root cause, then each cluster is adversarially verified — a skeptic tries to
refute it against the screenshot + the source CSS/emit code, and it only
survives if it can't be refuted).

## Full-run recipe

```sh
cargo build -p taliesin-server --release      # fresh binary
cd tools/ui-audit && npm install
node capture-run.mjs                          # ~1,200+ screenshots (background)
node probe-run.mjs                            # interactive features
# then ask Claude: "run the ui-audit workflow on .work/artifacts/manifest.json"
```

`.work/` is gitignored. Nothing here is in the cargo workspace.
