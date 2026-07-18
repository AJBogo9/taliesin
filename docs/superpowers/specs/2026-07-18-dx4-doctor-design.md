# DX4 — `taliesin doctor`, a standalone environment self-audit

Date: 2026-07-18. Backlog item **DX4** (§6 DX audit batch, Tier 2). Branch `dx4-doctor`.
Detail source: `notes/2026-07-18-dx-audit.md` (row 4; persona 🎓 Priya).

> **Autonomy note:** author asked me to continue without the interactive gate. The design
> fork (severity model + exit code, scope of checks, path arg, JSON) is resolved below with
> documented defaults, matching the DX5/DX11/DX10-followup pattern, for async review.

## Goal

A first-time user's first code cell fails on kernel wiring and they reach for a
`flutter doctor` / `quarto check`: a single command that says, up front, whether their
environment is ready to execute cells. Taliesin already *has* the probe logic
(`interpreter::probe`), but it is **buried inside `check` and only runs for languages a
document already uses** (circular: you can't diagnose a Python setup until you already have a
working-enough doc with a Python cell). `taliesin doctor` surfaces it as a standalone,
**unconditional** audit of Python and R interpreter + Jupyter kernel package, with ✓/⚠/✗
status, concrete fix commands, and **conda/active-env detection** (new: today only `.venv` +
`TALIESIN_PYTHON` feed resolution; nothing reports which env is *active*).

## Ground truth (grepped + measured against the running product 2026-07-18)

- **The probe is already a pure, reusable module** ([`interpreter.rs`](../../../crates/server/src/interpreter.rs)):
  `resolve_python(field, dir)` / `resolve_r(field, dir)` → `Resolved { path, provenance }`
  (`Provenance` = Field > Venv > Env > Default, with a human `label(lang)`); `probe(&Resolved,
  Lang)` → `Probe { runs, version, kernel_pkg_ok, error }` (spawns `<bin> --version` + an
  `import ipykernel` / `library(IRkernel)`, never runs the doc, never panics).
- **`check` wraps it circularly** ([`check.rs:239-335`](../../../crates/server/src/check.rs)):
  `used_languages(blocks)` scans the *rendered doc's cells*, and only those languages get an
  `EnvEntry` (resolve + probe). A prose-only doc, or a doc whose Python cell you haven't
  written yet, reports nothing. The human format is
  `  python: <path> (<provenance>), ipykernel present (<version>)` on stderr
  ([`check.rs:460-476`](../../../crates/server/src/check.rs)).
- **Ran the product:** `check` on a `{python}` doc →
  `Environment:\n  python: python3 (TALIESIN_PYTHON), ipykernel MISSING`. So the pieces exist;
  DX4 is to run them unconditionally and present them as a self-audit.
- **No active-env detection today.** `grep CONDA_PREFIX|VIRTUAL_ENV|conda` over
  `crates/server/src` finds only comments. This is the net-new piece.
- **Registration is guard-tested** (adding a subcommand touches four coupled places):
  - `every_dispatched_command_is_listed_in_commands` ([`main.rs:606`](../../../crates/server/src/main.rs)):
    a `Some("doctor") =>` arm in the dispatch match *requires* `"doctor"` in `COMMANDS`.
  - `subcommand_help_covers_documented_commands` ([`main.rs:624`](../../../crates/server/src/main.rs)):
    every `COMMANDS` entry (minus aliases/meta) *requires* a `subcommand_help("doctor")` page
    that names itself and contains `taliesin`.
  - `env_help_lists_every_runtime_env_var` ([`main.rs:448`](../../../crates/server/src/main.rs))
    only scans `TALIESIN_*` reads, so reading `CONDA_PREFIX`/`VIRTUAL_ENV` (non-`TALIESIN_`) is
    invisible to it. No `ENV_HELP` change needed.
- **`log` colorizes on stderr only** (`paint`/`colored` are private, gated on
  `stderr().is_terminal()`). `doctor`'s report is its primary output → stdout, so it needs a
  small stdout-aware color helper (NO_COLOR + `stdout().is_terminal()`), same ANSI palette.

## Resolved decisions (autonomous, documented)

1. **`doctor [dir]` (default `.`).** Resolves interpreters against the target dir, honouring an
   `_site.yml` `python:`/`r:` field and a project `.venv` (exactly as a build/preview would), so
   it audits the *real* project setup, not a generic one. A single-doc project has no config
   fields → `None` (bare resolution).
2. **Probes BOTH Python and R, unconditionally** — the un-circular fix (the backlog's core
   complaint). `doctor` has no document, so it cannot know which language you need; it reports
   both.
