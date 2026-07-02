//! The determinism invariant for concurrent site builds (Task 8): building the same
//! multi-page site sequentially (`--jobs 1`) and concurrently (`--jobs N`) must produce
//! byte-identical output. Parallelism may only change *scheduling*, never *results*.
//!
//! The corpus is `corpus/demo-book` — a small multi-page book with cross-page xrefs and
//! math but **no code cells**, so the comparison is kernel-free and fast while still
//! exercising the whole concurrent scheduler (spawn N tasks, bound by a semaphore, write
//! per-page files, aggregate in page order). A kernel-needing variant is gated on
//! `QMD_FAST_PYTHON`, mirroring the rest of the suite.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace `corpus/` dir (this crate lives two levels down at `crates/server`).
fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(name)
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qmd-parbuild-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Recursively copy `src` into `dst`, skipping any build/cache residue (`_book`, `_site`,
/// `_freeze`) so each run starts from clean source.
fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        if matches!(name_s.as_ref(), "_book" | "_site" | "_freeze") {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).unwrap();
        }
    }
}

/// Run `qmd-fast build <root> --jobs <jobs>` and return every rendered HTML file under
/// `<root>/<out_subdir>`, keyed by its path relative to that output dir.
fn build_and_collect(root: &Path, out_subdir: &str, jobs: &str) -> BTreeMap<String, Vec<u8>> {
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(root)
        .arg("--jobs")
        .arg(jobs)
        .output()
        .expect("run build");
    assert!(
        status.status.success(),
        "build (--jobs {jobs}) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr),
    );
    let out = root.join(out_subdir);
    let mut map = BTreeMap::new();
    collect_html(&out, &out, &mut map);
    assert!(
        !map.is_empty(),
        "no .html produced under {} (--jobs {jobs})",
        out.display()
    );
    map
}

fn collect_html(dir: &Path, base: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let p = entry.path();
        if p.is_dir() {
            collect_html(&p, base, map);
        } else if p.extension().and_then(|e| e.to_str()) == Some("html") {
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().to_string();
            map.insert(rel, fs::read(&p).unwrap());
        }
    }
}

/// Assert two output maps are byte-identical (same set of files, identical bytes).
fn assert_identical(seq: &BTreeMap<String, Vec<u8>>, par: &BTreeMap<String, Vec<u8>>) {
    let seq_keys: Vec<&String> = seq.keys().collect();
    let par_keys: Vec<&String> = par.keys().collect();
    assert_eq!(
        seq_keys, par_keys,
        "sequential and concurrent builds produced a different set of pages"
    );
    for (rel, seq_bytes) in seq {
        let par_bytes = &par[rel];
        assert_eq!(
            seq_bytes,
            par_bytes,
            "page {rel} differs between sequential (--jobs 1) and concurrent (--jobs 4) build \
             ({} vs {} bytes) — parallelism changed the result, breaking the determinism invariant",
            seq_bytes.len(),
            par_bytes.len(),
        );
    }
}

