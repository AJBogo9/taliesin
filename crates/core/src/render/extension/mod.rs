//! Declarative shortcodes: expand `{{< name args >}}` invocations to inline HTML. The one
//! built-in that expands here is `{{< input … >}}` (a reactive control); `{{< include >}}`
//! is resolved a whole pass earlier (`crate::includes`).
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
    // The fence state is `(fence_char, run_len)`, not a boolean: a `` ``` `` line inside a
    // longer ```` ```` ```` sample is not a closing fence, and a boolean toggle read it as
    // one. That desynced both ways — a shortcode shown inside a nested code sample got
    // expanded into live markup, and an odd number of inner fence lines left the flag stuck
    // "inside code" for the rest of the document, so a real control below silently vanished.
    // `divs::next_code_state` is the helper the other two line-scanning passes over this
    // same buffer already share.
    let mut in_code: Option<(char, usize)> = None;
    // Deduplicates `{{< input >}}` control ids across the document, so two controls that
    // bind the same reactive name get distinct DOM ids (`tali-in-rate`, `tali-in-rate-1`). Threaded
    // here (not per line) because the id must be name-based, not line-based — see
    // `input_shortcode`.
    let mut input_ids: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let next = super::divs::next_code_state(in_code, line);
        // Literal on the fence marker itself (either end) and on every line between.
        if in_code.is_some() || next.is_some() {
            out.push_str(line); // it's an example, not an invocation
        } else {
            out.push_str(&expand_in_line(line, i + 1, &mut warnings, &mut input_ids));
        }
        in_code = next;
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    (out, warnings)
}

/// Every built-in shortcode name the tool implements, and the whole CLOSED vocabulary —
/// which is what lets a leftover `{{< video >}}` draw a located, named warning instead of
/// shipping as literal text in silence. Neither is expanded by `expand_in_line`'s general
/// path: `input` is dispatched ahead of it, and `include` is resolved a whole pass earlier
/// (`crate::includes`).
pub const SHORTCODE_NAMES: &[&str] = &["input", "include"];

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
            // Nothing else expands here. Keep the invocation verbatim (nothing is lost),
            // but warn: a typo'd — or RETIRED — shortcode name should be visible in the
            // build log / preview diagnostics, not shipped as literal text into the page.
            // `include` is handled in an earlier pass (`includes::resolve`); a leftover one
            // means that pass already reported it, so don't double-warn.
            let name = inner.split_whitespace().next().unwrap_or(inner);
            if name != "include" {
                // A name this tool USED to expand gets its removal note instead of the bare
                // "unknown" — the same distinction `RETIRED_KEYS` draws everywhere else, and
                // read from that same scoped register (scope `shortcode`) so there is no
                // second list to keep. The `{{<` opener stays in the message either way:
                // `codes::classify` keys `TAL-SHORTCODE` on it.
                let tail = match crate::frontmatter::retired_note("shortcode", name) {
                    Some(note) => format!(": {note}"),
                    None => " (left as literal text)".to_string(),
                };
                warnings.push(
                    Warning::new(format!(
                        "unknown shortcode `{{{{< {name} >}}}}` at line {line_no}{tail}"
                    ))
                    .at(None, line_no as u32),
                );
            }
            out.push_str(&line[i..end + 3]); // unknown: keep verbatim
            i = end + 3;
        } else {
            let ch = line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
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

#[cfg(test)]
mod unknown_shortcode_tests {
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
    fn an_unknown_shortcode_stays_literal_and_warns_on_its_own_line() {
        // Nothing is lost — the invocation is copied through verbatim — but it must be
        // VISIBLE, located, in the build log and the preview diagnostics. This is the whole
        // reason the shortcode vocabulary is closed.
        let src = "intro\n\n{{< sidebar x >}}\n";
        let (html, warnings) = expand_shortcodes(src);
        assert!(html.contains("{{< sidebar x >}}"), "kept verbatim: {html}");
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].message,
            "unknown shortcode `{{< sidebar >}}` at line 3 (left as literal text)"
        );
        assert_eq!(warnings[0].line, Some(3), "warning: {:?}", warnings[0]);
    }

    #[test]
    fn a_retired_shortcode_answers_with_its_removal_note_not_a_bare_unknown() {
        // The register's whole job: a removal and a typo are different mistakes, and the
        // author has to be told which one they made. Read out of the scoped `RETIRED_KEYS`
        // under the `shortcode` scope, so there is no second list.
        let w = warn_msgs("{{< video tour.mp4 caption=\"A tour\" >}}\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].starts_with("unknown shortcode `{{< video >}}` at line 1: it was removed on"),
            "must carry the register's own note: {w:?}"
        );
        assert!(
            !w[0].contains("did you mean"),
            "a retirement is not a did-you-mean: {w:?}"
        );
    }

    #[test]
    fn a_shortcode_inside_a_code_fence_or_inline_code_is_an_example_not_an_invocation() {
        // The documentation case: the guide shows shortcode spellings in fenced blocks and
        // inline code. Those are examples — expansion already skips them, and the warning
        // must not reach around that.
        assert!(warn_msgs("```\n{{< sidebar x >}}\n```\n").is_empty());
        assert!(warn_msgs("see `{{< sidebar x >}}` here\n").is_empty());
    }

    /// A nested fence must not end the outer sample. The pass tracked fences with a bare
    /// boolean toggle until 2026-08-13, so an inner ``` inside a longer ```` sample closed
    /// the region early — and the desync ran both ways.
    #[test]
    fn a_nested_code_fence_does_not_desync_the_shortcode_pass() {
        // Forwards: the shortcode inside the nested sample is an example, so it stays
        // literal. The toggle expanded it into live markup (and diagnosed it).
        let src = "````markdown\n```\n{{< input type=number name=inner >}}\n```\n````\n";
        let (html, warnings) = expand_shortcodes(src);
        assert!(
            html.contains("{{< input type=number name=inner >}}"),
            "{html}"
        );
        assert!(!html.contains("data-tali-input"), "not expanded: {html}");
        assert!(warnings.is_empty(), "{warnings:?}");

        // Backwards, and this is the silent one: an ODD number of fence lines in the
        // mis-classified region left the flag stuck "inside code" for the whole rest of the
        // document, so a genuine control below was never expanded and never diagnosed.
        let src = "````markdown\n```\nunclosed inner sample\n````\n\n{{< input type=number name=real >}}\n";
        let (html, warnings) = expand_shortcodes(src);
        assert!(
            html.contains("data-tali-input=\"real\""),
            "the control after the sample must still expand: {html}"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_leftover_include_does_not_double_warn() {
        // `includes::resolve` runs a whole pass earlier and already reported anything it
        // could not expand, so this pass must stay silent about it.
        assert!(warn_msgs("{{< include nope.tmd >}}\n").is_empty());
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
