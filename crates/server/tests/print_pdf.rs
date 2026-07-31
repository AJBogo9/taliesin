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

fn run_pdf(src: &Path, out: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["pdf", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .status()
        .expect("run taliesin pdf");
    assert!(status.success(), "`taliesin pdf` exited non-zero");
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
