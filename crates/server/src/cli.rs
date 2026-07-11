//! Front-door subcommands: `init` (scaffold a starter site) and `serve`/`preview`/`dev`
//! (launch the live preview server).
//!
//! **What:** `init` writes a minimal previewable site (`_site.yml` + `index.tmd`);
//! `serve` parses the preview flags (`--open`/`--host`/`--no-exec`/port) and dispatches
//! to the single-doc or multi-page server.
//!
//! **How to use:** `main()` dispatches `init` and `serve`/`preview`/`dev` to `cmd_init` /
//! `cmd_serve` here.
//!
//! **Depends on:** [`crate::serve`] + [`crate::serve_site`] (the two server entry points)
//! and [`crate::log`].

use crate::{log, serve, serve_site};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `_site.yml` for the scaffold: the minimal flat-native config (just a title).
const INIT_SITE_YML: &str = "title: My site\n";

/// `index.tmd` for the scaffold: a hello-world page that previews immediately and
/// points the new user at the next steps. `.tmd` is the native extension.
const INIT_INDEX_TMD: &str = "---\ntitle: Hello, Taliesin\n---\n\n\
    Welcome to your new [Taliesin](https://github.com/AJBogo9/taliesin) site.\n\n\
    Edit `index.tmd` and the preview reloads as you save.\n\n\
    ## Next steps\n\n\
    - Add more `.tmd` pages beside this one: each becomes its own page.\n\
    - Configure navigation and the title in `_site.yml`.\n\
    - Drop in a `{python}` or `{r}` code cell to run live output.\n";

/// `taliesin init [dir]`: scaffold a minimal previewable site into `dir` (default the
/// current directory). Writes `_site.yml` + `index.tmd`, then prints the preview hint.
pub(crate) fn cmd_init(dir: Option<&str>) -> ExitCode {
    let dir = Path::new(dir.unwrap_or("."));
    match scaffold_init(dir) {
        Ok(written) => {
            for f in &written {
                log::built(&f.display().to_string());
            }
            let where_ = if dir == Path::new(".") {
                ".".to_string()
            } else {
                dir.display().to_string()
            };
            println!("Scaffolded a Taliesin site. Preview it:\n  taliesin preview {where_}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// Write the starter files (`_site.yml`, `index.tmd`) into `dir`, creating it if
/// needed. Refuses to overwrite an existing file (so re-running `init` never clobbers
/// the user's work) and returns the paths written, or a human-readable error.
fn scaffold_init(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Err(format!("cannot create {}: {e}", dir.display()));
    }
    let files = [("_site.yml", INIT_SITE_YML), ("index.tmd", INIT_INDEX_TMD)];
    // Refuse to overwrite *any* target before writing *any*, so a partial scaffold
    // never lands on top of an existing project.
    for (name, _) in files {
        let path = dir.join(name);
        if path.exists() {
            return Err(format!(
                "{} already exists; refusing to overwrite (run `init` in an empty directory)",
                path.display()
            ));
        }
    }
    let mut written = Vec::new();
    for (name, contents) in files {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, contents) {
            return Err(format!("cannot write {}: {e}", path.display()));
        }
        written.push(path);
    }
    Ok(written)
}

/// What `taliesin new` can scaffold. Each maps to one file and one front-matter shape.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum NewKind {
    Post,
    Page,
    Deck,
}

/// The kind names, for the unknown-kind did-you-mean.
const NEW_KINDS: &[&str] = &["post", "page", "deck"];

impl NewKind {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "post" => Ok(Self::Post),
            "page" => Ok(Self::Page),
            "deck" => Ok(Self::Deck),
            other => Err(match taliesin_core::closest(other, NEW_KINDS) {
                Some(k) => format!("unknown kind `{other}` (did you mean `{k}`?)"),
                None => format!("unknown kind `{other}` (expected post, page, or deck)"),
            }),
        }
    }
}

/// A slug names a file inside the project, so it may not climb out of it or reach into a
/// subdirectory. Kept to the characters a URL wants anyway, which is what a page's path
/// becomes.
fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("the slug is empty (try `taliesin new post my-first-post`)".to_string());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!(
            "invalid slug `{slug}`: use lowercase letters, digits and hyphens \
             (it becomes the page's URL)"
        ));
    }
    Ok(())
}

