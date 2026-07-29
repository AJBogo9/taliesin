# PL14 — `check`'s Environment footer: probe only when needed, show only when degraded

> **PARTLY SUPERSEDED 2026-07-29 by backlog item 122 (owner ruling).** The "does not probe"
> half below is intact and is now enforced by `ProbePolicy::Never`. The "**no** Environment
> footer" half is reversed: a default human `check` on a target with `{python}`/`{r}` cells
> now prints `Environment (not probed):` naming the resolved interpreter, because the silence
> made `check` answer "no problems found", exit 0, on a document whose only cell could not
> run. This spec conflated two decisions — whether to SPAWN and whether to SPEAK — and the
> ruling separates them: name it always, spawn it only on request. The pinning test
> `default_human_check_omits_the_environment_block` is replaced by
> `default_human_check_names_the_interpreter_without_spawning_it`.
>
> The cost objection recorded here and in item 122 also **did not survive measurement**:
> `collect_environment` no longer walks anything, because `CheckScope` carries the language
> list off the render the diagnostics pass already did. Default `check` measured identical
> to the pre-change binary on all four projects in the tree (`docs/guide` 0.35 s,
> `corpus/tech-blog` 0.52 s, `docs/internals` 0.21 s, `site` 0.12 s).

`collect_environment` ran unconditionally and `check` printed an Environment footer whenever
non-empty — even all-green — on a command documented "does NOT execute code cells". So every
human/CI `check` of a doc with a `{python}`/`{r}` cell **spawned python3/R**, and duplicated
`taliesin doctor`'s job.

## Fix

The interpreter probe (which spawns) now runs only when the output or a gate needs it:

- **`--format json`** — always probes; the `environment` array stays always-on (agents/the MCP
  `check` tool want the full probe; `check_json` is unchanged).
- **`--require-kernel`** — probes, because it gates on kernel readiness. The human output then
  lists **only the degraded** languages (`!runs || !kernel_pkg_ok`) under
  `Environment (kernels not ready):` and tails with `run \`taliesin doctor\``. An all-green probe
  prints nothing (it's `doctor`'s business, not a linter's).
- **Default human `check`** — does **not** probe at all. No spawn on every keystroke/CI run, no
  Environment footer. `check` stays a pure static linter; the env audit is `doctor`'s.

The `--require-kernel` gate message and exit behaviour are unchanged.

## Tests

- `check_cli.rs::default_human_check_omits_the_environment_block` — forces a BROKEN interpreter,
  so it's deterministic AND pins the probe-skip (if the default path still probed, a broken
  interpreter would print a degraded block). Mutation-checked: reverting the skip fails it.
- `require_kernel_gates_a_missing_interpreter` extended: the degraded block +
  `taliesin doctor` pointer appear under `--require-kernel`.
- `json_check_still_carries_the_environment_probe` — `--format json` still yields the
  `environment` array. `mcp_stdio` (unchanged) keeps the MCP tool's `environment`.
- CLI reference prose updated. Verified end-to-end.
