//! C-READ-2: the reader's code download — "I want to run this myself".
//!
//! **What is left of the item.** C-READ-2 was filed as "code *and data* download". The data
//! half shipped separately as `{{< dataset >}}` (item 176), which states what a file is, how
//! big it is, what it hashes to and where it came from — the web-native answer, since a
//! multi-GB parquet does not belong in a folder of HTML. What had no surface at all was the
//! *code*: a reader looking at a page of figures had no way to get the source that produced
//! them except to select each listing by hand, in order, and paste them together.
//!
//! **Why this is server-side and not a client-side scrape of the DOM.** The obvious cheap
//! version walks `[data-tali-cell]` in the browser and joins what it finds. It is wrong in a
//! way that does not announce itself: a cell with `#| echo: false` **runs but emits no
//! listing**, so the scraped script silently omits exactly the setup cells authors most
//! often hide — imports, data loading, plot styling. A download that is quietly missing the
//! first third of the program is worse than no download, because the reader only finds out
//! after it fails. The block model still carries those cells (`Block::cell` is `Some` even
//! when `echo` is false), so assembling here sees the whole program.
//!
//! Emitted as one more generated `Block` (empty sourcepos, like References and the
//! footnotes section: the content is gathered from lines scattered through the document, so
//! no single range is honest).
//!
//! The download is a `data:` URL rather than a file the build writes, so a single built
//! `.html` carries its own code with no second artefact to keep beside it and nothing to
//! serve at read time — the same "the reader owns it outright" reasoning as the book `.zip`.

use super::{Block, html_escape};

/// The generated block's id.
///
/// Named once because FOUR consumers must agree to skip it, and they are in three modules:
/// the text projection (`taliesin read`), the search index, and `llms-full.txt`. Every one
/// of those turns a page into text, and this box is chrome offering the reader a file, not
/// something the page says — indexed, every computational page in a project answers a
/// Cmd-K search for "download" with identical boilerplate. A literal in each place is a
/// rename that silently un-skips one of them, which is why this is a constant and why
/// `the_code_download_box_stays_out_of_every_text_projection` checks all three at once.
pub(crate) const REPRO_BLOCK_ID: &str = "tali-repro";

/// One language's collected source.
struct LangCode {
    /// The language as authored (`python`, `r`, `js`, …).
    lang: String,
    /// Every cell of that language, in document order, separated by a blank line.
    code: String,
}

/// The file extension a language's script is offered under. Known languages get their real
/// extension; anything else falls back to `.txt` rather than inventing one — a registered
/// client language (`{glsl}`) or a future kernel should not silently produce a filename
/// that claims to be something the reader's tools will act on.
fn extension(lang: &str) -> &'static str {
    match lang {
        "python" => "py",
        "r" => "R",
        "js" | "javascript" => "js",
        "julia" => "jl",
        "glsl" => "glsl",
        _ => "txt",
    }
}

/// How the language is named to a reader.
fn display(lang: &str) -> String {
    match lang {
        "python" => "Python".to_string(),
        "r" => "R".to_string(),
        "js" | "javascript" => "JavaScript".to_string(),
        "julia" => "Julia".to_string(),
        "glsl" => "GLSL".to_string(),
        other => other.to_string(),
    }
}

/// Collect each language's cells: languages in first-appearance order, and within a
/// language the cells in document order.
///
/// Order is the whole value. A script whose statements are shuffled is not the program the
/// document ran, and the document's order IS the program's order — that is exactly what the
/// execution model guarantees (each cell sees the kernel state the ones above it left).
fn collect(blocks: &[Block]) -> Vec<LangCode> {
    let mut out: Vec<LangCode> = Vec::new();
    for cell in blocks.iter().filter_map(|b| b.cell.as_ref()) {
        // A browser-run language (`{js}`, `{glsl}`) is deliberately excluded, and the reason
        // is that this box's one claim would be FALSE for it. A kernel language is a script:
        // each cell sees the state the ones above it left, so document order is program
        // order. `{js}` is a reactive GRAPH — the runtime orders cells by dependency, and a
        // cell referencing a `viewof` input or the page's DOM has no meaning outside the
        // runtime that hosts it. Concatenating those in document order would produce a file
        // that is not the program that ran, offered under a sentence saying it is.
        //
        // Nothing is lost: re-running a `{js}` document is reloading the page, which already
        // works, and the source stays visible in the listing and in view-source.
        if super::client_lang::client_lang(&cell.lang).is_some() {
            continue;
        }
        let code = cell.code.trim_end();
        if code.trim().is_empty() {
            continue;
        }
        match out.iter_mut().find(|l| l.lang == cell.lang) {
            Some(existing) => {
                existing.code.push_str("\n\n");
                existing.code.push_str(code);
            }
            None => out.push(LangCode {
                lang: cell.lang.clone(),
                code: code.to_string(),
            }),
        }
    }
    out
}

