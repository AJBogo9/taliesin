//! The single source of truth for *which* Python interpreter a document runs
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

/// Where a resolved interpreter path came from, in precedence order. Carried so the
/// kernel-start log line and `check`'s Environment section can name the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// A `_site.yml` `python:` field (highest precedence).
    Field,
    /// A project-local `<dir>/.venv/bin/python`.
    Venv,
    /// The `TALIESIN_PYTHON` env var.
    Env,
    /// A `.venv` found by walking *up* from the project dir, e.g. the repository root's
    /// venv for a book that lives at `docs/book`.
    AncestorVenv,
    /// The bare `python3` fallback (no concrete choice was made).
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
    ///
    /// An [`AncestorVenv`](Provenance::AncestorVenv) counts as project-supplied too, and the
    /// call is deliberate. It is tempting to read an ancestor venv as "above the payload,
    /// therefore mine" — but the walk starts at the *target* the user named, and
    /// `check untrusted/docs/book` climbs through `untrusted/`, so the venv it finds can sit
    /// squarely inside the directory someone sent you. The two cases are indistinguishable
    /// from the path alone, so this fails closed: the conservative branch costs only a live
    /// probe inside `check`, which `doctor` still performs on demand.
    pub fn is_project_supplied(self) -> bool {
        matches!(
            self,
            Provenance::Field | Provenance::Venv | Provenance::AncestorVenv
        )
    }

    /// Human label naming where the interpreter came from, for the kernel-start log
    /// line and `check`'s Environment section (e.g. `.venv`, `TALIESIN_PYTHON`).
    pub fn label(self) -> &'static str {
        match self {
            Provenance::Field => "_site.yml python:",
            Provenance::Venv => ".venv",
            Provenance::Env => "TALIESIN_PYTHON",
            Provenance::AncestorVenv => "ancestor .venv",
            Provenance::Default => "python3",
        }
    }
}

/// The directory names that fence the upward `.venv` walk. A marker directory is
/// **probed before the walk stops on it** — the venv that matters most in practice sits
/// beside the `.git` at a repository root, and a stop-then-probe order would be the one
/// arrangement that never resolves.
const BOUNDARY_MARKERS: &[&str] = &[".git", "pyproject.toml"];

/// The record of the upward `.venv` walk: what it looked at, where it stopped and why.
/// Carried on every Python [`Resolved`] even when a higher-precedence source won, so
/// `doctor` can explain *why* a venv the author can see was not the one picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VenvSearch {
    /// The ancestor directories examined, nearest first. Excludes the project dir
    /// itself, which the higher-precedence [`Provenance::Venv`] step already probed.
    pub examined: Vec<PathBuf>,
    /// The last directory examined (the marker directory, or the filesystem root).
    pub stopped_at: PathBuf,
    /// The marker that halted the walk, or `None` when it ran out of parents.
    pub stopped_by: Option<&'static str>,
    /// The interpreter found, if any.
    pub found: Option<PathBuf>,
}

impl VenvSearch {
    /// One line naming what the upward walk examined and where it stopped, shared by the
    /// build failure, `doctor` and `check`. It is the answer to "why didn't it use the
    /// venv I can see?", which is otherwise unanswerable without reading this file.
    pub fn summary(&self) -> String {
        let looked = if self.examined.is_empty() {
            "no ancestors".to_string()
        } else {
            self.examined
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "searched {looked}; stopped at {}{}",
            self.stopped_at.display(),
            self.stopped_by.map_or(String::new(), |m| format!(" ({m})")),
        )
    }
}

/// Every source resolution consulted, in precedence order, recorded by the resolver
/// itself. This is the material for the "no kernel available" build error and for
/// `doctor`/`check`; producing it *as the decision is made* is what keeps the report
/// from drifting away from the logic it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trail {
    /// The `_site.yml` `python:` field, already resolved against the project dir.
    pub field: Option<PathBuf>,
    /// `<project_dir>/.venv/bin/python{,3}`, when present.
    pub local_venv: Option<PathBuf>,
    /// `TALIESIN_PYTHON`, when set.
    pub env: Option<PathBuf>,
    /// The upward walk.
    pub ancestor: Option<VenvSearch>,
    /// The bare `python3` last resort.
    pub fallback: PathBuf,
}

