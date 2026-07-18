# DX3 — Auto-wire the config JSON Schema on `init`

Date: 2026-07-18. Backlog item **DX3** (§6 DX audit batch, Tier 1 discoverability family).
Branch `dx3-init-schema-autowire`. Detail source: `notes/2026-07-18-dx-audit.md`.

> **Autonomy note:** the author is away and asked me to proceed without the interactive
> design-review gate. DX3 is a well-scoped `[surface]` item with a clear audit description; the
> only real forks (where the schema files live, which schemas to emit) are resolved below with
> documented defaults, per the "act on the sensible default and note the assumption" directive.

## Goal

Make `taliesin init` produce a project whose `_site.yml` **already** autocompletes and
red-squiggles against the config JSON Schema in any editor with a YAML language server — zero
manual step. Today the schema exists and `taliesin schema` can emit it, but the user must (a)
know the command exists, (b) run it, and (c) hand-add the `# yaml-language-server: $schema=…`
modeline. The audit calls this "the config-authoring equivalent of the shell completion they just
shipped."

## Ground truth (grepped + read against source 2026-07-18, before pricing)

- **The schemas are bundled constants**, drift-locked to the validator:
  `taliesin_core::schema::SITE_SCHEMA` + `FRONTMATTER_SCHEMA`
  ([`crates/core/src/schema.rs:13,16`](../../../crates/core/src/schema.rs)), sourced from
  `crates/core/assets/schema/tali-{site,frontmatter}.schema.json`.
- **`taliesin schema [--out <dir>]`** already writes both files and even prints the exact
  modeline to add ([`query.rs:192-232`](../../../crates/server/src/query.rs), the hint at L221-223:
  `add \`# yaml-language-server: $schema={dir}/tali-site.schema.json\` atop _site.yml`). DX3 is
  "do that last mile automatically at `init`."
- **`init` scaffolds three flat files** — `_site.yml` (`INIT_SITE_YML = "title: My site\n"`),
  `index.tmd`, `AGENTS.md` — via `scaffold_init`
  ([`cli.rs:20,89-120`](../../../crates/server/src/cli.rs)), with an **all-or-nothing overwrite
  guard** (refuses if *any* target exists) and returns the written paths (surfaced in the console
  list and the `--json {created}` receipt).
- **`init` is the only producer of `_site.yml`.** `new` (post/page/deck/paper) writes single docs,
  no site config ([`cli.rs:122-130,325`](../../../crates/server/src/cli.rs)). So DX3 touches `init`
  only. (DX10 covers teaching the `new` scaffolds.)
- **All three site walkers skip `.`/`_`-prefixed dirs:** page discovery
  ([`discovery.rs:117`](../../../crates/core/src/site/discovery.rs)), `mirror_assets`
  ([`build.rs:~1757`](../../../crates/server/src/build.rs) — `name.starts_with('_') ||
  name.starts_with('.')`), and referenced-source deploy (copies only files a page links to). So a
  `.taliesin/` schema dir becomes **neither a phantom page nor shipped output**.
- **`.taliesin/` is an unused namespace** in the repo today.

## Resolved decisions (autonomous, documented)

1. **Schema files → `<dir>/.taliesin/tali-site.schema.json` + `.taliesin/tali-frontmatter.schema.json`.**
   A tool-namespaced dot-dir: skipped by every site walker (no phantom page, never in `_site/`),
   keeps the project root uncluttered, and bundles both schemas together mirroring
   `taliesin schema`. Both are emitted (the site one is modeline-wired; the front-matter one is
   present for the editor/companion to apply to `.tmd` front matter).
2. **`_site.yml` starts with the modeline:**
   `# yaml-language-server: $schema=.taliesin/tali-site.schema.json`. It is a YAML comment (the
   config parser ignores it) and a relative path (resolves from `_site.yml`'s own directory). It
   references the **site** schema because that is the schema for `_site.yml`. The front-matter
   schema is **not** modeline-wired into `.tmd` files: a `.tmd` is not a YAML document a
   yaml-language-server processes; that wiring belongs to the VS Code companion, not a modeline.
3. **Written set + overwrite guard include the schema files.** Re-running `init` still refuses to
   clobber; the console list and `--json {created}` enumerate the schema paths too.
4. **DRY:** reuse the `SITE_SCHEMA`/`FRONTMATTER_SCHEMA` constants (the same source
   `taliesin schema` uses), so the scaffolded schemas cannot drift from the validator.

## Changes

### `crates/server/src/cli.rs`

- **`INIT_SITE_YML`** becomes:
  ```
  # yaml-language-server: $schema=.taliesin/tali-site.schema.json
  title: My site
  ```
- **`scaffold_init`**: extend the `files` list with the two schema entries
  (`.taliesin/tali-site.schema.json` → `SITE_SCHEMA`, `.taliesin/tali-frontmatter.schema.json` →
  `FRONTMATTER_SCHEMA`), and in the write loop create each target's parent dir before writing
  (`if let Some(parent) = path.parent() { create_dir_all(parent) }`, mirroring the pattern in
  `build.rs` `copy_local_assets`). The overwrite guard already iterates the same list, so it now
  also protects the schema files.

No other files change. `taliesin schema` is untouched (DX3 reuses its constants, not its command).

## Testability (TDD)

Extend the existing `init_scaffolds_a_previewable_site` unit test (`cli.rs`):

- **The load-bearing pin (no drift):** read the scaffolded `_site.yml`, extract the `$schema=`
  path from its first line, join it to the scaffold dir, and assert (a) that file **exists** and
  (b) its contents **== `SITE_SCHEMA`**. This single assertion proves the modeline points at a
  real, correct schema — the two can never silently drift.
- `.taliesin/tali-frontmatter.schema.json` exists and == `FRONTMATTER_SCHEMA`.
- `_site.yml` first line **is** the modeline, and it still parses to a mapping with `title` (the
  comment doesn't break config parsing).
- `written` includes both schema paths.
- Re-running still returns the "already exists" refusal (guard covers the new files).

Mutation-check: break the modeline path (point it at a non-existent file) → the "resolves to an
existing file whose body == SITE_SCHEMA" assertion fails → revert.

## Verification

- `cargo test -p taliesin-server` (the extended init test) + `cargo test -p taliesin-core`.
- `cargo fmt --check`, `cargo clippy -p taliesin-server --all-targets -- -D warnings`.
- **Integration smoke:** `init` into a temp dir; assert on disk `_site.yml` first line + the two
  `.taliesin/*.schema.json`; `serve` the temp dir and confirm it previews with **no new `_site.yml`
  warning** (the modeline comment is inert); `build` the temp dir and confirm `_site/` contains
  **no** `.taliesin/` (dotdir-skip holds).
- Confirm `taliesin schema` output is byte-identical to the scaffolded files (same constants).

## Non-goals

- **No front-matter modeline** injected into `.tmd` files (not a YAML doc; companion's job).
- **No schema refresh/upgrade** command (`init`'s schemas are a snapshot, exactly like
  `taliesin schema --out`; a future `schema --refresh` is out of scope).
- **No `new`/paper/post change** (DX10).
- **No `.gitignore`** written (the schemas are small static files; committing them is harmless and
  lets collaborators get validation too — the user decides).

## Invariant safety

No output-format change, no CDN, no preview write-back. HTML-only, block model, and the
`MAX_WARM_PAGES`/`exec_pool.rs` freeze are all untouched. The only new artifact is a walker-skipped
dot-dir of static JSON emitted at scaffold time.
