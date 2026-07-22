//! Declarative shortcodes: expand `{{< name args >}}` invocations to inline HTML. The
//! built-ins are `{{< embed deck.tmd >}}` (an isolated deck iframe), `{{< video
//! clip.mp4 >}}` (a framed screencast), and `{{< input … >}}` (a reactive control).
//! Line-preserving so the include source map stays valid; `use super::*` reaches the
//! shared `Warning` and HTML-escape helpers.

use super::*;

/// Expand declarative shortcodes (`{{< name args >}}`) to inline HTML. Line-preserving
/// — each invocation opens and closes on one line — so the include source map stays
/// valid. Fenced code blocks are skipped, so a `{{< … >}}` shown as an *example* in a
/// code fence stays literal; unknown shortcodes are left untouched (with a warning).
pub(super) fn expand_shortcodes(src: &str) -> (String, Vec<Warning>) {
    let mut warnings: Vec<Warning> = Vec::new();
    if !src.contains("{{<") {
        return (src.to_string(), warnings);
    }
    let mut out = String::with_capacity(src.len());
    let mut in_code = false;
    // Deduplicates `{{< input >}}` control ids across the document, so two controls that
    // bind the same reactive name get distinct DOM ids (`qin-rate`, `qin-rate-1`). Threaded
    // here (not per line) because the id must be name-based, not line-based — see
    // `input_shortcode`.
    let mut input_ids: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line); // literal inside a code block (it's an example)
        } else {
            out.push_str(&expand_in_line(line, i + 1, &mut warnings, &mut input_ids));
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    (out, warnings)
}

