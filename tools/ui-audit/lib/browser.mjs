// Puppeteer (core) launch + the per-page helpers: theme forcing, the settle
// predicate, and console/network collectors. All findings from the honing pass
// live here.

import puppeteer from 'puppeteer-core';

export const DEFAULT_CHROME =
  process.env.CHROME_PATH || '/usr/bin/google-chrome';

// The three viewports from the project's UI-testing matrix: mobile, laptop
// landscape, and the easy-to-forget narrow-tall portrait band.
export const DEFAULT_VIEWPORTS = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'laptop', width: 1440, height: 900 },
  { name: 'portrait', width: 900, height: 1440 },
];

export const DEFAULT_THEMES = ['light', 'dark'];

export async function launch({ chromePath = DEFAULT_CHROME } = {}) {
  return puppeteer.launch({
    executablePath: chromePath,
    headless: true,
    args: [
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--disable-dev-shm-usage',
      '--disable-gpu',
    ],
  });
}

// Force a theme deterministically. localStorage seeding is the dominant lever
// (it wins over front-matter `theme:` and OS in the pre-paint head script);
// media emulation is belt-and-braces for auto pages. `tali-theme`
// covers single-doc + site/book pages.
export async function forceTheme(page, theme) {
  await page.evaluateOnNewDocument((mode) => {
    try {
      localStorage.setItem('tali-theme', mode);
    } catch {
      /* localStorage may be unavailable on some origins; ignore */
    }
  }, theme);
  await page.emulateMediaFeatures([
    { name: 'prefers-color-scheme', value: theme },
  ]);
}

// Collect console messages, uncaught page errors, failed requests, and >=400
// responses. Attach BEFORE navigation.
export function attachCollectors(page) {
  const consoleMsgs = [];
  const network = [];

  page.on('console', (msg) => {
    consoleMsgs.push({
      type: msg.type(),
      text: msg.text(),
      location: msg.location?.() ?? null,
    });
  });
  page.on('pageerror', (err) => {
    consoleMsgs.push({ type: 'pageerror', text: String(err?.message || err) });
  });
  page.on('requestfailed', (req) => {
    network.push({
      kind: 'requestfailed',
      url: req.url(),
      resourceType: req.resourceType(),
      error: req.failure()?.errorText ?? null,
    });
  });
  page.on('response', (res) => {
    const status = res.status();
    if (status >= 400) {
      network.push({
        kind: 'http-error',
        url: res.url(),
        status,
        resourceType: res.request().resourceType(),
      });
    }
  });

  return { consoleMsgs, network };
}

// Wait until the page has visually settled: SSR math/highlighting need no wait,
// but web fonts, images, mermaid and {js} cells do. On timeout we
// screenshot anyway rather than hang the whole run. The ceiling only bites when
// something never signals ready (an erroring {js} cell that never
// becomes ready); real content settles well under it.
export async function settle(page, { timeout = 6000 } = {}) {
  try {
    await page.waitForFunction(() => document.readyState === 'complete', {
      timeout,
    });
  } catch {
    /* keep going */
  }
  try {
    await page.evaluate(() =>
      Promise.race([
        document.fonts?.ready ?? Promise.resolve(),
        new Promise((r) => setTimeout(r, 3000)),
      ]),
    );
  } catch {
    /* ignore */
  }
  try {
    await page.waitForFunction(
      () => {
        const imgsOk = [...document.images].every((i) => i.complete);
        const mermaidOk = !document.querySelector(
          'pre.mermaid:not([data-processed])',
        );
        const jsOk = [...document.querySelectorAll('.tali-js-cell')].every(
          (c) => {
            // A `{js}` cell can finish having painted nothing (a `//| name:` value
            // publisher returns a Number; an `//| input:` effect returns undefined),
            // so child-count alone reports those cells as never-settled forever.
            // `data-tali-done` is stamped when the cell's run() resolves.
            const s = c.querySelector('script[type="application/tali-js"]');
            if (s && s.hasAttribute('data-tali-done')) return true;
            // Fallback for pages built by a binary predating that signal, and for
            // cells excluded from the run (a dependency cycle paints a diagnostic).
            const o = c.querySelector('.tali-js-out');
            return !o || o.childElementCount > 0;
          },
        );
        return imgsOk && mermaidOk && jsOk;
      },
      { timeout, polling: 100 },
    );
    return true;
  } catch {
    return false; // timed out; caller still screenshots
  }
}

// Cheap DOM heuristics that PRE-FLAG a page for the analysis agents. These are
// hints, never verdicts.
export async function domFlags(page) {
  return page.evaluate(() => {
    const vw = window.innerWidth;
    const docEl = document.documentElement;
    const horizontalOverflow = docEl.scrollWidth > vw + 1;

    const pastRight = [];
    const brokenImages = [];
    const all = document.querySelectorAll('body *');
    let scanned = 0;
    for (const el of all) {
      if (scanned++ > 4000) break; // bound the scan
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || cs.display === 'none') continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) {
        if (el.tagName === 'IMG' && el.complete && el.naturalWidth === 0) {
          brokenImages.push(el.currentSrc || el.src || '(no src)');
        }
        continue;
      }
      // element extends meaningfully past the right edge (not an intentional
      // off-canvas element far to the left)
      if (r.right > vw + 2 && r.left >= 0 && r.left < vw) {
        pastRight.push({
          tag: el.tagName.toLowerCase(),
          cls: (el.className && String(el.className).slice(0, 60)) || '',
          right: Math.round(r.right),
        });
      }
    }
    return {
      title: document.title || '',
      horizontalOverflow,
      scrollWidth: docEl.scrollWidth,
      innerWidth: vw,
      pastRight: pastRight.slice(0, 25),
      pastRightCount: pastRight.length,
      brokenImages: brokenImages.slice(0, 25),
      brokenImageCount: brokenImages.length,
    };
  });
}
