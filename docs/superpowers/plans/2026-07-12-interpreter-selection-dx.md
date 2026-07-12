# Interpreter selection DX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Python/R interpreter selection **visible** (logged at kernel start, reported by `check`) and **project-reproducible** (a `_site.yml` `python:`/`r:` pin and `.venv` auto-detect that beat a stray global `TALIESIN_PYTHON`), without touching the Do-NOT-touch exec/kernel execution core.

**Architecture:** A new pure `crates/server/src/interpreter.rs` becomes the single resolution+probe truth. Resolution runs once at each of the three entry points (single-doc serve, site serve, build) and is pushed into every `Executor` via a new `set_interpreters` setter (mirroring `set_progress`/`set_warm_pool`) and into the warm pool, so the pool pre-warms the *same* interpreter the executor runs. `exec.rs`/`kernel.rs` execution semantics are unchanged: they only receive already-resolved paths, log one line at first kernel start, and name the path in the failure diagnostic.

**Tech Stack:** Rust (edition 2024, workspace resolver 3), `std::process::Command` for probing, `serde_yaml` for config, existing `crate::log` sink.

## Global Constraints

- **Do-NOT-touch execution core.** What runs, how it caches (cumulative-hash freeze keys), and kernel lifecycle stay byte-identical. Selecting a *different* interpreter busts the freeze cache exactly as a manual env change does today (the interpreter path already feeds `interp_id`); selecting the *same* interpreter by a new route is a cache hit. Add no new cache-invalidation logic.
- **Precedence (approved), first existing match wins.** Python: `_site.yml python:` → `<project_dir>/.venv/bin/python` (then `.venv/bin/python3`) → `TALIESIN_PYTHON` → `python3`. R: `_site.yml r:` → `TALIESIN_R` → `R` (no `.venv` step).
- **`.venv` auto-detect is a pure existence check** (Unix `bin/` only; no `Scripts/`, no `venv`/`virtualenv`/`.env` name sprawl). It never executes the interpreter.
- **Resolution never fails**: it always yields *some* path; a broken choice surfaces through the existing kernel-start failure path, now naming the resolved interpreter.
- **`check` stays a static gate.** The new interpreter probe is *informational* and MUST NOT change `check`'s exit code (a CI box without Python must not fail static linting).
- **Minimal config / perfect-the-default.** With no `python:`/`r:` field the default stays near-perfect (`.venv` → env → `python3`), so most projects need zero config.
- **No em dashes or en dashes** in any code, comment, or doc copy (project + user rule). Use commas/colons/parentheses.
- **rustfmt-clean.** A `PostToolUse` hook runs `rustfmt` on every edited `.rs`; CI enforces it.
- **Env reads must be injectable for tests.** Resolution reads `std::env::var_os` only in the thin public wrapper; the testable core takes the env value as a parameter (Rust 2024 makes `set_var` unsafe and tests run multi-threaded, so never set process env in a test). This mirrors `warm_pool::try_reserve_slot`'s pure-core-plus-thin-wrapper style.

## Decision (JSON shape) — RULED 2026-07-12: object form (per spec)

