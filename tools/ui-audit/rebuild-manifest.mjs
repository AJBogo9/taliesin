#!/usr/bin/env node
// Rebuild artifacts/manifest.json from the per-cell meta .json files already on
// disk. capture-run.mjs writes the manifest only at the very END of a run, so a
// run that is killed or deadlocks before finishing forfeits its manifest even
// though every captured cell already wrote its own meta .json. This walks those
// metas and reconstructs a faithful manifest, salvaging the whole capture.
//
// It derives each manifest record from the meta exactly as captureCell() would:
// errorCount/warnCount from the console list, screenshot path from the sibling
// .png (null when the shot failed), overflow/title from domFlags. Output is
// byte-shape-compatible with a normal run's manifest.pages[].
//
// Usage: node rebuild-manifest.mjs [--out <dir>] [--scale N]   (default .work)

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '../..');

let outDir = path.join(__dirname, '.work');
let scale = null;
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a === '--out') outDir = path.resolve(process.argv[++i]);
  else if (a === '--scale') scale = parseFloat(process.argv[++i]) || null;
  else {
    console.error(`unknown flag: ${a}`);
    process.exit(2);
  }
}

const artifactsRoot = path.join(outDir, 'artifacts');
if (!fs.existsSync(artifactsRoot)) {
  console.error(`no artifacts dir: ${artifactsRoot}`);
  process.exit(1);
}

// artifacts/<unit>/<routeSlug>/<viewport>__<theme>.json
function* metaFiles(root) {
  for (const unit of fs.readdirSync(root)) {
    const unitDir = path.join(root, unit);
    if (!fs.statSync(unitDir).isDirectory()) continue;
    for (const slug of fs.readdirSync(unitDir)) {
      const slugDir = path.join(unitDir, slug);
      if (!fs.statSync(slugDir).isDirectory()) continue;
      for (const f of fs.readdirSync(slugDir))
        if (f.endsWith('.json')) yield path.join(slugDir, f);
    }
  }
}

const pages = [];
const viewportSet = new Set();
const themeSet = new Set();
let parseErrors = 0;

for (const metaPath of metaFiles(artifactsRoot)) {
  let m;
  try {
    m = JSON.parse(fs.readFileSync(metaPath, 'utf8'));
  } catch {
    parseErrors++;
    continue;
  }
  const pngPath = metaPath.replace(/\.json$/, '.png');
  const hasPng = fs.existsSync(pngPath);
  const rel = (p) => path.relative(artifactsRoot, p);
  const consoleMsgs = Array.isArray(m.console) ? m.console : [];
  const network = Array.isArray(m.network) ? m.network : [];
  const errs = consoleMsgs.filter(
    (c) => c.type === 'error' || c.type === 'pageerror',
  );
  const warns = consoleMsgs.filter(
    (c) => c.type === 'warning' || c.type === 'warn',
  );
  const df = m.domFlags || {};
  viewportSet.add(m.viewport);
  themeSet.add(m.theme);
  pages.push({
    unit: m.unit,
    route: m.route,
    sourceFile: m.sourceFile,
    format: m.format,
    viewport: m.viewport,
    theme: m.theme,
    settled: !!m.settled,
    navError: m.navError || undefined,
    cellError: m.cellError || undefined,
    screenshot: hasPng ? rel(pngPath) : null,
    meta: rel(metaPath),
    title: df.title || '',
    errorCount: errs.length,
    warnCount: warns.length,
    failedRequests: network.length,
    horizontalOverflow: !!df.horizontalOverflow,
    pastRightCount: df.pastRightCount || 0,
    brokenImageCount: df.brokenImageCount || 0,
    consoleErrors: errs.slice(0, 20).map((c) => ({
      type: c.type,
      text: (c.text || '').slice(0, 300),
      url: c.location?.url || null,
    })),
    networkFailures: network.slice(0, 20),
  });
}

pages.sort((a, b) =>
  `${a.unit}${a.route}${a.viewport}${a.theme}`.localeCompare(
    `${b.unit}${b.route}${b.viewport}${b.theme}`,
  ),
);

const missing = pages.filter((p) => !p.screenshot);
const manifest = {
  generatedAt: new Date().toISOString(),
  rebuiltFromArtifacts: true,
  repoRoot: REPO_ROOT,
  viewports: [...viewportSet],
  themes: [...themeSet],
  scale,
  unitCount: new Set(pages.map((p) => p.unit)).size,
  pageCount: new Set(pages.map((p) => `${p.unit}${p.route}`)).size,
  cellCount: pages.length,
  missingScreenshots: missing.length,
  buildFailures: [],
  pages,
};

const outPath = path.join(artifactsRoot, 'manifest.json');
fs.writeFileSync(outPath, JSON.stringify(manifest, null, 2));
console.log(
  `[rebuild] ${manifest.unitCount} units, ${manifest.pageCount} pages, ` +
    `${manifest.cellCount} cells, ${manifest.missingScreenshots} missing screenshots` +
    (parseErrors ? `, ${parseErrors} unreadable metas` : '') +
    `\n          ${outPath}`,
);
if (missing.length)
  console.log(
    `          missing shots: ` +
      missing
        .map((p) => `${p.unit}${p.route} ${p.viewport}/${p.theme}`)
        .join(', '),
  );