#[test]
fn sequential_and_concurrent_book_build_are_byte_identical() {
    // Two independent copies of the same source, built sequentially and concurrently, so
    // a shared `_freeze` can't make the second run trivially match the first.
    let base = tmp_dir("book");
    let seq_root = base.join("seq");
    let par_root = base.join("par");
    copy_tree(&corpus("demo-book"), &seq_root);
    copy_tree(&corpus("demo-book"), &par_root);

    let seq = build_and_collect(&seq_root, "_book", "1");
    let par = build_and_collect(&par_root, "_book", "4");

    assert_identical(&seq, &par);
    assert!(
        seq.len() >= 5,
        "expected the multi-page book to render >=5 pages, got {}: {:?}",
        seq.len(),
        seq.keys().collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn concurrent_build_is_self_consistent_across_runs() {
    // The same source built concurrently twice must also be byte-identical (catches any
    // ordering nondeterminism the seq-vs-par test could miss if both happened to share a
    // bias). Second run reuses the same tree, so `_freeze` is warm — still must match.
    let base = tmp_dir("repeat");
    let root = base.join("site");
    copy_tree(&corpus("demo-book"), &root);

    let first = build_and_collect(&root, "_book", "4");
    let second = build_and_collect(&root, "_book", "4");
    assert_identical(&first, &second);

    let _ = fs::remove_dir_all(&base);
}

/// Write a minimal website whose `index.qmd` carries a `listing:` over a `posts/`
/// directory of N sibling pages. The index is the canonical *cross-page consumer*: its
/// rendered cards are built from each sibling's front matter (title/date/description/
/// category). No code cells, so the build is kernel-free and fast. Returns the sibling
/// titles in the order the listing should show them (newest date first, the spec default).
fn write_listing_site(root: &Path, n_posts: usize) -> Vec<String> {
    fs::create_dir_all(root.join("posts")).unwrap();
    fs::write(
        root.join("_site.yml"),
        "title: Listing Site\nnav:\n  left:\n    - text: Blog\n      href: index.qmd\n",
    )
    .unwrap();
    // The index page: a `listing:` grid over `posts/`. This is the page whose *output*
    // would depend on its siblings if any build-order edge existed.
    fs::write(
        root.join("index.qmd"),
        "---\ntitle: Blog\nlisting:\n  contents: posts\n  sort: \"date desc\"\n  type: grid\n  categories: true\n---\n\nWelcome to the blog.\n",
    )
    .unwrap();
    // Siblings with ascending dates → the listing (date desc) shows them newest first.
    let mut titles_newest_first = Vec::new();
    for i in 0..n_posts {
        let title = format!("Post Number {i}");
        let dir = root.join(format!("posts/post-{i}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("index.qmd"),
            format!(
                "---\ntitle: \"{title}\"\ndate: \"2026-01-{day:02}\"\n\
                 description: \"Summary of post {i}.\"\ncategories: [cat{i}]\n---\n\n\
                 # {title}\n\nBody of post {i}.\n",
                day = i + 1,
            ),
        )
        .unwrap();
        titles_newest_first.push(title);
    }
    titles_newest_first.reverse(); // newest date (highest i) first
    titles_newest_first
}

/// Cross-page ordering safety (Task 9). A `listing:` index page is the canonical case the
/// brief flags: its output is assembled from its *sibling* pages. We assert two things at
/// once: (1) the jobs=1 and jobs=N builds are byte-identical (so concurrency never reordered
/// what the index renders), and (2) the index's cards reflect *every* sibling, in the
/// listing's declared order. Together these lock in the finding that this path has **no
/// build-order dependency**: listing cards come from sibling front matter resolved on the
/// in-memory `Site` model at discovery (before any page builds), never from a sibling's
/// rendered HTML — so a future change that started reading built output would break this.
#[test]
fn listing_index_reflects_all_siblings_jobs1_vs_jobs_n() {
    let base = tmp_dir("listing");
    let seq_root = base.join("seq");
    let par_root = base.join("par");
    let titles_newest_first = write_listing_site(&seq_root, 6);
    write_listing_site(&par_root, 6);

    let seq = build_and_collect(&seq_root, "_site", "1");
    let par = build_and_collect(&par_root, "_site", "4");

    // (1) Determinism: the whole site (index + every post) is byte-identical.
    assert_identical(&seq, &par);

    // (2) The concurrent index reflects every sibling, in listing order. Read the
    //     concurrently-built index (the harder case: posts may finish in any order).
    let index = String::from_utf8(par["index.html"].clone()).expect("index.html is utf-8");
    let mut last_pos = 0usize;
    for title in &titles_newest_first {
        let needle = format!("class=\"qmd-card-title\">{title}</h3>");
        let pos = index.as_str()[last_pos..].find(&needle).unwrap_or_else(|| {
            panic!(
                "listing index is missing sibling card `{title}` (or it is out of date-desc \
                 order) under a concurrent build — cross-page listing aggregation is not \
                 honoring all siblings:\n{index}"
            )
        });
        last_pos += pos + needle.len();
    }
    // Each sibling's description + category also rode along (card built from front matter).
    assert!(
        index.contains("Summary of post 0."),
        "listing card did not carry the sibling's description"
    );
    assert!(
        index.contains("data-cat=\"cat0\""),
        "listing card did not carry the sibling's category badge/filter"
    );

    let _ = fs::remove_dir_all(&base);
}

/// Write a minimal multi-page site (`_site.yml` + N pages) whose Python cells emit
/// **deterministic** output (arithmetic + a fixed DataFrame — no matplotlib, whose PNG
/// bytes are nondeterministic even between two sequential runs, see the doc note below).
/// This isolates the scheduler under real kernels without the test depending on a corpus
/// that happens to render reproducibly.
fn write_kernel_site(root: &Path, n_pages: usize) {
    fs::create_dir_all(root).unwrap();
    let mut nav = String::from("title: Kernel Site\ntoc: true\nnavbar:\n  left:\n");
    for i in 0..n_pages {
        nav.push_str(&format!("    - page{i}.qmd\n"));
        let page = format!(
            "---\ntitle: Page {i}\n---\n\n# Page {i}\n\n\
             Some prose so the page is not all code.\n\n\
             ```{{python}}\n\
             total = sum(range(1, {n}))\n\
             print(f\"page {i} total = {{total}}\")\n\
             total\n\
             ```\n",
            i = i,
            n = (i + 5) * 3,
        );
        fs::write(root.join(format!("page{i}.qmd")), page).unwrap();
    }
    fs::write(root.join("_site.yml"), nav).unwrap();
}

/// Kernel-backed determinism: a multi-page site *with real Python cells*, built
/// sequentially and concurrently, must still match byte-for-byte — the per-page kernel +
/// per-page `_freeze/<rel>.json` isolation is what makes concurrent kernels safe. Gated on
/// `QMD_FAST_PYTHON` like the rest of the exec tests.
///
/// NB: deliberately avoids matplotlib. Its rasterized PNG bytes differ even between two
/// *sequential* builds (a known nondeterministic-bytes source the brief calls out), so a
/// figure corpus would flake regardless of the scheduler. Textual cell output is stable.
#[test]
fn sequential_and_concurrent_match_with_code_cells() {
    if std::env::var_os("QMD_FAST_PYTHON").is_none() {
        eprintln!("skipping: QMD_FAST_PYTHON not set (no kernel)");
        return;
    }
    let base = tmp_dir("cells");
    let seq_root = base.join("seq");
    let par_root = base.join("par");
    write_kernel_site(&seq_root, 5);
    write_kernel_site(&par_root, 5);

    let seq = build_and_collect(&seq_root, "_site", "1");
    let par = build_and_collect(&par_root, "_site", "4");
    assert_identical(&seq, &par);
    assert!(
        seq.len() >= 5,
        "expected >=5 kernel-built pages, got {}",
        seq.len()
    );

    let _ = fs::remove_dir_all(&base);
}

/// File-isolation under concurrency (brief Step 5): two pages that export to the *same
/// relative path* (`figures/x.pdf`) must not clobber each other. Each page builds with its
/// kernel cwd set to the page's own directory (`Executor::in_dir(base)`), so a relative
/// write resolves under that page's dir — pages in different directories land in different
/// absolute files. We give the two pages *different* plots and assert both PDFs exist and
/// differ after a concurrent build (no last-writer-wins clobber).
///
/// Documented limitation: this isolation is per *directory*, not per page. Two pages in the
/// *same* directory share a cwd, so both `figures/x.pdf` would resolve to one file — but
/// that is already true of a sequential build today (cwd = page dir), so concurrency adds
/// no new hazard; it is out of scope here and unchanged by this task.
#[test]
fn concurrent_pages_with_same_relative_export_do_not_clobber() {
    if std::env::var_os("QMD_FAST_PYTHON").is_none() {
        eprintln!("skipping: QMD_FAST_PYTHON not set (no kernel)");
        return;
    }
    let base = tmp_dir("figiso");
    let root = base.join("site");
    fs::create_dir_all(root.join("alpha")).unwrap();
    fs::create_dir_all(root.join("beta")).unwrap();
    fs::write(
        root.join("_site.yml"),
        "title: Fig Isolation\nnavbar:\n  left:\n    - alpha/index.qmd\n    - beta/index.qmd\n",
    )
    .unwrap();
    // Two pages in different dirs, each exporting `figures/x.pdf` with a *different* plot.
    // NB: plain matplotlib (no `matplotlib.use('Agg')`) — the fig-export hook rides the
    // kernel's *inline* Figure formatter, which forcing the Agg backend would bypass.
    let page = |label: &str, y: &str| {
        format!(
            "---\ntitle: {label}\n---\n\n# {label}\n\n\
             ```{{python}}\n#| fig-export: figures/x.pdf\n\
             import matplotlib.pyplot as plt\n\
             plt.figure()\nplt.plot([0, 1, 2], {y})\nplt.title('{label}')\nplt.show()\n\
             ```\n"
        )
    };
    fs::write(root.join("alpha/index.qmd"), page("Alpha", "[0, 1, 4]")).unwrap();
    fs::write(root.join("beta/index.qmd"), page("Beta", "[4, 1, 0]")).unwrap();

    // Concurrent build: both pages' kernels run with cwd = their own dir.
    build_and_collect(&root, "_site", "4");

    // The fig-export PDFs land beside each page's *source* (cwd = page dir), not in `_site`.
    let alpha_pdf = root.join("alpha/figures/x.pdf");
    let beta_pdf = root.join("beta/figures/x.pdf");
    assert!(
        alpha_pdf.is_file(),
        "alpha's fig-export missing at {}",
        alpha_pdf.display()
    );
    assert!(
        beta_pdf.is_file(),
        "beta's fig-export missing at {}",
        beta_pdf.display()
    );
    let a = fs::read(&alpha_pdf).unwrap();
    let b = fs::read(&beta_pdf).unwrap();
    assert!(
        !a.is_empty() && !b.is_empty(),
        "a fig-export PDF is empty (alpha {} B, beta {} B)",
        a.len(),
        b.len()
    );
    // Different plots → different files: proves neither page clobbered the other.
    assert_ne!(
        a, b,
        "the two pages' same-named exports are byte-identical — one clobbered the other \
         (cwd isolation failed under concurrency)"
    );

    let _ = fs::remove_dir_all(&base);
}
