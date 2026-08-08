//! Local `<video>`/`<source>`/`poster=` media existence validation.

use super::helpers::{is_local_ref, start_line, tag_attr};
use crate::render::{Block, Severity, Warning};
use std::path::Path;

/// Unique local **video** refs (`src=`/`poster=`) on every `<video …>` tag plus any
/// `<source …>` *nested inside a `<video>` element*. Mirrors `local_img_refs` for the
/// one media case a static check *can* resolve: `{{< video clip.mp4 >}}` (and hand-written
/// `<video src>`/`<video><source></video>`) emit a literal local path, so a missing file is
/// catchable. `<audio>` (and its `<source>` children, often streamed/generated) and `<img>`
/// are handled elsewhere — so we track `<video>`…`</video>` nesting and skip a `<source>`
/// that belongs to an `<audio>` element.
fn local_media_refs(html: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    let mut in_video = false;
    while let Some(pos) = html[i..].find('<') {
        let tag_start = i + pos;
        let after = &html[tag_start..];
        let Some(rel_end) = after.find('>') else {
            break;
        };
        i = tag_start + rel_end + 1;
        let tag = &after[..rel_end];
        // Track <video>…</video> nesting so a <source> is only checked when it belongs to
        // a <video> (not an <audio>, whose sources are often streamed/generated).
        if after.starts_with("</video") {
            in_video = false;
            continue;
        }
        if after.starts_with("<audio") {
            in_video = false; // an <audio> opens a non-video media context
            continue;
        }
        let scan = if after.starts_with("<video") {
            in_video = true;
            true
        } else {
            after.starts_with("<source") && in_video
        };
        if !scan {
            continue;
        }
        for attr in ["src=\"", "poster=\""] {
            if let Some(val) = tag_attr(tag, attr)
                && is_local_ref(val)
                && !out.contains(&val)
            {
                out.push(val);
            }
        }
    }
    out
}

/// Local `<video src>`/`<source src>`/`poster=` references (from `{{< video clip.mp4 >}}`
/// or raw `<video>`) whose target file does not exist under the doc base dir — a broken
/// clip that ships silently. The video sibling of [`super::assets::validate_local_assets`]:
/// absolute (`/...`) and external refs are out of scope. (`{{< video >}}` renders to raw
/// `<video src>`, so scanning the emitted HTML catches the shortcode and hand-written
/// `<video>` alike.)
pub fn validate_local_media(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_media_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || base.join(path).is_file() {
                continue;
            }
            let w = Warning::new(format!(
                "local video not found: `{path}` (no such file under the document directory)"
            ))
            .severity(Severity::Error);
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
