//! Shell completion: the `completions <shell>` script generators and (added later) the
//! hidden `__complete` runtime brain they call. All completion logic lives here so
//! behavior cannot drift between shells: each shim only relays the brain's output.

use crate::log;
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
