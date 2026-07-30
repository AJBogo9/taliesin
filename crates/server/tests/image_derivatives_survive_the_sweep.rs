//! The AVIF derivatives a build produces must still be there when the build finishes.
//!
//! **Why this test exists, specifically.** `build::sweep_stale` deletes every file under the
//! output tree that is not in its `keep` set — that is how a renamed page stops lingering
//! across rebuilds. A derivative is written by `build_one_page` and appears in *no* other
//! source of `keep`, so omitting one line in `build.rs` writes every `.avif` and then deletes
//! it again, on the same build, leaving a page whose `<picture>` points at files that are not
//! there. Every unit test in `image_opt` stays green through that, because none of them runs
//! the sweep.
//!
//! It is also the only test that covers the *normalized, output-relative* form those paths
//! have to take: `keep`'s own entries come from `strip_prefix(out)`, so an unnormalized entry
//! silently never matches.

use std::path::Path;
use std::process::Command;

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tali-imgsweep-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_site_build_keeps_the_avif_files_it_wrote() {
    let dir = tmp("site");
    let out = dir.join("_out");
    // 320x164, so exactly one rung and one encode: this test pays for a real AVIF encode and
    // has no reason to pay for three.
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/media/fit-small.png");
    std::fs::copy(&fixture, dir.join("shot.png")).expect("corpus fixture exists");
    std::fs::write(dir.join("_site.yml"), "title: \"Images\"\n").unwrap();
    std::fs::write(
        dir.join("index.tmd"),
        "---\ntitle: \"Home\"\n---\n\n![A shot.](shot.png){#fig-shot}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("build runs");
    assert!(
        output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(out.join("index.html")).expect("the page was written");
    let srcset = html
        .split_once("srcset=\"")
        .map(|(_, r)| r.split('"').next().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(
        !srcset.is_empty(),
        "the page must offer an AVIF srcset:\n{}",
        html.lines()
            .filter(|l| l.contains("<img") || l.contains("<picture"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Every file the page advertises must exist AFTER the sweep, which is the whole point.
    for entry in srcset.split(", ") {
        let rel = entry.split_whitespace().next().unwrap_or("");
        assert!(
            out.join(rel).is_file(),
            "the page advertises {rel} but the build's stale sweep removed it \
             (build.rs must extend `keep` with image_opt::Stats::written)"
        );
    }
    // And the never-upscale rule survives a real build: 320 px source, one rung.
    assert_eq!(
        srcset.split(", ").count(),
        1,
        "a 320px source must yield exactly one rung, not an upscaled second: {srcset}"
    );
}
