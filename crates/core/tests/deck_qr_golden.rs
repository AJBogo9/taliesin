//! Golden regression net for the deck's self-contained offline QR encoder (C-ADD-2,
//! `assets/js/deck.js`). The encoder is ~180 lines of table-driven bit manipulation
//! (Reed-Solomon over GF(256), format + version info bit tables, ISO/IEC 18004 mask-
//! penalty scoring) — the one deck capability with real algorithmic surface and no other
//! net under it. It was verified bit-for-bit against a reference encoder and by decoding
//! its output, but that check doesn't live in the repo; this pins the encoder's output for
//! a fixed set of URLs so a later edit to any table or `penalty()` regresses loudly instead
//! of silently shipping unscannable codes.
//!
//! It runs the ACTUAL bundled encoder through `node` (extracted from deck.js, so the test
//! and the shipped code can't drift) and skips cleanly when node is absent, like the
//! kernel-dependent tests. Each fingerprint is `<size>:<darkModules>:<fnv1a-hex>` over the
//! module matrix; the goldens below were taken from an encoder state whose every case was
//! confirmed scannable (opencv-decoded back to the exact input).

use std::process::Command;

/// (input URL, expected `size:darkModules:hash` fingerprint). Spans versions 1..10 and
/// exercises byte-mode UTF-8 (multibyte), file:// and long multi-block symbols.
const CASES: &[(&str, &str)] = &[
    ("http://x", "21:240:d952f6fb"),
    (
        "http://localhost:4388/#/ask-what-if-live?rate=15",
        "29:452:9db31160",
    ),
    (
        "https://talks.example.com/2026/native-decks.html#/a-deeper-topic?rate=15&mode=dark",
        "37:712:e5159bbb",
    ),
    (
        "file:///home/user/Talks/deck.html#/12/3?x=42",
        "29:428:849062f4",
    ),
    ("café résumé 日本語 ✓", "25:330:06d98ca4"),
    (
        "https://example.com/really/long/path/to/a/deck.html#/section-seven-the-big-reveal?a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8&i=9&j=10&k=11",
        "41:845:d4bf06a0",
    ),
];

/// Slice the `var qrEncode = (function () { … return encode; })();` IIFE out of deck.js so
/// the test drives the exact shipped encoder (not a copy).
fn extract_encoder() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/js/deck.js");
    let src = std::fs::read_to_string(path).expect("deck.js should exist");
    let start = src
        .find("var qrEncode = (function () {")
        .expect("deck.js should define the qrEncode IIFE");
    let ret = start
        + src[start..]
            .find("return encode;")
            .expect("qrEncode IIFE should end with `return encode;`");
    let end = ret
        + src[ret..]
            .find("})();")
            .expect("qrEncode IIFE should close with `})();`")
        + "})();".len();
    src[start..end].to_string()
}

#[test]
fn deck_qr_encoder_matches_golden_fingerprints() {
    let encoder = extract_encoder();
    // The fingerprint (FNV-1a-ish over the module bits) is computed IN node with the same
    // expression used to mint the goldens, so the Rust side only string-compares.
    let script = format!(
        "{encoder}\n\
         function fp(t){{var q=qrEncode(t),n=q.size,m=q.mods,d=0,h=2166136261>>>0;\
         for(var r=0;r<n;r++)for(var c=0;c<n;c++){{var b=m[r][c]?1:0;d+=b;h=((h^b)*16777619)>>>0;}}\
         return n+':'+d+':'+(h>>>0).toString(16).padStart(8,'0');}}\
         process.argv.slice(1).forEach(function(t){{console.log(fp(t));}});"
    );

    let inputs: Vec<&str> = CASES.iter().map(|(i, _)| *i).collect();
    let out = match Command::new("node")
        .arg("-e")
        .arg(&script)
        .args(&inputs)
        .output()
    {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping deck_qr_golden: `node` not found on PATH");
            return;
        }
        Err(e) => panic!("failed to launch node: {e}"),
    };
    assert!(
        out.status.success(),
        "node failed running the extracted QR encoder:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: Vec<String> = String::from_utf8(out.stdout)
        .expect("node output is utf-8")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        got.len(),
        CASES.len(),
        "expected {} fingerprints, got {}",
        CASES.len(),
        got.len()
    );
    for ((input, want), have) in CASES.iter().zip(got.iter()) {
        assert_eq!(
            have, want,
            "QR encoder output changed for {input:?}\n  expected {want}\n  got      {have}\n\
             If this change is intentional (a deliberate encoder change, re-verified as still \
             scannable), update the golden; otherwise a table/penalty regression shipped."
        );
    }
}
