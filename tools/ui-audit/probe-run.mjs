#!/usr/bin/env node
// Interaction-probe CLI: for each representative target, spawn a live `taliesin
// preview`, drive the relevant feature probes, and record pass/fail. Writes
// .work/probe-results.json.
//
// A live preview (not a static build) is required: click-to-source depends on
// the preview-only window.TALIESIN_DOC + websocket, and search/hover indices are
// served live too.
//
// Usage: node probe-run.mjs [--bin <taliesin>] [--out <dir>] [--only <feature>]

import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { launch } from './lib/browser.mjs';
import {
  probeDeck,
  probeSearch,
  probeLightbox,
  probeHover,
  probeToc,
  probeClickToSource,
  probeCursorSync,
} from './lib/probe.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '../..');

// Each group = one preview server + the probes it can serve. Grouped to minimise
// server spawns. click-to-source lives on the gallery doc (a normal article with
// visible blocks) rather than the deck (present-mode slides are inert).
const GROUPS = [
  {
    target: 'corpus/deck.tmd',
    kind: 'doc',
    tasks: [{ feature: 'deck-nav', nav: '/?tali=present', run: probeDeck }],
  },
  {
    target: 'corpus/media/gallery.tmd',
    kind: 'doc',
    tasks: [
      { feature: 'lightbox', nav: '/', run: probeLightbox },
      { feature: 'click-to-source', nav: '/', run: probeClickToSource, cdp: true },
      { feature: 'cursor-sync', nav: '/', run: probeCursorSync },
    ],
  },
  {
    target: 'docs/internals',
    kind: 'site',
    tasks: [
      { feature: 'search', nav: '/architecture.html', run: (p) => probeSearch(p) },
    ],
  },
  // The right-rail TOC is a NON-BOOK surface: `Site::page_toc` returns false for a book
  // ahead of the page's own `toc:`, so no chapter of `docs/internals` has ever emitted a
  // `#TOC` and the probe that pointed there could only fail. `corpus/analyst` is a plain
  // site with `toc: true` and enough headings on both pages.
  {
    target: 'corpus/analyst',
    kind: 'site',
    tasks: [{ feature: 'toc-scrollspy', nav: '/methods.html', run: probeToc }],
  },
  {
    target: 'corpus/demo-book',
    kind: 'site',
    tasks: [{ feature: 'hover-preview', nav: '/results.html', run: probeHover }],
  },
];

function parseArgs(argv) {
  const args = { bin: null, out: path.join(__dirname, '.work'), only: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--bin') args.bin = argv[++i];
    else if (a === '--out') args.out = path.resolve(argv[++i]);
    else if (a === '--only') args.only = argv[++i];
  }
  return args;
}

function resolveBin(explicit) {
  if (explicit) return explicit;
  if (process.env.TALIESIN_BIN) return process.env.TALIESIN_BIN;
  const release = path.join(REPO_ROOT, 'target/release/taliesin');
  if (fs.existsSync(release)) return release;
  return 'taliesin';
}

function waitForServer(port, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tick = () => {
      const req = http.get(
        { host: '127.0.0.1', port, path: '/', timeout: 1500 },
        (res) => {
          res.resume();
          resolve(true);
        },
      );
      req.on('error', () => {
        if (Date.now() > deadline) reject(new Error('preview never came up'));
        else setTimeout(tick, 300);
      });
      req.on('timeout', () => {
        req.destroy();
        if (Date.now() > deadline) reject(new Error('preview timeout'));
        else setTimeout(tick, 300);
      });
    };
    tick();
  });
}

async function withPreview(bin, target, port, fn) {
  const child = spawn(bin, ['preview', target, String(port)], {
    cwd: REPO_ROOT,
    env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let log = '';
  child.stdout.on('data', (d) => (log += d));
  child.stderr.on('data', (d) => (log += d));
  try {
    await waitForServer(port);
    return await fn(`http://127.0.0.1:${port}`);
  } finally {
    child.kill('SIGINT');
    await new Promise((r) => setTimeout(r, 400));
    if (!child.killed) child.kill('SIGKILL');
  }
}

async function main() {
  const args = parseArgs(process.argv);
  const bin = resolveBin(args.bin);
  fs.mkdirSync(args.out, { recursive: true });

  const browser = await launch();
  const results = [];
  let port = 4390;

  try {
    for (const group of GROUPS) {
      const tasks = args.only
        ? group.tasks.filter((t) => t.feature === args.only)
        : group.tasks;
      if (!tasks.length) continue;

      const myPort = port++;
      process.stdout.write(`[preview] ${group.target} :${myPort} ... `);
      try {
        await withPreview(bin, group.target, myPort, async (base) => {
          console.log('up');
          for (const task of tasks) {
            const page = await browser.newPage();
            await page.setViewport({ width: 1440, height: 900 });
            let cdpFrames = [];
            if (task.cdp) {
              const cdp = await page.target().createCDPSession();
              await cdp.send('Network.enable');
              cdp.on('Network.webSocketFrameSent', (e) =>
                cdpFrames.push(e.response?.payloadData || ''),
              );
            }
            let res;
            try {
              await page.goto(base + task.nav, {
                waitUntil: 'networkidle0',
                timeout: 30000,
              });
              res = task.cdp
                ? await task.run(page, cdpFrames)
                : await task.run(page);
            } catch (e) {
              res = {
                feature: task.feature,
                ok: false,
                assertion: '(navigation/probe threw)',
                detail: { error: String(e?.message || e) },
              };
            }
            res.target = group.target;
            results.push(res);
            console.log(
              `   ${res.ok ? 'PASS' : 'FAIL'}  ${res.feature}` +
                (res.ok ? '' : `  (${JSON.stringify(res.detail)})`),
            );
            await page.close().catch(() => {});
          }
        });
      } catch (e) {
        console.log(`preview failed: ${String(e?.message || e)}`);
        for (const task of tasks) {
          results.push({
            feature: task.feature,
            target: group.target,
            ok: false,
            assertion: '(preview did not start)',
            detail: { error: String(e?.message || e) },
          });
        }
      }
    }
  } finally {
    await browser.close();
  }

  const out = {
    generatedAt: new Date().toISOString(),
    bin,
    results,
    passed: results.filter((r) => r.ok).length,
    failed: results.filter((r) => !r.ok).length,
  };
  const outPath = path.join(args.out, 'probe-results.json');
  fs.writeFileSync(outPath, JSON.stringify(out, null, 2));
  console.log(
    `\n[probe] ${out.passed} passed, ${out.failed} failed -> ${outPath}`,
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
