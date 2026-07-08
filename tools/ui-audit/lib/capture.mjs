// The matrix capture loop for one unit: for each page x viewport x theme, open
// a fresh tab, force the theme, settle, screenshot, and dump console/network/DOM
// flags. Emits one manifest record per cell.

import fs from 'node:fs';
import path from 'node:path';
import {
  attachCollectors,
  forceTheme,
  settle,
  domFlags,
} from './browser.mjs';

function routeSlug(route) {
  return (
    route.replace(/^\//, '').replace(/\.html$/, '').split('/').join('__') ||
    'index'
  );
}

// Bounded-concurrency map.
async function mapPool(items, concurrency, fn) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const i = next++;
      results[i] = await fn(items[i], i);
    }
  }
  const n = Math.max(1, Math.min(concurrency, items.length));
  await Promise.all(Array.from({ length: n }, worker));
  return results;
}

async function captureCell(browser, cell, serverUrl, artifactsRoot) {
  const { page: pageRec, viewport, theme } = cell;
  const dir = path.join(
    artifactsRoot,
    pageRec.unit,
    routeSlug(pageRec.route),
  );
  fs.mkdirSync(dir, { recursive: true });
  const stem = `${viewport.name}__${theme}`;
  const shotRel = path.relative(
    artifactsRoot,
    path.join(dir, `${stem}.png`),
  );
  const metaRel = path.relative(
    artifactsRoot,
    path.join(dir, `${stem}.json`),
  );

  let tab = null;
  let collectors = { consoleMsgs: [], network: [] };
  let settled = false;
  let flags = null;
  let navError = null;
  let cellError = null;
  try {
    tab = await browser.newPage();
    collectors = attachCollectors(tab);
    await forceTheme(tab, theme);
    await tab.setViewport({
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: 1,
    });
    const url = serverUrl + pageRec.route;
    try {
      await tab.goto(url, { waitUntil: 'networkidle0', timeout: 30000 });
    } catch (e) {
      navError = String(e?.message || e);
    }
    settled = await settle(tab);
    try {
      flags = await domFlags(tab);
    } catch (e) {
      flags = { error: String(e?.message || e) };
    }
    try {
      await tab.screenshot({
        path: path.join(artifactsRoot, shotRel),
        fullPage: true,
      });
    } catch (e) {
      // Fall back to a viewport-only shot; some very tall / crashing pages
      // reject a fullPage capture.
      cellError = String(e?.message || e);
      try {
        await tab.screenshot({
          path: path.join(artifactsRoot, shotRel),
          fullPage: false,
        });
        cellError += ' (fell back to viewport screenshot)';
      } catch (e2) {
        cellError += ` | viewport shot also failed: ${String(e2?.message || e2)}`;
      }
    }
  } catch (e) {
    // Never let a single cell abort the whole run (incl. a dead browser at
    // newPage()).
    cellError = (cellError ? cellError + ' | ' : '') + String(e?.message || e);
  } finally {
    if (tab) await tab.close().catch(() => {});
  }

  const errorCount = collectors.consoleMsgs.filter(
    (m) => m.type === 'error' || m.type === 'pageerror',
  ).length;
  const warnCount = collectors.consoleMsgs.filter(
    (m) => m.type === 'warning' || m.type === 'warn',
  ).length;

  const meta = {
    unit: pageRec.unit,
    route: pageRec.route,
    sourceFile: pageRec.sourceFile,
    format: pageRec.format,
    viewport: viewport.name,
    theme,
    settled,
    navError,
    cellError,
    console: collectors.consoleMsgs,
    network: collectors.network,
    domFlags: flags,
  };
  fs.writeFileSync(
    path.join(artifactsRoot, metaRel),
    JSON.stringify(meta, null, 2),
  );

  return {
    unit: pageRec.unit,
    route: pageRec.route,
    sourceFile: pageRec.sourceFile,
    format: pageRec.format,
    viewport: viewport.name,
    theme,
    settled,
    navError: navError || undefined,
    cellError: cellError || undefined,
    screenshot: shotRel,
    meta: metaRel,
    title: flags?.title || '',
    errorCount,
    warnCount,
    failedRequests: collectors.network.length,
    horizontalOverflow: !!flags?.horizontalOverflow,
    pastRightCount: flags?.pastRightCount || 0,
    brokenImageCount: flags?.brokenImageCount || 0,
    // Actual error payloads so the audit can harvest console/network FACTS
    // mechanically (never at an agent's discretion).
    consoleErrors: collectors.consoleMsgs
      .filter((m) => m.type === 'error' || m.type === 'pageerror')
      .slice(0, 20)
      .map((m) => ({ type: m.type, text: (m.text || '').slice(0, 300), url: m.location?.url || null })),
    networkFailures: collectors.network.slice(0, 20),
  };
}

const CRASH_RE = /Target closed|Session closed|crashed|detached/i;

// Retry a cell once if the renderer tab crashed. Under concurrency, large/tall
// pages can crash a tab; a fresh tab (by which time sibling tabs have freed
// resources) almost always succeeds.
async function captureCellWithRetry(browser, cell, serverUrl, artifactsRoot) {
  const rec = await captureCell(browser, cell, serverUrl, artifactsRoot);
  if (rec.cellError && CRASH_RE.test(rec.cellError)) {
    await new Promise((r) => setTimeout(r, 500));
    const retry = await captureCell(browser, cell, serverUrl, artifactsRoot);
    retry.retried = true;
    return retry;
  }
  return rec;
}

export async function captureUnit({
  browser,
  unit,
  pages,
  serverUrl,
  viewports,
  themes,
  artifactsRoot,
  jobs = 3,
}) {
  const cells = [];
  for (const page of pages) {
    for (const viewport of viewports) {
      for (const theme of themes) {
        cells.push({ page, viewport, theme });
      }
    }
  }
  return mapPool(cells, jobs, (cell) =>
    captureCellWithRetry(browser, cell, serverUrl, artifactsRoot),
  );
}
