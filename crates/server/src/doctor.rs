//! `taliesin doctor`: a standalone environment self-audit. Surfaces the interpreter +
//! kernel-package probe (`crate::interpreter`) unconditionally for both Python and R, plus
//! active conda/virtualenv detection and `_site.yml` sanity, with ✓/⚠/✗ status and fix
//! commands. Answers "is my environment ready to run code cells?" before any document exists
//! (a document-scoped lint only sees the languages that document already uses, which is
//! circular — and wave 9 removed the interpreter probe from that path entirely).
//!
//! Pure core (`interpreter_check`/`active_env_check`/`overall_ok`): probe results + env vars
//! are injected, so the severity logic is unit-tested without spawning or touching process
//! env. `cmd_doctor` is the thin I/O wrapper. Never executes the user's document.

use crate::interpreter::{Probe, Provenance, Resolved};
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

/// Severity of one audit line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Error,
}

impl Status {
    fn glyph(self) -> char {
        match self {
            Status::Ok => '✓',
            Status::Warn => '⚠',
            Status::Error => '✗',
        }
    }
    fn color(self) -> &'static str {
        match self {
            Status::Ok => "\x1b[32m",    // green
            Status::Warn => "\x1b[33m",  // yellow
            Status::Error => "\x1b[31m", // red
        }
    }
    fn json(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Error => "error",
        }
    }
}

/// One line of the audit: a named check, its status, a human detail (may be multi-line), and
/// an optional fix command.
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
    fix: Option<String>,
    /// For an interpreter line: whether cells in that language will actually execute.
    /// Deliberately independent of `status` — a bare `PATH` interpreter that works is
    /// usable (cells execute) yet still earns a ⚠, because nothing in the project chose
    /// it. `None` on the non-interpreter lines, which say nothing about execution.
    executes: Option<bool>,
}

/// The `pip install` line that installs the missing kernel package into the exact
/// interpreter that was probed.
fn kernel_install_fix(r: &Resolved) -> String {
    format!("{} -m pip install ipykernel", r.path.display())
}

/// The fix when the *default* interpreter is simply absent (nothing was configured).
fn absent_default_fix() -> &'static str {
    "install Python 3, then: python3 -m pip install ipykernel"
}

/// The fix when the interpreter *works* but nothing in the project chose it. Names the
/// in-repo, committable options first — the upward `.venv` walk and a project-dir-relative
/// `python:` exist precisely so neither of these needs a per-machine symlink or wrapper.
fn project_env_fix() -> String {
    "create a .venv at the project or repository root, or set `python:` in _site.yml (a \
     relative path resolves against it)"
        .to_string()
}

