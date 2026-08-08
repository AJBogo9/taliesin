//! Deck output: the one-shot deck page, the live-deck client head/script, and the
//! slide model (grouping blocks into `<section>`s by heading level and `---`
//! breaks). Split out of the render module; `use super::*` pulls in the shared
//! block model and helpers (Block, RenderedDoc, html escaping, slugify,
//! block_heading_level, KaTeX CSS, theme_style, code_head/code_scripts).

use super::*;

/// taliesin's own deck engine, bundled (no CDN): `deck.css` is the layout + theme
/// and `deck.js` the navigation/scaling engine (`window.TaliesinDeck`). Inlined into
/// both the one-shot page and the live client, like KaTeX/mermaid.
pub(super) const DECK_CSS: &str = include_str!("../../assets/css/deck.css");
pub(super) const DECK_JS: &str = include_str!("../../assets/js/deck.js");

/// The pieces a caller supplies to [`assemble_deck_page`]; the deck analogue of
/// [`super::PageParts`]. The static build passes the empty preview slots; the
/// live-deck server fills `extra_head`/`slides_attr`/`after_deck` and composes
/// its own client-driven `tail`. The shared `<head>` lives once in the builder.
pub struct DeckParts<'a> {
    /// Already HTML-escaped `<title>` text.
    pub title: &'a str,
    /// Pre-built `description`/OpenGraph/Twitter `<meta>` block, or `""`. A standalone
    /// build fills this from the deck's own front matter; the live preview passes `""`
    /// (nothing scrapes a localhost deck), and a site deck already carries the richer
    /// URL-aware block on `include-in-header`.
    pub social: &'a str,
    /// BCP-47 language tag for `<html lang>` (e.g. `en`); callers default to `en`.
    pub lang: &'a str,
    /// A pre-built `<link rel="icon" …>`. The standalone build passes the bundled
    /// default mark (like a page), the live/site preview paths their own route.
    pub favicon: &'a str,
    pub theme_default: &'a str,
    /// A custom/extension `theme:` owns its colours (no pre-paint mode script).
    pub theme_is_custom: bool,
    pub theme_css: &'a str,
    /// Ship the KaTeX stylesheet (only-if-math for the build, always for a live deck).
    pub ship_katex: bool,
    /// Preview-only `<head>` additions (the dev-menu CSS); `""` for the build.
    pub extra_head: &'a str,
    pub include_in_header: &'a str,
    pub include_before_body: &'a str,
    /// Attributes on the `.tali-slides` container (` id="tali-root"` for the live mount).
    pub slides_attr: &'a str,
    /// The slide HTML (`<section>`s).
    pub slides: &'a str,
    /// Persistent deck chrome (front-matter `footer:`/`logo:`) as HTML, placed as a fixed
    /// sibling of `.tali-slides` inside `.tali-deck` so it stays put across slide changes.
    /// `""` when the deck sets neither. Built by [`deck_overlay_html`].
    pub deck_overlay: &'a str,
    /// Markup right after the `.tali-deck` container (the live status node); `""` build.
    pub after_deck: &'a str,
    /// Everything after the deck body: the deck-engine script + the format-specific
    /// init/enhancer/client scripts + `include-after-body`, composed by the caller
    /// (the static `TaliesinDeck.initialize` flow and the client-driven live flow differ,
    /// and the live flow is load-order-sensitive).
    pub tail: &'a str,
    /// How the deck's framework CSS is delivered. `Inline` bakes it in (the standalone
    /// build and both live previews); `External` links the shared `_assets/` pair, which
    /// only the multi-page `build <dir>` path has.
    pub assets: AssetMode<'a>,
}

