//! Declarative shortcodes: expand `{{< name args >}}` invocations to inline HTML. The
//! built-ins are `{{< embed deck.tmd >}}` (an isolated deck iframe), `{{< video
//! clip.mp4 >}}` (a framed screencast), `{{< input … >}}` (a reactive control) and
//! `{{< dataset … >}}` (a data-provenance card). Line-preserving so the include source
//! map stays valid; `use super::*` reaches the shared `Warning` and HTML-escape helpers.

use super::*;

pub(crate) mod dataset;

/// What a shortcode may need beyond its own arguments. Only `{{< dataset >}}` uses it so
/// far: a provenance card reads the file it names (size, digest) and the front-matter
/// `datasets:` entry that annotates it, neither of which is on the invocation line.
///
/// `base_dir` is `None` for a render with no filesystem context, which is a real case
/// (`render_document` on a string). A dataset card then states only what was declared,
/// rather than claiming a size it could not measure.
pub(super) struct ShortcodeCtx<'a> {
    pub base_dir: Option<&'a std::path::Path>,
    pub datasets: Vec<dataset::Declared>,
}

/// Expand declarative shortcodes (`{{< name args >}}`) to inline HTML. Line-preserving
/// — each invocation opens and closes on one line — so the include source map stays
/// valid. Fenced code blocks are skipped, so a `{{< … >}}` shown as an *example* in a
/// code fence stays literal; unknown shortcodes are left untouched (with a warning).
pub(super) fn expand_shortcodes(
    src: &str,
    base_dir: Option<&std::path::Path>,
) -> (String, Vec<Warning>) {
    let mut warnings: Vec<Warning> = Vec::new();
    if !src.contains("{{<") {
        return (src.to_string(), warnings);
    }
    let ctx = ShortcodeCtx {
        base_dir,
        datasets: dataset::declared(src),
    };
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
            out.push_str(&expand_in_line(
                line,
                i + 1,
                &mut warnings,
                &mut input_ids,
                &ctx,
            ));
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
/// shortcodes whose *arguments* `render_shortcode` lints. `input` and `dataset` are
/// dispatched ahead of it in [`expand_in_line`], and `include` is resolved a whole pass
/// earlier (`crate::includes`). A feature report built on `SHORTCODE_SPECS` alone would
/// report three of the five as not existing.
pub(crate) const SHORTCODE_NAMES: &[&str] = &["embed", "video", "input", "dataset", "include"];

/// Every `{{< name args… >}}` written in `src`, as `(name, args)`, for the
/// feature-adoption report (`crate::features`).
///
/// Applies the expander's own two skip rules so a shortcode shown as an EXAMPLE is not
/// counted as a use: a fenced code block is literal, and so is an inline backtick span.
/// This is what stops `docs/guide/reference/shortcodes.tmd` (which shows every shortcode
/// in backticks and fences) from reading as the heaviest user of all of them.
pub(crate) fn scan_shortcodes(src: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut in_code = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code || !line.contains("{{<") {
            continue;
        }
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
                    break; // unterminated on this line: the expander leaves it as written
                };
                let end = i + 3 + rel_end;
                let toks = tokenize_args(line[i + 3..end].trim());
                if let Some((name, args)) = toks.split_first() {
                    out.push((name.clone(), args.to_vec()));
                }
                i = end + 3;
            } else {
                i += 1;
            }
        }
    }
    out
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
    ctx: &ShortcodeCtx<'_>,
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
            // `{{< dataset >}}` is expanded here for the same reason `input` is: it needs
            // the line number and the warning sink, plus the document's directory and its
            // `datasets:` declarations, none of which `render_shortcode` carries.
            if inner.split_whitespace().next() == Some("dataset") {
                let toks = tokenize_args(inner);
                match toks.get(1) {
                    Some(target) => {
                        let (html, ws) =
                            dataset::render(target, &ctx.datasets, ctx.base_dir, line_no);
                        warnings.extend(ws);
                        out.push_str(&html);
                    }
                    None => {
                        warnings.push(
                            Warning::new(format!(
                                "`{{{{< dataset >}}}}` at line {line_no} has no source path \
                                 (write `{{{{< dataset data/file.csv >}}}}` or a URL)"
                            ))
                            .at(None, line_no as u32),
                        );
                        out.push_str(&line[i..end + 3]);
                    }
                }
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
        return source_path("embed", args)
            .filter(|p| url_scheme(p).is_none())
            .map(|p| embed_html(&p, embed_title(args).as_deref()));
    }
    // `{{< video clip.mp4 [controls] [audio] [captions=clip.vtt] [dark=clip-dark.mp4]
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

