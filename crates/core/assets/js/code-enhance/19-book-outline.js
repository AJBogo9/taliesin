
// --- book drawer: a per-chapter section outline ------------------------------
// The Chapters drawer is a book's only always-available cross-chapter navigation, and it
// was a flat list of chapter titles: at 12 chapters that is fine, at 60 it is a scroll list
// whose only orientation cue is the scrollbar. Each chapter row gains a disclosure holding
// that chapter's own anchored headings, so "where does this chapter go" is answerable
// without opening the chapter.
//
// The section data is read from the SAME lazily-loaded `search-index.js` the Cmd-K palette
// uses, not from a second build artifact. Measured before choosing: the index is 172 KB raw
// / 60 KB gzipped on the largest dogfood book and an outline-only sidecar would be ~13x
// smaller gzipped — but it is one subresource, already cached per page-load, and Cmd-K
// pulls it anyway. A second artifact would mean a second copy of `search::page_fragment`'s
// ordering recipe, a second whole-project assembly and a second `refresh_*_for_page`
// invalidation, which costs more than the bytes it saves.
//
// Nothing here is emitted server-side: with JS off the drawer stays the flat list it has
// always been, rather than shipping a `<button aria-expanded>` that cannot expand.
function taliInitBookOutline() {
  var btn = document.getElementById('tali-book-drawer-btn');
  var list = document.getElementById('tali-book-chapters');
  if (!btn || !list || btn.dataset.outlineWired) return;
  btn.dataset.outlineWired = '1';
  /** @type {HTMLElement} */
  var target = list;
  // Hydrate on every open, not once: in a live preview an edit can add, rename or remove a
  // heading, and `taliLoadSearchIndex` already re-fetches there (a static build's index is
  // immutable, so its second call is a no-op). `taliBookOutline` compares signatures, so an
  // unchanged book rebuilds nothing and a reader's expanded rows survive.
  btn.addEventListener('click', function () {
    var load = window.taliLoadSearchIndex;
    if (load) load(function () { taliBookOutline(target); });
    else taliBookOutline(target);
  });
}

