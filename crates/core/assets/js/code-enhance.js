
// --- Enhancer registry (the public extension hook) ---------------------------
// An *enhancer* is `fn(root)` that decorates freshly-mounted DOM. An extension's
// JS opts in with `window.qmdEnhancers.register(fn)`; the registered fn then runs
// after every (re)mount in the live preview, on DOMContentLoaded in the static
// build, and once immediately if it registers after the page is already mounted
// (an extension script loaded in `include-after-body`). Enhancers MUST be
// idempotent — guard with a data-attribute — since they re-run on every change.
// The built-in copy-button / lightbox / etc. below (and mermaid, in its own
// mermaid.js) register through the exact same API, so a third-party enhancer is
// indistinguishable from core's.
(function () {
  if (window.qmdEnhancers) return;
  var list = [];
  var mounted = false;
  function run1(fn, root) {
    try { fn(root || document); } catch (e) { console.error('[qmd] enhancer failed', e); }
  }
  window.qmdEnhancers = {
    register: function (fn) {
      if (typeof fn === 'function') {
        list.push(fn);
        if (mounted) run1(fn, document); // late registration: catch up on existing DOM
      }
      return this;
    },
    run: function (root) {
      mounted = true;
      for (var i = 0; i < list.length; i++) run1(list[i], root);
    },
  };
  // The single entry point every caller uses (live client, static build, reveal).
  window.qmdEnhanceCode = function (root) { window.qmdEnhancers.run(root); };
})();

// Shared clipboard helper: navigator.clipboard in a secure context, with a hidden-textarea
// execCommand fallback for insecure contexts (file://, plain-http --host LAN). Never throws;
// calls onOk on success, onFail (optional) on total failure.
function qmdCopyText(text, onOk, onFail) {
  function legacy() {
    try {
      var ta = document.createElement('textarea');
      ta.value = text; ta.setAttribute('readonly', '');
      ta.style.position = 'fixed'; ta.style.top = '0'; ta.style.opacity = '0';
      document.body.appendChild(ta); ta.select();
      var done = document.execCommand('copy'); document.body.removeChild(ta);
      return done;
    } catch (e) { return false; }
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(onOk, function () { if (legacy()) onOk(); else if (onFail) onFail(); });
  } else if (legacy()) { onOk(); }
  else if (onFail) { onFail(); }
}

