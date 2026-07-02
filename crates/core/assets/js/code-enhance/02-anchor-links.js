// Reveal a `#` on each heading and numbered float (figure / listing / table); activating it
// copies that anchor's canonical deep link (the section/figure permalink, complementing the
// selection toolbar's text-fragment Share). Reader-side, clipboard-only — never writes the
// source. Per-element idempotent (a host already carrying its .tali-anchor is skipped), so it
// survives the live-preview re-mounts; skipped on decks (their own nav). `root` is always the
// whole #tali-root container, so a descendant query suffices.
function taliInitAnchorLinks(root) {
  if (document.querySelector('.tali-deck')) return;
  if (!window.__qmdAnchorLive) {
    var l = document.createElement('span');
    l.className = 'tali-sr-only';
    l.setAttribute('aria-live', 'polite');
    document.body.appendChild(l);
    window.__qmdAnchorLive = l;
  }
  function announce(msg) { var r = window.__qmdAnchorLive; r.textContent = ''; r.textContent = msg; }
  function decorate(host, id) {
    if (!host || !id || host.dataset.taliAnchored) return;
    host.dataset.taliAnchored = '1';
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
    host.appendChild(a);
  }
  var scope = root || document;
  [].forEach.call(scope.querySelectorAll('h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]'),
    function (h) { decorate(h, h.id); });
  // A numbered float carries its id on the wrapper; drop the `#` into its caption.
  [].forEach.call(scope.querySelectorAll('figcaption, caption'), function (c) {
    var wrap = c.parentElement;
    if (wrap && wrap.id) decorate(c, wrap.id);
  });
  // A theorem carries its id on the wrapper; drop the `#` into its head paragraph.
  [].forEach.call(scope.querySelectorAll('.tali-theorem[id]'), function (t) {
    var head = t.querySelector('.tali-theorem-head');
    if (head) decorate(head, t.id);
  });
}

