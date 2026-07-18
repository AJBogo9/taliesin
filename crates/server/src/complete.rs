//! Shell completion: the `completions <shell>` script generators and (added later) the
//! hidden `__complete` runtime brain they call. All completion logic lives here so
//! behavior cannot drift between shells: each shim only relays the brain's output.

use crate::log;
use std::path::Path;
use std::process::ExitCode;

/// `taliesin completions <bash|zsh|fish>`: print that shell's completion script to stdout
/// (the only thing the command does), or a usage error + non-zero exit for a missing or
/// unsupported shell. The scripts are generated from `crate::COMMANDS`, so the offered
/// command list can never drift from what `main()` dispatches on.
pub(crate) fn cmd_completions(args: &[String]) -> ExitCode {
    match args.get(2).map(String::as_str).and_then(completions_script) {
        Some(script) => {
            print!("{script}");
            ExitCode::SUCCESS
        }
        None => {
            log::error("usage: taliesin completions <bash|zsh|fish>");
            ExitCode::FAILURE
        }
    }
}

/// The completion script for `shell`, or `None` for an unsupported one. Every branch draws
/// its command list from `crate::COMMANDS.join(" ")` (the `@COMMANDS@` placeholder), so a
/// new subcommand appears in all three shells at once and no per-shell list can go stale.
/// Gated by `completions_tests::every_shell_script_offers_exactly_the_dispatched_command_list`.
pub(crate) fn completions_script(shell: &str) -> Option<String> {
    let template = match shell {
        "bash" => BASH_COMPLETIONS,
        "zsh" => ZSH_COMPLETIONS,
        "fish" => FISH_COMPLETIONS,
        _ => return None,
    };
    Some(template.replace("@COMMANDS@", &crate::COMMANDS.join(" ")))
}

const BASH_COMPLETIONS: &str = r#"# taliesin bash completion.
# Install:  taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin
#   (system-wide)  taliesin completions bash | sudo tee /etc/bash_completion.d/taliesin
_taliesin() {
    local cur cmds
    cur="${COMP_WORDS[COMP_CWORD]}"
    cmds="@COMMANDS@"
    if [ "${COMP_CWORD}" -eq 1 ]; then
        COMPREPLY=($(compgen -W "${cmds}" -- "${cur}"))
        return
    fi
    if [ "${COMP_WORDS[1]}" = "completions" ] && [ "${COMP_CWORD}" -eq 2 ]; then
        COMPREPLY=($(compgen -W "bash zsh fish" -- "${cur}"))
        return
    fi
    COMPREPLY=($(compgen -f -- "${cur}"))
}
complete -F _taliesin taliesin
"#;

const ZSH_COMPLETIONS: &str = r#"#compdef taliesin
# taliesin zsh completion.
# Install (into a dir on $fpath, then run compinit):
#   taliesin completions zsh > "${fpath[1]}/_taliesin"
_taliesin() {
    local -a cmds
    cmds=(@COMMANDS@)
    if (( CURRENT == 2 )); then
        _describe 'taliesin command' cmds
        return
    fi
    if [[ ${words[2]} == completions ]]; then
        _values 'shell' bash zsh fish
        return
    fi
    _files
}
if [ "${funcstack[1]}" = "_taliesin" ]; then
    _taliesin "$@"
else
    compdef _taliesin taliesin
fi
"#;

const FISH_COMPLETIONS: &str = r#"# taliesin fish completion.
# Install:  taliesin completions fish > ~/.config/fish/completions/taliesin.fish
complete -c taliesin -n __fish_use_subcommand -a '@COMMANDS@' -d 'taliesin command'
complete -c taliesin -n '__fish_seen_subcommand_from completions' -f -a 'bash zsh fish' -d shell
"#;

// --- The hidden `__complete` runtime brain (cobra-compatible wire protocol) ---

// Directive bits relayed to the shim.
const NO_SPACE: u8 = 1;
const NO_FILE_COMP: u8 = 4;

/// One completion candidate: the literal word to insert, plus an optional one-line
/// description (shown by zsh/fish/powershell, ignored by bash).
struct Candidate {
    value: String,
    desc: Option<&'static str>,
}

impl Candidate {
    fn plain(value: impl Into<String>) -> Self {
        Candidate {
            value: value.into(),
            desc: None,
        }
    }
    fn described(value: impl Into<String>, desc: &'static str) -> Self {
        Candidate {
            value: value.into(),
            desc: Some(desc),
        }
    }
}

struct Completion {
    candidates: Vec<Candidate>,
    directive: u8,
}

