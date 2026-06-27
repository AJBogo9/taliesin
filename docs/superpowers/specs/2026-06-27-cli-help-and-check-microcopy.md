# CLI help + check microcopy hardening (2026-06-27)

Release-hardening lane: three CLI text/microcopy fixes in `crates/server/src/main.rs`,
plus a tiny optional kernel-hint correctness fix. The arg parser stays hand-rolled
(intentionally not clap). Documentation lands in `docs/guide/reference/cli.qmd`.

## 1. Per-subcommand `--help` / `-h`

Today only the top-level `usage()` exists; `qmd-fast build --help` falls through to
`cmd_build`, which treats `--help` as an unknown flag and then errors on a missing
positional. Goal: `qmd-fast <cmd> --help` (and `-h`) prints a focused help for that
subcommand (synopsis + its flags + a one-line example) and exits 0.

Design:
- A new `subcommand_help(cmd: &str) -> Option<&'static str>` returns the focused help
  text for a known subcommand (canonical name; aliases map to their canonical help).
- `main()` dispatch detects a `--help`/`-h` token appearing *after* a known
  subcommand and, before dispatching to the `cmd_*`, prints that subcommand's help and
  returns `ExitCode::SUCCESS`. Kept as a simple match, matching the hand-rolled style.
- Covered subcommands: `preview` (+ aliases `dev`/`serve`), `build`, `check`,
  `render`, `schema`, `blocks`, `init`.
- The top-level `--help`/`-h`/`help`/no-args path is unchanged (prints `usage()`).

## 2. `check --format json` honesty

`cmd_check`'s error path (unreadable/missing file, empty site) calls `log::error`,
which writes *human* text to stderr even when `--format json` was requested — so
`qmd-fast check missing.qmd --format json | jq` chokes on a non-JSON stream mixed
into the pipeline (and stdout is silent, so `jq` sees empty input).

Fix: when `--format json` is set and `collect_diagnostics` returns `Err`, emit a
single JSON object `{"error": "<message>"}` to **stdout** and exit non-zero, so the
JSON stream stays valid and parseable. Human format keeps the current
`log::error` stderr message. The `--format` value is parsed *before* the error path
so the format is known when the error is produced. (Argument errors that happen
before a format is even parsable — missing path, bad `--format` value — stay human;
those are usage errors, not check results.)

## 3. `_quarto.yml` migration breadcrumb

A directory with a `_quarto.yml` but no `_site.yml` currently yields the confusing
diagnostic `_site.yml: no _site.yml at <root>` (the message names a file that does
not exist in the project). For a user arriving from Quarto this is a dead end.

Fix: a small helper `quarto_migration_hint(dir: &Path) -> Option<String>` returns a
breadcrumb when `dir` has `_quarto.yml` and no `_site.yml`:

> found `_quarto.yml` — qmd-fast uses `_site.yml` (a flat native schema), not
> Quarto's `_quarto.yml`; run `qmd-fast init` or see the docs.

It is checked at the `check`/`build` directory entry (in `main.rs`, not core):
- `cmd_check`: when the target is such a directory, emit the breadcrumb (respecting
  `--format`: a `{file:"_quarto.yml", message:…}` diagnostic for json / a
  `path: message` line for human) and exit non-zero, *instead of* running the site
  diagnostics that would surface the confusing core warning.
- `build_site`: log the breadcrumb (warn) so the same dir built with `build` is not
  left only with `no _site.yml at <root>`.

No change to `crates/core/src/site/` is required (the helper is pure `Path` probing in
main.rs), keeping this lane out of the site module.

## Optional: language-aware kernel-unavailable hint (done)

`build_page_executing` logged a hardcoded `QMD_FAST_PYTHON` hint even when an **R**
cell's kernel was the one that failed. `exec::Executor::diagnostic()` already returns
a language-aware message that names the right env var (`QMD_FAST_R` for R). The fix is
purely in `main.rs`: log `ex.diagnostic()`'s text when present instead of the
hardcoded string. No change to `exec.rs`/`kernel.rs` (Do-NOT-touch) is needed.

## Tests (new `cli_microcopy_tests` module at the end of the test region)

- `check --format json` on a missing path yields valid JSON (`{"error":…}`) — pin the
  shape via a small `json_error(msg)` helper that the error path also uses.
- `quarto_migration_hint` fires for a `_quarto.yml`-only dir and is `None` once a
  `_site.yml` is present (and `None` for a plain dir).
- `subcommand_help` returns help text for each covered subcommand (and `None` for an
  unknown one), and each text names the subcommand + an example.

## Invariants held

- Single editing surface, HTML-only, Do-NOT-touch (exec/freeze/kernel, divs/cite/
  includes/numbering) all untouched.
- Does not touch `collect_diagnostics` or the corpus guard test (another lane owns
  those); new tests live in a separate module at the end of the file.
