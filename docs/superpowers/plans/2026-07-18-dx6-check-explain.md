# DX6 plan — `check --explain <CODE>` + per-diagnostic `docs_url` (TDD)

Spec: `docs/superpowers/specs/2026-07-18-dx6-check-explain.md`. Branch `dx6-check-explain`.
Each step writes the failing test first, then the code, then greens it.

## Step 1 — core catalog (`crates/core/src/diagnostics/codes.rs`)

**Tests first** (in the existing `#[cfg(test)] mod tests`):

1. `every_code_has_an_explanation` — for each distinct code in `TABLE` ∪ `{GENERIC}`,
   `explain(code).is_some()`, and its `title/cause/fix` are non-empty.
2. `no_orphan_explanations` — every `EXPLANATIONS[i].code` is in `all_codes()`; codes are
   unique (no dupes).
3. `explain_is_case_insensitive` — `explain("tal-fm-key")` == `explain("TAL-FM-KEY")`;
   `explain("nope").is_none()`.
4. `all_codes_is_sorted_deduped_and_contains_generic` — sorted, unique, includes
   `TAL-CHECK`, len == distinct TABLE codes + 1.
5. `docs_url_is_the_computed_anchor` — `docs_url("TAL-XREF-UNREF")` ==
   `"{DIAGNOSTICS_DOC_URL}#tal-xref-unref"`.

**Code:**
- `pub struct Explanation { pub code: &'static str, pub title, pub cause, pub fix }`.
- `const EXPLANATIONS: &[Explanation]` — 24 entries (list in spec §Current state); prose
  grounded in the real validators (wording confirmed by grep: e.g. `TAL-XREF-UNREF` =
  `include: false` drops a labeled cell's output / a theorem id missing its `thm-` prefix;
  `TAL-A11Y-ALT` = missing OR placeholder alt; `TAL-REACTIVE` = unknown `{js}` input or a
  dependency cycle). No em/en dashes in the strings.
- `pub fn explain(code) -> Option<&'static Explanation>` (ASCII-uppercase the input, linear
  scan).
- `pub fn all_codes() -> Vec<&'static str>` — collect `TABLE` codes + `GENERIC`, sort, dedup.
- `pub const DIAGNOSTICS_DOC_URL` + `pub fn docs_url(code) -> String`.

## Step 2 — generated catalog + bless (`codes.rs` + `docs/DIAGNOSTICS.md`)

**Test first:** `diagnostics_md_matches_committed` — read
`{CARGO_MANIFEST_DIR}/../../docs/DIAGNOSTICS.md`, assert it equals `diagnostics_markdown()`,
or rewrite under `TALIESIN_BLESS=1` (a local `bless_or_assert` mirroring `schema.rs`). Fails
first because the file does not exist.

**Code:** `pub fn diagnostics_markdown() -> String` — a stable header + `## {code}\n\n**{title}**\n\n{cause}\n\nTo fix: {fix}\n` per code in `all_codes()` order, trailing newline.
Then `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes` to write `docs/DIAGNOSTICS.md`;
re-run without BLESS to green.

## Step 3 — `docs_url` on every diagnostic (`crates/server/src/check.rs`)

**Test first:** extend `format_json_emits_diagnostics_and_environment_object` (or a new
`diagnostics_carry_a_docs_url`) — `parsed["diagnostics"][0]["docs_url"]` is a string
starting with `DIAGNOSTICS_DOC_URL` and ending in the lowercased code anchor. Also assert
`format_human` output is unchanged (no `http`/`docs_url` leak).

**Code:** add `docs_url: String` to `Diagnostic`; set it in `Diagnostic::new` via
`codes::docs_url(code)`. `#[serde(...)]` order keeps `--format human` untouched (it ignores
the struct's serde entirely). Confirm `check_human_output_is_unchanged_by_codes` still holds.

## Step 4 — `--explain` in `cmd_check` (`check.rs`)

**Tests first** (unit, in `check.rs mod tests`, calling a new pure
`fn explain_output(code: Option<&str>, format: &str) -> Result<String, String>` so the
render is testable without spawning a process):

1. `explain_known_code_human` — `explain_output(Some("TAL-XREF-UNREF"), "human")` contains
   the title, "To fix:", and the `docs_url`.
2. `explain_known_code_json` — parses to `{code,title,cause,fix,docs_url}`, code echoed
   uppercase.
3. `explain_is_case_insensitive_via_output` — `Some("tal-fm-key")` resolves.
4. `explain_unknown_code_is_err_with_hint` — `Err` mentions the bad code and (for a
   near-miss like `TAL-XREF-UNDEFF`) a did-you-mean.
5. `explain_no_code_lists_all_codes` — `None` human lists every code; json is an array of
   `{code,title,docs_url}` of len `all_codes().len()`.

**Code:**
- `CHECK_FLAGS = &["--format", "--explain"]`.
- In `cmd_check`'s parse loop, add a `--explain` arm: set `explain = true`; peek `it.clone().next()`,
  and if the next token does not start with `-`, consume it as `explain_code`.
- After the loop, before the "path required" check: `if explain { … print explain_output;
  return SUCCESS/FAILURE per Ok/Err … }`. Route `Err` through the same json/human split as the
  render-error path (`json_error` for json, `log::error` for human).
- `explain_output` builds the human block / `serde_json::json!` object / index list.

## Step 5 — help + completion (`main.rs`, `complete.rs`)

**Tests first:**
- `complete.rs`: extend the check cases — `values(&["check", "--explain", ""])` returns
  `all_codes()`; and the existing `flag_table_covers_help` / `flag_of_check` guards still
  pass with `--explain` added.
**Code:**
- `main.rs`: add `--explain <CODE>` to the `check` line in `usage()` and a `--explain` flag
  entry + example to `subcommand_help("check")`.
- `complete.rs`: `flags_for("check") += ("--explain", true, …)`; §3 add
  `if prev == "--explain" { return enumerated(cur, &all_codes) }` (materialize
  `all_codes()` into a `Vec<&str>` for `enumerated`).

## Step 6 — end-to-end (`crates/server/tests/check_cli.rs`)

**Tests:** `explain_human_prints_cause_and_fix`, `explain_json_is_structured`,
`explain_unknown_code_exits_nonzero`, `explain_no_arg_lists_codes`, and
`diagnostics_json_carries_docs_url` (real binary on `corpus/diagnostics/typos.tmd`).

## Step 7 — verify + close

`cargo test -p taliesin-core`, `cargo test -p taliesin-server`, `cargo fmt --check`,
`cargo clippy --workspace`. Live: the four `--explain` shapes + `docs_url` on a real check.
Then ff-merge to `main`, delete DX6 from `notes/backlog.md`, add an `AUDITS.md` closure note,
update the `dx-audit` memory. Push only when the author asks.
