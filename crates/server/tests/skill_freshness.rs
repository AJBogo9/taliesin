//! The shipped Claude Code skill (`editor/claude-code/skills/taliesin/SKILL.md`) is pinned
//! against the live binary so it can't rot the way the retired external scaffolder did
//! (it shipped `.qmd`/`quarto preview` because it lived outside the binary).
//!
//! Two guards: every `taliesin <verb>` the skill names must be a real dispatchable command
//! (checked against `taliesin completions bash`'s own command list), and the skill must not
//! carry the stale tokens the project shed.

use std::collections::HashSet;
use std::process::Command;

fn skill_md() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../editor/claude-code/skills/taliesin/SKILL.md"
    );
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read SKILL.md at {path}: {e}"))
}

/// The real command set, read from the binary's own completion script (`cmds="…"`), so this
/// can never disagree with what `main()` dispatches.
fn real_commands() -> HashSet<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["completions", "bash"])
        .output()
        .expect("run taliesin completions bash");
    let script = String::from_utf8_lossy(&out.stdout);
    let line = script
        .lines()
        .find(|l| l.trim_start().starts_with("cmds="))
        .expect("completions script defines cmds=");
    line.split('"')
        .nth(1)
        .expect("cmds is a quoted list")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Every `taliesin <lowercase-verb>` in the skill (prose uses `Taliesin` for the tool name,
/// so only real command invocations are lowercase).
fn named_verbs(md: &str) -> Vec<String> {
    let mut verbs = Vec::new();
    let mut rest = md;
    while let Some(i) = rest.find("taliesin ") {
        let after = &rest[i + "taliesin ".len()..];
        let verb: String = after
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
            .collect();
        if !verb.is_empty() {
            verbs.push(verb);
        }
        rest = after;
    }
    verbs
}

#[test]
fn skill_names_only_real_subcommands() {
    let md = skill_md();
    let cmds = real_commands();
    let verbs = named_verbs(&md);
    assert!(
        !verbs.is_empty(),
        "the skill should show `taliesin <verb>` commands"
    );
    for v in &verbs {
        assert!(
            cmds.contains(v),
            "SKILL.md names `taliesin {v}`, which is not a real command ({cmds:?})"
        );
    }
    // The loop-closer verbs must actually be taught.
    for must in ["check", "build"] {
        assert!(
            verbs.iter().any(|v| v == must),
            "the skill must teach `taliesin {must}`"
        );
    }
}

#[test]
fn skill_has_no_stale_tokens() {
    let md = skill_md().to_lowercase();
    for stale in [".qmd", "quarto", "revealjs", ".reveal", "reveal.js"] {
        assert!(
            !md.contains(stale),
            "SKILL.md carries the stale token `{stale}` (the skill drives Taliesin, not the shed tooling)"
        );
    }
}

#[test]
fn skill_teaches_the_single_editing_surface_and_the_check_gate() {
    let md = skill_md();
    assert!(
        md.contains("--format json"),
        "the skill must document the machine-readable check gate"
    );
    assert!(
        md.to_lowercase().contains("never the preview") || md.to_lowercase().contains("read-only"),
        "the skill must state edit-the-source-never-the-preview"
    );
}
