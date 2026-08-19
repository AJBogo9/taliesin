//! Theming: the pre-paint `<head>` script, and nothing else.
//!
//! **There is no author-facing theme control.** Both palettes always ship and the reader's
//! DEVICE picks between them, so a document cannot pin itself to one. The `theme:` front
//! matter key — a `.css`/`.scss` file, or an `_extensions/<name>/theme.css` bundle inlined
//! after the base stylesheet — was CUT on 2026-08-17 along with `_extensions/` itself,
//! which existed for nothing else. It was not merely unused: a custom theme meant two
//! different things depending on which token it touched, because `tokens-dark.css` keys the
//! dark palette on `html[data-theme="dark"]` and outranks a theme file's `:root`. Overriding
//! one of the 17 tokens the dark palette re-declares applied in light only; overriding any
//! of the other 28 applied in both modes whatever the colour did on a dark ground. The
//! feature's own corpus pin set three tokens, all in the 17, so it was inert in dark mode
//! and nothing said so. **Do not re-add an author theme key**; a reader-side override is a
//! different thing and still exists (see below).
//!
//! The dev toggle's wiring is NOT here — it lives beside its button in
//! `web-client/client.js`, which never ships in a build. That toggle is unaffected by the
//! cut: it is a READER-side override in `localStorage`, written only by the preview client,
//! so previewing a page in either mode still works while a built page always follows the
//! device. Author control and reader control were always different things.

