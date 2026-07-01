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

  var showTimer = null, hideTimer = null, pinned = false, currentLink = null;

  function eligible(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#qmd-link-preview');
  }
  // Clone a node for the card, stripping interactive chrome that has no place in a
  // read-only preview: the heading/caption `#` permalink (qmdInitAnchorLinks) and code
  // copy buttons. Without this the cloned `#` shows in the card (and in a heading's
  // textContent as "Title#").
  function cleanClone(node) {
    return qmdCloneStripped(node);
  }
  // Build the preview body for a target element. A heading shows itself plus the
  // following block(s) up to the next heading; anything else is cloned whole.
  function buildPreview(target) {
    if (/^H[1-6]$/.test(target.tagName)) {
      var frag = document.createElement('div');
      var head = document.createElement('div');
      head.className = 'qmd-lp-head';
      head.textContent = cleanClone(target).textContent;
      frag.appendChild(head);
      var n = target.nextElementSibling, added = 0;
      while (n && added < 2 && !/^H[1-6]$/.test(n.tagName) && !n.id) {
        frag.appendChild(cleanClone(n));
        added++; n = n.nextElementSibling;
      }
      return frag;
    }
    return cleanClone(target);
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
    currentLink = link;
    card.classList.add('open');
    place(link);
  }
  function scheduleShow(link) {
    clearTimeout(hideTimer); clearTimeout(showTimer);
    showTimer = setTimeout(function () { show(link); }, 140);
  }
  // A pinned card survives mouse-leave / page scroll; only Esc or a click outside
  // releases it (`forceHide`). `hide` is the soft dismiss used by hover/scroll.
  function hide() { clearTimeout(showTimer); if (pinned) return; card.classList.remove('open'); currentLink = null; }
  function forceHide() { pinned = false; card.classList.remove('pinned'); clearTimeout(showTimer); card.classList.remove('open'); currentLink = null; }
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
  // Scrolling the page dismisses the card, but scrolling INSIDE the card (its own
  // overflow, `max-height:50vh`) must not — otherwise you can never read past the fold.
  window.addEventListener('scroll', function (e) {
    var t = e.target;
    if (t && t.nodeType === 1 && t.closest && t.closest('#qmd-link-preview')) return;
    hide();
  }, true);
  // Click the card to PIN it (survives mouse-leave + page scroll) so you can move into
  // an overflowing card and scroll it; Esc or a click outside releases the pin.
  card.addEventListener('click', function () { pinned = true; card.classList.add('pinned'); });
  document.addEventListener('mousedown', function (e) {
    if (!card.classList.contains('open')) return;
    var t = e.target;
    if (t && t.closest && (t.closest('#qmd-link-preview') ||
        (currentLink && (t === currentLink || currentLink.contains(t))))) return;
    forceHide();
  });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') forceHide(); });
}

