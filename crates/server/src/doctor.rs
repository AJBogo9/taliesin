//! `taliesin doctor`: a standalone environment self-audit. Surfaces the interpreter +
//! kernel-package probe (`crate::interpreter`) unconditionally for both Python and R, plus
//! active conda/virtualenv detection and `_site.yml` sanity, with ✓/⚠/✗ status and fix
//! commands. Answers "is my environment ready to run code cells?" before any document exists
//! (the probe inside `check` only runs for languages a doc already uses, which is circular).
//!
//! Pure core (`interpreter_check`/`active_env_check`/`overall_ok`): probe results + env vars
//! are injected, so the severity logic is unit-tested without spawning or touching process
//! env. `cmd_doctor` is the thin I/O wrapper. Never executes the user's document.

use crate::interpreter::{Lang, Probe, Provenance, Resolved};
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
}

/// The `pip install` / `install.packages` line that installs the missing kernel package into
/// the exact interpreter that was probed.
fn kernel_install_fix(lang: Lang, r: &Resolved) -> String {
    match lang {
        Lang::Python => format!("{} -m pip install ipykernel", r.path.display()),
        Lang::R => format!("{} -e \"install.packages('IRkernel')\"", r.path.display()),
    }
}

/// The fix when the *default* interpreter is simply absent (nothing was configured).
fn absent_default_fix(lang: Lang) -> &'static str {
    match lang {
        Lang::Python => "install Python 3, then: python3 -m pip install ipykernel",
        Lang::R => "install R, then in R: install.packages(\"IRkernel\")",
    }
}

