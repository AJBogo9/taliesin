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

async function captureCell(browser, cell, serverUrl, artifactsRoot, scale = 1) {
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
  let watchdog = null;
  let collectors = { consoleMsgs: [], network: [] };
  let settled = false;
  let flags = null;
  let navError = null;
  let cellError = null;
  try {
    tab = await browser.newPage();
    // Hard wall-clock watchdog: if any op (goto/settle/domFlags/screenshot)
    // wedges on an unresponsive renderer, force-close the tab so its promise
    // rejects instead of hanging this worker forever. The rejection reads as
    // "Target closed" -> CRASH_RE -> one retry on a fresh tab. This is the
    // safety net the old sequential loop lacked (a wedged tab hung the whole run).
    watchdog = setTimeout(() => {
      if (tab) tab.close().catch(() => {});
    }, 60000);
    collectors = attachCollectors(tab);
    await forceTheme(tab, theme);
    await tab.setViewport({
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: scale,
    });
    const url = serverUrl + pageRec.route;
    try {
      // `domcontentloaded` (not `networkidle0`): the built pages are static and
      // locally served, so waiting for 500ms of zero network activity mostly
      // just burns the 30s ceiling on any page with a lingering socket. settle()
      // below is the real readiness gate (fonts, images, mermaid, {js} output,
      // deck-ready), so DOM-ready + settle covers the same ground far faster.
      await tab.goto(url, { waitUntil: 'domcontentloaded', timeout: 20000 });
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
    if (watchdog) clearTimeout(watchdog);
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
const HARD_MS = 90000;

// Synthetic record for a cell we had to abandon (mirrors captureCell's shape so
// the manifest + orchestrator stay uniform). No screenshot: it never rendered.
function abandonedRecord(cell, why) {
  const p = cell.page;
  return {
    unit: p.unit, route: p.route, sourceFile: p.sourceFile, format: p.format,
    viewport: cell.viewport.name, theme: cell.theme,
    settled: false, navError: null, cellError: why, hardTimeout: true,
    screenshot: null, meta: null, title: '',
    errorCount: 0, warnCount: 0, failedRequests: 0,
    horizontalOverflow: false, pastRightCount: 0, brokenImageCount: 0,
    consoleErrors: [], networkFailures: [],
  };
}

// captureCell catches everything and never rejects, so its only failure mode is
// HANGING: on a wedged renderer even tab.close() (a CDP call) can block, so the
// 60s in-cell watchdog can't rescue it. This hard wall-clock race is the real
// backstop -- if a cell hasn't returned in 90s we abandon it and free the worker
// so the pool never deadlocks (the orphaned captureCell + its tab leak until the
// browser is torn down at the end, which is fine). Without this, one wedged tab
// stalls the whole run (workers pile up in ep_poll waiting on dead CDP sockets).
function withHardTimeout(promise, ms, onTimeout) {
  return new Promise((resolve) => {
    let settled = false;
    const t = setTimeout(() => {
      if (!settled) { settled = true; resolve(onTimeout()); }
    }, ms);
    const finish = (v) => { if (!settled) { settled = true; clearTimeout(t); resolve(v); } };
    promise.then(finish, (e) => finish(onTimeout(String(e?.message || e))));
  });
}

// Retry a cell once if the renderer tab CRASHED (fast, fresh tab usually wins).
// A hard-timeout is NOT retried: a wedged page just wedges again and burns
// another 90s. `getBrowser` relaunches a dead browser so a crash that killed the
// whole browser is recovered before the retry.
export async function captureCellWithRetry(getBrowser, cell, serverUrl, artifactsRoot, scale) {
  const attempt = async () => {
    const browser = await getBrowser();
    return withHardTimeout(
      captureCell(browser, cell, serverUrl, artifactsRoot, scale),
      HARD_MS,
      (extra) => abandonedRecord(cell, `hard-timeout: capture exceeded 90s${extra ? ' | ' + extra : ''}`),
    );
  };
  const rec = await attempt();
  if (rec.cellError && CRASH_RE.test(rec.cellError) && !rec.hardTimeout) {
    await new Promise((r) => setTimeout(r, 500));
    const retry = await attempt();
    retry.retried = true;
    return retry;
  }
  return rec;
}
