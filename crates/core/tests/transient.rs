//! Editing-transience robustness: the preview re-renders on every keystroke, so
//! the renderer must never panic (or hang) on the *partial* document states a user
//! passes through while typing — a half-typed `:::` fence, an unterminated code
//! block, a dangling `$`, an unclosed YAML frame. A panic mid-edit shows the error
//! overlay and drops the live preview, which is exactly the value prop this tool is
//! built on. `render_document` is the pure core pipeline (no kernel/includes).

use std::panic::{AssertUnwindSafe, catch_unwind};
use taliesin_core::{diff_blocks, render_document};

/// Render `src`, capturing a panic as `Err(message)` instead of aborting the run.
fn try_render(src: &str) -> Result<usize, String> {
    catch_unwind(AssertUnwindSafe(|| render_document(src).blocks.len())).map_err(|e| {
        e.downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic>".into())
    })
}

/// Every line-prefix of every corpus + docs document: line 1, then lines 1-2, …,
/// the whole file. This is "type the document top to bottom"; each prefix is a
/// state the live preview actually renders.
#[test]
fn line_prefixes_of_every_doc_never_panic() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut qmds = Vec::new();
    collect_qmds(std::path::Path::new(root).join("corpus"), &mut qmds);
    collect_qmds(std::path::Path::new(root).join("docs"), &mut qmds);
    assert!(
        qmds.len() > 10,
        "expected to find the corpus; got {}",
        qmds.len()
    );

    // Sample line-prefixes: every boundary for short docs, step-capped to ~20 per
    // doc for long ones, so the sweep stays CI-fast. (The exhaustive every-line run
    // is ~10x slower and surfaced nothing beyond `half_typed_constructs`; to run it
    // locally, set the step to 1.)
    let mut failures = Vec::new();
    let mut prefixes_rendered = 0usize;
    for path in &qmds {
        let src = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = src.lines().collect();
        let total = lines.len();
        let step = (total / 20).max(1);
        let mut n = 1;
        while n <= total {
            let prefix = lines[..n].join("\n");
            prefixes_rendered += 1;
            if let Err(msg) = try_render(&prefix) {
                failures.push(format!("{}:{n} lines -> panic: {msg}", path.display()));
            }
            if n == total {
                break;
            }
            n = (n + step).min(total); // always finish on the complete document
        }
    }
    eprintln!(
        "rendered {prefixes_rendered} sampled line-prefixes across {} docs",
        qmds.len()
    );
    assert!(
        failures.is_empty(),
        "transient prefixes panicked:\n{}",
        failures.join("\n")
    );
}

/// Hand-picked half-typed constructs: the exact intermediate states between two
/// valid documents that a user types through.
#[test]
fn half_typed_constructs_never_panic() {
    let cases = [
        ("unclosed fenced div", ":::"),
        ("unclosed fenced div with attr", "::: {.callout-note}\nbody"),
        ("unclosed code fence", "```python\nprint(1)"),
        ("unclosed code fence info only", "```{python}"),
        ("dangling inline math", "text $x = "),
        ("dangling display math", "$$\n\\frac{a}{b}"),
        ("unterminated frontmatter", "---\ntitle: x"),
        ("frontmatter colon only", "---\ntitle:"),
        ("half-typed shortcode", "{{< embed deck"),
        ("half-typed include", "{{< include "),
        ("unclosed link", "see [the docs"),
        ("unclosed image", "![alt"),
        ("unclosed raw html", "<div class=\"x\">text"),
        ("unclosed html comment", "<!-- todo"),
        ("bare heading hash", "#"),
        ("unclosed emphasis", "this is *important"),
        ("unclosed table row", "| a | b |\n| -"),
        ("dangling cite bracket", "as shown in [@"),
        ("unclosed footnote", "text[^"),
        ("just a backslash", "\\"),
        ("nested unclosed divs", "::: a\n::: b\ncontent"),
        ("fence inside div, both open", "::: note\n```python"),
        ("attr block unclosed", "para\n{#id .class"),
        ("only frontmatter delimiter", "---"),
    ];
    let mut failures = Vec::new();
    for (name, src) in cases {
        if let Err(msg) = try_render(src) {
            failures.push(format!("{name:?} ({src:?}) -> panic: {msg}"));
        }
    }
    assert!(
        failures.is_empty(),
        "half-typed constructs panicked:\n{}",
        failures.join("\n")
    );
}

/// The full incremental seam under "typing": render consecutive line-prefixes and
/// diff each adjacent pair, the way the dev server does on every save. The diff must
/// never panic (block ids are unique, so the LCS is well-formed), so the live
/// preview applies a valid op set at every keystroke.
#[test]
fn diffing_consecutive_prefixes_never_panics() {
    // A feature-dense document: headings, a fenced div, a code fence, math, a table,
    // a list — the constructs whose half-typed states are riskiest to diff.
    let doc = "# Title\n\nIntro with $x^2$ math.\n\n::: {.callout-note}\nA note.\n:::\n\n\
        ```python\nprint(1)\n```\n\n## Section\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n\
        - one\n- two\n\nMore $\\frac{a}{b}$ and **bold**.\n";
    let lines: Vec<&str> = doc.lines().collect();
    let mut failures = Vec::new();
    for n in 1..lines.len() {
        let a = lines[..n].join("\n");
        let b = lines[..n + 1].join("\n");
        let res = catch_unwind(AssertUnwindSafe(|| {
            let va = render_document(&a);
            let vb = render_document(&b);
            diff_blocks(&va.blocks, &vb.blocks).len()
        }));
        if res.is_err() {
            failures.push(format!("diff of prefixes {n}->{} panicked", n + 1));
        }
    }
    assert!(
        failures.is_empty(),
        "incremental diff panicked mid-type:\n{}",
        failures.join("\n")
    );
}

fn collect_qmds(dir: std::path::PathBuf, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_qmds(p, out);
        } else if p.extension().is_some_and(|x| x == "qmd") {
            out.push(p);
        }
    }
}
