
// --- Keyboard-scrollable overflow (WCAG 2.1.1) -------------------------------
// A wide `<pre>` or `<table>` is a horizontal scroll container, but a mouse-only
// scroll region is a keyboard trap for content: a keyboard user can't reach the
// clipped columns. Making the container focusable (`tabindex="0"`) lets the arrow
// keys scroll it, and `role="region"` + a name announces it. We only tag containers
// that ACTUALLY overflow (so non-wide blocks don't clutter the tab order or the
// landmark list), and re-evaluate on resize. Idempotent: keyed on data-scroll-a11y.
(function () {
  if (!window.taliEnhancers) return;
  /** @param {Element} el */
  function label(el) {
    return el.tagName === 'TABLE' ? 'Scrollable table' : 'Scrollable code';
  }
  /** @param {Element} el */
  function sync(el) {
    // A scroll container inside the lightbox / a deck manages its own focus.
    var overflows = el.scrollWidth - el.clientWidth > 1;
    var tagged = el.hasAttribute('data-scroll-a11y');
    if (overflows && !tagged) {
      // Don't clobber an author/runtime tabindex or role already present.
      if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '0');
      if (!el.hasAttribute('role')) el.setAttribute('role', 'region');
      if (!el.hasAttribute('aria-label')) el.setAttribute('aria-label', label(el));
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
      // A mermaid diagram / an inner code <pre> already handled by the lightbox
      // stays as-is; only tag the actual scroll box.
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
