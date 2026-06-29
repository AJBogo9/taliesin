// Native category filter for `listing: { categories: true }`: the server emits a
// chip row (`.qmd-cat-filter`) above the card grid; each card's categories are read
// from its `.qmd-cat[data-cat]` badges. Clicking a chip — or a category tag on a card — toggles it
// (multi-select, OR semantics); an empty `data-cat` ("All") clears the filter.
// Works in the static build and the live preview; idempotent per filter.
function qmdInitCategoryFilter(root) {
  (root || document).querySelectorAll('.qmd-cat-filter').forEach(function (filter) {
    if (filter.dataset.qmdCat) return;
    filter.dataset.qmdCat = '1';
    var wrap = filter.closest('.qmd-listing-wrap');
    var listing = wrap && wrap.querySelector('.qmd-listing');
    if (!listing) return;
    var selected = new Set();
    var catsOf = function (card) {
      // Read the card's own category badges (each holds the exact name in data-cat),
      // so a category name containing a comma still matches (a delimited attribute
      // would mis-split it).
      return [...card.querySelectorAll('.qmd-cat[data-cat]')].map(function (b) {
        return b.getAttribute('data-cat');
      });
    };
    var apply = function () {
      listing.querySelectorAll('.qmd-card').forEach(function (card) {
        var show = selected.size === 0 || catsOf(card).some(function (c) { return selected.has(c); });
        card.style.display = show ? '' : 'none';
      });
      filter.querySelectorAll('.qmd-cat-chip').forEach(function (chip) {
        var c = chip.getAttribute('data-cat');
        chip.classList.toggle('qmd-cat-active', c === '' ? selected.size === 0 : selected.has(c));
      });
      listing.querySelectorAll('.qmd-cat[data-cat]').forEach(function (tag) {
        tag.classList.toggle('qmd-cat-on', selected.has(tag.getAttribute('data-cat')));
      });
    };
    var toggle = function (cat) {
      if (cat === '') selected.clear();
      else if (selected.has(cat)) selected.delete(cat);
      else selected.add(cat);
      apply();
    };
    filter.addEventListener('click', function (e) {
      var chip = e.target.closest('.qmd-cat-chip');
      if (chip) toggle(chip.getAttribute('data-cat') || '');
    });
    // A category tag on a card toggles its filter instead of opening the post.
    listing.addEventListener('click', function (e) {
      var tag = e.target.closest('.qmd-cat[data-cat]');
      if (!tag) return;
      e.preventDefault();
      e.stopPropagation();
      toggle(tag.getAttribute('data-cat'));
    });
    apply();
  });
}

