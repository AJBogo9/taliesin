//! Theming: resolve a doc's `theme:` (built-in light/dark, a `.css`/`.scss`
//! file, or an installed `_extensions/<name>/theme.css`), the pre-paint
//! `<head>` theme script + toggle wiring, extension `theme:` layers, and the
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
        // Built-in light/dark are always shipped (DARK_CSS) and selected at
        // runtime via `data-theme` (toggle / OS), so no per-page override CSS.
        "light" | "default" | "dark" => String::new(),
        // A named `.css`/`.scss` that can't be read is a typo worth flagging.
        // `try_join_in` refuses an absolute path or one escaping the project root, and
        // keeps the reason: a refused theme whose file plainly exists must not be
        // reported as "not found" or the author goes hunting for a typo that isn't there
        // (the same distinction the bibliography reader draws, `render/mod.rs`).
        path if path.ends_with(".css") || path.ends_with(".scss") => {
            let Some(base) = base_dir else {
                warnings.push(Warning::new(format!("theme file not found: {path}")));
                return String::new();
            };
            match crate::includes::try_join_in(base, path, root) {
                Ok(p) => match std::fs::read_to_string(&p) {
                    Ok(css) => css,
                    Err(_) => {
                        warnings.push(Warning::new(format!("theme file not found: {path}")));
                        String::new()
                    }
                },
                Err(reason) => {
                    warnings.push(Warning::new(refused_theme(path, reason)));
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
/// The default theme mode for the resolver script: an explicit `dark`/`light`
/// from front matter forces that mode; anything else returns `"auto"`, which the
/// pre-paint head script resolves by following the OS `prefers-color-scheme`,
/// falling back to **light** when the OS expresses no dark preference (see
/// [`theme_head`]). Custom CSS themes don't force a built-in mode.
pub(super) fn theme_default_mode(theme: Option<&str>) -> &'static str {
    match theme {
        Some("dark") => "dark",
        Some("light") | Some("default") => "light",
        _ => "auto",
    }
}
/// Inline `<head>` script (runs before paint, so no flash): set
/// `<html data-theme>` from the saved choice, else the front-matter default,
/// else the OS `prefers-color-scheme`. Also defines
/// `taliSetTheme`/`taliGetThemePref`/`taliGetThemeChoice` for the preview toggle
/// and the Settings picker, and keeps `auto` in sync with OS changes.
///
/// Two values, deliberately distinct: the **choice** is what the reader picked
/// (`auto`/`light`/`dark`), the **mode** is what actually paints
/// (`light`/`dark`, never `auto`). A picker has to render the choice, or
/// its `auto` option can never read as selected; everything else wants the mode.
///
/// The allowed list here is also the **migration** for a mode that is withdrawn: a stored
/// choice is validated against it and anything else reads as `auto`, so a reader whose
/// localStorage still says `sepia` (removed 2026-08-02, item 200) degrades to following the
/// OS rather than to a `data-theme` nothing paints. That is why there is no migration code.
pub fn theme_head(default_mode: &str) -> String {
    format!(
        r#"<script>
(function(){{
  // Resolution order for the active mode: a saved reader choice (tali-theme) always
  // wins; else a front-matter-forced "light"/"dark" mode; else (an unspecified or
  // `darkly`-style default, i.e. "auto") follow the OS `prefers-color-scheme`,
  // falling back to light when the OS expresses no dark preference. DEFAULT is a
  // function (not a constant) so the auto fallback re-reads the OS on every call —
  // the toggle, video sync, and the OS-change listener all see the live value.
  var MODE = "{default_mode}";
  function DEFAULT(){{
    if (MODE === "light" || MODE === "dark") return MODE;
    try {{
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) return "dark";
    }} catch(e) {{}}
    return "light";
  }}
  // The reader's stored CHOICE. Absent or unrecognized reads as "auto", so clearing
  // the key is what returns a page to following the OS — and so does a choice that no
  // longer exists, which is what makes withdrawing one safe with no migration step.
  function choice(){{
    var v = null;
    try {{ v = localStorage.getItem("tali-theme"); }} catch(e) {{}}
    return (v === "light" || v === "dark") ? v : "auto";
  }}
  // The MODE that actually paints: never "auto".
  function pref(){{
    var c = choice();
    return c === "auto" ? DEFAULT() : c;
  }}
  var BG = {{ dark: '#16181d', light: '#ffffff' }};
  function apply(){{
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
    try {{
      var head = document.head || document.getElementsByTagName("head")[0];
      var mc = document.querySelector('meta[name="theme-color"]');
      if (!mc && head) {{ mc = document.createElement("meta"); mc.setAttribute("name", "theme-color"); head.appendChild(mc); }}
      if (mc) mc.setAttribute("content", BG[mode] || '#ffffff');
    }} catch(e) {{}}
    // Let theme-dependent renderers (e.g. mermaid, whose SVG colours are baked at
    // render time) re-render on a toggle, and let the Settings picker re-sync its
    // pressed state: which tracks the choice, not the mode.
    try {{ window.dispatchEvent(new CustomEvent("tali:themechange", {{ detail: {{ mode: mode, choice: choice() }} }})); }} catch(e) {{}}
  }}
  apply();
  // Keep an "auto" page reactive to OS theme flips: re-apply only while the choice
  // is auto, so a reader who explicitly picked a theme is never overridden by the
  // OS. (Older Safari exposes addListener instead of addEventListener; guard for it
  // the way the rest of the code guards matchMedia.)
  try {{
    if (MODE !== "light" && MODE !== "dark" && window.matchMedia) {{
      var osDark = window.matchMedia('(prefers-color-scheme: dark)');
      var onOsChange = function(){{ if (choice() === "auto") apply(); }};
      if (osDark.addEventListener) osDark.addEventListener('change', onOsChange);
      else if (osDark.addListener) osDark.addListener(onOsChange);
    }}
  }} catch(e) {{}}
  // Picking "auto" REMOVES the key rather than storing "auto": a reader who never
  // touched the picker and one who explicitly chose auto are the same state, and the
  // OS listener above keys off exactly that.
  window.taliSetTheme = function(p){{
    try {{
      if (p === "light" || p === "dark") localStorage.setItem("tali-theme", p);
      else localStorage.removeItem("tali-theme");
    }} catch(e) {{}}
    apply();
  }};
  window.taliGetThemePref = function(){{ return pref(); }};
  window.taliGetThemeChoice = function(){{ return choice(); }};
  // Paper is white. dark.css recolours the syntax scopes with untokenised literals (a
  // dark-mode string is #a5d6ff: 1.6:1 on paper), so the print stylesheet's token reset
  // cannot reach them. (The diagnostic boxes are now token-derived, so the reset DOES reach
  // them; the syntax scopes are what still force the swap.) Drop the whole document to the
  // light theme for the duration of the print job and restore afterwards. `apply()`
  // restores colour-scheme, canvas, and mermaid.
  try {{
    window.addEventListener("beforeprint", function(){{
      var el = document.documentElement;
      el.setAttribute("data-theme", "light");
      el.style.colorScheme = "light";
      el.style.background = BG.light;
    }});
    window.addEventListener("afterprint", apply);
  }} catch(e) {{}}
  // Wire any `[data-tali-theme-toggle]` button (the dev
  // menu's on a single doc): toggle light <-> dark, icon reflects the current mode.
  // Shipped here (not in the preview client) so the toggle works in `build` too.
  var ICONS = {{ light: "{sun_icon}", dark: "{moon_icon}" }};
  // Flip light <-> dark, resolving the current mode first. Exposed as a global so the
  // Cmd-K command palette (web-client/search.js) and the dev-menu button share one path.
  window.taliToggleTheme = function(){{ window.taliSetTheme(pref() === "dark" ? "light" : "dark"); }};
  window.taliWireThemeToggles = function(){{
    var btns = document.querySelectorAll("[data-tali-theme-toggle]");
    for (var i = 0; i < btns.length; i++) {{
      (function(btn){{
        if (btn.getAttribute("data-wired")) return;
        btn.setAttribute("data-wired", "1");
        function sync(){{ var p = pref(); btn.innerHTML = ICONS[p] || ICONS.dark;
          btn.setAttribute("aria-label", "Theme: " + p + " (click to toggle light / dark)"); }}
        btn.addEventListener("click", function(){{ window.taliToggleTheme(); sync(); }});
        window.addEventListener("tali:themechange", sync);
        sync();
      }})(btns[i]);
    }}
  }};
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", window.taliWireThemeToggles);
  else window.taliWireThemeToggles();
}})();
</script>"#,
        sun_icon = THEME_ICON_SUN,
        moon_icon = THEME_ICON_MOON,
    )
}
// Monochrome theme-toggle icons (single-quoted attrs so they embed in JS double
// quotes; `currentColor` so they inherit the control's colour).
const THEME_ICON_SUN: &str = "<svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round'><circle cx='12' cy='12' r='4'/><path d='M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4'/></svg>";
const THEME_ICON_MOON: &str = "<svg width='15' height='15' viewBox='0 0 24 24' fill='currentColor'><path d='M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z'/></svg>";
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