/// Inline `<head>` script (runs before paint, so no flash): set `<html data-theme>` from the
/// reader's DEVICE (`prefers-color-scheme`), falling back to light when it expresses no
/// preference, and keep following it live. Also defines `taliSetTheme`, which owns the
/// stored override and the repaint.
///
/// **What it deliberately does NOT define is the dev menu's toggle BUTTON.** Until
/// 2026-08-16 this script also shipped `taliToggleTheme`, `taliWireThemeToggles` and two
/// inline SVG icons. Only `web-client/`'s dev menu ever creates a
/// `[data-tali-theme-toggle]`, and a build ships no client, so on every built page that
/// wiring ran once, matched nothing, and returned: 1,693 bytes (537 gzipped, 6% of a page)
/// of JS that could not fire. It is now in `web-client/client.js` beside the button it
/// wires. This script stays the ONE thing both the preview and the build need, and what
/// crosses the boundary is `taliSetTheme` plus the resolved mode, which `apply` publishes
/// as `html[data-theme]` (`pref` never returns `"auto"`, so the attribute IS the mode and
/// the client needs no private access to read it).
///
/// The comment here used to claim the opposite, that the wiring was "Shipped here (not in
/// the preview client) so the toggle works in `build` too". That was not a comment-only
/// defect: `docs/guide/reference/cli.tmd` was written from it and promised every reader of
/// the manual a build-time theme toggle that has never existed. Nothing gates prose against
/// this file, so do not re-add a control here without an emitter to go with it.
///
/// **It takes no argument, and that is the point.** A `default_mode` parameter carried the
/// front-matter `theme: light|dark` forcing until 2026-08-13. With no per-document input
/// there is no door a forced mode can come through, so "the page follows the device" is a
/// property of the signature rather than a claim every call site has to keep.
///
/// The stored choice survives for the dev toggle alone, and a value that is neither `light`
/// nor `dark` reads as "follow the device". That is also the migration for any mode ever
/// withdrawn: a reader whose localStorage still says `sepia` (removed 2026-08-02) degrades
/// to the device rather than to a `data-theme` nothing paints, with no migration code.
/// The script itself. A plain `const`, not a `format!`: it has carried no
/// interpolated value since the theme toggle moved to `web-client/client.js`, so the
/// braces are real JavaScript braces rather than `{{`/`}}` escapes, and rendering a page
/// no longer formats a 6 kB string. Keep it that way; re-introducing an argument means
/// re-escaping every brace below.
const THEME_HEAD_SCRIPT: &str = r#"<script>
(function(){
  // The reader's DEVICE decides the mode. DEVICE is a function (not a constant) so it
  // re-reads the OS on every call — the dev toggle and the OS-change listener both see
  // the live value. Falls back to light when the OS expresses no dark preference.
  function DEVICE(){
    try {
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) return "dark";
    } catch(e) {}
    return "light";
  }
  // The one stored override, written ONLY by the preview dev menu's quick toggle
  // (web-client/client.js), which never ships in a build — so on a built page this is
  // always absent and the device always wins. Absent or unrecognized reads as "auto",
  // which is also why withdrawing a mode needs no migration step.
  function choice(){
    var v = null;
    try { v = localStorage.getItem("tali-theme"); } catch(e) {}
    return (v === "light" || v === "dark") ? v : "auto";
  }
  // The MODE that actually paints: never "auto".
  function pref(){
    var c = choice();
    return c === "auto" ? DEVICE() : c;
  }
  var BG = { dark: '#14130f', light: '#ffffff' };
  function apply(){
    var mode = pref();
    var el = document.documentElement;
    el.setAttribute("data-theme", mode);
    // Set color-scheme + background inline, right here in the pre-paint head
    // script, so the browser's canvas is the theme colour from the very first
    // frame. Without this the canvas stays white until the inline <style> parses,
    // which shows as a white flash on every (cross-page) navigation in dark mode.
    el.style.colorScheme = mode === "dark" ? "dark" : "light";
    el.style.background = BG[mode] || '#ffffff';
    // Keep the mobile browser-chrome tint (`<meta name="theme-color">`) in lockstep with the
    // canvas, so a dark page no longer sits under a white status bar. Reuse the same BG map
    // (single source, no duplicated hex) and follow the in-page toggle, not just the OS. The
    // meta is created here rather than emitted statically so its value is never a stale literal.
    try {
      var head = document.head || document.getElementsByTagName("head")[0];
      var mc = document.querySelector('meta[name="theme-color"]');
      if (!mc && head) { mc = document.createElement("meta"); mc.setAttribute("name", "theme-color"); head.appendChild(mc); }
      if (mc) mc.setAttribute("content", BG[mode] || '#ffffff');
    } catch(e) {}
    // Let theme-dependent renderers (e.g. mermaid, whose SVG colours are baked at
    // render time) re-render when the mode changes. The detail carried a `choice` field
    // until 2026-08-13, for the Settings picker's pressed state alone; no listener reads
    // anything but `mode`.
    try { window.dispatchEvent(new CustomEvent("tali:themechange", { detail: { mode: mode } })); } catch(e) {}
  }
  apply();
  // Follow OS theme flips live. The listener is registered UNCONDITIONALLY: it used to
  // be skipped whenever front matter forced a mode, and that state no longer exists, so
  // a surviving guard would be a branch nothing can take. It still re-applies only while
  // the choice is auto, so the dev toggle is not fought by the OS mid-preview. (Older
  // Safari exposes addListener instead of addEventListener; guard for it the way the
  // rest of the code guards matchMedia.)
  try {
    if (window.matchMedia) {
      var osDark = window.matchMedia('(prefers-color-scheme: dark)');
      var onOsChange = function(){ if (choice() === "auto") apply(); };
      if (osDark.addEventListener) osDark.addEventListener('change', onOsChange);
      else if (osDark.addListener) osDark.addListener(onOsChange);
    }
  } catch(e) {}
  // Passing anything that is not "light"/"dark" REMOVES the key rather than storing it:
  // "following the device" and "never touched the toggle" are the same state, and the
  // OS listener above keys off exactly that.
  window.taliSetTheme = function(p){
    try {
      if (p === "light" || p === "dark") localStorage.setItem("tali-theme", p);
      else localStorage.removeItem("tali-theme");
    } catch(e) {}
    apply();
  };
  // Paper is white. dark.css recolours the syntax scopes with untokenised literals (a
  // dark-mode string is #a5d6ff: 1.6:1 on paper), so the print stylesheet's token reset
  // cannot reach them. (The diagnostic boxes are now token-derived, so the reset DOES reach
  // them; the syntax scopes are what still force the swap.) Drop the whole document to the
  // light theme for the duration of the print job and restore afterwards. `apply()`
  // restores colour-scheme, canvas, and mermaid.
  try {
    window.addEventListener("beforeprint", function(){
      var el = document.documentElement;
      el.setAttribute("data-theme", "light");
      el.style.colorScheme = "light";
      el.style.background = BG.light;
    });
    window.addEventListener("afterprint", apply);
  } catch(e) {}
})();
</script>"#;

pub fn theme_head() -> String {
    THEME_HEAD_SCRIPT.to_string()
}