/// Assemble a complete deck page from its parts: the single source of the deck
/// page skeleton + `<head>` (deck-theme pre-paint, bundled deck CSS, KaTeX), shared
/// by the static build and the live-deck preview. The deck-engine `<script>` and
/// the rest of the script tail are caller-composed.
pub fn assemble_deck_page(p: &DeckParts) -> String {
    // The deck stylesheet, KaTeX and the `{js}`-cell libs are the three head payloads that
    // differ by asset mode; everything else about the skeleton is identical, so the two
    // modes cannot drift apart in shape.
    let (style_block, katex, js_head_html) = match &p.assets {
        AssetMode::Inline => {
            // Only ship the (large) KaTeX stylesheet when the deck has math (build); a live
            // deck always ships it, since it can gain math on any edit.
            let katex = if p.ship_katex {
                format!("<style>{KATEX_CSS}</style>\n")
            } else {
                String::new()
            };
            // Native `{js}` cells need the vendored d3 + Plot libs (the enhancer rides in
            // code_scripts); gated on the slide body.
            let js_head_html = if has_js_cells(p.slides) {
                js_cell_head()
            } else {
                String::new()
            };
            let style =
                format!("<style>{FONTS_CSS}{TOKENS_CSS}{TOKENS_DARK_CSS}{DECK_CSS}</style>");
            (style, katex, js_head_html)
        }
        AssetMode::External(a) => {
            let katex = if p.ship_katex {
                format!("<link rel=\"stylesheet\" href=\"{}\">\n", a.katex_css)
            } else {
                String::new()
            };
            let js_head_html = if has_js_cells(p.slides) {
                format!("<script src=\"{}\" defer></script>", a.jslibs_js)
            } else {
                String::new()
            };
            // Item 150, same shape as the page's: the body face is a file here, so preload
            // it ahead of the sheet that would otherwise have to parse before it is found.
            let font_preload = if a.font_preload.is_empty() {
                String::new()
            } else {
                format!(
                    "<link rel=\"preload\" as=\"font\" type=\"font/woff2\" href=\"{}\" crossorigin>",
                    a.font_preload
                )
            };
            let style = format!(
                "{font_preload}<link rel=\"stylesheet\" href=\"{}\">",
                a.deck_css
            );
            (style, katex, js_head_html)
        }
    };
    // `theme` comes after the deck's own stylesheet so it overrides it; the css
    // folded into `include-in-header` follows last.
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\">\n<head>\n{GENERATOR_BANNER}\
         <meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n\
         <meta name=\"referrer\" content=\"no-referrer\" />\n\
         <meta name=\"generator\" content=\"Taliesin\" />\n\
         <title>{title}</title>{social}\n{favicon}{deck_theme}{style_block}\n{katex}{js_head}{theme}{in_header}{extra_head}\
         </head>\n<body>\n{before_body}<div class=\"tali-deck\">\n<div class=\"tali-slides\"{slides_attr}>\n{slides}</div>\n{overlay}</div>\n{after_deck}\
         {tail}</body>\n</html>\n",
        lang = escape_attr(p.lang),
        title = p.title,
        social = p.social,
        favicon = p.favicon,
        deck_theme = deck_theme_head(p.theme_default, p.theme_is_custom),
        theme = theme_style(p.theme_css),
        in_header = p.include_in_header,
        before_body = p.include_before_body,
        js_head = js_head_html,
        slides_attr = p.slides_attr,
        slides = p.slides,
        overlay = p.deck_overlay,
        after_deck = p.after_deck,
        extra_head = p.extra_head,
        tail = p.tail,
    )
}

/// The persistent deck-chrome overlay (front-matter `footer:`/`logo:`): a fixed sibling of
/// `.tali-slides` that stays put across slide changes. `footer` is escaped plain text;
/// `logo` is an image URL/path rendered as a decorative `<img>` (empty `alt` — it is
/// branding that repeats on every slide, not per-slide content). Returns `""` when the deck
/// sets neither, so a deck without chrome emits exactly what it did before.
pub fn deck_overlay_html(footer: Option<&str>, logo: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(src) = logo.map(str::trim).filter(|v| !v.is_empty()) {
        s.push_str(&format!(
            "<img class=\"tali-deck-logo\" src=\"{}\" alt=\"\" />\n",
            escape_attr(src)
        ));
    }
    if let Some(text) = footer.map(str::trim).filter(|v| !v.is_empty()) {
        let mut esc = String::new();
        escape_html(text, &mut esc);
        s.push_str(&format!("<div class=\"tali-deck-footer\">{esc}</div>\n"));
    }
    s
}