/// Map a resolved interpreter + its probe to one audit line (pure; the severity model).
fn interpreter_check(lang: Lang, r: &Resolved, p: &Probe) -> Check {
    let name = match lang {
        Lang::Python => "python",
        Lang::R => "r",
    };
    let pkg = match lang {
        Lang::Python => "ipykernel",
        Lang::R => "IRkernel",
    };
    let where_ = format!("{} ({})", r.path.display(), r.provenance.label(lang));
    let ver = p
        .version
        .clone()
        .map(|v| format!("{v}  ·  "))
        .unwrap_or_default();
    if p.runs && p.kernel_pkg_ok {
        return Check {
            name,
            status: Status::Ok,
            detail: format!("{where_}\n{ver}{pkg} present"),
            fix: None,
        };
    }
    if p.runs {
        return Check {
            name,
            status: Status::Warn,
            detail: format!("{where_}\n{ver}{pkg} MISSING"),
            fix: Some(kernel_install_fix(lang, r)),
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
            detail: format!("{where_}\n{err}  ·  {name} cells will render as source"),
            fix: Some(absent_default_fix(lang).to_string()),
        }
    } else {
        // A pointed-at interpreter (env / field / .venv) that is broken: a real error.
        Check {
            name,
            status: Status::Error,
            detail: format!("{where_}\n{err}"),
            fix: Some(format!(
                "point {} at a real interpreter, or unset it",
                r.provenance.label(lang)
            )),
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
    }
}

/// The audit passes (exit 0) unless a configured interpreter is broken (a `Status::Error`).
fn overall_ok(checks: &[Check]) -> bool {
    !checks.iter().any(|c| c.status == Status::Error)
}

const DOCTOR_FLAGS: &[&str] = &["--format"];

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
    let say =
        |name: &str, lang: &str| match checks.iter().find(|c| c.name == name).map(|c| c.status) {
            Some(Status::Ok) => format!("{lang} cells will execute"),
            _ => format!("{lang} cells will render as source"),
        };
    format!("{}; {}.", say("python", "python"), say("r", "R"))
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

fn print_json(checks: &[Check]) {
    let payload = serde_json::json!({
        "ok": overall_ok(checks),
        "checks": checks.iter().map(|c| serde_json::json!({
            "name": c.name,
            "status": c.status.json(),
            "detail": c.detail,
            "fix": c.fix,
        })).collect::<Vec<_>>(),
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
                    eprintln!(
                        "error: --format expects human or json (got {})",
                        other.unwrap_or("nothing")
                    );
                    return ExitCode::FAILURE;
                }
            },
            s if s.starts_with("--") => {
                crate::log::error(&crate::serve::unknown_flag_error(s, DOCTOR_FLAGS));
                return ExitCode::FAILURE;
            }
            s => dir = s.to_string(),
        }
    }
    let dir = Path::new(&dir);

    // Honour an `_site.yml` python:/r: field + its config sanity, exactly as a build would; a
    // single-doc project (no `_site.yml`) has no field pins and no config check.
    let site = dir
        .join("_site.yml")
        .exists()
        .then(|| taliesin_core::Site::discover(dir));
    let py = crate::interpreter::resolve_python(
        site.as_ref().and_then(|s| s.config.python.as_deref()),
        dir,
    );
    let r = crate::interpreter::resolve_r(site.as_ref().and_then(|s| s.config.r.as_deref()), dir);
    let py_probe = crate::interpreter::probe(&py, Lang::Python);
    let r_probe = crate::interpreter::probe(&r, Lang::R);

    let venv = std::env::var("VIRTUAL_ENV").ok();
    let conda_prefix = std::env::var("CONDA_PREFIX").ok();
    let conda_name = std::env::var("CONDA_DEFAULT_ENV").ok();

    let mut checks = vec![
        interpreter_check(Lang::Python, &py, &py_probe),
        interpreter_check(Lang::R, &r, &r_probe),
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
            },
            Some(w) => Check {
                name: "config",
                status: Status::Warn,
                detail: format!("_site.yml: {w}"),
                fix: Some("fix the YAML in _site.yml".to_string()),
            },
        });
    }

    if json {
        print_json(&checks);
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
        Resolved {
            path: PathBuf::from(path),
            provenance,
        }
    }

    #[test]
    fn ready_interpreter_is_ok() {
        let p = Probe {
            runs: true,
            version: Some("Python 3.11.4".into()),
            kernel_pkg_ok: true,
            error: None,
        };
        let c = interpreter_check(Lang::Python, &resolved("python3", Provenance::Default), &p);
        assert_eq!(c.status, Status::Ok);
        assert!(c.detail.contains("ipykernel present"), "{}", c.detail);
        assert!(c.fix.is_none());
    }

    #[test]
    fn missing_kernel_pkg_warns_with_install_fix() {
        let p = Probe {
            runs: true,
            version: Some("Python 3.11".into()),
            kernel_pkg_ok: false,
            error: None,
        };
        let c = interpreter_check(
            Lang::Python,
            &resolved("/proj/.venv/bin/python", Provenance::Venv),
            &p,
        );
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
        let c = interpreter_check(Lang::Python, &resolved("/bad/python", Provenance::Env), &p);
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
            error: Some("cannot run R: No such file or directory".into()),
        };
        let c = interpreter_check(Lang::R, &resolved("R", Provenance::Default), &p);
        assert_eq!(
            c.status,
            Status::Warn,
            "an absent default is not a misconfiguration"
        );
        assert!(c.fix.as_deref().unwrap().contains("IRkernel"));
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
    }

    #[test]
    fn overall_ok_is_false_only_with_an_error() {
        let ok = Check {
            name: "python",
            status: Status::Ok,
            detail: String::new(),
            fix: None,
        };
        let warn = Check {
            name: "r",
            status: Status::Warn,
            detail: String::new(),
            fix: None,
        };
        assert!(overall_ok(&[ok, warn]));
        let ok2 = Check {
            name: "python",
            status: Status::Ok,
            detail: String::new(),
            fix: None,
        };
        let err = Check {
            name: "python",
            status: Status::Error,
            detail: String::new(),
            fix: None,
        };
        assert!(!overall_ok(&[ok2, err]));
    }
}