// Build (or refresh) the section list under every chapter row of `list`.
/** @param {HTMLElement} list */
function taliBookOutline(list) {
  var idx = window.TALIESIN_SEARCH_INDEX;
  if (!idx || !idx.length) return;
  // Index urls are site-root-relative; the drawer's hrefs are relative to THIS page, which
  // sits at an arbitrary depth (and under a mount, at an arbitrary prefix). The search
  // URL's own directory IS the site root, so resolving both against it compares like with
  // like without depending on a separate root global.
  /** @type {URL | null} */
  var base = null;
  try { base = new URL(window.TALIESIN_SEARCH_URL || '.', location.href); } catch (e) { base = null; }
  if (!base) return;
  var root = base;
  /** @param {string} u @param {string | URL} against */
  function abs(u, against) {
    try { return new URL(u, against).href.split('#')[0]; } catch (e) { return ''; }
  }
  // Hydration re-parents chapter links (into `.tali-book-row` below), and moving an element
  // in the DOM blurs it. The drawer focuses the current chapter's link the instant it opens,
  // but this runs ~300ms later behind `taliLoadSearchIndex` — so opening the drawer put
  // focus in the panel and then silently threw it back to `<body>`, which is exactly the
  // "focus stays on .tali-book-body" the 2026-07-26 mobile audit measured (it read the
  // settled state and concluded the trap was never wired; the trap is wired and correct).
  // Remember who had focus and hand it back once the moves are done.
  var hadFocus = document.activeElement;
  /** @type {Record<string, Array<{i: string, t: string, l: number}>>} */
  var byPage = {};
  /** @type {Record<string, number>} */
  var shallowest = {};
  idx.forEach(function (e) {
    if (!e.l || !e.i) return; // `l` 0 is the page-title record; an id-less heading is unlinkable
    var key = abs(e.u || '', root);
    (byPage[key] = byPage[key] || []).push({ i: e.i, t: e.t, l: e.l });
    if (shallowest[key] == null || e.l < shallowest[key]) shallowest[key] = e.l;
  });

  list.querySelectorAll('a.tali-book-chapter').forEach(function (link, n) {
    var href = link.getAttribute('href') || '';
    var secs = byPage[abs(href, location.href)];
    // `closest`, not `parentElement`: after the first hydration the link's parent is the
    // `.tali-book-row` wrapper, so a refresh would otherwise append the section list into
    // the row and lose the wrapper it is meant to sit beside.
    var li = link.closest('li');
    if (!secs || !secs.length || !li) return;
    // Signature = exactly what the rows render from, so a heading edit in the preview
    // rebuilds and an unchanged book does not (which is what keeps expanded rows expanded).
    var sig = secs.map(function (s) { return s.l + ':' + s.i + ':' + s.t; }).join('|');
    var found = li.querySelector(':scope > .tali-book-sections');
    if (found instanceof HTMLElement && found.dataset.sig === sig) return;

    /** @type {HTMLElement} */
    var panel;
    if (found instanceof HTMLElement) {
      panel = found;
    } else {
      panel = document.createElement('ul');
      panel.className = 'tali-book-sections';
      panel.id = 'tali-book-sec-' + n;
      panel.hidden = true;
      li.appendChild(panel);
    }
    var hasRow = li.querySelector(':scope > .tali-book-row');
    if (!(hasRow instanceof HTMLElement)) {
      // The chapter link and its expander share a row; the section list is the row's
      // sibling so it spans the drawer's full width rather than the link's column.
      var wrap = document.createElement('div');
      wrap.className = 'tali-book-row';
      li.insertBefore(wrap, link);
      wrap.appendChild(link);
      hasRow = wrap;
    }
    var row = hasRow;
    var hasToggle = row.querySelector(':scope > .tali-book-expand');
    if (!(hasToggle instanceof HTMLElement)) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'tali-book-expand';
      b.setAttribute('aria-expanded', 'false');
      b.setAttribute('aria-controls', panel.id);
      // The chapter link also carries its prose-length span; a verbatim textContent would
      // announce the expander as "Sections of 1 Installation 431 words". Clone-strip-read,
      // the same trick the caption reader uses, leaves the read-only original intact.
      var label = link.cloneNode(true);
      if (label instanceof Element) {
        label.querySelectorAll('.tali-chap-words').forEach(function (x) { x.remove(); });
      }
      b.setAttribute('aria-label', 'Sections of ' + (label.textContent || '').trim());
      // A static chevron, exactly like the drawer launcher's own icon: no untrusted text
      // reaches innerHTML here (every section title goes through textContent below).
      b.innerHTML =
        "<svg width='11' height='11' viewBox='0 0 16 16' fill='none' stroke='currentColor' " +
        "stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
        "<path d='M6 3l5 5-5 5'/></svg>";
      var body = panel;
      b.addEventListener('click', function () {
        var open = b.getAttribute('aria-expanded') === 'true';
        b.setAttribute('aria-expanded', open ? 'false' : 'true');
        body.hidden = open;
      });
      row.appendChild(b);
      hasToggle = b;
    }
    var toggle = hasToggle;

    panel.textContent = '';
    var top = shallowest[abs(href, location.href)] || 1;
    secs.forEach(function (s) {
      // Indent relative to the PAGE's own shallowest heading, never the absolute level:
      // whether a chapter's sections land on h2, h3 or h4 depends on whether it emits a
      // title block and where it roots, so `l` would step a `###`-rooted chapter's
      // top-level sections three times beside a `##`-rooted chapter's.
      var depth = Math.min(Math.max(s.l - top + 1, 1), 4);
      var item = document.createElement('li');
      var a = document.createElement('a');
      a.className = 'tali-book-section tali-book-sd' + depth;
      a.setAttribute('href', href + '#' + s.i);
      a.textContent = s.t;
      item.appendChild(a);
      panel.appendChild(item);
    });
    panel.dataset.sig = sig;
    // The chapter being read opens without a click: its sections are the ones a reader is
    // most likely to jump between, and it doubles as the "you are here" cue in a long list.
    if (link.matches('[aria-current]') && toggle.getAttribute('aria-expanded') !== 'true') {
      toggle.setAttribute('aria-expanded', 'true');
      panel.hidden = false;
    }
  });
  // Outside the per-chapter signature check above, on purpose: an unchanged book skips its
  // rebuild, and the reader's position is exactly what changed while the drawer was shut.
  taliBookMarkSection(list);
  // Restore focus if a re-parent above dropped it. Guarded three ways: only when focus
  // actually landed on `<body>`/null (never steal it from wherever the reader has since
  // moved it), only for an element still in the document, and `preventScroll` because the
  // drawer is a fixed overlay — a default `.focus()` would scroll the article underneath.
  if (hadFocus instanceof HTMLElement && hadFocus !== document.activeElement &&
      (document.activeElement === document.body || document.activeElement === null) &&
      hadFocus.isConnected) {
    try { hadFocus.focus({ preventScroll: true }); } catch (e) { /* older engines */ }
  }
}

