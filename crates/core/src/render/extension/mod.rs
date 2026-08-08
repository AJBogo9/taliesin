//! Declarative shortcodes: expand `{{< name args >}}` invocations to inline HTML. The
//! built-ins are `{{< video clip.mp4 >}}` (a framed screencast) and `{{< input … >}}`
//! (a reactive control).
//! Line-preserving so the include source
//! map stays valid; `use super::*` reaches the shared `Warning` and HTML-escape helpers.

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
    // bind the same reactive name get distinct DOM ids (`tali-in-rate`, `tali-in-rate-1`). Threaded
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

/// Every built-in shortcode name the tool implements.
///
/// [`SHORTCODE_SPECS`] is NOT this list and must not be used as one: it holds only the
/// shortcodes whose *arguments* `render_shortcode` lints. `input` is dispatched ahead of
/// it in [`expand_in_line`], and `include` is resolved a whole pass earlier
/// (`crate::includes`). A feature report built on `SHORTCODE_SPECS` alone would report two
/// of the four as not existing.
pub const SHORTCODE_NAMES: &[&str] = &["video", "input", "include"];

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
                Some(html) => {
                    // Lint the ARGUMENTS of a shortcode that rendered. Until this landed,
                    // `render_shortcode` had no warning sink at all, so a typo'd flag
                    // (`control` for `controls`) or key (`postr=` for `poster=`) silently
                    // did nothing — the only typo surface in the tool that said nothing.
                    let toks = tokenize_args(inner);
                    if let Some((name, args)) = toks.split_first() {
                        validate_shortcode_args(name, args, line_no, warnings);
                    }
                    out.push_str(&html);
                }
                None => {
                    // Not a built-in shortcode. Keep it verbatim (nothing is lost), but
                    // warn: a typo'd shortcode name should be visible in the build log /
                    // preview diagnostics, not shipped as literal text into the page.
                    // `include` is handled in an earlier pass (`includes::resolve`); a
                    // leftover one means that pass already reported it, so don't double-warn.
                    let name = inner.split_whitespace().next().unwrap_or(inner);
                    // A KNOWN built-in that returned `None` is not an unknown shortcode: it
                    // is a built-in missing its positional path. Reporting it as unknown
                    // sent the author hunting a spelling that was already right.
                    if SHORTCODE_SPECS.iter().any(|(n, _, _)| *n == name) {
                        // Two different mistakes reach this branch, and reporting the
                        // second as the first is the failure this comment's neighbour
                        // already warns about: a source that IS written but is a URL must
                        // not be reported as a missing one, or the author goes hunting for
                        // a path they can plainly see.
                        let toks = tokenize_args(inner);
                        let src = toks
                            .split_first()
                            .and_then(|(_, args)| source_path(name, args));
                        warnings.push(
                            match src.as_deref().and_then(|s| url_scheme(s).map(|k| (s, k))) {
                                Some((s, scheme)) => {
                                    refused_url(name, "source", s, scheme, line_no)
                                }
                                None => Warning::new(format!(
                                    "`{{{{< {name} >}}}}` at line {line_no} has no source path \
                                     (write `{{{{< {name} file >}}}}`)"
                                ))
                                .at(None, line_no as u32),
                            },
                        );
                    } else if name != "include" {
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

/// Render one built-in `name args` shortcode (`video`), or `None` for any
/// other name (left verbatim by the caller). Args are `key=value` (named) or bare
/// (positional path); quotes group values with spaces.
fn render_shortcode(inner: &str) -> Option<String> {
    let toks = tokenize_args(inner);
    let (name, args) = toks.split_first()?;
    // `{{< video clip.mp4 [controls=false] [audio] [captions=clip.vtt] [dark=clip-dark.mp4]
    // [poster=…] [caption="…"] >}}` — a framed screencast (never autoplaying: playback is
    // user-initiated, see `video_html`), authored in Markdown so a page needs no raw
    // `<video>` HTML. With `dark=`, the light clip plays on a light page and the dark clip
    // on a dark page (toggled by `html[data-theme]`), so the screencast matches the theme.
    if name == "video" {
        // A path-valued named argument carrying a URL is DROPPED rather than failing the
        // whole shortcode: the clip still plays, it just plays without the poster (or the
        // dark variant, or the captions). That is the same graceful degradation
        // `validate_shortcode_args` already gives a typo'd option — "renders with what it
        // understood" — and `validate_shortcode_args` is what says so out loud.
        let path_arg = |key: &str| shortcode_named(args, key).filter(|v| url_scheme(v).is_none());
        return source_path("video", args)
            .filter(|src| url_scheme(src).is_none())
            .map(|src| {
                video_html(
                    &src,
                    path_arg("dark").as_deref(),
                    path_arg("poster").as_deref(),
                    shortcode_named(args, "caption").as_deref(), // prose, not a path
                    path_arg("captions").as_deref(),
                    playback_mode(args),
                )
            });
    }
    None
}

/// How a `{{< video >}}` plays back. Native controls are the DEFAULT (2026-08-03, visual
/// minimalism pass): hover-play and the click-to-lightbox touch path were both deleted, so
/// the browser's own control bar is the reader's only remaining way to start a clip at all.
///
/// * [`Playback::Controls`] (the default, or an explicit bare `controls`) — muted, looping,
///   native controls (scrubber, keyboard, fullscreen, PiP).
/// * [`Playback::Bare`] (`controls=false`) — muted, looping, no control bar at all: the
///   author's explicit opt-out for a purely decorative clip with no reader-facing play
///   affordance.
/// * [`Playback::Sound`] (`audio`) — a narrated explainer: native controls, and neither
///   `muted` nor `loop` (narration you cannot hear is pointless; narration that restarts
///   itself is hostile).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Playback {
    Controls,
    Bare,
    Sound,
}

/// Read the playback ladder off a `{{< video >}}`'s arguments.
///
/// `audio` forces `controls` (unmuted media with no pause/volume control is a WCAG 1.4.2
/// failure), and so does `captions=` (a `<track default>` with no control bar shows captions
/// the reader cannot turn off) — both win over an explicit `controls=false`, so neither
/// incoherent combination can be authored at all. `controls=false` is otherwise the one way
/// to opt out of the new default; a bare `controls` is still accepted (redundant now, but
/// harmless) so content written before the default flipped is unaffected.
fn playback_mode(args: &[String]) -> Playback {
    if shortcode_flag(args, "audio") {
        Playback::Sound
    } else if shortcode_named(args, "controls").as_deref() == Some("false")
        && shortcode_named(args, "captions").is_none()
    {
        Playback::Bare
    } else {
        Playback::Controls
    }
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

/// The first bare (non `key=value`) argument of a shortcode: the path to the media
/// file, relative to the embedding page.
///
/// A token is a *named* argument only when it looks like `key=value` with a plain
/// identifier key (`[A-Za-z][A-Za-z0-9_-]*` before the first `=`). Anything else is the
/// positional path, so a path carrying a query string (`clip.mp4?token=abc`) is **not**
/// mistaken for a named arg just because it contains an `=` after the `?`.
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

/// The URL scheme a shortcode's path argument carries, if any.
///
/// `{{< video tour.mp4 >}}` documents its positional argument as a path **relative to
/// the embedding page**, so a URL there names nothing the builder can reach. A
/// scheme-bearing token is therefore not a path at all, and passing it through put an
/// author-controlled URL directly into a `<video src>` with nothing but attribute
/// escaping in the way. It also slipped past `check`'s missing-local-media diagnostic,
/// which only looks at local files.
///
/// Two boundaries worth stating, because both are shapes that *look* like a scheme:
///
/// - A **single-letter** scheme is not reported: that is a Windows drive (`C:/clips/x.mp4`).
/// - A **query string** is not reported: `clip.mp4?token=a:b` splits at the first `:` into
///   a would-be scheme containing `?` and `=`, which the grammar below rejects. That case
///   is load-bearing — `is_named_arg` already goes out of its way to keep such a path
///   positional, and re-breaking it here would undo that.
///
/// Protocol-relative `//host/x.mp4` is reported as `//`: it is a network fetch wearing a
/// path's clothes, and it is the one shape that looks relative and is not.
fn url_scheme(tok: &str) -> Option<&str> {
    if tok.starts_with("//") {
        return Some("//");
    }
    let (scheme, _) = tok.split_once(':')?;
    let mut chars = scheme.chars();
    let head_ok = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic());
    let tail_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    (head_ok && tail_ok && scheme.len() > 1).then_some(scheme)
}

/// The positional source path of a built-in, by name — the one place the "which token is
/// the path?" rule lives, so the renderer and the diagnostic that explains a refusal read
/// the same token instead of re-deriving it from two copies of the rule.
fn source_path(name: &str, args: &[String]) -> Option<String> {
    match name {
        "video" => video_path(args),
        _ => None,
    }
}

/// The `key=value` arguments whose value is a PATH (resolved relative to the page), as
/// opposed to prose like `caption=` / `title=`. Kept beside [`SHORTCODE_SPECS`] because it
/// is the same closed vocabulary viewed by type: these are the keys [`url_scheme`] guards,
/// and a path-valued key added there but not here is silently unguarded.
const PATH_KEYS: [(&str, &[&str]); 1] = [("video", &["dark", "poster", "captions"])];

/// The path-valued keys declared for `name`.
fn path_keys(name: &str) -> &'static [&'static str] {
    PATH_KEYS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, keys)| *keys)
        .unwrap_or(&[])
}

