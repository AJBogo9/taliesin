//! An executed cell's image must not carry `alt="output"` (item 41's separable a11y half).
//!
//! Gated on `TALIESIN_PYTHON` like the rest of the exec tests; `TALIESIN_REQUIRE_KERNEL=1`
//! (set by `tools/gates.sh` and the CI kernel job) turns the skip into a hard failure so
//! this coverage cannot quietly die.
//!
//! **This lived in `tests/r_kernel.rs` until 2026-08-08**, when `{r}` was withdrawn. It was
//! the one assertion in that file which was never about R: the bug was in
//! `render_media`'s **generic** PNG fallback, which every inline image except matplotlib's
//! reached, and an R figure was merely where it was measured.
//!
//! **The replacement must not use matplotlib**, and that is the whole difficulty of porting
//! it. Matplotlib has its own twin-render path (the light/dark pair), which always emitted
//! `alt=""` and so bypasses the fallback entirely — a matplotlib cell here would pass
//! against the very code the bug lived in. PIL is the accessible non-matplotlib producer of
//! a raw `image/png` output, and it needs no extra install beyond what matplotlib already
//! pulls in.

use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-figalt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `Some(program)` when a Python interpreter is configured, `None` to skip — unless
/// `TALIESIN_REQUIRE_KERNEL=1`, which makes a missing interpreter a hard failure.
fn python_program() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the executed-figure \
                 alt-text path would silently not run"
            );
            eprintln!("skip: set TALIESIN_PYTHON to a python with ipykernel + pillow");
            None
        }
    }
}

/// The image is spliced into a captioned `<figure>`, so the caption is already the
/// accessible description. A second one reading "output" is noise a screen reader says
/// aloud before it reaches the sentence that means something.
#[test]
fn an_executed_figure_carries_no_alt_text_beside_its_caption() {
    let Some(program) = python_program() else {
        return;
    };
    let dir = tmp_dir("alt");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: A raw PNG\n---\n\n\
         ```{python}\nfrom PIL import Image\nImage.new(\"RGB\", (24, 16), (200, 30, 30))\n```\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_PYTHON", &program)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = std::fs::read_to_string(dir.join("doc.html")).expect("built page");
    // Needle the emitted tag, not the bare token: the page inlines the whole CSS/JS
    // payload, so a loose substring check can match something that is not this image.
    assert!(
        html.contains(r#"<img alt="" src="data:image/png;base64,"#),
        "an executed figure is emitted with an empty alt; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !html.contains(r#"<img alt="output""#),
        "`alt=\"output\"` is noise read aloud beside the caption that already describes it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