impl Trail {
    /// The ordered, human-readable "here is everything I looked at" report, for the
    /// build's hard failure and for `doctor`. One line per source, in the precedence
    /// order the resolver actually applied.
    pub fn report(&self, chosen: Provenance) -> String {
        let mark = |p: Provenance| if p == chosen { " <- used" } else { "" };
        let shown = |o: &Option<PathBuf>| match o {
            Some(p) => p.display().to_string(),
            None => "not set".to_string(),
        };
        let mut lines = vec![format!(
            "  1. {:<22} {}{}",
            Provenance::Field.label(),
            shown(&self.field),
            mark(Provenance::Field)
        )];
        lines.push(format!(
            "  2. {:<22} {}{}",
            "<project>/.venv",
            self.local_venv
                .as_ref()
                .map_or_else(|| "not found".to_string(), |p| p.display().to_string()),
            mark(Provenance::Venv)
        ));
        lines.push(format!(
            "  {}. {:<22} {}{}",
            lines.len() + 1,
            Provenance::Env.label(),
            shown(&self.env),
            mark(Provenance::Env)
        ));
        if let Some(s) = &self.ancestor {
            let where_ = s.summary();
            lines.push(format!(
                "  {}. {:<22} {}{}\n       ({where_})",
                lines.len() + 1,
                Provenance::AncestorVenv.label(),
                s.found
                    .as_ref()
                    .map_or_else(|| "not found".to_string(), |p| p.display().to_string()),
                mark(Provenance::AncestorVenv)
            ));
        }
        lines.push(format!(
            "  {}. {:<22} {}{}",
            lines.len() + 1,
            Provenance::Default.label(),
            self.fallback.display(),
            mark(Provenance::Default)
        ));
        lines.join("\n")
    }
}

/// A resolved interpreter: the path to launch, where it was chosen from, and the full
/// record of what else was considered.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub path: PathBuf,
    pub provenance: Provenance,
    pub trail: Trail,
}

impl Resolved {
    /// A `Resolved` for an interpreter chosen *outside* the search — the shape the
    /// `doctor`/`exec` unit tests need, where a concrete path is handed in directly and
    /// no source was ever consulted. Test-only: production code must go through
    /// [`resolve_python`], which is the whole point of this module.
    #[cfg(test)]
    pub(crate) fn fixed(path: impl Into<PathBuf>, provenance: Provenance) -> Self {
        Resolved {
            path: path.into(),
            provenance,
            trail: Trail {
                field: None,
                local_venv: None,
                env: None,
                ancestor: None,
                fallback: PathBuf::from("python3"),
            },
        }
    }
}

/// Resolve the Python interpreter. First match wins, in this order:
///
/// ```text
/// field  ->  <dir>/.venv  ->  TALIESIN_PYTHON  ->  ancestor .venv  ->  python3
/// ```
///
/// (`.venv` means `.venv/bin/python`, then `.venv/bin/python3`.) A relative `field`
/// resolves against `project_dir` — see [`field_path`]; the ancestor step is the upward
/// walk in [`ancestor_venv`].
///
/// **Why `TALIESIN_PYTHON` sits between the two venv steps.** The 2026-07-11 regression
/// this ordering protects (`venv_beats_a_set_env_python`) was specifically about a
/// *site-local* venv losing to a *global* env var: the author put a `.venv` beside the
/// project, and a stale `TALIESIN_PYTHON` exported in a shell profile silently won. That
/// rationale does not extend to a venv found several directories up. An env var is an
/// interpreter the user set on purpose, for this shell, now; a venv three levels above
/// the project is an inference from the filesystem. Letting the inference beat the
/// deliberate choice is the more surprising failure, and the harder one to debug.
///
/// This also matches uv, which is the closest current prior art: its docs rank a
/// discovered `.venv` above interpreters found on `PATH`, not above an explicit
/// `--python` / `UV_PYTHON` request — and `TALIESIN_PYTHON` is this tool's `UV_PYTHON`.
///
/// The ancestor walk runs even when something above it already won, so the losing venv
/// is still recorded in [`Trail`] and `doctor` can say *why* the venv the author can see
/// was not the one used. That is the debuggability this ordering owes the reader.
pub fn resolve_python(field: Option<&str>, project_dir: &Path) -> Resolved {
    resolve_python_env(
        field,
        project_dir,
        std::env::var_os("TALIESIN_PYTHON").as_deref(),
    )
}

