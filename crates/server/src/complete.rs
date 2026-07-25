//! Shell completion: the `completions <shell>` script generators and (added later) the
//! hidden `__complete` runtime brain they call. All completion logic lives here so
//! behavior cannot drift between shells: each shim only relays the brain's output.

use crate::log;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `taliesin completions <bash|zsh|fish> [--install]`: without `--install`, print that
/// shell's completion script to stdout (a usage error + non-zero exit for a missing or
/// unsupported shell); with `--install`, write it into the shell's conventional completion
/// dir (see `install_completions`). The scripts are generated from `crate::COMMANDS`, so
/// the offered command list can never drift from what `main()` dispatches on.
pub(crate) fn cmd_completions(args: &[String]) -> ExitCode {
    let rest = args.get(2..).unwrap_or(&[]);
    let install = rest.iter().any(|a| a == "--install");
    // The shell, if named, is the first non-flag token (so `--install zsh` and
    // `zsh --install` are equivalent).
    let shell_arg = rest
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(String::as_str);
    if install {
        return install_completions(shell_arg);
    }
    match shell_arg.and_then(completions_script) {
        Some(script) => {
            print!("{script}");
            ExitCode::SUCCESS
        }
        None => {
            log::error("usage: taliesin completions <bash|zsh|fish|powershell> [--install]");
            ExitCode::FAILURE
        }
    }
}

/// Map a user-supplied or `$SHELL`-derived shell name to its canonical form, or `None` for
/// an unsupported shell. `pwsh` folds into `powershell` (they share one script).
fn canonical_shell(name: &str) -> Option<&'static str> {
    match name {
        "bash" => Some("bash"),
        "zsh" => Some("zsh"),
        "fish" => Some("fish"),
        "powershell" | "pwsh" => Some("powershell"),
        _ => None,
    }
}

/// The canonical shell behind a `$SHELL` value (`/usr/bin/zsh` -> `zsh`), or `None` when it
/// is unset or unrecognized (the caller then asks for an explicit `<shell> --install`).
fn detect_shell(shell_env: Option<&str>) -> Option<&'static str> {
    let base = shell_env?.rsplit(['/', '\\']).next()?;
    let base = base.strip_suffix(".exe").unwrap_or(base);
    canonical_shell(base)
}

/// The home + XDG dirs the install path is derived from. Injected (not read inline) so
/// `install_plan` is a pure function unit tests can drive without touching the environment.
struct InstallEnv {
    home: Option<String>,
    xdg_data: Option<String>,
    xdg_config: Option<String>,
}

impl InstallEnv {
    fn from_env() -> Self {
        let nonempty = |k| std::env::var(k).ok().filter(|s: &String| !s.is_empty());
        InstallEnv {
            home: nonempty("HOME"),
            xdg_data: nonempty("XDG_DATA_HOME"),
            xdg_config: nonempty("XDG_CONFIG_HOME"),
        }
    }
}

/// What `--install` should do for a shell: write the script to a path (bash/zsh/fish), or
/// hand back a manual command when there is no reliable auto path (powershell's `$PROFILE`
/// can't be resolved from outside PowerShell).
enum InstallPlan {
    Write {
        path: PathBuf,
        /// A follow-up the file write can't perform for the user (zsh's `fpath` edit).
        manual: Option<String>,
    },
    Manual {
        command: String,
    },
}