pub(super) fn deck_page_from_doc(
    doc: &RenderedDoc,
    fallback_title: &str,
    mode: OutputMode,
    assets: AssetMode,
) -> String {
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // The static deck self-initializes the engine on load and runs the enhancers
    // once (no websocket client to drive them after a mount). `mode` gates the
    // enhancers exactly like an HTML page (e.g. a Build with a Mermaid diagram
    // inlines the vendored library instead of fetching it from a CDN).
    //
    // External mode replaces the two big inline blobs — the engine and the enhancer
    // bundle — with ONE classic (non-`defer`) `<script src>`. Classic external scripts
    // execute in document order before the inline scripts that follow them, so the
    // `TaliesinDeck.initialize(...)` call below still sees the facade and the
    // `include-after-body` position still sees the enhancer registry: the same
    // guarantees the inline tail gives, without the `defer` dance a page needs.
    let engine_and_enhancers = match &assets {
        AssetMode::Inline => format!(
            "<script>{DECK_JS}</script>\n\
             <script>\n  TaliesinDeck.initialize({{ hash: true, slideNumber: 'c/t', center: false }});\n</script>\n\
             {}\n",
            code_scripts_for(&slides, mode, true)
        ),
        AssetMode::External(a) => {
            // Mermaid keeps its own conditional file (shared with the site's pages, which is
            // the duplicate this whole mode exists to remove); the `{js}`-cell runtime stays
            // inline so a cell's `import("./x.js")` resolves against the page, not `_assets/`.
            let mermaid = if has_mermaid(&slides) {
                format!("\n<script src=\"{}\" defer></script>", a.mermaid_js)
            } else {
                String::new()
            };
            // Gated on `has_client_cells`, NOT `has_js_cells`: every registered client-side
            // language runs through this one runtime, and each language's own enhancer
            // registers into the object this script defines. Gating on `{js}` alone meant a
            // deck whose only cells were `{glsl}` shipped neither the runtime nor the
            // enhancer in a site build, so the cells were inert markup. The Inline arm never
            // had this gap (`code_scripts_for` gates each language separately); this arm is
            // the hand-rolled copy that drifted from it.
            let tali_js = if has_client_cells(&slides) {
                let glsl = if has_client_cells_of(&slides, "glsl") {
                    format!("\n<script>{GLSL_JS}</script>")
                } else {
                    String::new()
                };
                format!("\n<script>{TALIESIN_JS}</script>{glsl}")
            } else {
                String::new()
            };
            format!(
                "<script src=\"{}\"></script>\n\
                 <script>\n  TaliesinDeck.initialize({{ hash: true, slideNumber: 'c/t', center: false }});\n</script>\
                 {mermaid}{tali_js}\n",
                a.deck_js
            )
        }
    };
    let tail = format!(
        "{engine_and_enhancers}\
         <script>document.addEventListener('DOMContentLoaded',function(){{window.taliEnhanceCode&&window.taliEnhanceCode(document.body);}});</script>\n\
         {after_body}",
        after_body = doc.includes.after_body,
    );
    // A standalone-built deck gets the same bundled-mark favicon a standalone page
    // does (page.rs), so a built deck's tab has an icon and never 404s `/favicon.ico`.
    // The live-preview and site paths set their own favicon on `DeckParts` (a served
    // route / the site's configured mark) and reach `assemble_deck_page` directly.
    let favicon = super::page::default_favicon();
    let overlay = deck_overlay_html(doc.footer.as_deref(), doc.logo.as_deref());
    // A shared deck link deserves the same preview a shared page link gets (PA-H1). This is
    // the context-free block a standalone page uses — no `og:url`/`og:image`, because a
    // single file has no site URL to absolutize against.
    //
    // A deck built inside a SITE has already had the richer, URL-aware block pushed onto
    // `include-in-header` (`site::meta::deck_social_head`, which also has the branded
    // card), and it reaches this same function. Emitting the basic set unconditionally
    // would give that deck two `og:title`s, so the richer block wins by suppressing this
    // one rather than by ordering.
    let social = if doc.includes.in_header.contains("og:title") {
        String::new()
    } else {
        social_meta_head(Some(title), doc.description.as_deref(), false)
    };
    assemble_deck_page(&DeckParts {
        title: &t,
        social: &social,
        lang: doc.lang.as_deref().unwrap_or("en"),
        favicon: &favicon,
        theme_default: &doc.theme_default,
        theme_is_custom: doc.theme_is_custom,
        theme_css: &doc.theme_css,
        ship_katex: slides.contains("class=\"katex"),
        extra_head: "",
        include_in_header: &doc.includes.in_header,
        include_before_body: &doc.includes.before_body,
        slides_attr: "",
        slides: &slides,
        deck_overlay: &overlay,
        after_deck: "",
        tail: &tail,
        assets,
    })
}

