//! Live `{r}` cell execution against a real IRkernel.
//!
//! Why this file exists: `{r}` cells and `TALIESIN_R` are advertised as first-class in
//! the README, but **nothing exercised the R path**. CI installs only Python; there was
//! no `setup-r` step, no `TALIESIN_R`-gated test anywhere, and no assertion on
//! `KernelSpec::r()`'s argv. Meanwhile the Python side has a canary that HARD-FAILS if
//! its interpreter goes missing (`TALIESIN_REQUIRE_KERNEL`), built precisely so that
//! coverage could never silently regress to zero. The guard existed for the language the
//! author looks at; this is the same guard for the one he doesn't.
//!
//! Gated on `TALIESIN_R` like the rest of the exec tests. `TALIESIN_REQUIRE_R=1` (set by
//! the CI `kernel-r` job) turns the skip into a hard failure.

use std::path::PathBuf;
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-r-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

/// `Some(program)` when an R interpreter is configured, `None` to skip — unless
/// `TALIESIN_REQUIRE_R=1`, which makes a missing interpreter a hard failure so this
/// coverage cannot quietly die the way the pre-`TALIESIN_REQUIRE_KERNEL` exec tests could.
fn r_program() -> Option<String> {
    match std::env::var("TALIESIN_R") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_R").is_none(),
                "TALIESIN_REQUIRE_R=1 but TALIESIN_R is unset: the R execution path would \
                 silently go untested, which is exactly what this gate exists to prevent"
            );
            eprintln!("skipping: TALIESIN_R not set (no R kernel)");
            None
        }
    }
}

/// A real `{r}` cell executes and its stdout is spliced into the built page. This is the
/// R twin of the Python canary: it proves the IRkernel argv, the ZMQ handshake, stdout
/// capture, and warm state across cells all work, not just that R is installed.
#[test]
fn r_cells_execute_and_persist_state_across_cells() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("exec");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R probe\n---\n\n\
         ```{r}\nx <- 6 * 7\ncat(\"answer\", x, \"\\n\")\n```\n\n\
         ```{r}\n# the warm kernel must still hold `x` from the cell above\ncat(\"still\", x, \"\\n\")\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
        .env("TALIESIN_NO_CACHE", "1") // never let a freeze hit stand in for real execution
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let html = std::fs::read_to_string(dir.join("doc.html")).expect("built page");
    assert!(
        html.contains("answer 42"),
        "the {{r}} cell must actually execute; stderr was: {}\n",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        html.contains("still 42"),
        "the warm R kernel must keep `x` across cells (state persistence)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An R error surfaces as a diagnostic instead of failing silently or wedging the build.
#[test]
fn an_r_error_is_reported_not_swallowed() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("err");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R error probe\n---\n\n```{r}\nstop(\"deliberate failure\")\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    let html = std::fs::read_to_string(dir.join("doc.html")).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        html.contains("deliberate failure") || stderr.contains("deliberate failure"),
        "an R error must surface (page or console), not vanish.\nstderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The colour type byte of a PNG's IHDR: 2 = RGB (opaque), 6 = RGBA. Reading the header
/// is enough — an 8-bit RGB PNG has no alpha channel at all, so it *cannot* be
/// transparent, and that is the whole question here.
fn png_colour_type(data_uri_png_b64: &str) -> u8 {
    // The byte we want is at offset 25, so decoding the first 36 base64 chars (27 bytes)
    // is plenty — and avoids pulling a base64 crate into the workspace for one header.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(27);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in data_uri_png_b64.bytes().take(36) {
        let Some(v) = ALPHABET.iter().position(|&a| a == c) else {
            break;
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    assert_eq!(&out[1..4], b"PNG", "inline figure is a PNG data URI");
    // 8-byte signature, then the IHDR chunk: len(4) type(4) w(4) h(4) bitdepth(1) colour(1)
    out[25]
}

/// Pull every inline PNG payload out of a built page, in document order.
fn inline_pngs(html: &str) -> Vec<&str> {
    html.match_indices("data:image/png;base64,")
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            &rest[..rest.find('"').expect("the src attribute closes")]
        })
        .collect()
}

/// R's inline device is opened with an opaque white background, so a figure whose own
/// backgrounds the author made transparent still came out as a white slab — glaring on
/// the dark theme, which is the default. `KernelSpec::r`'s preamble now asks the device
/// for transparency.
///
/// Both halves are asserted, because the fix is only safe if it is *additive*: a figure
/// that asks for transparency gets an alpha channel, and a DEFAULT ggplot (which paints
/// its own white `plot.background`) is left exactly as opaque as it was. Without the
/// second half this test would still pass on a change that made every existing R figure
/// transparent, i.e. unreadable dark-on-dark.
#[test]
fn a_transparent_r_figure_keeps_its_alpha_and_a_default_one_stays_opaque() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("figbg");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R figure background\n---\n\n\
         ```{r}\n\
         suppressPackageStartupMessages(library(ggplot2))\n\
         options(repr.plot.width = 4, repr.plot.height = 3)\n\
         ggplot(mtcars, aes(wt, mpg)) + geom_point() +\n\
           theme(plot.background = element_rect(fill = \"transparent\", colour = NA),\n\
                 panel.background = element_rect(fill = \"transparent\", colour = NA))\n\
         ```\n\n\
         ```{r}\nggplot(mtcars, aes(wt, mpg)) + geom_point()\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let html = std::fs::read_to_string(dir.join("doc.html")).expect("built page");
    let pngs = inline_pngs(&html);
    assert_eq!(
        pngs.len(),
        2,
        "expected one figure per cell; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        png_colour_type(pngs[0]),
        6,
        "a figure that asked for a transparent background must keep an alpha channel — \
         without the device preamble it rasterises onto opaque white, which reads as a \
         white slab on the dark theme"
    );
    assert_eq!(
        png_colour_type(pngs[1]),
        2,
        "a DEFAULT ggplot paints its own white plot.background and must stay opaque: the \
         preamble is additive, and making every existing R figure transparent would make \
         them unreadable dark-on-dark"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Item 41's separable a11y half: an executed cell's image must not carry `alt="output"`.
///
/// The image is spliced into a captioned `<figure>`, so the caption is already the
/// accessible description. A second one reading "output" is noise a screen reader says
/// aloud before it reaches the sentence that means something. The matplotlib twin-render
/// path always emitted `alt=""`; every other inline image (an R figure, a PIL image) got
/// `alt="output"` from the generic PNG fallback in `render_media`.
///
/// Uses an R figure because that is where it was measured, and because the R path is the
/// one with no formatter of its own.
#[test]
fn an_executed_figure_carries_no_alt_text_beside_its_caption() {
    let Some(program) = r_program() else { return };
    let dir = tmp_dir("alt");
    let doc = dir.join("doc.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: R figure\n---\n\n\
         ```{r}\nplot(1:10, (1:10)^2)\n```\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", doc.to_str().unwrap()])
        .env("TALIESIN_R", &program)
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
