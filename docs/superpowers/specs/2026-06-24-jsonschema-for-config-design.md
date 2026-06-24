# jsonschema-for-config: design

> Beyond Quarto, Wave 1 (Pillar I), the gate after `nested-schema-validation`. Successor
> context in `BEYOND-QUARTO.md`. Approved 2026-06-24.

## Goal

Give the author's *editor* autocomplete, hover, and validation for qmd-fast's two YAML
config surfaces, with **zero qmd-fast LSP to build**, by shipping JSON Schemas the YAML
Language Server can consume. The schemas are generated from the *same* closed-set consts
the render-time validator already uses, so they can never drift from the validator.

This is the in-scope "single editing surface" on-ramp: the editor gets smarter about the
source, the preview stays a read-only view, and qmd-fast ships a static artifact rather
than a language server.

## Decisions (locked with the author 2026-06-24)

- **Both surfaces** get a schema: per-document YAML front-matter AND `_site.yml`.
- **`serde_json` is a dev-dependency**, not a runtime dependency. The generator and the
  golden-file test use it; the shipped binary does not. The committed schema files are
  bundled as static strings, and everything the runtime emits is those static strings.

## Architecture

Five pieces, each with one responsibility.

### 1. Schema generator (`crates/core/src/schema.rs`, test-only)

Pure functions that build `serde_json::Value` Draft-2020-12 schemas from the consts:

- `front_matter_schema() -> serde_json::Value`: an object schema whose `properties` are
  exactly `KNOWN_KEYS`, with nested object sub-schemas for `execute` / `listing` / `about`
  / `hero` built from `EXECUTE_KEYS` / `LISTING_KEYS` / `ABOUT_KEYS` / `HERO_KEYS`.
  `listing` accepts an object or an array of objects (matching `parse_listings`). `format`
  is left permissive (an extension owns its sub-keys), so it is described as "object or
  string" with no `additionalProperties` restriction.
- `site_config_schema() -> serde_json::Value`: an object schema whose `properties` are
  exactly `NATIVE_KEYS`.
- `to_pretty_json(value) -> String`: deterministic, stable pretty-print (serde_json's
  default `Map` is `BTreeMap`, so key order is stable without the `preserve_order`
  feature).

Schema policy, chosen to mirror the validator and avoid false positives:

- **`additionalProperties: false`** at every *closed* level (top-level front-matter, the
  four nested blocks, and `_site.yml`). This is what makes the editor flag an unknown key,
  exactly as the validator does at render time.
- **Value types stay loose.** A `type` is asserted only where the parser is unambiguously
  type-specific: `toc: boolean`, `max-items: integer` (`listing`), `categories: boolean`
  (`listing`). Flexible-value keys (`author`, `categories` top-level, `theme`, `css`,
  `image`, `chapters`, `nav`, `footer`, `mounts`, every string field) carry no `type`
  constraint, so the schema never fights a valid-but-flexible value. The schema's job is
  to catch unknown *keys*, not to re-implement the parser's value coercion.

The private consts (`KNOWN_KEYS`, `EXECUTE_KEYS`, `LISTING_KEYS`, `ABOUT_KEYS`,
`HERO_KEYS` in `frontmatter.rs`; `NATIVE_KEYS` in `site/config/mod.rs`) become `pub(crate)`
so `schema.rs` can read them. `render::validate::CELL_OPTION_KEYS` / `CALLOUT_KINDS` are
NOT used here (cell options and callouts are body constructs, not YAML config).

### 2. Committed + bundled schema files

`crates/core/assets/schema/qmd-frontmatter.schema.json` and `qmd-site.schema.json` are the
committed source of truth, exposed as `pub const FRONTMATTER_SCHEMA: &str` /
`SITE_SCHEMA: &str` via `include_str!`. The `$schema` modeline references these; the
runtime emits these. They are regenerated only via the bless path below, never hand-edited.

### 3. Golden-file drift lock (`crates/core/tests/schema.rs`)

`assert_eq!(to_pretty_json(front_matter_schema()), FRONTMATTER_SCHEMA)` and the same for
the site schema. Editing any closed-set const without regenerating fails the test. A
`QMD_FAST_BLESS=1` branch in the test rewrites the committed files instead of asserting,
so regeneration is `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --test schema`. This is
the single mechanism that keeps the schema and the validator in lockstep.

### 4. `qmd-fast schema [--out <dir>]` CLI subcommand (server crate)

Emits the two **bundled static strings** (no runtime `serde_json`): with no `--out`, prints
both to stdout with a header line each; with `--out <dir>`, writes
`<dir>/qmd-frontmatter.schema.json` and `<dir>/qmd-site.schema.json`. This lets an author
who has only the installed binary drop the schemas into their project (e.g. `.qmd/`) so
their editor can reference them by relative path. Dispatched from `main.rs`'s `match
args.get(1)` alongside `render` / `build` / `blocks` / `serve`, mirroring `build`'s `--out`
parsing.

### 5. Docs

A short "Editor autocomplete (config schema)" section in `docs/guide/reference/
configuration.qmd` (front-matter) and `docs/internals/sites.qmd` (or the guide's site
reference, `_site.yml`), showing:
- the `# yaml-language-server: $schema=./.qmd/qmd-site.schema.json` modeline for `_site.yml`
  (works out of the box once the file is emitted next to the config),
- the same for front-matter, with the caveat that the editor must treat `.qmd` front-matter
  as embedded YAML (e.g. the VS Code YAML extension with a front-matter mapping), since a
  vanilla YAML LSP does not parse a `.qmd` file as YAML.

## Out of scope (YAGNI / follow-ups)

- No schema for cell options / callout kinds: those are not YAML config; they stay
  render-time-only diagnostics.
- No live dev-server route serving the schema over HTTP, and no hosted public `$schema`
  URL. The CLI-emitted local file plus a relative-path modeline covers the on-ramp. Both
  are cheap follow-ups if a workflow wants them.
- No runtime JSON generation: the runtime only emits the committed static strings.

## Invariant safety

Purely additive. No change to the render pipeline, the block model, the diff, or the
validator's behavior. The schema only *describes* what the validator already enforces.
`serde_json` enters as a dev-dependency only.

## Testing

- The golden-file test (piece 3) is the core gate: it proves the committed schemas equal
  the generator output, which is derived from the live consts.
- A small unit test asserts each schema is structurally sane: it parses as JSON, declares
  the Draft-2020-12 `$schema`, and lists every `KNOWN_KEYS` / `NATIVE_KEYS` entry under
  `properties` with `additionalProperties: false` (so a future key addition that forgets
  the schema is caught).
- A CLI test (or a smoke step) runs `qmd-fast schema --out <tmp>` and asserts both files
  are written and parse as JSON.
