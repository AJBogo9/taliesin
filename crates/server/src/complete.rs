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
            .map(|(f, _, d)| Candidate::described(*f, *d))
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
    if prior.len() == 1 && prior[0] == "completions" {
        return enumerated(cur, &["bash", "zsh", "fish", "powershell"]);
    }

    // 5. First path positional (only the first; later positionals fall through to the
    //    shell's own file completion).
    if let Some(kind) = positional_kind(sub) {
        if positionals_seen(sub, &prior[1..]) == 0 {
            return complete_paths(cur, cwd, &kind);
        }
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
    fn format_value_completion() {
        assert_eq!(values(&["build", "--format", ""]), vec!["json".to_string()]);
        let human_json: Vec<String> = ["human", "json"].into_iter().map(String::from).collect();
        assert_eq!(values(&["check", "--format", ""]), human_json);
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
}
