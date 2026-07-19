# PL14 — `check`'s Environment footer: probe only when needed, show only when degraded

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
