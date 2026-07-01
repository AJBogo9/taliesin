// Reader highlights: select prose, click "Highlight", and the passage is marked. The
// highlight is the reader's own, stored in their localStorage anchored to the block's
// content-hash data-block-id + character offsets within the block's HIGHLIGHTABLE text
// (text nodes, skipping .katex/pre/code so KaTeX's duplicated MathML text and code's
// syntax spans don't corrupt offsets). Re-applied on every mount; exact and survives a
// re-render. Reader-side + read-only: never writes the author's source, never changes a
// block id/sourcepos. Skipped on decks.
function qmdInitHighlights() {
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome
  var KEY = 'qmd-hl:' + location.pathname;

  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function dispatch() { try { window.dispatchEvent(new CustomEvent('qmd:hlchange')); } catch (e) {} }

  // A text node is non-highlightable if it sits inside math/code within the block.
  function skip(node, block) {
    var p = node.parentNode;
    while (p && p !== block) {
      if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
          (p.classList && p.classList.contains('katex')))) return true;
      p = p.parentNode;
    }
    return false;
  }
  function textNodes(block) {
    var out = [], w = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) { if (!skip(n, block)) out.push(n); }
    return out;
  }
  function blockOf(node) {
    var el = node.nodeType === 1 ? node : node.parentNode;
    return el && el.closest ? el.closest('[data-block-id]') : null;
  }
  // Character offset of (node, nodeOffset) within the block's highlightable text, or -1.
  function offsetOf(block, node, nodeOffset) {
    var nodes = textNodes(block), pos = 0;
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i] === node) return pos + nodeOffset;
      pos += nodes[i].nodeValue.length;
    }
    return -1;
  }
  // Wrap [s, e) of the block's highlightable text in <mark> elements (one per text node).
  function applyOne(block, s, e, tag) {
    if (e <= s) return;
    var nodes = textNodes(block), pos = 0, segs = [], i;
    for (i = 0; i < nodes.length; i++) {
      var node = nodes[i], L = node.nodeValue.length, ns = pos, ne = pos + L;
      var os = Math.max(s, ns), oe = Math.min(e, ne);
      if (os < oe) segs.push({ node: node, ls: os - ns, le: oe - ns });
      pos = ne;
    }
    segs.forEach(function (seg) {
      var t = seg.node;
      if (seg.ls > 0) t = t.splitText(seg.ls);
      if (seg.le - seg.ls < t.nodeValue.length) t.splitText(seg.le - seg.ls);
      var mark = document.createElement('mark');
      mark.className = 'qmd-userhl';
      mark.setAttribute('data-hl', tag);
      t.parentNode.insertBefore(mark, t);
      mark.appendChild(t);
    });
  }
  function unwrapAll() {
    var marks = [].slice.call(document.querySelectorAll('mark.qmd-userhl')), blocks = [];
    marks.forEach(function (m) {
      var b = m.closest('[data-block-id]'); if (b && blocks.indexOf(b) < 0) blocks.push(b);
      m.replaceWith(document.createTextNode(m.textContent));
    });
    blocks.forEach(function (b) { b.normalize(); });
  }
  function applyAll() {
    unwrapAll();
    load().forEach(function (h) {
      var sel = '[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(h.id) : h.id) + '"]';
      var block = document.querySelector(sel);
      if (block) applyOne(block, h.s, h.e, h.id + ':' + h.s + ':' + h.e);
    });
  }

  if (!window.__qmdHL) {
    window.__qmdHL = true;

    // The selection toolbar: a bar holding Copy / Quote / Share link (clipboard-only) plus the
    // Highlight button (which keeps its exact behaviour, now as one child).
    var bar = document.createElement('div');
    bar.className = 'qmd-seltools';
    bar.setAttribute('role', 'toolbar');
    bar.setAttribute('aria-label', 'Selection actions');
    bar.hidden = true;
    var live = document.createElement('span');
    live.className = 'qmd-sr-only';
    live.setAttribute('aria-live', 'polite');

    var mode = null, pending = null, pendingTag = null;
    function announce(msg) { live.textContent = ''; live.textContent = msg; }

    // A clipboard-action child: clicking runs `run(done)`; `done(msg)` flashes the label.
    function action(label, run) {
      var b = document.createElement('button');
      b.type = 'button'; b.className = 'qmd-hl-action';
      b.textContent = label;
      var t = null;
      b.__reset = function () { if (t) { clearTimeout(t); t = null; } b.textContent = label; };
      b.addEventListener('click', function () {
        run(function (msg) {
          if (t) clearTimeout(t);
          b.textContent = msg; announce(msg);
          t = setTimeout(function () { b.textContent = label; t = null; }, 1200);
        });
      });
      return b;
    }
    var copyBtn = action('Copy', function (done) {
      qmdCopyText(pending.text, function () { done('Copied'); }, function () { done('Copy failed'); });
    });
    var quoteBtn = action('Quote', function (done) {
      var url = qmdBuildTextFragmentUrl(pending.text) || location.href;
      var label = (document.title || location.href).replace(/[\[\]()\\]/g, '\\$&');
      var md = pending.text.split(/\r?\n/).map(function (l) { return '> ' + l; }).join('\n') +
        '\n>\n> -- [' + label + '](<' + url + '>)'; // angle-bracket dest tolerates parens in url
      qmdCopyText(md, function () { done('Quote copied'); }, function () { done('Copy failed'); });
    });
    var shareBtn = action('Share link', function (done) {
      var url = qmdBuildTextFragmentUrl(pending.text);
      if (!url) { done('Nothing to link'); return; }
      qmdCopyText(url, function () {
        done('Link copied');
        if (location.protocol === 'file:') announce('Link copied; the highlight opens when served over http or https');
      }, function () { done('Copy failed'); });
    });
    // Cite: a BibTeX @misc entry that deep-links to the selection (drop straight into a .bib).
    var citeBtn = action('Cite', function (done) {
      var url = qmdBuildTextFragmentUrl(pending.text) || location.href;
      qmdCopyText(qmdBuildBibtex(document.title, url, new Date()), function () {
        done('Cited');
        if (location.protocol === 'file:') announce('Citation copied; the deep link opens when served over http or https');
      }, function () { done('Copy failed'); });
    });
    var extras = [copyBtn, quoteBtn, shareBtn, citeBtn];

    var btn = document.createElement('button'); // the Highlight / Remove-highlight child
    btn.type = 'button';
    btn.className = 'qmd-hl-action';
    extras.forEach(function (b) { bar.appendChild(b); });
    bar.appendChild(btn);
    document.body.appendChild(bar);
    document.body.appendChild(live);

    function resetExtras() { extras.forEach(function (b) { if (b.__reset) b.__reset(); }); }
    function hideBtn() { resetExtras(); bar.hidden = true; mode = null; pending = null; pendingTag = null; }
    function placeBtn(rect) {
      bar.hidden = false;                       // un-hide first so offsetWidth is measurable
      var w = bar.offsetWidth;
      var left = rect.left + rect.width / 2 - w / 2;
      bar.style.left = Math.max(8, Math.min(left, window.innerWidth - w - 8)) + 'px';
      bar.style.top = (rect.top - 38 >= 8 ? rect.top - 38 : rect.bottom + 8) + 'px';
    }
    function onSelect() {
      var sel = window.getSelection();
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) { if (mode === 'add') hideBtn(); return; }
      var r = sel.getRangeAt(0);
      if (bar.contains(r.startContainer)) return;
      var b1 = blockOf(r.startContainer), b2 = blockOf(r.endContainer);
      // A single-block prose selection can be HIGHLIGHTED (its offsets anchor to one
      // block-id). Any OTHER selection — spanning two blocks, or in code/math where the
      // offset walk skips — still gets the clipboard actions (Copy/Quote/Share/Cite),
      // which only need `pending.text`; it just doesn't get the Highlight button.
      var single = !!(b1 && b1 === b2 && !skip(r.startContainer, b1) && !skip(r.endContainer, b1));
      var s = -1, e = -1, text = '';
      if (single) {
        s = offsetOf(b1, r.startContainer, r.startOffset);
        e = offsetOf(b1, r.endContainer, r.endOffset);
        if (s < 0 || e < 0 || e <= s) single = false;
        else text = textNodes(b1).map(function (n) { return n.nodeValue; }).join('').slice(s, e);
      }
      if (!single) {
        text = (sel.toString() || '').trim();
        if (!text) { if (mode === 'add') hideBtn(); return; } // nothing usable to copy/quote
      }
      mode = 'add';
      pending = single
        ? { id: b1.getAttribute('data-block-id'), s: s, e: e, text: text }
        : { id: null, s: -1, e: -1, text: text };
      pendingTag = null;
      resetExtras();
      extras.forEach(function (b) { b.hidden = false; });
      btn.hidden = !single;                 // Highlight needs a single-block anchor
      btn.textContent = 'Highlight';
      placeBtn(r.getBoundingClientRect());
    }
    bar.addEventListener('mousedown', function (e) { e.preventDefault(); }); // keep the selection on any click
    btn.addEventListener('click', function () {
      if (mode === 'add' && pending) {
        var list = load();
        if (!list.some(function (h) { return h.id === pending.id && h.s === pending.s && h.e === pending.e; })) {
          list.push({ id: pending.id, s: pending.s, e: pending.e }); save(list); // keep the id:s:e schema
        }
        var sel = window.getSelection(); if (sel) sel.removeAllRanges();
        dispatch();
      } else if (mode === 'remove' && pendingTag) {
        save(load().filter(function (h) { return (h.id + ':' + h.s + ':' + h.e) !== pendingTag; }));
        dispatch();
      }
      hideBtn();
    });
    document.addEventListener('mouseup', function () { setTimeout(onSelect, 0); });
    document.addEventListener('keyup', function (e) {
      if (e.shiftKey || e.key === 'ArrowLeft' || e.key === 'ArrowRight') setTimeout(onSelect, 0);
    });
    // Touch / pen: a long-press selection never emits `mouseup`, so mirror it on `pointerup`
    // (mouse is already covered above — skip it here to avoid a double onSelect).
    document.addEventListener('pointerup', function (e) {
      if (e.pointerType && e.pointerType !== 'mouse') setTimeout(onSelect, 0);
    });
    // Safety net for mobile: dragging the selection handles after the press often surfaces only
    // `selectionchange`, never a fresh pointer event. Debounced so the bar settles (not flickers)
    // once the selection stops moving; onSelect itself guards single-block prose + bar-internal
    // selections, so a stray change can't mis-place the toolbar.
    var selTimer = null;
    document.addEventListener('selectionchange', function () {
      if (selTimer) clearTimeout(selTimer);
      selTimer = setTimeout(function () { selTimer = null; onSelect(); }, 350);
    });
    document.addEventListener('click', function (e) {
      var m = e.target.closest && e.target.closest('mark.qmd-userhl');
      if (m) {
        mode = 'remove'; pendingTag = m.getAttribute('data-hl'); pending = null;
        resetExtras();
        extras.forEach(function (b) { b.hidden = true; });
        btn.hidden = false;                 // ensure the Remove button shows (a prior cross-block selection may have hidden it)
        btn.textContent = 'Remove highlight';
        placeBtn(m.getBoundingClientRect());
        e.stopPropagation();
        return;
      }
      if (!bar.contains(e.target) && window.getSelection().isCollapsed) hideBtn();
    });
    window.addEventListener('scroll', function () { if (mode) hideBtn(); }, { passive: true });
    window.addEventListener('qmd:hlchange', applyAll);
  }

  applyAll();
}

