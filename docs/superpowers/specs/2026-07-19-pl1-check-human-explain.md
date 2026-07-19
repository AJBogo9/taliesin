# PL1 — Surface the code + severity + `--explain` in `check`'s human output

## Problem

`check` already computes a stable `TAL-*` code and a `severity` per diagnostic
(`Diagnostic` in `crates/server/src/check.rs`), ships a rustc-grade `--explain <CODE>`
catalog, and a drift-locked `docs/DIAGNOSTICS.md`. But the **human formatter throws it all
away**: `format_human` prints only `file:line: message`, and the summary is a bare
`N problem(s)`. So the whole DX6 investment (codes, severities, `--explain`) is invisible in
the output the ~99% of interactive/CI runs read. A user staring at a warning has no idea a
code exists, let alone that `--explain` would expand it.

`--format json` already carries `code`/`severity`/`docs_url`; only the human path is poorer.

## Fix (output-only; `--format json` byte-identical)

Three changes, all in the human branch of `check.rs`:

1. **Per-line: show severity + code.** Keep the greppable `file:line:` linter prefix
   (VS Code problem-matchers, `grep file:line`, gcc/clang/tsc all key off `file:line:` at the
   start of the line), and insert `severity[CODE]:` before the message — the gcc/clang shape
   `file:line: warning[CODE]: message`:
   - located:   `{file}:{line}: {severity}[{code}]: {message}`
   - unlocated: `{file}: {severity}[{code}]: {message}`

   *Deviation from the audit's literal "prefix each line `severity[CODE]`":* placing the
   severity/code after `file:line:` rather than at the absolute start preserves the linter
   convention this tool's own comment advertises ("Greppable `path:line: message` lines").
   The intent — surface code + severity — is fully met.

2. **Summary: split by severity.** `N problem(s)` → `N problem(s) (E error(s), W warning(s))`,
   listing only the non-zero categories. Keeps the leading `N problem(s)` token so existing
   greps/tests still match. Examples: `2 problems (1 error, 1 warning)`, `1 problem (1 warning)`.

3. **Footer: teach `--explain`.** When there is ≥1 diagnostic, print rustc's
   "For more information…" line pointing at the command:
   `For more information about a diagnostic, try `taliesin check --explain <CODE>`.`
   Each line above already shows a concrete `[CODE]` to substitute.

`format_json`, the JSON error envelope, the exit code, the Environment footer, and
`build`/`publish`'s own tally (a separate `build.rs` path) are untouched.

## Tests (TDD)

- **Update** `diagnostics_carry_a_docs_url_in_json_but_not_human`: human output must still
  never leak the `docs_url` (no `http`), but it now **does** carry the `TAL-` code and the
  severity word. Flip the `!contains("TAL-")` assertion to `contains`.
- **Update** `format_human_lists_located_lines`: assert the new `file:line: severity[CODE]:
  message` shape for both a located and an unlocated diagnostic.
- **Add** `human_summary_splits_by_severity_and_points_at_explain`: a run/summary-level test
  (or a small pure helper) asserting the split summary `(1 error, 1 warning)` and the
  `--explain` footer appear for a mixed diagnostic set, and that neither appears (footer)
  when there are no diagnostics.

Mutation-check each: revert the format change → the named test fails.