3. **Three severities, exit non-zero iff any ✗:**
   - **✓ ready** — the interpreter runs *and* its kernel package imports.
   - **⚠ warn** — the interpreter runs but the kernel package is missing (→ a concrete `fix:`
     install command), OR the *default* interpreter (`python3`/`R`, `Provenance::Default`) does
     not run (you simply don't have it; not a misconfiguration). Never gates the exit.
   - **✗ error** — an *explicitly configured* interpreter (`Provenance::Field`/`Env`/`Venv`)
     does not run at all: a broken `TALIESIN_PYTHON`, a bad `_site.yml python:`, a dead
     `.venv`. This is a real misconfiguration → exit non-zero.
   - Rationale: `doctor` is a human triage tool (informational, like `flutter doctor`), but a
     pointed-at-and-broken interpreter is unambiguously wrong and worth a scriptable failure.
     Exit-gating on *kernel readiness* for CI is a separate concern (DX18 `--require-kernel`).
4. **Active-env detection (new).** One informational line: if `CONDA_PREFIX` is set, name the
   conda env (`CONDA_DEFAULT_ENV` or the prefix's basename); else if `VIRTUAL_ENV` is set, name
   it; else "no active virtual/conda env (using the system PATH)". Answers "did you forget to
   activate your env?" It never gates (always ✓/informational). Reads the vars in `cmd_doctor`
   and passes them into a *pure* `active_env_check` (env injected, testable — the
   `interpreter.rs` discipline).
5. **Config sanity (light).** If the target dir has an `_site.yml`, ✓ when
   `Site::discover` yields no malformed-config warning (reusing
   `taliesin_core::site::is_malformed_config_warning`, as the build does), ⚠ otherwise (names
   the warning). Skipped entirely when there is no `_site.yml` (a single-doc project has none —
   not a problem).
6. **`--format json`** (consistent with `check`/`build`/`map`/`symbols`): emits
   `{ "ok": bool, "checks": [ {name, status, detail, fix?} ] }` to stdout, so an agent setting
   up an environment can verify it. Human format is the default.
7. **Pure, testable core** (mirrors `interpreter.rs`): `interpreter_check(lang, &Resolved,
   &Probe) -> Check`, `active_env_check(virtual_env, conda_prefix, conda_name) -> Check`, and
   `overall_ok(&[Check]) -> bool` are pure (probe + env injected); `cmd_doctor` is the thin I/O
   wrapper that resolves, probes, reads env, prints, and exits.

## Output (human)

```
taliesin doctor  ·  is your environment ready to run code cells?

  ✓  python   /proj/.venv/bin/python (.venv)
              Python 3.11.4  ·  ipykernel present
  ⚠  r        R (default)
              interpreter not found  ·  R cells will render as source
              fix:  install R, then in R: install.packages("IRkernel")
  ✓  env      active conda env: myproject
  ✓  config   _site.yml is valid

  1 ready, 1 warning.  Python cells will execute; R cells render as source.
```

A ✗ example (broken `TALIESIN_PYTHON`):

```
  ✗  python   /bad/path/python (TALIESIN_PYTHON)
              cannot run /bad/path/python: No such file or directory
              fix:  point TALIESIN_PYTHON at a real interpreter, or unset it
```

Fix commands:
- Python, kernel missing: `<resolved-python> -m pip install ipykernel`
- R, kernel missing: `<resolved-R> -e "install.packages('IRkernel')"`
- R absent (default): `install R, then in R: install.packages("IRkernel")`
- Configured interpreter broken: `point <provenance> at a real interpreter, or unset it`

## Changes

### `crates/server/src/doctor.rs` (new)
- `enum Status { Ok, Warn, Error }` (+ glyph `✓`/`⚠`/`✗` + ANSI color + serde `Serialize` as
  `"ok"`/`"warn"`/`"error"`).
- `struct Check { name, status, detail, fix: Option<String> }` (serde `Serialize`).
- `fn interpreter_check(lang: interpreter::Lang, r: &Resolved, p: &Probe) -> Check` — pure
  mapping (the severity model in decision 3 + the fix command).
- `fn active_env_check(venv: Option<&str>, conda_prefix: Option<&str>, conda_name: Option<&str>) -> Check`
  — pure.
- `fn overall_ok(checks: &[Check]) -> bool` — no `Status::Error` present.
- `fn cmd_doctor(args: &[String]) -> ExitCode` — parse `[dir]` + `--format json`; discover
  `_site.yml` fields; resolve + probe both langs; read `VIRTUAL_ENV`/`CONDA_PREFIX`/
  `CONDA_DEFAULT_ENV`; optional config check; print human or JSON; exit `SUCCESS`/`FAILURE`
  per `overall_ok`.
- A small stdout color helper (NO_COLOR + `stdout().is_terminal()`).

### `crates/server/src/main.rs`
- `mod doctor;`
- dispatch: `Some("doctor") => doctor::cmd_doctor(&args),`
- `COMMANDS`: add `"doctor"`.
- `subcommand_help`: a `"doctor"` page (names `doctor`, shows a `taliesin doctor` example).
- `usage()`: a `doctor` line under COMMANDS.

### Tests
- `doctor.rs` unit tests: `interpreter_check` for each (✓ ready; ⚠ kernel-missing carries the
  `-m pip install ipykernel` fix; ✗ Env-provenance-broken; ⚠ Default-broken not ✗);
  `active_env_check` (conda named; venv named; neither → "no active"); `overall_ok` (a ✗ →
  false, only ✓/⚠ → true).
- `crates/server/tests/doctor_cli.rs`: run the real binary — `taliesin doctor` prints the
  `python`/`r`/`env` sections; `--format json` emits `{ok, checks:[…]}` parseable JSON with a
  `python` check; a broken `TALIESIN_PYTHON=/bad/path` makes it exit non-zero with a ✗.

## Non-goals

- **Auto-installing kernels / creating venvs.** `doctor` diagnoses and prints the fix command;
  it never mutates the environment.
- **CI exit-gating on kernel readiness** (a *warning*-level gate). That is DX18
  (`check --require-kernel`); `doctor` only fails on a ✗ (configured-and-broken).
- **Non-kernel toolchain checks** (Node for `{js}` equivalence, Cloudflare for `publish`): out
  of scope; `doctor` is about cell execution readiness.

## Invariant safety

A new read-only subcommand + one pure module. No render-pipeline change, no output-format
change, no CDN, no preview write-back, no execution of the user's document (probe only spawns
`--version` + a kernel-package import). `data-block-id`/`data-sourcepos`, `MAX_WARM_PAGES` +
`exec_pool.rs` LRU untouched. Reuses the already-pinned `interpreter` module verbatim.
