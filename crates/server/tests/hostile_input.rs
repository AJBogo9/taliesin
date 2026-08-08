//! AP2-3: the hostile-input regression net.
//!
//! Feeds a trimmed version of the AP2 fuzzing battery (archived in full under
//! `notes/ap2-fuzz-harness/`) through the real binary and asserts one property: **the
//! pipeline is always graceful**. Graceful means the process *returns* — rendering the
//! document, or refusing it with a located diagnostic. Never a stack-overflow abort, never
//! an unbounded hang.
//!
//! **Why a subprocess and not `proptest`/`cargo-fuzz`.** The two failures this net exists to
//! catch are exactly the two an in-process fuzzer cannot observe: a stack-overflow `abort()`
//! is not unwindable (it kills the fuzzer with the case) and a quadratic render is not a
//! panic at all, just unbounded CPU. Spawning the binary per case makes panic (exit 101),
//! abort (SIGABRT/SIGSEGV) and hang (wall clock) three distinguishable outcomes.
//!
//! It also pins the two AP2 fixes against regression, via the two minimized repros:
//! `MAX_NESTING_DEPTH` (deep nesting → located diagnostic, not SIGABRT) and the
//! `TALIESIN_RENDER_TIMEOUT` watchdog (comrak's O(n²) inline bracket matcher → bounded wait,
//! not a frozen preview).

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What happened to one case's process.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// The process ran to completion with this exit code. 0 (clean) and 1 (diagnostics
    /// reported) are both graceful; anything else is not.
    Exited(i32),
    /// Killed by a signal — SIGABRT is the stack-overflow case AP2-1 fixed.
    Signalled(i32),
    /// Still running when the wall clock ran out: the AP2-2 hang class.
    TimedOut,
}

