
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
window.qmdEnhancers.register(qmdInitCategoryFilter);

// Native category filter for `listing: { categories: true }`: the server emits a
// chip row (`.qmd-cat-filter`) above the card grid and tags each card with
// `data-categories`. Clicking a chip — or a category tag on a card — toggles it
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
      var raw = card.getAttribute('data-categories');
      return raw ? raw.split(',') : [];
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
    'figure img,pre.mermaid{cursor:zoom-in}' +
    '#qmd-lightbox{position:fixed;inset:0;z-index:2147483000;display:none;flex-direction:column;' +
    'align-items:center;justify-content:center;gap:.9rem;padding:2rem;box-sizing:border-box;' +
    'background:rgba(10,12,16,.9);cursor:zoom-out;opacity:0;transition:opacity .15s ease}' +
    '#qmd-lightbox.open{display:flex;opacity:1}' +
    '#qmd-lightbox img{max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'background:var(--qmd-bg,#fff);border-radius:4px;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
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
    '<img alt=""><div class="qmd-lb-svg"></div><div class="qmd-lb-cap"></div>';
  document.body.appendChild(box);
  var lbImg = box.querySelector('img');
  var lbSvg = box.querySelector('.qmd-lb-svg');
  var lbCap = box.querySelector('.qmd-lb-cap');

  function openImg(srcImg) {
    lbSvg.style.display = 'none'; lbSvg.innerHTML = '';
    lbImg.style.display = '';
    lbImg.src = srcImg.currentSrc || srcImg.src;
    lbImg.alt = srcImg.alt || '';
    var fig = srcImg.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : (srcImg.alt || '');
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
  }
  function openMermaid(pre) {
    var svg = pre.querySelector('svg');
    if (!svg) return; // not rendered yet
    lbImg.style.display = 'none'; lbImg.removeAttribute('src');
    var clone = svg.cloneNode(true);
    clone.removeAttribute('width'); clone.removeAttribute('height');
    clone.style.maxWidth = 'none';
    lbSvg.innerHTML = ''; lbSvg.appendChild(clone);
    lbSvg.style.display = 'block';
    // Show the figure's caption in the zoom too (empty -> hidden by CSS).
    var fig = pre.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : '';
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
  }
  function close() {
    box.classList.remove('open');
    document.documentElement.style.overflow = ''; // restore page scroll
    lbImg.removeAttribute('src');
    lbSvg.innerHTML = '';
  }

  var unmodified = function (e) {
    return !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
  };
  document.addEventListener('click', function (e) {
    if (!e.target.closest) return;
    if (e.target.closest('figure img') && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openImg(e.target);
    } else {
      var pre = e.target.closest('pre.mermaid');
      if (pre && pre.querySelector('svg') && unmodified(e)) {
        e.preventDefault(); e.stopPropagation(); openMermaid(pre);
      }
    }
  }, true);
  // Keep a double-click on a figure/diagram from reaching click-to-source.
  document.addEventListener('dblclick', function (e) {
    if (e.target.closest && e.target.closest('figure img, pre.mermaid')) {
      e.preventDefault(); e.stopPropagation();
    }
  }, true);
  box.addEventListener('click', function (e) {
    if (e.target !== lbImg && !lbSvg.contains(e.target)) close();
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && box.classList.contains('open')) close();
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

