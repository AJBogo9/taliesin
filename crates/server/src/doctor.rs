//! `taliesin doctor`: a standalone environment self-audit. Surfaces the Python interpreter +
//! kernel-package probe (`crate::interpreter`) unconditionally, plus `_site.yml` sanity, with
//! ✓/⚠/✗ status and fix commands. Answers "is my environment ready to run code cells?" before
//! any document exists (a document-scoped lint only sees the languages that document already
//! uses, which is circular — and wave 9 removed the interpreter probe from that path
//! entirely).
//!
//! **Every row here can fail.** A separate `env` row printed the active conda/virtualenv at a
//! hard-coded `Status::Ok`, so it was a green ✓ that no environment could ever change — and
//! against the sentence "no active virtual/conda env" it read as "fine" while meaning
//! "nothing is configured". It also contradicted the row above it, which reports the resolved
//! interpreter's provenance (`(.venv)`) and its search trail, which is where that information
//! actually belongs. Deleted 2026-08-10; a tick that cannot vary teaches a reader to stop
//! reading ticks.
//!
//! Pure core (`interpreter_check`/`overall_ok`): probe results are injected, so the severity
//! logic is unit-tested without spawning or touching process env. `cmd_doctor` is the thin
//! I/O wrapper. Never executes the user's document.

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

