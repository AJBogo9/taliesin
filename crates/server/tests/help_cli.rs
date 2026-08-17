//! PL16: the top-level `help` groups its commands by purpose (Author / Preview & build /
//! Inspect / Editor), so the everyday three don't drown among the rest. This pins the
//! grouping + that no command is dropped by a future edit. The fourth section read
//! "Editor & agent" until Wave 2 cut every machine-facing verb out of it.

use std::process::Command;

fn help() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("help")
        .output()
        .expect("run taliesin help");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `(exit ok, stdout, stderr)` for an arbitrary argument list.
fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_groups_commands_by_purpose() {
    let h = help();
    // The four purpose sections are present, in order.
    let sections = ["Author", "Preview & build", "Inspect", "Editor"];
    let mut last = 0;
    for s in sections {
        let at = h
            .find(s)
            .unwrap_or_else(|| panic!("help is missing the `{s}` section:\n{h}"));
        assert!(at >= last, "sections out of order at `{s}`:\n{h}");
        last = at;
    }
    // Each command lands under the right section (a command appears after its header).
    let under = |cmd: &str, section: &str| {
        let sec = h.find(section).expect("section present");
        let c = h[sec..]
            .find(cmd)
            .unwrap_or_else(|| panic!("`{cmd}` not found under `{section}`:\n{h}"));
        // ...and before the NEXT section header, if any.
        sec + c
    };
    assert!(under("  init", "Author") < h.find("Preview & build").unwrap());
    assert!(under("  preview", "Preview & build") < h.find("Inspect").unwrap());
    assert!(under("  doctor", "Inspect") < h.find("Editor").unwrap());
    assert!(h.contains("Editor") && under("  lsp", "Editor") > 0);
    // No command was dropped in the reorder. ANCHORED to the two-space indent every
    // command row in `--help` carries: the list used to include the retired `run`, and
    // passed on the unanchored substring `"run "` matching ENV_HELP's prose "never run code
    // cells" — so the loop asserted a verb was documented by finding an unrelated sentence.
    for cmd in ["init", "preview", "build", "doctor", "lsp", "help"] {
        assert!(
            h.contains(&format!("  {cmd}")),
            "help dropped `{cmd}`:\n{h}"
        );
    }
    // …and the retired verb is not advertised as if it still existed.
    assert!(
        !h.contains("  run "),
        "`run` was cut in wave 13 but --help still lists it:\n{h}"
    );
}

/// An error path writes to **stderr and nothing else**.
///
/// `taliesin buidl .` printed 56 lines of help to stdout and one line of error to stderr,
/// so `taliesin buidl . 2>/dev/null` showed a wall of help and *lost the error entirely*,
/// and `taliesin buidl . | head` showed help with no error at all. The one line that
/// answers the question scrolled off the top of any terminal shorter than 57 rows. The
/// tool already got this right one function away: an unknown *flag* prints one line to
/// stderr and nothing to stdout.
#[test]
fn an_error_writes_to_stderr_and_nothing_to_stdout() {
    // An unknown command…
    let (ok, stdout, stderr) = run(&["buidl", "."]);
    assert!(!ok, "an unknown command exits non-zero");
    assert!(
        stdout.is_empty(),
        "an error path must write nothing to stdout, got {} lines:\n{stdout}",
        stdout.lines().count()
    );
    assert!(
        stderr.contains("build"),
        "the did-you-mean goes to stderr: {stderr}"
    );

    // …and a retired one, which carries its own note instead of a did-you-mean.
    let (ok, stdout, stderr) = run(&["run", "."]);
    assert!(!ok, "a retired command exits non-zero");
    assert!(
        stdout.is_empty(),
        "a retired verb must write nothing to stdout, got {} lines:\n{stdout}",
        stdout.lines().count()
    );
    assert!(
        stderr.contains("was removed"),
        "the retirement note goes to stderr: {stderr}"
    );

    // The success paths must not move: help is what those are FOR, and it belongs on
    // stdout so it can be piped.
    for args in [vec!["help"], vec!["--help"], vec![]] {
        let (ok, stdout, _) = run(&args);
        assert!(ok, "`taliesin {args:?}` succeeds");
        assert!(
            stdout.contains("Preview & build"),
            "`taliesin {args:?}` still prints the full help to stdout"
        );
    }
}
