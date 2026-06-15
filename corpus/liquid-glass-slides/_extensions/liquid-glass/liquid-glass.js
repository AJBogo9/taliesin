/**
 * liquid-glass — a RevealJS plugin
 *
 * Wraps each content slide's children in a .glass-panel div.
 * Toggles .lg-blur-active on the viewport so the blurred background
 * layer fades in on content slides and out on the title slide.
 */
var LiquidGlass = function () {
  return {
    id: 'liquid-glass',

    init: function (deck) {
      var viewport = document.querySelector('.reveal-viewport');

      // Inject background layers in bottom-up order:
      //   1. lg-blurred-bg (inserted first, sits at bottom of stack)
      //   2. lg-bg         (inserted second via insertBefore, sits above lg-blurred-bg)
      // .reveal (z-index 1) always sits above both.

      var blurBg = document.createElement('div');
      blurBg.className = 'lg-blurred-bg';
      viewport.insertBefore(blurBg, viewport.firstChild);

      // Inject the sharp background layer above the blurred one, fade in once loaded
      var bgEl = document.createElement('div');
      bgEl.className = 'lg-bg';
      viewport.insertBefore(bgEl, viewport.firstChild);

      var bgVal = getComputedStyle(document.documentElement)
        .getPropertyValue('--lg-bg-image').trim();
      var bgMatch = bgVal.match(/url\(['"]?([^'"()]+)['"]?\)/);
      if (bgMatch) {
        var img = new Image();
        img.onload = function () { bgEl.classList.add('lg-bg-loaded'); };
        img.src = bgMatch[1];
      }

      function isTitle(section) {
        return !section ||
          section.id === 'title-slide' ||
          section.classList.contains('quarto-title-block');
      }

      function wrapSection(section) {
        if (section.querySelector('.glass-panel')) return;
        var panel = document.createElement('div');
        panel.className = 'glass-panel';
        Array.from(section.childNodes).forEach(function (node) {
          panel.appendChild(node);
        });
        section.appendChild(panel);
      }

      function wrapSlides() {
        deck.getSlides().forEach(function (section) {
          if (isTitle(section)) return;
          wrapSection(section);
        });

        var titleSection = document.getElementById('title-slide');
        if (titleSection) wrapSection(titleSection);
      }

      function updateBackground() {
        var current = deck.getCurrentSlide();
        viewport.classList.toggle('lg-blur-active', !isTitle(current));
      }

      wrapSlides();
      updateBackground();

      deck.on('slidechanged', function () {
        wrapSlides();
        updateBackground();
      });
    }
  };
};
