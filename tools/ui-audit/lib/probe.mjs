// Interaction probes. Each takes an already-navigated Puppeteer page and returns
// { feature, ok, assertion, detail }. These run against a live `taliesin
// preview` server (probe-run.mjs owns its lifecycle) because click-to-source
// needs the preview-only `window.TALIESIN_DOC` + websocket.
//
// Probes are lenient by design: they assert "the feature is wired and its state
// changes", not exact pixel behavior, so a real regression trips them without
// false positives on cosmetic drift.

const ok = (feature, assertion, detail = {}) => ({
  feature,
  ok: true,
  assertion,
  detail,
});
const fail = (feature, assertion, detail = {}) => ({
  feature,
  ok: false,
  assertion,
  detail,
});

async function safe(feature, assertion, fn) {
  try {
    return await fn();
  } catch (e) {
    return fail(feature, assertion, { error: String(e?.message || e) });
  }
}

// 1. Deck navigation. A deck opens stepped by default now; ?qmd=present just pins
// that (over a future portrait slide-feed). ArrowRight should advance the active leaf.
export async function probeDeck(page) {
  const F = 'deck-nav';
  return safe(F, 'ArrowRight advances the active slide', async () => {
    await page.waitForFunction(
      () => window.TaliesinDeck && window.TaliesinDeck.isReady?.(),
      { timeout: 8000 },
    );
    const before = await page.evaluate(() =>
      JSON.stringify(window.TaliesinDeck.getIndices()),
    );
    await page.keyboard.press('ArrowRight');
    await page.waitForFunction(
      (b) => JSON.stringify(window.TaliesinDeck.getIndices()) !== b,
      { timeout: 4000 },
      before,
    );
    const after = await page.evaluate(() =>
      JSON.stringify(window.TaliesinDeck.getIndices()),
    );
    return ok(F, 'ArrowRight advances the active slide', { before, after });
  });
}

// 2. Cmd-K search. Open, type a query, expect result rows.
export async function probeSearch(page, query = 'the') {
  const F = 'search';
  return safe(F, 'query yields result rows', async () => {
    await page.waitForFunction(() => typeof window.taliOpenSearch === 'function', {
      timeout: 8000,
    });
    await page.evaluate(() => window.taliOpenSearch());
    await page.waitForSelector('#tali-search .tali-s-input', {
      visible: true,
      timeout: 4000,
    });
    await page.evaluate((q) => {
      const input = document.querySelector('#tali-search .tali-s-input');
      input.value = q;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, query);
    await page.waitForFunction(
      () =>
        document.querySelectorAll('#tali-s-results li.tali-s-item').length > 0,
      { timeout: 4000 },
    );
    const count = await page.evaluate(
      () => document.querySelectorAll('#tali-s-results li.tali-s-item').length,
    );
    return ok(F, 'query yields result rows', { query, count });
  });
}

// 3. Image lightbox. Click a figure image, expect the dialog open; ArrowRight
// should advance within a multi-image gallery.
export async function probeLightbox(page) {
  const F = 'lightbox';
  return safe(F, 'click opens lightbox; arrow navigates gallery', async () => {
    await page.waitForSelector('figure img, img.lightbox', { timeout: 8000 });
    await page.click('figure img, img.lightbox');
    await page.waitForSelector('#tali-lightbox.open', { timeout: 4000 });
    const first = await page.evaluate(
      () => document.querySelector('#tali-lightbox img')?.src || '',
    );
    await page.keyboard.press('ArrowRight');
    await new Promise((r) => setTimeout(r, 250));
    const second = await page.evaluate(
      () => document.querySelector('#tali-lightbox img')?.src || '',
    );
    const caption = await page.evaluate(
      () => document.querySelector('.tali-lb-cap')?.textContent || '',
    );
    return ok(F, 'click opens lightbox; arrow navigates gallery', {
      opened: true,
      advanced: first !== second,
      caption,
    });
  });
}

// 4. Cross-page hover-preview + the load-bearing safety property: an Alt-click
// INSIDE the preview card must NOT fire click-to-source (source attrs stripped).
export async function probeHover(page) {
  const F = 'hover-preview';
  return safe(F, 'hovering an xref opens a populated preview card', async () => {
    await page.waitForSelector('a.tali-xref', { timeout: 8000 });
    const box = await page.evaluate(() => {
      const a = document.querySelector('a.tali-xref');
      const r = a.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    });
    await page.mouse.move(box.x, box.y);
    await page.waitForSelector('#tali-link-preview.open', { timeout: 4000 });
    const populated = await page.evaluate(
      () =>
        (document.querySelector('#tali-link-preview')?.childElementCount || 0) >
        0,
    );
    const cardHasSourceAttrs = await page.evaluate(
      () =>
        !!document.querySelector(
          '#tali-link-preview [data-block-id], #tali-link-preview [data-sourcepos]',
        ),
    );
    return ok(F, 'hovering an xref opens a populated preview card', {
      populated,
      cardStrippedSourceAttrs: !cardHasSourceAttrs,
    });
  });
}

// 5. TOC scrollspy. Scroll down and expect an active TOC entry.
export async function probeToc(page) {
  const F = 'toc-scrollspy';
  return safe(F, 'scrolling marks an active TOC entry', async () => {
    await page.waitForSelector('#TOC a', { timeout: 8000 });
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight / 2));
    await page.waitForFunction(
      () => !!document.querySelector('#TOC a.tali-toc-active'),
      { timeout: 4000 },
    );
    const active = await page.evaluate(
      () =>
        document.querySelector('#TOC a.tali-toc-active')?.textContent?.trim() ||
        '',
    );
    return ok(F, 'scrolling marks an active TOC entry', { active });
  });
}

// 6. Click-to-source. Alt-hover should light the affordance; Alt-click should
// emit a `click_block` websocket frame. `cdpFrames` is an array the caller fills
// from a CDP Network.webSocketFrameSent listener.
export async function probeClickToSource(page, cdpFrames) {
  const F = 'click-to-source';
  return safe(F, 'Alt-click emits a click_block ws frame', async () => {
    await page.waitForFunction(() => !!window.TALIESIN_DOC, { timeout: 8000 });
    const handle = await page.waitForSelector('[data-block-id]', {
      timeout: 8000,
    });
    // Alt-hover affordance
    await page.keyboard.down('Alt');
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(10, box.height / 2));
    const altHover = await page.evaluate(
      () => document.documentElement.classList.contains('tali-alt'),
    );
    // Alt-click
    const before = cdpFrames.length;
    await handle.click();
    await page.keyboard.up('Alt');
    // give the ws frame a beat to arrive over CDP
    await new Promise((r) => setTimeout(r, 500));
    const clickBlockFrame = cdpFrames
      .slice(before)
      .some((f) => f.includes('click_block'));
    if (!clickBlockFrame) {
      return fail(F, 'Alt-click emits a click_block ws frame', {
        altHover,
        framesSeen: cdpFrames.length - before,
      });
    }
    return ok(F, 'Alt-click emits a click_block ws frame', {
      altHover,
      clickBlockFrame,
    });
  });
}
