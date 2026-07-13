//! A site build emits a per-page cited-references sidecar (`<page>.citations.json`) for
//! reader-side AI/crawler legibility: the citation keys a page actually cites, so a machine
//! can read a page's references without parsing its prose. url-gated (like the other SEO
//! artifacts) and kept across rebuilds by the stale-sweep.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-cites-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn build_emits_per_page_cited_keys_sidecar() {
    let dir = tmp_dir("build");
    // `url:` gates the SEO artifacts (incl. this sidecar).
    fs::write(dir.join("_site.yml"), "title: S\nurl: https://ex.com\n").unwrap();
    fs::write(
        dir.join("references.bib"),
        "@book{bishop2006pattern,\n title={Pattern Recognition},\n author={Bishop, C.},\n year={2006}\n}\n",
    )
    .unwrap();
    fs::create_dir_all(dir.join("posts/p")).unwrap();
    fs::write(
        dir.join("posts/p/index.tmd"),
        "---\ntitle: A Study\ndate: 2026-04-14\nbibliography: ../../references.bib\n---\n\nSee [@bishop2006pattern] and the diagram in @fig-x.\n\n# References\n",
    )
    .unwrap();
    // A page that cites nothing gets no sidecar.
    fs::write(
        dir.join("plain.tmd"),
        "---\ntitle: Plain\ndate: 2026-04-14\n---\n\nNo citations here.\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("run build");
    assert!(
        status.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let sidecar = out.join("posts/p/index.citations.json");
    let body = fs::read_to_string(&sidecar)
        .unwrap_or_else(|e| panic!("cited-refs sidecar at {}: {e}", sidecar.display()));
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("sidecar is json");
    let cited: Vec<&str> = parsed["cited"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        cited,
        ["bishop2006pattern"],
        "sidecar lists the cited key, and NOT the @fig-x cross-reference"
    );
    assert_eq!(parsed["page"], "posts/p/index.html");

    // A page that cites nothing produces no sidecar.
    assert!(
        !out.join("plain.citations.json").exists(),
        "a page with no citations gets no sidecar"
    );

    let _ = fs::remove_dir_all(&dir);
}
