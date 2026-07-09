#!/usr/bin/env node
// Capture stage CLI: build every (selected) unit to static HTML, serve it, and
// screenshot the full page x viewport x theme matrix with console/network/DOM
// logs. Writes .work/artifacts/ + manifest.json.
//
// The loop is PIPELINED: builds (CPU + Jupyter kernel) run one at a time but
// asynchronously, so the NEXT unit builds while a single cross-unit browser pool
// is still screenshotting the PREVIOUS one. All units' cells feed one shared
// queue of `--jobs` tabs, so the pool never drains to empty between units. This
// keeps the (otherwise ~90% idle) machine busy: capture is wait-bound, not
// compute-bound, so concurrency is the dominant speed lever.
//
// Usage:
//   node capture-run.mjs [--only <glob>] [--viewports mobile,laptop,portrait]
//                        [--themes light,dark] [--out <dir>] [--bin <taliesin>]
//                        [--jobs N] [--scale N] [--max-open N]
//                        [--shard i/N] [--merge] [--no-build] [--no-cache]
//
// --only accepts a glob/substring matched against unit slug or source, e.g.
//   --only 'corpus/deck.tmd'      one standalone
//   --only 'demo-book'            one site project
//   --only 'refs__*'              all refs standalones
// Pass --only multiple times to select several units.
//
// --shard i/N runs only shard i of N (0-based, units interleaved by index so
//   heavy units spread evenly). Launch N processes that SHARE one --out dir
//   (each writes manifest.shard-i.json; unit build/artifact dirs are disjoint),
//   then `node capture-run.mjs --merge` combines them into manifest.json.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { allUnits, matchesGlob, estimatePages } from './lib/units.mjs';
import { buildUnit, enumeratePages } from './lib/build.mjs';
import { serveDir } from './lib/serve.mjs';
import { launch, DEFAULT_VIEWPORTS, DEFAULT_THEMES } from './lib/browser.mjs';
import { captureCellWithRetry } from './lib/capture.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '../..');

// Waiting browser tabs cost RAM but almost no CPU, so on a many-thread box you
// can run far more than the historical jobs=3. Default to a chunk of the cores,
// clamped so a small machine does not oversubscribe and a big one does not
// crash-churn tabs on very large pages (a crashed tab auto-retries either way).
const CORES = os.availableParallelism?.() ?? os.cpus().length;
const DEFAULT_JOBS = Math.max(4, Math.min(10, CORES - 4));
// Total concurrent-tab target when fanning out across shard processes
// (--parallel). Capture is wait-bound but screenshot encoding is CPU work, so
// ~one tab per hardware thread saturates the box without thrashing.
const TAB_BUDGET = Math.max(4, CORES);

function parseArgs(argv) {
  const args = {
    only: [],
    viewports: null,
    themes: null,
    out: path.join(__dirname, '.work'),
    bin: null,
    jobs: DEFAULT_JOBS,
    jobsExplicit: false,
    scale: 1,
    maxOpen: 6,
    shard: null,
    parallel: 1,
    merge: false,
    build: true,
    noCache: false,
  };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    const val = () => argv[++i];
    if (a === '--only') args.only.push(val());
    else if (a === '--viewports') args.viewports = val().split(',');
    else if (a === '--themes') args.themes = val().split(',');
    else if (a === '--out') args.out = path.resolve(val());
    else if (a === '--bin') args.bin = val();
    else if (a === '--jobs') { args.jobs = parseInt(val(), 10) || DEFAULT_JOBS; args.jobsExplicit = true; }
    else if (a === '--scale') args.scale = parseFloat(val()) || 1;
    else if (a === '--max-open') args.maxOpen = Math.max(1, parseInt(val(), 10) || 6);
    else if (a === '--shard') args.shard = val();
    else if (a === '--parallel') args.parallel = Math.max(1, parseInt(val(), 10) || 1);
    else if (a === '--merge') args.merge = true;
    else if (a === '--no-build') args.build = false;
    else if (a === '--no-cache') args.noCache = true;
    else if (a === '--help' || a === '-h') {
      console.log(fs.readFileSync(path.join(__dirname, 'README.md'), 'utf8'));
      process.exit(0);
    } else {
      console.error(`unknown flag: ${a}`);
      process.exit(2);
    }
  }
  return args;
}

function resolveBin(explicit) {
  if (explicit) return explicit;
  if (process.env.TALIESIN_BIN) return process.env.TALIESIN_BIN;
  const release = path.join(REPO_ROOT, 'target/release/taliesin');
  if (fs.existsSync(release)) return release;
  return 'taliesin'; // PATH launcher
}