/// How a `{{< video >}}` plays back: a three-step ladder, not independent switches, so the
/// incoherent combinations cannot be authored at all.
///
/// * [`Playback::Preview`] (the default) — the silent screencast: muted, looping, no control
///   bar. Playback is the `18-media.js` hover/focus preview plus the lightbox on click.
/// * [`Playback::Controls`] (`controls`) — a long silent clip a reader needs to *scrub*:
///   native controls (scrubber, keyboard, fullscreen, PiP), still muted + looping, and the
///   hover-preview/lightbox wiring stands down (the browser's own bar owns the clicks).
/// * [`Playback::Sound`] (`audio`) — a narrated explainer: native controls, and neither
///   `muted` nor `loop` (narration you cannot hear is pointless; narration that restarts
///   itself is hostile).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Playback {
    Preview,
    Controls,
    Sound,
}

/// Read the playback ladder off a `{{< video >}}`'s arguments.
///
/// `audio` implies `controls` (unmuted media with no pause/volume control is a WCAG 1.4.2
/// failure), and so does `captions=` (a `<track default>` with no control bar shows captions
/// the reader cannot turn off). That is why these are one ladder and not three booleans:
/// two of the eight flag combinations are incoherent, and neither can be spelled here.
fn playback_mode(args: &[String]) -> Playback {
    if shortcode_flag(args, "audio") {
        Playback::Sound
    } else if shortcode_flag(args, "controls") || shortcode_named(args, "captions").is_some() {
        Playback::Controls
    } else {
        Playback::Preview
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

/// The URL scheme a shortcode's path argument carries, if any.
///
/// Both built-ins document their positional argument as a path **relative to the
/// embedding page** (`{{< video tour.mp4 >}}` — "the video file, relative to the page";
/// `{{< embed talk.tmd >}}` — "built beside the embedding page"), and an embed target is
/// additionally *built* as a local file, so a URL there names nothing the builder can
/// reach. A scheme-bearing token is therefore not a path at all, and passing it through
/// put an author-controlled URL directly into an `<iframe src>` / `<video src>` with
/// nothing but attribute escaping in the way. It also slipped past `check`'s
/// missing-local-media diagnostic, which only looks at local files.
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
        "embed" => embed_path(args),
        "video" => video_path(args),
        _ => None,
    }
}