/// Pre-paint `<head>` script that sets the deck's light/dark mode before first
/// paint, so an embedded deck never flashes a white panel on a dark page. The
/// mode is derived from the doc's resolved theme: an explicit `theme: dark`/
/// `light` forces it; the built-in default theme (`theme_default` "auto", no
/// custom CSS) follows the embedding page (a same-origin host) or the OS for a
/// standalone deck; a custom/extension theme (`custom_theme`) owns its own colours
/// and gets no script. The runtime helpers (`taliDeckApplyTheme`/`taliDeckSetTheme`)
/// are used by deck.js for the menu toggle and live host-theme following.
pub fn deck_theme_head(theme_default: &str, custom_theme: bool) -> String {
    let mode = match theme_default {
        "dark" => "dark",
        "light" => "light",
        _ if custom_theme => "none",
        _ => "auto",
    };
    if mode == "none" {
        return String::new();
    }
    format!(
        r#"<script>
(function(){{
  var DEFAULT = "{mode}";
  // The deck canvas per mode: light is deck.css's `html {{ background }}`, dark is the
  // `--tali-bg` token `html.tali-deck-dark` resolves to. Literals because this script runs
  // BEFORE deck.css parses (a computed read would see the UA default), so
  // `the_decks_pre_paint_script_keeps_theme_color_with_its_canvas` pins them to the CSS.
  var BG = {{ dark: '#16181d', light: '#ffffff' }};
  var embedded = window.self !== window.top;
  function osDark(){{ try {{ return matchMedia('(prefers-color-scheme: dark)').matches; }} catch(e){{ return false; }} }}
  function hostTheme(){{ try {{ var t = window.top.document.documentElement.getAttribute('data-theme'); return (t==='dark'||t==='light') ? t : null; }} catch(e){{ return null; }} }}
  function stored(){{ try {{ var v = localStorage.getItem('tali-deck-theme'); return (v==='dark'||v==='light') ? v : null; }} catch(e){{ return null; }} }}
  function resolve(){{
    if (embedded) {{ return hostTheme() || (osDark() ? 'dark' : 'light'); }}  // follow the host page
    var s = stored(); if (s) return s;                                        // a standalone deck's saved toggle
    if (DEFAULT==='dark'||DEFAULT==='light') return DEFAULT;                   // explicit front-matter
    return osDark() ? 'dark' : 'light';                                       // else the OS preference
  }}
  window.taliDeckEmbedded = embedded;
  window.taliDeckThemeManaged = true;
  window.taliDeckApplyTheme = function(){{ var m = resolve(); var el = document.documentElement; var prev = window.__taliDeckMode; window.__taliDeckMode = m; el.classList.toggle('tali-deck-dark', m==='dark'); el.style.colorScheme = m;
    // Keep the mobile browser-chrome tint with the canvas, as the page's pre-paint script
    // does — without this a dark deck sat under a white status bar (PA-H1). Created here
    // rather than emitted statically so the value can never be a stale literal, and so it
    // follows the deck's own toggle rather than only the OS.
    try {{ var hd = document.head || document.getElementsByTagName('head')[0]; var mc = document.querySelector('meta[name="theme-color"]'); if (!mc && hd) {{ mc = document.createElement('meta'); mc.setAttribute('name', 'theme-color'); hd.appendChild(mc); }} if (mc) mc.setAttribute('content', BG[m] || '#ffffff'); }} catch(e) {{}}
    // A live light/dark flip must re-render mermaid (it bakes colours into the SVG at
    // run() time); the page fires this same event, so a deck reuses it. Skip the first
    // apply (prev undefined) — the initial diagram render already reads the resolved mode.
    if (prev !== undefined && prev !== m) {{ try {{ window.dispatchEvent(new CustomEvent('tali:themechange', {{ detail: {{ mode: m }} }})); }} catch(e){{}} }}
    return m; }};
  window.taliDeckSetTheme = function(m){{ if (!embedded) {{ try {{ if (m==='dark'||m==='light') localStorage.setItem('tali-deck-theme', m); else localStorage.removeItem('tali-deck-theme'); }} catch(e){{}} }} return window.taliDeckApplyTheme(); }};
  window.taliDeckThemeChoice = function(){{ return stored() || 'auto'; }};  // 'auto' = no stored key (OS-follow)
  // A standalone deck in Auto follows a live OS light/dark flip, mirroring the page's pre-paint script.
  try {{ if (!embedded && window.matchMedia) {{ var mq = matchMedia('(prefers-color-scheme: dark)'); var onOs = function(){{ if (!stored()) window.taliDeckApplyTheme(); }}; if (mq.addEventListener) mq.addEventListener('change', onOs); else if (mq.addListener) mq.addListener(onOs); }} }} catch(e){{}}
  window.taliDeckApplyTheme();
}})();
</script>"#
    )
}

