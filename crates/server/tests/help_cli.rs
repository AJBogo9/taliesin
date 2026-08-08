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
    assert!(under("  check", "Inspect") < h.find("Editor").unwrap());
    assert!(h.contains("Editor") && under("  lsp", "Editor") > 0);
    // No command was dropped in the reorder.
    for cmd in [
        "init",
        "new ",
        "preview",
        "build ",
        "check",
        "doctor",
        "lsp",
        "completions",
        "help,",
    ] {
        assert!(h.contains(cmd), "help dropped `{cmd}`:\n{h}");
    }
}
