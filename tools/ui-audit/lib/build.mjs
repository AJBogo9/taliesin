// Build a unit to static HTML with `taliesin build --out`, then enumerate the
// emitted pages and map each back to its source .tmd.

import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

function* walkFiles(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const ent of entries) {
    const abs = path.join(dir, ent.name);
    if (ent.isDirectory()) yield* walkFiles(abs);
    else if (ent.isFile()) yield abs;
  }
}

// Run `taliesin build <source> --out <buildRoot>/<slug>`. Works for both a site
// directory and a single .tmd (the latter writes <outDir>/index.html). Returns
// a Promise<{ outDir, ok, status, log }>.
//
// Async (child_process.spawn, not spawnSync) on purpose: the pipeline builds the
// NEXT unit while the browser pool is still screenshotting the PREVIOUS one, and
// a blocking spawnSync would freeze the event loop and stall every capture tab
// for the whole build. Builds stay one-at-a-time (the producer awaits each), so
// there is still only ever a single `taliesin build` / Jupyter kernel running;
// only build<->capture overlap, never build<->build.
export function buildUnit(unit, { bin, buildRoot, repoRoot, noCache = false }) {
  const outDir = path.join(buildRoot, unit.slug);
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir, { recursive: true });

  const env = { ...process.env };
  if (noCache) env.TALIESIN_NO_CACHE = '1';

  return new Promise((resolve) => {
    let log = '';
    let child;
    try {
      child = spawn(bin, ['build', unit.source, '--out', outDir], {
        cwd: repoRoot,
        env,
      });
    } catch (e) {
      resolve({ outDir, ok: false, status: 'spawn-error', log: String(e?.message || e) });
      return;
    }
    child.stdout.on('data', (d) => (log += d));
    child.stderr.on('data', (d) => (log += d));
    child.on('error', (e) => {
      resolve({ outDir, ok: false, status: 'spawn-error', log: log + '\n' + String(e?.message || e) });
    });
    child.on('close', (status) => {
      resolve({ outDir, ok: status === 0, status, log });
    });
  });
}

// Given a built unit output dir, list every HTML page and resolve its source.
// Route is a URL path ("/posts/x/index.html"); sourceFile is repo-relative.
export function enumeratePages(unit, outDir, _repoRoot) {
  const pages = [];
  for (const abs of walkFiles(outDir)) {
    if (!abs.endsWith('.html')) continue;
    const base = path.basename(abs);
    if (base === '404.html') continue;

    const rel = path.relative(outDir, abs).split(path.sep).join('/'); // url-ish
    const routeNoHtml = rel.replace(/\.html$/, '');

    let sourceFile;
    let format;
    if (unit.type === 'standalone') {
      // A standalone builds to a single index.html; its source is the unit file.
      sourceFile = unit.source;
      format = unit.format || 'article';
    } else {
      sourceFile = `${unit.source}/${routeNoHtml}.tmd`;
      format = unit.kind;
    }

    pages.push({
      unit: unit.slug,
      unitSource: unit.source,
      unitType: unit.type,
      route: '/' + rel,
      file: abs,
      sourceFile,
      format,
    });
  }
  // Stable order: shallow routes first, then alphabetical.
  pages.sort((a, b) => {
    const da = a.route.split('/').length;
    const db = b.route.split('/').length;
    return da - db || a.route.localeCompare(b.route);
  });
  return pages;
}
