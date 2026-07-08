#!/usr/bin/env node
// Capture stage CLI: build every (selected) unit to static HTML, serve it, and
// screenshot the full page x viewport x theme matrix with console/network/DOM
// logs. Writes .work/artifacts/ + manifest.json.
//
// Usage:
//   node capture-run.mjs [--only <glob>] [--viewports mobile,laptop,portrait]
//                        [--themes light,dark] [--out <dir>] [--bin <taliesin>]
//                        [--jobs 4] [--no-build] [--no-cache]
//
// --only accepts a glob/substring matched against unit slug or source, e.g.
//   --only 'corpus/deck.tmd'      one standalone
//   --only 'demo-book'            one site project
//   --only 'refs__*'              all refs standalones
// Pass --only multiple times to select several units.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { allUnits, matchesGlob } from './lib/units.mjs';
import { buildUnit, enumeratePages } from './lib/build.mjs';
import { serveDir } from './lib/serve.mjs';
import { launch, DEFAULT_VIEWPORTS, DEFAULT_THEMES } from './lib/browser.mjs';
import { captureUnit } from './lib/capture.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const args = {
    only: [],
    viewports: null,
    themes: null,
    out: path.join(__dirname, '.work'),
    bin: null,
    jobs: 3,
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
    else if (a === '--jobs') args.jobs = parseInt(val(), 10) || 4;
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

async function main() {
  const args = parseArgs(process.argv);
  const bin = resolveBin(args.bin);
  const viewports = args.viewports
    ? DEFAULT_VIEWPORTS.filter((v) => args.viewports.includes(v.name))
    : DEFAULT_VIEWPORTS;
  const themes = args.themes || DEFAULT_THEMES;

  let units = allUnits(REPO_ROOT);
  if (args.only.length) {
    units = units.filter((u) => args.only.some((g) => matchesGlob(u, g)));
  }
  if (!units.length) {
    console.error('no units matched');
    process.exit(1);
  }

  const buildRoot = path.join(args.out, 'build');
  const artifactsRoot = path.join(args.out, 'artifacts');
  fs.mkdirSync(buildRoot, { recursive: true });
  fs.mkdirSync(artifactsRoot, { recursive: true });

  console.log(
    `[ui-audit] bin=${bin}\n[ui-audit] units=${units.length} ` +
      `viewports=${viewports.map((v) => v.name).join(',')} ` +
      `themes=${themes.join(',')} jobs=${args.jobs}`,
  );

  let browser = await launch();
  const allRecords = [];
  const buildFailures = [];

  try {
    for (const unit of units) {
     try {
      // A crashed browser (e.g. OOM under load) must not sink the whole run:
      // relaunch and carry on. Run capture and probe SEPARATELY, not at once.
      if (browser.connected === false) {
        console.log('[ui-audit] browser disconnected; relaunching');
        browser = await launch();
      }
      const outDir = path.join(buildRoot, unit.slug);
      if (args.build) {
        process.stdout.write(`[build] ${unit.source} ... `);
        const res = buildUnit(unit, {
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
            log: res.log.slice(-2000),
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
      const server = await serveDir(outDir);
      try {
        const records = await captureUnit({
          browser,
          unit,
          pages,
          serverUrl: server.url,
          viewports,
          themes,
          artifactsRoot,
          jobs: args.jobs,
        });
        allRecords.push(...records);
        const errs = records.reduce((n, r) => n + r.errorCount, 0);
        console.log(
          `[capture] ${unit.slug}: ${pages.length} pages, ` +
            `${records.length} cells, ${errs} console errors`,
        );
      } finally {
        await server.close();
      }
     } catch (e) {
       console.log(
         `[capture] ${unit.slug}: unit failed: ${String(e?.message || e)}`,
       );
       buildFailures.push({
         unit: unit.slug,
         source: unit.source,
         status: 'capture-error',
         log: String(e?.message || e),
       });
     }
    }
  } finally {
    await browser.close().catch(() => {});
  }

  const manifest = {
    generatedAt: new Date().toISOString(),
    repoRoot: REPO_ROOT,
    bin,
    viewports: viewports.map((v) => v.name),
    themes,
    unitCount: units.length,
    pageCount: new Set(allRecords.map((r) => `${r.unit}${r.route}`)).size,
    cellCount: allRecords.length,
    buildFailures,
    pages: allRecords,
  };
  const manifestPath = path.join(artifactsRoot, 'manifest.json');
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
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
