//! Front-door subcommands: `init` (scaffold a starter site) and `preview` (launch the
//! live preview server).
//!
//! **What:** `init` writes a minimal previewable site (`_site.yml` + `index.tmd` + one
//! example post); `cmd_serve` parses the preview flags (`--open`/`--no-exec`/port) and
//! starts the dev server.
//!
//! **How to use:** `main()` dispatches `init` and `preview` to `cmd_init` / `cmd_serve`
//! here. The `serve`/`dev` spellings of `preview` were retired in Wave 5, which is why
//! `cmd_serve` keeps a name its verb no longer has.
//!
//! **Depends on:** [`crate::serve_site`] (the dev server), [`crate::serve`] (its shared
//! plumbing + CLI error helpers) and [`crate::log`].

use crate::{log, serve, serve_site};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `_site.yml` for the scaffold: the minimal flat-native config, just a title and a
/// commented-out `url:`.
///
/// It carried a `# yaml-language-server: $schema=` modeline until Wave 8, pointing at a
/// copy of the bundled schema that `init` wrote into a `.taliesin/` dot-directory. Both are
/// gone, and stay gone: a project acquiring an unexplained dot-directory and a modeline for
/// the one config file `taliesin lsp` does not serve was the wrong trade, and it buys
/// nothing without a YAML language server installed to read either. An author who wants that
/// completion copies the schema out of the repository and writes the modeline themselves
/// (`docs/guide/reference/frontmatter.tmd`). The tool's own `_site.yml` files carry none.
///
/// `url:` is commented rather than absent because the scaffolded `index.tmd` carries a
/// `listing:` of dated posts — explicitly a blog, the one shape that wants a feed. Atom
/// feeds, `sitemap.xml` and `robots.txt` are all gated on `canonical_base()`, so with no
/// `url:` the entire publish-adjacent surface is off and the build summary simply omits it.
/// Commented-out, it is a knob the author *discovers* on first read of their own config
/// instead of a default that invents a hostname for them.
const INIT_SITE_YML: &str =
    "title: My site\n# url: \"https://example.com\"  # set this to publish a feed + sitemap\n";

/// `index.tmd` for the scaffold: a hello-world page that previews immediately and
/// points the new user at the next steps. `.tmd` is the native extension.
///
/// The `listing:` block is what wires the example post below into the homepage. Without
/// it, the first post an author has is reachable by typing its URL and by nothing else,
/// with the listing machinery already built and simply not pointed at the one directory
/// the scaffold writes. `type:` is spelled out even though `list` is the default: it is
/// the one knob a new author wants (swap it for `grid` and the homepage becomes a card
/// grid), and a scaffold is read as an example.
const INIT_INDEX_TMD: &str = "---\ntitle: Hello, Taliesin\nlisting:\n  contents: posts\n  type: list\n---\n\n\
    Welcome to your new [Taliesin](https://github.com/AJBogo9/taliesin) site.\n\n\
    Edit `index.tmd` and the preview reloads as you save.\n\n\
    ## Next steps\n\n\
    - Copy `posts/my-first-post/` to start another post (add `draft: true` to hold one back).\n\
    - Add more `.tmd` pages beside this one: each becomes its own page.\n\
    - Configure navigation and the title in `_site.yml`.\n\
    - Drop in a `{python}` code cell to run live output.\n";

/// `posts/my-first-post/index.tmd` for the scaffold: one dated post, correct on its first
/// save (it renders, and `build --check-only` passes on it with no diagnostics).
///
/// It was `taliesin new post <slug>`'s output until the verb was cut on 2026-08-17. A verb
/// carried slug validation, a kind register and a 337-line integration suite to write this
/// one file; a starter that already contains it costs a const, gives the author a template
/// to copy for the next post, and leaves the homepage's `listing:` with something to show.
/// Every front-matter key here is one the validator knows, which `init_cli.rs` asserts by
/// running the real lint over what the real binary wrote.
const INIT_POST_TMD: &str = "---\ntitle: \"My First Post\"\ndate: {date}\n\
    description: \"One sentence: what a reader will understand by the end.\"\n\
    categories: [writing]\n\
    ---\n\n\
    Open with the question this post answers.\n\n\
    ## The first idea\n\n\
    Save the file and the preview re-renders only the block you changed.\n";