// --- "you are here", one level below the chapter -----------------------------------------
// The drawer already answers "which chapter am I in": the current row carries `aria-current`
// and opens expanded. This answers the question one level down, WHICH SECTION of it, by
// marking the row for the section the reader is actually stopped in. Item 76 removed a book's
// right-rail TOC, so the drawer's expanded chapter is the only surface left that can carry a
// section-level position cue at all.
//
// Deliberately one-shot at open time rather than a scroll listener: the drawer is a modal
// overlay that locks the root scroller (`chrome.rs`'s BOOK_DRAWER_SCRIPT sets
// `documentElement.style.overflow`), so while it is open the reader's position cannot change
// and a spy would be watching for an event that cannot fire. The caller re-runs on every open.
//
// Only the current chapter's panel is marked. The other panels' rows come from the shared
// search index and name headings in OTHER documents, which have no position in this one.
/** @param {HTMLElement} list */
function taliBookMarkSection(list) {
  var cur = list.querySelector('a.tali-book-chapter[aria-current]');
  var li = cur && cur.closest('li');
  var panel = li && li.querySelector(':scope > .tali-book-sections');
  if (!(panel instanceof HTMLElement)) return;
  /** @type {Array<{a: Element, h: HTMLElement}>} */
  var rows = [];
  panel.querySelectorAll('a.tali-book-section').forEach(function (a) {
    var frag = (a.getAttribute('href') || '').split('#')[1] || '';
    var h = frag ? document.getElementById(decodeURIComponent(frag)) : null;
    if (h) rows.push({ a: a, h: h });
  });
  if (!rows.length) return;
  // The same activation line the right rail's spy uses (`toc-spy.js`): the heading's own
  // `scroll-margin-top`, which is exactly where a clicked link lands and which the browser
  // resolves to px under either chrome. Sampled once — `getComputedStyle` flushes style.
  var ln = parseFloat(getComputedStyle(rows[0].h).scrollMarginTop);
  if (isNaN(ln)) ln = 16;
  // Within one viewport of the bottom the last heading can never reach the line, so pin the
  // final row — otherwise a chapter's last section never lights up.
  var atBottom =
    window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2;
  /** @type {{a: Element, h: HTMLElement} | null} */
  var active = null;
  if (atBottom) {
    active = rows[rows.length - 1];
  } else {
    for (var i = 0; i < rows.length; i++) {
      // 1px tolerance, for the device-pixel quantization `toc-spy.js` documents: a heading
      // the reader just landed on can measure a hair BELOW the line and leave the previous
      // row marked.
      if (rows[i].h.getBoundingClientRect().top - ln > 1) break;
      active = rows[i];
    }
  }
  rows.forEach(function (r) {
    var on = r === active;
    r.a.classList.toggle('tali-book-section-active', on);
    // `location`, not `page`: the chapter link in the same list is already
    // `aria-current="page"`, and these are two different kinds of current. Nothing is marked
    // while the reader is still above the first heading, rather than lighting section 1 early.
    if (on) r.a.setAttribute('aria-current', 'location');
    else r.a.removeAttribute('aria-current');
  });
  if (!active) return;
  // Bring the mark into view by scrolling the PANEL, by hand. `scrollIntoView` walks every
  // scrollable ancestor, and the ancestor here is the article whose scroll offset IS the
  // reader's place in the chapter — the one thing a read-only overlay must not move.
  var sc = panel.closest('.tali-book-drawer-panel');
  if (!(sc instanceof HTMLElement)) return;
  var r = active.a.getBoundingClientRect();
  var box = sc.getBoundingClientRect();
  if (r.top < box.top) sc.scrollTop -= box.top - r.top;
  else if (r.bottom > box.bottom) sc.scrollTop += r.bottom - box.bottom;
}
