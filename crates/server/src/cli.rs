//! Front-door subcommands: `init` (scaffold a starter site), `new` (scaffold one document)
//! and `preview` (launch the live preview server).
//!
//! **What:** `init` writes a minimal previewable site (`_site.yml` + `index.tmd`);
//! `cmd_serve` parses the preview flags (`--open`/`--no-exec`/port) and starts the
//! dev server.
//!
//! **How to use:** `main()` dispatches `init`, `new` and `preview` to `cmd_init` /
//! `cmd_new` / `cmd_serve` here. The `serve`/`dev` spellings of `preview` were retired in
//! Wave 5, which is why `cmd_serve` keeps a name its verb no longer has.
//!
//! **Depends on:** [`crate::serve_site`] (the dev server), [`crate::serve`] (its shared
//! plumbing + CLI error helpers) and [`crate::log`].

use crate::{log, serve, serve_site};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `_site.yml` for the scaffold: the minimal flat-native config, just a title.
///
/// It carried a `# yaml-language-server: $schema=` modeline until Wave 8, pointing at a
/// copy of the bundled schema that `init` wrote into a `.taliesin/` dot-directory. Both are
/// gone: the VS Code companion already ships that schema and wires it through
/// `yamlValidation`, and a project acquiring an unexplained dot-directory to serve every
/// *other* editor was the wrong trade for the one config file `taliesin lsp` does not serve.
const INIT_SITE_YML: &str = "title: My site\n";

/// `index.tmd` for the scaffold: a hello-world page that previews immediately and
/// points the new user at the next steps. `.tmd` is the native extension.
const INIT_INDEX_TMD: &str = "---\ntitle: Hello, Taliesin\n---\n\n\
    Welcome to your new [Taliesin](https://github.com/AJBogo9/taliesin) site.\n\n\
    Edit `index.tmd` and the preview reloads as you save.\n\n\
    ## Next steps\n\n\
    - Scaffold a dated post with `taliesin new post my-first-post` (add `--draft` to hold it back).\n\
    - Add more `.tmd` pages beside this one: each becomes its own page.\n\
    - Configure navigation and the title in `_site.yml`.\n\
    - Drop in a `{python}` code cell to run live output.\n";

/// The authored files `taliesin init` writes, as `(project-relative path, contents)`. Pure,
/// so the CLI stays a thin wrapper over two constants.
///
/// It took a `template` argument until Wave 8, selecting between this one-page starter and a
/// `site` (nav + an About stub) and a `book` (three chapters). Both were shapes a writer
/// reaches by adding a `nav:` block or a `chapters:` list to the config they already have:
/// a menu in front of the first command anyone types, pinned by three corpus projects.
fn init_files() -> Vec<(PathBuf, String)> {
    [("_site.yml", INIT_SITE_YML), ("index.tmd", INIT_INDEX_TMD)]
        .into_iter()
        .map(|(name, contents)| (PathBuf::from(name), contents.to_string()))
        .collect()
}

/// Every long flag `init` accepts (drives the unknown-flag did-you-mean).
const INIT_FLAGS: &[&str] = &["--json", "--format"];