/// The authored files `taliesin init` writes, as `(project-relative path, contents)`. Pure
/// (the date is passed in), so the CLI stays a thin wrapper over three constants.
///
/// It took a `template` argument until Wave 8, selecting between this one-page starter and a
/// `site` (nav + an About stub) and a `book` (three chapters). Both were shapes a writer
/// reaches by adding a `nav:` block or a `chapters:` list to the config they already have:
/// a menu in front of the first command anyone types, pinned by three corpus projects.
fn init_files(today: &str) -> Vec<(PathBuf, String)> {
    vec![
        (PathBuf::from("_site.yml"), INIT_SITE_YML.to_string()),
        (PathBuf::from("index.tmd"), INIT_INDEX_TMD.to_string()),
        (
            PathBuf::from("posts")
                .join("my-first-post")
                .join("index.tmd"),
            INIT_POST_TMD.replace("{date}", today),
        ),
    ]
}

/// Every long flag `init` accepts, i.e. none: it drives the unknown-flag did-you-mean, and
/// an empty set means any `-flag` gets a bare "unknown flag" (or a retirement note).
const INIT_FLAGS: &[&str] = &[];

/// `taliesin init [dir]`: scaffold a minimal previewable site into `dir` (default the
/// current directory). Writes `_site.yml`, `index.tmd` and one dated example post, then
/// prints the preview hint.
///
/// It took `--json`/`--format` until 2026-08-13, printing a `{created, preview}` receipt.
/// Neither appeared anywhere in the manual, `human` was a pure no-op, and
/// `docs/guide/reference/cli.tmd` states that `build --check-only --format json` is the
/// tool's one machine-readable surface -- which deleting these makes true.
pub(crate) fn cmd_init(args: &[String]) -> ExitCode {
    let mut dir_arg: Option<&str> = None;
    let it = args[2..].iter();
    for a in it {
        match a.as_str() {
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
            {
                for f in &written {
                    log::built(&f.display().to_string());
                }
                // The next step ends INSIDE the project when `init` was given a directory.
                // It used to print `taliesin preview myblog`, which previews correctly but
                // leaves the author in the parent, one directory away from every path the
                // scaffolded homepage names.
                if where_ == "." {
                    println!("Scaffolded a Taliesin site. Preview it:\n  taliesin preview .");
                } else {
                    println!(
                        "Scaffolded a Taliesin site. Preview it:\n  cd {where_}\n  taliesin preview ."
                    );
                }
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
    write_scaffold(dir, &init_files(&today_utc()))
}

/// Today's date as `YYYY-MM-DD`, **UTC**, for the scaffolded post's `date:`. Taliesin has
/// no date dependency and does not want one for this (see the backlog's library-outsourcing
/// ruling), so the civil date is derived from the Unix day number directly. Near midnight
/// this can name yesterday or tomorrow in the author's local zone; the date is front matter
/// they can edit.
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

/// Write `files` (project-relative path → contents) under `root`, refusing to overwrite any
/// existing target before writing any of them and creating parent dirs as needed; returns
/// the paths written.
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
        let post = dir.join("posts").join("my-first-post").join("index.tmd");
        assert!(site_yml.exists(), "_site.yml written");
        assert!(index.exists(), "index.tmd written");
        assert!(post.exists(), "the example post written");
        assert_eq!(written, vec![site_yml.clone(), index.clone(), post.clone()]);

        // The post is dated, which is what the homepage's `listing:` orders on and what a
        // feed needs. `{date}` is a placeholder in the const and must not survive into the
        // file the author opens.
        let body = fs::read_to_string(&post).unwrap();
        assert!(
            body.contains(&format!("date: {}", today_utc())),
            "the example post carries today's date: {body}"
        );
        assert!(
            !body.contains("{date}"),
            "placeholder left unfilled: {body}"
        );

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
mod date_tests {
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
}
