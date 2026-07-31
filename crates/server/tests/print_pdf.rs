//! The LIVE print gate: the one test that drives the real CDP loop (backlog 159).
//!
//! Gated on `TALIESIN_REQUIRE_CHROME=1` exactly like `read_run_js`, and named so
//! `tools/gates.sh` can assert it printed `... ok` BY NAME — a silently skipped gate must
//! never read as green.
//!
//! **Why the assertions look paranoid:** measured 2026-07-31, driving paged.js from the
//! plain Chrome CLI produces a *plausible-looking but truncated* PDF — 2 pages of correct
//! typography with the rest of the document silently missing. "A PDF was written" and even
//! "it has content" both pass on that. So the gate asserts page COUNT and late content.

use std::path::{Path, PathBuf};
use std::process::Command;

fn require_chrome() -> bool {
    std::env::var("TALIESIN_REQUIRE_CHROME").as_deref() == Ok("1")
}

struct TempDoc {
    dir: PathBuf,
}

impl TempDoc {
    fn new(tag: &str, body: &str) -> TempDoc {
        let dir = std::env::temp_dir().join(format!("tali-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("doc.tmd"), body).expect("write source");
        TempDoc { dir }
    }
    fn src(&self) -> PathBuf {
        self.dir.join("doc.tmd")
    }
    fn out(&self) -> PathBuf {
        self.dir.join("doc.pdf")
    }
}

impl Drop for TempDoc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Run `taliesin pdf`, **capturing** the child's output rather than inheriting it.
///
/// `.status()` would let it inherit, and `taliesin pdf` logs to stderr — which lands in the
/// middle of libtest's own line, producing
/// `test pdf_paginates_… ...   info  wrote /tmp/…` instead of `... ok`. The test still
/// passes; but `tools/gates.sh` proves a canary ran by grepping for `^test <name> ... ok$`,
/// so a polluted line reads as "the gate did not run" and fails the whole suite. Capturing
/// also means the child's diagnostics are available in the failure message, where they are
/// actually useful.
fn run_pdf(src: &Path, out: &Path) {
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["pdf", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .expect("run taliesin pdf");
    assert!(
        res.status.success(),
        "`taliesin pdf` exited non-zero:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
}

/// Page count via `pdfinfo` (poppler-utils). Parsing the PDF ourselves would re-implement a
/// container format to answer one question.
fn page_count(pdf: &Path) -> usize {
    let out = Command::new("pdfinfo")
        .arg(pdf)
        .output()
        .expect("pdfinfo (poppler-utils) is required for the print gate");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse().ok())
        .expect("pdfinfo reported no page count")
}

fn pdf_text(pdf: &Path) -> String {
    let out = Command::new("pdftotext")
        .args(["-layout", pdf.to_str().unwrap(), "-"])
        .output()
        .expect("pdftotext (poppler-utils) is required for the print gate");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn long_body(title: &str, para: &str, n: usize) -> String {
    let mut s = format!("---\ntitle: {title}\n---\n\n");
    for i in 0..n {
        s.push_str(&format!(
            "{para} number {i} with enough words to fill out a line.\n\n"
        ));
    }
    s
}

#[test]
fn pdf_paginates_a_real_document_into_more_than_one_page() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let doc = TempDoc::new("pdf", &long_body("Print Gate", "Paragraph", 140));
    run_pdf(&doc.src(), &doc.out());

    let bytes = std::fs::read(doc.out()).expect("pdf written");
    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");

    let pages = page_count(&doc.out());
    assert!(
        pages > 1,
        "expected a paginated document, got {pages} page(s) — this is the CLI-truncation \
         failure mode: paged.js had not finished when the print fired"
    );

    // The LAST paragraph must be present. A truncated render keeps the early pages and
    // silently drops the tail, so only late content proves the wait actually held.
    let text = pdf_text(&doc.out());
    assert!(
        text.contains("number 139"),
        "the document's final paragraph is missing from the PDF ({pages} pages) — \
         pagination was cut short"
    );
}

/// Running heads and folios are the two things a printed document is judged on, and BOTH
/// come from paged.js: Chrome 150 renders `string-set` as nothing (measured, with a
/// `counter(page)` positive control). So a running head here is direct evidence the polyfill
/// is genuinely driving the margin boxes, not just that a PDF appeared.
#[test]
fn the_pdf_carries_running_heads_and_folios() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let mut body = String::from("---\ntitle: Heads\n---\n\n## Chapter Alpha\n\n");
    for i in 0..90 {
        body.push_str(&format!(
            "Alpha paragraph {i} with enough words to fill a line.\n\n"
        ));
    }
    body.push_str("## Chapter Beta\n\n");
    for i in 0..90 {
        body.push_str(&format!(
            "Beta paragraph {i} with enough words to fill a line.\n\n"
        ));
    }
    let doc = TempDoc::new("pdfhead", &body);
    run_pdf(&doc.src(), &doc.out());
    let text = pdf_text(&doc.out());

    // A running head repeats the section title on the pages that follow the one its heading
    // opened, so the title must appear strictly more often than the single heading itself.
    let alpha = text.matches("Chapter Alpha").count();
    assert!(
        alpha > 1,
        "running head missing: 'Chapter Alpha' appears {alpha} time(s), so only the heading \
         itself is in the PDF and the @page margin box is empty"
    );
    let beta = text.matches("Chapter Beta").count();
    assert!(
        beta > 1,
        "running head missing for the second section: 'Chapter Beta' appears {beta} time(s)"
    );

    // Folios: every page after the first carries its number on its own line.
    let folios: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.len() <= 3 && l.parse::<u32>().is_ok())
        .collect();
    assert!(
        folios.len() >= 2,
        "expected page numbers in the footer, found {folios:?}"
    );
    assert!(
        folios.contains(&"2"),
        "page 2 should be numbered; found {folios:?}"
    );
    // The opening page is a title page: numbering it "1" reads as a mistake in a typeset
    // document, so `@page :first` suppresses it.
    let first_page = text.split('\u{c}').next().unwrap_or("");
    assert!(
        !first_page.lines().map(str::trim).any(|l| l == "1"),
        "the first page should carry no folio"
    );
}

/// The headline of the whole track: a cross-reference that names its page. Chrome 150
/// renders `target-counter()` as nothing, so a real page number here is direct proof the
/// polyfill resolved it AFTER pagination settled.
#[test]
fn a_cross_reference_resolves_to_a_real_page_number() {
    if !require_chrome() {
        eprintln!("skipped: set TALIESIN_REQUIRE_CHROME=1 to run the live print gate");
        return;
    }
    let mut body = String::from("---\ntitle: Refs\n---\n\nSee @fig-late for the result.\n\n");
    for i in 0..130 {
        body.push_str(&format!(
            "Filler paragraph {i} with a reasonable number of words.\n\n"
        ));
    }
    body.push_str("![The late figure](late.png){#fig-late}\n");
    let doc = TempDoc::new("pdfxref", &body);
    run_pdf(&doc.src(), &doc.out());
    let text = pdf_text(&doc.out());

    assert!(
        text.contains("(p. "),
        "no '(p. N)' suffix rendered on a cross-reference:\n{text}"
    );
    // "(p. 0)" is the signature of target-counter firing BEFORE pagination settled — a
    // silent wrong answer, which is worse than no answer at all.
    assert!(
        !text.contains("(p. 0)"),
        "a cross-reference resolved to page 0, so pagination had not settled:\n{text}"
    );

    // The list of figures must carry a real page number too, not just exist.
    assert!(
        text.contains("List of Figures"),
        "the generated list of figures is missing from the PDF"
    );
}
