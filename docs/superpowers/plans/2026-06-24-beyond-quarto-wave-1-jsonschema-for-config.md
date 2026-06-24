# Beyond Quarto Wave 1 (Gate): jsonschema-for-config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate Draft-2020-12 JSON Schemas for qmd-fast's two YAML config surfaces (document front matter + `_site.yml`) from the SAME closed-set consts the validator uses, ship them as committed + bundled files, expose a `qmd-fast schema` command that emits them, and document the editor `# yaml-language-server: $schema=` on-ramp, so an editor's YAML language server validates config with zero qmd-fast LSP to build.

**Architecture:** A test-only generator (`crates/core/src/schema.rs`, `#[cfg(test)]`) builds `serde_json::Value` schemas from `frontmatter::KNOWN_KEYS` + the nested sets and `site::config::NATIVE_KEYS`, mirroring the validator: `additionalProperties: false` at every closed level, loose value types except where unambiguous. The committed `assets/schema/*.schema.json` files are bundled as `pub const` static strings; a golden-file test (with a `QMD_FAST_BLESS=1` regenerate path) keeps them equal to the generator output, so they cannot drift. `serde_json` is a dev-dependency only; the runtime (`qmd-fast schema`) emits the bundled strings.

**Tech Stack:** Rust edition 2024 / resolver 3; `serde_json` as a NEW dev-dependency of `qmd-fast-core` (test/generate only); integration tests under `crates/*/tests/` and in-file `#[cfg(test)]`.

## Global Constraints

