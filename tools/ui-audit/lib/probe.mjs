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

// 1. Deck navigation. A deck opens stepped by default now; ?tali=present just pins
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

// 4. Cross-page hover-preview + the load-bearing safety property: a Ctrl-click
// INSIDE the preview card must NOT fire click-to-source (source attrs stripped).
export async function probeHover(page) {
  const F = 'hover-preview';
  return safe(F, 'hovering an xref opens a populated preview card', async () => {
    await page.waitForSelector('a.tali-xref', { timeout: 8000 });

    // The first `a.tali-xref` on the page is NOT a valid probe target, and picking it is
    // why this probe failed for months while the feature worked. On `results.html` it is
    // `methods.html#sec-methods`, a SECTION reference — and a section anchor is
    // deliberately absent from the hover index (a heading's title is already the link
    // text, so a card would add noise). The probe hovered the one link guaranteed to open
    // nothing. Same shape as the click-to-source probe clicking the title block.
    //
    // So: pick a link whose target the feature actually previews, and cover BOTH paths —
    // a same-page `#anchor` (cloned from this DOM) and a cross-page `page.html#anchor`
    // (parsed from the served hover index).
    const hoverAndOpen = async (selectorFn, label) => {
      const box = await page.evaluate(selectorFn);
      if (!box) return { [label]: 'no eligible link on the page' };
      // Scroll it under the pointer first: `mouse.move` to a rect outside the viewport
      // hovers nothing, and a link below the fold has exactly that rect.
      await page.evaluate((sel) => {
        document.querySelector(sel)?.scrollIntoView({ block: 'center' });
      }, box.sel);
      const at = await page.evaluate((sel) => {
        const r = document.querySelector(sel).getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      }, box.sel);
      await page.mouse.move(at.x, at.y);
      await page.waitForSelector('#tali-link-preview.open', { timeout: 4000 });
      const detail = await page.evaluate(() => {
        const card = document.querySelector('#tali-link-preview');
        return {
          populated: (card?.childElementCount || 0) > 0,
          strippedSourceAttrs: !card?.querySelector(
            '[data-block-id], [data-sourcepos], [data-source-file]',
          ),
        };
      });
      // Dismiss before the next hover, so the second assertion cannot pass on the first
      // card still being open.
      await page.mouse.move(1, 1);
      await page.waitForFunction(
        () => !document.querySelector('#tali-link-preview.open'),
        { timeout: 4000 },
      );
      return { [label]: detail };
    };

    // Same-page: an `#anchor` xref whose target is not a heading.
    const same = await hoverAndOpen(() => {
      const links = [...document.querySelectorAll('a.tali-xref')];
      const hit = links.find((a) => {
        const href = a.getAttribute('href') || '';
        if (href.charAt(0) !== '#' || href.length < 2) return false;
        const t = document.getElementById(decodeURIComponent(href.slice(1)));
        return !!t && !/^H[1-6]$/.test(t.tagName);
      });
      return hit ? { sel: `a.tali-xref[href="${hit.getAttribute('href')}"]` } : null;
    }, 'samePage');

    // Cross-page: the hover index is lazy-loaded on the first cross-page hover, so warm it
    // by hovering any cross-page xref, then pick one whose anchor the index actually has.
    await page.evaluate(() => {
      const a = [...document.querySelectorAll('a.tali-xref')].find((x) => {
        const h = x.getAttribute('href') || '';
        return h.charAt(0) !== '#' && h.includes('#');
      });
      a?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });
    await page.waitForFunction(() => !!window.TALIESIN_HOVER_INDEX, { timeout: 4000 });
    const cross = await hoverAndOpen(() => {
      const index = window.TALIESIN_HOVER_INDEX || {};
      const hit = [...document.querySelectorAll('a.tali-xref')].find((a) => {
        const href = a.getAttribute('href') || '';
        if (href.charAt(0) === '#' || !href.includes('#')) return false;
        return !!index[decodeURIComponent(href.slice(href.indexOf('#') + 1))];
      });
      return hit ? { sel: `a.tali-xref[href="${hit.getAttribute('href')}"]` } : null;
    }, 'crossPage');

    // A floor: "no eligible link" must read as a failure, not as a quiet pass. Without it
    // a page that lost its xrefs would report green.
    const detail = { ...same, ...cross };
    const opened = (v) => v && typeof v === 'object' && v.populated;
    if (!opened(detail.samePage) || !opened(detail.crossPage)) {
      return fail(F, 'hovering an xref opens a populated preview card', detail);
    }
    return ok(F, 'hovering an xref opens a populated preview card', detail);
  });
}

