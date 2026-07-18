// --- "Cite this" box (B1) ----------------------------------------------------
// The three citation formats (BibTeX / CSL-JSON / RIS) are serialized server-side
// into <pre data-format> elements; the client only switches the visible one and
// wires copy/download. Idempotent (guarded via dataset), so it is safe to run on
// every (re)mount. With no JS the default BibTeX pane is still shown and selectable.
function taliInitCiteBox(root) {
  (root || document).querySelectorAll('.tali-cite-this').forEach(function (box) {
    if (box.dataset.citeInit) return;
    box.dataset.citeInit = '1';

    var tabs = [].slice.call(box.querySelectorAll('.tali-cite-tab'));
    var panes = [].slice.call(box.querySelectorAll('.tali-cite-out'));
    var copyBtn = box.querySelector('.tali-cite-copy');
    var dlBtn = box.querySelector('.tali-cite-download');
    if (!tabs.length || !panes.length) return;
    var copyLabel = copyBtn ? copyBtn.textContent : '';

    function activePane() {
      var shown = panes.filter(function (p) { return !p.hidden; });
      return shown[0] || panes[0];
    }
    function select(fmt) {
      tabs.forEach(function (t) {
        t.setAttribute('aria-selected', String(t.dataset.format === fmt));
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
        taliCopyText(activePane().textContent, function () {
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
        var blob = new Blob([pane.textContent], { type: 'text/plain;charset=utf-8' });
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