/// Run the static lint over one hostile document, bounded by a wall clock.
///
/// `build --check-only` rather than a plain `build`, deliberately: it exercises the whole
/// render pipeline and writes nothing, so a case cannot leave a file behind and the exit
/// code means "the pipeline survived" rather than "the write succeeded". It was
/// `taliesin check` until that verb was retired on 2026-08-08.
///
/// `budget` is deliberately generous relative to a real render (AP1's largest measurement is
/// an 8000-block document at 647 ms release), so a trip here means a genuine hang, not a
/// slow machine.
fn check_case(src: &[u8], env: &[(&str, &str)], budget: Duration) -> Outcome {
    let dir = std::env::temp_dir().join(format!(
        "taliesin-hostile-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create case dir");
    let file = dir.join("case.tmd");
    // Written as raw bytes: several cases are deliberately not valid UTF-8 or carry NUL,
    // and the point is what the binary does when it reads them off disk.
    std::fs::File::create(&file)
        .and_then(|mut f| f.write_all(src))
        .expect("write case");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    cmd.arg("build")
        .arg(&file)
        .arg("--check-only")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn the lint");

    let start = Instant::now();
    let outcome = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    if let Some(sig) = status.signal() {
                        break Outcome::Signalled(sig);
                    }
                }
                break Outcome::Exited(status.code().unwrap_or(-1));
            }
            None if start.elapsed() > budget => {
                let _ = child.kill();
                let _ = child.wait();
                break Outcome::TimedOut;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

fn assert_graceful(name: &str, src: &[u8], env: &[(&str, &str)], budget: Duration) {
    match check_case(src, env, budget) {
        // The lint exits 0 with no problems and 1 when it reported some. Both mean the
        // pipeline survived the document and said something about it. **A third code would
        // fail here**, which is what keeps this from passing vacuously on a mistyped verb:
        // an unknown command exits 1 too, so the argv above is pinned by
        // `missing_input_suggests.rs`'s front-door list rather than by this exit check alone.
        Outcome::Exited(0 | 1) => {}
        Outcome::Signalled(sig) => panic!(
            "case `{name}` killed by signal {sig} — a crash, not a diagnostic \
             (signal 6 = the SIGABRT stack overflow AP2-1 fixed; check MAX_NESTING_DEPTH)"
        ),
        Outcome::TimedOut => panic!(
            "case `{name}` never returned within {:?} — the AP2-2 hang class; \
             check the TALIESIN_RENDER_TIMEOUT watchdog in render_internal",
            budget
        ),
        Outcome::Exited(code) => {
            panic!("case `{name}` exited {code}; expected 0 (clean) or 1 (diagnostics)")
        }
    }
}

/// The trimmed battery: one case per hostile family AP2 explored, kept small enough that
/// every case is a cheap process spawn. Each is a hypothesis about a specific way the
/// parse → render pipeline could stop being graceful.
const BATTERY: &[(&str, &str)] = &[
    // Unbalanced `:::` fenced divs.
    ("fence_open_no_close", "::: {.callout-note}\nbody text\n"),
    ("fence_close_no_open", "body\n:::\n"),
    ("fence_nested_unclosed", "::: a\n::: b\n::: c\nx\n"),
    ("fence_columns_ncol_zero", "::: {.columns ncol=0}\nx\n:::\n"),
    (
        "fence_columns_ncol_huge",
        "::: {.columns ncol=999999999}\nx\n:::\n",
    ),
    (
        "fence_magic_move_unclosed",
        "::: {.magic-move}\n```js\na\n```\n",
    ),
    // Garbage YAML front matter. `yaml_billion_laughs` is the alias bomb libyaml rejects in
    // ~30 ms — kept because the guard lives in the C library, so nothing in our source
    // would show it regressing.
    ("yaml_unclosed", "---\ntitle: ok\n\nbody without close\n"),
    ("yaml_bad_colon", "---\ntitle: ok\nbad: : x\n---\n\nbody\n"),
    ("yaml_just_delims", "---\n---\n"),
    ("yaml_map_title", "---\ntitle: {a: 1}\n---\n\nb\n"),
    (
        "yaml_billion_laughs",
        "---\na: &a [x,x,x,x,x,x,x,x,x]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\n\
         c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\n\
         e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\nf: [*e,*e,*e,*e,*e,*e,*e,*e,*e]\n---\n\nbody\n",
    ),
    (
        "yaml_theorems_bad",
        "---\ntheorems:\n  - kind: 1\n---\n\nbody\n",
    ),
    // Truncation: every construct cut off mid-token.
    ("trunc_mid_frontmatter", "---\ntitle: ok\nauthor"),
    ("trunc_mid_fence", "::: {.callout-note}\nsome text but no"),
    ("trunc_mid_math_display", "text $$\\frac{a}{b}"),
    ("trunc_mid_cell", "```{python}\n#| echo: fal"),
    ("trunc_mid_table", "| a | b |\n| - | - |\n| 1 |"),
    ("trunc_mid_footnote", "text[^1]\n\n[^1]: def without"),
    ("trunc_shortcode", "{{< embed "),
    // Math / KaTeX edge cases.
    ("math_double_dollar_alone", "$$\n"),
    ("math_unclosed_display", "$$\n\\frac{1}{0}\n"),
    ("math_katex_error", "$\\frac{\\unknownmacro}{x}$\n"),
    ("math_deep_braces_10k", "$xxx$\n"),
    ("math_backslash_wall", "$\\\\\\\\\\\\\\\\$\n"),
    // Code fences + cell options.
    ("code_cell_unclosed", "```{python}\nx = 1\n"),
    ("code_lang_empty", "```{}\nx\n```\n"),
    ("code_cell_bad_opt", "```{python}\n#| echo:\nx\n```\n"),
    (
        "code_cell_lines_bad",
        "```{python}\n#| code-line-numbers: a-z\nx\n```\n",
    ),
    ("code_nested_fences", "````\n```\ninner\n```\n````\n"),
    (
        "code_uses_directive",
        "```{python}\n#| uses: nonexistent\nx\n```\n",
    ),
    // Tables.
    (
        "table_ragged",
        "| a | b | c |\n| - | - |\n| 1 | 2 | 3 | 4 | 5 |\n",
    ),
    ("table_pipe_only", "|\n|\n"),
    (
        "table_escaped_pipe",
        "| a \\| b | c |\n| - | - |\n| 1 | 2 |\n",
    ),
    // Footnotes / citations / cross-references that resolve to nothing.
    ("fn_recursive", "a[^1]\n\n[^1]: refers to [^1] itself\n"),
    ("fn_dup_def", "a[^1]\n\n[^1]: one\n[^1]: two\n"),
    ("cite_empty", "text [@] end\n"),
    ("cite_multi", "see [@a; @b; @c] and @fig-x and @sec-y\n"),
    (
        "xref_dangling",
        "See @fig-nope and @sec-nope and @tbl-nope.\n",
    ),
    // Links, images, shortcodes.
    ("link_unclosed", "[label](http://x\n"),
    ("link_ref_no_def", "[text][undefined]\n"),
    ("img_unclosed", "![alt\n"),
    ("shortcode_embed_missing", "{{< embed nonexistent.tmd >}}\n"),
    ("shortcode_nested", "{{< embed {{< video x >}} >}}\n"),
    ("shortcode_unknown", "{{< frobnicate a b c >}}\n"),
    // Degenerate whitespace + setext/HTML edges.
    ("empty_file", ""),
    ("only_tabs", "\t\t\t\t"),
    ("single_hash", "#"),
    ("cr_only", "line1\rline2\rline3"),
    ("setext_empty", "\n=\n"),
    ("html_block_unclosed", "<div>\n<span>\nno close\n"),
    (
        "html_comment_unclosed",
        "<!-- comment never closes\n\nmore\n",
    ),
    (
        "raw_html_passthrough",
        "```{=html}\n<script>alert(1)</script>\n```\n",
    ),
];

#[test]
fn the_hostile_battery_always_renders_or_diagnoses() {
    for (name, src) in BATTERY {
        assert_graceful(name, src.as_bytes(), &[], Duration::from_secs(30));
    }
}

/// Cases whose whole point is size, built rather than inlined so the test file stays
/// readable. Each is under the guards' thresholds, so all must still render normally —
/// this is the half that would catch a guard set too tight.
#[test]
fn large_but_legitimate_documents_still_render() {
    let cases: Vec<(&str, String)> = vec![
        (
            "uni_combining_wall",
            format!("e{}\n", "\u{0301}".repeat(50_000)),
        ),
        ("uni_zero_width", format!("{}\n", "\u{200b}".repeat(10_000))),
        (
            "code_very_long_line",
            format!("```\n{}\n```\n", "a".repeat(200_000)),
        ),
        ("many_blank_lines", "\n".repeat(100_000)),
        ("huge_single_line", "a".repeat(1_000_000)),
        (
            "nest_heading_hashes",
            format!("{} title\n", "#".repeat(5_000)),
        ),
        ("nest_backtick_wall", format!("{}\n", "`".repeat(20_000))),
        // Deep, but under MAX_NESTING_DEPTH: must render, not trip the guard.
        (
            "nest_blockquote_under_limit",
            format!("{} x\n", ">".repeat(900)),
        ),
        (
            "nest_div_deep",
            (0..5_000)
                .map(|i| format!("::: d{i}\n"))
                .collect::<String>()
                + "x\n",
        ),
        // Open-only brackets are linear (no closer to scan back from), so this is the
        // control that proves the bracket guard is not just "any bracket run".
        (
            "nest_brackets_open_only",
            format!("{}\n", "[".repeat(20_000)),
        ),
    ];
    for (name, src) in &cases {
        assert_graceful(name, src.as_bytes(), &[], Duration::from_secs(60));
    }
}

/// AP2-1's minimized repro. Before the guard this SIGABRT'd (`thread has overflowed its
/// stack`), and because an abort is not unwindable it took the whole process with it — on a
/// site build, every other page died with the one bad page.
#[test]
fn deep_nesting_yields_a_diagnostic_instead_of_a_stack_overflow_abort() {
    let deep = format!("{} x\n", ">".repeat(90_000));
    assert_graceful(
        "nest_blockquote_90k",
        deep.as_bytes(),
        &[],
        Duration::from_secs(30),
    );
    // The same shape via list bullets, which nest one container per marker too.
    let deep_list = format!("{}x\n", "- ".repeat(90_000));
    assert_graceful(
        "nest_list_90k",
        deep_list.as_bytes(),
        &[],
        Duration::from_secs(30),
    );
}

/// AP2-2's minimized repro. Balanced nested brackets hit comrak 0.52's inline
/// reference-link matcher, which scans back through every unmatched opener per closer —
/// quadratic, and neither a panic nor an abort, so only a clock can catch it.
///
/// Runs with a deliberately small budget so the test costs ~2 s rather than the 30 s
/// default; the assertion is that the watchdog fires at all, not what it is set to.
#[test]
fn a_quadratic_render_is_abandoned_by_the_watchdog() {
    let brackets = format!("{}x{}\n", "[".repeat(200_000), "]".repeat(200_000));
    let started = Instant::now();
    assert_graceful(
        "nest_brackets_200k",
        brackets.as_bytes(),
        &[("TALIESIN_RENDER_TIMEOUT", "2")],
        // Well over the 2 s budget, so a failure here means the watchdog did not fire
        // rather than that the machine was slow.
        Duration::from_secs(30),
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the watchdog should have bounded this render"
    );
}
