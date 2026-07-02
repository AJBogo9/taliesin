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
    '#tali-lightbox .tali-lb-close:hover{opacity:1}';
  document.head.appendChild(style);

  var box = document.createElement('div');
  box.id = 'tali-lightbox';
  box.setAttribute('role', 'dialog');
  box.setAttribute('aria-label', 'Image viewer'); // a role=dialog needs an accessible name
  box.innerHTML = '<button class="tali-lb-close" aria-label="Close">×</button>' +
    '<img alt=""><video class="tali-lb-video" muted loop playsinline></video>' +
    '<div class="tali-lb-svg"></div><div class="tali-lb-cap"></div>';
  document.body.appendChild(box);
  var lbImg = box.querySelector('img');
  var lbVideo = box.querySelector('.tali-lb-video');
  var lbSvg = box.querySelector('.tali-lb-svg');
  var lbCap = box.querySelector('.tali-lb-cap');
  var gallery = [], gIdx = -1; // the page's zoomable images, for ←/→ navigation
  var lbRelease = null;        // active focus-trap release while the lightbox is open

  // Open the box (add the class, lock scroll, trap focus on the close button once).
  function markOpen() {
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden';
    if (!lbRelease && window.qmdFocusTrap) lbRelease = window.qmdFocusTrap(box, box.querySelector('.tali-lb-close'));
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
    var cap = fc ? qmdCleanCaptionText(fc) : (img.alt || '');
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
    lbCap.textContent = qmdCleanCaptionText(fc);
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
    lbCap.textContent = qmdCleanCaptionText(fc);
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
    } else if ((vid = e.target.closest('.tali-video video')) && unmodified(e)) {
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
    if (e.target.closest && e.target.closest('figure img, img.lightbox, pre.mermaid, .tali-video video')) {
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

  // Expose the open helpers so the keyboard decoration (which runs per-mount through the
  // enhancer registry) can drive the lightbox by element type without re-delegating clicks.
  window.__qmdLightboxOpen = function (el) {
    if (el.matches && el.matches('figure img, img.lightbox')) openImg(el);
    else if (el.matches && el.matches('.tali-video video')) openVideo(el);
    else if (el.matches && el.matches('pre.mermaid')) { if (el.querySelector('svg')) openMermaid(el); }
  };
}

// Keyboard affordance (WCAG 2.1.1): the lightbox otherwise opens only via a delegated mouse
// click, so decoratable media is unreachable by keyboard. This per-mount, idempotent pass
// (guard `data-qmd-lb`) makes each zoomable element a focusable button that opens on
// Enter/Space. The capture-phase click delegation, focus-trap-on-open, Escape, and the ←/→
// gallery nav inside qmdInitLightbox are untouched; this only adds the keyboard entry point.
function qmdDecorateLightbox(root) {
  qmdInitLightbox(); // ensure the document-level machinery + window.__qmdLightboxOpen exist
  var scope = root || document;
  var els = scope.querySelectorAll('figure img, img.lightbox, pre.mermaid, .tali-video video');
  [].forEach.call(els, function (el) {
    if (el.getAttribute('data-qmd-lb')) return; // idempotent: decorate once per element
    el.setAttribute('data-qmd-lb', '1');
    el.setAttribute('tabindex', '0');
    el.setAttribute('role', 'button');
    var alt = el.getAttribute && el.getAttribute('alt');
    el.setAttribute('aria-label', alt ? alt : 'View image full size');
    el.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') {
        e.preventDefault(); // stop Space from scrolling the page
        if (window.__qmdLightboxOpen) window.__qmdLightboxOpen(el);
      }
    });
  });
}
window.qmdEnhancers.register(qmdDecorateLightbox);