// Build a W3C Text Fragment URL (#:~:text=) that deep-links to `rawText` on this page. Pure;
// returns null for empty input. A long selection uses the textStart,textEnd range form to keep
// the URL short. encTF escapes the three chars structurally significant in a text directive
// ('-' marks prefix/suffix, ',' separates parts, '&' separates directives) on top of
// encodeURIComponent, so the directive can never break out of itself.
function qmdBuildTextFragmentUrl(rawText) {
  var text = (rawText || '').replace(/\s+/g, ' ').trim();
  if (!text) return null;
  function encTF(s) { return encodeURIComponent(s).replace(/-/g, '%2D').replace(/,/g, '%2C').replace(/&/g, '%26'); }
  var start = text, end = null;
  if (text.length > 300) {
    var words = text.split(' ');
    if (words.length >= 12) { start = words.slice(0, 6).join(' '); end = words.slice(-6).join(' '); }
    else { var cut = text.slice(0, 300), sp = cut.lastIndexOf(' '); start = sp > 0 ? cut.slice(0, sp) : cut; }
  }
  var directive = 'text=' + encTF(start) + (end ? ',' + encTF(end) : '');
  // Preserve any element-id hash, drop any prior text fragment, emit exactly one ':~:'.
  // Concatenate the href by string (assigning u.hash would re-encode '%' to '%25').
  var u = new URL(location.href);
  var id = u.hash.replace(/^#/, '').split(':~:')[0];
  u.hash = '';
  return u.href + '#' + id + ':~:' + directive;
}

// Shared modal focus trap: while a modal is open, confine Tab/Shift+Tab to `container`, mark it
// aria-modal, and (on release) restore focus to the opener IF focus is still inside (a keyboard
// or programmatic close) — not when the user clicked elsewhere. Used by the lightbox + reader
// menu here and, via this global, by the Cmd-K palette in search.js. Returns release().
window.qmdFocusTrap = window.qmdFocusTrap || function (container, initial) {
  var prev = document.activeElement;
  var SEL = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
  container.setAttribute('aria-modal', 'true');
  function focusables() {
    return [].slice.call(container.querySelectorAll(SEL)).filter(function (el) {
      return el.offsetWidth > 0 || el.offsetHeight > 0 || el === document.activeElement;
    });
  }
  function onKey(e) {
    if (e.key !== 'Tab') return;
    var f = focusables();
    if (!f.length) { e.preventDefault(); return; }
    var first = f[0], last = f[f.length - 1], a = document.activeElement;
    if (!container.contains(a)) { e.preventDefault(); first.focus(); return; }
    if (e.shiftKey) { if (a === first) { e.preventDefault(); last.focus(); } }
    else if (a === last) { e.preventDefault(); first.focus(); }
  }
  document.addEventListener('keydown', onKey, true);
  try { (initial || focusables()[0] || container).focus(); } catch (e) {}
  return function () {
    document.removeEventListener('keydown', onKey, true);
    container.removeAttribute('aria-modal');
    if (container.contains(document.activeElement) && prev && prev.focus) {
      try { prev.focus(); } catch (e) {}
    }
  };
};

// --- Built-in enhancers (registered through the same public API) -------------

// Code blocks are highlighted server-side; the client only adds a copy button.
function qmdCopyButtons(root) {
  (root || document).querySelectorAll('pre > code').forEach(function (code) {
    var pre = code.parentElement;
    if (pre.dataset.enhanced) return;
    pre.dataset.enhanced = '1';
    // (Code is highlighted server-side; the client only adds the copy button.)
    // GitHub/Claude-style copy glyph (Octicons copy), swapping to a check on success.
    var copyIcon = '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"></path><path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"></path></svg>';
    var checkIcon = '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L1.22 8.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"></path></svg>';
    var btn = document.createElement('button');
    btn.className = 'qmd-copy';
    btn.type = 'button';
    btn.setAttribute('aria-label', 'Copy code');
    btn.innerHTML = copyIcon;
    btn.addEventListener('click', function () {
      // Secure context → navigator.clipboard; --host LAN / file:// → execCommand fallback.
      qmdCopyText(code.innerText, function () {
        btn.innerHTML = checkIcon;
        btn.classList.add('qmd-copied');
        btn.setAttribute('aria-label', 'Copied');
        setTimeout(function () { btn.innerHTML = copyIcon; btn.classList.remove('qmd-copied'); btn.setAttribute('aria-label', 'Copy code'); }, 1200);
      });
    });
    pre.appendChild(btn);
    // The button is absolutely positioned inside the <pre>, which is the horizontal
    // scroll container, so it would scroll away with the code. Counter-translate it by
    // the scroll offset to keep it pinned to the visible top-right corner.
    pre.addEventListener('scroll', function () {
      btn.style.transform = pre.scrollLeft ? 'translateX(' + pre.scrollLeft + 'px)' : '';
    }, { passive: true });
  });
}

// Register the built-ins through the public API. Lightbox / link-preview set
// themselves up once (document-level), so they ignore `root`.
window.qmdEnhancers.register(qmdCopyButtons);
window.qmdEnhancers.register(function () { qmdInitLightbox(); });
window.qmdEnhancers.register(function () { qmdInitLinkPreview(); });
window.qmdEnhancers.register(function () { qmdInitReaderMenu(); });
window.qmdEnhancers.register(function () { qmdInitReaderPrefs(); });
window.qmdEnhancers.register(function () { qmdInitReadingProgress(); });
window.qmdEnhancers.register(function () { qmdInitHighlights(); });
window.qmdEnhancers.register(function () { qmdInitHighlightIndex(); });
window.qmdEnhancers.register(function () { qmdInitBookmarks(); });
window.qmdEnhancers.register(qmdInitCategoryFilter);

// Native category filter for `listing: { categories: true }`: the server emits a
// chip row (`.qmd-cat-filter`) above the card grid; each card's categories are read
// from its `.qmd-cat[data-cat]` badges. Clicking a chip — or a category tag on a card — toggles it
// (multi-select, OR semantics); an empty `data-cat` ("All") clears the filter.
// Works in the static build and the live preview; idempotent per filter.
function qmdInitCategoryFilter(root) {
  (root || document).querySelectorAll('.qmd-cat-filter').forEach(function (filter) {
    if (filter.dataset.qmdCat) return;
    filter.dataset.qmdCat = '1';
    var wrap = filter.closest('.qmd-listing-wrap');
    var listing = wrap && wrap.querySelector('.qmd-listing');
    if (!listing) return;
    var selected = new Set();
    var catsOf = function (card) {
      // Read the card's own category badges (each holds the exact name in data-cat),
      // so a category name containing a comma still matches (a delimited attribute
      // would mis-split it).
      return [...card.querySelectorAll('.qmd-cat[data-cat]')].map(function (b) {
        return b.getAttribute('data-cat');
      });
    };
    var apply = function () {
      listing.querySelectorAll('.qmd-card').forEach(function (card) {
        var show = selected.size === 0 || catsOf(card).some(function (c) { return selected.has(c); });
        card.style.display = show ? '' : 'none';
      });
      filter.querySelectorAll('.qmd-cat-chip').forEach(function (chip) {
        var c = chip.getAttribute('data-cat');
        chip.classList.toggle('qmd-cat-active', c === '' ? selected.size === 0 : selected.has(c));
      });
      listing.querySelectorAll('.qmd-cat[data-cat]').forEach(function (tag) {
        tag.classList.toggle('qmd-cat-on', selected.has(tag.getAttribute('data-cat')));
      });
    };
    var toggle = function (cat) {
      if (cat === '') selected.clear();
      else if (selected.has(cat)) selected.delete(cat);
      else selected.add(cat);
      apply();
    };
    filter.addEventListener('click', function (e) {
      var chip = e.target.closest('.qmd-cat-chip');
      if (chip) toggle(chip.getAttribute('data-cat') || '');
    });
    // A category tag on a card toggles its filter instead of opening the post.
    listing.addEventListener('click', function (e) {
      var tag = e.target.closest('.qmd-cat[data-cat]');
      if (!tag) return;
      e.preventDefault();
      e.stopPropagation();
      toggle(tag.getAttribute('data-cat'));
    });
    apply();
  });
}

// Full-screen viewer for figure images AND mermaid diagrams. Set up once; uses
// event delegation in the capture phase so a click opens the lightbox WITHOUT
// triggering the block-level click/double-click handlers (highlight,
// click-to-source). Images are shown via <img>; mermaid SVGs are cloned live
// (so <foreignObject> labels keep rendering, which an <img> would drop). Modifier
// clicks pass through (new tab, reveal alt-zoom). Dismiss: backdrop, Esc, or x.
function qmdInitLightbox() {
  if (window.__qmdLightbox) return;
  window.__qmdLightbox = true;

  var style = document.createElement('style');
  style.textContent =
    'figure img,img.lightbox,pre.mermaid,.qmd-video video{cursor:zoom-in}' +
    '#qmd-lightbox{position:fixed;inset:0;z-index:2147483000;display:none;flex-direction:column;' +
    'align-items:center;justify-content:center;gap:.9rem;padding:2rem;box-sizing:border-box;' +
    'background:rgba(10,12,16,.9);cursor:zoom-out;opacity:0;transition:opacity .15s ease}' +
    '#qmd-lightbox.open{display:flex;opacity:1}' +
    '#qmd-lightbox img{max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'background:var(--qmd-bg,#fff);border-radius:4px;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#qmd-lightbox video{display:none;max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'border-radius:6px;background:#000;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#qmd-lightbox .qmd-lb-svg{display:none;width:92vw;max-width:1400px;max-height:86vh;overflow:auto;' +
    'cursor:default;background:var(--qmd-bg,#fff);border-radius:4px;padding:1.2rem;box-sizing:border-box;' +
    'box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#qmd-lightbox .qmd-lb-svg svg{display:block;width:100%;height:auto;max-width:100%}' +
    '#qmd-lightbox .qmd-lb-cap{color:#e8e8e8;font:14px ui-sans-serif,system-ui,sans-serif;' +
    'text-align:center;max-width:93vw}' +
    '#qmd-lightbox .qmd-lb-cap:empty{display:none}' +
    '#qmd-lightbox .qmd-lb-close{position:fixed;top:.6rem;right:1rem;color:#fff;background:none;' +
    'border:0;font-size:2.2rem;line-height:1;cursor:pointer;opacity:.75}' +
    '#qmd-lightbox .qmd-lb-close:hover{opacity:1}';
  document.head.appendChild(style);

  var box = document.createElement('div');
  box.id = 'qmd-lightbox';
  box.setAttribute('role', 'dialog');
  box.innerHTML = '<button class="qmd-lb-close" aria-label="Close">×</button>' +
    '<img alt=""><video class="qmd-lb-video" muted loop playsinline></video>' +
    '<div class="qmd-lb-svg"></div><div class="qmd-lb-cap"></div>';
  document.body.appendChild(box);
  var lbImg = box.querySelector('img');
  var lbVideo = box.querySelector('.qmd-lb-video');
  var lbSvg = box.querySelector('.qmd-lb-svg');
  var lbCap = box.querySelector('.qmd-lb-cap');
  var gallery = [], gIdx = -1; // the page's zoomable images, for ←/→ navigation
  var lbRelease = null;        // active focus-trap release while the lightbox is open

  // Open the box (add the class, lock scroll, trap focus on the close button once).
  function markOpen() {
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden';
    if (!lbRelease && window.qmdFocusTrap) lbRelease = window.qmdFocusTrap(box, box.querySelector('.qmd-lb-close'));
  }

  function hideAll() {
    lbImg.style.display = 'none'; lbImg.removeAttribute('src');
    lbVideo.style.display = 'none';
    try { lbVideo.pause(); } catch (e) {}
    lbVideo.removeAttribute('src');
    lbSvg.style.display = 'none'; lbSvg.innerHTML = '';
  }
  // Show gallery[i] (wrapping) with its caption + an (n / N) counter for multi-image sets.
  function showImageAt(i) {
    if (!gallery.length) return;
    gIdx = (i + gallery.length) % gallery.length;
    var img = gallery[gIdx];
    hideAll();
    lbImg.style.display = '';
    lbImg.src = img.currentSrc || img.src;
    lbImg.alt = img.alt || '';
    var fig = img.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    var cap = fc ? fc.textContent : (img.alt || '');
    if (gallery.length > 1) cap = (cap ? cap + '  ' : '') + '(' + (gIdx + 1) + ' / ' + gallery.length + ')';
    lbCap.textContent = cap;
    markOpen();
  }
  // Open the clicked image, building the page's gallery so ←/→ can step between images.
  function openImg(srcImg) {
    gallery = [].slice.call(document.querySelectorAll('figure img, img.lightbox'));
    var i = gallery.indexOf(srcImg);
    if (i < 0) { gallery = [srcImg]; i = 0; }
    showImageAt(i);
  }
  function openMermaid(pre) {
    var svg = pre.querySelector('svg');
    if (!svg) return; // not rendered yet
    hideAll();
    var clone = svg.cloneNode(true);
    clone.removeAttribute('width'); clone.removeAttribute('height');
    clone.style.maxWidth = 'none';
    lbSvg.appendChild(clone);
    lbSvg.style.display = 'block';
    // Show the figure's caption in the zoom too (empty -> hidden by CSS).
    var fig = pre.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : '';
    markOpen();
  }
  // A `{{< video >}}` screencast: play an enlarged copy (the clicked element is the
  // theme-visible variant; the hidden one is display:none and not clickable).
  function openVideo(vid) {
    hideAll();
    lbVideo.style.display = 'block'; // CSS defaults it to none; need an explicit value
    lbVideo.src = vid.currentSrc || vid.src;
    var p = lbVideo.play(); if (p && p.catch) p.catch(function () {});
    var fig = vid.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : '';
    markOpen();
  }
  function close() {
    box.classList.remove('open');
    document.documentElement.style.overflow = ''; // restore page scroll
    hideAll();
    gallery = []; gIdx = -1;
    if (lbRelease) { lbRelease(); lbRelease = null; }
  }

  var unmodified = function (e) {
    return !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
  };
  document.addEventListener('click', function (e) {
    if (!e.target.closest) return;
    var img = e.target.closest('figure img, img.lightbox'), vid;
    if (img && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openImg(img);
    } else if ((vid = e.target.closest('.qmd-video video')) && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openVideo(vid);
    } else {
      var pre = e.target.closest('pre.mermaid');
      if (pre && pre.querySelector('svg') && unmodified(e)) {
        e.preventDefault(); e.stopPropagation(); openMermaid(pre);
      }
    }
  }, true);
  // Keep a double-click on a figure/diagram/video from reaching click-to-source.
  document.addEventListener('dblclick', function (e) {
    if (e.target.closest && e.target.closest('figure img, img.lightbox, pre.mermaid, .qmd-video video')) {
      e.preventDefault(); e.stopPropagation();
    }
  }, true);
  box.addEventListener('click', function (e) {
    if (e.target !== lbImg && e.target !== lbVideo && !lbSvg.contains(e.target)) close();
  });
  document.addEventListener('keydown', function (e) {
    if (!box.classList.contains('open')) return;
    if (e.key === 'Escape') { close(); return; }
    // ←/→ step the image gallery (only while an image, not a video/diagram, is shown).
    if (gallery.length > 1 && lbImg.style.display !== 'none') {
      if (e.key === 'ArrowRight') { e.preventDefault(); showImageAt(gIdx + 1); }
      else if (e.key === 'ArrowLeft') { e.preventDefault(); showImageAt(gIdx - 1); }
    }
  });
}

// Quarto-style hover preview for internal links: hovering a citation, a cross
// reference, or a section link pops up a small card previewing its target (the
// reference entry, the figure + caption, the equation, the section heading + its
// first lines). Server-rendered, so the clone needs no re-running (math is already
// KaTeX HTML). Set up once via event delegation, so it survives block swaps;
// table-of-contents links are skipped (navigational, not worth a popup).
function qmdInitLinkPreview() {
  if (window.__qmdLinkPreview) return;
  window.__qmdLinkPreview = true;

  var style = document.createElement('style');
  style.textContent =
    '#qmd-link-preview{position:fixed;z-index:2147482000;max-width:min(440px,90vw);max-height:50vh;' +
    'overflow:auto;background:var(--qmd-bg,#fff);color:var(--qmd-fg,#111);' +
    'border:1px solid var(--qmd-border,#e0e0e0);border-radius:8px;box-shadow:0 6px 30px rgba(0,0,0,.22);' +
    'padding:.7rem .9rem;font-size:.9rem;line-height:1.45;opacity:0;transform:translateY(3px);' +
    'transition:opacity .12s ease,transform .12s ease;pointer-events:none;visibility:hidden;}' +
    '#qmd-link-preview.open{opacity:1;transform:none;pointer-events:auto;visibility:visible;}' +
    '#qmd-link-preview > :first-child{margin-top:0;}#qmd-link-preview > :last-child{margin-bottom:0;}' +
    '#qmd-link-preview img{max-width:100%;height:auto;}#qmd-link-preview figure{margin:0;}' +
    '#qmd-link-preview .qmd-lp-head{font-weight:600;}';
  document.head.appendChild(style);

  var card = document.createElement('div');
  card.id = 'qmd-link-preview';
  card.setAttribute('role', 'tooltip');
  document.body.appendChild(card);

  var showTimer = null, hideTimer = null;

  function eligible(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#qmd-link-preview');
  }
  // Build the preview body for a target element. A heading shows itself plus the
  // following block(s) up to the next heading; anything else is cloned whole.
  function buildPreview(target) {
    if (/^H[1-6]$/.test(target.tagName)) {
      var frag = document.createElement('div');
      var head = document.createElement('div');
      head.className = 'qmd-lp-head';
      head.textContent = target.textContent;
      frag.appendChild(head);
      var n = target.nextElementSibling, added = 0;
      while (n && added < 2 && !/^H[1-6]$/.test(n.tagName) && !n.id) {
        frag.appendChild(n.cloneNode(true));
        added++; n = n.nextElementSibling;
      }
      return frag;
    }
    return target.cloneNode(true);
  }
  function place(link) {
    var r = link.getBoundingClientRect();
    var cw = card.offsetWidth, ch = card.offsetHeight;
    var left = Math.min(Math.max(8, r.left), window.innerWidth - cw - 8);
    var top = r.top - ch - 8;             // prefer above the link
    if (top < 8) top = r.bottom + 8;      // flip below when there is no room
    card.style.left = left + 'px';
    card.style.top = Math.max(8, top) + 'px';
  }
  function show(link) {
    var id = decodeURIComponent((link.getAttribute('href') || '').slice(1));
    var target = id && document.getElementById(id);
    if (!target) return;
    var body = buildPreview(target);
    if (!body || !body.textContent.trim()) return;
    card.innerHTML = '';
    card.appendChild(body);
    card.classList.add('open');
    place(link);
  }
  function scheduleShow(link) {
    clearTimeout(hideTimer); clearTimeout(showTimer);
    showTimer = setTimeout(function () { show(link); }, 140);
  }
  function hide() { clearTimeout(showTimer); card.classList.remove('open'); }
  function scheduleHide() { clearTimeout(hideTimer); hideTimer = setTimeout(hide, 160); }

  document.addEventListener('mouseover', function (e) {
    var a = e.target.closest && e.target.closest("a[href^='#']");
    if (eligible(a)) scheduleShow(a);
  });
  document.addEventListener('mouseout', function (e) {
    var a = e.target.closest && e.target.closest("a[href^='#']");
    if (a && eligible(a)) {
      var to = e.relatedTarget;
      if (to && to.closest && to.closest('#qmd-link-preview')) return; // moving into the card
      scheduleHide();
    }
  });
  card.addEventListener('mouseenter', function () { clearTimeout(hideTimer); });
  card.addEventListener('mouseleave', scheduleHide);
  window.addEventListener('scroll', hide, true);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') hide(); });
}

// Reader menu: one launcher ("Aa", bottom-right) opening a single menu that the reader
// features mount their sections into (Reading, Display, Highlights) via
// window.qmdReaderMenu.addSection(title, node, onOpen). Consolidates what used to be three
// separate floating controls. Reader-side, read-only. Skipped on decks. Built once.
function qmdInitReaderMenu() {
  if (window.qmdReaderMenu) return;
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome

  var launcher = document.createElement('button');
  launcher.type = 'button';
  launcher.className = 'qmd-rmenu-toggle';
  launcher.textContent = 'Aa';
  launcher.setAttribute('aria-label', 'Reader menu');
  launcher.setAttribute('aria-haspopup', 'dialog');
  launcher.setAttribute('aria-expanded', 'false');

  var panel = document.createElement('div');
  panel.className = 'qmd-rmenu-panel';
  panel.setAttribute('role', 'dialog');
  panel.setAttribute('aria-label', 'Reader');
  panel.hidden = true;

  document.body.appendChild(launcher);
  document.body.appendChild(panel);

  // The reader menu is a light-dismiss POPOVER, not a modal (it doesn't cover/inert the page),
  // so it deliberately does NOT use qmdFocusTrap: aria-modal would mislead a screen reader, and
  // trapping/focus-restore fights the jump buttons + outside-click dismissal. aria-expanded on
  // the launcher + Esc-to-close (returning focus to the launcher) + click-away is the right shape.
  var sections = [];
  function openMenu() {
    panel.hidden = false; launcher.setAttribute('aria-expanded', 'true');
    sections.forEach(function (s) { if (s.onOpen) s.onOpen(); });
  }
  function closeMenu() { panel.hidden = true; launcher.setAttribute('aria-expanded', 'false'); }
  launcher.addEventListener('click', function (e) { e.stopPropagation(); if (panel.hidden) openMenu(); else closeMenu(); });
  document.addEventListener('click', function (e) {
    if (!panel.hidden && !panel.contains(e.target) && e.target !== launcher) closeMenu();
  });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape' && !panel.hidden) { closeMenu(); launcher.focus(); } });

  // Public API: each reader feature adds its own section and an optional refresh hook
  // (called when the menu opens). Returns a handle to show/hide the section.
  window.qmdReaderMenu = {
    close: closeMenu,
    addSection: function (title, node, onOpen) {
      var wrap = document.createElement('section');
      wrap.className = 'qmd-rmenu-section';
      if (title) { var h = document.createElement('h2'); h.textContent = title; wrap.appendChild(h); }
      wrap.appendChild(node);
      panel.appendChild(wrap);
      sections.push({ wrap: wrap, onOpen: onOpen });
      if (onOpen) onOpen();
      return { setVisible: function (v) { wrap.hidden = !v; } };
    }
  };
}

// Reader preferences: a reader-local text size / reading width / theme picker, mounted as
// the "Display" section of the reader menu. State lives in the reader's own localStorage and
// is applied before paint by the pre-paint head script (qmdSetTheme / qmdSetReaderPref /
// qmdResetReader in theme.rs), so this enhancer is only the UI. Read-only. Skipped on decks.
function qmdInitReaderPrefs() {
  if (window.__qmdReaderPrefs) return;
  if (!window.qmdSetReaderPref || !window.qmdReaderMenu) return; // need the pre-paint API + the menu host
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome
  window.__qmdReaderPrefs = true;

  var THEMES = [['light', 'Light'], ['dark', 'Dark'], ['sepia', 'Sepia']];
  var SIZES = [['0.9', 'small'], ['1', 'normal'], ['1.15', 'large'], ['1.3', 'x-large']];
  var WIDTHS = [['38rem', 'Narrow'], ['', 'Normal'], ['58rem', 'Wide']];
  var SIZE_FS = { '0.9': '.78rem', '1': '.95rem', '1.15': '1.15rem', '1.3': '1.4rem' };

  function curTheme() { return (window.qmdGetThemePref && window.qmdGetThemePref()) || 'light'; }
  function curSize() { return window.qmdGetReaderPref('scale') || '1'; }
  function curWidth() { return window.qmdGetReaderPref('width') || ''; }

  // One segmented control row. `labelFn(btn, opt)` customizes a button (else opt[1] text).
  function seg(title, options, getCur, onPick, labelFn) {
    var row = document.createElement('div');
    row.className = 'qmd-reader-row';
    var label = document.createElement('span');
    label.textContent = title;
    var group = document.createElement('div');
    group.className = 'qmd-reader-seg';
    group.setAttribute('role', 'group');
    group.setAttribute('aria-label', title);
    var buttons = [];
    options.forEach(function (opt) {
      var b = document.createElement('button');
      b.type = 'button';
      if (labelFn) labelFn(b, opt); else b.textContent = opt[1];
      b.addEventListener('click', function () { onPick(opt[0]); });
      group.appendChild(b);
      buttons.push(b);
    });
    function sync() {
      var cur = getCur();
      buttons.forEach(function (b, i) {
        b.setAttribute('aria-pressed', options[i][0] === cur ? 'true' : 'false');
      });
    }
    row.appendChild(label);
    row.appendChild(group);
    return { row: row, sync: sync };
  }

  var body = document.createElement('div');
  var themeSeg = seg('Theme', THEMES, curTheme, function (v) { window.qmdSetTheme(v); });
  var sizeSeg = seg('Text size', SIZES, curSize,
    function (v) { window.qmdSetReaderPref('scale', v === '1' ? null : v); },
    function (b, opt) { b.textContent = 'A'; b.style.fontSize = SIZE_FS[opt[0]] || '.95rem';
      b.setAttribute('aria-label', opt[1] + ' text'); });
  var widthSeg = seg('Width', WIDTHS, curWidth,
    function (v) { window.qmdSetReaderPref('width', v || null); });
  body.appendChild(themeSeg.row);
  body.appendChild(sizeSeg.row);
  body.appendChild(widthSeg.row);

  var reset = document.createElement('button');
  reset.className = 'qmd-reader-reset';
  reset.type = 'button';
  reset.textContent = 'Reset to defaults';
  reset.addEventListener('click', function () { if (window.qmdResetReader) window.qmdResetReader(); });
  body.appendChild(reset);

  function syncAll() { themeSeg.sync(); sizeSeg.sync(); widthSeg.sync(); }
  window.addEventListener('qmd:themechange', syncAll);
  window.addEventListener('qmd:readerchange', syncAll);
  window.qmdReaderMenu.addSection('Display', body, syncAll);
}

// Reading progress + resume: a thin top progress bar tied to scroll, a "N min left"
// estimate (prose only, code/math excluded), and a block-id-anchored resume position
// (reader-local, exact, survives reflow). Reader-side + read-only: derives from the live
// DOM and the reader's own localStorage; never writes the author's source. Skipped on
// decks. Idempotent (document-level, builds once).
function qmdInitReadingProgress() {
  if (window.__qmdProgress) return;
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome
  window.__qmdProgress = true;

  // Top-level content blocks (a [data-block-id] not nested inside another block).
  function contentBlocks() {
    return [].slice.call(document.querySelectorAll('[data-block-id]')).filter(function (el) {
      return !el.parentElement || !el.parentElement.closest('[data-block-id]');
    });
  }

  // Prose word count (code + math excluded), computed once / on block-set change.
  var totalMin = 1, counted = -1;
  function countWords() {
    var blocks = contentBlocks();
    if (blocks.length === counted) return;
    counted = blocks.length;
    var words = 0;
    blocks.forEach(function (el) {
      var clone = el.cloneNode(true);
      [].slice.call(clone.querySelectorAll('pre, code, .katex')).forEach(function (n) { n.remove(); });
      var m = (clone.textContent || '').match(/[^\s]+/g);
      if (m) words += m.length;
    });
    totalMin = Math.max(1, Math.round(words / 200));
  }

  var bar = document.createElement('div');
  bar.className = 'qmd-readbar';
  bar.setAttribute('aria-hidden', 'true');
  var fill = document.createElement('div');
  fill.className = 'qmd-readbar-fill';
  bar.appendChild(fill);
  document.body.appendChild(bar);

  // The "N min left" readout lives in the reader menu's "Reading" section (registered at
  // the end, once the word count is known); the bar itself stays ambient at the top.
  var readout = document.createElement('div');
  readout.className = 'qmd-rmenu-readout';
  function updateReadout() {
    var f = frac(), left = Math.ceil(totalMin * (1 - f));
    readout.textContent = (left > 0 ? '~' + left + ' min left' : 'Finished') + ' · ' + Math.round(f * 100) + '% read';
  }

  function frac() {
    var h = document.documentElement;
    var max = (h.scrollHeight || document.body.scrollHeight) - window.innerHeight;
    if (max <= 0) return 0;
    // window.scrollY is 0 at the top; `|| h.scrollTop` would wrongly treat 0 as falsy.
    var y = window.pageYOffset != null ? window.pageYOffset : h.scrollTop;
    return Math.min(1, Math.max(0, y / max));
  }
  var ticking = false;
  function render() {
    ticking = false;
    var f = frac();
    fill.style.width = (f * 100).toFixed(2) + '%';
  }
  function schedule() { if (!ticking) { ticking = true; requestAnimationFrame(render); } }

  // Resume position (block-id anchored), reader-local, keyed by page path.
  var KEY = 'qmd-pos:' + location.pathname;
  function topBlockId() {
    var blocks = contentBlocks();
    for (var i = 0; i < blocks.length; i++) {
      if (blocks[i].getBoundingClientRect().top >= -4) return blocks[i].getAttribute('data-block-id');
    }
    return blocks.length ? blocks[blocks.length - 1].getAttribute('data-block-id') : null;
  }
  var saveTimer = null;
  function saveSoon() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(function () {
      var f = frac(), id = topBlockId();
      try {
        if (f <= 0.02 || !id) localStorage.removeItem(KEY);
        else localStorage.setItem(KEY, f.toFixed(3) + '|' + id);
      } catch (e) {}
    }, 500);
  }

  var resumeEl = null, resumeArmed = false;
  function dismissResume() { if (resumeEl) { resumeEl.remove(); resumeEl = null; } }
  function maybeShowResume() {
    var raw = null;
    try { raw = localStorage.getItem(KEY); } catch (e) {}
    if (!raw) return;
    var parts = raw.split('|'), f = parseFloat(parts[0]), id = parts[1];
    if (!(f > 0.04) || !id) return;
    var sel = '[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]';
    var target = document.querySelector(sel);
    if (!target || Math.abs(frac() - f) < 0.03) return; // missing or already roughly there
    resumeEl = document.createElement('div');
    resumeEl.className = 'qmd-resume';
    var go = document.createElement('button');
    go.type = 'button'; go.className = 'qmd-resume-go';
    go.textContent = 'Resume reading · ' + Math.round(f * 100) + '% →';
    go.addEventListener('click', function () {
      target.scrollIntoView({ block: 'start', behavior: 'smooth' }); dismissResume();
    });
    var x = document.createElement('button');
    x.type = 'button'; x.className = 'qmd-resume-x';
    x.setAttribute('aria-label', 'Dismiss'); x.textContent = '×';
    x.addEventListener('click', dismissResume);
    resumeEl.appendChild(go); resumeEl.appendChild(x);
    document.body.appendChild(resumeEl);
    resumeArmed = false;
    setTimeout(function () { dismissResume(); }, 8000);
  }

  function onScroll() {
    schedule();
    saveSoon();
    // Dismiss the resume pill on the reader's own scroll (not the first programmatic tick).
    if (resumeEl) { if (resumeArmed) dismissResume(); else resumeArmed = true; }
  }

  countWords();
  render();
  if (window.qmdReaderMenu) window.qmdReaderMenu.addSection('Reading', readout, updateReadout);
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', schedule, { passive: true });
  window.addEventListener('qmd:readerchange', schedule);
  maybeShowResume();
}

// Reader highlights: select prose, click "Highlight", and the passage is marked. The
// highlight is the reader's own, stored in their localStorage anchored to the block's
// content-hash data-block-id + character offsets within the block's HIGHLIGHTABLE text
// (text nodes, skipping .katex/pre/code so KaTeX's duplicated MathML text and code's
// syntax spans don't corrupt offsets). Re-applied on every mount; exact and survives a
// re-render. Reader-side + read-only: never writes the author's source, never changes a
// block id/sourcepos. Skipped on decks.
function qmdInitHighlights() {
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome
  var KEY = 'qmd-hl:' + location.pathname;

  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function dispatch() { try { window.dispatchEvent(new CustomEvent('qmd:hlchange')); } catch (e) {} }

  // A text node is non-highlightable if it sits inside math/code within the block.
  function skip(node, block) {
    var p = node.parentNode;
    while (p && p !== block) {
      if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
          (p.classList && p.classList.contains('katex')))) return true;
      p = p.parentNode;
    }
    return false;
  }
  function textNodes(block) {
    var out = [], w = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) { if (!skip(n, block)) out.push(n); }
    return out;
  }
  function blockOf(node) {
    var el = node.nodeType === 1 ? node : node.parentNode;
    return el && el.closest ? el.closest('[data-block-id]') : null;
  }
  // Character offset of (node, nodeOffset) within the block's highlightable text, or -1.
  function offsetOf(block, node, nodeOffset) {
    var nodes = textNodes(block), pos = 0;
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i] === node) return pos + nodeOffset;
      pos += nodes[i].nodeValue.length;
    }
    return -1;
  }
  // Wrap [s, e) of the block's highlightable text in <mark> elements (one per text node).
  function applyOne(block, s, e, tag) {
    if (e <= s) return;
    var nodes = textNodes(block), pos = 0, segs = [], i;
    for (i = 0; i < nodes.length; i++) {
      var node = nodes[i], L = node.nodeValue.length, ns = pos, ne = pos + L;
      var os = Math.max(s, ns), oe = Math.min(e, ne);
      if (os < oe) segs.push({ node: node, ls: os - ns, le: oe - ns });
      pos = ne;
    }
    segs.forEach(function (seg) {
      var t = seg.node;
      if (seg.ls > 0) t = t.splitText(seg.ls);
      if (seg.le - seg.ls < t.nodeValue.length) t.splitText(seg.le - seg.ls);
      var mark = document.createElement('mark');
      mark.className = 'qmd-userhl';
      mark.setAttribute('data-hl', tag);
      t.parentNode.insertBefore(mark, t);
      mark.appendChild(t);
    });
  }
  function unwrapAll() {
    var marks = [].slice.call(document.querySelectorAll('mark.qmd-userhl')), blocks = [];
    marks.forEach(function (m) {
      var b = m.closest('[data-block-id]'); if (b && blocks.indexOf(b) < 0) blocks.push(b);
      m.replaceWith(document.createTextNode(m.textContent));
    });
    blocks.forEach(function (b) { b.normalize(); });
  }
  function applyAll() {
    unwrapAll();
    load().forEach(function (h) {
      var sel = '[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(h.id) : h.id) + '"]';
      var block = document.querySelector(sel);
      if (block) applyOne(block, h.s, h.e, h.id + ':' + h.s + ':' + h.e);
    });
  }

  if (!window.__qmdHL) {
    window.__qmdHL = true;

    // The selection toolbar: a bar holding Copy / Quote / Share link (clipboard-only) plus the
    // Highlight button (which keeps its exact behaviour, now as one child).
    var bar = document.createElement('div');
    bar.className = 'qmd-seltools';
    bar.setAttribute('role', 'toolbar');
    bar.setAttribute('aria-label', 'Selection actions');
    bar.hidden = true;
    var live = document.createElement('span');
    live.className = 'qmd-sr-only';
    live.setAttribute('aria-live', 'polite');

    var mode = null, pending = null, pendingTag = null;
    function announce(msg) { live.textContent = ''; live.textContent = msg; }

    // A clipboard-action child: clicking runs `run(done)`; `done(msg)` flashes the label.
    function action(label, run) {
      var b = document.createElement('button');
      b.type = 'button'; b.className = 'qmd-hl-action';
      b.textContent = label;
      var t = null;
      b.__reset = function () { if (t) { clearTimeout(t); t = null; } b.textContent = label; };
      b.addEventListener('click', function () {
        run(function (msg) {
          if (t) clearTimeout(t);
          b.textContent = msg; announce(msg);
          t = setTimeout(function () { b.textContent = label; t = null; }, 1200);
        });
      });
      return b;
    }
    var copyBtn = action('Copy', function (done) {
      qmdCopyText(pending.text, function () { done('Copied'); }, function () { done('Copy failed'); });
    });
    var quoteBtn = action('Quote', function (done) {
      var url = qmdBuildTextFragmentUrl(pending.text) || location.href;
      var label = (document.title || location.href).replace(/[\[\]()\\]/g, '\\$&');
      var md = pending.text.split(/\r?\n/).map(function (l) { return '> ' + l; }).join('\n') +
        '\n>\n> -- [' + label + '](<' + url + '>)'; // angle-bracket dest tolerates parens in url
      qmdCopyText(md, function () { done('Quote copied'); }, function () { done('Copy failed'); });
    });
    var shareBtn = action('Share link', function (done) {
      var url = qmdBuildTextFragmentUrl(pending.text);
      if (!url) { done('Nothing to link'); return; }
      qmdCopyText(url, function () {
        done('Link copied');
        if (location.protocol === 'file:') announce('Link copied; the highlight opens when served over http or https');
      }, function () { done('Copy failed'); });
    });
    var extras = [copyBtn, quoteBtn, shareBtn];

    var btn = document.createElement('button'); // the Highlight / Remove-highlight child
    btn.type = 'button';
    btn.className = 'qmd-hl-action';
    extras.forEach(function (b) { bar.appendChild(b); });
    bar.appendChild(btn);
    document.body.appendChild(bar);
    document.body.appendChild(live);

    function resetExtras() { extras.forEach(function (b) { if (b.__reset) b.__reset(); }); }
    function hideBtn() { resetExtras(); bar.hidden = true; mode = null; pending = null; pendingTag = null; }
    function placeBtn(rect) {
      bar.hidden = false;                       // un-hide first so offsetWidth is measurable
      var w = bar.offsetWidth;
      var left = rect.left + rect.width / 2 - w / 2;
      bar.style.left = Math.max(8, Math.min(left, window.innerWidth - w - 8)) + 'px';
      bar.style.top = (rect.top - 38 >= 8 ? rect.top - 38 : rect.bottom + 8) + 'px';
    }
    function onSelect() {
      var sel = window.getSelection();
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) { if (mode === 'add') hideBtn(); return; }
      var r = sel.getRangeAt(0);
      if (bar.contains(r.startContainer)) return;
      var b1 = blockOf(r.startContainer), b2 = blockOf(r.endContainer);
      if (!b1 || b1 !== b2 || skip(r.startContainer, b1) || skip(r.endContainer, b1)) {
        if (mode === 'add') hideBtn(); return;
      }
      var s = offsetOf(b1, r.startContainer, r.startOffset), e = offsetOf(b1, r.endContainer, r.endOffset);
      if (s < 0 || e < 0 || e <= s) { if (mode === 'add') hideBtn(); return; }
      // The selection text from the SAME math/code-free walk the offsets use (single block).
      var text = textNodes(b1).map(function (n) { return n.nodeValue; }).join('').slice(s, e);
      mode = 'add'; pending = { id: b1.getAttribute('data-block-id'), s: s, e: e, text: text }; pendingTag = null;
      resetExtras();
      extras.forEach(function (b) { b.hidden = false; });
      btn.textContent = 'Highlight';
      placeBtn(r.getBoundingClientRect());
    }
    bar.addEventListener('mousedown', function (e) { e.preventDefault(); }); // keep the selection on any click
    btn.addEventListener('click', function () {
      if (mode === 'add' && pending) {
        var list = load();
        if (!list.some(function (h) { return h.id === pending.id && h.s === pending.s && h.e === pending.e; })) {
          list.push({ id: pending.id, s: pending.s, e: pending.e }); save(list); // keep the id:s:e schema
        }
        var sel = window.getSelection(); if (sel) sel.removeAllRanges();
        dispatch();
      } else if (mode === 'remove' && pendingTag) {
        save(load().filter(function (h) { return (h.id + ':' + h.s + ':' + h.e) !== pendingTag; }));
        dispatch();
      }
      hideBtn();
    });
    document.addEventListener('mouseup', function () { setTimeout(onSelect, 0); });
    document.addEventListener('keyup', function (e) {
      if (e.shiftKey || e.key === 'ArrowLeft' || e.key === 'ArrowRight') setTimeout(onSelect, 0);
    });
    document.addEventListener('click', function (e) {
      var m = e.target.closest && e.target.closest('mark.qmd-userhl');
      if (m) {
        mode = 'remove'; pendingTag = m.getAttribute('data-hl'); pending = null;
        resetExtras();
        extras.forEach(function (b) { b.hidden = true; });
        btn.textContent = 'Remove highlight';
        placeBtn(m.getBoundingClientRect());
        e.stopPropagation();
        return;
      }
      if (!bar.contains(e.target) && window.getSelection().isCollapsed) hideBtn();
    });
    window.addEventListener('scroll', function () { if (mode) hideBtn(); }, { passive: true });
    window.addEventListener('qmd:hlchange', applyAll);
  }

  applyAll();
}