/// The `key=value` arguments whose value is a PATH (resolved relative to the page), as
/// opposed to prose like `caption=` / `title=`. Kept beside [`SHORTCODE_SPECS`] because it
/// is the same closed vocabulary viewed by type: these are the keys [`url_scheme`] guards,
/// and a path-valued key added there but not here is silently unguarded.
const PATH_KEYS: [(&str, &[&str]); 2] =
    [("embed", &[]), ("video", &["dark", "poster", "captions"])];

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
const SHORTCODE_SPECS: [(&str, &[&str], &[&str]); 2] = [
    ("embed", &["title"], &[]),
    (
        "video",
        &["dark", "poster", "caption", "captions"],
        &VIDEO_FLAGS,
    ),
];

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
    // The same optional argument under a `data-` name: `animate`/`point` carry their bounds
    // as `data-min`/`data-max`/`data-step` rather than as the HTML validation attributes,
    // because the element holding them is a hidden field (whose `min`/`max` the browser
    // would apply to a value the reader never types) or a `<span>` pad (where they are not
    // attributes at all).
    let data_attr = |data_key: &str, arg: &str| {
        shortcode_named(args, arg)
            .map(|v| format!(" {data_key}=\"{}\"", escape_attr(&v)))
            .unwrap_or_default()
    };
    // The label is a `<label for>` for a plain form field, but `animate` and `point` are
    // operated through a button group / a focusable pad while their VALUE lives in a hidden
    // field — and `<label for>` pointing at a hidden input names nothing. Those two take a
    // plain `<span>` with an id instead, which the operable element references with
    // `aria-labelledby`.
    let structural = kind == "animate" || kind == "point";
    let label_id = format!("{ctrl_id}-lbl");
    let control = match kind.as_str() {
        // A monotonic tick with transport controls. The value rides a hidden
        // `type="number"` so `readValue`'s existing `valueAsNumber` branch returns a NUMBER
        // (a `type="hidden"` field would hand every downstream cell the string "3"), and
        // the `hidden` ATTRIBUTE keeps it out of both the layout and the a11y tree.
        "animate" => {
            let btn = |act: &str, aria: &str, glyph: &str, pressed: &str| {
                format!(
                    "<button type=\"button\" class=\"tali-animate-btn\" data-tali-animate=\"{act}\" \
                     aria-label=\"{aria}\"{pressed}>{glyph}</button>"
                )
            };
            format!(
                "<span class=\"tali-animate-controls\" role=\"group\" aria-labelledby=\"{label_id}\">\
                 {play}{step}{reset}</span>\
                 <input id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\" \
                 data-tali-tick type=\"number\" hidden{min}{max}{step_a}{fps} value=\"{start}\">",
                play = btn("play", "Play", "▶", " aria-pressed=\"false\""),
                step = btn("step", "Step forward", "⏭", ""),
                reset = btn("reset", "Reset", "⏮", ""),
                min = data_attr("data-min", "min"),
                max = data_attr("data-max", "max"),
                step_a = data_attr("data-step", "step"),
                fps = data_attr("data-fps", "fps"),
                start = escape_attr(
                    value
                        .as_deref()
                        .or(shortcode_named(args, "min").as_deref())
                        .unwrap_or("0")
                ),
            )
        }
        // A draggable 2-D point. The published value is `{"x":…,"y":…}` JSON on a hidden
        // field tagged `data-tali-json`, which is the one widening `readValue` needed: the
        // string still round-trips through the URL fragment like every other control.
        "point" => format!(
            "<span class=\"tali-point-pad\" tabindex=\"0\" role=\"application\" \
             aria-labelledby=\"{label_id}\" aria-describedby=\"{ctrl_id}-out\"\
             {min}{max}{step}><span class=\"tali-point-dot\"></span></span>\
             <input id=\"{ctrl_id}\" class=\"tali-input-control\" data-tali-input=\"{name_a}\" \
             data-tali-json type=\"hidden\" value=\"{start}\">",
            min = data_attr("data-min", "min"),
            max = data_attr("data-max", "max"),
            step = data_attr("data-step", "step"),
            start = escape_attr(&point_value(value.as_deref())),
        ),
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
            // slider/range/number: numeric, sharing min/max/step/value
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
    // Every control whose value is not visible in the control itself gets a readout. For a
    // slider that is a convenience; for `animate` and `point` it is the ONLY place the
    // current value appears, and it is the live region a screen-reader user hears when the
    // pad moves — so those two get `aria-live` as well (a slider announces itself).
    let readout = match kind.as_str() {
        "slider" | "range" => Some(String::new()),
        "animate" => Some(html_escape(value.as_deref().unwrap_or("0"))),
        "point" => Some(String::new()),
        _ => None,
    }
    .map(|initial| {
        // The id + live region are for the two structural controls only: `point` points
        // its pad's `aria-describedby` here, and both need the readout to SPEAK because it
        // is the only place their value appears. A slider announces its own value, so it
        // keeps the markup it already had.
        let extra = if structural {
            format!(" id=\"{ctrl_id}-out\" aria-live=\"polite\"")
        } else {
            String::new()
        };
        // `for` ties the readout to the control it reflects, so AT announces them together.
        format!(
            "<output class=\"tali-input-out\"{extra} for=\"{ctrl_id}\" data-tali-out>{}</output>",
            if initial.is_empty() {
                html_escape(value.as_deref().unwrap_or(""))
            } else {
                initial
            }
        )
    })
    .unwrap_or_default();
    let label_html = if structural {
        format!(
            "<span class=\"tali-input-label\" id=\"{label_id}\">{}</span>",
            html_escape(&label)
        )
    } else {
        format!(
            "<label class=\"tali-input-label\" for=\"{ctrl_id}\">{}</label>",
            html_escape(&label)
        )
    };
    let kind_class = if structural {
        format!(" tali-input-{kind}")
    } else {
        String::new()
    };
    format!("<div class=\"tali-input{kind_class}\">{label_html}{control}{readout}</div>")
}

/// The `point` control's initial `{"x":…,"y":…}` JSON, from a `value="0.3,0.7"` argument.
/// Defaults to the centre of the default 0..1 domain, which is the only starting position
/// that is not a claim about the reader's data.
fn point_value(value: Option<&str>) -> String {
    let parsed = value.and_then(|v| {
        let (x, y) = v.split_once(',')?;
        Some((x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?))
    });
    let (x, y) = parsed.unwrap_or((0.5, 0.5));
    format!("{{\"x\":{x},\"y\":{y}}}")
}

