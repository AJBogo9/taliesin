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
    - Add more `.tmd` pages beside this one — each becomes its own page.\n\
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