/// How a refused URL argument reads to the author: what they wrote, why it is not a path,
/// and what the argument actually takes.
fn refused_url(name: &str, what: &str, value: &str, scheme: &str, line_no: usize) -> Warning {
    let scheme = if scheme == "//" {
        "protocol-relative".to_string()
    } else {
        format!("`{scheme}:`")
    };
    Warning::new(format!(
        "`{{{{< {name} >}}}}` at line {line_no}: {what} `{value}` is a {scheme} URL, not a \
         path — this shortcode takes a file relative to the page"
    ))
    .at(None, line_no as u32)
}

/// The bare (valueless) flags `{{< video >}}` accepts. They are not paths, so
/// [`video_path`] must skip them: the positional source is otherwise "the first token that
/// is not `key=value`", and `{{< video controls clip.mp4 >}}` would take `controls` as the
/// clip.
const VIDEO_FLAGS: [&str; 2] = ["controls", "audio"];

/// The `key=value` arguments each built-in shortcode honors, as `(name, keys, flags)`.
/// **The vocabulary is closed**, which is what lets [`validate_shortcode_args`] tell a typo
/// from an argument it simply has not heard of — the same closed-vocabulary contract front
/// matter, cell options and `_site.yml` are linted against.
const SHORTCODE_SPECS: [(&str, &[&str], &[&str]); 1] = [(
    // `controls` is both a bare flag (`VIDEO_FLAGS`, redundant now it is the default)
    // AND a named key (`controls=false`, the opt-out) — the two spellings coexist
    // deliberately so pre-existing `{{< video … controls >}}` content still validates.
    "video",
    &["dark", "poster", "caption", "captions", "controls"],
    &VIDEO_FLAGS,
)];