// --- deck slide model ---------------------------------------------------

/// The default `slide-level`: headings at this level start a new slide;
/// headings above it (h1) open a vertical stack of sub-slides.
const SLIDE_LEVEL: u8 = 2;

/// One slide's contents: the heading level that opened it (0 when opened by a
/// `---` break or leading content), an optional id slug, and the inner block
/// HTML (each block keeps its own `data-block-id`/`data-sourcepos`).
#[derive(Clone)]
struct SlideBuf {
    level: u8,
    from_rule: bool,
    id: Option<String>,
    blocks: Vec<String>,
}

/// A top-level (horizontal) slide, optionally carrying vertical sub-slides.
enum Top {
    Slide(SlideBuf),
    Stack {
        lead: SlideBuf,
        children: Vec<SlideBuf>,
    },
}

/// Build the inner HTML of the deck's `<div class="tali-slides">`: an optional title
/// slide from front matter, then one `<section>` per slide. Blocks are grouped
/// into slides by heading level (`SLIDE_LEVEL`) and `---` breaks, with h1s
/// wrapping their h2s as a vertical stack.
pub fn slides_html(title: Option<&str>, subtitle: Option<&str>, blocks: &[Block]) -> String {
    let mut out = String::new();
    if let Some(title) = title {
        out.push_str(
            "<section id=\"title-slide\" class=\"tali-title-slide center\">\n<h1 class=\"title\">",
        );
        escape_html(title, &mut out);
        out.push_str("</h1>\n");
        if let Some(sub) = subtitle {
            out.push_str("<p class=\"subtitle\">");
            escape_html(sub, &mut out);
            out.push_str("</p>\n");
        }
        out.push_str("</section>\n");
    }
    for top in group_slides(blocks, title.is_some()) {
        render_top(&top, &mut out);
    }
    out
}

/// A rendered deck's spoken-script summary, from the per-slide `data-script-secs`
/// estimates. Produced by [`script_summary`]; a caller (build/preview) shows it as one
/// console line, e.g. `~8:40 estimated across 12 slides (9 scripted)`.
pub struct ScriptSummary {
    /// Estimated seconds to speak every scripted slide's `::: {.notes}`.
    pub total_secs: u64,
    /// Content slides carrying a script (`::: {.notes}`).
    pub scripted: usize,
    /// Total navigable slides, matching the deck's own count (the front-matter title
    /// slide plus every content slide), so this agrees with the speaker window's
    /// "slide X / N" rather than reporting a different N.
    pub slides: usize,
}

