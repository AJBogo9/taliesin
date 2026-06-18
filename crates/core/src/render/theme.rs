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
    warnings: &mut Vec<String>,
) -> String {
    let Some(name) = theme else {
        return String::new();
    };
    match name {
        // Built-in light/dark are always shipped (DARK_CSS) and selected at
        // runtime via `data-theme` (toggle / OS), so no per-page override CSS.
        "light" | "default" | "dark" => String::new(),
        // A named `.css`/`.scss` that can't be read is a typo worth flagging.
        path if path.ends_with(".css") || path.ends_with(".scss") => {
            match base_dir.and_then(|b| std::fs::read_to_string(b.join(path)).ok()) {
                Some(css) => css,
                None => {
                    warnings.push(format!("theme file not found: {path}"));
                    String::new()
                }
            }
        }
        // An installed extension bundle: `_extensions/<name>/theme.css`. A bare
        // name isn't warned (it may be a Quarto built-in theme qmd-fast doesn't
        // ship, e.g. `darkly`, which harmlessly falls back to the default).
        ext => base_dir
            .and_then(|b| {
                std::fs::read_to_string(b.join("_extensions").join(ext).join("theme.css")).ok()
            })
            .unwrap_or_default(),
    }
}
/// The default theme mode for the resolver script: an explicit `dark`/`light`
/// from front matter forces that mode; otherwise `auto` follows the OS
/// (`prefers-color-scheme`). Custom CSS themes don't force a built-in mode.
pub(super) fn theme_default_mode(theme: Option<&str>) -> &'static str {
    match theme {
        Some("dark") => "dark",
        Some("light") | Some("default") => "light",
        _ => "auto",
    }
}
/// Inline `<head>` script (runs before paint, so no flash): set
/// `<html data-theme>` from the saved choice, else the front-matter default,
/// else the OS `prefers-color-scheme`. Also defines `qmdSetTheme`/`qmdGetThemePref`
/// for the preview toggle and keeps `auto` in sync with OS changes.
pub fn theme_head(default_mode: &str) -> String {
    format!(
        r#"<script>
(function(){{
  // Two fixed modes only — light or dark. No OS-following "auto": a `darkly`-style
  // (or unspecified) default resolves to dark; an explicit light theme to light.
  var DEFAULT = "{default_mode}" === "light" ? "light" : "dark";
  function pref(){{
    var v = null;
    try {{ v = localStorage.getItem("qmd-theme"); }} catch(e) {{}}
    return (v === "light" || v === "dark") ? v : DEFAULT;
  }}
  function apply(){{
    var mode = pref();
    var el = document.documentElement;
    el.setAttribute("data-theme", mode);
    // Set color-scheme + background inline, right here in the pre-paint head
    // script, so the browser's canvas is the theme colour from the very first
    // frame. Without this the canvas stays white until the inline <style> parses,
    // which shows as a white flash on every (cross-page) navigation in dark mode.
    el.style.colorScheme = mode;
    el.style.background = mode === "dark" ? '#16181d' : '#ffffff';
    // Let theme-dependent renderers (e.g. mermaid, whose SVG colours are baked at
    // render time) re-render on a toggle.
    try {{ window.dispatchEvent(new CustomEvent("qmd:themechange", {{ detail: {{ mode: mode }} }})); }} catch(e) {{}}
  }}
  apply();
  window.qmdSetTheme = function(p){{ try {{ localStorage.setItem("qmd-theme", p); }} catch(e) {{}} apply(); }};
  window.qmdGetThemePref = function(){{ return pref(); }};
  // Wire any `[data-qmd-theme-toggle]` button (the site navbar's, or the dev
  // menu's on a single doc): toggle light <-> dark, icon reflects the current mode.
  // Shipped here (not in the preview client) so the toggle works in `build` too.
  var ICONS = {{ light: "{sun_icon}", dark: "{moon_icon}" }};
  window.qmdWireThemeToggles = function(){{
    var btns = document.querySelectorAll("[data-qmd-theme-toggle]");
    for (var i = 0; i < btns.length; i++) {{
      (function(btn){{
        if (btn.getAttribute("data-wired")) return;
        btn.setAttribute("data-wired", "1");
        function sync(){{ var p = pref(); btn.innerHTML = ICONS[p] || ICONS.dark;
          btn.setAttribute("aria-label", "Theme: " + p + " (click to toggle light / dark)"); }}
        btn.addEventListener("click", function(){{ window.qmdSetTheme(pref() === "dark" ? "light" : "dark"); sync(); }});
        window.addEventListener("qmd:themechange", sync);
        sync();
      }})(btns[i]);
    }}
  }};
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", window.qmdWireThemeToggles);
  else window.qmdWireThemeToggles();
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
        // Take the first name from a scalar or a `[a, b]` list (Quarto allows a
        // list; the first entry is the base theme, the rest are SCSS layers).
        let v = v.trim_start_matches('[').split([',', ']']).next()?.trim();
        let v = v.trim_matches(['"', '\'']).trim();
        (!v.is_empty()).then(|| v.to_string())
    })
}
/// Inline every `.css` entry of an extension's `theme:` (a scalar or list) as a
/// `<style>` block, read relative to `dir`. Non-`.css` entries (named base themes,
/// uncompiled `.scss`) are skipped.
pub(super) fn resolve_theme_layers(v: Option<&serde_yaml::Value>, dir: &Path) -> String {
    let mut out = String::new();
    let mut push = |name: &str| {
        if name.ends_with(".css")
            && let Ok(css) = std::fs::read_to_string(dir.join(name))
        {
            out.push_str("<style>\n");
            out.push_str(&css);
            out.push_str("\n</style>\n");
        }
    };
    match v {
        Some(serde_yaml::Value::String(s)) => push(s),
        Some(serde_yaml::Value::Sequence(seq)) => {
            for item in seq {
                if let serde_yaml::Value::String(s) = item {
                    push(s);
                }
            }
        }
        _ => {}
    }
    out
}
/// Wrap resolved theme override CSS in a `<style>` (empty string when there is
/// no override, i.e. the default light theme).
pub(super) fn theme_style(theme_css: &str) -> String {
    if theme_css.trim().is_empty() {
        String::new()
    } else {
        format!("<style>{theme_css}</style>")
    }
}