/// `my-first-post` -> `My First Post`, the title an author would have typed anyway.
fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Today's date as `YYYY-MM-DD`, **UTC**. Taliesin has no date dependency and does not
/// want one for this (see the backlog's library-outsourcing ruling), so the civil date is
/// derived from the Unix day number directly. Near midnight this can name yesterday or
/// tomorrow in the author's local zone; the date is front matter they can edit.
fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 -> `(year, month, day)`. Howard Hinnant's `civil_from_days`,
/// exact for every date this program can see. Unit-tested against known days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The files a `taliesin new <kind> <slug>` writes, as `(project-relative path, contents)`.
///
/// Pure, so the corpus pin can compare the bytes exactly (`corpus/scaffold/`) and the CLI
/// can stay a thin wrapper. Every front-matter key here is one the validator knows; a
/// `check`-clean scaffold is asserted by `crates/server/tests/new_cli.rs`, and the emitted
/// documents are rendered and linted by the corpus regression net like any other document.
pub(crate) fn new_files(kind: NewKind, slug: &str, today: &str) -> Vec<(PathBuf, String)> {
    let title = title_from_slug(slug);
    let (path, body) = match kind {
        NewKind::Post => (
            PathBuf::from("posts").join(slug).join("index.tmd"),
            format!(
                "---\n\
                 title: \"{title}\"\n\
                 date: {today}\n\
                 description: \"One sentence: what a reader will understand by the end.\"\n\
                 categories: [writing]\n\
                 ---\n\
                 \n\
                 Open with the question this post answers.\n\
                 \n\
                 ## The first idea\n\
                 \n\
                 Save the file and the preview re-renders only the block you changed.\n"
            ),
        ),
        NewKind::Page => (
            PathBuf::from(format!("{slug}.tmd")),
            format!(
                "---\n\
                 title: \"{title}\"\n\
                 ---\n\
                 \n\
                 Save the file and the preview re-renders only the block you changed.\n"
            ),
        ),
        NewKind::Deck => (
            PathBuf::from(format!("{slug}.tmd")),
            format!(
                "---\n\
                 title: \"{title}\"\n\
                 subtitle: \"A subtitle\"\n\
                 format: deck\n\
                 ---\n\
                 \n\
                 ## The first slide\n\
                 \n\
                 - A point worth making\n\
                 - Another one\n\
                 \n\
                 ## The second slide\n\
                 \n\
                 Each `##` heading starts a new slide.\n"
            ),
        ),
    };
    vec![(path, body)]
}

/// `taliesin new <post|page|deck> <slug> [--dir <root>]`: scaffold one document, correct
/// on its first save. Refuses to overwrite, exactly as `init` does.
pub(crate) fn cmd_new(args: &[String]) -> ExitCode {
    let mut positional: Vec<&str> = Vec::new();
    let mut root = ".".to_string();
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" | "--out" => {
                if let Some(v) = it.next() {
                    root = v.clone();
                }
            }
            s if s.starts_with("--") => {
                log::error(&serve::unknown_flag_error(s, NEW_FLAGS));
                return ExitCode::FAILURE;
            }
            s => positional.push(s),
        }
    }
    let (Some(kind), Some(slug)) = (positional.first(), positional.get(1)) else {
        eprintln!("usage: taliesin new <post|page|deck> <slug> [--dir <root>]");
        return ExitCode::FAILURE;
    };
    let kind = match NewKind::parse(kind) {
        Ok(k) => k,
        Err(e) => {
            log::error(&e);
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = validate_slug(slug) {
        log::error(&e);
        return ExitCode::FAILURE;
    }
    let root = Path::new(&root);
    match write_new(root, kind, slug) {
        Ok(written) => {
            for f in &written {
                log::built(&f.display().to_string());
            }
            println!("Preview it:\n  taliesin preview {}", written[0].display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// Every long flag `new` accepts (drives the unknown-flag did-you-mean).
const NEW_FLAGS: &[&str] = &["--dir"];

/// Write the scaffold under `root`, refusing to overwrite any existing target before
/// writing any of them (so a partial scaffold never lands on the author's work).
fn write_new(root: &Path, kind: NewKind, slug: &str) -> Result<Vec<PathBuf>, String> {
    let files = new_files(kind, slug, &today_utc());
    for (rel, _) in &files {
        let path = root.join(rel);
        if path.exists() {
            return Err(format!(
                "{} already exists; refusing to overwrite",
                path.display()
            ));
        }
    }
    let mut written = Vec::new();
    for (rel, contents) in &files {
        let path = root.join(rel);
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!("cannot create {}: {e}", parent.display()));
        }
        if let Err(e) = std::fs::write(&path, contents) {
            return Err(format!("cannot write {}: {e}", path.display()));
        }
        written.push(path);
    }
    Ok(written)
}

/// Parse the optional `[port]` positional: absent -> the 4321 default; a present but
/// unparseable value is an error (not a silent fall-back to the default). Pure/unit-tested.
fn parse_port(raw: Option<&str>) -> Result<u16, String> {
    match raw {
        None => Ok(4321),
        Some(p) => p
            .parse()
            .map_err(|_| format!("invalid port: `{p}` (expected 0-65535)")),
    }
}

/// Every long flag `preview`/`serve`/`dev` accepts (drives the unknown-flag did-you-mean).
/// `--help`/`-h` are intercepted by `main()` before this parser runs, so they aren't here.
const SERVE_FLAGS: &[&str] = &["--open", "--host", "--no-exec", "--port"];

/// What `preview`/`serve`/`dev` parsed out of argv, before any environment or IO.
#[derive(Debug, PartialEq)]
pub(crate) struct ServeArgs<'a> {
    pub path: &'a str,
    pub port: u16,
    pub open: bool,
    pub expose: bool,
    pub no_exec: bool,
}

/// Parse `preview <file.tmd|dir> [port] [--port <N>] [--host] [--open] [--no-exec]`.
///
/// The port may be the second positional (the original spelling) or `--port <N>` /
/// `--port=<N>`. Without the flag, `--port 4400` tripped the unknown-flag did-you-mean and
/// suggested `--host`, which is two edits away and does something else entirely.
/// Pure + unit-tested: no environment reads, no filesystem.
pub(crate) fn parse_serve_args(args: &[String]) -> Result<ServeArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut flag_port: Option<&str> = None;
    let (mut open, mut expose, mut no_exec) = (false, false, false);

    let mut it = args[2..].iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--open" => open = true,
            "--host" => expose = true,
            "--no-exec" => no_exec = true,
            "--port" => {
                flag_port = Some(
                    it.next()
                        .map(String::as_str)
                        .ok_or_else(|| "--port needs a value (e.g. --port 4400)".to_string())?,
                );
            }
            s if s.starts_with("--port=") => flag_port = Some(&s["--port=".len()..]),
            // An unrecognized `--flag` is a hard error with a did-you-mean (never silently
            // dropped: a typo'd `--hots` would otherwise preview without exposing).
            s if s.starts_with("--") => return Err(serve::unknown_flag_error(s, SERVE_FLAGS)),
            s => positionals.push(s),
        }
    }

    let path = *positionals.first().ok_or_else(|| {
        "usage: taliesin preview <file.tmd|dir> [port] [--port <N>] [--host] [--open] [--no-exec]"
            .to_string()
    })?;
    // `--port` wins over the positional when both are given (the explicit flag is the
    // more deliberate spelling); a present-but-unparseable value is always an error.
    let port = parse_port(flag_port.or_else(|| positionals.get(1).copied()))?;
    Ok(ServeArgs {
        path,
        port,
        open,
        expose,
        no_exec,
    })
}