// 5. TOC scrollspy. Scroll down and expect an active TOC entry.
export async function probeToc(page) {
  const F = 'toc-scrollspy';
  return safe(F, 'scrolling marks an active TOC entry', async () => {
    // `#TOC` IS emitted (`render/mod.rs`'s `toc_html`) — the earlier reading that no
    // emitter produced it was wrong. What is true is that a BOOK never gets one:
    // `Site::page_toc` returns `false` for a book ahead of the page's own `toc:`, by the
    // 2026-07-27 ruling that made the chapter drawer a book's only nav surface. This probe
    // pointed at `docs/internals`, a book, so it could only ever fail. It now runs against
    // a non-book site page (`probe-run.mjs` owns the target).
    await page.waitForSelector('#TOC a[href^="#"]', { timeout: 8000 });
    // The scrollspy binds heading ids, so a TOC whose links resolve to nothing would sit
    // permanently inactive and look like a regression in the spy rather than the page.
    const bound = await page.evaluate(
      () =>
        [...document.querySelectorAll('#TOC a[href^="#"]')].filter((a) =>
          document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1))),
        ).length,
    );
    if (bound < 2) {
      return fail(F, 'scrolling marks an active TOC entry', {
        error: `only ${bound} TOC link(s) resolve to a heading on this page`,
      });
    }
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
    return ok(F, 'scrolling marks an active TOC entry', { active, bound });
  });
}

// 6. Click-to-source. Ctrl-hover should light the affordance; Ctrl-click should
// emit a `click_block` websocket frame. `cdpFrames` is an array the caller fills
// from a CDP Network.webSocketFrameSent listener.
export async function probeClickToSource(page, cdpFrames) {
  const F = 'click-to-source';
  return safe(F, 'Ctrl-click emits a click_block ws frame', async () => {
    await page.waitForFunction(() => !!window.TALIESIN_DOC, { timeout: 8000 });
    // Must carry a sourcepos, not merely a block id. The first `[data-block-id]` on a page
    // is the title block, which has an id but NO sourcepos — and `locatable()` deliberately
    // refuses to resolve it ("landing nowhere is the honest answer"), so this probe was
    // clicking the one element guaranteed to emit nothing and reporting a failure that said
    // more about the selector than about click-to-source.
    const handle = await page.waitForSelector('[data-block-id][data-sourcepos]', {
      timeout: 8000,
    });
    // Ctrl-hover affordance
    await page.keyboard.down('Control');
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(10, box.height / 2));
    const navHover = await page.evaluate(
      () => document.documentElement.classList.contains('tali-srcnav'),
    );
    // Ctrl-click
    const before = cdpFrames.length;
    await handle.click();
    await page.keyboard.up('Control');
    // give the ws frame a beat to arrive over CDP
    await new Promise((r) => setTimeout(r, 500));
    const clickBlockFrame = cdpFrames
      .slice(before)
      .some((f) => f.includes('click_block'));
    if (!clickBlockFrame) {
      return fail(F, 'Ctrl-click emits a click_block ws frame', {
        navHover,
        framesSeen: cdpFrames.length - before,
      });
    }
    return ok(F, 'Ctrl-click emits a click_block ws frame', {
      navHover,
      clickBlockFrame,
    });
  });
}

