// Skip-to-content link: a visually-hidden-until-focused link that jumps keyboard /
// screen-reader users past the chrome to the content. Build + site pages now emit the
// link + a focusable `<main id="tali-main" tabindex="-1">` SERVER-SIDE (page.rs), so it
// works with JS off; this only enhances. The live `#tali-root` mount has no server `<main>`,
// so the pair is synthesized there. Read-only, deck-skipped, idempotent.
function qmdInitSkipLink() {
  if (window.__qmdSkipLink) return;
  if (document.querySelector('.tali-deck')) return;
  var main =
    document.querySelector('main') ||
    document.getElementById('tali-root') ||
    document.querySelector('[data-block-id]');
  if (!main) return;
  window.__qmdSkipLink = true;
  if (!main.id) main.id = 'tali-main';
  main.setAttribute('tabindex', '-1');
  // Move focus (not just scroll) so a keyboard reader continues from the content. Wire
  // this onto the server-rendered link too (it ships as a plain anchor), so this path
  // enhances both the server-emitted and the JS-synthesized link.
  var focusMain = function () { setTimeout(function () { main.focus(); }, 0); };
  var existing = document.querySelector('.tali-skip');
  if (existing) { existing.addEventListener('click', focusMain); return; }
  var a = document.createElement('a');
  a.className = 'tali-skip';
  a.href = '#' + main.id;
  a.textContent = 'Skip to content';
  a.addEventListener('click', focusMain);
  document.body.insertBefore(a, document.body.firstChild);
}

