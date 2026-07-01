// Reader bookmarks: section markers. Hovering a heading reveals a star toggle in its left
// margin; clicking bookmarks that section. Bookmarked headings keep a persistent star, and
// the reader menu gathers them into a "Bookmarks" list (jump / remove). BOOK-WIDE: on a book
// every chapter shares one bookmark store (keyed by the book root), so the list shows marks
// from every chapter and jumps across pages; a single doc / website stays per-page. Bookmarks
// live in the reader's own localStorage; an entry is {page, anchor, block, label}. Reader-side
// + read-only. Decks skipped. Markers re-apply each pass; the toggle + listeners + menu section
// are set up once. (Note: some browsers isolate localStorage per file path, so "book-wide"
// spans chapters only when served over http(s) — see the portability audit.)
function qmdInitBookmarks() {
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome

  // A book shares one store across its chapters, keyed by the resolved book-root URL from the
  // topbar brand link (stable per book, present on every chapter); anything else stays per-page.
  var brand = document.querySelector('.qmd-book-brand');
  var KEY = brand && brand.href ? 'qmd-bm-book:' + brand.href : 'qmd-bm:' + location.pathname;
  function thisPage() { return location.origin + location.pathname; }

  function load() { try { return JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { return []; } }
  function save(list) { try { localStorage.setItem(KEY, JSON.stringify(list)); } catch (e) {} }
  function dispatch() { try { window.dispatchEvent(new CustomEvent('qmd:bmchange')); } catch (e) {} }
  function onThisPage(e) { return !!e && e.page === thisPage(); }
  function indexOfBlock(list, block) {
    for (var i = 0; i < list.length; i++) if (onThisPage(list[i]) && list[i].block === block) return i;
    return -1;
  }
  function has(block) { return indexOfBlock(load(), block) !== -1; }
  function findBlock(id) {
    return document.querySelector('[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]');
  }
  function headingFrom(t) {
    var h = t && t.closest ? t.closest('h1,h2,h3,h4,h5,h6') : null;
    return (h && h.getAttribute('data-block-id')) ? h : null;
  }
  // A short page label ("using/code.html" -> "code") for a cross-chapter entry.
  function pageName(url) {
    try { return (new URL(url).pathname.split('/').pop() || '').replace(/\.html?$/, ''); } catch (e) { return ''; }
  }
  var HEADS = 'h1[data-block-id],h2[data-block-id],h3[data-block-id],h4[data-block-id],h5[data-block-id],h6[data-block-id]';

  // Re-apply the persistent margin star to every bookmarked heading ON THIS PAGE (idempotent).
  function applyMarkers() {
    var list = load();
    document.querySelectorAll(HEADS).forEach(function (h) {
      h.classList.toggle('qmd-bookmarked', indexOfBlock(list, h.getAttribute('data-block-id')) !== -1);
    });
  }

  if (!window.__qmdBookmarks) {
    window.__qmdBookmarks = true;

    var toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'qmd-bm-toggle';
    toggle.hidden = true;
    toggle.setAttribute('aria-label', 'Bookmark this section');
    document.body.appendChild(toggle);

    var active = null, overToggle = false, hideTimer = null;
    function show(h) {
      active = h; clearTimeout(hideTimer);
      var on = has(h.getAttribute('data-block-id'));
      toggle.textContent = on ? '★' : '☆'; // filled / outline star
      toggle.setAttribute('aria-pressed', on ? 'true' : 'false');
      toggle.title = on ? 'Remove bookmark' : 'Bookmark this section';
      var r = h.getBoundingClientRect();
      toggle.style.top = (window.pageYOffset + r.top + r.height / 2 - 13) + 'px';
      toggle.style.left = Math.max(2, window.pageXOffset + r.left - 30) + 'px';
      toggle.hidden = false;
    }
    function scheduleHide() {
      clearTimeout(hideTimer);
      hideTimer = setTimeout(function () { if (!overToggle) toggle.hidden = true; }, 140);
    }
    // Hover reveal is desktop-only: a touch device has no hover, so `mouseover` either never
    // fires or fires once on tap and sticks. Gate it behind `(hover: hover)` and give touch its
    // own tap affordance below.
    var canHover = !window.matchMedia || window.matchMedia('(hover: hover)').matches;
    if (canHover) {
      document.addEventListener('mouseover', function (e) {
        var h = headingFrom(e.target);
        if (h) show(h);
        else if (e.target !== toggle) scheduleHide();
      });
      toggle.addEventListener('mouseenter', function () { overToggle = true; clearTimeout(hideTimer); });
      toggle.addEventListener('mouseleave', function () { overToggle = false; scheduleHide(); });
    }
    // Touch / pen: tapping a heading reveals its star (so a phone reader can bookmark it); the
    // toggle's own click then flips it. Tapping elsewhere (and not on the star) dismisses the
    // star. Gated to non-mouse pointers so it never double-fires with the hover path on desktop.
    document.addEventListener('pointerup', function (e) {
      if (e.pointerType === 'mouse') return;
      var h = headingFrom(e.target);
      if (h) { show(h); return; }
      if (e.target !== toggle && (!toggle.contains || !toggle.contains(e.target))) toggle.hidden = true;
    });
    toggle.addEventListener('click', function () {
      if (!active) return;
      var block = active.getAttribute('data-block-id'), list = load(), i = indexOfBlock(list, block);
      if (i === -1) {
        list.push({
          page: thisPage(),
          anchor: active.id || '',
          block: block,
          label: (active.textContent || '').replace(/\s+/g, ' ').trim().replace(/#$/, '').trim(),
        });
      } else {
        list.splice(i, 1);
      }
      save(list); dispatch(); show(active);
    });
    window.addEventListener('scroll', function () { if (!toggle.hidden) toggle.hidden = true; }, { passive: true });
    window.addEventListener('qmd:bmchange', applyMarkers);

    // The Bookmarks list in the reader menu (jump / remove). Degrades to just the margin
    // stars + hover toggle if the menu host is absent. A cross-chapter entry jumps to its
    // page; a current-page entry scrolls + flashes in place.
    if (window.qmdReaderMenu) {
      var body = document.createElement('div');
      function flash(b) { b.classList.remove('qmd-flash'); void b.offsetWidth; b.classList.add('qmd-flash'); }
      function renderList() {
        while (body.firstChild) body.removeChild(body.firstChild);
        var ul = document.createElement('ul'); ul.className = 'qmd-hlx-list';
        load().forEach(function (e) {
          var current = onThisPage(e);
          var block = current ? findBlock(e.block) : null;
          if (current && !block) return; // orphaned on the current page
          var li = document.createElement('li');
          var go = document.createElement('button');
          go.type = 'button'; go.className = 'qmd-hlx-go';
          var label = e.label || (block ? block.textContent : '') || 'Bookmark';
          label = label.replace(/\s+/g, ' ').trim().replace(/#$/, '').trim();
          if (label.length > 56) label = label.slice(0, 56) + '…';
          go.textContent = current ? label : label + ' · ' + (pageName(e.page) || 'other');
          go.addEventListener('click', function () {
            window.qmdReaderMenu.close();
            if (current && block) { block.scrollIntoView({ block: 'center', behavior: 'smooth' }); flash(block); }
            else { location.href = e.page + (e.anchor ? '#' + e.anchor : ''); }
          });
          var rm = document.createElement('button');
          rm.type = 'button'; rm.className = 'qmd-hlx-rm';
          rm.setAttribute('aria-label', 'Remove bookmark'); rm.textContent = '×';
          rm.addEventListener('click', function () {
            save(load().filter(function (x) { return !(x.page === e.page && x.block === e.block); }));
            dispatch();
          });
          li.appendChild(go); li.appendChild(rm); ul.appendChild(li);
        });
        body.appendChild(ul);
      }
      var section = window.qmdReaderMenu.addSection('Bookmarks', body, renderList);
      function refresh() {
        // Visible when there is any book-wide entry (a current-page one must still resolve).
        var any = load().some(function (e) { return !onThisPage(e) || !!findBlock(e.block); });
        section.setVisible(any);
        renderList();
      }
      window.addEventListener('qmd:bmchange', refresh);
      refresh();
    }
  }

  applyMarkers();
}
