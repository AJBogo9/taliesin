//! The single source of truth for *which* Python/R interpreter a document runs
//! against, and a read-only probe of that interpreter's Jupyter kernel package.
//!
//! Pure and fully unit-testable: resolution is env/field/existence logic (the env
//! read is injected in the testable `*_env` core, so tests never touch process
//! env), and `probe` only introspects the interpreter (`--version`, an import
//! check) — it never runs the user's document. Wiring lives at the build/serve
//! entry points (`serve`, `serve_site`, `build`); this module decides nothing
//! about execution, caching, or kernel lifecycle.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The two languages taliesin can execute against a Jupyter kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Python,
    R,
}

/// Where a resolved interpreter path came from, in precedence order. Carried so the
/// kernel-start log line and `check`'s Environment section can name the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// A `_site.yml` `python:` / `r:` field (highest precedence).
    Field,
    /// A project-local `<dir>/.venv/bin/python` (Python only).
    Venv,
    /// The `TALIESIN_PYTHON` / `TALIESIN_R` env var.
    Env,
    /// The bare `python3` / `R` fallback (no concrete choice was made).
    Default,
}

impl Provenance {
    /// Whether the *project being checked* chose this interpreter, as opposed to the user.
    ///
    /// The distinction only matters where taliesin spawns the binary as a side effect of a
    /// command the user did not think of as executing anything: `check` is the kernel-free,
    /// network-free pass an agent runs first on an unknown project, and a `_site.yml`
    /// `python:` field is a string that project's author wrote (item 81). A `.venv` counts
    /// too — it is the common *legitimate* case, and also just a path inside a directory
    /// someone else may have sent you. `TALIESIN_PYTHON` and the bare `python3` fallback are
    /// the user's own choice, so spawning those is not a surprise.
    pub fn is_project_supplied(self) -> bool {
        matches!(self, Provenance::Field | Provenance::Venv)
    }

    /// Human label naming where the interpreter came from, for the kernel-start log
    /// line and `check`'s Environment section (e.g. `.venv`, `TALIESIN_PYTHON`).
    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Provenance::Field, Lang::Python) => "_site.yml python:",
            (Provenance::Field, Lang::R) => "_site.yml r:",
            (Provenance::Venv, _) => ".venv",
            (Provenance::Env, Lang::Python) => "TALIESIN_PYTHON",
            (Provenance::Env, Lang::R) => "TALIESIN_R",
            (Provenance::Default, Lang::Python) => "python3",
            (Provenance::Default, Lang::R) => "R",
        }
    }
}

/// A resolved interpreter: the path to launch plus where it was chosen from.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub path: PathBuf,
    pub provenance: Provenance,
}

/// Resolve the Python interpreter: `field` -> `<dir>/.venv/bin/python` (then
/// `.venv/bin/python3`) -> `TALIESIN_PYTHON` -> `python3`. First existing match wins.
pub fn resolve_python(field: Option<&str>, project_dir: &Path) -> Resolved {
    resolve_python_env(
        field,
        project_dir,
        std::env::var_os("TALIESIN_PYTHON").as_deref(),
    )
}

/// Resolve the R interpreter: `field` -> `TALIESIN_R` -> `R`. No `.venv` step (there is
/// no R venv convention). `project_dir` is accepted for signature symmetry.
pub fn resolve_r(field: Option<&str>, project_dir: &Path) -> Resolved {
    resolve_r_env(
        field,
        project_dir,
        std::env::var_os("TALIESIN_R").as_deref(),
    )
}

/// Testable core of [`resolve_python`] with the env value injected.
fn resolve_python_env(field: Option<&str>, project_dir: &Path, env: Option<&OsStr>) -> Resolved {
    if let Some(f) = field.map(str::trim).filter(|s| !s.is_empty()) {
        return Resolved {
            path: PathBuf::from(f),
            provenance: Provenance::Field,
        };
    }
    for cand in [".venv/bin/python", ".venv/bin/python3"] {
        let p = project_dir.join(cand);
        if p.exists() {
            return Resolved {
                path: p,
                provenance: Provenance::Venv,
            };
        }
    }
    if let Some(env) = env {
        return Resolved {
            path: PathBuf::from(env),
            provenance: Provenance::Env,
        };
    }
    Resolved {
        path: PathBuf::from("python3"),
        provenance: Provenance::Default,
    }
}