/// Whether a bare token is more plausibly a **misspelled flag** than a source path. A path
/// carries an extension or a directory separator; a flag is a bare word. Without this the
/// positional-source rule ("the first token that is neither `key=value` nor a known flag")
/// hands the `src` to the typo: `{{< video control tour.mp4 >}}` rendered
/// `<video src="control">`, which is a broken player rather than a missing option.
fn looks_like_flag_typo(tok: &str, flags: &[&'static str]) -> bool {
    !tok.contains('.')
        && !tok.contains('/')
        && flags
            .iter()
            .any(|f| crate::frontmatter::closest(tok, &[f]).is_some())
}

/// The positional source path of a `{{< video >}}`: the first argument that is neither a
/// `key=value` named argument, nor one of the bare [`VIDEO_FLAGS`], nor a near-miss
/// spelling of one — so flag order is free and a typo'd flag does not become the clip.
fn video_path(args: &[String]) -> Option<String> {
    args.iter()
        .find(|a| {
            !is_named_arg(a)
                && !VIDEO_FLAGS.contains(&a.as_str())
                && !looks_like_flag_typo(a, &VIDEO_FLAGS)
        })
        .cloned()
}

/// Lint one built-in shortcode's arguments against its closed vocabulary, in the same
/// "unknown X `y` (did you mean `z`?)" voice the front-matter and `_site.yml` linters use.
///
/// **Why this exists at all:** `render_shortcode` had no warning sink, so
/// `{{< video x.mp4 control >}}` produced no controls and said nothing — the only typo
/// surface in the tool that was silent. It is a *warning*, never a failure: the shortcode
/// still renders with the options it did understand, exactly as before.
///
/// Everything is reported per token, so one line can name more than one mistake. The
/// positional path is not validated here (a missing or misspelled file is the asset
/// checker's job, and a path is not a vocabulary).
fn validate_shortcode_args(
    name: &str,
    args: &[String],
    line_no: usize,
    warnings: &mut Vec<Warning>,
) {
    let Some((_, keys, flags)) = SHORTCODE_SPECS.iter().find(|(n, _, _)| *n == name) else {
        return; // not a built-in with a declared vocabulary (`input` lints its own)
    };
    let mut saw_path = false;
    for tok in args {
        if is_named_arg(tok) {
            let key = tok.split('=').next().unwrap_or(tok);
            if !keys.contains(&key) {
                warnings.push(
                    Warning::new(format!(
                        "unknown `{{{{< {name} >}}}}` argument `{key}=`{} at line {line_no}",
                        suggestion(key, keys, flags)
                    ))
                    .at(None, line_no as u32),
                );
            } else if path_keys(name).contains(&key) {
                // A KNOWN key whose value is a path: the renderer drops it when it carries
                // a URL, so this is what tells the author it was dropped. `caption=` is
                // prose and is deliberately not in `path_keys`, so a colon in a sentence
                // never reaches here.
                let value = tok.split_once('=').map(|(_, v)| v).unwrap_or("");
                if let Some(scheme) = url_scheme(value) {
                    warnings.push(refused_url(
                        name,
                        &format!("`{key}=`"),
                        value,
                        scheme,
                        line_no,
                    ));
                }
            }
        } else if flags.contains(&tok.as_str()) {
            // a valid bare flag
        } else if !saw_path && !looks_like_flag_typo(tok, flags) {
            saw_path = true; // the positional source path
        } else {
            warnings.push(
                Warning::new(format!(
                    "unknown `{{{{< {name} >}}}}` option `{tok}`{} at line {line_no}",
                    suggestion(tok, keys, flags)
                ))
                .at(None, line_no as u32),
            );
        }
    }
}

/// " (did you mean `x`?)" for the nearest name in either vocabulary, or "". Named keys are
/// offered with their `=` so the suggestion is the literal text to type.
fn suggestion(tok: &str, keys: &[&'static str], flags: &[&'static str]) -> String {
    let all: Vec<&'static str> = keys.iter().chain(flags.iter()).copied().collect();
    match crate::frontmatter::closest(tok, &all) {
        Some(hit) if keys.contains(&hit) => format!(" (did you mean `{hit}=`?)"),
        Some(hit) => format!(" (did you mean `{hit}`?)"),
        None => String::new(),
    }
}

/// Whether a bare (valueless) flag token is present, matched whole.
fn shortcode_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// A shortcode's `key=value` argument by name (quotes already stripped by the tokenizer).
fn shortcode_named(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|a| a.strip_prefix(&prefix).map(str::to_string))
}