- Rust edition 2024, resolver 3.
- `serde_json` is added ONLY as a `[dev-dependencies]` entry of `qmd-fast-core`. The shipped binary must gain NO new runtime dependency: the generator is `#[cfg(test)]`, and `qmd-fast schema` emits the bundled static strings, never generating JSON at runtime. (`cargo tree -p qmd-fast-server | grep serde_json` must stay empty.)
- No em dashes or en dashes in any authored prose, comment, doc, or commit message. Use commas, colons, parentheses, or restructured sentences.
- CI enforces `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Each task ends green on all three.
- INVARIANT SAFETY: purely additive. No change to the render pipeline, the block model (`data-block-id` / `data-sourcepos` / `data-source-file`), the diff, the `:::` machine, cite/includes/numbering/exec/freeze, or the validator's behavior. The schema only DESCRIBES what the validator already enforces.
- The committed `assets/schema/*.schema.json` files are GENERATED, never hand-edited. Regenerate with `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`.
- Schemas are Draft 2020-12 (`"$schema": "https://json-schema.org/draft/2020-12/schema"`). Mirror the validator: `additionalProperties: false` at every closed level; assert a `type` only where the parser is unambiguously type-specific (`toc`/`echo`/`include`/`cache`/`categories` boolean, `max-items` integer); leave all other values unconstrained; leave `format:` fully permissive (an extension owns its sub-keys).
- The schemas cover only YAML config. Cell options and callout kinds are body constructs (not YAML), so they get NO schema and stay render-time-only diagnostics.

## File Structure

- `crates/core/src/schema.rs` (NEW): `pub const FRONTMATTER_SCHEMA` / `SITE_SCHEMA` (bundled via `include_str!`), a `#[cfg(test)] mod generate` (the `serde_json` generator), and `#[cfg(test)] mod tests` (golden-file + structural). One responsibility: defining + drift-locking the config schemas.
- `crates/core/assets/schema/qmd-frontmatter.schema.json`, `qmd-site.schema.json` (NEW, generated): the committed source of truth, bundled.
- `crates/core/src/lib.rs` (MODIFY): `pub mod schema;`.
- `crates/core/src/frontmatter.rs` (MODIFY): make `KNOWN_KEYS` + the four nested sets `pub(crate)`.
- `crates/core/src/site/config/mod.rs` (MODIFY): make `NATIVE_KEYS` `pub(crate)`. `crates/core/src/site/mod.rs` (MODIFY): re-export it (`pub(crate) use config::NATIVE_KEYS;`) so `schema.rs` can read it.
- `crates/core/Cargo.toml` (MODIFY): add `serde_json` under `[dev-dependencies]`.
- `crates/server/src/main.rs` (MODIFY): add the `schema` subcommand + `cmd_schema`.
- `crates/server/tests/schema_cli.rs` (NEW): the CLI integration test.
- `docs/guide/reference/configuration.qmd`, `docs/internals/sites.qmd` (MODIFY): the editor-autocomplete docs.

---

### Task 1: Generator, bundled schemas, and the drift-lock test (core)

**Files:**
- Modify: `crates/core/Cargo.toml` (dev-dependency)
- Modify: `crates/core/src/frontmatter.rs` (consts `pub(crate)`)
- Modify: `crates/core/src/site/config/mod.rs` + `crates/core/src/site/mod.rs` (`NATIVE_KEYS` `pub(crate)` + re-export)
- Modify: `crates/core/src/lib.rs` (`pub mod schema;`)
- Create: `crates/core/src/schema.rs`
- Create: `crates/core/assets/schema/qmd-frontmatter.schema.json`, `crates/core/assets/schema/qmd-site.schema.json` (generated via bless)

**Interfaces:**
- Produces: `qmd_fast_core::schema::FRONTMATTER_SCHEMA: &str`, `qmd_fast_core::schema::SITE_SCHEMA: &str` (the bundled committed schema strings; consumed by Task 2).
- Consumes: `crate::frontmatter::{KNOWN_KEYS, EXECUTE_KEYS, LISTING_KEYS, ABOUT_KEYS, HERO_KEYS}`, `crate::site::NATIVE_KEYS` (made `pub(crate)` in this task).

- [ ] **Step 1: Add `serde_json` as a dev-dependency**

In `crates/core/Cargo.toml`, add a `[dev-dependencies]` section (or extend the existing one) with:

```toml
[dev-dependencies]
serde_json = "1"
```

If a `[dev-dependencies]` section already exists, add the `serde_json = "1"` line to it rather than duplicating the header.

- [ ] **Step 2: Make the closed-set consts `pub(crate)`**

In `crates/core/src/frontmatter.rs`, change five declarations from `const` to `pub(crate) const` (keep their values + comments):
- `const KNOWN_KEYS` to `pub(crate) const KNOWN_KEYS`
- `const EXECUTE_KEYS` to `pub(crate) const EXECUTE_KEYS`
- `const LISTING_KEYS` to `pub(crate) const LISTING_KEYS`
- `const ABOUT_KEYS` to `pub(crate) const ABOUT_KEYS`
- `const HERO_KEYS` to `pub(crate) const HERO_KEYS`

In `crates/core/src/site/config/mod.rs`, change `const NATIVE_KEYS` to `pub(crate) const NATIVE_KEYS`.

In `crates/core/src/site/mod.rs`, add a re-export so `schema.rs` can reach it without depending on `config`'s module visibility. Find where `config` is declared / used and add near the other `pub(crate) use` re-exports:

```rust
pub(crate) use config::NATIVE_KEYS;
```

(If `mod config;` is declared private and a `pub(crate) use config::NATIVE_KEYS;` cannot resolve, widen the declaration to `pub(crate) mod config;`. Verify with `cargo build -p qmd-fast-core` after Step 5.)

- [ ] **Step 3: Declare the module**

In `crates/core/src/lib.rs`, add `pub mod schema;` alongside the other `pub mod` declarations (next to `pub mod render;` around line 35).

- [ ] **Step 4: Create placeholder schema files (so `include_str!` compiles)**

Create `crates/core/assets/schema/qmd-frontmatter.schema.json` and `crates/core/assets/schema/qmd-site.schema.json`, each containing exactly:

```json
{}
```

(These are placeholders. Step 8 regenerates them with the real schema via the bless path; they only need to exist so the `include_str!` consts in Step 5 compile.)

- [ ] **Step 5: Write `schema.rs` (bundled consts + generator + tests)**

Create `crates/core/src/schema.rs`:

```rust
//! JSON Schema for qmd-fast's YAML config surfaces (document front matter + `_site.yml`).
//!
//! The committed `assets/schema/*.schema.json` files, bundled here as static strings, are
//! generated from the SAME closed-set consts the validator uses (`frontmatter::KNOWN_KEYS`
//! plus the nested `EXECUTE`/`LISTING`/`ABOUT`/`HERO` sets, and `site::NATIVE_KEYS`), so the
//! schema cannot drift from what the validator enforces. They are regenerated ONLY via the
//! bless path in this module's tests (`QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib
//! schema`), never hand-edited. The `qmd-fast schema` CLI emits these strings so an editor's
//! YAML language server can validate config: the in-scope single-editing-surface on-ramp,
//! with no qmd-fast language server to build.

/// The Draft-2020-12 JSON Schema for a document's YAML front matter.
pub const FRONTMATTER_SCHEMA: &str = include_str!("../assets/schema/qmd-frontmatter.schema.json");

/// The Draft-2020-12 JSON Schema for a project's `_site.yml`.
pub const SITE_SCHEMA: &str = include_str!("../assets/schema/qmd-site.schema.json");

#[cfg(test)]
mod generate {
    use crate::frontmatter::{ABOUT_KEYS, EXECUTE_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS};
    use crate::site::NATIVE_KEYS;
    use serde_json::{Map, Value, json};

    /// A `properties` object: every key in `keys` maps to `{}` (any value), then `overrides`
    /// replace specific keys with a typed or nested sub-schema. serde_json's default `Map` is
    /// a `BTreeMap`, so keys serialize alphabetically and the output is deterministic.
    fn properties(keys: &[&str], overrides: &[(&str, Value)]) -> Value {
        let mut map = Map::new();
        for k in keys {
            map.insert((*k).to_string(), json!({}));
        }
        for (k, v) in overrides {
            map.insert((*k).to_string(), v.clone());
        }
        Value::Object(map)
    }

    /// A closed object schema: `type: object`, `additionalProperties: false`, exactly `keys`.
    fn closed_object(keys: &[&str], overrides: &[(&str, Value)]) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": properties(keys, overrides),
        })
    }

    fn boolean() -> Value {
        json!({ "type": "boolean" })
    }
    fn integer() -> Value {
        json!({ "type": "integer" })
    }

    pub fn front_matter_schema() -> Value {
        // execute: every child is a boolean.
        let execute_overrides: Vec<(&str, Value)> =
            EXECUTE_KEYS.iter().map(|k| (*k, boolean())).collect();
        let execute = closed_object(EXECUTE_KEYS, &execute_overrides);
        let listing_item =
            closed_object(LISTING_KEYS, &[("max-items", integer()), ("categories", boolean())]);
        // listing: a single mapping or a sequence of mappings (cv.qmd shape).
        let listing = json!({
            "oneOf": [listing_item.clone(), { "type": "array", "items": listing_item }]
        });
        let about = closed_object(ABOUT_KEYS, &[]);
        let hero = closed_object(HERO_KEYS, &[]);
        let overrides = [
            ("toc", boolean()),
            ("execute", execute),
            ("listing", listing),
            ("about", about),
            ("hero", hero),
            // An extension owns `format:`'s sub-keys, so leave it fully permissive.
            ("format", json!({})),
        ];
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "qmd-fast document front matter",
            "type": "object",
            "additionalProperties": false,
            "properties": properties(KNOWN_KEYS, &overrides),
        })
    }

    pub fn site_config_schema() -> Value {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "qmd-fast _site.yml",
            "type": "object",
            "additionalProperties": false,
            "properties": properties(NATIVE_KEYS, &[("toc", boolean())]),
        })
    }

    /// Deterministic pretty JSON with a trailing newline (so committed files end cleanly).
    pub fn to_pretty_json(value: &Value) -> String {
        let mut s = serde_json::to_string_pretty(value).expect("schema serializes");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::generate::{front_matter_schema, site_config_schema, to_pretty_json};
    use super::{FRONTMATTER_SCHEMA, SITE_SCHEMA};
    use serde_json::Value;

    /// Assert the generated schema equals the committed file, OR (under `QMD_FAST_BLESS=1`)
    /// rewrite the committed file from the generator. `rel_path` is relative to the core
    /// crate root (`CARGO_MANIFEST_DIR`).
    fn bless_or_assert(generated: String, committed: &str, rel_path: &str) {
        if std::env::var("QMD_FAST_BLESS").is_ok() {
            let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel_path);
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed {rel_path}");
        } else {
            assert_eq!(
                generated, committed,
                "schema drift in {rel_path}; regenerate with `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema`"
            );
        }
    }

    #[test]
    fn frontmatter_schema_matches_committed() {
        bless_or_assert(
            to_pretty_json(&front_matter_schema()),
            FRONTMATTER_SCHEMA,
            "assets/schema/qmd-frontmatter.schema.json",
        );
    }

    #[test]
    fn site_schema_matches_committed() {
        bless_or_assert(
            to_pretty_json(&site_config_schema()),
            SITE_SCHEMA,
            "assets/schema/qmd-site.schema.json",
        );
    }

    #[test]
    fn schemas_are_structurally_sane() {
        for (name, v) in [
            ("frontmatter", front_matter_schema()),
            ("site", site_config_schema()),
        ] {
            assert_eq!(
                v["$schema"], "https://json-schema.org/draft/2020-12/schema",
                "{name}: draft id"
            );
            assert_eq!(v["type"], "object", "{name}: type");
            assert_eq!(v["additionalProperties"], Value::Bool(false), "{name}: closed");
            assert!(v["properties"].is_object(), "{name}: has properties");
        }
        // Every closed-set key appears as a property, so a future key the validator gains but
        // the schema forgets is caught here (not just by the golden file).
        let fm = front_matter_schema();
        for k in crate::frontmatter::KNOWN_KEYS {
            assert!(fm["properties"].get(k).is_some(), "frontmatter schema missing `{k}`");
        }
        let site = site_config_schema();
        for k in crate::site::NATIVE_KEYS {
            assert!(site["properties"].get(k).is_some(), "site schema missing `{k}`");
        }
        // The committed bundles parse as JSON (catches an empty or corrupt committed file).
        serde_json::from_str::<Value>(FRONTMATTER_SCHEMA).expect("frontmatter bundle is valid JSON");
        serde_json::from_str::<Value>(SITE_SCHEMA).expect("site bundle is valid JSON");
    }
}
```

- [ ] **Step 6: Run the structural test to verify the generator (it should PASS; the golden tests FAIL on the placeholders)**

Run: `cargo test -p qmd-fast-core --lib schema 2>&1 | tail -30`
Expected: `schemas_are_structurally_sane` PASSES (it tests the generated `Value`, independent of the committed files) but FAILS its final two `serde_json::from_str` lines? No: `{}` parses fine, so the parse lines pass. `frontmatter_schema_matches_committed` and `site_schema_matches_committed` FAIL (generated schema != `{}` placeholder). This is the expected red state before blessing.

- [ ] **Step 7: Bless to generate the real committed schema files**

Run: `QMD_FAST_BLESS=1 cargo test -p qmd-fast-core --lib schema 2>&1 | tail -10`
This rewrites `crates/core/assets/schema/qmd-frontmatter.schema.json` and `qmd-site.schema.json` with the generator output. Then inspect them:

Run: `head -20 crates/core/assets/schema/qmd-site.schema.json && echo "---" && python3 -m json.tool crates/core/assets/schema/qmd-frontmatter.schema.json >/dev/null && echo "frontmatter is valid JSON"`
Expected: a Draft-2020-12 object schema with `additionalProperties: false` and the `_site.yml` keys under `properties`; the frontmatter file is valid JSON.

- [ ] **Step 8: Run the schema tests normally (now green)**

Run: `cargo test -p qmd-fast-core --lib schema`
Expected: all three tests PASS (the committed files now equal the generator output).

- [ ] **Step 9: Confirm no runtime dependency leaked + full gate**

Run: `cargo tree -p qmd-fast-server -e no-dev 2>/dev/null | grep serde_json && echo "LEAKED INTO RUNTIME" || echo "serde_json stays dev-only"`
Expected: `serde_json stays dev-only`.
Then: `cargo test -p qmd-fast-core 2>&1 | grep -E 'test result:' | grep -vE '0 failed' && echo FAILURES || echo "core green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `core green`, fmt clean, clippy clean.

- [ ] **Step 10: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/lib.rs crates/core/src/frontmatter.rs crates/core/src/site/config/mod.rs crates/core/src/site/mod.rs crates/core/src/schema.rs crates/core/assets/schema/qmd-frontmatter.schema.json crates/core/assets/schema/qmd-site.schema.json
git commit -m "feat(schema): generate drift-locked JSON Schemas for front matter + _site.yml"
```

---

### Task 2: The `qmd-fast schema` CLI subcommand (server)

**Files:**
- Modify: `crates/server/src/main.rs` (dispatch + `cmd_schema`)
- Create: `crates/server/tests/schema_cli.rs`

**Interfaces:**
- Consumes: `qmd_fast_core::schema::{FRONTMATTER_SCHEMA, SITE_SCHEMA}` (from Task 1).
- Produces: a `schema` subcommand: `qmd-fast schema` prints both schemas to stdout; `qmd-fast schema --out <dir>` writes `<dir>/qmd-frontmatter.schema.json` + `<dir>/qmd-site.schema.json`.

- [ ] **Step 1: Write the failing CLI test**

Create `crates/server/tests/schema_cli.rs`:

```rust
use std::process::Command;

/// `qmd-fast schema --out <dir>` writes both schema files, each a closed Draft-2020-12 schema.
#[test]
fn schema_subcommand_writes_both_files() {
    let dir = std::env::temp_dir().join(format!("qmd-schema-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let status = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .args(["schema", "--out", dir.to_str().unwrap()])
        .status()
        .expect("run qmd-fast schema");
    assert!(status.success(), "schema --out should succeed");
    for name in ["qmd-frontmatter.schema.json", "qmd-site.schema.json"] {
        let body = std::fs::read_to_string(dir.join(name)).expect("schema file written");
        assert!(
            body.contains("\"$schema\": \"https://json-schema.org/draft/2020-12/schema\""),
            "{name} carries the Draft-2020-12 id"
        );
        assert!(
            body.contains("\"additionalProperties\": false"),
            "{name} is a closed schema"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `qmd-fast schema` with no args prints both schemas to stdout.
#[test]
fn schema_subcommand_prints_to_stdout() {
    let out = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .arg("schema")
        .output()
        .expect("run qmd-fast schema");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("qmd-fast document front matter"), "prints the front-matter schema");
    assert!(stdout.contains("qmd-fast _site.yml"), "prints the site schema");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p qmd-fast-server --test schema_cli`
Expected: FAIL (the `schema` subcommand is unknown, so the binary prints the unknown-command error and exits non-zero / stdout lacks the titles).

- [ ] **Step 3: Add the subcommand dispatch + `cmd_schema`**

In `crates/server/src/main.rs`, add a match arm in `main`'s `match args.get(1).map(String::as_str)` (next to the `"blocks"` arm, around line 26):

```rust
        Some("schema") => cmd_schema(&args),
```

Then add the function (place it near `cmd_blocks`, and mirror `cmd_build`'s `--out` parsing):

```rust
/// Emit the bundled JSON Schemas for qmd-fast's YAML config (document front matter +
/// `_site.yml`) so an editor's YAML language server can validate them. With `--out <dir>`
/// it writes two files there; otherwise it prints both to stdout. The strings are the
/// committed, bundled schemas (no runtime JSON generation).
fn cmd_schema(args: &[String]) -> ExitCode {
    use qmd_fast_core::schema::{FRONTMATTER_SCHEMA, SITE_SCHEMA};
    let files = [
        ("qmd-frontmatter.schema.json", FRONTMATTER_SCHEMA),
        ("qmd-site.schema.json", SITE_SCHEMA),
    ];
    // Optional `--out <dir>` (alias `--dir`), parsed like `cmd_build`.
    let mut out: Option<String> = None;
    let mut it = args.iter().skip(2);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" | "--dir" => out = it.next().cloned(),
            _ => {}
        }
    }
    match out {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("qmd-fast schema: cannot create {dir}: {e}");
                return ExitCode::FAILURE;
            }
            for (name, body) in files {
                let path = std::path::Path::new(&dir).join(name);
                if let Err(e) = std::fs::write(&path, body) {
                    eprintln!("qmd-fast schema: cannot write {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
                println!("wrote {}", path.display());
            }
            println!(
                "add `# yaml-language-server: $schema={dir}/qmd-site.schema.json` atop _site.yml"
            );
        }
        None => {
            for (name, body) in files {
                println!("// {name}");
                print!("{body}");
            }
        }
    }
    ExitCode::SUCCESS
}
```

(If `ExitCode` / `std::process::ExitCode` is not already imported in `main.rs`, it is, since `main` returns `ExitCode`; reuse the existing import.)

- [ ] **Step 4: Run the CLI test**

Run: `cargo test -p qmd-fast-server --test schema_cli`
Expected: both tests PASS.

- [ ] **Step 5: Update the help text**

In `crates/server/src/main.rs`, find the `--help` / usage text block (the `Some("--help" | "-h" | "help") | None =>` arm around line 38) and add a line documenting the new subcommand, matching the existing format, for example after the `blocks` line:

```
  qmd-fast schema [--out <dir>]      emit JSON Schemas for _site.yml + front matter (editor autocomplete)
```

(Match the surrounding indentation/style of the existing usage lines exactly; if they are built with `println!` per line, add one more `println!` in the same style.)

- [ ] **Step 6: Full gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/main.rs crates/server/tests/schema_cli.rs
git commit -m "feat(cli): add `qmd-fast schema` to emit the config JSON Schemas"
```

---

### Task 3: Document the editor on-ramp

**Files:**
- Modify: `docs/guide/reference/configuration.qmd` (the primary section)
- Modify: `docs/internals/sites.qmd` (a short pointer)

**Interfaces:** none (docs only). This is prose; the "test" is that the docs render and the commands shown match Task 2's behavior.

- [ ] **Step 1: Add the editor-autocomplete section to `configuration.qmd`**

In `docs/guide/reference/configuration.qmd`, insert this section after the `## Project `_site.yml`` section and before `## Quarto migration` (around line 104):

````markdown
## Editor autocomplete (config schema)

qmd-fast ships JSON Schemas for both config surfaces so your editor's YAML
language server can autocomplete keys, show hovers, and flag unknown keys as you
type, before you render. There is no qmd-fast language server to install: the
schemas describe the same key sets qmd-fast validates against at render time, so
the editor and the renderer always agree.

Drop the schemas into your project:

```sh
qmd-fast schema --out .qmd
```

Then point the YAML language server at them with a modeline.

For `_site.yml` (a plain YAML file, works out of the box), add at the very top:

```yaml
# yaml-language-server: $schema=.qmd/qmd-site.schema.json
title: My site
```

For a document's front matter, put the modeline inside the `---` block:

```yaml
---
# yaml-language-server: $schema=.qmd/qmd-frontmatter.schema.json
title: My post
---
```

Front-matter validation needs an editor that treats a `.qmd` file's `---` block
as embedded YAML (for example the VS Code YAML extension). A plain YAML language
server does not parse a `.qmd` file as YAML, so the front-matter modeline only
takes effect with that support; `_site.yml` needs no such setup.
````

- [ ] **Step 2: Add a pointer in `sites.qmd`**

In `docs/internals/sites.qmd`, at the end of the `## The config model` section (around line 34-47, before `## Books vs websites`), add a short paragraph:

```markdown
qmd-fast generates a JSON Schema for `_site.yml` from `NATIVE_KEYS` (the same
closed set the config validator uses), emitted by `qmd-fast schema`. See the
Configuration reference (`../guide/reference/configuration.qmd`) for the editor
autocomplete on-ramp.
```

- [ ] **Step 3: Verify the docs render**

Run: `cargo run -q -p qmd-fast-server -- build docs/guide/reference/configuration.qmd /tmp/configuration.html 2>&1 | tail -5; grep -c 'Editor autocomplete' /tmp/configuration.html`
Expected: the build succeeds and the new heading appears (grep count >= 1).

- [ ] **Step 4: Commit**

```bash
git add docs/guide/reference/configuration.qmd docs/internals/sites.qmd
git commit -m "docs: editor autocomplete via the config JSON Schemas (yaml-language-server)"
```

---

## Self-Review

**Spec coverage** (the design at `docs/superpowers/specs/2026-06-24-jsonschema-for-config-design.md`):
- "Generator from the consts" + "additionalProperties:false at closed levels, loose value types, format permissive" → Task 1 Step 5 (`front_matter_schema`/`site_config_schema`, the `closed_object` helper, the `toc`/`max-items`/`categories`/execute-children type overrides, `format` left `{}`).
- "Committed + bundled files" → Task 1 (placeholder then bless then commit; `pub const … = include_str!`).
- "Golden-file drift lock with bless" → Task 1 Step 5 `tests` + Steps 7-8 (`QMD_FAST_BLESS=1`).
- "serde_json as a dev-dependency, no runtime dep" → Task 1 Step 1 + the Step 9 `cargo tree` guard + the generator being `#[cfg(test)]` and the CLI emitting static strings.
- "`qmd-fast schema [--out <dir>]` CLI" → Task 2.
- "Docs for both surfaces, with the front-matter editor caveat" → Task 3.
- "Structural sanity test" → Task 1 Step 5 `schemas_are_structurally_sane`.
- Out-of-scope items (no cell-option/callout schema, no live server route, no hosted URL) are honored: none of the tasks add them.

**Placeholder scan:** No TBD/TODO. The `{}` files in Task 1 Step 4 are an explicit, explained bootstrap for `include_str!`, regenerated in Step 7 (not a left-behind placeholder). The committed JSON content is a generated artifact (produced by the bless run), which is why its bytes are not transcribed here; the generator that produces it is complete code, and Steps 7-8 verify the result. Every code step shows complete code.

**Type consistency:** `FRONTMATTER_SCHEMA` / `SITE_SCHEMA` are defined in Task 1 and consumed by the same names in Task 2. The generator helpers (`properties`, `closed_object`, `boolean`, `integer`, `front_matter_schema`, `site_config_schema`, `to_pretty_json`) are all defined and used within Task 1. `KNOWN_KEYS` + the nested sets and `NATIVE_KEYS` are made `pub(crate)` in Task 1 Step 2 before being read in Step 5. `listing_item` is cloned for its second use in the `oneOf`. The CLI consumes `qmd_fast_core::schema::{FRONTMATTER_SCHEMA, SITE_SCHEMA}`, matching the `pub const` names.

**Scope check:** Three independently testable, independently committable tasks. Task 1 is the core (generator + drift lock), Task 2 the user-facing CLI, Task 3 the docs. No render/block-model/validator-behavior change; `serde_json` confined to dev. Each task ends on a green gate.