/// The generated "Run this yourself" block for a document with code cells, or `None` when
/// it has none.
pub(super) fn repro_block(blocks: &[Block]) -> Option<Block> {
    let langs = collect(blocks);
    if langs.is_empty() {
        return None;
    }
    let mut links = String::new();
    for l in &langs {
        // Cell source arrives with its `#|` option lines stripped, which leaves the first
        // cell of a language starting on a blank line. Trim once, here, rather than per
        // cell: the blank line BETWEEN two cells is deliberate separation.
        let l = &LangCode {
            lang: l.lang.clone(),
            code: l.code.trim().to_string(),
        };
        // base64 rather than percent-encoding: the payload is arbitrary source text, and
        // `#`, `%`, `&` and a non-ASCII identifier each break a naively-built data: URL in a
        // different way. A `data:` URL on `<a download>` is a download, not a navigation, so
        // the top-level data:-navigation block does not apply.
        let b64 = super::base64_encode(l.code.as_bytes());
        let ext = extension(&l.lang);
        let name = display(&l.lang);
        let lines = l.code.lines().count();
        links.push_str(&format!(
            "<li><a class=\"tali-repro-dl\" download=\"cells.{ext}\" \
             href=\"data:text/plain;charset=utf-8;base64,{b64}\">\
             Download the {name} ({lines} line{s})</a></li>",
            s = if lines == 1 { "" } else { "s" },
        ));
    }
    // `aria-label` rather than a visible <h2>: the box is an aside about the document, and a
    // heading here would enter the page outline and the TOC between the last real section
    // and the footnotes, where it reads as content the author wrote.
    let html = format!(
        "<aside class=\"tali-repro\" data-block-id=\"{REPRO_BLOCK_ID}\" aria-label=\"Run this yourself\">\
         <p class=\"tali-repro-lead\">{lead}</p><ul class=\"tali-repro-list\">{links}</ul></aside>",
        lead = html_escape(
            "Every code cell on this page, in the order it ran — including any the author \
             chose not to display.",
        ),
    );
    Some(Block {
        id: REPRO_BLOCK_ID.to_string(),
        sourcepos: String::new(),
        source_file: None,
        html,
        cell: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render_document;

    fn block_html(src: &str) -> Option<String> {
        repro_block(&render_document(src).blocks).map(|b| b.html)
    }

    fn payload(html: &str, ext: &str) -> String {
        let needle =
            format!("download=\"cells.{ext}\" href=\"data:text/plain;charset=utf-8;base64,");
        let at = html
            .find(&needle)
            .unwrap_or_else(|| panic!("no cells.{ext} link in: {html}"))
            + needle.len();
        let end = html[at..].find('"').expect("a closed href");
        let b64 = &html[at..at + end];
        String::from_utf8(base64_decode(b64)).expect("valid utf-8")
    }

    /// Minimal decoder, test-only: the encoder is `render::base64_encode`, and a test that
    /// re-implemented the *encoder* would agree with a broken one.
    fn base64_decode(s: &str) -> Vec<u8> {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut acc: u32 = 0;
        let mut bits = 0;
        let mut out = Vec::new();
        for c in s.bytes().filter(|c| *c != b'=') {
            let v = T.iter().position(|t| *t == c).expect("base64 alphabet") as u32;
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }

    #[test]
    fn a_document_with_no_cells_offers_nothing() {
        assert!(block_html("Just prose.\n\n```python\nnot_a_cell = 1\n```\n").is_none());
    }

    /// The reason this is server-side. A hidden cell runs and produces no listing, so a
    /// browser-side scrape of the rendered page cannot see it — and setup cells (imports,
    /// data loading) are exactly the ones authors hide.
    #[test]
    fn a_hidden_cell_is_in_the_download_even_though_it_renders_no_listing() {
        let src = "```{python}\n#| echo: false\nimport numpy as np\n```\n\n\
                   ```{python}\nprint(np.pi)\n```\n";
        let doc = render_document(src);
        assert!(
            !doc.blocks.iter().any(|b| b.html.contains("import numpy")),
            "the hidden cell must render no listing, or this test proves nothing"
        );
        let code = payload(&block_html(src).expect("a box"), "py");
        assert!(code.contains("import numpy as np"), "got: {code}");
        assert!(code.contains("print(np.pi)"), "got: {code}");
    }

    /// Document order IS program order: the kernel state each cell sees is what the cells
    /// above it left. A script assembled out of order is not the program that ran.
    #[test]
    fn cells_are_concatenated_in_document_order() {
        let code = payload(
            &block_html("```{python}\nfirst = 1\n```\n\n```{python}\nsecond = 2\n```\n")
                .expect("a box"),
            "py",
        );
        assert!(
            code.find("first = 1") < code.find("second = 2"),
            "got: {code}"
        );
    }

    /// Two kernels in one document are two programs; concatenating them would produce one
    /// file that is valid in neither.
    #[test]
    fn each_language_gets_its_own_download() {
        let html = block_html("```{python}\nx = 1\n```\n\n```{r}\ny <- 2\n```\n").expect("a box");
        assert_eq!(payload(&html, "py").trim(), "x = 1");
        assert_eq!(payload(&html, "R").trim(), "y <- 2");
    }

    /// A `{js}` document offers no download, because the box's claim — "in the order it
    /// ran" — is false for a reactive graph, whose order is its dependencies. This is the
    /// case that keeps the affordance honest rather than merely present.
    #[test]
    fn a_browser_run_language_is_not_offered_as_a_script() {
        assert!(block_html("```{js}\nconst x = 1;\n```\n").is_none());
        // ...but a kernel language in the SAME document still is.
        let html = block_html("```{js}\nconst x = 1;\n```\n\n```{python}\ny = 2\n```\n")
            .expect("the python half is still a script");
        assert_eq!(payload(&html, "py").trim(), "y = 2");
        assert!(!html.contains("cells.js"), "no js download: {html}");
    }

    /// The box is chrome, and every projection that turns a page into text must drop it.
    ///
    /// Checked in ONE test rather than three, because the failure mode is a *partial*
    /// rename: three modules skip this block by id, and a literal in each is a rename that
    /// silently un-skips one of them. Two of the three were found only by building a real
    /// site and grepping the artefacts, not by any test that existed.
    #[test]
    fn the_code_download_box_stays_out_of_every_text_projection() {
        let doc = render_document("# Title\n\nProse.\n\n```{python}\nx = 1\n```\n");
        assert!(
            doc.blocks.iter().any(|b| b.id == REPRO_BLOCK_ID),
            "the fixture must actually carry the box, or this test proves nothing"
        );
        // `taliesin read` / `skim` (and the search index + llms-full.txt, whose own
        // extractors filter on the same constant).
        let text = crate::render::text::project(&doc.blocks);
        assert!(
            !text.contains("Download the"),
            "the download box leaked into the text projection:\n{text}"
        );
        // The cells themselves are still there — the box is dropped, not the content.
        assert!(
            text.contains("x = 1"),
            "the cell source still projects:\n{text}"
        );
    }

    /// An unknown language must not be handed a filename that claims a toolchain.
    #[test]
    fn an_unrecognized_language_falls_back_to_a_plain_extension() {
        assert_eq!(extension("python"), "py");
        assert_eq!(extension("r"), "R");
        assert_eq!(extension("ocaml"), "txt");
    }
}
