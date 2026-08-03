//! Every emitted block must have **exactly one root element**.
//!
//! The preview client mounts an incoming block with
//! `template.content.firstElementChild` (`web-client/client.js`), so a block whose
//! html has two or more roots is only half-mounted: `update` swaps in the first
//! root and drops the rest, `insert` inserts only the first, and `remove` strands
//! the extra roots in the page forever. The block id still changes, so the op
//! *looks* applied while the DOM keeps the old content — preview then disagrees
//! with what `build` publishes, which is the one thing the block model exists to
//! prevent.
//!
//! This is the corpus-wide version, plus the round-trip that proves an edit to a
//! multi-root construct arrives at the client as one swappable element.

use std::fs;
use std::path::{Path, PathBuf};

/// Elements with no closing tag (HTML void elements). A void element at depth 0 is
/// a root all by itself.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose content is raw text, not markup: a `<` inside them (`a < b` in a
/// `{js}` cell) is text, not a tag, so the scanner must skip to the close tag.
const RAW_TEXT: &[&str] = &["script", "style", "textarea", "title"];

/// Walk `html` and count how many *root* nodes it has: top-level elements plus any
/// run of top-level non-whitespace text (the client drops those too — it takes the
/// first element *child*, so a stray text node is silently lost the same way).
///
/// Deliberately a scanner and not a parser: the input is our own emitted HTML, and
/// a dependency-free counter is one that can be read at review time. It is
/// self-tested in `root_count_counts_what_the_client_would_mount`.
fn root_count(html: &str) -> usize {
    let b = html.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;
    let mut roots = 0usize;
    let mut in_top_text = false;
    while i < b.len() {
        if b[i] != b'<' {
            if depth == 0 && !b[i].is_ascii_whitespace() && !in_top_text {
                in_top_text = true;
                roots += 1;
            }
            i += 1;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i + 4..]
                .find("-->")
                .map(|r| i + 4 + r + 3)
                .unwrap_or(b.len());
            continue;
        }
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            i = html[i..].find('>').map(|r| i + r + 1).unwrap_or(b.len());
            continue;
        }
        let closing = html[i..].starts_with("</");
        let name_start = if closing { i + 2 } else { i + 1 };
        if !html[name_start..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            // A bare `<` in text (`a < b`), not the start of a tag.
            if depth == 0 && !in_top_text {
                in_top_text = true;
                roots += 1;
            }
            i += 1;
            continue;
        }
        let name: String = html[name_start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_ascii_lowercase();
        // Find this tag's `>`, skipping any inside a quoted attribute value
        // (`alt="a > b"`, an inline handler, an SVG `d=` path).
        let mut j = name_start + name.len();
        let mut quote: Option<u8> = None;
        while j < b.len() {
            match (quote, b[j]) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, c @ (b'"' | b'\'')) => quote = Some(c),
                (None, b'>') => break,
                (None, _) => {}
            }
            j += 1;
        }
        let self_closing = j > 0 && b[j - 1] == b'/';
        let end = (j + 1).min(b.len());
        in_top_text = false;
        if closing {
            depth = depth.saturating_sub(1);
            i = end;
            continue;
        }
        if depth == 0 {
            roots += 1;
        }
        if VOID.contains(&name.as_str()) || self_closing {
            i = end;
            continue;
        }
        depth += 1;
        if RAW_TEXT.contains(&name.as_str()) {
            // Skip the body and resume ON the close tag, which the loop then pops:
            // an early `continue` here (without the `depth += 1` above) is what made
            // this scanner report a `<figcaption>` after a `{js}` cell's `<script>`
            // as a second root, i.e. a whole class of false positives.
            let close = format!("</{name}");
            i = match html[end..].find(&close) {
                Some(rel) => end + rel,
                None => b.len(),
            };
            continue;
        }
        i = end;
    }
    roots
}

#[test]
fn root_count_counts_what_the_client_would_mount() {
    // A probe whose every row is negative is a broken probe: these are the
    // known-positive rows. Each pair is (html, roots the client would see).
    let cases: &[(&str, usize)] = &[
        ("<p>one</p>", 1),
        ("<p>one</p>\n<p>two</p>", 2),
        ("<div><p>nested</p><p>still one root</p></div>", 1),
        ("<img src=\"a.png\">", 1),
        ("<img src=\"a.png\"><img src=\"b.png\">", 2),
        ("<div class=\"a\"><img alt=\"a > b\" src=\"x\"></div>", 1),
        // raw text: the `<` in the script body is not a tag
        ("<div><script>if (a < b) { x(); }</script></div>", 1),
        ("<script>a < b</script><p>after</p>", 2),
        // the shape of a `{js}` figure cell: a sibling AFTER a raw-text element,
        // inside a wrapper. Counting this as 2 is the scanner bug that made 15
        // correct corpus figures look like defects.
        (
            "<figure><script>a < b</script><figcaption>c</figcaption></figure>",
            1,
        ),
        (
            "<div><style>p > a { color: red }</style><p>after</p></div>",
            1,
        ),
        // a top-level text node is dropped by firstElementChild just as surely
        ("text before<div>x</div>", 2),
        ("<!-- comment --><div>x</div>", 1),
        ("<svg viewBox=\"0 0 1 1\"><path d=\"M0 0 L1 1\"/></svg>", 1),
        ("  <div>x</div>\n  ", 1),
    ];
    for (html, want) in cases {
        assert_eq!(root_count(html), *want, "root_count({html:?})");
    }
}

