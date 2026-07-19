//! The interactive `new`/`init` wizard: `dialoguer` prompts shown only when a human runs an
//! under-specified command at a real terminal.
//!
//! **Load-bearing gate:** every prompt here is reached only through [`is_interactive`], which
//! is false whenever stdin is not a TTY (CI, a pipe, an agent) or the caller asked to skip
//! prompting (`-y`/`--yes`) or for machine output (`--json`). So the non-interactive surface is
//! exactly what it was before the wizard existed; the wizard is purely additive at a human TTY.
//!
//! This module is the only place that touches `dialoguer`; the callers stay pure by resolving
//! the missing pieces here and then running the same scaffold path as the flag-driven route.

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use std::io::{self, IsTerminal};

/// Whether to run the wizard: a human at a real terminal who did not pass `-y`/`--yes` (skip
/// prompting) or `--json` (machine output). False in CI, pipes, and agents, so those paths keep
/// the historical flag-driven behavior.
pub(crate) fn is_interactive(yes: bool, json: bool) -> bool {
    should_prompt(yes, json, io::stdin().is_terminal())
}

/// The pure gate behind [`is_interactive`], with the terminal check injected so it is
/// testable: prompt only when a real TTY is present and neither `-y`/`--yes` nor `--json`
/// asked to opt out.
fn should_prompt(yes: bool, json: bool, is_tty: bool) -> bool {
    !yes && !json && is_tty
}

/// Arrow-key pick from `items`, returning the chosen index (`default` is preselected). Only
/// called once [`is_interactive`] has confirmed a TTY.
pub(crate) fn select(prompt: &str, items: &[&str], default: usize) -> io::Result<usize> {
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(items)
        .default(default)
        .interact()
        .map_err(|e| io::Error::other(e.to_string()))
}

/// Free-text input with an optional default and a validator that re-prompts (rather than
/// aborting) on a rejected value, so a mistyped slug is corrected in place.
pub(crate) fn input<F>(prompt: &str, default: Option<&str>, validate: F) -> io::Result<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    let theme = ColorfulTheme::default();
    let mut builder = Input::<String>::with_theme(&theme).with_prompt(prompt);
    if let Some(d) = default {
        builder = builder.default(d.to_string());
    }
    builder
        .validate_with(move |s: &String| validate(s))
        .interact_text()
        .map_err(|e| io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::should_prompt;

    #[test]
    fn prompts_only_at_a_tty_without_yes_or_json() {
        assert!(
            should_prompt(false, false, true),
            "a human at a TTY is prompted"
        );
        assert!(
            !should_prompt(true, false, true),
            "-y opts out even at a TTY"
        );
        assert!(!should_prompt(false, true, true), "--json never prompts");
        assert!(
            !should_prompt(false, false, false),
            "no TTY (CI/pipe/agent) never prompts"
        );
    }
}