/// Summarize a rendered deck's spoken-script duration by summing the per-slide
/// `data-script-secs` estimates that [`slides_html`] emits. `None` when no slide
/// carries a script (not a deck, or a deck with no `::: {.notes}`), so a caller can
/// stay silent rather than report an empty estimate.
pub fn script_summary(html: &str) -> Option<ScriptSummary> {
    const ATTR: &str = "data-script-secs=\"";
    // Content slides (`class="tali-slide"`) plus the front-matter title slide
    // (`class="tali-title-slide …"`), so the count matches the deck's navigable total.
    let slides = html.matches("class=\"tali-slide\"").count()
        + html.matches("class=\"tali-title-slide").count();
    let mut total_secs = 0u64;
    let mut scripted = 0usize;
    for (i, _) in html.match_indices(ATTR) {
        let rest = &html[i + ATTR.len()..];
        if let Some(end) = rest.find('"')
            && let Ok(secs) = rest[..end].parse::<u64>()
        {
            total_secs += secs;
            scripted += 1;
        }
    }
    (scripted > 0).then_some(ScriptSummary {
        total_secs,
        scripted,
        slides,
    })
}

/// Split blocks into flat slides at slide-level headings and `---` breaks,
/// then nest h2 slides under any preceding h1 as a vertical stack. `has_title`
/// reserves the injected `id="title-slide"` so a slide literally titled "Title
/// Slide" can't collide with the front-matter title slide.
fn group_slides(blocks: &[Block], has_title: bool) -> Vec<Top> {
    let flat = split_slides(blocks, has_title);
    let mut tops: Vec<Top> = Vec::new();
    let mut i = 0;
    while i < flat.len() {
        let opens_stack = flat[i].level != 0 && flat[i].level < SLIDE_LEVEL && !flat[i].from_rule;
        if opens_stack {
            let lead = flat[i].clone();
            i += 1;
            let mut children = Vec::new();
            // Gather following slides as vertical children until the next
            // above-slide-level heading or a `---` break pops the stack.
            while i < flat.len() {
                let c = &flat[i];
                let breaks = c.from_rule || (c.level != 0 && c.level < SLIDE_LEVEL);
                if breaks {
                    break;
                }
                children.push(flat[i].clone());
                i += 1;
            }
            if children.is_empty() {
                tops.push(Top::Slide(lead));
            } else {
                tops.push(Top::Stack { lead, children });
            }
        } else {
            tops.push(Top::Slide(flat[i].clone()));
            i += 1;
        }
    }
    tops
}