fn collect_tmd(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let p = entry.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "_extensions" || name == "expected" || name == "_site" {
                continue;
            }
            collect_tmd(&p, out);
        } else if taliesin_core::ext::is_source_path(&p) {
            out.push(p);
        }
    }
}

#[test]
fn every_block_in_every_real_document_has_exactly_one_root() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    collect_tmd(&repo.join("corpus"), &mut files);
    collect_tmd(&repo.join("docs"), &mut files);
    files.sort();
    assert!(
        files.len() >= 100,
        "expected the corpus + docs documents, found {}",
        files.len()
    );

    let mut offenders = Vec::new();
    let mut unaddressable = Vec::new();
    for f in &files {
        let label = f.strip_prefix(&repo).unwrap_or(f).display().to_string();
        let src = fs::read_to_string(f).unwrap();
        let doc = taliesin_core::render_document_with_includes(&src, f.parent().unwrap());
        for b in &doc.blocks {
            let n = root_count(&b.html);
            let where_ = format!(
                "{label} block {} at {}: {}",
                b.id,
                b.sourcepos,
                b.html.chars().take(160).collect::<String>()
            );
            if !b.html.contains(&format!("data-block-id=\"{}\"", b.id)) {
                // Nothing in the DOM claims this block's id, so no op ever targets it.
                // `emit_html_block` documents the only shapes this is allowed to be:
                // an HTML comment or a stray closing tag (an author closing a `<div>`
                // they opened in an earlier block). Real content must never land here —
                // it would be invisible to click-to-source and to every incremental op.
                let lead = b.html.trim_start();
                if !(lead.starts_with("<!--") || lead.starts_with("</")) {
                    unaddressable.push(where_);
                }
                continue;
            }
            if n != 1 {
                offenders.push(format!("{where_} — has {n} roots"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "{} id-carrying blocks are not single-root, so the preview client would mount \
         only part of each:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
    assert!(
        unaddressable.is_empty(),
        "{} blocks carry no `data-block-id`, and are not the comment/closing-tag \
         fragments that are allowed to:\n{}",
        unaddressable.len(),
        unaddressable.join("\n")
    );
}

#[test]
fn a_multi_root_construct_round_trips_through_a_live_swap() {
    // `render_document` leaves shortcodes literal — expansion is part of the
    // includes pass, so this must go through `render_document_with_includes`.
    use taliesin_core::{BlockOp, diff_blocks, render_document_with_includes};
    let render = |src: &str| render_document_with_includes(src, Path::new("."));

    // Three consecutive `{{< input >}}` controls are one HTML block in the source
    // (this is the shape shipped in `corpus/descent/index.tmd`). Editing one of
    // them must produce an Update whose html the client can mount whole.
    let doc = |max: &str| {
        format!(
            "# Playground\n\n\
             {{{{< input name=\"lr\" type=\"slider\" min=\"0.01\" max=\"{max}\" step=\"0.01\" value=\"0.12\" label=\"step size\" >}}}}\n\
             {{{{< input name=\"beta\" type=\"slider\" min=\"0\" max=\"0.9\" step=\"0.05\" value=\"0\" label=\"momentum\" >}}}}\n\
             {{{{< input name=\"steps\" type=\"slider\" min=\"1\" max=\"60\" step=\"1\" value=\"25\" label=\"steps\" >}}}}\n\
             \nAfter.\n"
        )
    };
    let v1 = render(&doc("0.35"));
    let v2 = render(&doc("0.75"));

    let controls = v1
        .blocks
        .iter()
        .find(|b| b.html.contains("tali-input"))
        .expect("the three controls render");
    assert_eq!(
        root_count(&controls.html),
        1,
        "the three controls must arrive as one mountable element: {}",
        controls.html
    );
    assert_eq!(
        controls.html.matches("data-tali-input").count(),
        3,
        "all three controls must survive inside that one root: {}",
        controls.html
    );

    let ops = diff_blocks(&v1.blocks, &v2.blocks);
    let updated = ops
        .iter()
        .find_map(|op| match op {
            BlockOp::Update { target_id, html } if *target_id == controls.id => Some(html),
            _ => None,
        })
        .unwrap_or_else(|| panic!("editing a slider must update its block in place: {ops:?}"));
    // The op the client applies: one root (so `fragment()` mounts all of it) and
    // the edited value actually inside it.
    assert_eq!(root_count(updated), 1, "the swapped-in html: {updated}");
    assert!(
        updated.contains("max=\"0.75\""),
        "the edit must be in the swapped html: {updated}"
    );
    assert_eq!(
        updated.matches("data-tali-input").count(),
        3,
        "a swap must carry every control, not just the first: {updated}"
    );
}
