// Full-screen viewer for figure images AND mermaid diagrams. Set up once; uses
// event delegation in the capture phase so a click opens the lightbox WITHOUT
// triggering the block-level click/double-click handlers (highlight,
// click-to-source). Images are shown via <img>; mermaid SVGs are cloned live
// (so <foreignObject> labels keep rendering, which an <img> would drop). Modifier
// clicks pass through (new tab, reveal alt-zoom). Dismiss: backdrop, Esc, or x.
function taliInitLightbox() {
  if (window.__taliLightbox) return;
  window.__taliLightbox = true;

  var style = document.createElement('style');
  style.textContent =
    'figure img,img.lightbox,pre.mermaid,.tali-video video{cursor:zoom-in}' +
    '#tali-lightbox{position:fixed;inset:0;z-index:2147483000;display:none;flex-direction:column;' +
    'align-items:center;justify-content:center;gap:.9rem;padding:2rem;box-sizing:border-box;' +
    'background:rgba(10,12,16,.9);cursor:zoom-out;opacity:0;transition:opacity .15s ease}' +
    '#tali-lightbox.open{display:flex;opacity:1}' +
    '#tali-lightbox img{max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'background:var(--tali-bg,#fff);border-radius:4px;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#tali-lightbox video{display:none;max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'border-radius:6px;background:#000;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#tali-lightbox .tali-lb-svg{display:none;width:92vw;max-width:1400px;max-height:86vh;overflow:auto;' +
    'cursor:default;background:var(--tali-bg,#fff);border-radius:4px;padding:1.2rem;box-sizing:border-box;' +
    'box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#tali-lightbox .tali-lb-svg svg{display:block;width:100%;height:auto;max-width:100%}' +
    '#tali-lightbox .tali-lb-cap{color:#e8e8e8;font:14px ui-sans-serif,system-ui,sans-serif;' +
    'text-align:center;max-width:93vw}' +
    '#tali-lightbox .tali-lb-cap:empty{display:none}' +
    '#tali-lightbox .tali-lb-close{position:fixed;top:.6rem;right:1rem;color:#fff;background:none;' +
    'border:0;font-size:2.2rem;line-height:1;cursor:pointer;opacity:.75}' +
    '#tali-lightbox .tali-lb-close:hover{opacity:1}' +
    // Prev/next controls: hidden by default, shown only while a multi-image gallery is
    // active (the `.has-gallery` class). Large tap targets so touch users can step too.
    '#tali-lightbox .tali-lb-nav{position:fixed;top:50%;transform:translateY(-50%);display:none;' +
    'align-items:center;justify-content:center;width:3rem;height:4.5rem;color:#fff;' +
    'background:rgba(0,0,0,.35);border:0;border-radius:8px;font-size:2.2rem;line-height:1;' +
    'cursor:pointer;opacity:.7;transition:opacity .15s ease,background .15s ease}' +
    '#tali-lightbox .tali-lb-nav:hover{opacity:1;background:rgba(0,0,0,.55)}' +
    '#tali-lightbox.has-gallery .tali-lb-nav{display:flex}' +
    '#tali-lightbox .tali-lb-prev{left:.6rem}#tali-lightbox .tali-lb-next{right:.6rem}' +
    // No open/close fade for a reader who asked for reduced motion (PA-B7).
    '@media (prefers-reduced-motion: reduce){#tali-lightbox{transition:none}}';
  document.head.appendChild(style);

  var box = document.createElement('div');
  box.id = 'tali-lightbox';
  box.setAttribute('role', 'dialog');
  box.setAttribute('aria-label', 'Image viewer'); // a role=dialog needs an accessible name
  box.innerHTML = '<button class="tali-lb-close" aria-label="Close">×</button>' +
    '<button class="tali-lb-nav tali-lb-prev" aria-label="Previous image">‹</button>' +
    '<button class="tali-lb-nav tali-lb-next" aria-label="Next image">›</button>' +
    '<img alt=""><video class="tali-lb-video" muted loop playsinline></video>' +
    // aria-live so stepping the gallery (←/→) announces the new caption + "(n / N)" counter
    // to a screen reader (PA-A2); every sibling enhancer already has one.
    '<div class="tali-lb-svg"></div><div class="tali-lb-cap" aria-live="polite"></div>';
  document.body.appendChild(box);
  // These are the fixed children just written into `box.innerHTML` above, so they are
  // always present; cast to their concrete element types (non-null) accordingly.
  var lbImg = /** @type {HTMLImageElement} */ (box.querySelector('img'));
  var lbVideo = /** @type {HTMLVideoElement} */ (box.querySelector('.tali-lb-video'));
  var lbSvg = /** @type {HTMLElement} */ (box.querySelector('.tali-lb-svg'));
  var lbCap = /** @type {HTMLElement} */ (box.querySelector('.tali-lb-cap'));
  var lbPrev = /** @type {HTMLElement} */ (box.querySelector('.tali-lb-prev'));
  var lbNext = /** @type {HTMLElement} */ (box.querySelector('.tali-lb-next'));
  var gallery = /** @type {HTMLImageElement[]} */ ([]), gIdx = -1; // page's zoomable images, for ←/→ nav
  var lbRelease = /** @type {(() => void) | null} */ (null); // active focus-trap release while open

  // Open the box (add the class, lock scroll, trap focus on the close button once).
  function markOpen() {
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden';
    if (!lbRelease && window.taliFocusTrap) lbRelease = window.taliFocusTrap(box, box.querySelector('.tali-lb-close'));
  }

  function hideAll() {
    lbImg.style.display = 'none'; lbImg.removeAttribute('src');
    lbVideo.style.display = 'none';
    try { lbVideo.pause(); } catch (e) {}
    lbVideo.removeAttribute('src');
    lbSvg.style.display = 'none'; lbSvg.innerHTML = '';
    box.classList.remove('has-gallery'); // hide prev/next until a multi-image set is shown
  }
  // Show gallery[i] (wrapping) with its caption + an (n / N) counter for multi-image sets.
  /** @param {number} i */
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
    var cap = fc ? taliCleanCaptionText(fc) : (img.alt || '');
    if (gallery.length > 1) cap = (cap ? cap + '  ' : '') + '(' + (gIdx + 1) + ' / ' + gallery.length + ')';
    lbCap.textContent = cap;
    box.classList.toggle('has-gallery', gallery.length > 1); // reveal prev/next only for a set
    markOpen();
  }
  // Open the clicked image, building the page's gallery so ←/→ can step between images.
  /** @param {HTMLImageElement} srcImg */
  function openImg(srcImg) {
    // Only visible images join the gallery: a `dark=` figure has two <img>s but one is
    // theme-hidden (display:none → no offsetParent), so it must not become a phantom
    // ←/→ step or inflate the (n / N) counter. The clicked image is always kept.
    gallery = /** @type {HTMLImageElement[]} */ ([].slice.call(document.querySelectorAll('figure img, img.lightbox')))
      .filter(function (im) { return im === srcImg || im.offsetParent !== null; });
    var i = gallery.indexOf(srcImg);
    if (i < 0) { gallery = [srcImg]; i = 0; }
    showImageAt(i);
  }
  /** @param {Element} pre */
  function openMermaid(pre) {
    var svg = pre.querySelector('svg');
    if (!svg) return; // not rendered yet
    hideAll();
    var clone = /** @type {SVGElement} */ (svg.cloneNode(true));
    clone.removeAttribute('width'); clone.removeAttribute('height');
    clone.style.maxWidth = 'none';
    lbSvg.appendChild(clone);
    lbSvg.style.display = 'block';
    // Show the figure's caption in the zoom too (empty -> hidden by CSS).
    var fig = pre.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = taliCleanCaptionText(fc);
    markOpen();
  }
  // A `{{< video >}}` screencast: play an enlarged copy (the clicked element is the
  // theme-visible variant; the hidden one is display:none and not clickable).
  /** @param {HTMLVideoElement} vid */
  function openVideo(vid) {
    hideAll();
    lbVideo.style.display = 'block'; // CSS defaults it to none; need an explicit value
    lbVideo.src = vid.currentSrc || vid.src;
    var p = lbVideo.play(); if (p && p.catch) p.catch(function () {});
    var fig = vid.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = taliCleanCaptionText(fc);
    markOpen();
  }
  function close() {
    box.classList.remove('open');
    document.documentElement.style.overflow = ''; // restore page scroll
    hideAll();
    gallery = []; gIdx = -1;
    if (lbRelease) { lbRelease(); lbRelease = null; }
  }

  /** @param {MouseEvent} e */
  var unmodified = function (e) {
    return !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
  };
  // In a deck's overview, a click on a tile must NAVIGATE to that slide, not zoom the
  // media it happens to contain. The deck's own click-to-navigate is a bubble-phase
  // listener, so it never runs if this capture-phase delegate opens the viewer (and
  // stops the event) first. Bail here so the click bubbles through to navigation.
  /** @param {Element | null} t */
  var inDeckOverview = function (t) { return !!(t && t.closest && t.closest('.tali-deck.overview')); };
  document.addEventListener('click', function (e) {
    var t = /** @type {Element | null} */ (e.target);
    if (!t || !t.closest) return;
    if (inDeckOverview(t)) return; // overview: let the click reach the deck's slide navigation
    // Mutually-exclusive image / video / mermaid targets, as early returns (was an
    // if / else-if / else chain — same semantics, but each branch narrows its own type).
    var img = /** @type {HTMLImageElement | null} */ (t.closest('figure img, img.lightbox'));
    if (img && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openImg(img); return;
    }
    var vid = /** @type {HTMLVideoElement | null} */ (t.closest('.tali-video video'));
    if (vid && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openVideo(vid); return;
    }
    var pre = t.closest('pre.mermaid');
    if (pre && pre.querySelector('svg') && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openMermaid(pre);
    }
  }, true);
  // Keep a double-click on a figure/diagram/video from reaching click-to-source.
  document.addEventListener('dblclick', function (e) {
    var t = /** @type {Element | null} */ (e.target);
    if (t && t.closest && !inDeckOverview(t) && t.closest('figure img, img.lightbox, pre.mermaid, .tali-video video')) {
      e.preventDefault(); e.stopPropagation();
    }
  }, true);
  box.addEventListener('click', function (e) {
    // The nav buttons live on the backdrop; stopPropagation below keeps their click from
    // reaching here (which would treat it as an outside-click and close the viewer).
    var t = /** @type {Node | null} */ (e.target);
    if (t !== lbImg && t !== lbVideo && !lbSvg.contains(t)) close();
  });
  // Visible prev/next controls (mouse + touch): keyboard ←/→ already works, but the
  // gallery had no on-screen affordance. stopPropagation so the backdrop-close doesn't fire.
  lbPrev.addEventListener('click', function (e) { e.stopPropagation(); showImageAt(gIdx - 1); });
  lbNext.addEventListener('click', function (e) { e.stopPropagation(); showImageAt(gIdx + 1); });
  // Touch swipe steps the gallery (only while a multi-image set is shown).
  var touchX = /** @type {number | null} */ (null);
  box.addEventListener('touchstart', function (e) {
    touchX = e.touches.length === 1 ? e.touches[0].clientX : null;
  }, { passive: true });
  box.addEventListener('touchend', function (e) {
    if (touchX === null || !box.classList.contains('has-gallery')) { touchX = null; return; }
    var dx = e.changedTouches[0].clientX - touchX;
    touchX = null;
    if (Math.abs(dx) > 40) showImageAt(dx < 0 ? gIdx + 1 : gIdx - 1);
  }, { passive: true });
  document.addEventListener('keydown', function (e) {
    if (!box.classList.contains('open')) return;
    if (e.key === 'Escape') { close(); return; }
    // ←/→ step the image gallery (only while an image, not a video/diagram, is shown).
    if (gallery.length > 1 && lbImg.style.display !== 'none') {
      if (e.key === 'ArrowRight') { e.preventDefault(); showImageAt(gIdx + 1); }
      else if (e.key === 'ArrowLeft') { e.preventDefault(); showImageAt(gIdx - 1); }
    }
  });

  // Keyboard entry point for the decorated types (images + mermaid), driven by the per-mount
  // decoration below. A `{{< video >}}` is intentionally absent: it is not keyboard-decorated
  // (it keeps native media semantics), and its mouse click-zoom goes through the delegation above.
  window.__taliLightboxOpen = function (el) {
    if (el.matches && el.matches('figure img, img.lightbox')) openImg(/** @type {HTMLImageElement} */ (el));
    else if (el.matches && el.matches('pre.mermaid')) { if (el.querySelector('svg')) openMermaid(el); }
  };
}

