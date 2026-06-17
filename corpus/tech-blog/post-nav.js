(async () => {
  try {
    const path = window.location.pathname;
    if (!path.includes('/posts/')) return;

    const href = path.endsWith('/')     ? path + 'index.html'
               : path.endsWith('.html') ? path
               :                          path + '/index.html';

    const [listings, search] = await Promise.all([
      fetch('/listings.json').then(r => r.json()),
      fetch('/search.json').then(r => r.json()),
    ]);

    const blog = listings.find(l => l.listing === '/blog.html');
    if (!blog) return;

    const items = blog.items;
    const idx = items.indexOf(href);
    if (idx === -1) return;

    const titles = {};
    search.forEach(s => {
      if (s.section === '' && s.href.startsWith('posts/')) {
        titles['/' + s.href] = s.title;
      }
    });

    const older = idx < items.length - 1 ? items[idx + 1] : null;
    const newer = idx > 0               ? items[idx - 1] : null;

    const makeLink = (itemHref, direction, label) => {
      const a = document.createElement('a');
      a.href = itemHref;
      a.className = `post-nav-item post-nav-${direction}`;
      const dir = document.createElement('span');
      dir.className = 'post-nav-direction';
      dir.textContent = label;
      const title = document.createElement('span');
      title.className = 'post-nav-title';
      title.textContent = titles[itemHref] ?? (direction === 'prev' ? 'Previous post' : 'Next post');
      a.append(dir, title);
      return a;
    };

    const nav = document.createElement('nav');
    nav.setAttribute('aria-label', 'Post navigation');
    nav.className = 'post-nav';

    if (older) nav.appendChild(makeLink(older, 'prev', '← Older'));

    const allLink = document.createElement('a');
    allLink.href = '/blog.html';
    allLink.className = 'post-nav-all';
    allLink.textContent = 'All posts';
    nav.appendChild(allLink);

    if (newer) nav.appendChild(makeLink(newer, 'next', 'Newer →'));

    const reuse = document.getElementById('quarto-reuse');
    const appendix = document.getElementById('quarto-appendix');
    if (reuse) reuse.before(nav);
    else if (appendix) appendix.before(nav);
    else document.querySelector('main')?.appendChild(nav);
  } catch (_) {
    // progressive enhancement — fail silently
  }
})();

(() => {
  try {
    const categoryList = document.querySelector('#quarto-margin-sidebar .quarto-listing-category');
    if (!categoryList) return;

    const wrapper = document.createElement('div');
    wrapper.className = 'mobile-category-filter';

    const title = document.createElement('p');
    title.className = 'mobile-category-title';
    title.textContent = 'Categories';

    const clone = categoryList.cloneNode(true);
    wrapper.append(title, clone);
    document.querySelector('.quarto-listing')?.before(wrapper);

    const selected = new Set();

    const updateActive = () => {
      clone.querySelectorAll('.category').forEach(pill => {
        const cat = pill.getAttribute('data-category');
        pill.classList.toggle('active',
          cat === '' ? selected.size === 0 : selected.has(cat)
        );
      });
    };

    const updateCardHighlights = () => {
      document.querySelectorAll('.listing-category').forEach(div => {
        const match = div.getAttribute('onclick')?.match(/'([^']+)'/);
        if (!match) return;
        const active = selected.size > 0 && selected.has(match[1]);
        div.classList.toggle('active', active);
      });
    };

    const applyFilter = () => {
      const listingId = document.querySelector('[id^="listing-"]')?.id;
      const list = window['quarto-listings']?.[listingId];
      if (!list) return;

      if (selected.size === 0) {
        list.filter();
      } else {
        list.filter(item => {
          const raw = item.values().categories;
          if (!raw) return false;
          const postCats = decodeURIComponent(atob(raw)).split(',');
          return [...selected].some(s => postCats.includes(decodeURIComponent(atob(s))));
        });
      }
    };

    const setupPills = () => {
      const listingId = document.querySelector('[id^="listing-"]')?.id;
      const list = window['quarto-listings']?.[listingId];
      list?.on('updated', updateCardHighlights);

      clone.querySelectorAll('.category').forEach(pill => {
        pill.onclick = null;
        pill.addEventListener('click', e => {
          e.preventDefault();
          e.stopImmediatePropagation();
          pill.onclick = null;
          const cat = pill.getAttribute('data-category');
          if (cat === '') {
            selected.clear();
          } else {
            selected.has(cat) ? selected.delete(cat) : selected.add(cat);
          }
          updateActive();
          updateCardHighlights();
          applyFilter();
        });
      });

      document.querySelectorAll('.listing-category').forEach(div => {
        div.onclick = e => {
          e.preventDefault();
          e.stopPropagation();
          const match = div.getAttribute('onclick')?.match(/'([^']+)'/);
          if (!match) return;
          const cat = match[1];
          selected.has(cat) ? selected.delete(cat) : selected.add(cat);
          updateActive();
          updateCardHighlights();
          applyFilter();
          wrapper.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        };
      });

      updateActive();
    };

    setTimeout(setupPills, 0);
  } catch (_) {
    // progressive enhancement — fail silently
  }
})();

(() => {
  try {
    if (!window.location.pathname.includes('/posts/')) return;

    const toc = document.getElementById('TOC');
    const sidebar = document.getElementById('quarto-margin-sidebar');
    if (!toc || !sidebar) return;

    const firstSection = document.querySelector('#quarto-document-content section');
    if (!firstSection) return;

    // On mobile, physically move #TOC into the content flow so Bootstrap's
    // scroll-spy works on it directly (no clone needed).
    const mq = window.matchMedia('(max-width: 767.98px)');

    const moveToContent = () => {
      toc.classList.add('toc-inline');
      firstSection.before(toc);
    };

    const moveToSidebar = () => {
      toc.classList.remove('toc-inline');
      sidebar.prepend(toc);
    };

    const update = () => mq.matches ? moveToContent() : moveToSidebar();
    mq.addEventListener('change', update);
    update();
  } catch (_) {
    // progressive enhancement — fail silently
  }
})();