// My highlights: an index + Markdown export over the reader's highlights (qmdInitHighlights).
// When the page has any highlights, a "N highlights" button (bottom-left) opens a panel that
// lists them, jumps to one, removes one, or exports them all as Markdown into a selectable
// textarea (and best-effort to the clipboard). Reader-side + read-only: reads the same
// localStorage; coordinates with the highlighter via the qmd:hlchange event. Skipped on decks.
function qmdInitHighlightIndex() {
  if (document.querySelector('.qmd-deck')) return;
  if (!window.qmdReaderMenu) return;          // need the menu host
  if (window.__qmdHLIndex) return;
  window.__qmdHLIndex = true;

  var KEY = 'qmd-hl:' + location.pathname;
  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function changed() { try { window.dispatchEvent(new CustomEvent('qmd:hlchange')); } catch (e) {} }

  // Same highlightable-text rule as the highlighter, so offsets resolve identically.
  function skip(node, block) {
    var p = node.parentNode;
    while (p && p !== block) {
      if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
          (p.classList && p.classList.contains('katex')))) return true;
      p = p.parentNode;
    }
    return false;
  }
  function blockText(block) {
    var w = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, null), n, s = '';
    while ((n = w.nextNode())) { if (!skip(n, block)) s += n.nodeValue; }
    return s;
  }
  function findBlock(id) {
    return document.querySelector('[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]');
  }
  function textOf(h) { var b = findBlock(h.id); return b ? blockText(b).slice(h.s, h.e) : null; }
  function flash(block) { block.classList.remove('qmd-flash'); void block.offsetWidth; block.classList.add('qmd-flash'); }

  var body = document.createElement('div');
  function render() {
    while (body.firstChild) body.removeChild(body.firstChild);
    var ul = document.createElement('ul'); ul.className = 'qmd-hlx-list';
    load().forEach(function (h) {
      var t = textOf(h); if (t == null) return; // block gone (orphaned highlight)
      var li = document.createElement('li');
      var go = document.createElement('button');
      go.type = 'button'; go.className = 'qmd-hlx-go';
      go.textContent = t.length > 90 ? t.slice(0, 90) + '…' : t;
      go.addEventListener('click', function () {
        var b = findBlock(h.id);
        if (b) { window.qmdReaderMenu.close(); b.scrollIntoView({ block: 'center', behavior: 'smooth' }); flash(b); }
      });
      var rm = document.createElement('button');
      rm.type = 'button'; rm.className = 'qmd-hlx-rm';
      rm.setAttribute('aria-label', 'Remove'); rm.textContent = '×';
      rm.addEventListener('click', function () {
        var tag = h.id + ':' + h.s + ':' + h.e;
        save(load().filter(function (x) { return (x.id + ':' + x.s + ':' + x.e) !== tag; }));
        changed();
      });
      li.appendChild(go); li.appendChild(rm); ul.appendChild(li);
    });
    body.appendChild(ul);

    var actions = document.createElement('div'); actions.className = 'qmd-hlx-actions';
    var exp = document.createElement('button');
    exp.type = 'button'; exp.className = 'qmd-hlx-export'; exp.textContent = 'Export as Markdown';
    var ta = document.createElement('textarea');
    ta.className = 'qmd-hlx-out'; ta.readOnly = true; ta.hidden = true;
    ta.setAttribute('aria-label', 'Highlights as Markdown');
    exp.addEventListener('click', function () {
      var md = '# ' + (document.title || 'Highlights') + '\n\n' + location.href + '\n\n';
      load().forEach(function (h) { var t = textOf(h); if (t != null) md += '> ' + t.replace(/\s+/g, ' ').trim() + '\n\n'; });
      ta.value = md; ta.hidden = false; ta.focus(); ta.select();
      // Best-effort clipboard; the visible textarea is the reliable path. (Not qmdCopyText: its
      // execCommand fallback would steal the selection from the textarea in an insecure context.)
      try { if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(md); } catch (e) {}
    });
    actions.appendChild(exp); actions.appendChild(ta); body.appendChild(actions);
  }

  var section = window.qmdReaderMenu.addSection('Highlights', body, render);
  function refresh() {
    var n = load().filter(function (h) { return textOf(h) != null; }).length;
    section.setVisible(n > 0);
    render();
  }
  window.addEventListener('qmd:hlchange', refresh);
  refresh();
}