/// The built-in `{{< input name="k" type="slider" … >}}` reactive control: a static,
/// keyboard-accessible labeled control whose value feeds the `{js}` reactive graph
/// (tali-js.js registers `[data-tali-input]` and reuses the same `registerInput`/`scheduleFrom`
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
    // A line-based id (the old `tali-in-<line>`) re-hashes the block on any shift, forcing a live
    // re-render that discards the control's DOM/JS state (and, in a deck, defeats the
    // section-signature re-mount that keeps untouched slides alive). Deduped so two controls
    // binding the same name still get unique ids; an anonymous control (no name, hence no
    // reactive identity to preserve) keeps the line-based fallback.
    let ctrl_id = if name.is_empty() {
        format!("tali-in-{line_no}")
    } else {
        dedup_with_suffix(format!("tali-in-{}", slugify(&name)), input_ids)
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
                "<select id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\">{opts}</select>"
            )
        }
        "checkbox" => {
            let checked = if value.as_deref() == Some("true") {
                " checked"
            } else {
                ""
            };
            format!(
                "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\" type=\"checkbox\"{checked}>"
            )
        }
        "text" => format!(
            "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\" type=\"text\"{}>",
            num_attr("value")
        ),
        other => {
            // slider/number: numeric, sharing min/max/step/value. A `slider` is HTML's
            // `type="range"`; the two are the same control under different names, which is
            // why offering both spellings to the author bought nothing.
            let html_type = if other == "number" { "number" } else { "range" };
            format!(
                "<input id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\" type=\"{html_type}\"{}{}{}{}>",
                num_attr("min"),
                num_attr("max"),
                num_attr("step"),
                num_attr("value")
            )
        }
    };
    // A slider's value is not visible in the control itself, so it gets a readout. `for`
    // ties it to the control it reflects, so AT announces them together.
    let readout = match kind.as_str() {
        "slider" => format!(
            "<output class=\"tali-input-out\" for=\"{ctrl_id}\" data-tali-out>{}</output>",
            html_escape(value.as_deref().unwrap_or(""))
        ),
        _ => String::new(),
    };
    let label_html = format!(
        "<label class=\"tali-input-label\" for=\"{ctrl_id}\">{}</label>",
        html_escape(&label)
    );
    format!("<div class=\"tali-input\">{label_html}{control}{readout}</div>")
}