// Keyboard affordance (WCAG 2.1.1): the lightbox otherwise opens only via a delegated mouse
// click, so decoratable media is unreachable by keyboard. This per-mount, idempotent pass
// (guard `data-tali-lb`) makes each zoomable element a focusable button that opens on
// Enter/Space. The capture-phase click delegation, focus-trap-on-open, Escape, and the ←/→
// gallery nav inside taliInitLightbox are untouched; this only adds the keyboard entry point.
/** @param {ParentNode | null} [root] */
function taliDecorateLightbox(root) {
  taliInitLightbox(); // ensure the document-level machinery + window.__taliLightboxOpen exist
  var scope = root || document;
  // Images + mermaid diagrams only. A `<video>` keeps its native media semantics: stamping
  // role="button"/aria-label onto it mislabels it in the a11y tree, and the clip already
  // autoplays inline. Mouse click-to-zoom on a video still works via the click delegation above.
  // All matches are HTMLElements (img / pre); typed so `el.addEventListener('keydown')`
  // resolves to KeyboardEvent (plain Element's event map lacks keydown).
  var els = /** @type {NodeListOf<HTMLElement>} */ (scope.querySelectorAll('figure img, img.lightbox, pre.mermaid'));
  els.forEach(function (el) {
    if (el.getAttribute('data-tali-lb')) return; // idempotent: decorate once per element
    var isMermaid = el.matches && el.matches('pre.mermaid');
    // A decorative image (author set an explicit empty alt) stays out of the tab
    // order — forcing it to a focusable "zoom" button contradicts the author's
    // "ignore me" marking. An explicit `.lightbox` opt-in overrides that.
    if (el.tagName === 'IMG' && el.getAttribute('alt') === '' && !el.classList.contains('lightbox')) {
      return;
    }
    el.setAttribute('data-tali-lb', '1');
    el.setAttribute('tabindex', '0');
    // Don't collapse a mermaid diagram into a `role="button"` LEAF — that hides its
    // SVG title/desc from assistive tech. `role="figure"` keeps the diagram content
    // in the a11y tree while the element stays focusable + keyboard-zoomable.
    el.setAttribute('role', isMermaid ? 'figure' : 'button');
    var alt = el.getAttribute && el.getAttribute('alt');
    el.setAttribute(
      'aria-label',
      isMermaid ? 'Diagram — press Enter to zoom' : alt ? alt : 'View image full size'
    );
    el.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') {
        e.preventDefault(); // stop Space from scrolling the page
        if (window.__taliLightboxOpen) window.__taliLightboxOpen(el);
      }
    });
  });
}
if (window.taliEnhancers) window.taliEnhancers.register(taliDecorateLightbox);