// Reader bookmarks: section markers. Hovering a heading reveals a star toggle in its left
// margin; clicking bookmarks that section. Bookmarked headings keep a persistent star, and
// the reader menu gathers them into a "Bookmarks" list (jump / remove). Bookmarks live in
// the reader's own localStorage, anchored to the heading's data-block-id (exact; an orphaned
// id is skipped). Reader-side + read-only. Decks skipped. Markers re-apply each pass; the
// toggle + listeners + menu section are set up once.
function qmdInitBookmarks() {
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome
  var KEY = 'qmd-bm:' + location.pathname;

  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function dispatch() { try { window.dispatchEvent(new CustomEvent('qmd:bmchange')); } catch (e) {} }
  function has(id) { return load().indexOf(id) !== -1; }
  function findBlock(id) {
    return document.querySelector('[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]');
  }
  function headingFrom(t) {
    var h = t && t.closest ? t.closest('h1,h2,h3,h4,h5,h6') : null;
    return (h && h.getAttribute('data-block-id')) ? h : null;
  }
  var HEADS = 'h1[data-block-id],h2[data-block-id],h3[data-block-id],h4[data-block-id],h5[data-block-id],h6[data-block-id]';

  // Re-apply the persistent margin star to every bookmarked heading (idempotent).
  function applyMarkers() {
    var ids = load();
    document.querySelectorAll(HEADS).forEach(function (h) {
      h.classList.toggle('qmd-bookmarked', ids.indexOf(h.getAttribute('data-block-id')) !== -1);
    });
  }

  if (!window.__qmdBookmarks) {
    window.__qmdBookmarks = true;

    var toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'qmd-bm-toggle';
    toggle.hidden = true;
    toggle.setAttribute('aria-label', 'Bookmark this section');
    document.body.appendChild(toggle);

    var active = null, overToggle = false, hideTimer = null;
    function show(h) {
      active = h; clearTimeout(hideTimer);
      var on = has(h.getAttribute('data-block-id'));
      toggle.textContent = on ? '★' : '☆'; // filled / outline star
      toggle.setAttribute('aria-pressed', on ? 'true' : 'false');
      toggle.title = on ? 'Remove bookmark' : 'Bookmark this section';
      var r = h.getBoundingClientRect();
      toggle.style.top = (window.pageYOffset + r.top + r.height / 2 - 13) + 'px';
      toggle.style.left = Math.max(2, window.pageXOffset + r.left - 30) + 'px';
      toggle.hidden = false;
    }
    function scheduleHide() {
      clearTimeout(hideTimer);
      hideTimer = setTimeout(function () { if (!overToggle) toggle.hidden = true; }, 140);
    }
    document.addEventListener('mouseover', function (e) {
      var h = headingFrom(e.target);
      if (h) show(h);
      else if (e.target !== toggle) scheduleHide();
    });
    toggle.addEventListener('mouseenter', function () { overToggle = true; clearTimeout(hideTimer); });
    toggle.addEventListener('mouseleave', function () { overToggle = false; scheduleHide(); });
    toggle.addEventListener('click', function () {
      if (!active) return;
      var id = active.getAttribute('data-block-id'), list = load(), i = list.indexOf(id);
      if (i === -1) list.push(id); else list.splice(i, 1);
      save(list); dispatch(); show(active);
    });
    window.addEventListener('scroll', function () { if (!toggle.hidden) toggle.hidden = true; }, { passive: true });
    window.addEventListener('qmd:bmchange', applyMarkers);

    // The Bookmarks list in the reader menu (jump / remove). Degrades to just the margin
    // stars + hover toggle if the menu host is absent.
    if (window.qmdReaderMenu) {
      var body = document.createElement('div');
      function flash(block) { block.classList.remove('qmd-flash'); void block.offsetWidth; block.classList.add('qmd-flash'); }
      function renderList() {
        while (body.firstChild) body.removeChild(body.firstChild);
        var ul = document.createElement('ul'); ul.className = 'qmd-hlx-list';
        load().forEach(function (id) {
          var block = findBlock(id); if (!block) return; // orphaned heading
          var li = document.createElement('li');
          var go = document.createElement('button');
          go.type = 'button'; go.className = 'qmd-hlx-go';
          var t = (block.textContent || '').replace(/\s+/g, ' ').trim();
          go.textContent = t.length > 60 ? t.slice(0, 60) + '…' : t;
          go.addEventListener('click', function () {
            window.qmdReaderMenu.close(); block.scrollIntoView({ block: 'center', behavior: 'smooth' }); flash(block);
          });
          var rm = document.createElement('button');
          rm.type = 'button'; rm.className = 'qmd-hlx-rm';
          rm.setAttribute('aria-label', 'Remove bookmark'); rm.textContent = '×';
          rm.addEventListener('click', function () {
            save(load().filter(function (x) { return x !== id; })); dispatch();
          });
          li.appendChild(go); li.appendChild(rm); ul.appendChild(li);
        });
        body.appendChild(ul);
      }
      var section = window.qmdReaderMenu.addSection('Bookmarks', body, renderList);
      function refresh() {
        section.setVisible(load().filter(function (id) { return !!findBlock(id); }).length > 0);
        renderList();
      }
      window.addEventListener('qmd:bmchange', refresh);
      refresh();
    }
  }

  applyMarkers();
}