/// Where `shell`'s completion script installs, given the environment. `None` only when
/// `$HOME` is unresolvable (every path below needs it). XDG overrides are honored so the
/// target matches whatever `bash-completion`/`fish`/a framework actually reads.
fn install_plan(shell: &str, env: &InstallEnv) -> Option<InstallPlan> {
    let home = env.home.as_deref()?;
    let data = env
        .xdg_data
        .clone()
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let config = env
        .xdg_config
        .clone()
        .unwrap_or_else(|| format!("{home}/.config"));
    let plan = match shell {
        "bash" => InstallPlan::Write {
            path: PathBuf::from(format!("{data}/bash-completion/completions/taliesin")),
            manual: None,
        },
        "zsh" => {
            let dir = format!("{data}/zsh/site-functions");
            InstallPlan::Write {
                path: PathBuf::from(format!("{dir}/_taliesin")),
                manual: Some(format!(
                    "if completion doesn't appear, add `fpath+=({dir})` to ~/.zshrc before `compinit`"
                )),
            }
        }
        "fish" => InstallPlan::Write {
            path: PathBuf::from(format!("{config}/fish/completions/taliesin.fish")),
            manual: None,
        },
        "powershell" => InstallPlan::Manual {
            command: "taliesin completions powershell >> $PROFILE".to_string(),
        },
        _ => return None,
    };
    Some(plan)
}

/// `taliesin completions [<shell>] --install`: resolve the shell (explicit arg, else
/// `$SHELL`), then write the script to its conventional dir. Prints where it landed plus
/// any manual follow-up; exits non-zero only on a real failure (unknown shell, undetectable
/// shell, unresolvable `$HOME`, or an I/O error), never for the informational powershell case.
fn install_completions(shell_arg: Option<&str>) -> ExitCode {
    let shell = match shell_arg {
        Some(s) => match canonical_shell(s) {
            Some(c) => c,
            None => {
                log::error(&format!(
                    "unknown shell `{s}` (expected bash, zsh, fish, or powershell)"
                ));
                return ExitCode::FAILURE;
            }
        },
        None => match detect_shell(std::env::var("SHELL").ok().as_deref()) {
            Some(c) => c,
            None => {
                log::error(
                    "could not detect your shell from $SHELL; run e.g. `taliesin completions zsh --install`",
                );
                return ExitCode::FAILURE;
            }
        },
    };
    match install_plan(shell, &InstallEnv::from_env()) {
        Some(InstallPlan::Write { path, manual }) => {
            if let Some(parent) = path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                log::error(&format!("could not create {}: {e}", parent.display()));
                return ExitCode::FAILURE;
            }
            let script = completions_script(shell).expect("a canonical shell has a script");
            if let Err(e) = std::fs::write(&path, script) {
                log::error(&format!("could not write {}: {e}", path.display()));
                return ExitCode::FAILURE;
            }
            log::info(&format!(
                "installed {shell} completion -> {}",
                path.display()
            ));
            if let Some(m) = manual {
                log::info(&m);
            }
            log::info("restart your shell to pick it up");
            ExitCode::SUCCESS
        }
        Some(InstallPlan::Manual { command }) => {
            log::info(&format!(
                "{shell}: automatic install isn't supported; run:\n  {command}"
            ));
            ExitCode::SUCCESS
        }
        None => {
            log::error("could not resolve $HOME for the install location");
            ExitCode::FAILURE
        }
    }
}

/// The completion script for `shell`, or `None` for an unsupported one. Each script is a
/// thin shim that relays the command line to `taliesin __complete` and feeds the result
/// back to the shell, so the completion logic lives in one place (`complete_line`) and
/// cannot drift between shells.
pub(crate) fn completions_script(shell: &str) -> Option<String> {
    let script = match shell {
        "bash" => BASH_COMPLETIONS,
        "zsh" => ZSH_COMPLETIONS,
        "fish" => FISH_COMPLETIONS,
        "powershell" => POWERSHELL_COMPLETIONS,
        _ => return None,
    };
    Some(script.to_string())
}

