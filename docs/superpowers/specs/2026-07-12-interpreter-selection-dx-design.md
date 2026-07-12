# Interpreter selection DX — design

**Date:** 2026-07-12
**Status:** implemented (2026-07-12) on `feat/interpreter-selection-dx`, per `docs/superpowers/plans/2026-07-12-interpreter-selection-dx.md`. `check --format json` shape ruled to the object form (breaks the VS Code companion's `src/diagnostics.ts`; needs a lockstep update).
**Backlog item:** Tier 2 hardening — "Interpreter selection is silent + has no project-local override (DX; S+M)" (`notes/ROADMAP.md` / AUDITS)
**Related:** the warm-pool (`crates/server/src/warm_pool.rs`) and executor (`crates/server/src/exec.rs`) interpreter resolution; the `check` subcommand (`crates/server/src/check.rs`)

## Context

Interpreter selection is resolved once, silently, deep in `Executor::build`
(`crates/server/src/exec.rs`):

```rust
let python = env::var_os("TALIESIN_PYTHON").map(PathBuf::from).unwrap_or_else(|| "python3".into());
let r      = env::var_os("TALIESIN_R").map(PathBuf::from).unwrap_or_else(|| "R".into());
```

Two gaps bit a real user on **2026-07-11**: a global `TALIESIN_PYTHON` in `~/.zshrc`
(pointing at an interpreter without `ipykernel`) errored a whole book's ~35 cells, and
nothing told them *which* interpreter was being used — a dep-less interpreter is
indistinguishable from a genuine code error.

1. **No "which python?" signal.** The resolved interpreter path is never surfaced. When
   cells fail, the user can't tell whether their code is wrong or the wrong interpreter ran.
2. **No project-local declaration.** A project cannot pin its interpreter, so the only
   controls are a *global* env var (which fights every other project on the machine) and
   the bare `python3`/`R` default. A project with its own `.venv` still has to be told
   about it via the global env var.

There is also a **latent correctness bug**: the warm pool
(`warm_pool.rs::boot_pool`) reads `TALIESIN_PYTHON` **directly and independently** of the
executor. If a project ever selects a different interpreter than the env var, the pool
would pre-warm the *wrong* Python while the executor cold-starts the right one — a silent
interpreter mismatch. Fixing selection centrally closes this too.

## Goal

Make interpreter selection **visible** and **project-reproducible**, without touching the
Do-NOT-touch exec/kernel execution semantics (what runs, how it caches, kernel lifecycle).

Concretely:

- A project can pin its interpreter in `_site.yml` (`python:` / `r:`), and a project with a
  local `.venv` is auto-detected — so a committed project is reproducible regardless of the
  ambient shell.
- The resolved interpreter path + where it came from is **logged** the first time each
  language's kernel starts.
- `taliesin check` reports the resolved interpreter and whether its Jupyter kernel package
  is importable (like `quarto check`).

### Precedence (approved)

**Project pins win**, so a stray global env var cannot break a project:

| # | Source | Python | R |
|---|--------|--------|---|
| 1 | `_site.yml` field | `python:` | `r:` |
| 2 | local venv | `<project_dir>/.venv/bin/python` (then `.venv/bin/python3`) | — (no R equivalent) |
| 3 | env var | `TALIESIN_PYTHON` | `TALIESIN_R` |
| 4 | default | `python3` | `R` |

First existing match wins. This is exactly what would have prevented the 2026-07-11 bug: a
local `.venv` (or a `python:` pin) beats the global `TALIESIN_PYTHON`.

### Invariants honored

- **Do-NOT-touch execution core untouched.** Resolution moves to the build/serve *entry*
  and a new pure module; `exec.rs`/`kernel.rs` change only by (a) receiving the already-
  resolved paths and (b) one log line at first kernel start. What executes, how it caches
  (cumulative-hash freeze keys), and kernel lifecycle are unchanged — freeze determinism is
  preserved (an interpreter path is already part of the cache key via `interp_id`, so
  selecting a *different* interpreter correctly busts the cache exactly as a manual env
  change does today; selecting the *same* interpreter by a new route is byte-identical).
- **Minimal config / perfect-the-default.** The new `python:`/`r:` knobs are justified: they
  are the reproducibility fix the user approved. The default with **no** field stays
  near-perfect (`.venv` auto-detect → env → `python3`), so most projects still need zero
  config.
- **Single editing surface / preview-is-read-only** untouched (selection is a build/serve
  concern, never a source write-back).
- **`check` stays a static gate.** The new interpreter probe is *informational* and does
  **not** change `check`'s exit code (a CI box without Python must not fail static linting).

## Non-goals (scope boundary)

- **No per-page `python:` front-matter.** Declaration is project-level (`_site.yml`) only; a
  single `.tmd` with no config relies on `.venv`/env/default. Per-page interpreter switching
  is out of scope (it would fragment a book's kernel model).
- **No Windows `.venv` layout.** Auto-detect is Unix `bin/` only; `Scripts/` is noted as
  future (project is Linux-focused).
- **No `venv`/`.env`/`virtualenv` name sprawl.** Only `.venv` is auto-detected (the modern
  `python -m venv` / `uv` default). A non-standard venv dir is selected via the `python:`
  field.
- **No auto-install / no interpreter bootstrapping.** `check` reports what's missing; it
  never installs `ipykernel`/`IRkernel`.
- **No change to the "kernel unavailable" render fallback** beyond including the resolved
  path in the existing diagnostic.
- Nothing else from the Tier 2 list (ungraceful-death sweep, flaky timing tests, R ANSI,
  etc.) — those were explicitly de-scoped for this session.

## Design

### 1. New pure module `crates/server/src/interpreter.rs`

The single source of resolution truth, fully unit-testable (no live kernel needed).

```rust
pub enum Provenance { Field, Venv, Env, Default }   // + fn label(&self, lang) -> &str

pub struct Resolved { pub path: PathBuf, pub provenance: Provenance }

/// field → <dir>/.venv/bin/python (then .venv/bin/python3) → TALIESIN_PYTHON → python3
pub fn resolve_python(field: Option<&str>, project_dir: &Path) -> Resolved;

/// field → TALIESIN_R → R   (no .venv step)
pub fn resolve_r(field: Option<&str>, project_dir: &Path) -> Resolved;

/// Spawn `<py> --version` + `<py> -c "import ipykernel"` (or R + `library(IRkernel)`).
/// Environment introspection only — never runs the user's document.
pub fn probe(resolved: &Resolved, lang: Lang) -> Probe;   // { runs, version, kernel_pkg_ok, error }
```

- `Provenance::label` produces the human string for logs/`check`
  (`_site.yml python:`, `.venv`, `TALIESIN_PYTHON`, `python3`).
- `.venv` detection tests **existence** only (does not execute), so it's cheap and pure
  given a `project_dir`.

### 2. `_site.yml` gains `python:` / `r:`

In `crates/core/src/site/config/mod.rs`:

- Add `pub python: Option<String>` and `pub r: Option<String>` to `SiteConfig`.
- Add `"python"` and `"r"` to `NATIVE_KEYS` (so typos keep warning via the existing lint).
- Parse them in the existing `_site.yml` → `SiteConfig` path (string scalars).

Project-level only; carried on the resolved `SiteConfig` every site entry point already
holds.

### 3. Wiring — resolve once at each entry, feed pool **and** executors

Resolution happens at the three entry points; the resolved paths are pushed into the
`Executor` via a new `&mut self` setter (mirroring `set_progress`/`set_warm_pool`), and the
resolved **python** is also handed to the warm pool so it warms the same interpreter.

- **`Executor::set_interpreters(&mut self, python: Resolved, r: Resolved)`** — stores the
  paths (overwriting the env/default computed in `build`) and the provenance (for the log).
- **Single-doc `serve/mod.rs`:** `resolve_python(None, base_dir)` + `resolve_r(None, base_dir)`
  → `set_interpreters`. Gets `.venv`-beside-the-doc / env / default. (No warm pool here.)
- **Site `serve_site/mod.rs` + `build.rs`:** `resolve_python(config.python.as_deref(),
  site_root)` and `resolve_r(config.r.as_deref(), site_root)` → each pooled executor via
  `set_interpreters`; the resolved python path is passed to `warm_pool_for_preview` /
  `warm_pool_for_build`.
- **`warm_pool_for_preview` / `warm_pool_for_build` / `boot_pool`** take the resolved
  `Option<&Path>` **and** its provenance instead of reading `TALIESIN_PYTHON`. The pool
  boots when the interpreter is **concretely chosen** (provenance ∈ {Field, Venv, Env}) and
  stays inert on the bare `Default` — preserving today's "don't speculatively boot a
  possibly-absent `python3`" behavior while fixing the mismatch. (`Executor::build` keeps
  its current env/default computation as the fallback for callers — tests — that never call
  `set_interpreters`.)

### 4. "Which python?" signal

The **first time** each language's kernel is actually started (cold *or* warm-pool), log
one line at the existing kernel-start log site in `exec.rs`, guarded by a per-language
"already logged" flag so it fires once per executor:

```
python → /home/bogo/proj/.venv/bin/python  (from .venv)
r      → /usr/bin/R                          (from TALIESIN_R)
```

Only languages the document actually runs are logged (the hook is at kernel start, so an
R-free doc never claims an R interpreter). The existing "kernel unavailable" diagnostic
(`Executor::diagnostic`) is extended to include the resolved path, so a failure names the
exact interpreter.

### 5. `taliesin check` — informational Environment section

Extend `check.rs` with an **Environment** section, printed after the diagnostics:

- Determine which languages the checked path uses (scan the rendered block model / cell
  langs — `check` already renders in memory).
- For each used language: `resolve_*` the interpreter, then `probe` it, and print
  `path + provenance + <kernel-pkg> present/absent (+ version)`, e.g.
  `python: /proj/.venv/bin/python (.venv) — ipykernel ✓ (Python 3.12.3)` or
  `python: /usr/bin/python3 (python3) — ipykernel MISSING`.
- **Informational only:** this section never changes the exit code (preserving `check`'s
  static-gate contract). `--format json` gains an `environment` array alongside
  `diagnostics`.
- If the path has no code cells, the section is omitted (or a one-line "no code cells").

## Error handling

- **Resolution never fails.** It always yields *some* path; a broken choice surfaces through
  the existing kernel-start failure path (now naming the resolved interpreter).
- **`.venv` auto-detect** is a pure existence check; a dangling `.venv/bin/python` symlink
  that doesn't execute still fails loudly at kernel start (with its path in the message),
  which is strictly better than the silent-wrong-interpreter status quo.
- **`probe`** tolerates a missing binary / import error and reports it as `kernel_pkg_ok =
  false` with the captured stderr; it never panics or blocks `check`.
- **Freeze cache:** an interpreter change flips the cache key (via the existing `interp_id`),
  so a re-selection re-runs cells correctly; selecting the *same* interpreter by a new route
  is a cache hit (byte-identical). No new cache invalidation logic.

## Testing (TDD)

Pure unit tests (no live kernel):

- `resolve_python`: precedence — `.venv` (created in a temp dir) beats a set
  `TALIESIN_PYTHON` **(the actual 2026-07-11 bug)**; field beats `.venv`; falls to env then
  `python3`; `.venv/bin/python` preferred over `.venv/bin/python3`.
- `resolve_r`: field → env → `R`, with **no** `.venv` step.
- `Provenance::label` strings.
- `SiteConfig` parse: `python:`/`r:` populate the fields; a typo (`pyton:`) warns via
  `NATIVE_KEYS`.

Wiring / integration:

- Warm pool boots for provenance ∈ {Field, Venv, Env} and is inert for `Default` (unit-test
  the gate arithmetic, mirroring `try_reserve_slot`'s pure-test style).
- Kernel-gated: `probe` reports `ipykernel` presence truthfully against `TALIESIN_PYTHON`;
  `check` on a python-cell corpus doc prints the Environment section.
- Corpus regression net stays green (`cargo test -p taliesin-core` + server crate tests).

## Files touched

| File | Change |
|------|--------|
| `crates/server/src/interpreter.rs` | **new** — pure resolution + probe |
| `crates/server/src/exec.rs` | `set_interpreters` setter; log-once at kernel start; path in `diagnostic` (careful, minimal) |
| `crates/server/src/warm_pool.rs` | `boot_pool` + `warm_pool_for_*` take resolved python + provenance gate |
| `crates/server/src/serve/mod.rs` | resolve + `set_interpreters` (single-doc) |
| `crates/server/src/serve_site/mod.rs` + `exec_pool.rs` | resolve from `config` + site root; feed pool + executors |
| `crates/server/src/build.rs` | resolve from `config`; feed pool + executors |
| `crates/server/src/check.rs` | Environment section (informational) |
| `crates/server/src/main.rs` / `lib.rs` | register `mod interpreter` |
| `crates/core/src/site/config/mod.rs` | `python:`/`r:` fields + `NATIVE_KEYS` |
| docs (guide reference) | document `python:`/`r:` + `check` Environment section (small) |
