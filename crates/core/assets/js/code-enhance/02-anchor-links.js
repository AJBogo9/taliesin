// Reveal a `#` on each heading and numbered float (figure / listing / table); activating it
// copies that anchor's canonical deep link (the section/figure permalink, complementing the
// selection toolbar's text-fragment Share). Reader-side, clipboard-only — never writes the
// source. Per-element idempotent (a host already carrying its .tali-anchor is skipped), so it
// survives the live-preview re-mounts; skipped on decks (their own nav). `root` is always the
// whole #tali-root container, so a descendant query suffices.
/** @param {ParentNode | null} [root] */
function taliInitAnchorLinks(root) {
  if (document.querySelector('.tali-deck')) return;
  if (!window.__taliAnchorLive) {
    var l = document.createElement('span');
    l.className = 'tali-sr-only';
    l.setAttribute('aria-live', 'polite');
    document.body.appendChild(l);
    window.__taliAnchorLive = l;
  }
  /** @param {string} msg */
  function announce(msg) { var r = window.__taliAnchorLive; if (!r) return; r.textContent = ''; r.textContent = msg; }
  /** @param {Element | null} host @param {string} id */
  function decorate(host, id) {
    if (!host || !id) return;
    var el = /** @type {HTMLElement} */ (host);
    if (el.dataset.taliAnchored) return;
    el.dataset.taliAnchored = '1';
    var a = document.createElement('a');
    a.className = 'tali-anchor';
    a.href = '#' + id;
    a.setAttribute('aria-label', 'Copy link to this section');
    a.textContent = '#';
    a.addEventListener('click', function () {
      // Don't preventDefault: clicking also sets the URL hash, so the address bar shows the
      // shareable anchor (the page is already here, so there is no jump).
      taliCopyText(taliAnchorUrl(id), function () {
        a.classList.add('tali-anchor-copied');
        a.textContent = '✓';
        announce('Link copied');
        setTimeout(function () { a.classList.remove('tali-anchor-copied'); a.textContent = '#'; }, 1200);
      }, function () { announce('Copy failed'); });
    });
    el.appendChild(a);
  }
  var scope = root || document;
  scope.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]')
    .forEach(function (h) { decorate(h, h.id); });
  // A numbered float carries its id on the wrapper; drop the `#` into its caption.
  scope.querySelectorAll('figcaption, caption').forEach(function (c) {
    var wrap = c.parentElement;
    if (wrap && wrap.id) decorate(c, wrap.id);
  });
  // A theorem carries its id on the wrapper; drop the `#` into its head paragraph.
  scope.querySelectorAll('.tali-theorem[id]').forEach(function (t) {
    var head = t.querySelector('.tali-theorem-head');
    if (head) decorate(head, t.id);
  });
}

