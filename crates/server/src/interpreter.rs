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
}