/// `taliesin init [dir] [--json]`: scaffold a minimal previewable site into `dir` (default
/// the current directory). Writes `_site.yml` + `index.tmd`, then prints the preview hint
/// (or, with `--json`, a `{created, preview}` receipt).
pub(crate) fn cmd_init(args: &[String]) -> ExitCode {
    let mut dir_arg: Option<&str> = None;
    let mut json = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            // `--format json` / `--format human`: accept the long spelling too (json is the
            // shorthand), so `init --format json` doesn't dead-end.
            "--format" => match it.next().map(|s| s.as_str()) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    log::error(&serve::bad_format_error(other));
                    return ExitCode::FAILURE;
                }
            },
            // Any leading dash is a flag, not a directory name. `--` alone would be enough
            // if `-y` had never existed; it did until Wave 8, and a leftover `taliesin init
            // -y` must not scaffold a project into a directory called `-y`.
            s if s.starts_with('-') => {
                log::error(&serve::unknown_flag_error(s, INIT_FLAGS));
                return ExitCode::FAILURE;
            }
            s if dir_arg.is_none() => dir_arg = Some(s),
            _ => {}
        }
    }

    let dir_owned: String = dir_arg.unwrap_or(".").to_string();

    let dir = Path::new(&dir_owned);
    let where_ = if dir == Path::new(".") {
        ".".to_string()
    } else {
        dir.display().to_string()
    };
    match scaffold_init(dir) {
        Ok(written) => {
            if json {
                let created: Vec<String> =
                    written.iter().map(|p| p.display().to_string()).collect();
                let payload = serde_json::json!({
                    "created": created,
                    "preview": format!("taliesin preview {where_}"),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                for f in &written {
                    log::built(&f.display().to_string());
                }
                println!("Scaffolded a Taliesin site. Preview it:\n  taliesin preview {where_}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// Scaffold the starter into `dir`, creating it if needed. Refuses to overwrite an existing
/// file (so re-running `init` never clobbers the user's work) and returns the paths written.
fn scaffold_init(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Err(format!("cannot create {}: {e}", dir.display()));
    }
    write_scaffold(dir, &init_files())
}

/// Write `files` (project-relative path → contents) under `root`, refusing to overwrite any
/// existing target before writing any of them and creating parent dirs as needed. Shared by
/// `init` (project scaffold) and `new` (document scaffold); returns the paths written.
fn write_scaffold(root: &Path, files: &[(PathBuf, String)]) -> Result<Vec<PathBuf>, String> {
    // Refuse to overwrite *any* target before writing *any*, so a partial scaffold never
    // lands on top of an existing project.
    for (rel, _) in files {
        let path = root.join(rel);
        if path.exists() {
            return Err(format!(
                "{} already exists; refusing to overwrite",
                path.display()
            ));
        }
    }
    let mut written = Vec::new();
    for (rel, contents) in files {
        let path = root.join(rel);
        // A nested target (`posts/<slug>/index.tmd`) needs its parent created first.
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

/// What `taliesin new` can scaffold: a dated blog post, which is the document shape this
/// tool is for. The `deck` kind went with the slide-deck engine in Wave 5; `page` and
/// `paper` went in Wave 8, leaving one, so there is no `NewKind` enum any more, on the
/// precedent Wave 5 set when it deleted a one-variant `DocFormat` rather than keeping it.
/// The positional survives (`taliesin new post <slug>`), because it is what a retired kind
/// is typed into and what a second kind would be added to.
const NEW_KINDS: &[&str] = &["post"];

/// Kinds this verb used to scaffold, and the one line that says what to do instead.
///
/// The same job [`crate::RETIRED_COMMANDS`] does for a verb, and for the same reason: every
/// one of these is edit-distance 3 or more from `post`, so a removed kind would otherwise
/// fall through to "unknown kind `paper` (expected post)", which is technically true and
/// silent about the fact that the tool used to do exactly what was asked.
const RETIRED_NEW_KINDS: &[(&str, &str)] = &[
    (
        "deck",
        "removed on 2026-08-08 with the slide-deck engine: write the talk as a page of prose",
    ),
    (
        "page",
        "removed on 2026-08-08: a page is a `.tmd` file with a `title:` in its front matter, \
         so write it directly beside `index.tmd`",
    ),
    (
        "paper",
        "removed on 2026-08-08: scaffold a `post` and add `bibliography: [references.bib]` \
         to its front matter",
    ),
];

/// Accept the one live kind, or explain what happened to the one that was typed.
fn parse_new_kind(raw: &str) -> Result<(), String> {
    if NEW_KINDS.contains(&raw) {
        return Ok(());
    }
    // A removal, not a misspelling: answering one of these with a did-you-mean would
    // send the author to a kind that scaffolds something else entirely.
    if let Some((_, note)) = RETIRED_NEW_KINDS.iter().find(|(name, _)| *name == raw) {
        return Err(format!("`new {raw}` was {note}"));
    }
    Err(match taliesin_core::closest(raw, NEW_KINDS) {
        Some(k) => format!("unknown kind `{raw}` (did you mean `{k}`?)"),
        None => format!("unknown kind `{raw}` (expected post)"),
    })
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

/// Per-invocation options for `taliesin new` (beyond the slug). `Default` is today's
/// behavior, so an unflagged scaffold is byte-identical to before.
#[derive(Clone, Copy, Default)]
struct NewOpts {
    /// `--draft`: mark the scaffold `draft: true`, holding it out of the published build.
    draft: bool,
}

/// The files a `taliesin new post <slug>` writes, as `(project-relative path, contents)`.
///
/// Pure, so the CLI can stay a thin wrapper. Every front-matter key here is one the
/// validator knows; a `check`-clean scaffold is asserted end-to-end (through the real
/// binary, then the real `check`) by `crates/server/tests/new_cli.rs`.
fn new_files(slug: &str, today: &str, opts: NewOpts) -> Vec<(PathBuf, String)> {
    let title = title_from_slug(slug);
    // `--draft` splices a `draft: true` line into the front matter (right after `title:`);
    // default off emits nothing, keeping the unflagged scaffold byte-identical.
    let draft = if opts.draft { "draft: true\n" } else { "" };
    let body = format!(
        "---\n\
         title: \"{title}\"\n{draft}\
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
    );
    vec![(PathBuf::from("posts").join(slug).join("index.tmd"), body)]
}

/// `taliesin new post <slug> [--dir <root>]`: scaffold one document, correct on its first
/// save. Refuses to overwrite, exactly as `init` does.
pub(crate) fn cmd_new(args: &[String]) -> ExitCode {
    let mut positional: Vec<&str> = Vec::new();
    let mut root = ".".to_string();
    let mut json = false;
    let mut opts = NewOpts::default();
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // `--dir` = the scaffold-input root (where the project lives). The undocumented
            // `--out` alias was dropped — `--out` is the output-dir flag on build/publish.
            "--dir" => {
                if let Some(v) = it.next() {
                    root = v.clone();
                }
            }
            // `--json` prints `{kind, slug, created, preview}` (pure JSON to stdout), so an
            // agent knows exactly what it made and where. Suppresses the human hints.
            "--json" => json = true,
            // `--format json` / `--format human`: accept the long spelling too (json is the
            // shorthand), so `new --format json` doesn't dead-end.
            "--format" => match it.next().map(|s| s.as_str()) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    log::error(&serve::bad_format_error(other));
                    return ExitCode::FAILURE;
                }
            },
            // `--draft` marks the scaffold `draft: true` (held out of the published build).
            "--draft" => opts.draft = true,
            // Any leading dash is a flag, not a kind or a slug. `--` alone would be enough
            // if `-y` had never existed; it did until Wave 8, and a leftover `taliesin new
            // -y post x` must not be read as a request to scaffold a kind called `-y`.
            s if s.starts_with('-') => {
                log::error(&serve::unknown_flag_error(s, NEW_FLAGS));
                return ExitCode::FAILURE;
            }
            s => positional.push(s),
        }
    }

    match positional.first() {
        Some(k) => {
            if let Err(e) = parse_new_kind(k) {
                log::error(&e);
                return ExitCode::FAILURE;
            }
        }
        None => return new_usage(),
    }

    let slug: String = match positional.get(1) {
        Some(s) => (*s).to_string(),
        None => return new_usage(),
    };
    if let Err(e) = validate_slug(&slug) {
        log::error(&e);
        return ExitCode::FAILURE;
    }
    let root = Path::new(&root);
    match write_new(root, &slug, opts) {
        Ok(written) => {
            if json {
                println!("{}", new_json(&slug, &written));
            } else {
                for f in &written {
                    log::built(&f.display().to_string());
                }
                println!("{}", new_next_steps(&written[0]));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// What to do with a freshly scaffolded document.
fn new_next_steps(written: &Path) -> String {
    format!("Preview it:\n  taliesin preview {}", written.display())
}

/// The `new` usage line, printed when a kind/slug is missing and there's no TTY to prompt at.
/// Derived from `new`'s `--help` synopsis so the two can't drift.
fn new_usage() -> ExitCode {
    crate::usage_error("new")
}

/// The `--json` receipt for a scaffold: `{kind, slug, created:[...], preview}` as pretty
/// JSON. `kind` stays in the payload with one kind left, because a receipt an agent already
/// parses should not change shape for a reason the agent cannot see.
fn new_json(slug: &str, written: &[PathBuf]) -> String {
    let created: Vec<String> = written.iter().map(|p| p.display().to_string()).collect();
    let preview = written
        .first()
        .map(|p| format!("taliesin preview {}", p.display()))
        .unwrap_or_default();
    let payload = serde_json::json!({
        "kind": NEW_KINDS[0],
        "slug": slug,
        "created": created,
        "preview": preview,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Every long flag `new` accepts (drives the unknown-flag did-you-mean).
const NEW_FLAGS: &[&str] = &["--dir", "--json", "--format", "--draft"];

/// Write the scaffold under `root`, refusing to overwrite any existing target before
/// writing any of them (so a partial scaffold never lands on the author's work).
fn write_new(root: &Path, slug: &str, opts: NewOpts) -> Result<Vec<PathBuf>, String> {
    write_scaffold(root, &new_files(slug, &today_utc(), opts))
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

/// Every long flag `preview` accepts (drives the unknown-flag did-you-mean).
/// `--help`/`-h` are intercepted by `main()` before this parser runs, so they aren't here.
pub(crate) const SERVE_FLAGS: &[&str] = &["--open", "--no-exec", "--port"];

/// What `preview` parsed out of argv, before any environment or IO.
#[derive(Debug, PartialEq)]
pub(crate) struct ServeArgs<'a> {
    pub path: &'a str,
    pub port: u16,
    pub open: bool,
    pub no_exec: bool,
}

/// Parse `preview <file.tmd|dir> [port] [--port <N>] [--open] [--no-exec]`.
///
/// The port may be the second positional (the original spelling) or `--port <N>` /
/// `--port=<N>`. Without the flag, `--port 4400` tripped the unknown-flag did-you-mean and
/// was answered with an unrelated flag two edits away.
/// Pure + unit-tested: no environment reads, no filesystem.
pub(crate) fn parse_serve_args(args: &[String]) -> Result<ServeArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut flag_port: Option<&str> = None;
    let (mut open, mut no_exec) = (false, false);

    let mut it = args[2..].iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--open" => open = true,
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
            // dropped: a typo'd `--noexec` would otherwise preview and run every cell).
            s if s.starts_with("--") => return Err(serve::unknown_flag_error(s, SERVE_FLAGS)),
            s => positionals.push(s),
        }
    }

    let path = *positionals
        .first()
        .ok_or_else(|| crate::usage_line("preview"))?;
    // `--port` wins over the positional when both are given (the explicit flag is the
    // more deliberate spelling); a present-but-unparseable value is always an error.
    let port = parse_port(flag_port.or_else(|| positionals.get(1).copied()))?;
    Ok(ServeArgs {
        path,
        port,
        open,
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
    // `--open` is a flag and only a flag. The `TALIESIN_OPEN` env var that also set it was
    // retired in Wave 5: a second spelling for a flag is not worth its documentation.
    let open = parsed.open;
    // `--no-exec` is sugar for `TALIESIN_NO_EXEC=1`. Two readers, one owner
    // (`taliesin_core::render::no_exec_in_force`): `exec::Executor` skips the kernel, and the
    // render pass leaves a `{js}` cell as source, since a `{js}` cell is a code cell whose
    // runtime is the browser (item 79). It does NOT sanitize raw HTML — see the CLI
    // reference's "Documents you did not write".
    if parsed.no_exec {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel
        // threads spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("TALIESIN_NO_EXEC", "1") };
    }
    // A directory is a project; a `.tmd` is one document, served as the project it belongs
    // to (or as a project of its own). One server handles both — there is no separate
    // single-document server to dispatch to.
    let result = serve_site::run(
        serve_site::Target::at(PathBuf::from(parsed.path)),
        parsed.port,
        open,
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
        }
    }
}

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

        // `--port 4400` used to error with "unknown flag `--port` (did you mean …?)".
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

        // An unknown flag still gets a did-you-mean.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--prot", "4400"]);
        let err = parse_serve_args(&a).unwrap_err();
        assert!(err.contains("--prot"), "{err}");
        assert!(err.contains("--port"), "{err}");

        // Flags are not swallowed as positionals.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--no-exec", "--open"]);
        let p = parse_serve_args(&a).unwrap();
        assert!(p.no_exec && p.open && p.port == 4321);
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

        // And nothing the author did not ask for. `init` wrote a `.taliesin/` dot-directory
        // holding a copy of the bundled `_site.yml` schema until Wave 8, wired through a
        // modeline on the config's first line; zero such directories existed anywhere in
        // this repository, including in the author's own projects.
        assert!(
            !dir.join(".taliesin").exists(),
            "init scaffolds no dot-directory"
        );
        assert!(
            !cfg.contains("yaml-language-server"),
            "no schema modeline: {cfg}"
        );

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
        assert!(parse_new_kind("post").is_ok());
        let e = parse_new_kind("pots").unwrap_err();
        assert!(e.contains("did you mean `post`?"), "got: {e}");
        let e = parse_new_kind("zzzzzz").unwrap_err();
        assert!(e.contains("expected post"), "got: {e}");
    }

    /// A kind this verb used to scaffold answers with what to do instead, never with a
    /// did-you-mean. Every retired name is far enough from `post` that the distance rule
    /// declines, which is the silence this register replaces, not a wrong suggestion it
    /// overrides.
    #[test]
    fn a_retired_kind_names_what_to_do_instead() {
        for (name, note) in RETIRED_NEW_KINDS {
            assert_eq!(
                taliesin_core::closest(name, NEW_KINDS),
                None,
                "`{name}` is supposed to be out of distance-2 reach of `post`"
            );
            assert!(!note.is_empty(), "`{name}` retired with no note");
            let e = parse_new_kind(name).unwrap_err();
            assert!(e.contains(name), "the error names the kind typed: {e}");
            assert!(
                !e.contains("did you mean"),
                "the retired note replaces the did-you-mean, it does not follow it: {e}"
            );
            assert!(
                !NEW_KINDS.contains(name),
                "`{name}` is retired but still offered"
            );
        }
    }
}