/// Testable core of [`resolve_python`] with the env value injected.
fn resolve_python_env(field: Option<&str>, project_dir: &Path, env: Option<&OsStr>) -> Resolved {
    let dir = taliesin_core::includes::absolutize(project_dir);
    let trail = Trail {
        field: field_path(field, &dir),
        local_venv: local_venv(&dir),
        env: env.map(PathBuf::from),
        // Computed even when a higher-precedence source already won. It is a handful of
        // `exists()` calls, and recording the venv that *lost* is what lets `doctor`
        // answer "why isn't it using the venv I can see?" instead of silently omitting it.
        ancestor: Some(ancestor_venv(&dir)),
        fallback: PathBuf::from("python3"),
    };
    let (path, provenance) = if let Some(f) = trail.field.clone() {
        (f, Provenance::Field)
    } else if let Some(v) = trail.local_venv.clone() {
        (v, Provenance::Venv)
    } else if let Some(e) = trail.env.clone() {
        (e, Provenance::Env)
    } else if let Some(a) = trail.ancestor.as_ref().and_then(|s| s.found.clone()) {
        (a, Provenance::AncestorVenv)
    } else {
        (trail.fallback.clone(), Provenance::Default)
    };
    Resolved {
        path,
        provenance,
        trail,
    }
}

/// `<dir>/.venv/bin/python`, then `<dir>/.venv/bin/python3`; the first that exists.
fn local_venv(dir: &Path) -> Option<PathBuf> {
    [".venv/bin/python", ".venv/bin/python3"]
        .into_iter()
        .map(|c| dir.join(c))
        .find(|p| p.exists())
}

/// A `python:` / `r:` field, resolved the way every config format resolves a relative
/// path: **against the directory holding the config file**, never the process cwd
/// (Cargo.toml, tsconfig.json, ESLint, ruff.toml and Quarto's `_quarto.yml` all do this).
/// Without it the only field value that ever worked was an absolute path — which is
/// machine-specific, so it cannot be committed, which is why declaring the venv in-repo
/// was impossible and per-machine symlinks and wrapper scripts were the only way through.
///
/// Two values are deliberately left untouched:
/// - an **absolute** path, which already means one thing everywhere;
/// - a **bare program name** (`python3.12` — no separator), which `Command::new` resolves
///   against `PATH`, not against the cwd. It was never the broken case, and re-basing it
///   onto the project dir would break a working config.
fn field_path(field: Option<&str>, dir: &Path) -> Option<PathBuf> {
    let f = field.map(str::trim).filter(|s| !s.is_empty())?;
    let p = Path::new(f);
    if p.is_absolute() || !f.contains(std::path::MAIN_SEPARATOR) {
        return Some(p.to_path_buf());
    }
    Some(taliesin_core::includes::absolutize(&dir.join(p)))
}