/// Replace every `{{< name args >}}` that opens and closes on this line with its
/// declared template; leave unrecognized ones (and unterminated spans) verbatim.
/// Inline code spans (`` `…` ``, ``` ``…`` ```) are copied through untouched, so a
/// shortcode shown as an *example* in backticks (e.g. `` `{{< embed x.tmd >}}` ``)
/// stays literal — mirroring how fenced blocks are skipped in `expand_shortcodes`.
fn expand_in_line(
    line: &str,
    line_no: usize,
    warnings: &mut Vec<Warning>,
    input_ids: &mut std::collections::HashMap<String, u32>,
) -> String {
    if !line.contains("{{<") {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < line.len() {
        if bytes[i] == b'`' {
            // An inline code span: copy through the matching backtick run verbatim
            // so a `{{< … >}}` inside it is not expanded.
            let run = line[i..].bytes().take_while(|&c| c == b'`').count();
            let ticks = &line[i..i + run];
            if let Some(rel) = line[i + run..].find(ticks) {
                let close = i + run + rel + run;
                out.push_str(&line[i..close]);
                i = close;
            } else {
                out.push_str(ticks); // unterminated run: copy the backticks, keep scanning
                i += run;
            }
        } else if line[i..].starts_with("{{<") {
            let Some(rel_end) = line[i + 3..].find(">}}") else {
                out.push_str(&line[i..]); // no close on this line: leave as written
                break;
            };
            let end = i + 3 + rel_end;
            let inner = line[i + 3..end].trim();
            // The built-in `{{< input >}}` reactive control needs the line number + the
            // warning sink (for located diagnostics), which render_shortcode doesn't carry,
            // so it is expanded here.
            if inner.split_whitespace().next() == Some("input") {
                let toks = tokenize_args(inner);
                out.push_str(&input_shortcode(&toks[1..], line_no, warnings, input_ids));
                i = end + 3;
                continue;
            }
            match render_shortcode(inner) {
                Some(html) => out.push_str(&html),
                None => {
                    // Not a built-in shortcode. Keep it verbatim (nothing is lost), but
                    // warn: a typo'd shortcode name should be visible in the build log /
                    // preview diagnostics, not shipped as literal text into the page.
                    // `include` is handled in an earlier pass (`includes::resolve`); a
                    // leftover one means that pass already reported it, so don't double-warn.
                    let name = inner.split_whitespace().next().unwrap_or(inner);
                    if name != "include" {
                        warnings.push(
                            Warning::new(format!(
                                "unknown shortcode `{{{{< {name} >}}}}` at line {line_no} \
                                 (left as literal text)"
                            ))
                            .at(None, line_no as u32),
                        );
                    }
                    out.push_str(&line[i..end + 3]); // unknown: keep verbatim
                }
            }
            i = end + 3;
        } else {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Render one built-in `name args` shortcode (`embed` / `video`), or `None` for any
/// other name (left verbatim by the caller). Args are `key=value` (named) or bare
/// (positional path); quotes group values with spaces.
fn render_shortcode(inner: &str) -> Option<String> {
    let toks = tokenize_args(inner);
    let (name, args) = toks.split_first()?;
    // `{{< embed deck.tmd [title="…"] >}}` embeds another document's deck view in an
    // isolating iframe with a fullscreen affordance (the deck is built/served as a
    // dependency, see `embed_targets`).
    if name == "embed" {
        return embed_path(args).map(|p| embed_html(&p, embed_title(args).as_deref()));
    }
    // `{{< video clip.mp4 [dark=clip-dark.mp4] [poster=…] [caption="…"] >}}` — a framed,
    // autoplaying, muted, looping screencast, authored in Markdown so a page needs no raw
    // `<video>` HTML. With `dark=`, the light clip plays on a light page and the dark clip
    // on a dark page (toggled by `html[data-theme]`), so the screencast matches the theme.
    if name == "video" {
        return embed_path(args).map(|src| {
            video_html(
                &src,
                shortcode_named(args, "dark").as_deref(),
                shortcode_named(args, "poster").as_deref(),
                shortcode_named(args, "caption").as_deref(),
            )
        });
    }
    None
}

/// Whitespace-split `inner`, keeping quoted values (`key="a b"`) as one token and
/// stripping the surrounding quotes.
fn tokenize_args(inner: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// The first bare (non `key=value`) argument of an `embed`/`video` shortcode: the path
/// to the deck document or media file, relative to the embedding page.
///
/// A token is a *named* argument only when it looks like `key=value` with a plain
/// identifier key (`[A-Za-z][A-Za-z0-9_-]*` before the first `=`). Anything else is the
/// positional path, so a path carrying a query string (`clip.mp4?token=abc`) is **not**
/// mistaken for a named arg just because it contains an `=` after the `?`.
fn embed_path(args: &[String]) -> Option<String> {
    args.iter().find(|a| !is_named_arg(a)).cloned()
}

/// Whether `tok` is a `key=value` named shortcode argument: an identifier key
/// (`[A-Za-z][A-Za-z0-9_-]*`) immediately followed by `=`. A `?` (or any other
/// non-identifier character) before the first `=` means the `=` belongs to a query
/// string / value, not a key, so the token is positional (a path) instead.
fn is_named_arg(tok: &str) -> bool {
    let Some(key) = tok.split('=').next().filter(|_| tok.contains('=')) else {
        return false;
    };
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The optional `title="…"` argument (used as the iframe's accessible name).
fn embed_title(args: &[String]) -> Option<String> {
    shortcode_named(args, "title")
}

/// A shortcode's `key=value` argument by name (quotes already stripped by the tokenizer).
fn shortcode_named(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|a| a.strip_prefix(&prefix).map(str::to_string))
}

/// The built-in `{{< input name="k" type="slider" … >}}` reactive control: a static,
/// keyboard-accessible labeled control whose value feeds the `{js}` reactive graph
/// (qmd-js.js registers `[data-qmd-input]` and reuses the same `registerInput`/`scheduleFrom`
/// path as `//| viewof` cells). Five types (slider/range, number, checkbox, text, select);
/// the slider gets a live `<output>` readout. Emits located diagnostics (missing name,
/// unknown type with a did-you-mean, select without options) via `validate_input`. Raw-HTML,
/// passed through — the block model assigns it an id/sourcepos like any HTML block. Read-only:
/// reader interaction with the rendered view, never a source write.
fn input_shortcode(
    args: &[String],
    line_no: usize,
    warnings: &mut Vec<Warning>,
    input_ids: &mut std::collections::HashMap<String, u32>,
) -> String {
    let name = shortcode_named(args, "name").unwrap_or_default();
    let kind = shortcode_named(args, "type").unwrap_or_else(|| "slider".to_string());
    let label = shortcode_named(args, "label").unwrap_or_else(|| name.clone());
    let options = shortcode_named(args, "options");
    let value = shortcode_named(args, "value");
    for w in super::validate::validate_input(
        (!name.is_empty()).then_some(name.as_str()),
        Some(kind.as_str()),
        options.as_deref(),
        line_no,
        None,
    ) {
        warnings.push(w);
    }
    // Derive the control's DOM id from its reactive name, not the source line, so the
    // block's content-hash `data-block-id` stays stable when an edit above shifts the line.
    // A line-based id (the old `qin-<line>`) re-hashes the block on any shift, forcing a live
    // re-render that discards the control's DOM/JS state (and, in a deck, defeats the
    // section-signature re-mount that keeps untouched slides alive). Deduped so two controls
    // binding the same name still get unique ids; an anonymous control (no name, hence no
    // reactive identity to preserve) keeps the line-based fallback.
    let ctrl_id = if name.is_empty() {
        format!("qin-{line_no}")
    } else {
        dedup_with_suffix(format!("qin-{}", slugify(&name)), input_ids)
    };
    let name_a = escape_attr(&name);
    let num_attr = |k: &str| {
        shortcode_named(args, k)
            .map(|v| format!(" {k}=\"{}\"", escape_attr(&v)))
            .unwrap_or_default()
    };
    let control = match kind.as_str() {
        "select" => {
            let opts: String = options
                .as_deref()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(|o| {
                    let sel = if value.as_deref() == Some(o) {
                        " selected"
                    } else {
                        ""
                    };
                    format!("<option{sel}>{}</option>", html_escape(o))
                })
                .collect();
            format!(
                "<select id=\"{ctrl_id}\" class=\"tali-input-control\" data-qmd-input=\"{name_a}\">{opts}</select>"
            )
        }
        "checkbox" => {
            let checked = if value.as_deref() == Some("true") {
                " checked"
            } else {
                ""
            };
            format!(
                "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-qmd-input=\"{name_a}\" type=\"checkbox\"{checked}>"
            )
        }
        "text" => format!(
            "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-qmd-input=\"{name_a}\" type=\"text\"{}>",
            num_attr("value")
        ),
        other => {
            // slider/range/number: numeric, sharing min/max/step/value
            let html_type = if other == "number" { "number" } else { "range" };
            format!(
                "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-qmd-input=\"{name_a}\" type=\"{html_type}\"{}{}{}{}>",
                num_attr("min"),
                num_attr("max"),
                num_attr("step"),
                num_attr("value")
            )
        }
    };
    let readout = if kind == "slider" || kind == "range" {
        // `for` ties the readout to the control it reflects, so AT announces them together.
        format!(
            "<output class=\"tali-input-out\" for=\"{ctrl_id}\" data-qmd-out>{}</output>",
            html_escape(value.as_deref().unwrap_or(""))
        )
    } else {
        String::new()
    };
    format!(
        "<div class=\"tali-input\"><label class=\"tali-input-label\" for=\"{ctrl_id}\">{}</label>{control}{readout}</div>",
        html_escape(&label)
    )
}

/// The HTML for a `{{< video >}}`: a framed muted/looping `<video>` (a silent screencast)
/// with an optional caption. Playback is **user-initiated** (hover / focus / tap via the
/// `18-media.js` enhancer), never `autoplay` — an autoplaying loop beside body text is a
/// WCAG 2.2.2 ("Pause, Stop, Hide") failure. With a `dark` source, both clips are emitted
/// and CSS shows the one matching `html[data-theme]`. Raw-HTML, passed through.
fn video_html(
    src: &str,
    dark: Option<&str>,
    poster: Option<&str>,
    caption: Option<&str>,
) -> String {
    let poster_attr = poster
        .map(|p| format!(" poster=\"{}\"", escape_attr(p)))
        .unwrap_or_default();
    // The caption names the video for assistive tech; a caption-less clip is a generic
    // "Screencast". Escaped since it lands in a double-quoted attribute.
    let label_attr = format!(
        " aria-label=\"{}\"",
        escape_attr(caption.unwrap_or("Screencast"))
    );
    // A light/dark PAIR ships both clips but only one is ever visible, so the theme-hidden
    // one must not download. The pair carries `data-src` (no `src`); `syncThemeVideos`
    // (theme.rs) promotes `data-src`→`src` on the visible variant only, on load + every
    // theme change — so exactly one clip downloads. A single-source video keeps an eager
    // `src` (works without JS; nothing to save). `preload="metadata"` renders the first
    // frame as a still while paused (no autoplay forces the load anymore). `tabindex="0"`
    // makes the clip keyboard-reachable so focus can start it (parity with hover).
    let video = |s: &str, class: &str, lazy: bool| {
        let src_attr = if lazy { "data-src" } else { "src" };
        format!(
            "<video{cls} {src_attr}=\"{}\"{poster_attr} muted loop playsinline preload=\"metadata\" tabindex=\"0\"{label_attr}></video>",
            escape_attr(s),
            cls = if class.is_empty() {
                String::new()
            } else {
                format!(" class=\"{class}\"")
            },
        )
    };
    let videos = match dark {
        Some(d) => format!(
            "{}{}",
            video(src, "tali-video-light", true),
            video(d, "tali-video-dark", true)
        ),
        None => video(src, "", false),
    };
    let cap = caption
        .map(|c| format!("<figcaption>{}</figcaption>", html_escape(c)))
        .unwrap_or_default();
    format!("<figure class=\"tali-video\">{videos}{cap}</figure>")
}

/// Map a deck source path to its built output URL (`x.tmd` → `x.html`), leaving a
/// path that is already `.html` (or anything else) untouched.
fn deck_href(path: &str) -> String {
    match crate::ext::strip_source_ext(path) {
        Some(stem) => format!("{stem}.html"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;

    #[test]
    fn slider_output_is_tied_to_its_control_with_for() {
        // PA-M9: the live `<output>` readout must carry `for="<control-id>"` so AT associates the
        // reading with the range input it reflects. The control id is name-derived (`qin-<slug>`).
        let mut ids = std::collections::HashMap::new();
        let mut warns = Vec::new();
        let html = input_shortcode(
            &["type=slider".to_string(), "name=freq".to_string()],
            1,
            &mut warns,
            &mut ids,
        );
        assert!(
            html.contains("id=\"qin-freq\""),
            "sanity: the control carries its id: {html}"
        );
        assert!(
            html.contains("for=\"qin-freq\" data-qmd-out"),
            "the <output> readout must be tied to its control via for=: {html}"
        );
    }

    #[test]
    fn embed_external_link_announces_the_new_tab() {
        // PA-M11: the embed's `target="_blank"` "Open ↗" link gives no programmatic new-tab cue.
        // A visually-hidden suffix keeps the visible label terse while AT announces the new tab.
        let html = embed_html("deck.tmd", None);
        assert!(
            html.contains("target=\"_blank\""),
            "sanity: the link opens a new tab: {html}"
        );
        assert!(
            html.contains("<span class=\"tali-sr-only\"> (opens in a new tab)</span>"),
            "the new-tab link needs a visually-hidden cue for AT: {html}"
        );
    }
}

/// The HTML for an embedded deck: a responsive 16:9 iframe (isolating the deck's
/// full-viewport CSS/JS/keyboard from the host page) plus a fullscreen button and an
/// "open in a new tab" link. Emitted as a raw-HTML block, which the renderer passes
/// through.
fn embed_html(path: &str, title: Option<&str>) -> String {
    let href = escape_attr(&deck_href(path));
    // `title` lands in a double-quoted attribute, so escape `"` too (escape_attr,
    // not html_escape) — otherwise a `"` in the title breaks out of the attribute.
    let title = escape_attr(title.unwrap_or("Embedded slide deck"));
    // One fullscreen control only: the labelled `⤢ Fullscreen` button in the bar below.
    // (A second floating `⤢` overlay on the stage was redundant — same requestFullscreen().)
    format!(
        "<div class=\"tali-embed\">\
         <div class=\"tali-embed-stage\">\
         <iframe class=\"tali-embed-frame\" src=\"{href}\" title=\"{title}\" loading=\"lazy\" allowfullscreen></iframe>\
         </div>\
         <div class=\"tali-embed-bar\">\
         <button type=\"button\" class=\"tali-embed-btn\" onclick=\"this.closest('.tali-embed').querySelector('iframe').requestFullscreen()\">\u{2922} Fullscreen</button>\
         <a class=\"tali-embed-btn\" href=\"{href}\" target=\"_blank\" rel=\"noopener\">Open \u{2197}<span class=\"tali-sr-only\"> (opens in a new tab)</span></a>\
         </div></div>"
    )
}

/// Invoke `f` with the inner body of each `{{< … >}}` on `line` that is *not* inside
/// an inline code span, so a shortcode shown as an example in backticks is ignored
/// (the same discipline `expand_in_line` uses when expanding).
fn each_shortcode(line: &str, mut f: impl FnMut(&str)) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < line.len() {
        if bytes[i] == b'`' {
            let run = line[i..].bytes().take_while(|&c| c == b'`').count();
            let ticks = &line[i..i + run];
            match line[i + run..].find(ticks) {
                Some(rel) => i = i + run + rel + run,
                None => i += run,
            }
        } else if line[i..].starts_with("{{<") {
            let Some(rel_end) = line[i + 3..].find(">}}") else {
                break;
            };
            let end = i + 3 + rel_end;
            f(line[i + 3..end].trim());
            i = end + 3;
        } else {
            i += line[i..].chars().next().unwrap().len_utf8();
        }
    }
}

/// Every deck referenced by a `{{< embed PATH >}}` in `src` (paths as written,
/// relative to the page), deduped and in document order. Fenced and inline code are
/// skipped so an `embed` shown as an example stays inert. The site build/preview uses
/// this to also build/serve each referenced deck.
pub fn embed_targets(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_code = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        each_shortcode(line, |inner| {
            let toks = tokenize_args(inner);
            if let Some((name, args)) = toks.split_first()
                && name == "embed"
                && let Some(p) = embed_path(args)
                && !out.contains(&p)
            {
                out.push(p);
            }
        });
    }
    out
}
