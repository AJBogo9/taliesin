//! Run the live-edit benchmark on a real corpus doc and emit artifacts: a markdown
//! table to stdout (snapshotted into RESULTS.md) and RESULTS.json (the raw metrics
//! the hero demo cites). Pure measurement, it never writes the corpus doc.

use live_edit_bench::{
    markdown_report, measure_live_edit, measure_project_save, project_markdown_report,
};
use std::path::Path;

/// Runs kept, best one published. `RESULTS.md` said "best of twelve" while the binary
/// measured exactly once, so the protocol lived in a sentence a reader had to obey by hand
/// and a plain `cargo run` silently published a best-of-one. It is in the instrument now.
const BEST_OF: usize = 12;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let doc = format!("{manifest}/../../corpus/tech-blog/posts/em-algorithm/index.tmd");
    let src = std::fs::read_to_string(&doc).expect("read the em-algorithm corpus doc");
    let base = Path::new(&doc).parent().expect("doc has a parent dir");

    // Structural fields (op counts, payload bytes) are deterministic across runs, so
    // keeping the fastest run picks a timing without changing any published shape.
    let runs: Vec<_> = (0..BEST_OF)
        .map(|_| {
            measure_live_edit(
                "corpus/tech-blog/posts/em-algorithm/index.tmd",
                &src,
                base,
                |s| {
                    s.replace(
                        "Let's start from a practical example.",
                        "A freshly typed opening line.\n\nLet's start from a practical example.",
                    )
                },
            )
        })
        .collect();
    let mut m = runs
        .iter()
        .min_by_key(|m| m.warm_edit_ns)
        .expect("BEST_OF is non-zero")
        .clone();
    // ONLY the first render in this process is actually cold: syntect's syntax set and the
    // other lazy statics are built on first use, so every later iteration measures a warm
    // one. Taking the best of twelve here would publish ~13 ms as a "cold render" against a
    // true ~135 ms — a tenfold understatement of the very number the warm edit is compared
    // to. Best-of applies to the repeatable rows; cold keeps the one honest sample.
    m.cold_render_ns = runs[0].cold_render_ns;

    print!("{}", markdown_report(&m));

    // The second question: what one save costs a whole PROJECT. The doc-level rows above
    // measure one document's seam, but a site preview also runs the O(pages) xref harvest on
    // every save, so a book-sized save is not the warm-edit figure.
    let projects: Vec<_> = ["docs/guide", "docs/internals", "corpus/tech-blog"]
        .iter()
        .filter_map(|rel| measure_project_save(rel, Path::new(&format!("{manifest}/../../{rel}"))))
        .collect();
    println!();
    print!("{}", project_markdown_report(&projects));

    let json = serde_json::to_string_pretty(&serde_json::json!({
        "doc": m,
        "projects": projects,
    }))
    .expect("serialize metrics");
    let out = format!("{manifest}/RESULTS.json");
    std::fs::write(&out, json + "\n").expect("write RESULTS.json");
    eprintln!("wrote {out}");
}