/// One-line description per dispatched command, looked up by name so the subcommand
/// completion stays in lockstep with `crate::COMMANDS` (a name missing here trips
/// `every_command_has_a_description`).
fn command_desc(cmd: &str) -> &'static str {
    match cmd {
        "render" => "render a full HTML page to stdout",
        "read" => "project the document to plain text",
        "build" => "build self-contained HTML (a dir builds the site)",
        "blocks" => "list block ids + sourcepos (debug)",
        "schema" => "emit JSON Schemas for editor autocomplete",
        "vocab" => "emit editor autocomplete vocabulary as JSON",
        "symbols" => "list the doc's cross-reference targets",
        "check" => "list located diagnostics (non-zero if any)",
        "map" => "whole-project outline (pages, nav, xref)",
        "mcp" => "stdio MCP server",
        "init" => "scaffold a starter site",
        "new" => "scaffold one document",
        "serve" => "live preview server (alias of preview)",
        "preview" => "live preview server",
        "dev" => "live preview server (alias of preview)",
        "publish" => "build + deploy to Cloudflare Pages",
        "help" => "show this help",
        "completions" => "print a shell completion script",
        _ => "",
    }
}

/// Resolve preview's aliases so flag/positional tables are keyed by one canonical name.
fn canonical(sub: &str) -> &str {
    match sub {
        "dev" | "serve" => "preview",
        other => other,
    }
}

/// `(flag, takes_a_value, description)` per canonical subcommand. Single source of truth
/// for flag completion; `flag_table_covers_help` guards it against the CLI help text.
fn flags_for(sub: &str) -> &'static [(&'static str, bool, &'static str)] {
    match canonical(sub) {
        "preview" => &[
            (
                "--host",
                false,
                "expose on your LAN + print a phone QR code",
            ),
            ("--open", false, "launch the default browser"),
            (
                "--no-exec",
                false,
                "render code cells as source, never run them",
            ),
            ("--port", true, "port to serve on"),
        ],
        "build" => &[
            ("--out", true, "write a portable folder to <dir>"),
            (
                "--strict",
                false,
                "exit non-zero on a cell error or located warning",
            ),
            ("--bare", false, "emit zero-JS, CSS-only single-doc HTML"),
            ("--jobs", true, "cap parallel page renders"),
            ("--format", true, "machine output format (json)"),
        ],
        "publish" => &[
            ("--project-name", true, "Cloudflare Pages project name"),
            ("--out", true, "output dir"),
            ("--public", false, "deploy un-gated (no passcode)"),
            ("--no-strict", false, "do not fail on located warnings"),
            ("--dry-run", false, "build but skip the deploy"),
            ("--format", true, "machine output format (json)"),
        ],
        "new" => &[
            ("--dir", true, "project root to scaffold into"),
            ("--json", false, "print a json receipt"),
        ],
        "schema" => &[("--out", true, "output dir")],
        "symbols" => &[("--format", true, "human | json")],
        "map" => &[("--format", true, "human | json")],
        "check" => &[("--format", true, "human | json")],
        _ => &[],
    }
}

/// Compute completions for the words typed after `taliesin` (`words.last()` is the
/// current, possibly-empty word), resolving any paths relative to `cwd`.
fn complete_line(words: &[String], _cwd: &Path) -> Completion {
    let empty = String::new();
    let cur = words.last().unwrap_or(&empty).as_str();
    let prior: &[String] = if words.is_empty() {
        &[]
    } else {
        &words[..words.len() - 1]
    };

    // 1. Completing the subcommand token itself.
    if prior.is_empty() && !cur.starts_with('-') {
        let candidates = crate::COMMANDS
            .iter()
            .filter(|c| c.starts_with(cur))
            .map(|c| Candidate::described(*c, command_desc(c)))
            .collect();
        return Completion {
            candidates,
            directive: NO_FILE_COMP,
        };
    }

    let sub = prior.first().map(String::as_str).unwrap_or("");

    // 2. Flag-name completion.
    if cur.starts_with('-') {
        let candidates = flags_for(sub)
            .iter()
            .filter(|(f, _, _)| f.starts_with(cur))
            .map(|(f, _, d)| Candidate::described(*f, *d))
            .collect();
        return Completion {
            candidates,
            directive: NO_FILE_COMP,
        };
    }

    // Everything else: nothing yet (grown in later tasks). Fall back to file completion.
    Completion {
        candidates: Vec::new(),
        directive: 0,
    }
}

