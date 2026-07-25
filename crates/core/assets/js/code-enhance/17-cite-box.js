// --- "Cite this" box (B1) ----------------------------------------------------
// The three citation formats (BibTeX / CSL-JSON / RIS) are serialized server-side
// into <pre data-format> elements; the client only switches the visible one and
// wires copy/download. Idempotent (guarded via dataset), so it is safe to run on
// every (re)mount. With no JS the default BibTeX pane is still shown and selectable.
/** @param {ParentNode | null} [root] */
function taliInitCiteBox(root) {
  var boxes = /** @type {NodeListOf<HTMLElement>} */ ((root || document).querySelectorAll('.tali-cite-this'));
  boxes.forEach(function (box) {
    if (box.dataset.citeInit) return;
    box.dataset.citeInit = '1';

    var tabs = /** @type {HTMLElement[]} */ ([].slice.call(box.querySelectorAll('.tali-cite-tab')));
    var panes = /** @type {HTMLElement[]} */ ([].slice.call(box.querySelectorAll('.tali-cite-out')));
    // const so the null-narrowing survives into the copy/download click closures below.
    const copyBtn = /** @type {HTMLElement | null} */ (box.querySelector('.tali-cite-copy'));
    const dlBtn = /** @type {HTMLElement | null} */ (box.querySelector('.tali-cite-download'));
    if (!tabs.length || !panes.length) return;
    var copyLabel = copyBtn ? copyBtn.textContent : '';

    function activePane() {
      var shown = panes.filter(function (p) { return !p.hidden; });
      return shown[0] || panes[0];
    }
    /** @param {string | undefined} fmt */
    function select(fmt) {
      tabs.forEach(function (t) {
        var on = t.dataset.format === fmt;
        t.setAttribute('aria-selected', String(on));
        // Roving tabindex, same as tabset.js: the tablist is one stop in the tab sequence
        // and the arrows (below) move within it. `cite_this.rs` emits the same shape, so
        // this maintains it rather than establishing it.
        t.tabIndex = on ? 0 : -1;
      });
      panes.forEach(function (p) { p.hidden = p.dataset.format !== fmt; });
    }

    tabs.forEach(function (tab, i) {
      tab.addEventListener('click', function () { select(tab.dataset.format); });
      // ARIA tablist keyboard: Left/Right + Home/End move between formats.
      tab.addEventListener('keydown', function (e) {
        var j = e.key === 'ArrowRight' ? i + 1
          : e.key === 'ArrowLeft' ? i - 1
          : e.key === 'Home' ? 0
          : e.key === 'End' ? tabs.length - 1
          : null;
        if (j === null) return;
        e.preventDefault();
        var next = tabs[(j + tabs.length) % tabs.length];
        select(next.dataset.format);
        next.focus();
      });
    });

    if (copyBtn) {
      copyBtn.addEventListener('click', function () {
        taliCopyText(activePane().textContent || '', function () {
          copyBtn.dataset.copied = 'true';
          copyBtn.textContent = 'Copied';
          setTimeout(function () {
            delete copyBtn.dataset.copied;
            copyBtn.textContent = copyLabel;
          }, 1200);
        });
      });
    }

    if (dlBtn) {
      dlBtn.addEventListener('click', function () {
        var pane = activePane();
        var blob = new Blob([pane.textContent || ''], { type: 'text/plain;charset=utf-8' });
        var url = URL.createObjectURL(blob);
        var a = document.createElement('a');
        a.href = url;
        a.download = pane.dataset.filename || 'citation.txt';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        setTimeout(function () { URL.revokeObjectURL(url); }, 0);
      });
    }
  });
}
