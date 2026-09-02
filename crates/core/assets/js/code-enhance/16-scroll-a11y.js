
// --- Keyboard-scrollable overflow (WCAG 2.1.1) -------------------------------
// A wide `<pre>` or `<table>` is a horizontal scroll container, but a mouse-only
// scroll region is a keyboard trap for content: a keyboard user can't reach the
// clipped columns. Making the container focusable (`tabindex="0"`) lets the arrow
// keys scroll it, and on a `<pre>` `role="region"` + a name announces it. We only tag
// containers that ACTUALLY overflow (so non-wide blocks don't clutter the tab order or
// the landmark list), and re-evaluate on resize. Idempotent: keyed on data-scroll-a11y.
//
// The role goes on the `<pre>` ONLY. An explicit `role` REPLACES an element's implicit
// one, and `<pre>` has none worth keeping (`generic`), while `<table>` has `table`: the
// context role its own `<tr>`/`<th>`/`<td>` descendants require. `role="region"` on the
// table therefore announced "scrollable table region" and then handed over flat text,
// with no row/column count, no header association and no table-navigation keys, on
// exactly the wide tables where a reader needs them most (live on the guide's
// front-matter reference). The affordance a keyboard user actually needs is the
// `tabindex`, and a table already announces itself as a table, so nothing is lost by
// leaving its role alone.
(function () {
  if (!window.taliEnhancers) return;
  /** @param {Element} el */
  function isTable(el) {
    return el.tagName === 'TABLE';
  }
  /** @param {Element} el */
  function label(el) {
    return isTable(el) ? 'Scrollable table' : 'Scrollable code';
  }
  /** @param {Element} el */
  function sync(el) {
    var overflows = el.scrollWidth - el.clientWidth > 1;
    var tagged = el.hasAttribute('data-scroll-a11y');
    if (overflows && !tagged) {
      // Don't clobber an author/runtime tabindex or role already present.
      if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '0');
      if (!isTable(el) && !el.hasAttribute('role')) el.setAttribute('role', 'region');
      // Never over a `<caption>` either: an `aria-label` OUTRANKS the caption in the
      // accessible-name computation, so labelling a captioned table would replace the
      // author's own "Table 1: ..." with a scroll affordance. A caption only names the
      // table once the `table` role survives, which is why this guard arrives with the
      // one above and not before it.
      if (!el.hasAttribute('aria-label') && !(isTable(el) && el.querySelector(':scope > caption'))) {
        el.setAttribute('aria-label', label(el));
      }
      el.setAttribute('data-scroll-a11y', '');
    } else if (!overflows && tagged) {
      // Shrunk back within its box: undo only what we added.
      if (el.getAttribute('tabindex') === '0') el.removeAttribute('tabindex');
      if (el.getAttribute('role') === 'region') el.removeAttribute('role');
      if (el.getAttribute('aria-label') === label(el)) el.removeAttribute('aria-label');
      el.removeAttribute('data-scroll-a11y');
    }
  }
  /** @type {string[]} */
  var roots = [];
  /** @param {ParentNode | null} [root] */
  function scan(root) {
    (root || document).querySelectorAll('pre, table').forEach(function (el) {
      // A mermaid diagram gets its own `overflow-x: auto` in base.css and is skipped
      // here to avoid double-tagging it; only tag the actual scroll box.
      if (el.classList && el.classList.contains('mermaid')) return;
      sync(el);
    });
  }
  var raf = 0;
  function onResize() {
    if (raf) return;
    raf = requestAnimationFrame(function () { raf = 0; scan(document); });
  }
  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    scan(root);
    if (roots.indexOf('resize') === -1) {
      roots.push('resize');
      window.addEventListener('resize', onResize, { passive: true });
    }
  }
  window.taliEnhancers.register(enhance);
})();
