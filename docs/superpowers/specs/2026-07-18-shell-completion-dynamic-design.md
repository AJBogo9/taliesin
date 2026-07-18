# Dynamic shell completion: `.tmd`-aware `preview`/`build` targets

**Date:** 2026-07-18
**Status:** design, pending implementation
**Scope:** developer-tooling ergonomics only. Not a framework capability, not
corpus-pinned, does not touch rendering / the block model / any load-bearing invariant.

## Problem

Pressing Tab after `tali preview` or `tali build` offers every file and directory in
the tree. Finding the `.tmd` document (or the site/book root) you actually want to
open means wading past `target/`, `.git/`, `Cargo.lock`, and the rest. The author
wants Tab to surface only plausible targets.

The `completions` subcommand already exists ([cli.rs](../../../crates/server/src/cli.rs),
`cmd_completions` + `completions_script`) and ships static scripts for bash/zsh/fish.
But every script falls back to unfiltered file completion for path arguments
(`compgen -f` in bash, `_files` in zsh, fish's default). So the machinery to *install*
completion is present; the *filtering* is what's missing. This is an upgrade of that
subcommand from static to dynamic, not a greenfield feature.

## Goals

- Tab on a path argument offers **only** `.tmd` files plus directories that can lead to
  one, with site/book roots surfaced first.
- Tab offers subcommand names and per-subcommand flags, with inline descriptions where
  the shell supports them.
- One place owns all the logic (Rust), so behavior can't drift between shells.
- Cover **four** shells: zsh, bash, fish, PowerShell.
- No new document configuration. Installation is a documented one-liner per shell.

## Non-goals

- No `--install` command that writes into shell config dirs (OS-path detection is
  fiddly and failure-prone; documented one-liners are safer and predictable).
- No `clap` migration. The CLI parses `std::env::args()` by hand; a `clap_complete`
  rewrite is out of proportion to this feature.
- No corpus document. Nothing under `corpus/` exercises shell completion, by design.

## Approach (chosen)

**One Rust brain, four thin shims** (the cobra / `gh` / `rustup` model), chosen over:

- *Extending the static scripts per shell* — would re-implement the `.tmd`-filtering
  and dir-pruning logic four times in four shell dialects, kept in sync by hand.
- *`clap_complete`* — requires replacing the hand-rolled arg parser.

The dynamic model writes the filtering logic once in Rust and gives each shell a small
shim that calls it, so adding a shell later is nearly free and no per-shell copy can go
stale.

## Architecture

### Two subcommands

- **`taliesin __complete <words…>`** — hidden runtime brain. The shim passes every word
  typed after `taliesin`; the final element is the current (possibly empty) word under
  the cursor. Prints candidates + a directive, exits 0. Never appears in `usage()`,
  `COMMANDS`, or `subcommand_help` (so it stays out of help and did-you-mean).
- **`taliesin completions <bash|zsh|fish|powershell>`** — existing public subcommand,
  extended. Prints the shim for that shell. `powershell` is added; the three existing
  entries are rewritten from static bodies to dynamic shims.

### Where the code lives

New module `crates/server/src/complete.rs` owns the brain (`__complete`) and the shim
templates. `cmd_completions` moves there from `cli.rs` (or stays in `cli.rs` and calls
into `complete.rs` for the brain — implementer's choice; the brain and its tests are
the substance). Dispatch in [main.rs](../../../crates/server/src/main.rs):

```rust
Some("__complete")  => complete::cmd_complete(&args),      // hidden
Some("completions") => complete::cmd_completions(&args),   // public (moved)
```

`__complete` is **not** added to `COMMANDS` (the did-you-mean / help list). `completions`
is already there. The `completions <bash|zsh|fish>` help string in `main.rs` gains
`powershell`.

## The wire protocol (cobra-compatible)

`__complete` prints zero or more candidate lines, then a final directive line:

```
corpus/          # directory candidate — trailing slash
index.tmd
about.tmd
:5               # directive bitmask (last line, starts with ':')
```

- Candidate line: `value` or `value\tdescription`. `value` is the **whole word** as it
  should appear after completion (`corpus/foo.tmd`, not the leaf `foo.tmd`), which is
  unambiguous across all four shells. `description` (optional, after a tab) is shown by
  zsh / fish / PowerShell and ignored by bash.
- Directory candidates end in `/`.
- Directive bits mirror cobra so the shims are near-drop-in (written fresh, not copied):
  `NoSpace = 1`, `NoFileComp = 4`.
  - Filtered results (subcommands, flags, `.tmd`/dir sets): `NoFileComp` set, so the
    shell does **not** fall back to showing everything. Directory-bearing results also
    set `NoSpace` (→ `:5`) so completing a dir doesn't insert a space and you keep
    descending.
  - Positions we deliberately don't specialize (see below): emit `:0` so the shell
    does its normal file completion.

## Brain behavior, by cursor position

Let `words` be the args after `taliesin`, `cur` the last (current) word, `sub` the
resolved subcommand (`words[0]`, with `dev`/`serve` → `preview`).

1. **Completing the subcommand** (`words.len() == 1`, `cur` has no leading `-`):
   return public subcommand names matching `cur` (from `COMMANDS` minus `help`, each
   with a one-line description drawn from the same source as `usage()` where practical).
2. **`cur` starts with `-`**: return the flags valid for `sub`, with descriptions.
   Flag sets per subcommand come from a single table in `complete.rs` (see below).
3. **Enumerated positional**: `new <TAB>` → `post page deck paper`; `completions <TAB>`
   → `bash zsh fish powershell`; `--format <TAB>` → `human json` (or `json` for build).
4. **First path positional** for a path-taking subcommand: the filtered set (next
   section), respecting whether the subcommand wants a file, a dir, or both.
5. **Anything else** (e.g. `build`'s second positional output path, `--out <dir>`
   values, free-form flag values like `--project-name`): directive `:0`, plain file
   completion.

### Per-subcommand first-positional target type

| Subcommand(s)                          | First positional | Offer |
|----------------------------------------|-------------------|-------|
| `preview` / `dev` / `serve`, `build`, `check` | file **or** dir | `.tmd` files + descendable dirs |
| `render`, `read`, `blocks`, `symbols`  | file            | `.tmd` files + descendable dirs |
| `map`, `publish`                       | dir             | descendable dirs only |
| `init`                                 | dir (optional)  | descendable dirs only |
| `new`                                  | kind enum       | `post page deck paper` |
| `schema`, `vocab`, `mcp`               | none / flags    | flags only |

"Descendable dir" = a directory offered so you can navigate into it, subject to the
pruning rule. File-only subcommands still offer dirs (you must be able to descend to a
nested `.tmd`); they just never offer non-`.tmd` files as terminal candidates.

### Path filtering rule

Given `cur` (a partial path), resolve its parent directory and leaf prefix, then within
that parent:

- **Files:** offer entries ending in `.tmd` whose name matches the leaf prefix.
- **Directories:** offer entries matching the leaf prefix that **contain a `.tmd`
  anywhere below**, each with a trailing `/`. The containment test is a recursive walk
  that:
  - ignores `.git`, `target`, `node_modules`, `_site`, `_freeze`, and dot-directories;
  - is depth-capped (≈6) so a pathological tree can't stall a keystroke;
  - short-circuits on the first `.tmd` found.
- **Ordering:** directories that are **site/book roots** (contain `_site.yml`) sort
  first, then remaining dirs, then `.tmd` files; each group alphabetical.
- A dir whose only `.tmd` is buried deep still appears (recursive test), so you can
  always reach a valid target. Only genuinely `.tmd`-free subtrees are hidden — exactly
  the requested behavior.

### Flag table

A single `const` table in `complete.rs` maps each subcommand → its flags (+ one-line
descriptions), e.g. `preview` → `--host`, `--open`, `--no-exec`, `--port`; `build` →
`--out`, `--strict`, `--bare`, `--jobs`, `--format`; `publish` → `--project-name`,
`--out`, `--public`, `--no-strict`, `--dry-run`, `--format`; `new` → `--dir`, `--json`;
etc. This table is the single source of truth for flag completion and is asserted
against the help text by a test (below), the same anti-drift discipline the existing
`@COMMANDS@` placeholder uses.

## The shims

Each generated shim is small and does the same three things: assemble the current
words, call `taliesin __complete …`, and feed the parsed candidates + directive to the
shell's completion mechanism (`compadd` for zsh, `COMPREPLY` for bash, a function-driven
`complete` for fish, `Register-ArgumentCompleter` for PowerShell). Trailing `/` on a
candidate + the `NoSpace` directive drive descend-without-space. Modeled on cobra's
proven shim logic; written fresh to avoid license entanglement.

