//! Run the live-edit benchmark on a real corpus doc and emit artifacts: a markdown
//! table to stdout (snapshotted into RESULTS.md) and RESULTS.json (the raw metrics
//! the hero demo cites). Pure measurement, it never writes the corpus doc.
//!
//! `cargo run --release -p live-edit-bench [-- <port>]`. The end-to-end rows drive the
//! release binary this workspace builds, so run `cargo build --release -p taliesin-server`
//! first; without it they are skipped. `<port>` is where their preview listens (default:
//! any free port).

use live_edit_bench::e2e::{SaveToOp, measure_preview, release_binary};
use live_edit_bench::{
    e2e_markdown_report, markdown_report, measure_live_edit, measure_project_passes,
    project_markdown_report, write_synthetic_book,
};
use std::path::{Path, PathBuf};

/// Runs kept, best one published. `RESULTS.md` said "best of twelve" while the binary
/// measured exactly once, so the protocol lived in a sentence a reader had to obey by hand
/// and a plain `cargo run` silently published a best-of-one. It is in the instrument now.
const BEST_OF: usize = 12;

/// Runs of each whole-project pass, and rounds of each end-to-end save; the median is
/// published.
const RUNS: usize = 10;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let repo = Path::new(manifest).join("../..");
    let port: u16 = std::env::args()
        .nth(1)
        .map(|p| p.parse().expect("the one argument is a port"))
        .unwrap_or(0);
    let load = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|l| l.split_whitespace().next().map(str::to_string));
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
    // measure one document's seam, but a site preview also runs whole-project passes on
    // every save, so a book-sized save is not the warm-edit figure. The synthetic books give
    // the per-page slope a size the real projects here do not reach.
    let scratch = std::env::temp_dir().join(format!("live-edit-bench-{}", std::process::id()));
    let book = |pages: usize| -> PathBuf {
        let root = scratch.join(format!("book{pages}"));
        write_synthetic_book(&root, pages).expect("write the synthetic book");
        root
    };
    let projects: Vec<(String, PathBuf, &str)> = vec![
        (
            "docs/guide".into(),
            repo.join("docs/guide"),
            "using/choosing.tmd",
        ),
        ("docs/internals".into(), repo.join("docs/internals"), ""),
        (
            "corpus/tech-blog".into(),
            repo.join("corpus/tech-blog"),
            "posts/em-algorithm/index.tmd",
        ),
        ("synthetic book".into(), book(100), "chapters/ch050.tmd"),
        ("synthetic book".into(), book(500), "chapters/ch250.tmd"),
    ];
    let passes: Vec<_> = projects
        .iter()
        .filter_map(|(label, root, _)| measure_project_passes(label, root, RUNS))
        .collect();
    println!();
    print!("{}", project_markdown_report(&passes));

    // The third: what the author waits for, save to first message, which only the real
    // binary can answer.
    let mut end_to_end: Vec<SaveToOp> = Vec::new();
    match release_binary() {
        Some(bin) => {
            for ((label, root, page), pass) in projects.iter().zip(&passes) {
                if page.is_empty() {
                    continue;
                }
                match measure_preview(&bin, label, root, page, pass.pages, RUNS, port) {
                    Ok(row) => end_to_end.push(row),
                    Err(e) => eprintln!("end to end on {label}: {e}"),
                }
            }
            println!();
            print!("{}", e2e_markdown_report(&end_to_end));
        }
        None => eprintln!(
            "no release binary (cargo build --release -p taliesin-server): end-to-end rows skipped"
        ),
    }
    let _ = std::fs::remove_dir_all(&scratch);

    let json = serde_json::to_string_pretty(&serde_json::json!({
        "load_average_1m_before": load,
        "doc": m,
        "projects": passes,
        "end_to_end": end_to_end,
    }))
    .expect("serialize metrics");
    let out = format!("{manifest}/RESULTS.json");
    std::fs::write(&out, json + "\n").expect("write RESULTS.json");
    eprintln!("wrote {out}");
}
