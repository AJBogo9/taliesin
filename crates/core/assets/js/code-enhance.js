
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
      var text = code.innerText;
      var ok = function () {
        btn.innerHTML = checkIcon;
        btn.classList.add('qmd-copied');
        btn.setAttribute('aria-label', 'Copied');
        setTimeout(function () { btn.innerHTML = copyIcon; btn.classList.remove('qmd-copied'); btn.setAttribute('aria-label', 'Copy code'); }, 1200);
      };
      // navigator.clipboard only exists in a secure context; over --host (plain http
      // on the LAN, e.g. a phone) fall back to a hidden-textarea execCommand copy so
      // the button still copies and confirms with the check.
      var legacy = function () {
        try {
          var ta = document.createElement('textarea');
          ta.value = text; ta.setAttribute('readonly', '');
          ta.style.position = 'fixed'; ta.style.top = '0'; ta.style.opacity = '0';
          document.body.appendChild(ta); ta.select();
          var done = document.execCommand('copy'); document.body.removeChild(ta);
          return done;
        } catch (e) { return false; }
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(ok, function () { if (legacy()) ok(); });
      } else if (legacy()) {
        ok();
      }
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
window.qmdEnhancers.register(function () { qmdInitReaderPrefs(); });
window.qmdEnhancers.register(function () { qmdInitReadingProgress(); });
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
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
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
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
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
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
  }
  function close() {
    box.classList.remove('open');
    document.documentElement.style.overflow = ''; // restore page scroll
    hideAll();
    gallery = []; gIdx = -1;
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

// Reader preferences ("Aa" control): a reader-local text size / reading width / theme
// picker. State lives in the reader's own localStorage and is applied before paint by the
// pre-paint head script (qmdSetTheme / qmdSetReaderPref / qmdResetReader in theme.rs), so
// this enhancer is only the UI. Read-only: it never writes the author's source. Skipped on
// decks (own chrome). Idempotent (document-level, builds once).
function qmdInitReaderPrefs() {
  if (window.__qmdReaderPrefs) return;
  if (!window.qmdSetReaderPref) return;            // pre-paint API absent (older page)
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

  var btn = document.createElement('button');
  btn.className = 'qmd-reader-toggle';
  btn.type = 'button';
  btn.textContent = 'Aa';
  btn.setAttribute('aria-label', 'Reading settings');
  btn.setAttribute('aria-haspopup', 'dialog');
  btn.setAttribute('aria-expanded', 'false');

  var panel = document.createElement('div');
  panel.className = 'qmd-reader-panel';
  panel.setAttribute('role', 'dialog');
  panel.setAttribute('aria-label', 'Reading settings');
  panel.hidden = true;

  var head = document.createElement('h2');
  head.textContent = 'Reading settings';
  panel.appendChild(head);

  var themeSeg = seg('Theme', THEMES, curTheme, function (v) { window.qmdSetTheme(v); });
  var sizeSeg = seg('Text size', SIZES, curSize,
    function (v) { window.qmdSetReaderPref('scale', v === '1' ? null : v); },
    function (b, opt) { b.textContent = 'A'; b.style.fontSize = SIZE_FS[opt[0]] || '.95rem';
      b.setAttribute('aria-label', opt[1] + ' text'); });
  var widthSeg = seg('Width', WIDTHS, curWidth,
    function (v) { window.qmdSetReaderPref('width', v || null); });

  panel.appendChild(themeSeg.row);
  panel.appendChild(sizeSeg.row);
  panel.appendChild(widthSeg.row);

  var reset = document.createElement('button');
  reset.className = 'qmd-reader-reset';
  reset.type = 'button';
  reset.textContent = 'Reset to defaults';
  reset.addEventListener('click', function () { if (window.qmdResetReader) window.qmdResetReader(); });
  panel.appendChild(reset);

  function syncAll() { themeSeg.sync(); sizeSeg.sync(); widthSeg.sync(); }
  syncAll();

  function open() { panel.hidden = false; btn.setAttribute('aria-expanded', 'true'); }
  function close() { panel.hidden = true; btn.setAttribute('aria-expanded', 'false'); }

  btn.addEventListener('click', function (e) { e.stopPropagation(); if (panel.hidden) open(); else close(); });
  document.addEventListener('click', function (e) {
    if (!panel.hidden && !panel.contains(e.target) && e.target !== btn) close();
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && !panel.hidden) { close(); btn.focus(); }
  });
  window.addEventListener('qmd:themechange', syncAll);
  window.addEventListener('qmd:readerchange', syncAll);

  document.body.appendChild(btn);
  document.body.appendChild(panel);
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
  var time = document.createElement('div');
  time.className = 'qmd-readbar-time';
  time.setAttribute('aria-hidden', 'true');
  time.hidden = true;
  document.body.appendChild(bar);
  document.body.appendChild(time);

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
    var left = Math.ceil(totalMin * (1 - f));
    if (f > 0.985 || left <= 0) { time.hidden = true; }
    else { time.hidden = false; time.textContent = left + ' min left'; }
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
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', schedule, { passive: true });
  window.addEventListener('qmd:readerchange', schedule);
  maybeShowResume();
}