Install hints (already present for three shells; PowerShell added) stay as header
comments and in `subcommand_help("completions")`:

```
bash        taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin
zsh         taliesin completions zsh  > "${fpath[1]}/_taliesin"    # then: compinit
fish        taliesin completions fish > ~/.config/fish/completions/taliesin.fish
powershell  taliesin completions powershell >> $PROFILE
```

## Launcher fast-path

The machine-local launcher [~/.local/bin/taliesin](/home/bogo/.local/bin/taliesin)
runs a `find -newer` rebuild check on every invocation. A one-line guard makes it skip
that check and `exec` the current binary immediately when `$1 == __complete`, so Tab
never triggers a rebuild or prints `building…` mid-keystroke. Distributed users invoke
the real binary directly and are unaffected. (This edit is outside the repo; noted here
for completeness, applied during implementation.)

## Testing

The brain is shared by all four shells, so it carries the test weight; the shims are
verified structurally + by live smoke where the shell is available.

- **`crates/server/tests/complete_cli.rs`** (new): build a temp tree via `tempfile` —
  a top-level `.tmd`, a `_site.yml` site dir with a `.tmd`, a nested-only-`.tmd` dir, an
  empty dir, and a `target/` dir with a decoy `.tmd`. Drive `taliesin __complete …`
  through `assert_cmd` and assert:
  - `.tmd` files present; the empty dir and `target/` **absent**;
  - the nested-only dir **present** (recursive containment);
  - the `_site.yml` dir sorted first;
  - directory candidates end in `/`; the directive is `:5` for dir-bearing results;
  - `__complete` with one word lists subcommand names; `preview -` lists preview flags;
    `new ''` lists the four kinds.