`check --format json` currently prints a **top-level JSON array** of diagnostics, consumed by the VS Code companion (`src/diagnostics.ts` parses the array) and pinned by the test `format_json_emits_file_line_message_array` (`check.rs`). **Author ruling: restructure to the spec's object form `{ "diagnostics": [...], "environment": [...] }`.** Task 8 implements it, updates the pinned test to the object shape, and this is a **breaking change to the companion** that must be updated in lockstep (tracked separately in the companion repo, `src/diagnostics.ts`). No longer a blocker.

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/server/src/interpreter.rs` | **new** — the pure resolution + probe module (`Lang`, `Provenance`, `Resolved`, `resolve_python`, `resolve_r`, `Probe`, `probe`) |
| `crates/server/src/main.rs` | register `mod interpreter;` |
| `crates/core/src/site/config/mod.rs` | `python:`/`r:` fields on `SiteConfig` + `NATIVE_KEYS` + parse |
| `crates/server/src/exec.rs` | `set_interpreters` setter; provenance fields; log-once at kernel start; resolved path in `diagnostic`; `kernel_lang` → `pub(crate)` |
| `crates/server/src/warm_pool.rs` | `boot_pool` + `warm_pool_for_*` take a resolved `&Resolved`; `should_warm` provenance gate |
| `crates/server/src/serve/mod.rs` | resolve + `set_interpreters` (single-doc; no pool) |
| `crates/server/src/serve_site/mod.rs` + `serve_site/exec_pool.rs` | resolve from `config` + site root; feed pool + per-page executors |
| `crates/server/src/build.rs` | resolve from `config` (site) / `None` (single file); feed pool + executors + deck build |
| `crates/server/src/check.rs` | informational Environment section |
| `docs/guide/reference/*.tmd` | document `python:`/`r:` + `check` Environment section |

---

### Task 1: `interpreter.rs` resolution core (pure)

**Files:**
- Create: `crates/server/src/interpreter.rs`
- Modify: `crates/server/src/main.rs:12` (register module)
- Test: inline `#[cfg(test)] mod tests` in `interpreter.rs`

**Interfaces:**
- Produces: `pub enum Lang { Python, R }`; `pub enum Provenance { Field, Venv, Env, Default }` with `pub fn label(self, lang: Lang) -> &'static str`; `pub struct Resolved { pub path: PathBuf, pub provenance: Provenance }` (derives `Clone`); `pub fn resolve_python(field: Option<&str>, project_dir: &Path) -> Resolved`; `pub fn resolve_r(field: Option<&str>, project_dir: &Path) -> Resolved`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/server/src/interpreter.rs` (create the file with just the test module first so it fails to compile, which is the "red"):

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --lib interpreter 2>&1 | tail -20`
Expected: compile error — `resolve_python_env`, `Provenance`, `Lang`, `Resolved`, `resolve_r_env` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/server/src/interpreter.rs` (above the test module):

```rust
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

/// Resolve the Python interpreter: `field` → `<dir>/.venv/bin/python` (then
/// `.venv/bin/python3`) → `TALIESIN_PYTHON` → `python3`. First existing match wins.
pub fn resolve_python(field: Option<&str>, project_dir: &Path) -> Resolved {
    resolve_python_env(field, project_dir, std::env::var_os("TALIESIN_PYTHON").as_deref())
}

/// Resolve the R interpreter: `field` → `TALIESIN_R` → `R`. No `.venv` step (there is
/// no R venv convention). `project_dir` is accepted for signature symmetry.
pub fn resolve_r(field: Option<&str>, project_dir: &Path) -> Resolved {
    resolve_r_env(field, project_dir, std::env::var_os("TALIESIN_R").as_deref())
}

/// Testable core of [`resolve_python`] with the env value injected.
fn resolve_python_env(field: Option<&str>, project_dir: &Path, env: Option<&OsStr>) -> Resolved {
    if let Some(f) = field.map(str::trim).filter(|s| !s.is_empty()) {
        return Resolved { path: PathBuf::from(f), provenance: Provenance::Field };
    }
    for cand in [".venv/bin/python", ".venv/bin/python3"] {
        let p = project_dir.join(cand);
        if p.exists() {
            return Resolved { path: p, provenance: Provenance::Venv };
        }
    }
    if let Some(env) = env {
        return Resolved { path: PathBuf::from(env), provenance: Provenance::Env };
    }
    Resolved { path: PathBuf::from("python3"), provenance: Provenance::Default }
}

/// Testable core of [`resolve_r`] with the env value injected.
fn resolve_r_env(field: Option<&str>, _project_dir: &Path, env: Option<&OsStr>) -> Resolved {
    if let Some(f) = field.map(str::trim).filter(|s| !s.is_empty()) {
        return Resolved { path: PathBuf::from(f), provenance: Provenance::Field };
    }
    if let Some(env) = env {
        return Resolved { path: PathBuf::from(env), provenance: Provenance::Env };
    }
    Resolved { path: PathBuf::from("R"), provenance: Provenance::Default }
}
```

Register the module in `crates/server/src/main.rs` (the `mod` list is alphabetical-ish around line 12, after `mod freeze;`/before `mod kernel;`):

```rust
mod interpreter;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --lib interpreter 2>&1 | tail -20`
Expected: PASS (6 tests). If `interpreter` is flagged dead-code (nothing calls it yet), that is expected until Task 4/5 wire it; the tests still run. Add `#![allow(dead_code)]`-free code by keeping items `pub` (they are consumed later); a transient `unused` warning on `resolve_python`/`resolve_r`/`probe` until Task 4-8 is acceptable but prefer to land Task 1+2 then 4+ in sequence so warnings clear.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/interpreter.rs crates/server/src/main.rs
git commit -m "feat(interpreter): pure Python/R interpreter resolution (precedence + provenance)"
```

---

### Task 2: `interpreter.rs` probe (read-only environment introspection)

**Files:**
- Modify: `crates/server/src/interpreter.rs`
- Test: inline tests in `interpreter.rs`

**Interfaces:**
- Consumes: `Resolved`, `Lang` from Task 1.
- Produces: `pub struct Probe { pub runs: bool, pub version: Option<String>, pub kernel_pkg_ok: bool, pub error: Option<String> }`; `pub fn probe(resolved: &Resolved, lang: Lang) -> Probe`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/server/src/interpreter.rs`:

```rust
    #[test]
    fn probe_of_a_missing_binary_reports_not_runnable_without_panicking() {
        let r = Resolved {
            path: std::path::PathBuf::from("/nonexistent/tali/python-xyz"),
            provenance: Provenance::Field,
        };
        let p = probe(&r, Lang::Python);
        assert!(!p.runs, "a missing binary must not report as runnable");
        assert!(!p.kernel_pkg_ok);
        assert!(p.error.is_some(), "a spawn failure is captured, not swallowed");
    }

    #[test]
    fn probe_reports_ipykernel_against_a_real_python() {
        // Kernel-gated: without TALIESIN_PYTHON this asserts nothing (no live interp).
        let Some(py) = std::env::var_os("TALIESIN_PYTHON") else {
            eprintln!("SKIPPED (no live interpreter): set TALIESIN_PYTHON to probe ipykernel");
            return;
        };
        let r = Resolved { path: py.into(), provenance: Provenance::Env };
        let p = probe(&r, Lang::Python);
        assert!(p.runs, "a real python should run --version");
        assert!(p.version.is_some(), "version string captured");
        // kernel_pkg_ok reflects reality; we only assert it is a determinate bool
        // and that when false an error is captured.
        if !p.kernel_pkg_ok {
            assert!(p.error.is_some());
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --lib interpreter 2>&1 | tail -20`
Expected: compile error — `Probe`/`probe` not found.

- [ ] **Step 3: Write the implementation**

Add to `crates/server/src/interpreter.rs` (after `resolve_r_env`, before the test module):

```rust
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
        Err(e) => (false, None, Some(format!("cannot run {}: {e}", bin.display()))),
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

    Probe { runs, version, kernel_pkg_ok, error }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --lib interpreter 2>&1 | tail -20`
Expected: PASS (`probe_of_a_missing_binary...` runs; `probe_reports_ipykernel...` runs or prints SKIPPED).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/interpreter.rs
git commit -m "feat(interpreter): read-only probe of interpreter version + Jupyter kernel package"
```

---

### Task 3: `_site.yml` gains `python:` / `r:`

**Files:**
- Modify: `crates/core/src/site/config/mod.rs:28-60` (struct), `:113-131` (`NATIVE_KEYS`), `:200-219` (parse)
- Test: inline `#[cfg(test)] mod tests` in `config/mod.rs` (check for existing test module; add if absent)

**Interfaces:**
- Produces: `SiteConfig.python: Option<String>`, `SiteConfig.r: Option<String>` (project-level interpreter pins).

- [ ] **Step 1: Write the failing test**

Find the test module in `crates/core/src/site/config/mod.rs` (search `mod tests`). Add:

```rust
    #[test]
    fn parses_python_and_r_interpreter_pins() {
        let mut w = Vec::new();
        let v: serde_yaml::Value =
            serde_yaml::from_str("title: X\npython: .venv/bin/python\nr: /usr/bin/R\n").unwrap();
        let cfg = parse_native(&v, &mut w);
        assert_eq!(cfg.python.as_deref(), Some(".venv/bin/python"));
        assert_eq!(cfg.r.as_deref(), Some("/usr/bin/R"));
        assert!(w.is_empty(), "valid keys warn about nothing: {w:?}");
    }

    #[test]
    fn a_typod_interpreter_key_warns_via_native_keys() {
        let mut w = Vec::new();
        let v: serde_yaml::Value = serde_yaml::from_str("pyton: .venv/bin/python\n").unwrap();
        let _ = parse_native(&v, &mut w);
        assert!(
            w.iter().any(|m| m.contains("pyton")),
            "an unknown config key must warn (did-you-mean python): {w:?}"
        );
    }
```

If no `mod tests` exists in this file, add one:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // (tests above)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib site::config 2>&1 | tail -20`
Expected: FAIL — `cfg.python`/`cfg.r` fields do not exist (compile error).

- [ ] **Step 3: Write the implementation**

In `crates/core/src/site/config/mod.rs`, add two fields to `SiteConfig` (after `pub publish: Option<PublishConfig>,` near line 60):

```rust
    /// Project-pinned Python interpreter (`python:` in `_site.yml`), highest
    /// precedence in interpreter resolution. `None` falls back to `.venv`/env/default.
    pub python: Option<String>,
    /// Project-pinned R interpreter (`r:` in `_site.yml`). `None` falls back to env/`R`.
    pub r: Option<String>,
```

Add the keys to `NATIVE_KEYS` (the slice at line 113; insert before the closing `]`, after `"publish",`):

```rust
    "python",
    "r",
```

Populate them in `parse_native` (the `SiteConfig { ... }` literal at line 200; add before the closing `}`):

```rust
        python: str_of("python"),
        r: str_of("r"),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-core --lib site::config 2>&1 | tail -20`
Expected: PASS (both new tests). Then `cargo build -p taliesin-core` to confirm no other `SiteConfig { .. }` literal broke (search for exhaustive constructions; `SiteConfig::default()` derives all-`None`, so only `parse_native` constructs it fully).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/config/mod.rs
git commit -m "feat(site): _site.yml python:/r: interpreter pins (native keys + parse)"
```

---

### Task 4: `Executor::set_interpreters` + provenance + log-once + diagnostic path

**Files:**
- Modify: `crates/server/src/exec.rs` — `LangState` (`:135`), `Executor` struct (`:150`), `build` (`:216`), setters region (`:257`), `spec` (`:262`), `diagnostic` (`:274`), `ensure_kernel` (`:679`), `kernel_lang` (`:94` visibility)
- Test: inline tests in `exec.rs` (there is an existing `#[cfg(test)] mod` with executor tests)

**Interfaces:**
- Consumes: `crate::interpreter::{Lang, Provenance, Resolved}` (Tasks 1).
- Produces: `pub fn set_interpreters(&mut self, python: Resolved, r: Resolved)`; `pub(crate) fn kernel_lang(...)` (re-exported visibility for Task 8).

- [ ] **Step 1: Write the failing test**

Add to the executor `#[cfg(test)] mod` in `crates/server/src/exec.rs` (near the existing bogus-interpreter test around line 1144):

```rust
    /// `set_interpreters` overrides the env/default python, and a bogus resolved path
    /// surfaces in the diagnostic verbatim — so a failure names the exact interpreter
    /// (the 2026-07-11 "which python?" gap). A bogus path fails Kernel::start
    /// deterministically, so this needs no live kernel.
    #[test]
    fn diagnostic_names_the_resolved_interpreter() {
        use crate::interpreter::{Provenance, Resolved};
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut ex = Executor::new();
            ex.set_interpreters(
                Resolved {
                    path: std::path::PathBuf::from("/nonexistent/tali/py-abc"),
                    provenance: Provenance::Field,
                },
                Resolved {
                    path: std::path::PathBuf::from("R"),
                    provenance: Provenance::Default,
                },
            );
            let blocks = vec![cell_block("python", "print(1)")];
            let _ = ex.run(blocks).await;
            let diag = ex.diagnostic().expect("a bogus interpreter yields a diagnostic");
            assert!(
                diag.contains("/nonexistent/tali/py-abc"),
                "diagnostic must name the resolved interpreter path: {diag}"
            );
        });
    }
```

Note: reuse the file's existing cell-block test helper. Search the test module for how other tests build a `{python}` cell block (e.g. a `cell_block(lang, code)` helper or an inline `Block { cell: Some(Cell { .. }), .. }`); use that exact constructor so the test compiles. If no helper exists, mirror the construction used by the neighbouring bogus-interpreter test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --lib exec::tests::diagnostic_names_the_resolved_interpreter 2>&1 | tail -20`
Expected: FAIL — `set_interpreters` not found (compile error).

- [ ] **Step 3: Write the implementation**

(a) `LangState` (line 135): add an announce-once flag. It derives `Default`, so add the field:

```rust
    /// Whether this executor has already logged which interpreter this language runs
    /// (the "which python?" signal). Reset by `restart_kernel` (which clears `langs`),
    /// so a manual restart re-announces.
    announced: bool,
```

(b) `Executor` struct (line 150): add provenance fields after `r: PathBuf,`:

```rust
    python_prov: crate::interpreter::Provenance,
    r_prov: crate::interpreter::Provenance,
```

(c) `build` (line 216): compute provenance alongside the existing path logic. Replace the body's python/r resolution + the struct literal's `python,`/`r,` region with:

```rust
        let (python, python_prov) = match std::env::var_os("TALIESIN_PYTHON") {
            Some(p) => (PathBuf::from(p), crate::interpreter::Provenance::Env),
            None => (PathBuf::from("python3"), crate::interpreter::Provenance::Default),
        };
        let (r, r_prov) = match std::env::var_os("TALIESIN_R") {
            Some(p) => (PathBuf::from(p), crate::interpreter::Provenance::Env),
            None => (PathBuf::from("R"), crate::interpreter::Provenance::Default),
        };
        Self {
            python,
            r,
            python_prov,
            r_prov,
            langs: HashMap::new(),
            freeze,
            force_next: false,
            no_exec: std::env::var_os("TALIESIN_NO_EXEC").is_some(),
            work_dir: None,
            sink: None,
            page: None,
            pool: None,
        }
```

(d) Add the setter in the setters region (after `set_warm_pool`, ~line 259):

```rust
    /// Override the interpreters this executor runs (and the pool warms), with their
    /// provenance for the "which python?" log line. Called once by each build/serve
    /// entry point after resolving `_site.yml`/`.venv`/env. A `&mut self` setter (not a
    /// consuming builder) so a pooled `&mut Executor` can be pointed at the resolved
    /// interpreters. Executors that never call this keep the env/default from `build`.
    pub fn set_interpreters(
        &mut self,
        python: crate::interpreter::Resolved,
        r: crate::interpreter::Resolved,
    ) {
        self.python = python.path;
        self.python_prov = python.provenance;
        self.r = r.path;
        self.r_prov = r.provenance;
    }
```

(e) `diagnostic` (line 274): name the resolved path. Replace the `let var = ...` block and the two `format!`s so the message includes the path. Change to:

```rust
        self.langs.iter().find_map(|(lang, s)| {
            if s.kernel.is_some() || s.failed_at.is_none() {
                return None;
            }
            let (var, path) = if *lang == "r" {
                ("TALIESIN_R", self.r.display().to_string())
            } else {
                ("TALIESIN_PYTHON", self.python.display().to_string())
            };
            Some(match &s.last_error {
                Some(e) => format!(
                    "{lang} kernel unavailable ({path}) — {e}. Code cells render as source; \
                     fix the interpreter ({var} or _site.yml {lang}:) and click Restart kernel."
                ),
                None => format!(
                    "{lang} kernel unavailable ({path}); code cells render as source \
                     (set {var} or _site.yml {lang}: to an interpreter with the Jupyter kernel, then Restart kernel)."
                ),
            })
        })
```

(f) `ensure_kernel` (line 679): announce once, after the early-return block commits to a boot. Precompute the label before the borrow block (owned values, so no borrow conflict with `state`). Right after `let work_dir = self.work_dir.clone();` (line 684) add:

```rust
        let (prov, lang_enum) = if lang == "r" {
            (self.r_prov, crate::interpreter::Lang::R)
        } else {
            (self.python_prov, crate::interpreter::Lang::Python)
        };
```

Then inside the existing block, immediately after the `failed_at` backoff `if` (after line 699, still inside the `{ }` that borrows `state`), add:

```rust
            if !state.announced {
                crate::log::kernel(&format!(
                    "{lang} -> {}  (from {})",
                    program.display(),
                    prov.label(lang_enum)
                ));
                state.announced = true;
            }
```

(g) `kernel_lang` (line 94): widen visibility for Task 8's language scan:

```rust
pub(crate) fn kernel_lang(lang: &str) -> Option<&'static str> {
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --lib exec 2>&1 | tail -30`
Expected: PASS, including `diagnostic_names_the_resolved_interpreter` and every pre-existing exec test (no regression to warm-reuse/plan logic).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/exec.rs
git commit -m "feat(exec): set_interpreters + log-once at kernel start + resolved path in diagnostic"
```

---

### Task 5: Warm pool takes a resolved interpreter + provenance gate

**Files:**
- Modify: `crates/server/src/warm_pool.rs` — `warm_pool_for_preview` (`:531`), `warm_pool_for_build` (`:541`), `boot_pool` (`:553`); add `should_warm`
- Test: inline `#[cfg(test)] mod tests` in `warm_pool.rs`

**Interfaces:**
- Consumes: `crate::interpreter::{Provenance, Resolved}`.
- Produces: `pub async fn warm_pool_for_preview(python: &Resolved) -> Option<Arc<WarmPool>>`; `pub async fn warm_pool_for_build(size: usize, python: &Resolved) -> Option<Arc<WarmPool>>`; `fn should_warm(prov: Provenance) -> bool`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/server/src/warm_pool.rs`:

```rust
    /// The pool boots for a concretely-chosen interpreter (field/.venv/env) and stays
    /// inert on the bare `python3` default — preserving "don't speculatively boot a
    /// possibly-absent python3" while now warming a project's `.venv`/pin. Pure gate,
    /// no live kernel (mirrors `try_reserve_slot`'s pure-test style).
    #[test]
    fn should_warm_only_for_a_concrete_interpreter() {
        use crate::interpreter::Provenance;
        assert!(should_warm(Provenance::Field));
        assert!(should_warm(Provenance::Venv));
        assert!(should_warm(Provenance::Env));
        assert!(!should_warm(Provenance::Default), "bare python3 must stay inert");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --lib warm_pool::tests::should_warm_only_for_a_concrete_interpreter 2>&1 | tail -20`
Expected: FAIL — `should_warm` not found.

- [ ] **Step 3: Write the implementation**

In `crates/server/src/warm_pool.rs`, replace the three functions at lines 531-556. New bodies:

```rust
/// Whether the warm pool should boot for an interpreter of this provenance. A
/// concretely-chosen interpreter (`_site.yml` field, a project `.venv`, or
/// `TALIESIN_PYTHON`) is worth pre-warming; the bare `python3` default is not (we
/// never speculatively boot a forkserver against a possibly-absent `python3`). Pure,
/// unit-tested without a live kernel.
fn should_warm(prov: crate::interpreter::Provenance) -> bool {
    !matches!(prov, crate::interpreter::Provenance::Default)
}

/// Build the one warm pool a **preview server** owns, warming the resolved `python`.
/// Returns `None` (every page cold-starts, exactly as before) when the interpreter is
/// the bare `python3` default; otherwise boots the forkserver (an inert pool on boot
/// failure still cold-starts, no regression).
pub async fn warm_pool_for_preview(python: &crate::interpreter::Resolved) -> Option<Arc<WarmPool>> {
    let want = crate::build_budget::preview_warm_pool_size();
    boot_pool(want, python).await
}

/// Build the one warm pool a **site build** owns, asking for `size` pre-warmed kernels
/// of the resolved `python`. Returns `None` when `size == 0` or the interpreter is the
/// bare `python3` default.
pub async fn warm_pool_for_build(
    size: usize,
    python: &crate::interpreter::Resolved,
) -> Option<Arc<WarmPool>> {
    if size == 0 {
        return None;
    }
    boot_pool(size, python).await
}

/// Boot a warm pool of `want` kernels of the resolved `python`, or `None` when the
/// interpreter wasn't concretely chosen (bare default), so we never speculatively boot
/// a forkserver against a possibly-absent `python3`. A boot *failure* isn't `None`
/// here — `WarmPool::new` degrades to an inert pool the executor treats as a cold start.
async fn boot_pool(want: usize, python: &crate::interpreter::Resolved) -> Option<Arc<WarmPool>> {
    if !should_warm(python.provenance) {
        return None;
    }
    Some(Arc::new(WarmPool::new(&python.path, want).await))
}
```

- [ ] **Step 4: Run test to verify it passes; the callers won't compile yet**

Run: `cargo test -p taliesin-server --lib warm_pool::tests::should_warm_only_for_a_concrete_interpreter 2>&1 | tail -20`
Expected: the unit test PASSES, but `cargo build` fails at the call sites (`serve_site/mod.rs:790`, `build.rs:997`) which still call the old zero-arg signatures. Those are fixed in Task 7. That is expected mid-sequence; commit this task's file, then land Task 6+7 before the next full build.

If you prefer a green build at every commit, do Tasks 5→7 as one combined commit. Otherwise:

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/warm_pool.rs
git commit -m "feat(warm-pool): boot for a resolved interpreter, inert on the bare default"
```

---

### Task 6: Wire single-doc serve + single-file build

**Files:**
- Modify: `crates/server/src/serve/mod.rs:991` (single-doc preview executor)
- Modify: `crates/server/src/build.rs:413` (`build_page_executing`, single-file build executor)

**Interfaces:**
- Consumes: `crate::interpreter::{resolve_python, resolve_r}`, `Executor::set_interpreters`.
- Single-doc contexts have no `_site.yml`, so `field = None`; the project dir is the doc's own dir (so a `.venv` beside the doc is picked).

- [ ] **Step 1: Wire single-doc preview**

In `crates/server/src/serve/mod.rs`, right after the executor is created (line 991, `let mut executor = crate::exec::Executor::with_freeze(freeze_path).in_dir(&app.base_dir);`) insert:

```rust
        // Resolve this document's interpreters from its own directory (no _site.yml here,
        // so a `.venv` beside the doc / env / default), so a project-local venv beats a
        // stray global TALIESIN_PYTHON and the first kernel start logs which one ran.
        executor.set_interpreters(
            crate::interpreter::resolve_python(None, &app.base_dir),
            crate::interpreter::resolve_r(None, &app.base_dir),
        );
```

- [ ] **Step 2: Wire single-file build**

In `crates/server/src/build.rs`, in `build_page_executing` where the executor is created (line 413, the `exec::Executor::with_freeze(...)` for the single-file build). Confirm the in-scope base dir variable name (search the function for `base`), then after the executor binding add:

```rust
    // Single-file build: no _site.yml, so resolve from the doc's own dir (.venv / env /
    // default). Feeds the same set_interpreters path the site build uses.
    exec.set_interpreters(
        crate::interpreter::resolve_python(None, base),
        crate::interpreter::resolve_r(None, base),
    );
```

Match the actual local binding names at that site (the executor may be `exec` or another name; `base` is the doc dir). Keep it a single `set_interpreters` call right after creation, before `.run(...)`.

- [ ] **Step 3: Verify by building + running (no new unit test; this is wiring)**

The behaviour is exercised end-to-end, not by a fresh unit (resolution itself is covered by Task 1). Verify:

Run: `cargo build -p taliesin-server 2>&1 | tail -5`
Expected: compiles (single-doc paths no longer reference removed signatures).

Run (manual, kernel-gated): create a temp dir with a `.venv/bin/python` and a `x.tmd` holding a `{python}` cell, then:
`cargo run -p taliesin-server -- preview /tmp/itest/x.tmd 4388`
Expected: stderr logs `python -> /tmp/itest/.venv/bin/python  (from .venv)` at first kernel start (even with a global `TALIESIN_PYTHON` set — the `.venv` wins).

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/serve/mod.rs crates/server/src/build.rs
git commit -m "feat(serve,build): resolve + set interpreters for single-doc preview and single-file build"
```

---

### Task 7: Wire site preview (serve_site) + site build + deck build

**Files:**
- Modify: `crates/server/src/serve_site/exec_pool.rs` (`ExecPool` carries resolved python/r; `make` applies them)
- Modify: `crates/server/src/serve_site/mod.rs:790` (resolve from config + root; pass to pool + warm pool)
- Modify: `crates/server/src/build.rs:997` (resolve from `site.config` + root), `:754/:798` (`build_one_page` receives + sets interpreters), `:1075` (deck build)

**Interfaces:**
- Consumes: `SiteConfig.python`/`SiteConfig.r` (Task 3), `resolve_python`/`resolve_r` (Task 1), `warm_pool_for_preview(&Resolved)`/`warm_pool_for_build(size, &Resolved)` (Task 5), `Executor::set_interpreters` (Task 4).
- Site contexts pass `config.python.as_deref()`/`config.r.as_deref()` as the field and the **site root** as `project_dir`.

- [ ] **Step 1: `ExecPool` carries the resolved interpreters**

In `crates/server/src/serve_site/exec_pool.rs`:

Add two fields to `struct ExecPool` (after `warm_pool`):

```rust
    /// The resolved Python/R interpreters (from `_site.yml` python:/r: / .venv / env /
    /// default), applied to every page executor so the pool and the executors agree on
    /// which interpreter runs. `Default` (the unit-test `Default::default`) resolves to
    /// the env/default via `Executor::build`, i.e. no override.
    python: Option<crate::interpreter::Resolved>,
    r: Option<crate::interpreter::Resolved>,
```

Update `ExecPool::new` to take them:

```rust
    pub(super) fn new(
        freeze_dir: PathBuf,
        warm_pool: Option<Arc<crate::warm_pool::WarmPool>>,
        python: crate::interpreter::Resolved,
        r: crate::interpreter::Resolved,
    ) -> Self {
        ExecPool {
            freeze_dir,
            warm_pool,
            python: Some(python),
            r: Some(r),
            ..Default::default()
        }
    }
```

Apply them in `make` (after `ex.set_warm_pool(self.warm_pool.clone());`, before `ex`):

```rust
        if let (Some(py), Some(r)) = (&self.python, &self.r) {
            ex.set_interpreters(py.clone(), r.clone());
        }
```

The `#[derive(Default)]` on `ExecPool` gives `python: None, r: None` for the tests (which construct `ExecPool::default()`), so those keep `Executor::build`'s env/default — no test change needed.

- [ ] **Step 2: Resolve in the site preview builder**

In `crates/server/src/serve_site/mod.rs`, `spawn_builder` (line 790). Replace the warm-pool line + `ExecPool::new` call:

```rust
        // Resolve the project's interpreters once (from _site.yml python:/r:, a project
        // .venv, env, or default) against the site root, so every page executor and the
        // warm pool agree on which interpreter runs. Read the config under the site lock.
        let (py, r) = {
            let site = app.site.lock();
            (
                crate::interpreter::resolve_python(site.config.python.as_deref(), &app.root),
                crate::interpreter::resolve_r(site.config.r.as_deref(), &app.root),
            )
        };
        let warm_pool = crate::warm_pool::warm_pool_for_preview(&py).await;
        let mut pool = ExecPool::new(app.root.join("_freeze"), warm_pool, py, r);
```

Confirm `SiteApp` exposes `site` (a `Mutex<Site>`) and `root: PathBuf` (both seen at `serve_site/mod.rs:39-40`, and `Site` holds `config`). If the config field on `Site` is named other than `config`, adjust `site.config.python`.

- [ ] **Step 3: Resolve in the site build + thread into pages and decks**

In `crates/server/src/build.rs`, `build_site_async` (around line 997). Before `let warm_pool = ...`, resolve once:

```rust
    // Resolve the project's interpreters once from _site.yml python:/r: (or .venv / env /
    // default) against the site root, shared by every page + deck executor and the pool.
    let interp_python = crate::interpreter::resolve_python(site.config.python.as_deref(), root);
    let interp_r = crate::interpreter::resolve_r(site.config.r.as_deref(), root);
```

Change the warm-pool line to pass the resolved python:

```rust
    let warm_pool = warm_pool::warm_pool_for_build(split.warm_pool, &interp_python).await;
```

Thread the resolved pair into `build_one_page` (line 754 signature + line 797-806 body) and the deck build (line 1075). `build_one_page` is called from the per-page JoinSet closure; add two params:

- Extend `build_one_page`'s signature (line 754) with `python: &crate::interpreter::Resolved, r: &crate::interpreter::Resolved` (or clone-per-task `Resolved` if the closure moves them; `Resolved: Clone`, and the JoinSet tasks are spawned per page, so clone `interp_python`/`interp_r` into each task like `site`/`out` are `.clone()`d at lines 1011-1014).
- In `build_one_page`, after `exec.set_warm_pool(warm_pool);` (line 805) add:

```rust
    exec.set_interpreters(python.clone(), r.clone());
```

- For the deck executor (line 1075), after it is created add the same:

```rust
        exec.set_interpreters(interp_python.clone(), interp_r.clone());
```

(Use whichever `interp_*` bindings are in scope at the deck-build site; they live in `build_site_async` alongside the deck loop.)

- [ ] **Step 4: Verify by building + running (wiring; corpus is the net)**

Run: `cargo build -p taliesin-server 2>&1 | tail -5`
Expected: compiles (all `warm_pool_for_*` / `ExecPool::new` callers updated).

Run: `cargo test -p taliesin-server --lib exec_pool 2>&1 | tail -10`
Expected: PASS (eviction tests unaffected; `ExecPool::default()` still valid).

Run (manual, kernel-gated): a site dir with `_site.yml` containing `python: .venv/bin/python` and a page with a `{python}` cell:
`cargo run -p taliesin-server -- build /tmp/itest-site 2>&1 | grep 'python ->'`
Expected: `python -> .../.venv/bin/python  (from _site.yml python:)`.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/serve_site/exec_pool.rs crates/server/src/serve_site/mod.rs crates/server/src/build.rs
git commit -m "feat(site): resolve interpreters from _site.yml/.venv and feed pool + page/deck executors"
```

---

### Task 8: `taliesin check` informational Environment section

> **BLOCKER:** resolve the "Open decision (JSON shape)" above with the author before implementing the JSON half of this task. Steps below implement the spec's object form; if the author picks a compat option, adjust Step 3's JSON accordingly.

**Files:**
- Modify: `crates/server/src/check.rs` — new `EnvEntry` + `collect_environment`; extend `cmd_check` printing; JSON restructure
- Test: inline tests in `check.rs`

**Interfaces:**
- Consumes: `crate::interpreter::{resolve_python, resolve_r, probe, Lang, Resolved}`, `crate::exec::kernel_lang` (Task 4 visibility), `SiteConfig.python`/`r` (Task 3).
- Produces: `struct EnvEntry { lang, path, provenance, kernel_pkg, kernel_pkg_ok, version }` (serializable); `fn collect_environment(path: &Path) -> Vec<EnvEntry>`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` in `crates/server/src/check.rs`:

```rust
    #[test]
    fn environment_is_empty_for_a_doc_with_no_code_cells() {
        let dir = std::env::temp_dir().join("tali-check-nocells");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# Title\n\nJust prose, no cells.\n").unwrap();
        assert!(
            collect_environment(&f).is_empty(),
            "a doc with no python/r cells reports no Environment entries"
        );
    }

    #[test]
    fn environment_lists_python_for_a_python_cell_doc() {
        let dir = std::env::temp_dir().join("tali-check-pycell");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# T\n\n```{python}\nprint(1)\n```\n").unwrap();
        let env = collect_environment(&f);
        assert_eq!(env.len(), 1, "one entry for the single python language used");
        assert_eq!(env[0].lang, "python");
        // Path + provenance are populated; kernel_pkg_ok reflects the box (may be false
        // in CI). The section is informational, so we assert shape, not availability.
        assert!(!env[0].path.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --lib check::tests::environment 2>&1 | tail -20`
Expected: FAIL — `collect_environment` / `EnvEntry` not found.

- [ ] **Step 3: Write the implementation**

In `crates/server/src/check.rs`, add the type + collector (near `collect_diagnostics`, after the `Diagnostic` definitions):

```rust
/// One line of the informational Environment section: the interpreter `check`
/// resolved for a language the document runs, and whether its Jupyter kernel package
/// is importable. Serialized into `--format json` and printed after the diagnostics.
#[derive(serde::Serialize)]
struct EnvEntry {
    lang: &'static str,
    path: String,
    provenance: String,
    /// `ipykernel` (python) / `IRkernel` (r).
    kernel_pkg: &'static str,
    kernel_pkg_ok: bool,
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Which executable languages (`python`/`r`) a document actually uses, in first-seen
/// order. Renders in memory (like the diagnostics collectors) and scans the block
/// model's cells, so `{{< include >}}`d cells count. Stops once both are seen.
fn used_languages(blocks: &[taliesin_core::Block]) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for b in blocks {
        if let Some(c) = &b.cell
            && let Some(lang) = crate::exec::kernel_lang(&c.lang)
            && !seen.contains(&lang)
        {
            seen.push(lang);
            if seen.len() == 2 {
                break;
            }
        }
    }
    seen
}

/// Build one `EnvEntry` for `lang` given the resolved interpreter.
fn env_entry(lang: &'static str, resolved: &crate::interpreter::Resolved) -> EnvEntry {
    let lang_enum = if lang == "r" {
        crate::interpreter::Lang::R
    } else {
        crate::interpreter::Lang::Python
    };
    let p = crate::interpreter::probe(resolved, lang_enum);
    EnvEntry {
        lang,
        path: resolved.path.display().to_string(),
        provenance: resolved.provenance.label(lang_enum).to_string(),
        kernel_pkg: if lang == "r" { "IRkernel" } else { "ipykernel" },
        kernel_pkg_ok: p.kernel_pkg_ok,
        version: p.version,
        error: p.error,
    }
}

/// The informational Environment section for a file or site: for each executable
/// language the target uses, the resolved interpreter + kernel-package probe. Never
/// affects `check`'s exit code. `field` pins come from `_site.yml` for a site;
/// a single file has none. Empty when the target has no python/r cells.
fn collect_environment(path: &Path) -> Vec<EnvEntry> {
    if path.is_dir() {
        let site = taliesin_core::Site::discover(path);
        // Union of languages across pages, and the project-level field pins + root.
        let mut langs: Vec<&'static str> = Vec::new();
        for page in &site.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(path);
            let doc = taliesin_core::render_document_with_includes_scoped(
                &src,
                base,
                site.chapter_for(page),
            );
            for l in used_languages(&doc.blocks) {
                if !langs.contains(&l) {
                    langs.push(l);
                }
            }
            if langs.len() == 2 {
                break;
            }
        }
        langs
            .into_iter()
            .map(|lang| {
                let resolved = if lang == "r" {
                    crate::interpreter::resolve_r(site.config.r.as_deref(), path)
                } else {
                    crate::interpreter::resolve_python(site.config.python.as_deref(), path)
                };
                env_entry(lang, &resolved)
            })
            .collect()
    } else {
        let Ok(src) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let doc = taliesin_core::render_document_with_includes(&src, base);
        used_languages(&doc.blocks)
            .into_iter()
            .map(|lang| {
                let resolved = if lang == "r" {
                    crate::interpreter::resolve_r(None, base)
                } else {
                    crate::interpreter::resolve_python(None, base)
                };
                env_entry(lang, &resolved)
            })
            .collect()
    }
}
```

Extend `cmd_check` (lines 268-283) to compute + emit the section. Insert `let environment = collect_environment(target);` after `diags` is bound (line 254 region, after the `Ok(d) => d` match), then:

- **JSON (object form per spec — see BLOCKER):** replace the `format_json(&diags)` print with:

```rust
    if format == "json" {
        let payload = serde_json::json!({
            "diagnostics": diags,
            "environment": environment,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()));
    } else {
```

- **Human (after the problem summary, still stderr):** before `if diags.is_empty() { ExitCode::SUCCESS }`, add:

```rust
    if !environment.is_empty() {
        eprintln!("\nEnvironment:");
        for e in &environment {
            let pkg = if e.kernel_pkg_ok {
                match &e.version {
                    Some(v) => format!("{} present ({v})", e.kernel_pkg),
                    None => format!("{} present", e.kernel_pkg),
                }
            } else {
                format!("{} MISSING", e.kernel_pkg)
            };
            eprintln!("  {}: {} ({}) — {}", e.lang, e.path, e.provenance, pkg);
        }
    }
```

The `error` field carries probe stderr for the JSON consumer; the human line shows MISSING (kept terse). Note the em-dash-free copy: the `—` above must be written with a comma or colon per the no-dashes rule; use `"  {}: {} ({}), {}"`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --lib check 2>&1 | tail -30`
Expected: the two new tests PASS. The pre-existing `format_json_emits_file_line_message_array` test will now FAIL (the JSON top level changed to an object). Update that test to assert the object shape (`payload["diagnostics"].is_array()` + `payload["environment"].is_array()`), and note the change in the commit body. This test change is the in-repo half of the BLOCKER above.

Run: `cargo test -p taliesin-server --lib 2>&1 | tail -15`
Expected: whole server crate green.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/check.rs
git commit -m "feat(check): informational Environment section (resolved interpreter + kernel-pkg probe)"
```

---

### Task 9: Docs + final regression sweep

**Files:**
- Modify: `docs/guide/reference/*.tmd` (the `_site.yml` config reference + the `check` reference at `docs/guide/reference/cli.tmd`)

**Interfaces:** none (docs).

- [ ] **Step 1: Document `python:` / `r:`**

Find the `_site.yml` config reference page under `docs/guide/reference/` (search for existing keys like `favicon:` or `mounts:`) and add a short subsection documenting `python:` / `r:`: the precedence (field → `.venv` → env → default), that a `.venv` beside the project is auto-detected, and that a pin beats a global `TALIESIN_PYTHON` (reproducibility). Keep copy em-dash-free.

- [ ] **Step 2: Document the `check` Environment section**

In `docs/guide/reference/cli.tmd` (the `check` section, near line 45), add a sentence: `check` prints an informational Environment section listing the resolved interpreter + whether its Jupyter kernel package is importable, and that this section never changes the exit code (static gate preserved). If the JSON shape changed (Task 8), document the new `{ diagnostics, environment }` object shape here.

- [ ] **Step 3: Full regression sweep**

Run: `cargo test -p taliesin-core 2>&1 | tail -15`
Expected: corpus invariants + core unit tests green.

Run: `cargo test -p taliesin-server 2>&1 | tail -15`
Expected: server crate green (interpreter, exec, warm_pool, check, exec_pool).

Run: `cargo build --release -p taliesin-server 2>&1 | tail -5`
Expected: clean release build (needed before any build-and-inspect per CLAUDE.md's stale-asset note).

Run (kernel-gated manual): `taliesin preview docs/guide` and confirm the first `{python}`/`{r}` kernel start logs `lang -> <path>  (from <source>)`, and `taliesin check docs/guide` prints an Environment line.

- [ ] **Step 4: Commit**

```bash
git add docs/guide/reference
git commit -m "docs(guide): document _site.yml python:/r: pins + check Environment section"
```

---

## Self-Review

**Spec coverage:**
- Goal "visible" → Task 4 (log-once + diagnostic path), Task 8 (`check` Environment). ✓
- Goal "project-reproducible" → Task 3 (`python:`/`r:`), Task 1 (`.venv` auto-detect + precedence), Task 6/7 (wiring feeds the pins). ✓
- Precedence table → Task 1 `resolve_python`/`resolve_r` + tests. ✓
- Latent warm-pool mismatch bug → Task 5 (pool warms the *resolved* python, gated by provenance) + Task 7 (same `Resolved` feeds pool and executors). ✓
- Invariants (exec core untouched, freeze determinism, single editing surface, `check` static gate) → Global Constraints + Task 4 note + Task 8 "informational only". ✓
- Non-goals (no per-page front-matter, no Windows `.venv`, only `.venv` name, no auto-install) → honored (resolution is project-level; `.venv/bin` Unix only; probe never installs). ✓
- Files-touched table → Tasks 1-9 cover every row. ✓

**Placeholder scan:** wiring Tasks 6/7 intentionally verify by build + manual kernel-gated run rather than a fresh unit (resolution logic is unit-tested in Task 1; the wiring has no pure seam). Every code step shows real code. The one deliberately deferred detail is the exact local binding names at `build.rs:413`/`:754`/`:1075` (the plan instructs the executor to match them in-place) because they depend on the surrounding function bodies.

**Type consistency:** `Resolved`/`Provenance`/`Lang` from Task 1 are used verbatim in Tasks 4/5/7/8. `set_interpreters(python: Resolved, r: Resolved)` signature is consistent across exec.rs (def), exec_pool.rs, serve/mod.rs, serve_site, build.rs. `warm_pool_for_preview(&Resolved)` / `warm_pool_for_build(size, &Resolved)` consistent between Task 5 (def) and Task 7 (callers). `kernel_lang` made `pub(crate)` in Task 4 and consumed in Task 8.

**Open risk carried to handoff:** the `check --format json` top-level shape change (Task 8) breaks the VS Code companion + a pinned test; flagged as a BLOCKER requiring the author's ruling before Task 8 ships.
