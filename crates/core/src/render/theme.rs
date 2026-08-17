//! Theming: resolve a doc's `theme:` (built-in light/dark, a `.css`/`.scss`
//! file, or an installed `_extensions/<name>/theme.css`), the pre-paint
//! `<head>` theme script (the dev toggle's wiring is NOT here; it lives beside its
//! button in `web-client/client.js`), extension `theme:` layers, and the
//! `<style>` wrappers. Split out of the render module; `use super::*` reaches
//! the shared imports (Path, serde_yaml).

use super::*;

/// A theme is an extension that ships CSS. Two minimal themes are built in
/// (`light` is the default `:root`, `dark` overrides it); any other name
/// resolves to a `.css`/`.scss` file or an installed `_extensions/<name>/`
/// bundle, both relative to the document. Returns the override CSS to inline
/// after the base stylesheet (empty for the default light theme).
pub(super) fn resolve_theme(
    theme: Option<&str>,
    base_dir: Option<&Path>,
    root: Option<&Path>,
    warnings: &mut Vec<Warning>,
) -> String {
    let Some(name) = theme else {
        return String::new();
    };
    match name {
        // The three built-in mode names, retired 2026-08-13. They never carried override
        // CSS (both palettes always ship and are selected at runtime via `data-theme`), so
        // all that is left to say is that they no longer select. This is a warning on a
        // VALUE of a live key — `theme:` still takes a `.css` file or an `_extensions/`
        // bundle — which is why it is spelled out here rather than being a key-level rule.
        "light" | "default" | "dark" => {
            warnings.push(Warning::new(format!(
                "`theme: {name}` no longer selects a mode: the page follows the reader's \
                 device setting, so delete the key"
            )));
            String::new()
        }
        // A named `.css`/`.scss` that can't be read is a typo worth flagging.
        // `try_join_in` refuses an absolute path or one escaping the project root, and
        // keeps the reason: a refused theme whose file plainly exists must not be
        // reported as "not found" or the author goes hunting for a typo that isn't there
        // (the same distinction the bibliography reader draws, `render/mod.rs`).
        path if path.ends_with(".css") || path.ends_with(".scss") => {
            let Some(base) = base_dir else {
                warnings.push(
                    Warning::new(format!("theme file not found: {path}")).severity(Severity::Error),
                );
                return String::new();
            };
            match crate::includes::try_join_in(base, path, root) {
                Ok(p) => match std::fs::read_to_string(&p) {
                    Ok(css) => css,
                    Err(_) => {
                        warnings.push(
                            Warning::new(format!("theme file not found: {path}"))
                                .severity(Severity::Error),
                        );
                        String::new()
                    }
                },
                Err(reason) => {
                    warnings
                        .push(Warning::new(refused_theme(path, reason)).severity(Severity::Error));
                    String::new()
                }
            }
        }
        // An installed extension bundle: `_extensions/<name>/theme.css`, resolved through
        // the SAME containment check as every other author-named path. A bare name isn't
        // warned (it may be a legacy built-in theme taliesin doesn't ship, e.g. `darkly`,
        // which harmlessly falls back to the default) — but a name that climbs out of the
        // project is not a legacy theme, so a refusal is reported rather than swallowed.
        ext => {
            let Some(base) = base_dir else {
                return String::new();
            };
            // `Path::join` REPLACES the base on an absolute argument, so `theme: /etc`
            // used to read `/etc/theme.css` outright — item 80's `mounts:` footgun in a
            // second place. Refuse it here rather than letting it be spliced into the
            // relative bundle path below, where `_extensions//etc/theme.css` would
            // normalize to something contained but silently wrong.
            if Path::new(ext).is_absolute() || Path::new(ext).has_root() {
                warnings.push(Warning::new(refused_theme(
                    ext,
                    crate::includes::Refused::OutsideRoot,
                )));
                return String::new();
            }
            match crate::includes::try_join_in(base, &format!("_extensions/{ext}/theme.css"), root)
            {
                Ok(p) => std::fs::read_to_string(&p).unwrap_or_default(),
                Err(reason) => {
                    warnings.push(Warning::new(refused_theme(ext, reason)));
                    String::new()
                }
            }
        }
    }
}

/// How a refused `theme:` reads to the author. Names the boundary that rejected it, so
/// the message distinguishes "your file is missing" from "your file is there and was
/// deliberately not read" — the two have different fixes.
fn refused_theme(named: &str, reason: crate::includes::Refused) -> String {
    match reason {
        crate::includes::Refused::OutsideRoot => {
            format!("theme `{named}` is outside the project root and was not read")
        }
        crate::includes::Refused::SymlinkOutsideRepo => format!(
            "theme `{named}` is a symlink whose target is outside the project repository \
             and was not read"
        ),
    }
}
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
  var BG = { dark: '#14130f', light: '#fbf9f5' };
  function apply(){
    var mode = pref();
    var el = document.documentElement;
    el.setAttribute("data-theme", mode);
    // Set color-scheme + background inline, right here in the pre-paint head
    // script, so the browser's canvas is the theme colour from the very first
    // frame. Without this the canvas stays white until the inline <style> parses,
    // which shows as a white flash on every (cross-page) navigation in dark mode.
    el.style.colorScheme = mode === "dark" ? "dark" : "light";
    el.style.background = BG[mode] || '#fbf9f5';
    // Keep the mobile browser-chrome tint (`<meta name="theme-color">`) in lockstep with the
    // canvas, so a dark page no longer sits under a white status bar. Reuse the same BG map
    // (single source, no duplicated hex) and follow the in-page toggle, not just the OS. The
    // meta is created here rather than emitted statically so its value is never a stale literal.
    try {
      var head = document.head || document.getElementsByTagName("head")[0];
      var mc = document.querySelector('meta[name="theme-color"]');
      if (!mc && head) { mc = document.createElement("meta"); mc.setAttribute("name", "theme-color"); head.appendChild(mc); }
      if (mc) mc.setAttribute("content", BG[mode] || '#fbf9f5');
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
/// Detect the `theme:` front-matter value (top-level or nested under `format:`).
pub(super) fn detect_theme(front_matter: &str) -> Option<String> {
    front_matter.lines().find_map(|line| {
        let v = line.trim().strip_prefix("theme:")?.trim();
        // Take the first name from a scalar or a `[a, b]` list (the first
        // entry is the base theme, the rest are SCSS layers).
        let v = v.trim_start_matches('[').split([',', ']']).next()?.trim();
        let v = v.trim_matches(['"', '\'']).trim();
        (!v.is_empty()).then(|| v.to_string())
    })
}
/// Wrap resolved theme override CSS in a `<style>` (empty string when there is
/// no override, i.e. the default light theme).
pub(super) fn theme_style(theme_css: &str) -> String {
    if theme_css.trim().is_empty() {
        String::new()
    } else {
        // The id lets the dev server hot-swap theme CSS in place (no reload) on a
        // `.css`/theme edit; absent in a build with no custom theme, which is fine.
        format!("<style id=\"tali-theme\">{theme_css}</style>")
    }
}