/// First pass: a new slide begins at any heading with level <= `SLIDE_LEVEL` or
/// at a `---` break (whose `<hr>` is dropped). Deeper headings and other blocks
/// accrete onto the current slide. Empty slides (e.g. back-to-back breaks) are
/// dropped.
fn split_slides(blocks: &[Block], has_title: bool) -> Vec<SlideBuf> {
    let mut slides: Vec<SlideBuf> = Vec::new();
    let mut cur: Option<SlideBuf> = None;
    // Dedup section ids across the deck (`## X` twice -> `x`, `x-1`), so repeated
    // headings — common with auto-animate, where a title is shared — don't collide
    // in the DOM (hash + getElementById would otherwise only ever find the first).
    let mut id_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    // The front-matter title slide occupies id="title-slide"; reserve it so a slide
    // literally titled "Title Slide" dedups to "title-slide-1" instead of a dup id.
    if has_title {
        id_counts.insert("title-slide".to_string(), 1);
    }
    for b in blocks {
        if is_slide_break(&b.html) {
            slides.extend(cur.take());
            cur = Some(SlideBuf {
                level: 0,
                from_rule: true,
                id: None,
                blocks: Vec::new(),
            });
            continue; // the `<hr>` is the delimiter, not content
        }
        if let Some(level) = block_heading_level(&b.html)
            && level <= SLIDE_LEVEL
        {
            slides.extend(cur.take());
            // An explicit `{#id}` (carried verbatim in `data-slide-anchor`) stays exactly
            // so `@sec-x` / `#hash` resolve to this section; only a heading-text fallback is
            // slugged. Both dedup through the same map so a repeat can't collide.
            let id = match extract_attr(&b.html, "data-slide-anchor") {
                // `data-slide-anchor` is HTML-attr-escaped; unescape so render_section
                // re-escapes exactly once (else an id with & < > " double-escapes and its
                // `@ref`/`#hash` no longer resolves). The text-slug branch is already raw.
                Some(anchor) => dedup_with_suffix(unescape_html(&anchor), &mut id_counts),
                None => dedup_slug(&strip_tags(&b.html), &mut id_counts),
            };
            cur = Some(SlideBuf {
                level,
                from_rule: false,
                id: Some(id),
                blocks: vec![b.html.clone()],
            });
            continue;
        }
        match &mut cur {
            Some(s) => s.blocks.push(b.html.clone()),
            None => {
                cur = Some(SlideBuf {
                    level: 0,
                    from_rule: false,
                    id: None,
                    blocks: vec![b.html.clone()],
                })
            }
        }
    }
    slides.extend(cur);
    slides.retain(|s| !s.blocks.is_empty());
    slides
}

fn render_top(top: &Top, out: &mut String) {
    match top {
        Top::Slide(s) => render_section(s, out),
        Top::Stack { lead, children } => {
            out.push_str("<section>\n");
            render_section(lead, out);
            for c in children {
                render_section(c, out);
            }
            out.push_str("</section>\n");
        }
    }
}

fn render_section(s: &SlideBuf, out: &mut String) {
    // A per-slide background is emitted as `data-background-*` on the heading; hoist
    // it onto the <section> (where the deck engine reads it) and strip it off the heading.
    let (bg_attrs, lead) = s
        .blocks
        .first()
        .map(|b| take_bg_attrs(b))
        .unwrap_or_default();
    out.push_str("<section");
    if let Some(id) = s.id.as_deref().filter(|id| !id.is_empty()) {
        out.push_str(&format!(" id=\"{}\"", escape_attr(id)));
    }
    // ARIA: each leaf slide announces as a slide group so a screen reader can navigate
    // the deck. Additive on the <section> open tag (never on an inner [data-block-id]),
    // so block ids stay byte-stable. The "Slide N of M" aria-label is applied at runtime
    // by deck.js, where the flat slide order across vertical stacks is known.
    out.push_str(" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\"");
    // `data-level` is the slide's nesting depth (1 = a vertical-stack lead, 2 = a leaf
    // content slide). No runtime JS reads it; it exists as the corpus test's stable
    // slide-count anchor (`crates/core/tests/corpus.rs` counts `data-level="2"`), so keep
    // emitting it even though it looks inert here.
    if s.level != 0 {
        out.push_str(&format!(" data-level=\"{}\"", s.level));
    }
    // Estimated speaking time for this slide's `::: {.notes}` script, so the speaker
    // window can show planned-vs-elapsed and the build console a deck total. A slide
    // with no notes carries no attribute (absence is the "no script" signal).
    if let Some(secs) = script_secs(&s.blocks) {
        out.push_str(&format!(" data-script-secs=\"{secs}\""));
    }
    out.push_str(&bg_attrs);
    out.push_str(">\n");
    // `. . .` pause markers: drop the marker block and turn every block
    // after it (until the next pause or the end of the slide) into a `.fragment`,
    // so it shows on the next step via the existing fragment engine.
    let mut paused = false;
    for (i, b) in s.blocks.iter().enumerate() {
        if i == 0 {
            out.push_str(&lead);
            out.push('\n');
            continue;
        }
        if is_pause(b) {
            paused = true;
            continue;
        }
        // A per-slide background / `auto-animate` only applies at the <section>
        // level (hoisted from the slide's lead heading above). On a deeper heading
        // mid-slide it can't apply, so strip it rather than leak an inert `data-*`
        // attribute onto the heading.
        let (_, b) = take_bg_attrs(b);
        if paused {
            out.push_str(&add_fragment_class(&b));
        } else {
            out.push_str(&b);
        }
        out.push('\n');
    }
    out.push_str("</section>\n");
}