/// The HTML for a `{{< video >}}`: a framed `<video>` with an optional caption. Playback is
/// **never** `autoplay` — an autoplaying loop beside body text is a WCAG 2.2.2 ("Pause,
/// Stop, Hide") failure — and the [`Playback`] ladder decides how the reader starts it: the
/// default silent screencast is muted + looping and driven by the `18-media.js`
/// hover/focus/lightbox enhancer, while `controls`/`audio` hand the clip to the browser's own
/// player (scrubber, keyboard, volume, fullscreen, PiP — no player library, and no restyling
/// of controls the browser already ships). `captions=` adds a caption `<track>`. With a
/// `dark` source, both clips are emitted and CSS shows the one matching `html[data-theme]`.
/// Raw-HTML, passed through.
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
        Playback::Preview => (" muted loop", ""),
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
    // makes the clip keyboard-reachable so focus can start it (parity with hover).
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

/// Map a deck source path to its built output URL (`x.tmd` → `x.html`), leaving a
/// path that is already `.html` (or anything else) untouched.
fn deck_href(path: &str) -> String {
    match crate::ext::strip_source_ext(path) {
        Some(stem) => format!("{stem}.html"),
        None => path.to_string(),
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
                && let Some(p) = source_path("embed", args)
                // A URL is not a deck the builder can build, and the renderer already
                // declines to embed it — collecting it here would hand `build` a target
                // it can only fail on.
                && url_scheme(&p).is_none()
                && !out.contains(&p)
            {
                out.push(p);
            }
        });
    }
    out
}

#[cfg(test)]
mod arg_validation_tests {
    use super::*;

    /// Every warning `expand_shortcodes` produces for one line of source.
    fn warn_msgs(src: &str) -> Vec<String> {
        expand_shortcodes(src, None)
            .1
            .into_iter()
            .map(|w| w.message)
            .collect()
    }

    #[test]
    fn a_url_source_is_refused_because_both_built_ins_take_a_page_relative_path() {
        // Item 97. Both built-ins document the positional argument as a file relative to
        // the page, and an embed target is additionally BUILT as a local file — so a
        // scheme-bearing token is not a path, yet it went straight into `<iframe src>` /
        // `<video src>` with only attribute escaping in the way, and slipped past `check`'s
        // missing-local-media diagnostic (which only looks at local files).
        for src in [
            "{{< embed javascript:alert(1) >}}\n",
            "{{< video javascript:alert(1) >}}\n",
            "{{< embed //evil.example/x.tmd >}}\n",
            "{{< video https://evil.example/x.mp4 >}}\n",
            "{{< video data:text/html,x >}}\n",
        ] {
            let (html, warnings) = expand_shortcodes(src, None);
            // The shortcode does not expand at all. What is left is the source text
            // verbatim — the existing "nothing is lost" path an unrecognised shortcode
            // already takes — so the URL survives as inert page text and never becomes an
            // `<iframe src>` / `<video src>`. Assert the ATTRIBUTE, not the substring:
            // the literal `{{< … >}}` still contains the URL, which is the point.
            assert!(
                !html.contains("<iframe") && !html.contains("<video"),
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
            None,
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
            "{{< embed talk.tmd >}}\n",
            "{{< video tour.mp4 dark=tour-dark.mp4 poster=tour.jpg caption=\"A: a tour\" >}}\n",
        ] {
            let (_, warnings) = expand_shortcodes(src, None);
            let msgs: Vec<String> = warnings.into_iter().map(|w| w.message).collect();
            assert!(msgs.is_empty(), "{src:?} must not warn: {msgs:?}");
        }
        // And a caption is prose: a colon in a sentence is not a scheme.
        let (html, _) =
            expand_shortcodes("{{< video tour.mp4 caption=\"Fig 1: the tour\" >}}\n", None);
        assert!(html.contains("Fig 1: the tour"), "caption survives: {html}");
    }

    #[test]
    fn a_url_embed_is_not_collected_as_a_build_target() {
        // `embed_targets` is what the site build walks to build each referenced deck.
        // A URL is not a deck it can build, so collecting one hands `build` a target it
        // can only fail on.
        assert_eq!(
            embed_targets("{{< embed talk.tmd >}}\n{{< embed https://e.x/d.tmd >}}\n"),
            vec!["talk.tmd".to_string()],
            "only the real local deck is a build target"
        );
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
        let (html, warnings) = expand_shortcodes("{{< video control tour.mp4 >}}\n", None);
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
            "{{< embed deck.tmd >}}\n",
            "{{< embed deck.tmd title=\"The deck\" >}}\n",
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
        let (_, warnings) = expand_shortcodes("intro\n\n{{< video tour.mp4 control >}}\n", None);
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
            html.contains("for=\"qin-freq\" data-tali-out"),
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