// Longest-processing-time (LPT) shard assignment: sort units by estimated size
// descending, then greedily place each on the currently least-loaded shard.
// Deterministic, so every shard process computes the identical split and just
// keeps its own bucket. Balances far better than round-robin when a few
// multi-page units dominate (round-robin can drop two big books on one shard).
function lptShardUnits(units, N, shardIndex, repoRoot) {
  const weighted = units.map((u, i) => ({ u, w: estimatePages(u, repoRoot), i }));
  weighted.sort((a, b) => b.w - a.w || a.i - b.i);
  const loads = new Array(N).fill(0);
  const buckets = Array.from({ length: N }, () => []);
  for (const { u, w } of weighted) {
    let m = 0;
    for (let k = 1; k < N; k++) if (loads[k] < loads[m]) m = k;
    buckets[m].push(u);
    loads[m] += w;
  }
  return buckets[shardIndex];
}

// A tiny multi-waiter signal: every pending wait() resolves on the next fire().
function makeSignal() {
  let waiters = [];
  return {
    wait: () => new Promise((r) => waiters.push(r)),
    fire: () => {
      const w = waiters;
      waiters = [];
      for (const r of w) r();
    },
  };
}

// Merge shard manifests (manifest.shard-*.json) in one artifacts dir into a
// single manifest.json. Shards share the artifacts root, so the relative
// screenshot/meta paths stay valid; we just concatenate the page + failure lists.
function mergeShards(artifactsRoot) {
  const files = fs
    .readdirSync(artifactsRoot)
    .filter((f) => /^manifest\.shard-\d+\.json$/.test(f))
    .sort();
  if (!files.length) {
    console.error(`no manifest.shard-*.json in ${artifactsRoot}`);
    process.exit(1);
  }
  const pages = [];
  const buildFailures = [];
  let base = null;
  for (const f of files) {
    const m = JSON.parse(fs.readFileSync(path.join(artifactsRoot, f), 'utf8'));
    base ||= m;
    pages.push(...(m.pages || []));
    buildFailures.push(...(m.buildFailures || []));
  }
  const merged = {
    ...base,
    generatedAt: new Date().toISOString(),
    unitCount: undefined,
    pageCount: new Set(pages.map((r) => `${r.unit}${r.route}`)).size,
    cellCount: pages.length,
    buildFailures,
    pages,
  };
  const outPath = path.join(artifactsRoot, 'manifest.json');
  fs.writeFileSync(outPath, JSON.stringify(merged, null, 2));
  console.log(
    `[ui-audit] merged ${files.length} shard manifests -> ${merged.pageCount} pages, ` +
      `${merged.cellCount} cells\n           ${outPath}`,
  );
}

// Fan the capture across N sibling processes, each with its own browser (breaks
// the single-CDP-socket / single-Node-thread ceiling a lone process hits at high
// tab counts, and parallelizes the builds across idle cores). Each child runs
// one shard into the SHARED --out dir; then we merge the per-shard manifests.
async function runParallel(args) {
  const N = args.parallel;
  const perShard = args.jobsExplicit
    ? args.jobs
    : Math.max(2, Math.round(TAB_BUDGET / N));
  const artifactsRoot = path.join(args.out, 'artifacts');
  fs.mkdirSync(artifactsRoot, { recursive: true });
  // Drop stale shard manifests so the final merge can't fold in a prior run.
  for (const f of fs.readdirSync(artifactsRoot))
    if (/^manifest\.shard-\d+\.json$/.test(f))
      fs.rmSync(path.join(artifactsRoot, f));

  console.log(
    `[ui-audit] parallel: ${N} shards x ${perShard} jobs ` +
      `(~${N * perShard} tabs) -> ${args.out}`,
  );

  const passthrough = [];
  for (const g of args.only) passthrough.push('--only', g);
  if (args.viewports) passthrough.push('--viewports', args.viewports.join(','));
  if (args.themes) passthrough.push('--themes', args.themes.join(','));
  if (args.bin) passthrough.push('--bin', args.bin);
  if (!args.build) passthrough.push('--no-build');
  if (args.noCache) passthrough.push('--no-cache');

  const waits = [];
  for (let i = 0; i < N; i++) {
    const argv = [
      __filename,
      '--shard', `${i}/${N}`,
      '--jobs', String(perShard),
      '--scale', String(args.scale),
      '--max-open', String(args.maxOpen),
      '--out', args.out,
      ...passthrough,
    ];
    const child = spawn(process.execPath, argv, { cwd: __dirname });
    const tag = `[s${i}]`;
    const pipe = (stream, sink) => {
      let buf = '';
      stream.on('data', (d) => {
        buf += d;
        let nl;
        while ((nl = buf.indexOf('\n')) >= 0) {
          sink(`${tag} ${buf.slice(0, nl)}`);
          buf = buf.slice(nl + 1);
        }
      });
      stream.on('end', () => {
        if (buf.trim()) sink(`${tag} ${buf}`);
      });
    };
    pipe(child.stdout, (l) => console.log(l));
    pipe(child.stderr, (l) => console.error(l));
    waits.push(new Promise((r) => child.on('close', (code) => r({ i, code }))));
  }

  const results = await Promise.all(waits);
  const failed = results.filter((r) => r.code !== 0);
  if (failed.length)
    console.error(
      `[ui-audit] ${failed.length}/${N} shards exited non-zero: ` +
        failed.map((r) => r.i).join(','),
    );
  mergeShards(artifactsRoot);
}

