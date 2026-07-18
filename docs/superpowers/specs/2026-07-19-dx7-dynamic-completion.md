# DX7 — Dynamic value completion (spec)

Date: 2026-07-19. Extends the shipped shell-completion + companion-completion surfaces.

## What the audit asked, measured against the product

DX7 (audit item #7): "complete page/deck names, post slugs, `@`-xref targets *with
descriptions* (gh/fzf pattern) + a one-liner 'install completion now'."

Grepping the named surfaces first (per the backlog's anti-rot rule) shows the flagship
piece is **already shipped**:

- **`@`-xref completion with descriptions** — `editor/vscode/src/completions.ts`
  (`xref` case) already merges buffer `{#id}` anchors + the `taliesin symbols` registry
  and shows `Figure N` / `Section N` details. Citation keys, div-class, cell-option and
  front-matter completion are shipped too. **Do not rebuild.**
- **page/deck/slug values in the *shell*** — pages/decks/posts are `.tmd` *paths*, which
  `crates/server/src/complete.rs` already path-completes (site/book roots first). There is
  no CLI slot that takes an `@`-xref or a bare slug. So the shell side is covered by the
  existing path completion; nothing to add there.

Two genuine gaps remain, and this change ships both:

1. **Shell: `taliesin completions --install`** — today the user must hand-run
   `taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin`.
   `--install` detects the shell (from `$SHELL`, overridable with an explicit
   `completions <shell> --install`) and writes the script to the conventional per-shell
   path, printing a confirmation + any manual follow-up.
2. **Editor: `{{< embed … >}}` / `{{< include … >}}` file-target completion** — the one
   place "page/deck names, post slugs" appear as *values in a document*. Typing the first
   argument of an `embed`/`include` shortcode offers the sibling `.tmd` files +
   directories (to descend), relative to the current document.

## Part 1 — shell install (Rust, `crates/server/src/complete.rs`)

Pure, unit-testable core; a thin wrapper does the I/O.

- `canonical_shell(&str) -> Option<&'static str>`: `bash|zsh|fish|powershell|pwsh` → canonical.
- `detect_shell(shell_env: Option<&str>) -> Option<&'static str>`: basename of `$SHELL`,
  then `canonical_shell`.
- `install_plan(shell, &InstallEnv) -> Option<InstallPlan>` where
  `InstallEnv { home, xdg_data, xdg_config }` (all `Option<String>`, read from env in
  `from_env`) and
  ```
  enum InstallPlan {
      Write { path: PathBuf, manual: Option<String> }, // bash / zsh / fish
      Manual { command: String },                       // powershell (no reliable auto path)
  }
  ```
  Paths (XDG-aware):
  - bash → `${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions/taliesin`, no manual.
  - zsh  → `${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions/_taliesin`, manual = add
    that dir to `fpath` before `compinit`.
  - fish → `${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions/taliesin.fish`, no manual.
  - powershell → `Manual { "taliesin completions powershell >> $PROFILE" }`.
- `cmd_completions` gains an `--install` branch (flag anywhere in the args; the shell is the
  first non-flag arg, or detected). No `--install` → unchanged (print script to stdout).
- `flags_for("completions")` gains `("--install", false, …)` so `completions --<TAB>`
  offers it and `flag_table_covers_help` stays green once the help text mentions `--install`.
- The `completions` positional (shell kinds) is offered whenever no shell positional is
  present yet, so `completions --install <TAB>` still offers shells.

## Part 2 — editor shortcode completion (TS, companion)

- `complete.ts`: add `{ kind: "shortcode-path"; shortcode: "embed" | "include"; typed }` to
  `CompletionContext`, detected by `\{\{<\s*(embed|include)\s+([^\s>]*)$` on the line prefix
  (matches only while typing the *first* argument). Add a pure
  `shortcodePathCandidates(entries, typed, fileDetail)` that filters `.tmd` files + subdirs
  by the typed leaf and returns relative insert-values (dirs suffixed `/`, ignore-dirs
  hidden). Both are `vscode`-free → unit-tested in `node:test`.
- `completions.ts`: the `shortcode-path` case reads the doc-relative directory, calls the
  pure helper (`fileDetail` = "deck / page" for embed, "partial" for include), and maps to
  `CompletionItem`s with a replace range covering the typed path (folders re-trigger
  suggest to keep descending). Register `/` as an extra trigger char.

## Out of scope (noted, not built)

- Deck-vs-page labelling by reading each candidate's front-matter (per-candidate file I/O in
  the completion hot path). The uniform "deck / page" detail is honest; a cheap first-line
  peek is a clean future enhancement.
- Internal `[](page.tmd)` markdown-link target completion (a larger, separate context).

## Verification

- `cargo test -p taliesin-core -p taliesin-server` (drift tests + new `install_plan` tests).
- End-to-end: run `taliesin completions --install` under a throwaway `HOME`/`$SHELL` and
  assert the file lands.
- `npm test` in `editor/vscode` (node:test) + `npm run build` + a tsc typecheck.