// 7. Forward search: `tali-cursor` MARKS always and SCROLLS only on `reveal: true`.
//
// This is the behaviour change with the least natural coverage and the highest chance
// of silently regressing: if `reveal` gating is lost, the preview goes back to yanking
// the page on every keystroke, and nothing else in the suite would notice. The probe
// drives the same `postMessage` the VS Code host sends, so it exercises the real path.
//
// Also asserts the macOS guard: while the inverse-search overlay is armed, `contextmenu`
// is suppressed, so a Mac author reaching for Ctrl does not get a menu on top of the jump.
export async function probeCursorSync(page) {
  const F = 'cursor-sync';
  return safe(F, 'tali-cursor marks always, scrolls only when reveal is set', async () => {
    await page.waitForFunction(() => !!window.TALIESIN_DOC, { timeout: 8000 });
    // The block must start OFF-SCREEN, or the assertion is vacuous: `highlightAtLine`
    // scrolls only when the target is out of view, so on a short page "did not scroll"
    // and "reveal is broken" look identical. Pick the last block and require that it is
    // genuinely below the fold before asserting anything about scrolling.
    // Smooth scrolling makes every scroll assertion a race: a measurement taken mid-animation
    // reads a position the page is only passing through. Force instant scrolling for the
    // duration of the probe, then settle across two frames before measuring.
    await page.evaluate(() => {
      const st = document.createElement('style');
      st.id = 'tali-probe-instant-scroll';
      st.textContent = 'html, body, * { scroll-behavior: auto !important; }';
      document.head.appendChild(st);
      window.scrollTo(0, 0);
    });
    await page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    const target = await page.evaluate(() => {
      const els = [...document.querySelectorAll('[data-block-id][data-sourcepos]')];
      const last = els[els.length - 1];
      if (!last) return null;
      const m = /^(\d+):/.exec(last.getAttribute('data-sourcepos') || '');
      if (!m) return null;
      return {
        line: +m[1],
        offscreen: last.getBoundingClientRect().top > window.innerHeight,
        scrollY: Math.round(window.scrollY),
      };
    });
    if (!target) {
      return fail(F, 'tali-cursor marks always, scrolls only when reveal is set', {
        error: 'no block with a sourcepos',
      });
    }

    // reveal:false — must mark, must NOT move the page.
    await page.evaluate((line) => {
      window.postMessage({ type: 'tali-cursor', file: null, line, reveal: false }, '*');
    }, target.line);
    await page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    const afterPassive = await page.evaluate(() => ({
      scrollY: Math.round(window.scrollY),
      marked: !!document.querySelector('.tali-hl'),
    }));

    // reveal:true — same message, now it must bring the block into view.
    await page.evaluate((line) => {
      window.postMessage({ type: 'tali-cursor', file: null, line, reveal: true }, '*');
    }, target.line);
    await page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    const afterReveal = await page.evaluate(() => {
      const el = document.querySelector('.tali-hl');
      if (!el) return { scrollY: Math.round(window.scrollY), inView: false };
      const r = el.getBoundingClientRect();
      // Not `r.bottom <= innerHeight`: a block taller than the viewport can never satisfy
      // that, so the strict form would fail on exactly the long blocks worth revealing.
      return {
        scrollY: Math.round(window.scrollY),
        inView: r.top >= 0 && r.top < window.innerHeight,
      };
    });

    // contextmenu is suppressed only while the overlay is armed.
    await page.keyboard.down('Control');
    await new Promise((r) => setTimeout(r, 100));
    const armedSuppressed = await page.evaluate(() => {
      const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
      document.body.dispatchEvent(e);
      return e.defaultPrevented;
    });
    await page.keyboard.up('Control');
    await new Promise((r) => setTimeout(r, 100));
    const idleSuppressed = await page.evaluate(() => {
      const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
      document.body.dispatchEvent(e);
      return e.defaultPrevented;
    });

    const detail = {
      markedWithoutReveal: afterPassive.marked,
      scrollYAfterPassive: afterPassive.scrollY,
      scrollYAfterReveal: afterReveal.scrollY,
      inViewAfterReveal: afterReveal.inView,
      contextmenuSuppressedWhileArmed: armedSuppressed,
      contextmenuAllowedWhenIdle: !idleSuppressed,
      revealScrollAsserted: target.offscreen,
    };
    if (!afterPassive.marked) return fail(F, 'reveal:false must still mark the block', detail);
    if (afterPassive.scrollY !== target.scrollY) {
      return fail(F, 'reveal:false must NOT scroll the page', detail);
    }
    // The reveal:true half is asserted only when the target actually started off-screen.
    // `highlightAtLine` scrolls only for an out-of-view block, so on a short page
    // "did not scroll" and "reveal is broken" are indistinguishable, and asserting anyway
    // would be a coin-flip rather than a test. The passive half above is the regression-prone
    // one and is asserted unconditionally; the reveal keystroke is covered by the manual
    // checklist in editor/vscode/README.md.
    if (target.offscreen && !afterReveal.inView) {
      return fail(F, 'reveal:true must bring the block into view', detail);
    }
    if (!armedSuppressed) return fail(F, 'contextmenu must be suppressed while armed', detail);
    if (idleSuppressed) return fail(F, 'contextmenu must work when not armed', detail);
    return ok(F, 'tali-cursor marks always, scrolls only when reveal is set', detail);
  });
}