async function main() {
  const args = parseArgs(process.argv);
  const artifactsRoot = path.join(args.out, 'artifacts');

  if (args.merge) {
    mergeShards(artifactsRoot);
    return;
  }

  // Top-level fan-out into shard processes (children carry --shard, not
  // --parallel, so they fall through to the single-process path below).
  if (args.parallel > 1 && !args.shard) {
    await runParallel(args);
    return;
  }

  const bin = resolveBin(args.bin);
  const viewports = args.viewports
    ? DEFAULT_VIEWPORTS.filter((v) => args.viewports.includes(v.name))
    : DEFAULT_VIEWPORTS;
  const themes = args.themes || DEFAULT_THEMES;

  let units = allUnits(REPO_ROOT);
  if (args.only.length) {
    units = units.filter((u) => args.only.some((g) => matchesGlob(u, g)));
  }
  let shardI = null;
  let shardN = null;
  if (args.shard) {
    const [iStr, nStr] = String(args.shard).split('/');
    shardI = parseInt(iStr, 10);
    shardN = parseInt(nStr, 10);
    if (!(shardN > 0 && shardI >= 0 && shardI < shardN)) {
      console.error(`bad --shard ${args.shard}: expected i/N with 0 <= i < N`);
      process.exit(2);
    }
    units = lptShardUnits(units, shardN, shardI, REPO_ROOT);
  }
  if (!units.length) {
    console.error('no units matched');
    process.exit(1);
  }

  const buildRoot = path.join(args.out, 'build');
  fs.mkdirSync(buildRoot, { recursive: true });
  fs.mkdirSync(artifactsRoot, { recursive: true });

  console.log(
    `[ui-audit] bin=${bin}\n[ui-audit] units=${units.length} ` +
      `viewports=${viewports.map((v) => v.name).join(',')} ` +
      `themes=${themes.join(',')} jobs=${args.jobs} maxOpen=${args.maxOpen}` +
      (args.shard ? ` shard=${shardI}/${shardN}` : ''),
  );

  // ---- browser (single instance, transparently relaunched on crash) ---------
  let browser = await launch();
  let relaunchP = null;
  async function ensureBrowser() {
    if (browser && browser.connected !== false) return browser;
    if (!relaunchP) {
      relaunchP = (async () => {
        console.log('[ui-audit] browser disconnected; relaunching');
        try {
          await browser.close();
        } catch {
          /* already gone */
        }
        browser = await launch();
        relaunchP = null;
      })();
    }
    await relaunchP;
    return browser;
  }

  // ---- pipeline state -------------------------------------------------------
  const cellQueue = []; // { cell, serverUrl, unitSlug }
  const servers = new Map(); // unitSlug -> { server, remaining, source, pageCount }
  const allRecords = [];
  const buildFailures = [];
  let buildsDone = false;
  const work = makeSignal(); // fired when cells enqueued or builds finish
  const drain = makeSignal(); // fired when a server closes (releases backpressure)

  // Producer: build units one at a time (async, so capture keeps running), then
  // enqueue every cell against that unit's server. Backpressure caps how far the
  // builds run ahead of capture (bounds open servers + built-but-unshot output).
  async function producer() {
    for (const unit of units) {
      while (servers.size >= args.maxOpen) await drain.wait();

      const outDir = path.join(buildRoot, unit.slug);
      if (args.build) {
        process.stdout.write(`[build] ${unit.source} ... `);
        const res = await buildUnit(unit, {
          bin,
          buildRoot,
          repoRoot: REPO_ROOT,
          noCache: args.noCache,
        });
        if (!res.ok) {
          console.log(`FAILED (exit ${res.status})`);
          buildFailures.push({
            unit: unit.slug,
            source: unit.source,
            status: res.status,
            log: String(res.log).slice(-2000),
          });
          continue;
        }
        console.log('ok');
      } else if (!fs.existsSync(outDir)) {
        console.log(`[build] ${unit.source} skipped but no prior build; skip`);
        continue;
      }

      const pages = enumeratePages(unit, outDir, REPO_ROOT);
      if (!pages.length) {
        console.log(`[capture] ${unit.slug}: 0 pages`);
        continue;
      }

      let server;
      try {
        server = await serveDir(outDir);
      } catch (e) {
        buildFailures.push({
          unit: unit.slug,
          source: unit.source,
          status: 'serve-error',
          log: String(e?.message || e),
        });
        continue;
      }

      const cells = [];
      for (const page of pages)
        for (const viewport of viewports)
          for (const theme of themes) cells.push({ page, viewport, theme });

      servers.set(unit.slug, {
        server,
        remaining: cells.length,
        source: unit.source,
        pageCount: pages.length,
      });
      for (const cell of cells)
        cellQueue.push({ cell, serverUrl: server.url, unitSlug: unit.slug });
      work.fire();
    }
    buildsDone = true;
    work.fire();
  }

  // Consumer: pull cells off the shared queue, screenshot, and when a unit's last
  // cell lands, close its server, report its summary, and release backpressure.
  async function worker() {
    for (;;) {
      if (cellQueue.length === 0) {
        if (buildsDone) return;
        await work.wait();
        continue;
      }
      const item = cellQueue.shift();
      const rec = await captureCellWithRetry(
        ensureBrowser,
        item.cell,
        item.serverUrl,
        artifactsRoot,
        args.scale,
      );
      allRecords.push(rec);

      const s = servers.get(item.unitSlug);
      if (s) {
        s.remaining -= 1;
        if (s.remaining === 0) {
          await s.server.close().catch(() => {});
          servers.delete(item.unitSlug);
          const unitRecs = allRecords.filter((r) => r.unit === item.unitSlug);
          const errs = unitRecs.reduce((n, r) => n + r.errorCount, 0);
          console.log(
            `[capture] ${item.unitSlug}: ${s.pageCount} pages, ` +
              `${unitRecs.length} cells, ${errs} console errors`,
          );
          drain.fire();
        }
      }
    }
  }

  try {
    await Promise.all([
      producer(),
      ...Array.from({ length: Math.max(1, args.jobs) }, () => worker()),
    ]);
  } finally {
    for (const { server } of servers.values())
      await server.close().catch(() => {});
    await browser.close().catch(() => {});
  }

  const manifest = {
    generatedAt: new Date().toISOString(),
    repoRoot: REPO_ROOT,
    bin,
    viewports: viewports.map((v) => v.name),
    themes,
    scale: args.scale,
    shard: args.shard || null,
    unitCount: units.length,
    pageCount: new Set(allRecords.map((r) => `${r.unit}${r.route}`)).size,
    cellCount: allRecords.length,
    buildFailures,
    pages: allRecords,
  };
  const manifestName = args.shard
    ? `manifest.shard-${shardI}.json`
    : 'manifest.json';
  const manifestPath = path.join(artifactsRoot, manifestName);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

  const totalErrs = allRecords.reduce((n, r) => n + r.errorCount, 0);
  const totalFailedReq = allRecords.reduce((n, r) => n + r.failedRequests, 0);
  const overflowCells = allRecords.filter((r) => r.horizontalOverflow).length;
  console.log(
    `\n[ui-audit] done: ${manifest.pageCount} pages, ${manifest.cellCount} cells\n` +
      `           console errors: ${totalErrs} | failed requests: ${totalFailedReq} | ` +
      `horizontal-overflow cells: ${overflowCells}\n` +
      `           manifest: ${manifestPath}`,
  );
  if (buildFailures.length) {
    console.log(`           build failures: ${buildFailures.length}`);
  }
  if (args.shard) {
    console.log(
      `           (shard ${shardI}/${shardN}; run \`node capture-run.mjs --merge --out ${args.out}\` after all shards)`,
    );
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