const BASH_COMPLETIONS: &str = r#"# taliesin bash completion (dynamic).
# Install: taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin
_taliesin() {
    local IFS=$'\n' line directive=0
    local words=("${COMP_WORDS[@]:1:COMP_CWORD}")
    COMPREPLY=()
    for line in $(taliesin __complete "${words[@]}" 2>/dev/null); do
        if [[ $line == :* ]]; then
            directive=${line#:}
            continue
        fi
        COMPREPLY+=("${line%%$'\t'*}")
    done
    (( directive & 1 )) && compopt -o nospace 2>/dev/null
    (( directive & 4 )) || compopt -o default 2>/dev/null
}
complete -F _taliesin taliesin tali
"#;

const ZSH_COMPLETIONS: &str = r#"#compdef taliesin tali
# taliesin zsh completion (dynamic).
# Install (into a dir on $fpath, then run compinit):
#   taliesin completions zsh > "${fpath[1]}/_taliesin"
_taliesin() {
    local -a args
    args=("${(@)words[2,CURRENT]}")
    local out directive=0
    out="$(taliesin __complete "${args[@]}" 2>/dev/null)"
    local -a described plain
    local line val
    for line in ${(f)out}; do
        [[ $line == :* ]] && { directive=${line#:}; continue; }
        val=${line%%$'\t'*}
        [[ $line == *$'\t'* ]] && described+=("${val}:${line#*$'\t'}")
        plain+=("$val")
    done
    if (( directive & 1 )); then
        # Path completion: keep the trailing slash live so you can keep descending.
        (( ${#plain} )) && compadd -S '' -- "${plain[@]}"
    elif (( ${#described} == ${#plain} && ${#described} > 0 )); then
        _describe 'taliesin' described
    elif (( ${#plain} )); then
        compadd -- "${plain[@]}"
    elif (( (directive & 4) == 0 )); then
        _files
    fi
}
if [ "${funcstack[1]}" = "_taliesin" ]; then
    _taliesin "$@"
else
    compdef _taliesin taliesin tali
fi
"#;

const FISH_COMPLETIONS: &str = r#"# taliesin fish completion (dynamic).
# Install: taliesin completions fish > ~/.config/fish/completions/taliesin.fish
function __taliesin_complete
    set -l tokens (commandline -opc) (commandline -ct)
    set -l words $tokens[2..-1]
    taliesin __complete $words 2>/dev/null | while read -l line
        string match -q ':*' -- $line; and continue
        echo $line
    end
end
complete -c taliesin -f -a '(__taliesin_complete)'
complete -c tali -f -a '(__taliesin_complete)'
"#;

const POWERSHELL_COMPLETIONS: &str = r#"# taliesin PowerShell completion (dynamic).
# Install: taliesin completions powershell >> $PROFILE
Register-ArgumentCompleter -Native -CommandName @('taliesin','tali') -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $elements = $commandAst.CommandElements
    $words = @()
    for ($i = 1; $i -lt $elements.Count; $i++) { $words += $elements[$i].ToString() }
    if ($wordToComplete -eq '' -and ($words.Count -eq 0 -or $words[-1] -ne '')) { $words += '' }
    (& taliesin __complete @words 2>$null) | ForEach-Object {
        if ($_ -like ':*') { return }
        $parts = $_ -split "`t", 2
        $value = $parts[0]
        $desc = if ($parts.Count -gt 1) { $parts[1] } else { $parts[0] }
        [System.Management.Automation.CompletionResult]::new($value, $value, 'ParameterValue', $desc)
    }
}
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
        "doctor" => "audit the environment for running code cells",
        "map" => "whole-project outline (pages, nav, xref)",
        "skim" => "the book's skimmable layers as one linear stream",
        "mcp" => "stdio MCP server",
        "lsp" => "stdio LSP server (live diagnostics in any editor)",
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
            ("--json", false, "shorthand for --format json"),
        ],
        "publish" => &[
            ("--project-name", true, "Cloudflare Pages project name"),
            ("--out", true, "output dir"),
            ("--public", false, "deploy un-gated (no passcode)"),
            ("--no-strict", false, "do not fail on located warnings"),
            ("--dry-run", false, "build but skip the deploy"),
            ("--format", true, "machine output format (json)"),
            ("--json", false, "shorthand for --format json"),
        ],
        "new" => &[
            ("--dir", true, "project root to scaffold into"),
            (
                "--draft",
                false,
                "mark the scaffold draft: true (held out of the build)",
            ),
            (
                "--tour",
                false,
                "deck only: scaffold a guided, self-explaining deck",
            ),
            ("--json", false, "print a json receipt"),
            ("--format", true, "human | json (alias for --json)"),
            ("--yes", false, "skip the interactive prompt"),
        ],
        "schema" => &[("--out", true, "output dir")],
        "symbols" => &[
            ("--format", true, "human | json"),
            ("--json", false, "shorthand for --format json"),
        ],
        "map" => &[
            ("--format", true, "human | json"),
            ("--json", false, "shorthand for --format json"),
        ],
        "skim" => &[
            ("--format", true, "human | json"),
            ("--json", false, "shorthand for --format json"),
        ],
        "read" => &[
            ("--run", false, "execute cells + report produced output"),
            ("--format", true, "human | json"),
            ("--json", false, "shorthand for --format json"),
        ],
        "check" => &[
            ("--format", true, "human | json"),
            ("--json", false, "shorthand for --format json"),
            ("--explain", true, "explain a diagnostic code (TAL-...)"),
            (
                "--errors-only",
                false,
                "report + gate on errors, not warnings",
            ),
            (
                "--require-kernel",
                false,
                "also fail if a used language's kernel isn't ready",
            ),
            (
                "--stdin",
                false,
                "lint the buffer piped on stdin (unsaved edits)",
            ),
        ],
        "completions" => &[(
            "--install",
            false,
            "write the script into your shell's completion dir",
        )],
        "init" => &[
            ("--template", true, "starter: basic | site | book"),
            ("--json", false, "shorthand for --format json"),
            ("--format", true, "human | json"),
            ("--yes", false, "skip the interactive prompt"),
        ],
        _ => &[],
    }
}

/// A fixed set of candidate values (subcommand kinds, shells, format values), filtered by
/// the current prefix. The completion is authoritative, so file fallback is suppressed.
fn enumerated(cur: &str, values: &[&'static str]) -> Completion {
    let candidates = values
        .iter()
        .filter(|v| v.starts_with(cur))
        .map(|v| Candidate::plain(*v))
        .collect();
    Completion {
        candidates,
        directive: NO_FILE_COMP,
    }
}

/// Count the positional (non-flag, non-flag-value) args already sitting between the
/// subcommand and the cursor.
fn positionals_seen(sub: &str, rest: &[String]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < rest.len() {
        let tok = rest[i].as_str();
        if tok.starts_with('-') {
            // A value-taking flag (`--out dir`) also consumes the next token, unless it was
            // written `--out=dir`.
            if !tok.contains('=')
                && flags_for(sub)
                    .iter()
                    .any(|(f, takes, _)| *f == tok && *takes)
            {
                i += 1;
            }
        } else {
            n += 1;
        }
        i += 1;
    }
    n
}

const IGNORE_DIRS: &[&str] = &[".git", "target", "node_modules", "_site", "_freeze"];
const TMD_WALK_DEPTH: usize = 6;

/// Whether a subcommand's first positional path may be a `.tmd` file, a directory, or
/// either.
enum PathKind {
    File,
    FileOrDir,
    Dir,
}

impl PathKind {
    /// Whether `.tmd` files are offered as terminal candidates. Directories are always
    /// offered regardless, so a nested target stays reachable by descending.
    fn offers_files(&self) -> bool {
        matches!(self, PathKind::File | PathKind::FileOrDir)
    }
}

/// The first positional's path type, or `None` when the subcommand takes no path.
fn positional_kind(sub: &str) -> Option<PathKind> {
    match canonical(sub) {
        "preview" | "build" | "check" => Some(PathKind::FileOrDir),
        "render" | "read" | "blocks" | "symbols" => Some(PathKind::File),
        "map" | "publish" | "init" => Some(PathKind::Dir),
        _ => None,
    }
}

/// Complete a path word: `.tmd` files (when `kind.offers_files()`) plus directories that
/// contain a `.tmd` anywhere below, site/book roots first, dirs suffixed `/`.
fn complete_paths(cur: &str, cwd: &Path, kind: &PathKind) -> Completion {
    // Split into (dir_part incl. trailing slash, leaf prefix).
    let (dir_part, leaf) = match cur.rfind('/') {
        Some(i) => (&cur[..=i], &cur[i + 1..]),
        None => ("", cur),
    };
    let base = cwd.join(dir_part);
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Completion {
            candidates: Vec::new(),
            directive: NO_FILE_COMP,
        };
    };

    let mut site_dirs: Vec<Candidate> = Vec::new();
    let mut dirs: Vec<Candidate> = Vec::new();
    let mut files: Vec<Candidate> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(leaf) {
            continue;
        }
        // Hide dotfiles unless the user is explicitly typing a dot prefix.
        if name.starts_with('.') && !leaf.starts_with('.') {
            continue;
        }
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_dir() {
            if IGNORE_DIRS.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            if !dir_contains_tmd(&path, TMD_WALK_DEPTH) {
                continue;
            }
            let value = format!("{dir_part}{name}/");
            if path.join("_site.yml").exists() {
                site_dirs.push(Candidate::described(value, "site / book root"));
            } else {
                dirs.push(Candidate::plain(value));
            }
        } else if kind.offers_files() && ty.is_file() && name.ends_with(".tmd") {
            files.push(Candidate::plain(format!("{dir_part}{name}")));
        }
    }

    site_dirs.sort_by(|a, b| a.value.cmp(&b.value));
    dirs.sort_by(|a, b| a.value.cmp(&b.value));
    files.sort_by(|a, b| a.value.cmp(&b.value));

    let mut candidates = site_dirs;
    candidates.append(&mut dirs);
    candidates.append(&mut files);

    let mut directive = NO_FILE_COMP;
    if candidates.iter().any(|c| c.value.ends_with('/')) {
        directive |= NO_SPACE;
    }
    Completion {
        candidates,
        directive,
    }
}

/// True if `dir` holds a `.tmd` file within `depth` levels, skipping the build/vcs dirs in
/// `IGNORE_DIRS` and dot-dirs. Short-circuits on the first hit. `file_type()` does not
/// follow symlinks, so symlinked dirs are skipped and cannot cause a cycle.
fn dir_contains_tmd(dir: &Path, depth: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let Ok(ty) = entry.file_type() else { continue };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ty.is_file() {
            if name.ends_with(".tmd") {
                return true;
            }
        } else if ty.is_dir()
            && depth > 0
            && !name.starts_with('.')
            && !IGNORE_DIRS.contains(&name.as_ref())
        {
            subdirs.push(entry.path());
        }
    }
    subdirs.into_iter().any(|d| dir_contains_tmd(&d, depth - 1))
}

/// Compute completions for the words typed after `taliesin` (`words.last()` is the
/// current, possibly-empty word), resolving any paths relative to `cwd`.
fn complete_line(words: &[String], cwd: &Path) -> Completion {
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
            .map(|(f, _, d)| Candidate::described(*f, d))
            .collect();
        return Completion {
            candidates,
            directive: NO_FILE_COMP,
        };
    }

    // 3. Value of the flag immediately before the cursor.
    if let Some(prev) = prior.last() {
        if prev.as_str() == "--format" {
            let vals: &[&str] = if canonical(sub) == "build" {
                &["json"]
            } else {
                &["human", "json"]
            };
            return enumerated(cur, vals);
        }
        // `check --explain <TAB>` offers the closed, drift-locked set of diagnostic codes.
        if prev.as_str() == "--explain" && canonical(sub) == "check" {
            let codes = taliesin_core::diagnostics::codes::all_codes();
            return enumerated(cur, &codes);
        }
        // `init --template <TAB>` offers the three starters.
        if prev.as_str() == "--template" && canonical(sub) == "init" {
            return enumerated(cur, &["basic", "site", "book"]);
        }
        // Other value-taking flags (--out/--dir/--jobs/--port/--project-name): let the
        // shell complete the value (a dir, a number, a name); nothing smart to add.
        if flags_for(sub)
            .iter()
            .any(|(f, takes, _)| *f == prev.as_str() && *takes)
        {
            return Completion {
                candidates: Vec::new(),
                directive: 0,
            };
        }
    }

    // 4. Enumerated first positionals.
    if prior.len() == 1 && prior[0] == "new" {
        return enumerated(cur, &["post", "page", "deck", "paper"]);
    }
    // `completions` offers the shell kinds until one is chosen, so it also fires after
    // `completions --install` (where an interleaved flag pushed prior.len() past 1).
    if canonical(sub) == "completions"
        && !cur.starts_with('-')
        && positionals_seen("completions", &prior[1..]) == 0
    {
        return enumerated(cur, &["bash", "zsh", "fish", "powershell"]);
    }

    // 5. First path positional (only the first; later positionals fall through to the
    //    shell's own file completion).
    if let Some(kind) = positional_kind(sub)
        && positionals_seen(sub, &prior[1..]) == 0
    {
        return complete_paths(cur, cwd, &kind);
    }

    // 6. Nothing special: let the shell do its normal file completion.
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
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let script = completions_script(shell)
                .unwrap_or_else(|| panic!("a `{shell}` completion script"));
            assert!(!script.trim().is_empty(), "`{shell}` script is non-empty");
            assert!(
                script.contains("taliesin"),
                "`{shell}` names the binary: {script}"
            );
            // Every shim delegates to the one brain rather than hardcoding logic.
            assert!(
                script.contains("__complete"),
                "`{shell}` calls __complete: {script}"
            );
            // Both the canonical name and the `tali` alias get completion. Use a per-shell
            // marker: a bare `contains("tali")` is vacuously true (it is inside "taliesin").
            let alias_marker = match shell {
                "bash" | "zsh" => "taliesin tali",
                "fish" => "-c tali",
                "powershell" => "'tali'",
                _ => unreachable!(),
            };
            assert!(
                script.contains(alias_marker),
                "`{shell}` registers the tali alias ({alias_marker:?}): {script}"
            );
        }
        assert!(
            completions_script("tcsh").is_none(),
            "an unsupported shell yields no script (so the CLI errors, not emits junk)"
        );
    }

    #[test]
    fn an_unknown_or_empty_shell_yields_no_script() {
        // `cmd_completions` turns `None` into a non-zero exit + a usage error (rather than a
        // silent empty success); the branch under test is `completions_script`'s `Option`.
        assert!(completions_script("").is_none());
        assert!(completions_script("elvish").is_none());
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

    #[test]
    fn enumerated_positionals() {
        assert_eq!(
            values(&["new", ""]),
            ["post", "page", "deck", "paper"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            values(&["completions", ""]),
            ["bash", "zsh", "fish", "powershell"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn completions_offers_install_flag_and_still_offers_shells() {
        // `completions --<TAB>` offers the install flag.
        assert!(values(&["completions", "--"]).contains(&"--install".to_string()));
        // `completions --install <TAB>` still offers the shell kinds (the flag interleaves
        // ahead of the shell positional, so a naive prior.len()==1 check would miss it).
        assert_eq!(
            values(&["completions", "--install", ""]),
            ["bash", "zsh", "fish", "powershell"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        // Once a shell positional is present, shells are no longer offered.
        assert!(values(&["completions", "bash", ""]).is_empty());
    }

    #[test]
    fn format_value_completion() {
        assert_eq!(values(&["build", "--format", ""]), vec!["json".to_string()]);
        let human_json: Vec<String> = ["human", "json"].into_iter().map(String::from).collect();
        assert_eq!(values(&["check", "--format", ""]), human_json);
    }

    #[test]
    fn explain_value_completes_to_the_code_set() {
        // `check --explain <TAB>` offers the drift-locked diagnostic-code vocabulary; a
        // prefix filters it (`TAL-A` -> the a11y codes).
        let all: Vec<String> = taliesin_core::diagnostics::codes::all_codes()
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(values(&["check", "--explain", ""]), all);
        let a11y = values(&["check", "--explain", "TAL-A11Y-"]);
        assert!(
            a11y.iter().all(|c| c.starts_with("TAL-A11Y-")) && !a11y.is_empty(),
            "prefix filters to the a11y family: {a11y:?}"
        );
    }

    fn fixture(tag: &str) -> std::path::PathBuf {
        use std::fs;
        let dir =
            std::env::temp_dir().join(format!("tali-complete-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("site")).unwrap();
        fs::create_dir_all(dir.join("nested/deep")).unwrap();
        fs::create_dir_all(dir.join("empty")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("index.tmd"), "# hi\n").unwrap();
        fs::write(dir.join("site/_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("site/page.tmd"), "# p\n").unwrap();
        fs::write(dir.join("nested/deep/buried.tmd"), "# b\n").unwrap();
        fs::write(dir.join("target/decoy.tmd"), "# decoy\n").unwrap();
        dir
    }

    fn path_values(dir: &std::path::Path, words: &[&str]) -> Vec<String> {
        let owned: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        complete_line(&owned, dir)
            .candidates
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    #[test]
    fn path_completion_filters_and_orders() {
        let dir = fixture("filter");
        let got = path_values(&dir, &["preview", ""]);
        assert!(
            got.contains(&"index.tmd".to_string()),
            "offers .tmd file: {got:?}"
        );
        assert!(
            got.contains(&"site/".to_string()),
            "offers site root: {got:?}"
        );
        assert!(
            got.contains(&"nested/".to_string()),
            "offers dir with a buried .tmd: {got:?}"
        );
        assert!(
            !got.iter().any(|v| v.starts_with("empty")),
            "hides .tmd-free dir: {got:?}"
        );
        assert!(
            !got.iter().any(|v| v.starts_with("target")),
            "hides ignore-set dir: {got:?}"
        );
        // Site/book root is ordered before plain dirs and files.
        let site = got.iter().position(|v| v == "site/").unwrap();
        let nested = got.iter().position(|v| v == "nested/").unwrap();
        assert!(site < nested, "site root first: {got:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dir_only_subcommands_hide_tmd_files() {
        let dir = fixture("dironly");
        let got = path_values(&dir, &["map", ""]);
        assert!(
            !got.contains(&"index.tmd".to_string()),
            "map offers no .tmd file: {got:?}"
        );
        assert!(
            got.contains(&"site/".to_string()),
            "map still offers a site dir: {got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_directive_sets_nospace_when_dirs_present() {
        let dir = fixture("directive");
        let d = complete_line(&["preview".to_string(), "".to_string()], &dir).directive;
        assert_eq!(
            d,
            NO_SPACE | NO_FILE_COMP,
            "dirs present => NoSpace|NoFileComp"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flag_table_covers_help() {
        // Collect every `--flag` mentioned anywhere in main.rs help text, then assert each
        // lives in some subcommand's `flags_for` table (mirrors the ENV_HELP drift gate).
        let src = include_str!("main.rs");
        let bytes = src.as_bytes();
        let mut mentioned = std::collections::BTreeSet::new();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'-' && bytes[i + 1] == b'-' {
                let start = i;
                let mut j = i + 2;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                    j += 1;
                }
                let tok = &src[start..j];
                // Flags that appear in help but are NOT taliesin subcommand flags: the
                // global --help/--version, and flags inside external-tool command examples
                // (wrangler's `--production-branch` in the publish setup hint).
                const NON_TABLE_FLAGS: &[&str] = &["--help", "--version", "--production-branch"];
                if tok.len() > 2 && !NON_TABLE_FLAGS.contains(&tok) {
                    mentioned.insert(tok.to_string());
                }
                i = j;
            } else {
                i += 1;
            }
        }
        let known: std::collections::BTreeSet<&str> = crate::COMMANDS
            .iter()
            .flat_map(|c| flags_for(c).iter().map(|(f, _, _)| *f))
            .collect();
        for flag in &mentioned {
            assert!(
                known.contains(flag.as_str()),
                "`{flag}` appears in help but is missing from flags_for (add it or the table drifts)"
            );
        }
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;

    fn env(home: Option<&str>, data: Option<&str>, config: Option<&str>) -> InstallEnv {
        InstallEnv {
            home: home.map(String::from),
            xdg_data: data.map(String::from),
            xdg_config: config.map(String::from),
        }
    }

    /// The write-path of a `Write` plan, or `None` for a `Manual`/absent plan.
    fn write_path(shell: &str, e: &InstallEnv) -> Option<PathBuf> {
        match install_plan(shell, e)? {
            InstallPlan::Write { path, .. } => Some(path),
            InstallPlan::Manual { .. } => None,
        }
    }

    #[test]
    fn canonical_shell_folds_pwsh_and_rejects_unknown() {
        assert_eq!(canonical_shell("zsh"), Some("zsh"));
        assert_eq!(canonical_shell("pwsh"), Some("powershell"));
        assert_eq!(canonical_shell("powershell"), Some("powershell"));
        assert_eq!(canonical_shell("tcsh"), None);
        assert_eq!(canonical_shell(""), None);
    }

    #[test]
    fn detect_shell_reads_the_basename_of_shell() {
        assert_eq!(detect_shell(Some("/usr/bin/zsh")), Some("zsh"));
        assert_eq!(detect_shell(Some("/bin/bash")), Some("bash"));
        assert_eq!(detect_shell(Some("fish")), Some("fish"));
        assert_eq!(
            detect_shell(Some(r"C:\Program Files\PowerShell\pwsh.exe")),
            Some("powershell")
        );
        assert_eq!(detect_shell(Some("/usr/bin/tcsh")), None);
        assert_eq!(detect_shell(None), None);
    }

    #[test]
    fn install_plan_uses_xdg_default_paths() {
        let e = env(Some("/home/u"), None, None);
        assert_eq!(
            write_path("bash", &e).unwrap(),
            PathBuf::from("/home/u/.local/share/bash-completion/completions/taliesin")
        );
        assert_eq!(
            write_path("zsh", &e).unwrap(),
            PathBuf::from("/home/u/.local/share/zsh/site-functions/_taliesin")
        );
        assert_eq!(
            write_path("fish", &e).unwrap(),
            PathBuf::from("/home/u/.config/fish/completions/taliesin.fish")
        );
    }

    #[test]
    fn install_plan_honors_xdg_overrides() {
        let e = env(Some("/home/u"), Some("/data"), Some("/cfg"));
        assert_eq!(
            write_path("bash", &e).unwrap(),
            PathBuf::from("/data/bash-completion/completions/taliesin")
        );
        assert_eq!(
            write_path("fish", &e).unwrap(),
            PathBuf::from("/cfg/fish/completions/taliesin.fish")
        );
    }

    #[test]
    fn zsh_carries_a_manual_fpath_hint_others_do_not() {
        let e = env(Some("/home/u"), None, None);
        match install_plan("zsh", &e).unwrap() {
            InstallPlan::Write { manual, .. } => {
                assert!(manual.unwrap().contains("fpath"), "zsh hints at fpath")
            }
            InstallPlan::Manual { .. } => panic!("zsh writes a file"),
        }
        match install_plan("bash", &e).unwrap() {
            InstallPlan::Write { manual, .. } => assert!(manual.is_none(), "bash auto-loads"),
            InstallPlan::Manual { .. } => panic!("bash writes a file"),
        }
    }

    #[test]
    fn powershell_is_manual_only() {
        let e = env(Some("/home/u"), None, None);
        match install_plan("powershell", &e).unwrap() {
            InstallPlan::Manual { command } => assert!(command.contains("$PROFILE")),
            InstallPlan::Write { .. } => panic!("$PROFILE can't be resolved from outside pwsh"),
        }
    }

    #[test]
    fn install_plan_needs_a_home() {
        // Every install path is HOME-relative, so without it there is no plan.
        assert!(install_plan("bash", &env(None, Some("/data"), None)).is_none());
    }
}
