// My highlights: an index + Markdown export over the reader's highlights (qmdInitHighlights).
// When the page has any highlights, a "N highlights" button (bottom-left) opens a panel that
// lists them, jumps to one, removes one, or exports them all as Markdown into a selectable
// textarea (and best-effort to the clipboard). Reader-side + read-only: reads the same
// localStorage; coordinates with the highlighter via the qmd:hlchange event. Skipped on decks.
function qmdInitHighlightIndex() {
  if (document.querySelector('.tali-deck')) return;
  if (!window.qmdReaderMenu) return;          // need the menu host
  if (window.__qmdHLIndex) return;
  window.__qmdHLIndex = true;

  var KEY = 'tali-hl:' + location.pathname;
  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function changed() { try { window.dispatchEvent(new CustomEvent('qmd:hlchange')); } catch (e) {} }

  // Same highlightable-text rule as the highlighter, so offsets resolve identically.
  function skip(node, block) {
    var p = node.parentNode;
    while (p && p !== block) {
      if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
          (p.classList && p.classList.contains('katex')))) return true;
      p = p.parentNode;
    }
    return false;
  }
  function blockText(block) {
    var w = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, null), n, s = '';
    while ((n = w.nextNode())) { if (!skip(n, block)) s += n.nodeValue; }
    return s;
  }
  function findBlock(id) {
    return document.querySelector('[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]');
  }
  function textOf(h) { var b = findBlock(h.id); return b ? blockText(b).slice(h.s, h.e) : null; }
  function flash(block) { block.classList.remove('tali-flash'); void block.offsetWidth; block.classList.add('tali-flash'); }

  var body = document.createElement('div');
  function render() {
    while (body.firstChild) body.removeChild(body.firstChild);
    var ul = document.createElement('ul'); ul.className = 'tali-hlx-list';
    load().forEach(function (h) {
      var t = textOf(h); if (t == null) return; // block gone (orphaned highlight)
      var li = document.createElement('li');
      var go = document.createElement('button');
      go.type = 'button'; go.className = 'tali-hlx-go';
      go.textContent = t.length > 90 ? t.slice(0, 90) + '…' : t;
      go.addEventListener('click', function () {
        var b = findBlock(h.id);
        if (b) { window.qmdReaderMenu.close(); b.scrollIntoView({ block: 'center', behavior: 'smooth' }); flash(b); }
      });
      var rm = document.createElement('button');
      rm.type = 'button'; rm.className = 'tali-hlx-rm';
      rm.setAttribute('aria-label', 'Remove'); rm.textContent = '×';
      rm.addEventListener('click', function () {
        var tag = h.id + ':' + h.s + ':' + h.e;
        save(load().filter(function (x) { return (x.id + ':' + x.s + ':' + x.e) !== tag; }));
        changed();
      });
      li.appendChild(go); li.appendChild(rm); ul.appendChild(li);
    });
    body.appendChild(ul);

    var actions = document.createElement('div'); actions.className = 'tali-hlx-actions';
    var exp = document.createElement('button');
    exp.type = 'button'; exp.className = 'tali-hlx-export'; exp.textContent = 'Export as Markdown';
    var ta = document.createElement('textarea');
    ta.className = 'tali-hlx-out'; ta.readOnly = true; ta.hidden = true;
    ta.setAttribute('aria-label', 'Highlights as Markdown');
    exp.addEventListener('click', function () {
      var md = '# ' + (document.title || 'Highlights') + '\n\n' + location.href + '\n\n';
      load().forEach(function (h) { var t = textOf(h); if (t != null) md += '> ' + t.replace(/\s+/g, ' ').trim() + '\n\n'; });
      ta.value = md; ta.hidden = false; ta.focus(); ta.select();
      // Best-effort clipboard; the visible textarea is the reliable path. (Not qmdCopyText: its
      // execCommand fallback would steal the selection from the textarea in an insecure context.)
      try { if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(md); } catch (e) {}
    });
    actions.appendChild(exp); actions.appendChild(ta); body.appendChild(actions);
  }

  var section = window.qmdReaderMenu.addSection('Highlights', body, render);
  function refresh() {
    var n = load().filter(function (h) { return textOf(h) != null; }).length;
    section.setVisible(n > 0);
    render();
  }
  window.addEventListener('qmd:hlchange', refresh);
  refresh();
}

