# DX6 — `check --explain <CODE>` + per-diagnostic `docs_url`

**Status:** spec (autonomous; the author is away — see *Resolved decisions* below).
**Persona:** 🎓 (a learner meeting a diagnostic code for the first time), 🤖 (an agent that
matched a `TAL-*` code in `check --format json` and wants the canonical fix).
**Size:** S–M · new.
**Backlog:** §6 DX audit batch, DX6; rationale `notes/2026-07-18-dx-audit.md` row 6.

## Problem

`check --format json` already stamps every diagnostic with a stable `TAL-*` code
(`crates/core/src/diagnostics/codes.rs`). But a code is only a *label*: nothing expands
`TAL-XREF-UNREF` into "why did this fire, and what is the one edit that fixes it?". rustc
solved exactly this with `rustc --explain E0502`. Taliesin's codes should be equally
actionable: a reader (or an agent) holding a code should be able to turn it into cause +
canonical fix without scraping prose or reading source.

Two deliverables, from the backlog line ("`check --explain <code>` + a per-diagnostic
`docs_url`"):

1. **`taliesin check --explain <CODE>`** — expand a code into `title` / `cause` / `fix`,
   rustc `--explain` style, in the terminal (offline, always correct).
2. **A per-diagnostic `docs_url`** — every diagnostic in `check --format json` (and the
   shared `build`/`publish` `diagnostics_json`) carries a `docs_url` pointing at where to
   read more about its code.

## Current state (measured, not assumed)

- `crates/core/src/diagnostics/codes.rs` holds a `TABLE` of `(message-substring, code,
  severity)` + `classify()` + `extract_suggestion()`. **24 distinct codes** exist today
  (23 families + the `GENERIC = "TAL-CHECK"` fallback). Measured live:
  `./target/debug/taliesin check corpus/diagnostics/typos.tmd --format json` emits real
  codes (`TAL-FM-KEY`, `TAL-XREF-UNDEF`, `TAL-CALLOUT-KIND`, `TAL-CELL-OPTION`,
  `TAL-CHECK`).
- `crates/server/src/check.rs` owns `Diagnostic { code, severity, file, line, message,
  suggestion? }`, built by `Diagnostic::new` which calls `codes::classify` +
  `codes::extract_suggestion`. `cmd_check` parses `--format` and rejects unknown `--flags`
  with a did-you-mean; `CHECK_FLAGS = &["--format"]`.
- `check --format human` output is deliberately **byte-identical to pre-codes** (codes never
  leak into linter lines); the codes-work test `check_human_output_is_unchanged_by_codes`
  pins this.
- **No `docs_url` concept exists anywhere** (grepped). The only canonical tool URL baked in
  is `https://github.com/AJBogo9/taliesin` (in `main.rs` `usage()`).
- Completion: `flags_for("check") = &[("--format", true, "human | json")]`; flag-value
  completion (`complete.rs` §3) special-cases `--format` → `human|json`. Drift gate
  `flag_table_covers_help` requires every `--flag` mentioned in `main.rs` help to live in
  some `flags_for` table.
- `taliesin_core::closest(key, &[&'static str]) -> Option<&'static str>` is the shared
  Levenshtein did-you-mean helper (defined in `frontmatter.rs`, used by `main.rs`).
- Bless idiom for generated-and-committed artifacts already exists: `schema.rs`'s
  `bless_or_assert` asserts a generated string equals a committed file, or rewrites it under
  `TALIESIN_BLESS=1`. Committed schema files live under `crates/core/assets/`.
- **This is a flag on an existing command, not a subcommand.** It therefore does NOT touch
  the 5 subcommand-guard sites (dispatch/`COMMANDS`/`subcommand_help`/`usage`/`command_desc`).
  It touches: `flags_for("check")`, the `check` help block, and (for value completion)
  `complete.rs` §3.

## Resolved decisions (autonomous, documented)

The backlog flagged four forks; each is resolved below with its rationale, in the
"Resolved decisions" pattern used by DX4/DX11.

### D1 — `--explain` is a flag on `check`, not a new subcommand

`taliesin check --explain <CODE>`, mirroring `rustc --explain E0502` (which the backlog
names). Codes are surfaced *by* `check`, so `check --explain` is the natural, discoverable
follow-up; keeping it a flag co-locates code-knowledge with the command that emits codes and
avoids adding a whole subcommand's guard surface. In `--explain` mode, `check` skips the
in-memory render / static-lint / environment-probe path entirely: it prints the explanation
and exits (`0` for a known code, non-zero for an unknown one). A positional path, if also
given, is ignored (matches rustc's `--explain E0502 foo.rs`).

### D2 — `--format` still applies to `--explain`

`--explain <CODE> --format json` emits a structured object
`{ code, title, cause, fix, docs_url }` (agent-grade, consistent with the codes work's
theme); default `human` prints a rustc-style block:

```
TAL-XREF-UNREF: a labeled float or theorem cannot be cross-referenced

<cause paragraph>

To fix: <fix paragraph>

Learn more: https://github.com/AJBogo9/taliesin/blob/main/docs/DIAGNOSTICS.md#tal-xref-unref
```

Reuses the existing `--format human|json` parsing and validation in `cmd_check`.

### D3 — the prose catalog lives next to the code table, drift-locked

A second table `EXPLANATIONS: &[Explanation]` in `crates/core/src/diagnostics/codes.rs`,
where `Explanation = { code, title, cause, fix }`, one entry per distinct code **including
`GENERIC` (`TAL-CHECK`)**. Two drift tests (the DX5 vocab-guard pattern):

- **completeness**: every distinct code in `TABLE` ∪ `{GENERIC}` has an `EXPLANATIONS`
  entry (a new family without an explanation fails the build).
- **no orphans**: every `EXPLANATIONS.code` is a real code (in `TABLE` or `GENERIC`).

`pub fn explain(code: &str) -> Option<&'static Explanation>` (case-insensitive) and
`pub fn all_codes() -> Vec<&'static str>` (sorted distinct codes, for completion + the
did-you-mean candidate set + the bare-`--explain` index) round it out.

### D4 — `docs_url` is computed, never stored; the anchor resolves for real

`pub fn docs_url(code: &str) -> String` returns
`format!("{DIAGNOSTICS_DOC_URL}#{}", code.to_ascii_lowercase())`, where
`DIAGNOSTICS_DOC_URL = "https://github.com/AJBogo9/taliesin/blob/main/docs/DIAGNOSTICS.md"`.
Being computed, it can never drift from the code.

It resolves for real because `docs/DIAGNOSTICS.md` is a **committed catalog generated from
`EXPLANATIONS`** (`diagnostics_markdown()`), with one `## TAL-FM-KEY` heading per code —
GitHub renders those as `#tal-fm-key` anchors. The file is drift-locked by a
`TALIESIN_BLESS=1` bless test that mirrors `schema.rs::bless_or_assert` (edit the table
without regenerating → the test fails on push). `docs/` already hosts loose repo markdown
(`docs/THEMING.md`) and its book-walker only ingests `.tmd`, so a `.md` there is inert.

Every `Diagnostic` gains a `docs_url` field, serialized in `--format json` only. `human`
`check` output stays byte-identical (the field never touches the linter lines). Because
`Diagnostic` is shared, `build`/`publish` `diagnostics_json` inherit `docs_url` too — the
same agent-grade shape everywhere.

### D5 — unknown / missing code behavior

- **Unknown code** (`check --explain BOGUS`): non-zero exit. `human` →
  `log::error("unknown diagnostic code `BOGUS`" [+ "(did you mean `TAL-…`?)" via
  `closest(code, all_codes())`] + a "run `taliesin check --explain` to list all codes"
  hint). `json` → the existing `{"error": …}` envelope (`check::json_error`), so a
  `| jq` pipeline stays valid. Lookup is case-insensitive (`tal-fm-key` works).
- **No code** (`check --explain` alone, or `--explain --format json`): **list all codes**
  (an index), exit `0`. `human` → `CODE — title` per line; `json` → an array of
  `{ code, title, docs_url }`. The vocabulary is small and closed, so enumeration is a
  genuine discoverability win ("what can `check` tell me?"); this deliberately deviates from
  rustc (which errors), justified by the closed, enumerable set. `--explain` consumes the
  next token as the code **only if it does not start with `-`** (so `--explain --format
  json` is index-mode-json, not "code = `--format`").

### D6 — `--explain` completes to the static code list

`flags_for("check")` gains `("--explain", true, "explain a diagnostic code (TAL-…)")`;
`complete.rs` §3 gains a `--explain` branch enumerating `all_codes()`. So
`taliesin check --explain <TAB>` offers the ~24 codes. This is **static-vocabulary**
completion (DX6 hygiene), distinct from DX7's dynamic-from-document completion — the code
set is fixed and drift-locked, not derived from the doc under the cursor.

## Scope / non-goals

- **In:** the `--explain` flag (human + json + index + unknown), `docs_url` on every
  diagnostic's JSON, the `EXPLANATIONS` catalog + drift tests, the generated
  `docs/DIAGNOSTICS.md`, `--explain` flag + value completion, the `check` help + `--help`
  block.
- **Out:** a published docs *website* with per-code pages (the tool ships no production
  domain; GitHub's markdown anchors are the honest, offline-consistent, always-resolvable
  home). Dynamic/document-derived completion (DX7). Changing any diagnostic *message* or
  *severity* (only *adding* an explanation layer on top). Human `check` output stays
  byte-identical.

## Files

- `crates/core/src/diagnostics/codes.rs` — `Explanation`, `EXPLANATIONS`, `explain`,
  `all_codes`, `DIAGNOSTICS_DOC_URL`, `docs_url`, `diagnostics_markdown`; drift + bless
  tests.
- `docs/DIAGNOSTICS.md` — NEW, generated catalog (blessed).
- `crates/server/src/check.rs` — `docs_url` on `Diagnostic`; `--explain` parse + the
  human/json/index/unknown render; `CHECK_FLAGS += "--explain"`; tests.
- `crates/server/src/main.rs` — `check` `usage()` line + `subcommand_help("check")` gain
  `--explain`.
- `crates/server/src/complete.rs` — `flags_for("check") += --explain`; §3 `--explain` value
  completion; tests.
- `crates/server/tests/check_cli.rs` — end-to-end `--explain` (human, json, unknown, index).

## Verification

- `cargo test -p taliesin-core` (drift + bless + explain unit tests) and
  `cargo test -p taliesin-server` (check.rs unit + `check_cli.rs` integration + the
  completion/help guard tests).
- `cargo fmt --check`, `cargo clippy`.
- Live product: `taliesin check --explain TAL-XREF-UNREF`, `… --format json | jq`,
  `… --explain bogus`, `… --explain` (index), and confirm `check corpus/diagnostics/typos.tmd
  --format json` now carries `docs_url` on each diagnostic. (No browser: `check` is CLI/JSON,
  no UI surface.)
- `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes` regenerates `docs/DIAGNOSTICS.md`
  cleanly; a re-run without BLESS passes.