/// The `config` row, from every diagnostic `_site.yml` itself produced.
///
/// **Every one of them, not only the YAML parse failure.** The row asked
/// `is_malformed_config_warning` and so printed a green `✓ _site.yml is valid` on a file
/// `build --check-only` rejected with two errors: an unknown or typo'd key is read, then
/// dropped, so the site's `title:` silently falls back to a default. A ✓ that means "the
/// YAML parsed" while saying "the config is valid" is the same overclaim the `env` row was
/// deleted for on 2026-08-10, except this one can vary and so looked like it was working.
///
/// Still a ⚠ rather than a ✗ (`doctor` exits non-zero only on a broken *interpreter*): the
/// pre-publish gate for a project is `build <dir> --check-only`, which reports all of these
/// with their line numbers, and the fix line says so rather than repeating the list here.
fn config_check(warnings: &[String]) -> Check {
    // The "no `_site.yml` here" advisory is benign and cannot reach this row anyway (the
    // caller only builds a `Site` when the file exists); filtered so it never could.
    let mut bad = warnings
        .iter()
        .filter(|w| !taliesin_core::site::is_missing_config_warning(w));
    let Some(first) = bad.next() else {
        return Check {
            name: "config",
            status: Status::Ok,
            detail: "_site.yml is valid".to_string(),
            fix: None,
            executes: None,
        };
    };
    let more = match bad.count() {
        0 => String::new(),
        n => format!("\n(+{n} more)"),
    };
    Check {
        name: "config",
        status: Status::Warn,
        detail: format!("_site.yml: {first}{more}"),
        fix: Some("run `taliesin build <dir> --check-only` for the located list".to_string()),
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
    // A path that is not there is a typo, and answering it is worse than refusing it: the
    // interpreter resolution walks UP from `dir` looking for a `.venv`, so
    // `doctor ~/blog/pots` happily reported on `~/blog`'s environment (or on `/`'s) and
    // exited 0. The one question this verb answers is "is THIS project ready", and it
    // cannot be answered about a directory that does not exist. `build` already refuses the
    // same way (Fable audit FA29).
    if !dir.exists() {
        crate::log::error(&format!(
            "cannot read {}: No such file or directory",
            dir.display()
        ));
        return ExitCode::FAILURE;
    }

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

    let mut checks = vec![interpreter_check(&py, &py_probe)];
    if let Some(site) = &site {
        checks.push(config_check(site.config_warnings()));
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

    /// The `/bin/true` lie, at this layer: the probe now reports an exit-0 non-Python as
    /// not-running with the mismatch named (see `interpreter::tests`), and the existing
    /// configured-but-broken path must turn that into a red ✗ whose detail says WHY —
    /// never a green "ipykernel present" beside a summary promising execution.
    #[test]
    fn a_non_python_binary_draws_a_red_verdict_naming_the_mismatch() {
        let p = Probe {
            runs: false,
            version: None,
            kernel_pkg_ok: false,
            error: Some(
                "/bin/true did not identify as Python (`--version` said `true (GNU \
                 coreutils) 9.4`)"
                    .into(),
            ),
        };
        let c = interpreter_check(&resolved("/bin/true", Provenance::Env), &p);
        assert_eq!(c.status, Status::Error);
        assert!(
            c.detail.contains("did not identify as Python"),
            "the detail names the mismatch: {}",
            c.detail
        );
        assert_eq!(c.executes, Some(false));
        assert!(
            summary(std::slice::from_ref(&c)).contains("render as source"),
            "the summary must not promise execution"
        );
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

    /// The readiness summary reads each language's verdict off **that language's** check. It
    /// is the one line most readers act on, and with two checks in the list a lookup that
    /// picks the wrong one still produces a plausible sentence — so a second, non-language
    /// check has to be present for the assertion to mean anything. That second row is
    /// `config`; it was `env` until that row was deleted on 2026-08-10 for being a ✓ that
    /// could never be anything else.
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
            summary(&[check("python", Some(true)), check("config", None)]),
            "python cells will execute."
        );
        // The other direction, so a summary that ignored `executes` altogether — or read
        // the wrong check — cannot pass both halves. An interpreter present but missing
        // its kernel package renders as source, and saying otherwise promises execution
        // that will not happen.
        assert_eq!(
            summary(&[check("python", Some(false)), check("config", None)]),
            "python cells will render as source."
        );
    }

    /// The row must not be greener than `build --check-only`. It reported only the YAML
    /// parse failure, so a `titel:` typo — which is *read*, dropped, and silently defaults
    /// the site title — came back `✓ _site.yml is valid` while `--check-only` on the same
    /// file exited 1 with two errors.
    #[test]
    fn the_config_row_reports_every_site_yml_diagnostic_not_only_bad_yaml() {
        let clean = config_check(&[]);
        assert_eq!(clean.status, Status::Ok);
        assert_eq!(clean.detail, "_site.yml is valid");

        // A typo'd key: no YAML parse failure anywhere in the message.
        let typo = config_check(&[
            "_site.yml:1: unknown config key `titel` (did you mean `title`?)".to_string(),
        ]);
        assert_eq!(typo.status, Status::Warn, "detail: {}", typo.detail);
        assert!(typo.detail.contains("titel"), "{}", typo.detail);
        assert!(
            !typo.detail.contains("is valid"),
            "must not also claim validity: {}",
            typo.detail
        );
        assert!(
            typo.fix.as_deref().unwrap().contains("--check-only"),
            "the fix points at the gate that lists them all: {:?}",
            typo.fix
        );

        // The scheme-less `url:` warning is the one no text filter could have caught: it
        // carries neither the malformed-YAML prefix nor an `_site.yml` prefix at all.
        let url = config_check(&["url: `ex.com` has no scheme — sitemap, robots.txt, feed \
                                  and og:url need an absolute URL"
            .to_string()]);
        assert_eq!(url.status, Status::Warn, "detail: {}", url.detail);

        // Several are summarised, not dumped: the located list belongs to `--check-only`.
        let many = config_check(&["one".to_string(), "two".to_string(), "three".to_string()]);
        assert!(many.detail.contains("one"), "{}", many.detail);
        assert!(many.detail.contains("(+2 more)"), "{}", many.detail);

        // The benign "no _site.yml here" advisory is not a config defect.
        let missing = config_check(&[format!("{} .", taliesin_core::site::MISSING_CONFIG_PREFIX)]);
        assert_eq!(missing.status, Status::Ok, "detail: {}", missing.detail);
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
            name: "config",
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
