# Publish hardening — design (`--public` gate opt-out + strict-by-default)

Date: 2026-07-12
Backlog: Section B items #15 + #16 ([notes/backlog.md](../../../notes/backlog.md)).
Branch: `publish-hardening`.

## Problem

`taliesin publish <dir>` (Cloudflare Pages via Wrangler) has two defaults that fight a
robust workflow:

1. **The passcode gate is unconditional.** Every deploy injects
   `functions/_middleware.js` (HTTP Basic-Auth behind the `PASSWORD` secret). There is no
   supported way to deploy a genuinely public site, so a public blog has to go out through
   a side-channel `deploy` skill instead of the real command. (#15, ruled: *add an opt-in*.)
2. **`publish` is lax by default.** It only runs the full strict check when passed
   `--strict`; a plain `publish` will happily deploy a page with a crashed cell, a broken
   cross-ref, or a missing image, at exit 0. Deploying a broken page *publicly* is the
   worst failure mode. (#16, ruled: *strict + `--no-strict`*.)

Both are one-command-surface changes; they live almost entirely in
[crates/server/src/publish.rs](../../../crates/server/src/publish.rs), with one config
field added in [crates/core/src/site/config/mod.rs](../../../crates/core/src/site/config/mod.rs).

## Scope

In scope: the `publish` CLI flags + precedence, the `publish.gate` config field + its
typo-lint, the gate-injection decision, the strict default. Out of scope: any change to the
gate mechanism itself (`_middleware.js`), the Wrangler invocation, or the build pipeline.
Retiring the side-channel `deploy` skill is a follow-up noted at the end, not part of this
branch's code.

## Design

### #15 — `--public` / `publish.gate: false`

**Config.** Add `gate: Option<bool>` to `PublishConfig`:

```rust
pub struct PublishConfig {
    pub provider: Option<String>,
    pub project: Option<String>,
    pub gate: Option<bool>, // absent => gated (the safe default)
}
```

- Parse it in `publish_from` (a bool value; a non-bool leaves it `None` → default gated).
- Add `"gate"` to `PUBLISH_KEYS` so `validate_publish`'s did-you-mean covers a typo
  (`gat:`/`gated:` → suggest `gate`). A dropped gate setting must never fail *open*, so the
  parse defaults to gated on any ambiguity.

**CLI.** Add a `--public` flag to `PublishArgs` (`public: bool`) + `PUBLISH_FLAGS`.

**Precedence** (most specific wins), resolved to a single `gate: bool` in `cmd_publish`:

1. `--public` on the command line → `gate = false`.
2. else `publish.gate:` in `_site.yml` if present → `gate = that`.
3. else default → `gate = true` (unchanged behavior).

There is deliberately **no** `--private`/`--gate` flag: gated is already the default, so a
force-gate flag would be dead weight (minimal-config lens). If a config sets `gate: false`
and you want a one-off gated deploy, that is a config edit, not a flag — acceptably rare.

**Behavior.** `inject_gate` is called only when `gate == true`. When `gate == false`, skip
it and log a loud, unmissable line so a public deploy is never silent:

```
log::warn("publishing WITHOUT a passcode gate — this site will be PUBLIC");
```

(`--dry-run` prints the same warning + still skips injection, so a dry run faithfully
previews the public/gated decision.)

### #16 — strict by default + `--no-strict`

- Flip the default: `PublishArgs.strict` starts `true`.
- Add `--no-strict` → sets it `false`.
- Keep accepting `--strict` as a **redundant no-op** (sets `true`, already the default) so
  existing invocations/docs/muscle-memory don't error; it self-documents intent. Both flags
  in `PUBLISH_FLAGS`; `--no-strict` also in the did-you-mean set.
- The usage string becomes:
  `taliesin publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run]`
- `strict` threads unchanged into `run_site_build(root, out, strict, None)` — no build-side
  change; `publish` just defaults it to `true`.

If `--strict` and `--no-strict` are both passed, last-one-wins (natural from the argv loop);
document it in the flag help rather than erroring.

## Testing (the regression net)

Publish is a deploy-only command; the pin is unit + integration tests, not a corpus doc.

**Unit (`parse_publish_args`, already has a test module):**
- `--public` sets `public = true`; absent → `false`.
- default `strict == true`; `--no-strict` → `false`; `--strict` → `true`; `--no-strict
  --strict` → `true` (last wins) and vice-versa.
- `--public`/`--no-strict` are known flags (no unknown-flag error); a near-miss
  (`--publik`, `--no-strict`) still produces a did-you-mean.

**Config (`config/mod.rs` tests):**
- `publish:\n  gate: false` parses to `gate: Some(false)`; absent → `None`.
- `publish:\n  gat: false` warns with a `gate` did-you-mean.

**Integration ([tests/publish.rs](../../../crates/server/tests/publish.rs), via `--dry-run`
so nothing deploys):**
- default dry-run on a tiny site writes `<out>/functions/_middleware.js`.
- `--public` dry-run does **not** write `_middleware.js`; stderr carries the PUBLIC warning.
- `publish.gate: false` in `_site.yml` (no flag) also skips the gate.
- strict-by-default: a site with a broken cross-ref exits non-zero on a plain dry-run;
  `--no-strict` on the same site exits zero.

## Gate-the-gate check

The "gate not written under `--public`" test must be shown to fail if injection is left
unconditional (mutation-check it against the exact behavior it guards) before it's trusted —
per the standing *gate the gate* rule.

## Follow-up (not this branch)

Once `publish --public` lands, the side-channel `deploy` skill is redundant and should be
retired (delete the skill file, update any doc that points at it). Tracked in the backlog.
