//! reveal.js deck output: the one-shot deck page, the live-deck client
//! head/script, and the slide model (grouping blocks into `<section>`s by
//! heading level and `---` breaks). Split out of the render module; `use
//! super::*` pulls in the shared block model and helpers (Block, RenderedDoc,
//! html escaping, slugify, block_heading_level, KaTeX CSS, theme_style,
//! code_head/code_scripts).

use super::*;

/// Slide-specific tweaks layered over the deck theme (left-aligned content,
/// centered title slide, readable code/math).
const REVEAL_EXTRA_CSS: &str = include_str!("../../assets/css/reveal-extra.css");

/// qmd-fast's own deck engine, bundled (no CDN): `deck.css` is the layout and
/// `deck.js` the navigation/scaling engine, exposing a `window.Reveal`-shaped
/// facade so reveal extensions and the preview client work unchanged. Inlined
/// into both the one-shot page and the live client, like KaTeX/OJS/mermaid.
const DECK_CSS: &str = include_str!("../../assets/css/deck.css");
const DECK_JS: &str = include_str!("../../assets/js/deck.js");

pub(super) fn reveal_page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Only ship the (large) KaTeX stylesheet when the deck actually has math.
    let katex_css = if slides.contains("class=\"katex") {
        format!("<style>{KATEX_CSS}</style>\n")
    } else {
        String::new()
    };
    // Interactive `{ojs}` cells need the Observable runtime + init, exactly like an
    // HTML page — only shipped when the deck actually has OJS cells. The runtime
    // renders into the cell divs regardless of which slide is showing, so reactive
    // outputs are live the moment their slide appears.
    let (ojs_head_html, ojs_init_html) = if has_ojs(&slides) {
        (ojs_head(), ojs_init())
    } else {
        (String::new(), String::new())
    };
    // A custom `theme:` CSS layer and the `include-*` front-matter apply to decks
    // just like HTML pages — a deck (or an installed reveal theme extension) can
    // restyle reveal and inject head/body markup. `theme` comes after reveal's own
    // stylesheets so it overrides them; the css folded into `include-in-header`
    // follows last.
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no\" />\n\
         <title>{t}</title>\n{links}{katex_css}<style>{REVEAL_EXTRA_CSS}</style>\n{code_head}\n{ojs_head}{theme}{in_header}\
         </head>\n<body>\n{before_body}<div class=\"reveal\">\n<div class=\"slides\">\n{slides}</div>\n</div>\n\
         {script}\n<script>\n  Reveal.initialize({{ hash: true, slideNumber: 'c/t', center: false }});\n</script>\n\
         {code_scripts}\n\
         <script>document.addEventListener('DOMContentLoaded',function(){{window.qmdEnhanceCode&&window.qmdEnhanceCode(document.body);}});</script>\n\
         {ojs_init}{after_body}</body>\n</html>\n",
        links = format!("<style>{DECK_CSS}</style>\n"),
        theme = theme_style(&doc.theme_css),
        in_header = doc.includes.in_header,
        before_body = doc.includes.before_body,
        after_body = doc.includes.after_body,
        script = format!("<script>{DECK_JS}</script>"),
        code_head = code_head(),
        code_scripts = code_scripts(),
        ojs_head = ojs_head_html,
        ojs_init = ojs_init_html,
    )
}

/// `<head>` markup for the live deck client: the bundled deck layout plus the
/// KaTeX stylesheet (a live deck may gain math on any edit) and the slide
/// tweaks. The blog [`client_styles`] body CSS is deliberately omitted — it
/// would fight the deck layout.
pub fn reveal_client_head() -> String {
    format!(
        "<style>{DECK_CSS}</style>\n<style>{KATEX_CSS}</style>\n<style>{REVEAL_EXTRA_CSS}</style>"
    )
}

/// The deck engine `<script>` for the live deck client; load it before the
/// preview client so the `window.Reveal` facade is defined when the deck mounts.
pub fn reveal_client_script() -> String {
    format!("<script>{DECK_JS}</script>")
}

// --- reveal.js slide model ----------------------------------------------

/// Quarto's default `slide-level`: headings at this level start a new slide;
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

/// Build the inner HTML of reveal's `<div class="slides">`: an optional title
/// slide from front matter, then one `<section>` per slide. Blocks are grouped
/// into slides by heading level (`SLIDE_LEVEL`) and `---` breaks, with h1s
/// wrapping their h2s as a vertical stack.
pub fn slides_html(title: Option<&str>, subtitle: Option<&str>, blocks: &[Block]) -> String {
    let mut out = String::new();
    if let Some(title) = title {
        out.push_str("<section id=\"title-slide\" class=\"quarto-title-block center\">\n<h1 class=\"title\">");
        escape_html(title, &mut out);
        out.push_str("</h1>\n");
        if let Some(sub) = subtitle {
            out.push_str("<p class=\"subtitle\">");
            escape_html(sub, &mut out);
            out.push_str("</p>\n");
        }
        out.push_str("</section>\n");
    }
    for top in group_slides(blocks) {
        render_top(&top, &mut out);
    }
    out
}

/// Split blocks into flat slides at slide-level headings and `---` breaks,
/// then nest h2 slides under any preceding h1 as a vertical stack.
fn group_slides(blocks: &[Block]) -> Vec<Top> {
    let flat = split_slides(blocks);
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
fn split_slides(blocks: &[Block]) -> Vec<SlideBuf> {
    let mut slides: Vec<SlideBuf> = Vec::new();
    let mut cur: Option<SlideBuf> = None;
    // Dedup section ids across the deck (`## X` twice -> `x`, `x-1`), so repeated
    // headings — common with auto-animate, where a title is shared — don't collide
    // in the DOM (hash + getElementById would otherwise only ever find the first).
    let mut id_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
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
            cur = Some(SlideBuf {
                level,
                from_rule: false,
                id: Some(dedup_slug(&strip_tags(&b.html), &mut id_counts)),
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
    if s.level != 0 {
        out.push_str(&format!(" class=\"slide level{}\"", s.level));
    } else {
        out.push_str(" class=\"slide\"");
    }
    out.push_str(&bg_attrs);
    out.push_str(">\n");
    for (i, b) in s.blocks.iter().enumerate() {
        out.push_str(if i == 0 { &lead } else { b });
        out.push('\n');
    }
    out.push_str("</section>\n");
}

/// Pull any `data-background-*` attributes out of a slide's lead block (its heading)
/// — they sit in the opening tag — returning (attrs for the `<section>`, lead block
/// with them removed).
fn take_bg_attrs(html: &str) -> (String, String) {
    if !html.contains("data-background") && !html.contains("data-auto-animate") {
        return (String::new(), html.to_string());
    }
    let gt = html.find('>').unwrap_or(html.len());
    let (head, tail) = html.split_at(gt);
    let mut attrs = String::new();
    let mut rest = String::new();
    let mut i = 0;
    while i < head.len() {
        if (head[i..].starts_with(" data-background")
            || head[i..].starts_with(" data-auto-animate"))
            && let Some(eq) = head[i..].find("=\"")
            && let Some(qend) = head[i + eq + 2..].find('"')
        {
            let end = i + eq + 2 + qend + 1;
            attrs.push_str(&head[i..end]);
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