/// Map a resolved interpreter + its probe to one audit line (pure; the severity model).
fn interpreter_check(r: &Resolved, p: &Probe) -> Check {
    let name = "python";
    let pkg = "ipykernel";
    let where_ = format!("{} ({})", r.path.display(), r.provenance.label());
    // Where the upward `.venv` walk looked and where it stopped. Shown on every line —
    // including the ones that resolved fine — because "why did it pick that one?" is
    // exactly the question a *successful-looking* wrong pick raises.
    let searched = r
        .trail
        .ancestor
        .as_ref()
        .map(|s| format!("\n.venv search: {}", s.summary()))
        .unwrap_or_default();
    let ver = p
        .version
        .clone()
        .map(|v| format!("{v}  ·  "))
        .unwrap_or_default();
    if p.runs && p.kernel_pkg_ok {
        // It works — but if nothing in the project selected it, a green ✓ overstates the
        // case. `python3 (python3)` is whatever is on `PATH`; it runs cells, it just does
        // not have the project's packages, and reading it as "ready" is how a build gets
        // as far as `ModuleNotFoundError` in every cell. Still exit-0 (`overall_ok` fails
        // only on `Status::Error`) — an unconfigured environment is worth naming, not a
        // broken one.
        let chosen = !matches!(r.provenance, Provenance::Default);
        return Check {
            name,
            status: if chosen { Status::Ok } else { Status::Warn },
            detail: if chosen {
                format!("{where_}\n{ver}{pkg} present{searched}")
            } else {
                format!(
                    "{where_}\n{ver}{pkg} present, but nothing in this project selected \
                     this interpreter{searched}"
                )
            },
            fix: (!chosen).then(project_env_fix),
            executes: Some(true),
        };
    }
    if p.runs {
        return Check {
            name,
            status: Status::Warn,
            detail: format!("{where_}\n{ver}{pkg} MISSING{searched}"),
            fix: Some(kernel_install_fix(r)),
            executes: Some(false),
        };
    }
    // Does not run at all.
    let err = p
        .error
        .clone()
        .unwrap_or_else(|| "interpreter not found".to_string());
    if matches!(r.provenance, Provenance::Default) {
        // You just don't have it (nothing was configured): a warning, not a misconfiguration.
        Check {
            name,
            status: Status::Warn,
            detail: format!("{where_}\n{err}  ·  {name} cells will render as source{searched}"),
            fix: Some(absent_default_fix().to_string()),
            executes: Some(false),
        }
    } else {
        // A pointed-at interpreter (env / field / .venv) that is broken: a real error.
        Check {
            name,
            status: Status::Error,
            detail: format!("{where_}\n{err}{searched}"),
            fix: Some(format!(
                "point {} at a real interpreter, or unset it",
                r.provenance.label()
            )),
            executes: Some(false),
        }
    }
}

/// The active conda/virtualenv line (pure; env injected). Informational (always ✓): it never
/// gates the exit, it just answers "which env is active / did you forget to activate one?".
fn active_env_check(
    venv: Option<&str>,
    conda_prefix: Option<&str>,
    conda_name: Option<&str>,
) -> Check {
    let detail = if let Some(prefix) = conda_prefix.filter(|s| !s.is_empty()) {
        let name = conda_name
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                Path::new(prefix)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(prefix)
                    .to_string()
            });
        format!("active conda env: {name}")
    } else if let Some(v) = venv.filter(|s| !s.is_empty()) {
        format!("active virtualenv: {v}")
    } else {
        "no active virtual/conda env (using the system PATH)".to_string()
    };
    Check {
        name: "env",
        status: Status::Ok,
        detail,
        fix: None,
        executes: None,
    }
}

/// The audit passes (exit 0) unless a configured interpreter is broken (a `Status::Error`).
fn overall_ok(checks: &[Check]) -> bool {
    !checks.iter().any(|c| c.status == Status::Error)
}

const DOCTOR_FLAGS: &[&str] = &["--format", "--json"];

