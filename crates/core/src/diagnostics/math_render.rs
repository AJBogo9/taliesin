//! KaTeX render-failure detection.
//!
//! Server-side math runs with `throw_on_error = false` (a bad expression renders as a
//! red `katex-error` span inline rather than aborting the document), so a malformed
//! `$…$` was the one render path that produced NO diagnostic — it shipped silently.
//! This walks the rendered block model, finds those error spans, and re-surfaces the
//! KaTeX message on the located click-to-source channel so it reaches `check`/`--strict`
//! and the preview overlay.

use super::helpers::start_line;
use crate::render::{Block, Warning};

const ERROR_MARKER: &str = "class=\"katex-error\"";
const TITLE_ATTR: &str = "title=\"";

/// One located [`Warning`] per `$…$`/`$$…$$` that KaTeX could not parse. The message
/// echoes KaTeX's own `title=` (which names the cause and quotes the offending source);
/// the location is the containing block's start line.
pub fn validate_math(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for msg in katex_errors(&b.html) {
            let w = Warning::new(format!("math failed to render: {msg}"));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// The decoded `title=` message of every `class="katex-error"` span in `html`. Valid
/// KaTeX output never contains the substring `katex-error`, so this can't false-match.
fn katex_errors(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find(ERROR_MARKER) {
        let at = i + pos;
        i = at + ERROR_MARKER.len();
        // KaTeX emits `class="katex-error" title="…"` in the same opening tag, so the
        // next `title="…"` belongs to this span.
        let msg = html[at..]
            .find(TITLE_ATTR)
            .map(|p| at + p + TITLE_ATTR.len())
            .and_then(|s| {
                html[s..]
                    .find('"')
                    .map(|len| decode_entities(&html[s..s + len]))
            })
            .unwrap_or_else(|| "KaTeX parse error".to_string());
        out.push(msg);
    }
    out
}

/// Decode the handful of HTML entities KaTeX puts in a `title=` attribute back to text
/// (`&amp;` last, so an already-decoded `<`/`>`/`'` isn't re-processed).
fn decode_entities(s: &str) -> String {
    s.replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
