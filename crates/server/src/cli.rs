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
const SERVE_FLAGS: &[&str] = &["--open", "--host", "--no-exec"];

pub(crate) fn cmd_serve(args: &[String]) -> ExitCode {
    // Positionals are <file.tmd> [port]; flags (--open, --host) may appear anywhere.
    let positionals: Vec<&String> = args[2..].iter().filter(|a| !a.starts_with("--")).collect();
    // An unrecognized `--flag` is a hard error with a did-you-mean (not silently filtered
    // out of the positionals — a typo'd `--hots` would otherwise preview without exposing).
    for a in &args[2..] {
        if a.starts_with("--") && !SERVE_FLAGS.contains(&a.as_str()) {
            log::error(&serve::unknown_flag_error(a, SERVE_FLAGS));
            return ExitCode::FAILURE;
        }
    }
    let flag = |name: &str| args.iter().any(|a| a == name);
    let open = flag("--open") || std::env::var_os("QMD_FAST_OPEN").is_some();
    let expose = flag("--host") || std::env::var_os("QMD_FAST_HOST").is_some();
    // `--no-exec` is sugar for `QMD_FAST_NO_EXEC=1`, which `exec::Executor` reads:
    // preview a document you don't trust without running its code cells.
    if flag("--no-exec") {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel
        // threads spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("QMD_FAST_NO_EXEC", "1") };
    }
    let Some(path) = positionals.first() else {
        eprintln!("usage: taliesin preview <file.tmd|dir> [port] [--host] [--open] [--no-exec]");
        return ExitCode::FAILURE;
    };
    // The optional second positional is the port; a present-but-unparseable value
    // is an error rather than a silent fall-back to the default.
    let port: u16 = match parse_port(positionals.get(1).map(|s| s.as_str())) {
        Ok(n) => n,
        Err(msg) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
    };
    // A directory is a multi-page site project; a single `.tmd` is one document.
    let result = if Path::new(path.as_str()).is_dir() {
        serve_site::run(PathBuf::from(path), port, open, expose)
    } else {
        serve::run(PathBuf::from(path), port, open, expose)
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