- **Extend `completions_tests`**: the existing
  `every_shell_script_offers_exactly_the_dispatched_command_list` guard is kept/adapted;
  add a case that `completions powershell` prints a non-empty script with its sentinel,
  and that each shim references `__complete`.
- **Flag-table drift guard**: a test asserting every flag named in a subcommand's help
  text appears in the flag table (mirrors `env_help_lists_every_runtime_env_var`).
- **Live smoke**: exercise the zsh shim in-session (this environment's shell is zsh) and
  bash if present. fish / PowerShell are likely absent in the sandbox and will be marked
  *not live-verified* for the author to confirm on their machine.

## Documentation

- The `completions` help string in `main.rs` + `subcommand_help("completions")` gain
  `powershell`.
- A short **Shell completion** page under `docs/guide/reference/` (dogfooded `.tmd`):
  what it does, the copy-paste install line per shell, and the one behavioral note (only
  `.tmd`-bearing paths are offered; site roots first). Added in the same change so the
  guide never advertises a feature it doesn't document.

## Risks / open questions

- **Per-keystroke latency.** The recursive containment walk runs on every Tab. Mitigated
  by the ignore-set, depth cap, and first-hit short-circuit; the temp-tree test can
  assert a generous upper bound but real-world feel is confirmed by live smoke.
- **`cur` with an absolute path or `~`.** v1 resolves relative to CWD and treats an
  absolute `cur` relative to `/`; `~` expansion is left to the shell before the word
  reaches `__complete`. Documented, not a blocker.
- **Second-positional detection.** v1 only specializes the *first* non-flag positional;
  later positionals get plain file completion (`:0`). Keeps the arg-position logic
  simple and is correct for every current subcommand.