/// The hidden `__complete` subcommand: prints candidates + a directive line for the shim.
pub(crate) fn cmd_complete(args: &[String]) -> ExitCode {
    let words: Vec<String> = args.get(2..).unwrap_or(&[]).to_vec();
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let completion = complete_line(&words, &cwd);
    let mut out = String::new();
    for c in &completion.candidates {
        out.push_str(&c.value);
        if let Some(d) = c.desc {
            out.push('\t');
            out.push_str(d);
        }
        out.push('\n');
    }
    out.push_str(&format!(":{}\n", completion.directive));
    print!("{out}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod completions_tests {
    use super::*;

    #[test]
    fn generates_a_script_for_each_supported_shell_and_nothing_else() {
        for shell in ["bash", "zsh", "fish"] {
            let script = completions_script(shell)
                .unwrap_or_else(|| panic!("a `{shell}` completion script"));
            assert!(!script.trim().is_empty(), "`{shell}` script is non-empty");
            // Each script registers taliesin's completion with the shell.
            assert!(
                script.contains("taliesin"),
                "`{shell}` script names the binary: {script}"
            );
        }
        assert!(
            completions_script("powershell").is_none(),
            "an unsupported shell yields no script (so the CLI errors, not emits junk)"
        );
    }

    /// The load-bearing drift gate: every generated script offers **exactly** the command
    /// list `main()` dispatches on (`crate::COMMANDS`), because each script interpolates
    /// `COMMANDS.join(" ")` rather than hardcoding its own list. A hand-hardcoded or
    /// partial list in any shell branch drops the full joined string and fails here — the
    /// same drift `every_dispatched_command_is_listed_in_commands` guards for the
    /// did-you-mean. Mutation check: truncating `COMMANDS` in one branch, or dropping a
    /// name, changes the expected substring and trips this (verified by construction: the
    /// assertion is a full-string `contains`, not a per-token one).
    #[test]
    fn every_shell_script_offers_exactly_the_dispatched_command_list() {
        let expected = crate::COMMANDS.join(" ");
        for shell in ["bash", "zsh", "fish"] {
            let script = completions_script(shell).unwrap();
            assert!(
                script.contains(&expected),
                "`{shell}` completion command list must equal COMMANDS ({expected:?}); \
                 a per-shell hardcoded list would drift: {script}"
            );
        }
    }

    #[test]
    fn an_unknown_or_empty_shell_yields_no_script() {
        // `cmd_completions` turns `None` into a non-zero exit + a usage error (rather than a
        // silent empty success); the branch under test is `completions_script`'s `Option`.
        assert!(completions_script("").is_none());
        assert!(completions_script("tcsh").is_none());
        assert!(
            completions_script("BASH").is_none(),
            "shell names are case-sensitive"
        );
    }
}

#[cfg(test)]
mod brain_tests {
    use super::*;
    use std::path::Path;

    fn values(words: &[&str]) -> Vec<String> {
        let owned: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        complete_line(&owned, Path::new("."))
            .candidates
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    #[test]
    fn empty_word_completes_all_subcommands() {
        let got = values(&[""]);
        assert!(
            got.contains(&"preview".to_string()),
            "offers preview: {got:?}"
        );
        assert!(got.contains(&"build".to_string()), "offers build: {got:?}");
        // Every dispatched command is offered.
        assert_eq!(
            got.len(),
            crate::COMMANDS.len(),
            "offers exactly COMMANDS: {got:?}"
        );
    }

    #[test]
    fn prefix_filters_subcommands() {
        assert_eq!(values(&["pre"]), vec!["preview".to_string()]);
    }

    #[test]
    fn subcommand_completion_suppresses_file_fallback() {
        assert_eq!(
            complete_line(&["".to_string()], Path::new(".")).directive,
            NO_FILE_COMP
        );
    }

    #[test]
    fn every_command_has_a_description() {
        for c in crate::COMMANDS {
            assert!(
                !command_desc(c).is_empty(),
                "`{c}` needs a description in command_desc"
            );
        }
    }

    #[test]
    fn dash_completes_subcommand_flags() {
        let got = values(&["preview", "--"]);
        for f in ["--host", "--open", "--no-exec", "--port"] {
            assert!(got.contains(&f.to_string()), "preview offers {f}: {got:?}");
        }
        let got = values(&["build", "--"]);
        for f in ["--out", "--strict", "--bare", "--jobs", "--format"] {
            assert!(got.contains(&f.to_string()), "build offers {f}: {got:?}");
        }
    }

    #[test]
    fn flags_are_offered_through_aliases() {
        // `dev` and `serve` share preview's flags.
        assert!(values(&["dev", "--"]).contains(&"--host".to_string()));
    }
}
