//! Item 78: the figure recolour has no notion of "text sitting on a data fill", so it can
//! *cause* the contrast failure it exists to prevent.
//!
//! `MPL_THEME_PREAMBLE`'s `_tali_recolour` sets **every** `Text` in a figure to the reader's
//! foreground so a figure tracks the page theme. That is right for titles, axis labels and
//! ticks, which sit on the transparent page background. It is wrong for an annotation drawn
//! *inside* a data-coloured mark — a heatmap cell, an image — whose background does **not**
//! change with the theme. Measured on `corpus/tech-blog/posts/pca-geometry/`: the `1.00`
//! cells are near-black `#67000d`, the author wrote `color="white"` for exactly that reason,
//! and the light render recoloured those labels to near-black `#1a1a1a`, making them
//! illegible on near-black.
//!
//! The author cannot fix it in the document, because an explicit `color=` is precisely what
//! the recolour overrides. That is what makes it a tool defect and not a corpus one.
//!
//! These drive the **shipped** preamble through a real kernel rather than a copy of it: the
//! test asks the kernel for the colours `_tali_recolour` left behind. A copy of the function
//! in the test would pass forever while the shipped one regressed.
//!
//! Gated on `TALIESIN_PYTHON`; `TALIESIN_REQUIRE_KERNEL=1` turns the skip into a hard
//! failure so this coverage cannot silently regress to zero. Also needs matplotlib — with
//! `TALIESIN_REQUIRE_KERNEL=1` a matplotlib-less interpreter is a failure too, since the
//! whole point of the file is a matplotlib behaviour.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-fig-theme-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn python_or_skip() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the figure-recolour \
                 contrast pin would silently skip."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

/// Whether `py` can import matplotlib. Under the CI canary a missing matplotlib is a hard
/// failure, mirroring `python_or_skip`: a green run here must mean the property was checked.
fn matplotlib_or_skip(py: &str) -> bool {
    let ok = Command::new(py)
        .args(["-c", "import matplotlib"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        assert!(
            std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
            "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON has no matplotlib: the \
             figure-recolour contrast pin would silently skip."
        );
        eprintln!("skipping: TALIESIN_PYTHON has no matplotlib");
    }
    ok
}

/// Build a doc whose cell prints what `_tali_recolour` did, run it, return the page HTML.
///
/// The cell reaches into `_tali_recolour` directly: the function the preamble installed is
/// the unit under test, and calling it lets the test read colours back as text instead of
/// decoding two PNGs and comparing pixels.
fn run_probe(tag: &str, cell: &str, py: &str) -> String {
    let dir = tmp_dir(tag);
    let src = dir.join("doc.tmd");
    fs::write(
        &src,
        format!("---\ntitle: T\n---\n\n```{{python}}\n{cell}\n```\n"),
    )
    .unwrap();
    let out = dir.join("out.html");
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", src.to_str().unwrap(), out.to_str().unwrap()])
        .env("TALIESIN_PYTHON", py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run taliesin build");
    assert!(
        status.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    fs::read_to_string(&out).expect("read built page")
}

/// A heatmap with a white annotation on a dark cell, plus ordinary chrome (title, axis
/// label). Prints `ANNOTATION=<colour>` and `TITLE=<colour>` after a light-theme recolour.
const HEATMAP_PROBE: &str = r##"
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

fig, ax = plt.subplots()
ax.imshow(np.array([[1.0, 0.0], [0.0, 1.0]]), cmap="RdBu_r", vmin=-1, vmax=1)
# The author's deliberate choice: white text, because the cell under it is near-black.
ann = ax.text(0, 0, "1.00", ha="center", va="center", color="white")
ax.set_title("Covariance")
ax.set_xlabel("feature")

# Recolour for the LIGHT theme exactly as the inline formatter does, then read back.
saved = _tali_recolour(fig, "#1a1a1a", "#d0d0d0")
print("ANNOTATION=%s" % (ann.get_color(),))
print("TITLE=%s" % (ax.title.get_color(),))
print("XLABEL=%s" % (ax.xaxis.label.get_color(),))
for _set, _val in reversed(saved):
    _set(_val)
print("RESTORED=%s" % (ann.get_color(),))
plt.close(fig)
"##;

#[test]
fn an_annotation_on_a_data_fill_keeps_its_own_colour() {
    let Some(py) = python_or_skip() else { return };
    if !matplotlib_or_skip(&py) {
        return;
    }
    let html = run_probe("heatmap", HEATMAP_PROBE, &py);

    // The defect: this used to read `ANNOTATION=#1a1a1a` — near-black text placed on a
    // near-black heatmap cell, in the render where the author had explicitly asked for white.
    assert!(
        html.contains("ANNOTATION=white"),
        "text drawn inside a data fill must keep its own colour (the fill does not change \
         with the page theme, so recolouring the text to the page foreground is what makes \
         it illegible). Got:\n{}",
        extract_probe_lines(&html)
    );

    // …and the fix must not disarm the feature. Chrome sitting on the transparent page
    // background is exactly what the recolour is for, and must still track the theme.
    assert!(
        html.contains("TITLE=#1a1a1a"),
        "a title sits on the page background and must still follow the theme. Got:\n{}",
        extract_probe_lines(&html)
    );
    assert!(
        html.contains("XLABEL=#1a1a1a"),
        "an axis label must still follow the theme. Got:\n{}",
        extract_probe_lines(&html)
    );
    // The restore path must stay exact, or the second (dark) render starts from a mutated
    // figure and the two variants drift.
    assert!(
        html.contains("RESTORED=white"),
        "the figure is restored exactly after rendering. Got:\n{}",
        extract_probe_lines(&html)
    );
}

/// A data-space annotation on an ordinary line plot sits on the transparent page
/// background, so it MUST still be recoloured. This is the false-positive guard: a fix that
/// skipped every `transData` text would break every labelled line chart.
const LINEPLOT_PROBE: &str = r##"
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

fig, ax = plt.subplots()
ax.plot([0, 1, 2], [0, 1, 4])
note = ax.text(1.0, 1.0, "inflection")
saved = _tali_recolour(fig, "#1a1a1a", "#d0d0d0")
print("NOTE=%s" % (note.get_color(),))
for _set, _val in reversed(saved):
    _set(_val)
plt.close(fig)
"##;

#[test]
fn a_data_space_annotation_with_no_fill_under_it_still_follows_the_theme() {
    let Some(py) = python_or_skip() else { return };
    if !matplotlib_or_skip(&py) {
        return;
    }
    let html = run_probe("lineplot", LINEPLOT_PROBE, &py);
    assert!(
        html.contains("NOTE=#1a1a1a"),
        "an annotation on the transparent page background must still track the theme -- \
         otherwise the fix for item 78 breaks every labelled line chart. Got:\n{}",
        extract_probe_lines(&html)
    );
}

/// Pull the probe's `KEY=value` lines out of the built page for a readable failure message.
fn extract_probe_lines(html: &str) -> String {
    html.lines()
        .filter(|l| {
            ["ANNOTATION=", "TITLE=", "XLABEL=", "RESTORED=", "NOTE="]
                .iter()
                .any(|k| l.contains(k))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