/// Words per minute assumed for spoken narration: deliberate presentation delivery
/// with pauses, slower than silent reading (~200-250) or casual speech (~150-160). A
/// single tuned constant, not a config knob (the estimate is an authoring aid; the
/// speaker window's planned-vs-elapsed readout is where an author calibrates it).
const SCRIPT_WPM: f64 = 130.0;

/// Estimated seconds to speak a slide's `::: {.notes}` script, from the notes' word
/// count at [`SCRIPT_WPM`]. `None` when the slide has no notes, so no attribute is
/// emitted and the slide is excluded from the deck's "scripted" tally.
fn script_secs(blocks: &[String]) -> Option<u64> {
    let words: usize = blocks
        .iter()
        .filter(|h| is_notes_block(h))
        // `strip_tags_separated` inserts a space at every tag boundary, so text from
        // adjacent paragraphs inside the notes stays word-separated rather than fusing.
        .map(|h| strip_tags_separated(h).split_whitespace().count())
        .sum();
    (words > 0).then(|| (words as f64 / SCRIPT_WPM * 60.0).round() as u64)
}

/// A `::: {.notes}` speaker-notes block (its own top-level slide block, matching how
/// the deck CSS hides `.notes` and the speaker window reads it).
fn is_notes_block(html: &str) -> bool {
    html.trim_start().starts_with("<div class=\"notes\"")
}

/// A pause marker: a paragraph whose only text is `. . .`. It is dropped
/// from the slide and turns the following block(s) into fragments.
fn is_pause(html: &str) -> bool {
    html.starts_with("<p") && strip_tags(html).trim() == ". . ."
}

/// Add `fragment` to a block's opening-tag class list (creating `class="fragment"`
/// when none exists), so the existing fragment engine hides it until its step.
fn add_fragment_class(html: &str) -> String {
    let Some(gt) = tag_end(html) else {
        return html.to_string();
    };
    let (open, rest) = html.split_at(gt);
    if let Some(ci) = open.find("class=\"") {
        let at = ci + "class=\"".len();
        format!("{}fragment {}{}", &open[..at], &open[at..], rest)
    } else {
        format!("{open} class=\"fragment\"{rest}")
    }
}

/// Pull any `data-background-*` attributes out of a slide's lead block (its heading)
/// — they sit in the opening tag — returning (attrs for the `<section>`, lead block
/// with them removed).
fn take_bg_attrs(html: &str) -> (String, String) {
    if !html.contains("data-background")
        && !html.contains("data-auto-animate")
        && !html.contains("data-slide-anchor")
    {
        return (String::new(), html.to_string());
    }
    let gt = tag_end(html).unwrap_or(html.len());
    let (head, tail) = html.split_at(gt);
    let mut attrs = String::new();
    let mut rest = String::new();
    let mut i = 0;
    while i < head.len() {
        // `data-slide-anchor` is consumed by the slide model as the section id — drop it
        // from the heading (don't hoist it as a stray attr); background/auto-animate hoist
        // onto the `<section>`.
        let is_anchor = head[i..].starts_with(" data-slide-anchor");
        if (is_anchor
            || head[i..].starts_with(" data-background")
            || head[i..].starts_with(" data-auto-animate"))
            && let Some(eq) = head[i..].find("=\"")
            && let Some(qend) = head[i + eq + 2..].find('"')
        {
            let end = i + eq + 2 + qend + 1;
            if !is_anchor {
                attrs.push_str(&head[i..end]);
            }
            i = end;
            continue;
        }
        let ch = head[i..].chars().next().unwrap();
        rest.push(ch);
        i += ch.len_utf8();
    }
    (attrs, format!("{rest}{tail}"))
}

fn is_slide_break(html: &str) -> bool {
    html.starts_with("<hr")
}
