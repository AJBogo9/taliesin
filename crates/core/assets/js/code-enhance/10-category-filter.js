// Native category filter for `listing: { categories: true }`: the server emits a
// chip row (`.tali-cat-filter`) above the card grid; each card's categories are read
// from its `.tali-cat[data-cat]` badges. Clicking a chip — or a category tag on a card — toggles it
// (multi-select, OR semantics); an empty `data-cat` ("All") clears the filter.
// Works in the static build and the live preview; idempotent per filter.
/** @param {ParentNode | null} [root] */
function taliInitCategoryFilter(root) {
  var filters = /** @type {NodeListOf<HTMLElement>} */ ((root || document).querySelectorAll('.tali-cat-filter'));
  filters.forEach(function (filter) {
    if (filter.dataset.taliCat) return;
    filter.dataset.taliCat = '1';
    var wrap = filter.closest('.tali-listing-wrap');
    // const so the null-guard survives into apply()/the click closures below.
    const listing = wrap && wrap.querySelector('.tali-listing');
    if (!wrap || !listing) return;
    // A polite live region announces the result of a filter change; without it the
    // chips silently reorder the page for a screen-reader user. `tali-sr-only` is the
    // same visually-hidden class 03-focus-mode.js uses for its announcements.
    var live = document.createElement('span');
    live.className = 'tali-sr-only';
    live.setAttribute('aria-live', 'polite');
    wrap.appendChild(live);
    var announced = false;
    var selected = /** @type {Set<string | null>} */ (new Set());
    /** @param {Element} card */
    var catsOf = function (card) {
      // Read the card's own category badges (each holds the exact name in data-cat),
      // so a category name containing a comma still matches (a delimited attribute
      // would mis-split it).
      return [...card.querySelectorAll('.tali-cat[data-cat]')].map(function (b) {
        return b.getAttribute('data-cat');
      });
    };
    var apply = function () {
      var shown = 0, total = 0;
      /** @type {NodeListOf<HTMLElement>} */ (listing.querySelectorAll('.tali-card')).forEach(function (card) {
        var show = selected.size === 0 || catsOf(card).some(function (c) { return selected.has(c); });
        // Hide the `<li>`, not the card inside it: the listing is a real list (PA-M3), so
        // hiding only the card would leave an empty list item holding its grid cell open
        // AND still counted by AT as one of the "N items".
        var item = /** @type {HTMLElement} */ (card.closest('.tali-listing-item') || card);
        item.style.display = show ? '' : 'none';
        total++;
        if (show) shown++;
      });
      filter.querySelectorAll('.tali-cat-chip').forEach(function (chip) {
        var c = chip.getAttribute('data-cat');
        var on = c === '' ? selected.size === 0 : selected.has(c);
        chip.classList.toggle('tali-cat-active', on);
        chip.setAttribute('aria-pressed', on ? 'true' : 'false');
      });
      listing.querySelectorAll('.tali-cat[data-cat]').forEach(function (tag) {
        tag.classList.toggle('tali-cat-on', selected.has(tag.getAttribute('data-cat')));
      });
      // The first apply() is the initial paint, not a change: announcing there would
      // speak the unfiltered count at page load. Only real toggles announce.
      if (announced) live.textContent = 'Showing ' + shown + ' of ' + total + ' posts';
      announced = true;
    };
    // Deep-link the active filter so a filtered view is shareable + survives reload +
    // restores on Back. Repeated `?cat=` params (not a delimited list) keep a category
    // name that contains a comma intact, matching how the badges carry exact names.
    // replaceState (not push) means scrolling/toggling doesn't spam history.
    var writeUrl = function () {
      try {
        var params = new URLSearchParams(location.search);
        params.delete('cat');
        selected.forEach(function (c) { if (c != null) params.append('cat', c); });
        var qs = params.toString();
        history.replaceState(history.state, '', location.pathname + (qs ? '?' + qs : '') + location.hash);
      } catch (e) {}
    };
    /** @param {string | null} cat */
    var toggle = function (cat) {
      if (cat === '') selected.clear();
      else if (selected.has(cat)) selected.delete(cat);
      else selected.add(cat);
      apply();
      writeUrl();
    };
    filter.addEventListener('click', function (e) {
      var t = /** @type {Element | null} */ (e.target);
      var chip = t && t.closest('.tali-cat-chip');
      if (chip) toggle(chip.getAttribute('data-cat') || '');
    });
    // A category tag on a card toggles its filter instead of opening the post.
    listing.addEventListener('click', function (e) {
      var t = /** @type {Element | null} */ (e.target);
      var tag = t && t.closest('.tali-cat[data-cat]');
      if (!tag) return;
      e.preventDefault();
      e.stopPropagation();
      toggle(tag.getAttribute('data-cat'));
    });
    // Restore a filter from the URL (?cat=…) on load — ignoring any name that is not a
    // real chip, so a stale/hand-edited param can never hide every card.
    var validCats = new Set();
    filter.querySelectorAll('.tali-cat-chip').forEach(function (chip) {
      var c = chip.getAttribute('data-cat');
      if (c) validCats.add(c);
    });
    try {
      new URLSearchParams(location.search).getAll('cat').forEach(function (c) {
        if (validCats.has(c)) selected.add(c);
      });
    } catch (e) {}
    apply();
  });
}

