# PL15 — Document `new --draft`/`--tour`; derive the drift-prone `usage:` one-liners

`new` parses `--draft`/`--tour` (and the `init` scaffold *advertises* `--draft`), but both help
surfaces listed only `[--dir] [--json]`. Separately, the hand-maintained one-line `usage:`
strings had drifted — `build`'s omitted the `--format json` that `subcommand_help("build")`
documents.

## Fix

- **Document `--draft`/`--tour`** on `new` across all three surfaces: `subcommand_help("new")`
  (synopsis + flags), the top-level `usage()` line, and — because a consistency test enforces
  it — the shell-completion `flags_for("new")` table (so they tab-complete too).
- **Single source of truth for the `usage:` synopsis.** New `command_synopsis(cmd)` returns the
  first line of `subcommand_help(cmd)`. The `new` (cli.rs) and `build` (build.rs)
  missing-positional errors now print `usage: {command_synopsis(cmd)}` instead of a parallel
  hand-written literal, so they can't drift from the `--help` block. This fixes the `build` case
  concretely (it now carries `--format json`) and future-proofs `new`.

*Scope note:* only the two commands the audit named (`new`, `build`) were rerouted through
`command_synopsis`. The other missing-positional one-liners (render/read/blocks/map/symbols/
check/publish) still print their own literals; routing all of them is a larger cross-file change
than this item's "Trivial-S", deferred rather than rushed.

## Tests

- Extended `main.rs::subcommand_help_covers_documented_commands`: `new` help documents
  `--draft` + `--tour`; the derived `command_synopsis("new")` carries them and
  `command_synopsis("build")` carries `--format json`. Mutation-checked (removing `--tour` from
  the synopsis fails it).
- `complete.rs::flag_table_covers_help` (pre-existing drift gate) now also proves the new flags
  are completable. Verified end-to-end: `taliesin new` / `taliesin build` with no positional
  print the correct, flag-complete synopsis.
