
// --- Media playback: user-initiated video + single active player -------------
// `{{< video >}}` screencasts never autoplay (WCAG 2.2.2 "Pause, Stop, Hide": a
// looping clip beside body text that auto-starts is a failure). Playback is
// user-initiated across every input mode:
//   - HOVER (pointerenter/leave): a transient inline preview on desktop;
//   - KEYBOARD FOCUS (focusin/out): plays inline while the video is focused, so a
//     keyboard user can play it too (the video is tabindex=0 + labelled);
//   - CLICK / TAP: opens the video in the lightbox (an enlarged, playing copy) via
//     the pre-existing capture-phase delegation in 11-lightbox.js. That is the touch
//     play path AND the explicit "watch it properly" affordance, so this fragment
//     deliberately does NOT also bind a click/pointerup handler (it would fight the
//     lightbox, which stopPropagation()s the click).
// `prefers-reduced-motion: reduce` suppresses the transient hover/focus play; an
// explicit tap still plays (via the lightbox), a user request for the motion.
// `data-playing` on the `<figure>` drives the paused play-glyph overlay.
//
// Separately, ONE document-level `play` listener enforces a single active player of
// any kind (audio or video): starting one pauses every other — including the lightbox
// copy, so opening it pauses the inline clip. The per-figure wiring is a registered
// enhancer (idempotent, re-run on each mount); the document-level listeners live in
// this IIFE body, which runs exactly once per page load.
(function () {
  if (!window.taliEnhancers) return;

  var rmq = window.matchMedia ? window.matchMedia('(prefers-reduced-motion: reduce)') : null;
  function reduceMotion() { return !!(rmq && rmq.matches); }

  // A pointer press also FOCUSES the video (it is tabindex=0), which would fire
  // focusin->play just as the click opens the lightbox — a brief inline flicker. Track
  // whether a pointer gesture is in flight (document-level, once) so the focus handlers
  // fire ONLY for real keyboard focus; the pointer path is hover + the lightbox.
  var pointering = false;
  document.addEventListener('pointerdown', function () { pointering = true; }, true);
  document.addEventListener('pointerup', function () { setTimeout(function () { pointering = false; }, 0); }, true);

  /** The theme-visible `<video>` in a figure (the one not display:none), or null.
   * @param {HTMLElement} fig @returns {HTMLVideoElement | null} */
  function visibleVideo(fig) {
    var vids = fig.querySelectorAll('video');
    for (var i = 0; i < vids.length; i++) {
      if (getComputedStyle(vids[i]).display !== 'none') return vids[i];
    }
    return null;
  }
  /** Promote a lazy pair clip's `data-src`->`src` so it can play. Idempotent.
   * @param {HTMLVideoElement} v */
  function promote(v) {
    if (!v.getAttribute('src')) {
      var d = v.getAttribute('data-src');
      if (d) v.setAttribute('src', d);
    }
  }
  /** Play the figure's visible clip (promoting its src first). @param {HTMLElement} fig */
  function play(fig) {
    var v = visibleVideo(fig);
    if (!v) return;
    promote(v);
    var p = v.play();
    if (p && p.catch) p.catch(function () {});
  }
  /** @param {HTMLElement} fig */
  function pause(fig) {
    var vids = fig.querySelectorAll('video');
    for (var i = 0; i < vids.length; i++) { try { vids[i].pause(); } catch (e) {} }
  }

  /** Wire one `.tali-video` figure. Idempotent (keyed on data-media-wired).
   * @param {HTMLElement} fig */
  function wire(fig) {
    if (fig.getAttribute('data-media-wired')) return;
    fig.setAttribute('data-media-wired', '1');

    // Keep `data-playing` (which drives the paused play-glyph overlay) synced to the
    // real element state, so an EXTERNAL pause (the single-active-player coordinator,
    // or theme.rs pausing the hidden variant) also brings the glyph back.
    var vids = fig.querySelectorAll('video');
    for (var i = 0; i < vids.length; i++) {
      vids[i].addEventListener('play', function () { fig.setAttribute('data-playing', '1'); });
      vids[i].addEventListener('pause', function () {
        var vis = visibleVideo(fig);
        if (!vis || vis.paused) fig.removeAttribute('data-playing');
      });
    }

    // Hover: transient inline preview (suppressed under reduced motion).
    fig.addEventListener('pointerenter', function () { if (!reduceMotion()) play(fig); });
    fig.addEventListener('pointerleave', function () { pause(fig); });
    // Keyboard focus (parity with hover) — guarded so a mouse click's focus churn does
    // not flicker the inline clip as the lightbox opens.
    fig.addEventListener('focusin', function () { if (!pointering && !reduceMotion()) play(fig); });
    fig.addEventListener('focusout', function () { if (!pointering) pause(fig); });
  }

  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    (root || document).querySelectorAll('.tali-video').forEach(function (el) {
      if (el instanceof HTMLElement) wire(el);
    });
  }
  window.taliEnhancers.register(enhance);

  // Single active player (global, cross-type): when any media starts, pause every
  // other `<audio>`/`<video>`. Media `play` events do NOT bubble, so this is capture-
  // phase; it covers raw `<audio>` and the lightbox copy too, not just `{{< video >}}`.
  document.addEventListener('play', function (e) {
    var t = e.target;
    if (!(t instanceof HTMLMediaElement)) return;
    document.querySelectorAll('audio, video').forEach(function (m) {
      if (m !== t && m instanceof HTMLMediaElement && !m.paused) { try { m.pause(); } catch (err) {} }
    });
  }, true);
})();
