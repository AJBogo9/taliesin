//! Fenced-code language validation.
//!
//! A fence whose language token resolves to no syntax degrades to escaped plain
//! text. That is silent: the block still renders, just unstyled, so a typo
//! (` ```pyton `) reads as "highlighting is broken" rather than "you misspelled
//! python". This surfaces it on the same located channel as the rest of the family.

use super::helpers::start_line;
use crate::highlight::known_language;
use crate::render::{Block, Warning};

/// The language token of every highlighted fence in `html`.
///
/// `emit.rs` writes the *raw* fence label into `class="language-{l}"`, on both
/// static fences and executable cells. `{mermaid}` cells never reach here: they
/// emit a bare `<pre class="mermaid">` with no `<code>` element. The literal
/// `class="language-` cannot appear in code *content*, because a block's text is
/// HTML-escaped (`class=&quot;language-`) before it is embedded.
fn fence_languages(html: &str) -> Vec<&str> {
    const ATTR: &str = "class=\"language-";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find(ATTR) {
        let start = i + pos + ATTR.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.push(&html[start..start + len]);
        i = start + len;
    }
    out
}

/// Warn on any fenced code block whose language will not be highlighted.
///
/// Tokens that render plain on purpose (`text`, `console`, `output`, …) are
/// accepted by [`known_language`] and never warn.
pub fn validate_code_languages(blocks: &[Block]) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        for lang in fence_languages(&b.html) {
            if known_language(lang) {
                continue;
            }
            let w = Warning::new(format!(
                "unknown code language `{lang}`: this block renders as plain text \
                 (check the spelling, or use `text` if that is intended)"
            ));
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
