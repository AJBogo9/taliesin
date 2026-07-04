//! Run the live-edit benchmark on a real corpus doc and emit artifacts: a markdown
//! table to stdout (snapshotted into RESULTS.md) and RESULTS.json (the raw metrics
//! the hero demo cites). Pure measurement, it never writes the corpus doc.

use live_edit_bench::{markdown_report, measure_live_edit};
use std::path::Path;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let doc = format!("{manifest}/../../corpus/posts/em-algorithm/index.tmd");
    let src = std::fs::read_to_string(&doc).expect("read the em-algorithm corpus doc");
    let base = Path::new(&doc).parent().expect("doc has a parent dir");

    let m = measure_live_edit("corpus/posts/em-algorithm/index.tmd", &src, base, |s| {
        s.replace(
            "Let's start from a practical example.",
            "A freshly typed opening line.\n\nLet's start from a practical example.",
        )
    });

    print!("{}", markdown_report(&m));

    let json = serde_json::to_string_pretty(&m).expect("serialize metrics");
    let out = format!("{manifest}/RESULTS.json");
    std::fs::write(&out, json + "\n").expect("write RESULTS.json");
    eprintln!("wrote {out}");
}