/// The HTML for a `{{< video >}}`: a framed `<video>` with an optional caption. Playback is
/// **never** `autoplay` — an autoplaying loop beside body text is a WCAG 2.2.2 ("Pause,
/// Stop, Hide") failure — and the [`Playback`] ladder decides how the reader starts it: by
/// default the clip is muted + looping with native `controls` (the browser's own player —
/// scrubber, keyboard, volume, fullscreen, PiP — no player library, and no restyling of
/// controls the browser already ships), `controls=false` opts out of the control bar
/// entirely, and `audio` unmutes/unloops and forces controls back on. `captions=` adds a
/// caption `<track>`. With a `dark` source, both clips are emitted and CSS shows the one
/// matching `html[data-theme]`. Raw-HTML, passed through.
fn video_html(
    src: &str,
    dark: Option<&str>,
    poster: Option<&str>,
    caption: Option<&str>,
    captions: Option<&str>,
    playback: Playback,
) -> String {
    let poster_attr = poster
        .map(|p| format!(" poster=\"{}\"", escape_attr(p)))
        .unwrap_or_default();
    // `muted loop` is the silent-screencast contract; a narrated clip drops both (you cannot
    // hear a muted narration, and a lecture that silently restarts itself is hostile).
    // `controls` hands playback to the browser's native player.
    let (silent_attrs, controls_attr) = match playback {
        Playback::Bare => (" muted loop", ""),
        Playback::Controls => (" muted loop", " controls"),
        Playback::Sound => ("", " controls"),
    };
    // A caption track for the narration (WCAG 1.2.2). `default` shows it without a click;
    // the control bar `controls` guarantees (see `playback_mode`) is what lets it be turned
    // back off. No `srclang`: it is optional for `kind="captions"`, and guessing a language
    // would be worse than omitting it.
    let track = captions
        .map(|c| {
            format!(
                "<track kind=\"captions\" src=\"{}\" label=\"Captions\" default>",
                escape_attr(c)
            )
        })
        .unwrap_or_default();
    // The caption names the video for assistive tech; a caption-less clip falls back to what
    // it is — a silent "Screencast", or a narrated "Video". Escaped since it lands in a
    // double-quoted attribute.
    let label_attr = format!(
        " aria-label=\"{}\"",
        escape_attr(caption.unwrap_or(match playback {
            Playback::Sound => "Video",
            _ => "Screencast",
        }))
    );
    // A light/dark PAIR ships both clips but only one is ever visible, so the theme-hidden
    // one must not download. The pair carries `data-src` (no `src`); `syncThemeVideos`
    // (theme.rs) promotes `data-src`→`src` on the visible variant only, on load + every
    // theme change — so exactly one clip downloads. A single-source video keeps an eager
    // `src` (works without JS; nothing to save). `preload="metadata"` renders the first
    // frame as a still while paused (no autoplay forces the load anymore). `tabindex="0"`
    // makes the clip keyboard-reachable: focus no longer starts playback itself (hover-play
    // was deleted 2026-08-03, visual minimalism pass; native `controls` is the only way to
    // play now), it just speeds keyboard reach to that control bar instead of requiring a
    // Tab through everything ahead of it.
    let video = |s: &str, class: &str, lazy: bool| {
        let src_attr = if lazy { "data-src" } else { "src" };
        format!(
            "<video{cls} {src_attr}=\"{}\"{poster_attr}{silent_attrs}{controls_attr} playsinline preload=\"metadata\" tabindex=\"0\"{label_attr}>{track}</video>",
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

#[cfg(test)]
mod arg_validation_tests {
    use super::*;

    /// Every warning `expand_shortcodes` produces for one line of source.
    fn warn_msgs(src: &str) -> Vec<String> {
        expand_shortcodes(src)
            .1
            .into_iter()
            .map(|w| w.message)
            .collect()
    }

    #[test]
    fn a_url_source_is_refused_because_the_built_in_takes_a_page_relative_path() {
        // Item 97. `{{< video >}}` documents its positional argument as a file relative to
        // the page, so a scheme-bearing token is not a path — yet it went straight into a
        // `<video src>` with only attribute escaping in the way, and slipped past `check`'s
        // missing-local-media diagnostic (which only looks at local files).
        for src in [
            "{{< video javascript:alert(1) >}}\n",
            "{{< video https://evil.example/x.mp4 >}}\n",
            "{{< video data:text/html,x >}}\n",
        ] {
            let (html, warnings) = expand_shortcodes(src);
            // The shortcode does not expand at all. What is left is the source text
            // verbatim — the existing "nothing is lost" path an unrecognised shortcode
            // already takes — so the URL survives as inert page text and never becomes an
            // `<iframe src>` / `<video src>`. Assert the ATTRIBUTE, not the substring:
            // the literal `{{< … >}}` still contains the URL, which is the point.
            assert!(
                !html.contains("<video"),
                "no media element may be built from a URL source: {html}"
            );
            assert!(
                !html.contains("src=\""),
                "and nothing may carry it in a src attribute: {html}"
            );
            let msgs: Vec<String> = warnings.into_iter().map(|w| w.message).collect();
            assert_eq!(msgs.len(), 1, "exactly one warning for {src:?}: {msgs:?}");
            assert!(
                msgs[0].contains("not a path"),
                "and it must say WHY, not report a missing source the author can see: \
                 {msgs:?}"
            );
        }
    }

    #[test]
    fn a_url_in_a_path_valued_argument_is_dropped_but_the_clip_still_plays() {
        // The named path arguments are the obvious bypass of the check above: `dark=` is
        // literally a second video source. Dropping the argument (rather than failing the
        // whole shortcode) matches how a typo'd option already degrades — "renders with
        // what it understood".
        let (html, warnings) = expand_shortcodes(
            "{{< video tour.mp4 dark=javascript:alert(1) poster=//e.x/p.png >}}\n",
        );
        assert!(
            html.contains("src=\"tour.mp4\"") || html.contains("data-src=\"tour.mp4\""),
            "the clip itself still plays: {html}"
        );
        assert!(
            !html.contains("javascript:alert(1)") && !html.contains("//e.x/p.png"),
            "neither URL reaches the page: {html}"
        );
        let msgs: Vec<String> = warnings.into_iter().map(|w| w.message).collect();
        assert_eq!(msgs.len(), 2, "one per refused argument: {msgs:?}");
        assert!(
            msgs.iter().any(|m| m.contains("`dark=`"))
                && msgs.iter().any(|m| m.contains("`poster=`")),
            "each names the argument it dropped: {msgs:?}"
        );
    }

    #[test]
    fn the_scheme_check_leaves_real_paths_alone() {
        // The controls that stop the check above from passing by refusing everything.
        // Each of these is a shape that LOOKS scheme-ish and is a legitimate path.
        for src in [
            "{{< video clip.mp4 >}}\n",
            // A query string: `is_named_arg` already goes out of its way to keep this
            // positional, and splitting at the first `:` must not undo that.
            "{{< video clip.mp4?token=a:b >}}\n",
            // A Windows drive is a single-letter "scheme" and is not one.
            "{{< video C:/clips/tour.mp4 >}}\n",
            "{{< video tour.mp4 dark=tour-dark.mp4 poster=tour.jpg caption=\"A: a tour\" >}}\n",
        ] {
            let (_, warnings) = expand_shortcodes(src);
            let msgs: Vec<String> = warnings.into_iter().map(|w| w.message).collect();
            assert!(msgs.is_empty(), "{src:?} must not warn: {msgs:?}");
        }
        // And a caption is prose: a colon in a sentence is not a scheme.
        let (html, _) = expand_shortcodes("{{< video tour.mp4 caption=\"Fig 1: the tour\" >}}\n");
        assert!(html.contains("Fig 1: the tour"), "caption survives: {html}");
    }

    #[test]
    fn a_typod_bare_flag_warns_with_a_did_you_mean() {
        // Item 77 residual: `{{< video x.mp4 control >}}` (for `controls`) got no controls
        // and no warning, because `render_shortcode` had no warning sink — while every
        // other typo surface in the tool (front matter, cell options, `_site.yml`) warns.
        let w = warn_msgs("{{< video tour.mp4 control >}}\n");
        assert_eq!(w.len(), 1, "exactly one warning: {w:?}");
        assert!(
            w[0].contains("control") && w[0].contains("controls"),
            "the typo must be named and its fix suggested: {w:?}"
        );
    }

    #[test]
    fn a_typod_named_argument_warns_too() {
        // Equally silent today, and the same one-character mistake: `postr=` for `poster=`.
        let w = warn_msgs("{{< video tour.mp4 postr=cover.png >}}\n");
        assert_eq!(w.len(), 1, "exactly one warning: {w:?}");
        assert!(
            w[0].contains("postr") && w[0].contains("poster"),
            "the typo'd key must be named and its fix suggested: {w:?}"
        );
    }

    #[test]
    fn a_typod_flag_before_the_path_does_not_steal_the_source() {
        // Worse than ignored: the positional source is "the first token that is neither
        // `key=value` nor a known flag", so a typo'd flag WRITTEN FIRST became the `src`
        // and the real clip became a stray argument. A bare token with no `.` and no `/`
        // that is within edit distance 2 of a known flag is read as the typo it is.
        let (html, warnings) = expand_shortcodes("{{< video control tour.mp4 >}}\n");
        assert!(
            html.contains("src=\"tour.mp4\""),
            "the real clip must still be the source: {html}"
        );
        let msgs: Vec<_> = warnings.into_iter().map(|w| w.message).collect();
        assert_eq!(msgs.len(), 1, "exactly one warning: {msgs:?}");
        assert!(msgs[0].contains("control"), "…naming the typo: {msgs:?}");
    }

    #[test]
    fn every_valid_spelling_stays_silent() {
        // The half that keeps this honest: a warning on correct authoring is worse than
        // no warning at all. Each of these is a documented, working invocation.
        for src in [
            "{{< video tour.mp4 >}}\n",
            "{{< video tour.mp4 controls >}}\n",
            "{{< video tour.mp4 audio captions=tour.vtt >}}\n",
            "{{< video tour.mp4 dark=tour-dark.mp4 poster=cover.png caption=\"A tour\" >}}\n",
            // A path carrying a query string: the `=` belongs to the value, not a key.
            "{{< video clip.mp4?token=abc >}}\n",
        ] {
            assert!(
                warn_msgs(src).is_empty(),
                "valid authoring must not warn ({src:?}): {:?}",
                warn_msgs(src)
            );
        }
    }

    #[test]
    fn a_builtin_with_no_source_path_says_so_instead_of_unknown_shortcode() {
        // `render_shortcode` returns `None` when a known built-in has no positional path,
        // which fell through to the unknown-name branch — so a real `video` was reported
        // as an unknown shortcode, sending the author to hunt a spelling that was right.
        let w = warn_msgs("{{< video >}}\n");
        assert_eq!(w.len(), 1, "exactly one warning: {w:?}");
        assert!(
            !w[0].contains("unknown shortcode"),
            "a known built-in must not be reported as unknown: {w:?}"
        );
        assert!(
            w[0].contains("video") && w[0].contains("path"),
            "it must say what is missing: {w:?}"
        );
        // A genuinely unknown name keeps its own message.
        let u = warn_msgs("{{< vidoe tour.mp4 >}}\n");
        assert!(
            u.iter().any(|m| m.contains("unknown shortcode")),
            "an unknown name still reports as unknown: {u:?}"
        );
    }

    #[test]
    fn an_argument_warning_is_located_on_its_own_line() {
        // Located like every other render warning, so the preview can link to it.
        let (_, warnings) = expand_shortcodes("intro\n\n{{< video tour.mp4 control >}}\n");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].line, Some(3), "warning: {:?}", warnings[0]);
    }

    #[test]
    fn a_shortcode_inside_a_code_fence_is_not_validated() {
        // The documentation case: the guide shows `{{< video … >}}` spellings in fenced
        // blocks and inline code. Those are examples, not invocations — expansion already
        // skips them, and validation must not reach around that.
        assert!(warn_msgs("```\n{{< video x.mp4 control >}}\n```\n").is_empty());
        assert!(warn_msgs("see `{{< video x.mp4 control >}}` here\n").is_empty());
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;

    #[test]
    fn slider_output_is_tied_to_its_control_with_for() {
        // PA-M9: the live `<output>` readout must carry `for="<control-id>"` so AT associates the
        // reading with the range input it reflects. The control id is name-derived (`tali-in-<slug>`).
        let mut ids = std::collections::HashMap::new();
        let mut warns = Vec::new();
        let html = input_shortcode(
            &["type=slider".to_string(), "name=freq".to_string()],
            1,
            &mut warns,
            &mut ids,
        );
        assert!(
            html.contains("id=\"tali-in-freq\""),
            "sanity: the control carries its id: {html}"
        );
        assert!(
            html.contains("for=\"tali-in-freq\" data-tali-out"),
            "the <output> readout must be tied to its control via for=: {html}"
        );
    }
}
