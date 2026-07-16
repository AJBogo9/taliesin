// Hover preview for internal links: hovering a citation or a cross
// reference pops up a small card previewing its target (the reference entry, the
// figure + caption, the equation, the table). Server-rendered, so the clone needs no
// re-running (math is already KaTeX HTML). Set up once via event delegation, so it
// survives block swaps. Section-heading links get NO preview (they carry no useful
// extra context beyond their title); table-of-contents links are skipped too.
function taliInitLinkPreview() {
  if (window.__qmdLinkPreview) return;
  window.__qmdLinkPreview = true;

  var style = document.createElement('style');
  style.textContent =
    '#tali-link-preview{position:fixed;z-index:2147482000;max-width:min(440px,90vw);max-height:50vh;' +
    'overflow:auto;background:var(--tali-bg,#fff);color:var(--tali-fg,#111);' +
    'border:1px solid var(--tali-border,#e0e0e0);border-radius:8px;box-shadow:0 6px 30px rgba(0,0,0,.22);' +
    'padding:.7rem .9rem;font-size:.9rem;line-height:1.45;opacity:0;transform:translateY(3px);' +
    'transition:opacity .12s ease,transform .12s ease;pointer-events:none;visibility:hidden;}' +
    '#tali-link-preview.open{opacity:1;transform:none;pointer-events:auto;visibility:visible;}' +
    '#tali-link-preview > :first-child{margin-top:0;}#tali-link-preview > :last-child{margin-bottom:0;}' +
    '#tali-link-preview img{max-width:100%;height:auto;}#tali-link-preview figure{margin:0;}' +
    '#tali-link-preview .tali-lp-head{font-weight:600;}';
  document.head.appendChild(style);

  var card = document.createElement('div');
  card.id = 'tali-link-preview';
  card.setAttribute('role', 'tooltip');
  document.body.appendChild(card);

  var showTimer = null, hideTimer = null, pinned = false, currentLink = null, lastHovered = null;

  // Same-page target: an in-page fragment link (the original behavior).
  function eligibleSame(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#tali-link-preview');
  }
  // Cross-page target: a resolved cross-reference to another page — a `.tali-xref` whose
  // href is `page.html#anchor` (not a bare `#frag`). Its target lives in a different
  // document, so it's previewed from the served hover index, not the current DOM.
  function eligibleCross(a) {
    if (!a || !a.classList.contains('tali-xref')) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) === '#' || href.indexOf('#') < 0) return false;
    return !a.closest('#TOC') && !a.closest('#tali-link-preview');
  }
  function eligible(a) { return eligibleSame(a) || eligibleCross(a); }
  // Clone a node for the card, stripping interactive chrome that has no place in a
  // read-only preview: the heading/caption `#` permalink (taliInitAnchorLinks) and code
  // copy buttons. Without this the cloned `#` shows in the card (and in a heading's
  // textContent as "Title#").
  function cleanClone(node) {
    return taliCloneStripped(node);
  }
  // A preview card is a read-only view appended OUTSIDE #tali-root. Any block cloned into
  // it (same-page target) or parsed from the served snippet (cross-page target) still
  // carries the DEFINING block's source-tracking attrs. Left in place they (a) duplicate a
  // `data-block-id`, breaking the block model's in-DOM uniqueness invariant, and (b) make
  // the card a live Alt-click click-to-source target — a read-only preview must never be
  // one. Strip all three attrs from everything under `scope` so both paths share one rule.
  function stripSourceAttrs(scope) {
    [].forEach.call(scope.querySelectorAll('[data-block-id], [data-sourcepos], [data-source-file]'), function (n) {
      n.removeAttribute('data-block-id');
      n.removeAttribute('data-sourcepos');
      n.removeAttribute('data-source-file');
    });
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
    if (eligibleCross(link)) { showCross(link); return; }
    var id = decodeURIComponent((link.getAttribute('href') || '').slice(1));
    var target = id && document.getElementById(id);
    if (!target) return;
    // Section-heading links get no preview: a heading's title is already visible in the
    // link, so a card adds nothing but noise while reading.
    if (/^H[1-6]$/.test(target.tagName)) return;
    var body = cleanClone(target);
    if (!body || !body.textContent.trim()) return;
    card.innerHTML = '';
    card.appendChild(body);
    stripSourceAttrs(card); // read-only preview: never a click-to-source target, never a duplicate block id
    currentLink = link;
    card.classList.add('open');
    place(link);
  }
  // Lazy-load the served hover index on the first cross-page hover (a <script> load, so it
  // works under file:// like search-index.js), then run `cb` once it is present.
  var hoverFetched = false;
  function loadHoverThen(cb) {
    if (window.TALIESIN_HOVER_INDEX || !window.TALIESIN_HOVER_URL || hoverFetched) { cb(); return; }
    hoverFetched = true;
    var s = document.createElement('script');
    s.src = window.TALIESIN_HOVER_URL;
    s.onload = cb;
    s.onerror = cb;
    document.head.appendChild(s);
  }
  // A snippet's asset/link URLs are stored site-root-relative; prefix them with
  // TALIESIN_SITE_ROOT (this page's up-path to root) so they resolve from any depth.
  function resolveUrls(frag) {
    var root = window.TALIESIN_SITE_ROOT || '';
    if (!root) return;
    function relative(v) {
      return v && v.charAt(0) !== '#' && v.charAt(0) !== '/' &&
        v.indexOf('//') !== 0 && v.indexOf('://') < 0 &&
        v.indexOf('data:') !== 0 && v.indexOf('mailto:') !== 0 && v.indexOf('tel:') !== 0;
    }
    frag.querySelectorAll('img[src]').forEach(function (n) {
      var v = n.getAttribute('src'); if (relative(v)) n.setAttribute('src', root + v);
    });
    frag.querySelectorAll('a[href]').forEach(function (n) {
      var v = n.getAttribute('href'); if (relative(v)) n.setAttribute('href', root + v);
    });
  }
  function showCross(link) {
    loadHoverThen(function () {
      if (lastHovered !== link) return; // pointer moved away while the index loaded
      var href = link.getAttribute('href') || '';
      var anchor = decodeURIComponent(href.slice(href.indexOf('#') + 1));
      var snippet = (window.TALIESIN_HOVER_INDEX || {})[anchor];
      if (!snippet) return;
      // Parse inertly in a <template> (its images don't load until adopted), rebase URLs,
      // strip interactive chrome, then adopt the fragment into the card.
      var tpl = document.createElement('template');
      tpl.innerHTML = snippet;
      resolveUrls(tpl.content);
      tpl.content.querySelectorAll('.tali-anchor, .tali-copy').forEach(function (n) { n.remove(); });
      // The snippet carries the DEFINING page's block ids/sourcepos. Left intact, an Alt-click
      // inside this floating card would resolve click-to-source to the CURRENT page at the
      // foreign block's line — a wrong jump. `stripSourceAttrs` neutralizes it (same rule the
      // same-page path applies).
      stripSourceAttrs(tpl.content);
      if (!tpl.content.textContent.trim()) return;
      card.innerHTML = '';
      card.appendChild(tpl.content);
      clearDescribed(); // a previously-previewed link may still point at the card
      currentLink = link;
      link.setAttribute('aria-describedby', 'tali-link-preview');
      card.classList.add('open');
      place(link);
    });
  }
  function scheduleShow(link) {
    clearTimeout(hideTimer); clearTimeout(showTimer);
    showTimer = setTimeout(function () { show(link); }, 140);
  }
  // The open card describes its link, so a screen reader announces the preview instead of
  // silently painting it. Cleared on every dismissal path and before a different link opens.
  function clearDescribed() {
    if (currentLink) currentLink.removeAttribute('aria-describedby');
  }
  // A pinned card survives mouse-leave / page scroll; only Esc or a click outside
  // releases it (`forceHide`). `hide` is the soft dismiss used by hover/scroll.
  function hide() { clearTimeout(showTimer); if (pinned) return; clearDescribed(); card.classList.remove('open'); currentLink = null; }
  function forceHide() { pinned = false; card.classList.remove('pinned'); clearTimeout(showTimer); clearDescribed(); card.classList.remove('open'); currentLink = null; }
  function scheduleHide() { clearTimeout(hideTimer); hideTimer = setTimeout(hide, 160); }

  document.addEventListener('mouseover', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = a; scheduleShow(a); }
  });
  document.addEventListener('mouseout', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) {
      var to = e.relatedTarget;
      if (to && to.closest && to.closest('#tali-link-preview')) return; // moving into the card
      lastHovered = null;
      scheduleHide();
    }
  });
  // Keyboard parity with hover: a focused citation/xref link surfaces the same card. Without
  // this the preview is mouse-only. Same eligibility + same delays as the mouse path.
  document.addEventListener('focusin', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = a; scheduleShow(a); }
  });
  document.addEventListener('focusout', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = null; scheduleHide(); }
  });
  card.addEventListener('mouseenter', function () { clearTimeout(hideTimer); });
  card.addEventListener('mouseleave', scheduleHide);
  // Scrolling the page dismisses the card, but scrolling INSIDE the card (its own
  // overflow, `max-height:50vh`) must not — otherwise you can never read past the fold.
  window.addEventListener('scroll', function (e) {
    var t = e.target;
    if (t && t.nodeType === 1 && t.closest && t.closest('#tali-link-preview')) return;
    hide();
  }, true);
  // Click the card to PIN it (survives mouse-leave + page scroll) so you can move into
  // an overflowing card and scroll it; Esc or a click outside releases the pin.
  card.addEventListener('click', function () { pinned = true; card.classList.add('pinned'); });
  document.addEventListener('mousedown', function (e) {
    if (!card.classList.contains('open')) return;
    var t = e.target;
    if (t && t.closest && (t.closest('#tali-link-preview') ||
        (currentLink && (t === currentLink || currentLink.contains(t))))) return;
    forceHide();
  });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') forceHide(); });
}