/// Testable core of [`resolve_r`] with the env value injected.
fn resolve_r_env(field: Option<&str>, _project_dir: &Path, env: Option<&OsStr>) -> Resolved {
    if let Some(f) = field.map(str::trim).filter(|s| !s.is_empty()) {
        return Resolved {
            path: PathBuf::from(f),
            provenance: Provenance::Field,
        };
    }
    if let Some(env) = env {
        return Resolved {
            path: PathBuf::from(env),
            provenance: Provenance::Env,
        };
    }
    Resolved {
        path: PathBuf::from("R"),
        provenance: Provenance::Default,
    }
}

/// A read-only introspection of a resolved interpreter: does it run, what version,
/// and is its Jupyter kernel package importable. Never executes the user's document.
#[derive(Debug, Clone)]
pub struct Probe {
    /// The interpreter binary spawned and returned a version (it exists + runs).
    pub runs: bool,
    /// The `--version` string, trimmed, when `runs`.
    pub version: Option<String>,
    /// `ipykernel` (Python) / `IRkernel` (R) imported cleanly.
    pub kernel_pkg_ok: bool,
    /// The captured failure (spawn error, or the interpreter's stderr on a failed
    /// import), for the human/JSON report. `None` when everything succeeded.
    pub error: Option<String>,
}