/// Walk *up* from `dir` looking for a `.venv`, because a book almost never sits at the
/// project root: the site is `docs/explainers/` and the venv is at the repository root
/// two levels above. Probing only `<dir>/.venv` is what forced authors to hand-create an
/// untracked symlink plus a wrapper script exporting `TALIESIN_PYTHON`.
///
/// Upward search to a project boundary is the standard shape — cargo walks up for
/// `Cargo.toml`, npm walks up `node_modules`, pytest/ruff/mypy walk up for their config,
/// and uv's docs say a `.venv` "in the working directory or any of the parent
/// directories" is used. This is bounded twice over: it stops at the first
/// [`BOUNDARY_MARKERS`] directory, and failing that when it runs out of parents.
/// `Path::parent` strictly shortens the path, so either way it terminates in at most
/// `dir`'s component count — the walk is lexical, so no symlink can make it loop.
///
/// Two ordering details carry the whole feature:
/// - a marker directory is **probed before the walk stops on it**, because the venv that
///   matters in practice sits *beside* the `.git` at a repo root; stop-then-probe would
///   miss precisely the arrangement this exists for;
/// - the starting directory is examined for a marker but never probed for a `.venv` —
///   the higher-precedence [`Provenance::Venv`] step already owns that, and a project
///   that is its own checkout must still not borrow the venv of whatever encloses it.
fn ancestor_venv(dir: &Path) -> VenvSearch {
    let mut examined = Vec::new();
    let mut cur = dir.to_path_buf();
    loop {
        let stop = BOUNDARY_MARKERS
            .iter()
            .find(|m| cur.join(m).exists())
            .copied();
        if cur != dir {
            examined.push(cur.clone());
            if let Some(found) = local_venv(&cur) {
                return VenvSearch {
                    examined,
                    stopped_at: cur,
                    stopped_by: stop,
                    found: Some(found),
                };
            }
        }
        if stop.is_some() {
            return VenvSearch {
                examined,
                stopped_at: cur,
                stopped_by: stop,
                found: None,
            };
        }
        match cur.parent() {
            Some(p) if !p.as_os_str().is_empty() => cur = p.to_path_buf(),
            _ => {
                return VenvSearch {
                    examined,
                    stopped_at: cur,
                    stopped_by: None,
                    found: None,
                };
            }
        }
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
pub fn probe(resolved: &Resolved) -> Probe {
    use std::process::Command;
    let bin = &resolved.path;

    // 1. Version / runnability. A spawn failure (binary absent) is captured, not fatal.
    let (runs, version, mut error) = match Command::new(bin).arg("--version").output() {
        Ok(out) => {
            // Python prints its version on stdout (3.4+) or on stderr (older).
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
    //    only: importing ipykernel does not run the document.
    let mut kernel_pkg_ok = false;
    if runs {
        match Command::new(bin).args(["-c", "import ipykernel"]).output() {
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

    // A fresh temp project tree, **wiped first**. The upward-walk tests are decided by
    // which markers exist, so inheriting a previous run's `.git` would silently move the
    // boundary and make a green run meaningless.
    fn tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tali-interp-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    // `<dir>/.venv/bin/<exe>` (existence-only check, as resolution does).
    fn venv_at(dir: &Path, exe: &str) {
        let bin = dir.join(".venv/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(exe), b"#!/bin/sh\n").unwrap();
    }

    // A project-boundary marker (`.git`, `pyproject.toml`) in `dir`.
    fn marker(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), b"").unwrap();
    }

    // `<root>/docs/book`, the shape the whole feature exists for.
    fn book_under(root: &Path) -> PathBuf {
        let book = root.join("docs/book");
        std::fs::create_dir_all(&book).unwrap();
        book
    }

    #[test]
    fn finds_a_venv_at_the_repo_root_from_a_book_two_levels_down() {
        // The acceptance case: `<root>/.venv` with the book at `<root>/docs/book`, no env
        // var and no symlink. Note the marker sits in the SAME directory as the venv —
        // the walk must probe a boundary directory before stopping on it, or the one
        // arrangement that matters in practice (a venv beside `.git` at a repo root)
        // would be the one arrangement that never resolves.
        let root = tree("ancestor-hit");
        venv_at(&root, "python");
        marker(&root, ".git");
        let r = resolve_python_env(None, &book_under(&root), None);
        assert_eq!(r.provenance, Provenance::AncestorVenv);
        assert_eq!(r.path, root.join(".venv/bin/python"));
    }

    #[test]
    fn upward_search_stops_at_a_project_marker() {
        // A `.git` at `docs/` fences the walk: `<root>/.venv` above it belongs to a
        // different project, so it must not be borrowed.
        let root = tree("ancestor-boundary");
        venv_at(&root, "python");
        let book = book_under(&root);
        marker(&root.join("docs"), ".git");
        let r = resolve_python_env(None, &book, None);
        assert_eq!(r.provenance, Provenance::Default);
        let s = r.trail.ancestor.expect("python always records its walk");
        assert_eq!(s.stopped_at, root.join("docs"));
        assert_eq!(s.stopped_by, Some(".git"));
        assert!(s.found.is_none(), "the fenced-off venv must not be picked");
    }

    #[test]
    fn pyproject_toml_fences_the_walk_too() {
        let root = tree("ancestor-pyproject");
        venv_at(&root, "python");
        let book = book_under(&root);
        marker(&root.join("docs"), "pyproject.toml");
        let r = resolve_python_env(None, &book, None);
        assert_eq!(r.provenance, Provenance::Default);
        assert_eq!(r.trail.ancestor.unwrap().stopped_by, Some("pyproject.toml"));
    }

    #[test]
    fn upward_search_can_miss_and_fall_through() {
        let root = tree("ancestor-miss");
        marker(&root, ".git");
        let r = resolve_python_env(None, &book_under(&root), None);
        assert_eq!(r.provenance, Provenance::Default);
        assert_eq!(r.path, PathBuf::from("python3"));
        let s = r.trail.ancestor.unwrap();
        assert!(s.found.is_none());
        // Nearest-first, and it stopped on the marker rather than climbing to `/`.
        assert_eq!(s.examined, vec![root.join("docs"), root.clone()]);
    }

    #[test]
    fn a_set_env_python_beats_an_ancestor_venv() {
        // The 2026-07-11 fix (`venv_beats_a_set_env_python`) is about a venv the author
        // put *beside the project*. A venv several directories up is a weaker signal
        // than an interpreter the user set on purpose, so the env var wins there. This
        // matches uv, whose docs rank a discovered `.venv` above interpreters found on
        // PATH — not above an explicit `--python`/`UV_PYTHON` request.
        let root = tree("ancestor-vs-env");
        venv_at(&root, "python");
        marker(&root, ".git");
        let r = resolve_python_env(
            None,
            &book_under(&root),
            Some(OsStr::new("/usr/bin/python3")),
        );
        assert_eq!(r.provenance, Provenance::Env);
        // ...and the loser is still recorded, so `doctor` can explain the surprise
        // instead of leaving the author staring at a venv that was silently skipped.
        assert_eq!(
            r.trail.ancestor.unwrap().found,
            Some(root.join(".venv/bin/python"))
        );
    }

    #[test]
    fn a_project_local_venv_beats_an_ancestor_one() {
        let root = tree("local-vs-ancestor");
        venv_at(&root, "python");
        let book = book_under(&root);
        venv_at(&book, "python");
        let r = resolve_python_env(None, &book, None);
        assert_eq!(r.provenance, Provenance::Venv);
        assert_eq!(r.path, book.join(".venv/bin/python"));
    }

    #[test]
    fn a_relative_field_resolves_against_the_project_dir() {
        // `python: "../../.venv/bin/python"` written in `<root>/docs/book/_site.yml`
        // means `<root>/.venv/bin/python` from ANY cwd. Paths in a config file resolve
        // against that file's directory (Cargo.toml, tsconfig.json, ruff.toml and
        // Quarto's `_quarto.yml` all behave this way); resolving against the process cwd
        // left an absolute path as the only value that ever worked, which is exactly the
        // value that cannot be committed.
        let root = tree("relative-field");
        venv_at(&root, "python");
        let r = resolve_python_env(Some("../../.venv/bin/python"), &book_under(&root), None);
        assert_eq!(r.provenance, Provenance::Field);
        assert_eq!(r.path, root.join(".venv/bin/python"));
        assert!(
            r.path.is_absolute(),
            "an absolute result is what makes it cwd-independent"
        );
    }

    #[test]
    fn an_absolute_field_is_left_exactly_as_written() {
        let root = tree("absolute-field");
        let r = resolve_python_env(Some("/opt/py/bin/python"), &book_under(&root), None);
        assert_eq!(r.path, PathBuf::from("/opt/py/bin/python"));
    }

    #[test]
    fn an_ancestor_venv_is_treated_as_project_supplied() {
        // Fails closed: the walk climbs through the target the user named, so an
        // "ancestor" venv can still be inside a directory someone else sent you.
        assert!(Provenance::AncestorVenv.is_project_supplied());
        assert_eq!(Provenance::AncestorVenv.label(), "ancestor .venv");
    }

    #[test]
    fn the_report_names_every_source_in_precedence_order() {
        let root = tree("report-order");
        venv_at(&root, "python");
        marker(&root, ".git");
        let r = resolve_python_env(None, &book_under(&root), None);
        let report = r.trail.report(r.provenance);
        // Order is the contract: an error that lists sources out of order teaches the
        // reader the wrong precedence.
        let idx = |needle: &str| {
            report
                .find(needle)
                .unwrap_or_else(|| panic!("report must name {needle}:\n{report}"))
        };
        assert!(idx("_site.yml python:") < idx("<project>/.venv"));
        assert!(idx("<project>/.venv") < idx("TALIESIN_PYTHON"));
        assert!(idx("TALIESIN_PYTHON") < idx("ancestor .venv"));
        assert!(idx("ancestor .venv") < idx("python3"));
        assert!(
            report.contains("<- used"),
            "the winner is marked:\n{report}"
        );
        assert!(
            report.contains(&root.display().to_string()),
            "the report names where the walk stopped:\n{report}"
        );
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
    fn provenance_labels() {
        assert_eq!(Provenance::Field.label(), "_site.yml python:");
        assert_eq!(Provenance::Venv.label(), ".venv");
        assert_eq!(Provenance::Env.label(), "TALIESIN_PYTHON");
        assert_eq!(Provenance::AncestorVenv.label(), "ancestor .venv");
        assert_eq!(Provenance::Default.label(), "python3");
    }

    #[test]
    fn probe_of_a_missing_binary_reports_not_runnable_without_panicking() {
        let r = Resolved::fixed("/nonexistent/tali/python-xyz", Provenance::Field);
        let p = probe(&r);
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
        let r = Resolved::fixed(PathBuf::from(py), Provenance::Env);
        let p = probe(&r);
        assert!(p.runs, "a real python should run --version");
        assert!(p.version.is_some(), "version string captured");
        // kernel_pkg_ok reflects reality; we only assert that when false an error is
        // captured (the section is informational, so availability is not asserted).
        if !p.kernel_pkg_ok {
            assert!(p.error.is_some());
        }
    }
}