pub(crate) fn cmd_serve(args: &[String]) -> ExitCode {
    let parsed = match parse_serve_args(args) {
        Ok(p) => p,
        Err(msg) if msg.starts_with("usage:") => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
        Err(msg) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
    };
    let open = parsed.open || std::env::var_os("TALIESIN_OPEN").is_some();
    let expose = parsed.expose || std::env::var_os("TALIESIN_HOST").is_some();
    // `--no-exec` is sugar for `TALIESIN_NO_EXEC=1`, which `exec::Executor` reads:
    // preview a document you don't trust without running its code cells.
    if parsed.no_exec {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel
        // threads spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("TALIESIN_NO_EXEC", "1") };
    }
    // A directory is a multi-page site project; a single `.tmd` is one document.
    let result = if Path::new(parsed.path).is_dir() {
        serve_site::run(PathBuf::from(parsed.path), parsed.port, open, expose)
    } else {
        serve::run(PathBuf::from(parsed.path), parsed.port, open, expose)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
        }
    }
}

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
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preview_port_defaults_parses_and_rejects() {
        // No port positional -> the 4321 default; a valid number parses; a present-but-
        // unparseable value is an error (not a silent fall-back); > u16::MAX is rejected.
        assert_eq!(parse_port(None).unwrap(), 4321);
        assert_eq!(parse_port(Some("8080")).unwrap(), 8080);
        assert_eq!(parse_port(Some("0")).unwrap(), 0);
        assert!(parse_port(Some("not-a-port")).is_err());
        assert!(parse_port(Some("70000")).is_err());
    }

    fn argv(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn serve_accepts_port_as_a_flag_or_a_positional() {
        // The original spelling: the second positional.
        let a = argv(&["taliesin", "preview", "doc.tmd", "4400"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4400);

        // `--port 4400` used to error with "unknown flag `--port` (did you mean `--host`?)".
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port", "4400"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4400);

        // `--port=4400` too, and the flag may precede the path.
        let a = argv(&["taliesin", "preview", "--port=4400", "doc.tmd"]);
        let p = parse_serve_args(&a).unwrap();
        assert_eq!((p.port, p.path), (4400, "doc.tmd"));

        // Default when neither is given.
        let a = argv(&["taliesin", "preview", "doc.tmd"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4321);

        // The explicit flag wins over the positional.
        let a = argv(&["taliesin", "preview", "doc.tmd", "1111", "--port", "2222"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 2222);
    }

    #[test]
    fn serve_flag_errors_stay_loud() {
        // A bad port value is an error, never a silent fall-back to the default.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port", "not-a-port"]);
        assert!(parse_serve_args(&a).unwrap_err().contains("invalid port"));

        // `--port` with nothing after it names the fix.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port"]);
        assert!(parse_serve_args(&a).unwrap_err().contains("needs a value"));

        // An unknown flag still gets a did-you-mean, and `--prot` now resolves to
        // `--port` rather than to the unrelated `--host`.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--prot", "4400"]);
        let err = parse_serve_args(&a).unwrap_err();
        assert!(err.contains("--prot"), "{err}");
        assert!(err.contains("--port"), "{err}");

        // Flags are not swallowed as positionals.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--host", "--open"]);
        let p = parse_serve_args(&a).unwrap();
        assert!(p.expose && p.open && p.port == 4321);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-init-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn init_scaffolds_a_previewable_site() {
        let dir = tmp("scaffold");
        // The dir doesn't exist yet — `scaffold_init` must create it.
        let written = scaffold_init(&dir).expect("scaffold succeeds into a fresh dir");

        let site_yml = dir.join("_site.yml");
        let index = dir.join("index.tmd");
        assert!(site_yml.exists(), "_site.yml written");
        assert!(index.exists(), "index.tmd written");
        assert_eq!(written, vec![site_yml.clone(), index.clone()]);

        // The scaffold is a real, parseable site whose one page previews.
        let cfg = fs::read_to_string(&site_yml).unwrap();
        assert!(cfg.contains("title:"), "config has a title: {cfg}");
        let page = fs::read_to_string(&index).unwrap();
        assert!(
            page.starts_with("---") && page.contains("title:"),
            "index has front matter: {page}"
        );

        // Re-running refuses to overwrite (never clobbers existing work).
        let err = scaffold_init(&dir).expect_err("second init refuses to overwrite");
        assert!(err.contains("already exists"), "overwrite refused: {err}");

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod new_tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1)); // the epoch
        assert_eq!(civil_from_days(-1), (1969, 12, 31)); // before it
        assert_eq!(civil_from_days(59), (1970, 3, 1)); // 1970 is not a leap year
        assert_eq!(civil_from_days(11_016), (2000, 2, 29)); // 2000 is (a 400-year leap)
        assert_eq!(civil_from_days(20_581), (2026, 5, 8)); // a real post's date
        assert_eq!(civil_from_days(20_644), (2026, 7, 10));
    }

    #[test]
    fn today_is_a_well_formed_iso_date() {
        let t = today_utc();
        assert_eq!(t.len(), 10, "got {t}");
        let (y, rest) = t.split_at(4);
        assert!(y.parse::<u32>().unwrap() >= 2024, "got {t}");
        assert!(rest.starts_with('-') && rest[3..4] == *"-", "got {t}");
    }

    #[test]
    fn a_slug_becomes_the_title_an_author_would_have_typed() {
        assert_eq!(title_from_slug("my-first-post"), "My First Post");
        assert_eq!(title_from_slug("about"), "About");
        assert_eq!(title_from_slug("pca-2d"), "Pca 2d");
    }

    #[test]
    fn a_slug_may_not_escape_the_project_or_carry_a_path() {
        assert!(validate_slug("my-first-post").is_ok());
        for bad in ["", "../evil", "a/b", "Upper", "has space", "dot.tmd"] {
            assert!(validate_slug(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn an_unknown_kind_suggests_the_nearest() {
        assert!(NewKind::parse("post").is_ok());
        let e = NewKind::parse("pots").unwrap_err();
        assert!(e.contains("did you mean `post`?"), "got: {e}");
        let e = NewKind::parse("zzzzzz").unwrap_err();
        assert!(e.contains("expected post, page, or deck"), "got: {e}");
    }

    /// The scaffold's bytes are pinned by `corpus/scaffold/`, which the corpus regression
    /// net renders and lints like any other document. If `new` ever emits a front-matter
    /// key the validator rejects, `cargo test -p taliesin-core` fails; if it emits
    /// something else entirely, this fails.
    #[test]
    fn every_scaffold_matches_its_corpus_pin() {
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scaffold");
        for kind in [NewKind::Post, NewKind::Page, NewKind::Deck] {
            let slug = match kind {
                NewKind::Post => "my-first-post",
                NewKind::Page => "about",
                NewKind::Deck => "my-talk",
            };
            for (rel, contents) in new_files(kind, slug, "2026-07-10") {
                let pinned = std::fs::read_to_string(corpus.join(&rel))
                    .unwrap_or_else(|e| panic!("corpus pin for {kind:?} at {rel:?}: {e}"));
                assert_eq!(
                    contents,
                    pinned,
                    "`taliesin new {slug}` drifted from corpus/scaffold/{}",
                    rel.display()
                );
            }
        }
    }
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