/// Probe a resolved interpreter: `<bin> --version`, then an import of its Jupyter
/// kernel package. Tolerates a missing binary / import error (never panics, never
/// blocks `check`'s exit code).
pub fn probe(resolved: &Resolved, lang: Lang) -> Probe {
    use std::process::Command;
    let bin = &resolved.path;

    // 1. Version / runnability. A spawn failure (binary absent) is captured, not fatal.
    let (runs, version, mut error) = match Command::new(bin).arg("--version").output() {
        Ok(out) => {
            // Python prints version on stdout (3.4+) or stderr (older); R on stdout.
            let mut v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if v.is_empty() {
                v = String::from_utf8_lossy(&out.stderr).trim().to_string();
            }
            let v = v.lines().next().unwrap_or("").trim().to_string();
            (true, (!v.is_empty()).then_some(v), None)
        }
        Err(e) => (
            false,
            None,
            Some(format!("cannot run {}: {e}", bin.display())),
        ),
    };

    // 2. Kernel-package import (only if the binary runs). Environment introspection
    //    only: importing ipykernel/IRkernel does not run the document.
    let mut kernel_pkg_ok = false;
    if runs {
        let import = match lang {
            Lang::Python => Command::new(bin).args(["-c", "import ipykernel"]).output(),
            // `--vanilla` keeps startup deterministic; `-e` runs the import statement.
            Lang::R => Command::new(bin)
                .args(["--vanilla", "--slave", "-e", "library(IRkernel)"])
                .output(),
        };
        match import {
            Ok(out) if out.status.success() => kernel_pkg_ok = true,
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                error = Some(if msg.is_empty() {
                    "kernel package import failed".to_string()
                } else {
                    msg.lines().last().unwrap_or(&msg).to_string()
                });
            }
            Err(e) => error = Some(format!("cannot run {}: {e}", bin.display())),
        }
    }

    Probe {
        runs,
        version,
        kernel_pkg_ok,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    // A temp dir with a `.venv/bin/python` file present (existence-only check).
    fn venv_dir(name: &str, exe: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-interp-{name}"));
        let bin = dir.join(".venv/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(exe), b"#!/bin/sh\n").unwrap();
        dir
    }

    #[test]
    fn venv_beats_a_set_env_python() {
        // The actual 2026-07-11 bug: a global TALIESIN_PYTHON must NOT win over a
        // project-local .venv.
        let dir = venv_dir("venv-wins", "python");
        let r = resolve_python_env(None, &dir, Some(OsStr::new("/usr/bin/python3")));
        assert_eq!(r.provenance, Provenance::Venv);
        assert_eq!(r.path, dir.join(".venv/bin/python"));
    }

    #[test]
    fn field_beats_venv() {
        let dir = venv_dir("field-wins", "python");
        let r = resolve_python_env(Some("/opt/py/bin/python"), &dir, Some(OsStr::new("/x")));
        assert_eq!(r.provenance, Provenance::Field);
        assert_eq!(r.path, std::path::PathBuf::from("/opt/py/bin/python"));
    }

    #[test]
    fn venv_prefers_python_over_python3() {
        let dir = venv_dir("prefer-python", "python");
        std::fs::write(dir.join(".venv/bin/python3"), b"#!/bin/sh\n").unwrap();
        let r = resolve_python_env(None, &dir, None);
        assert_eq!(r.path, dir.join(".venv/bin/python"));
    }

    #[test]
    fn falls_to_env_then_default_python() {
        let empty = std::env::temp_dir().join("tali-interp-empty");
        std::fs::create_dir_all(&empty).unwrap();
        let with_env = resolve_python_env(None, &empty, Some(OsStr::new("/usr/local/bin/python")));
        assert_eq!(with_env.provenance, Provenance::Env);
        let no_env = resolve_python_env(None, &empty, None);
        assert_eq!(no_env.provenance, Provenance::Default);
        assert_eq!(no_env.path, std::path::PathBuf::from("python3"));
    }

    #[test]
    fn resolve_r_has_no_venv_step() {
        // A .venv beside the project must NOT be picked for R (no R venv convention).
        let dir = venv_dir("r-no-venv", "python");
        let with_env = resolve_r_env(None, &dir, Some(OsStr::new("/usr/bin/R")));
        assert_eq!(with_env.provenance, Provenance::Env);
        let no_env = resolve_r_env(None, &dir, None);
        assert_eq!(no_env.provenance, Provenance::Default);
        assert_eq!(no_env.path, std::path::PathBuf::from("R"));
    }

    #[test]
    fn provenance_labels() {
        assert_eq!(Provenance::Field.label(Lang::Python), "_site.yml python:");
        assert_eq!(Provenance::Field.label(Lang::R), "_site.yml r:");
        assert_eq!(Provenance::Venv.label(Lang::Python), ".venv");
        assert_eq!(Provenance::Env.label(Lang::Python), "TALIESIN_PYTHON");
        assert_eq!(Provenance::Env.label(Lang::R), "TALIESIN_R");
        assert_eq!(Provenance::Default.label(Lang::Python), "python3");
        assert_eq!(Provenance::Default.label(Lang::R), "R");
    }

    #[test]
    fn probe_of_a_missing_binary_reports_not_runnable_without_panicking() {
        let r = Resolved {
            path: std::path::PathBuf::from("/nonexistent/tali/python-xyz"),
            provenance: Provenance::Field,
        };
        let p = probe(&r, Lang::Python);
        assert!(!p.runs, "a missing binary must not report as runnable");
        assert!(!p.kernel_pkg_ok);
        assert!(
            p.error.is_some(),
            "a spawn failure is captured, not swallowed"
        );
    }

    #[test]
    fn probe_reports_ipykernel_against_a_real_python() {
        // Kernel-gated: without TALIESIN_PYTHON this asserts nothing (no live interp).
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            eprintln!("SKIPPED (no live interpreter): set TALIESIN_PYTHON to probe ipykernel");
            return;
        };
        let r = Resolved {
            path: py.into(),
            provenance: Provenance::Env,
        };
        let p = probe(&r, Lang::Python);
        assert!(p.runs, "a real python should run --version");
        assert!(p.version.is_some(), "version string captured");
        // kernel_pkg_ok reflects reality; we only assert that when false an error is
        // captured (the section is informational, so availability is not asserted).
        if !p.kernel_pkg_ok {
            assert!(p.error.is_some());
        }
    }
}