fn colored() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}
fn paint(text: &str, code: &str) -> String {
    if colored() {
        format!("{code}{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// A one-line readiness summary derived from the interpreter checks (honest about what will
/// execute vs render as source).
fn summary(checks: &[Check]) -> String {
    // Keyed on `executes`, not on `status`. A working-but-unselected interpreter is a ⚠
    // yet still runs cells; reading the status here would have the summary contradict the
    // line directly above it.
    let say = |name: &str, lang: &str| match checks
        .iter()
        .find(|c| c.name == name)
        .and_then(|c| c.executes)
    {
        Some(true) => format!("{lang} cells will execute"),
        _ => format!("{lang} cells will render as source"),
    };
    format!("{}.", say("python", "python"))
}

fn print_human(checks: &[Check]) {
    println!("taliesin doctor  ·  is your environment ready to run code cells?\n");
    for c in checks {
        let glyph = paint(&c.status.glyph().to_string(), c.status.color());
        let mut lines = c.detail.lines();
        let first = lines.next().unwrap_or("");
        println!("  {glyph}  {:<7} {first}", c.name);
        for l in lines {
            println!("             {l}");
        }
        if let Some(fix) = &c.fix {
            println!("             {} {fix}", paint("fix:", "\x1b[2m"));
        }
    }
    println!("\n  {}", summary(checks));
}

fn print_json(checks: &[Check], packages: &[(&'static str, crate::packages::Manifest)]) {
    let payload = serde_json::json!({
        "ok": overall_ok(checks),
        "checks": checks.iter().map(|c| serde_json::json!({
            "name": c.name,
            "status": c.status.json(),
            "detail": c.detail,
            "fix": c.fix,
        })).collect::<Vec<_>>(),
        "packages": packages.iter().map(|(lang, m)| (
            lang.to_string(),
            serde_json::json!({ "digest": m.digest, "packages": m.packages }),
        )).collect::<serde_json::Map<_, _>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
    );
}

/// `taliesin doctor [dir] [--format human|json]`: audit the environment for running code cells.
pub(crate) fn cmd_doctor(args: &[String]) -> ExitCode {
    let mut dir = ".".to_string();
    let mut json = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--format" => match it.next().map(|s| s.as_str()) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    crate::log::error(&crate::serve::bad_format_error(other));
                    return ExitCode::FAILURE;
                }
            },
            // `--json`: clig.dev shorthand for `--format json`.
            "--json" => json = true,
            s if s.starts_with("--") => {
                crate::log::error(&crate::serve::unknown_flag_error(s, DOCTOR_FLAGS));
                return ExitCode::FAILURE;
            }
            s => dir = s.to_string(),
        }
    }
    let dir = Path::new(&dir);

    // Honour an `_site.yml` python: field + its config sanity, exactly as a build would; a
    // single-doc project (no `_site.yml`) has no field pins and no config check.
    let site = dir
        .join("_site.yml")
        .exists()
        .then(|| taliesin_core::Site::discover(dir));
    let py = crate::interpreter::resolve_python(
        site.as_ref().and_then(|s| s.config.python.as_deref()),
        dir,
    );
    let py_probe = crate::interpreter::probe(&py);

    let venv = std::env::var("VIRTUAL_ENV").ok();
    let conda_prefix = std::env::var("CONDA_PREFIX").ok();
    let conda_name = std::env::var("CONDA_DEFAULT_ENV").ok();

    let mut checks = vec![
        interpreter_check(&py, &py_probe),
        active_env_check(
            venv.as_deref(),
            conda_prefix.as_deref(),
            conda_name.as_deref(),
        ),
    ];
    if let Some(site) = &site {
        let bad = site
            .warnings
            .iter()
            .find(|w| taliesin_core::site::is_malformed_config_warning(w));
        checks.push(match bad {
            None => Check {
                name: "config",
                status: Status::Ok,
                detail: "_site.yml is valid".to_string(),
                fix: None,
                executes: None,
            },
            Some(w) => Check {
                name: "config",
                status: Status::Warn,
                detail: format!("_site.yml: {w}"),
                fix: Some("fix the YAML in _site.yml".to_string()),
                executes: None,
            },
        });
    }

    // Which packages, not just which interpreter. `doctor` reported "ipykernel MISSING" and
    // never "which pandas", which is the half that decides whether a document reproduces —
    // CHI 2020's Reproduce and Reuse pain point, and the axis `_freeze/`'s cumulative key
    // structurally cannot see. Probed only for `--format json`: this is a machine channel,
    // and a human report does not want two hundred lines of version numbers. A language whose
    // interpreter cannot be probed is simply absent, rather than reported as an empty
    // environment.
    let mut packages: Vec<(&'static str, crate::packages::Manifest)> = Vec::new();
    if json {
        if let Some(m) = crate::packages::manifest(&py.path) {
            packages.push(("python", m));
        }
        print_json(&checks, &packages);
    } else {
        print_human(&checks);
    }
    if overall_ok(&checks) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn resolved(path: &str, provenance: Provenance) -> Resolved {
        Resolved::fixed(PathBuf::from(path), provenance)
    }

    #[test]
    fn a_working_bare_path_python_is_not_a_green_check() {
        // `✓ python  python3 (python3)` was the misleading line: the interpreter runs and
        // has ipykernel, so it reads as "you are ready" — but nothing in the project
        // chose it, so it is whatever `PATH` happens to point at and it does not have the
        // project's packages. A ⚠ says the true thing. It stays exit-0 (`overall_ok`
        // fails only on `Status::Error`): not having a project venv is a fact worth
        // naming, not a broken environment.
        let p = Probe {
            runs: true,
            version: Some("Python 3.11.4".into()),
            kernel_pkg_ok: true,
            error: None,
        };
        let c = interpreter_check(&resolved("python3", Provenance::Default), &p);
        assert_eq!(c.status, Status::Warn);
        assert!(
            c.fix.is_some(),
            "it must say how to point at a project environment"
        );
        // ...and the readiness summary must still tell the truth: these cells DO execute.
        assert!(
            summary(&[c]).contains("python cells will execute"),
            "a usable interpreter still executes cells, warning or not"
        );
    }

    #[test]
    fn an_interpreter_line_says_where_the_upward_search_stopped() {
        // Criterion: a wrong pick must be diagnosable without reading source.
        let root = std::env::temp_dir().join(format!("tali-doctor-{}", std::process::id()));
        let book = root.join("docs/book");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&book).unwrap();
        std::fs::write(root.join(".git"), b"").unwrap();
        let r = crate::interpreter::resolve_python(None, &book);
        let p = Probe {
            runs: true,
            version: None,
            kernel_pkg_ok: true,
            error: None,
        };
        let c = interpreter_check(&r, &p);
        assert!(
            c.detail.contains("stopped at") && c.detail.contains(&root.display().to_string()),
            "the line must name where the upward .venv search stopped: {}",
            c.detail
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Changed with the interpreter-resolution work: this used to assert that a bare
    /// `python3` (`Provenance::Default`) with ipykernel is a green ✓. That green check is
    /// the misleading one — see `a_working_bare_path_python_is_not_a_green_check` — so the
    /// fixture moves to an interpreter the *project* actually selected, which is what
    /// "ready" was always meant to mean. Everything else it asserts is untouched.
    #[test]
    fn a_project_selected_ready_interpreter_is_ok() {
        let p = Probe {
            runs: true,
            version: Some("Python 3.11.4".into()),
            kernel_pkg_ok: true,
            error: None,
        };
        let c = interpreter_check(&resolved("/proj/.venv/bin/python", Provenance::Venv), &p);
        assert_eq!(c.status, Status::Ok);
        assert!(c.detail.contains("ipykernel present"), "{}", c.detail);
        assert!(c.fix.is_none());
        assert_eq!(c.executes, Some(true));
    }

    #[test]
    fn missing_kernel_pkg_warns_with_install_fix() {
        let p = Probe {
            runs: true,
            version: Some("Python 3.11".into()),
            kernel_pkg_ok: false,
            error: None,
        };
        let c = interpreter_check(&resolved("/proj/.venv/bin/python", Provenance::Venv), &p);
        assert_eq!(c.status, Status::Warn);
        assert_eq!(
            c.fix.as_deref(),
            Some("/proj/.venv/bin/python -m pip install ipykernel")
        );
    }

    #[test]
    fn broken_configured_interpreter_is_an_error() {
        let p = Probe {
            runs: false,
            version: None,
            kernel_pkg_ok: false,
            error: Some("cannot run /bad/python: No such file or directory".into()),
        };
        let c = interpreter_check(&resolved("/bad/python", Provenance::Env), &p);
        assert_eq!(
            c.status,
            Status::Error,
            "a configured-but-broken interpreter errors"
        );
        assert!(c.fix.as_deref().unwrap().contains("TALIESIN_PYTHON"));
    }

    #[test]
    fn absent_default_interpreter_only_warns() {
        // No interpreter configured (Default) and it does not run: you just don't have it.
        let p = Probe {
            runs: false,
            version: None,
            kernel_pkg_ok: false,
            error: Some("cannot run python3: No such file or directory".into()),
        };
        let c = interpreter_check(&resolved("python3", Provenance::Default), &p);
        assert_eq!(
            c.status,
            Status::Warn,
            "an absent default is not a misconfiguration"
        );
        assert!(c.fix.as_deref().unwrap().contains("ipykernel"));
    }

    #[test]
    fn active_env_names_conda_then_venv_then_none() {
        assert!(
            active_env_check(None, Some("/opt/conda/envs/proj"), Some("proj"))
                .detail
                .contains("conda env: proj")
        );
        // conda prefix with no explicit name -> basename of the prefix.
        assert!(
            active_env_check(None, Some("/opt/conda/envs/proj"), None)
                .detail
                .contains("proj")
        );
        assert!(
            active_env_check(Some("/home/u/.venv"), None, None)
                .detail
                .contains("virtualenv: /home/u/.venv")
        );
        assert!(
            active_env_check(None, None, None)
                .detail
                .contains("no active")
        );
        // An EMPTY variable is not an active environment. Shells export `CONDA_PREFIX=` and
        // `VIRTUAL_ENV=` on deactivate rather than unsetting them, so "set" and "set to
        // something" are different questions and only the second one means anything here —
        // otherwise a deactivated shell is told it is inside an env with no name.
        assert!(
            active_env_check(Some(""), Some(""), Some(""))
                .detail
                .contains("no active"),
            "empty env vars are not an active environment"
        );
        assert!(
            active_env_check(Some("/home/u/.venv"), Some(""), None)
                .detail
                .contains("virtualenv: /home/u/.venv"),
            "an empty conda prefix must not shadow a real virtualenv"
        );
    }

    /// The readiness summary reads each language's verdict off **that language's** check. It
    /// is the one line most readers act on, and with two checks in the list a lookup that
    /// picks the wrong one still produces a plausible sentence — so the two must disagree
    /// for the assertion to mean anything.
    ///
    /// Changed with the interpreter-resolution work: the verdict is now read off
    /// `executes`, not off `status`. A working-but-unselected interpreter is a ⚠ that
    /// nonetheless runs cells, so keying on status would make this line contradict the
    /// interpreter line printed directly above it. The property under test is unchanged
    /// (each language read off its own check); only the field it reads moved.
    #[test]
    fn the_summary_reads_each_language_off_its_own_check() {
        let check = |name, executes| Check {
            name,
            status: Status::Ok,
            detail: String::new(),
            fix: None,
            executes,
        };
        assert_eq!(
            summary(&[check("python", Some(true)), check("env", None)]),
            "python cells will execute."
        );
        // The other direction, so a summary that ignored `executes` altogether — or read
        // the wrong check — cannot pass both halves. An interpreter present but missing
        // its kernel package renders as source, and saying otherwise promises execution
        // that will not happen.
        assert_eq!(
            summary(&[check("python", Some(false)), check("env", None)]),
            "python cells will render as source."
        );
    }

    #[test]
    fn overall_ok_is_false_only_with_an_error() {
        let ok = Check {
            name: "python",
            status: Status::Ok,
            detail: String::new(),
            fix: None,
            executes: None,
        };
        let warn = Check {
            name: "env",
            status: Status::Warn,
            detail: String::new(),
            fix: None,
            executes: None,
        };
        assert!(overall_ok(&[ok, warn]));
        let ok2 = Check {
            name: "python",
            status: Status::Ok,
            detail: String::new(),
            fix: None,
            executes: None,
        };
        let err = Check {
            name: "python",
            status: Status::Error,
            detail: String::new(),
            fix: None,
            executes: None,
        };
        assert!(!overall_ok(&[ok2, err]));
    }
}
