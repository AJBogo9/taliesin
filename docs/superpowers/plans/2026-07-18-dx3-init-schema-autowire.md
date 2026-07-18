# DX3 — Auto-wire config JSON Schema on `init`: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox syntax.

**Goal:** `taliesin init` emits `.taliesin/tali-{site,frontmatter}.schema.json` and prepends the
`# yaml-language-server: $schema=.taliesin/tali-site.schema.json` modeline to `_site.yml`, so config
autocompletes/validates in-editor with zero manual step.

**Architecture:** One file (`crates/server/src/cli.rs`): change `INIT_SITE_YML`, extend
`scaffold_init`'s file list + create parent dirs, extend the existing init test. Reuse the bundled
`taliesin_core::schema::{SITE_SCHEMA, FRONTMATTER_SCHEMA}` constants (same source as `taliesin schema`).

## Global Constraints

- Reuse the schema **constants** (no duplicated JSON, no drift from the validator).
- Schema files in a `.taliesin/` dot-dir (skipped by all site walkers — no phantom page, never in `_site/`).
- Modeline is a YAML comment referencing a **relative** path from `_site.yml`.
- Overwrite guard stays all-or-nothing and now covers the schema files.
- `cargo fmt`-clean (rustfmt PostToolUse hook).

---

### Task 1: Wire schemas + modeline into `init`

**Files:** Modify `crates/server/src/cli.rs` (`INIT_SITE_YML` L20; `scaffold_init` L89-120; test `init_scaffolds_a_previewable_site` L613-644).

- [ ] **Step 1: Update the test first (failing).** Replace the `written ==` assertion block and add the schema/modeline assertions:

```rust
        let site_yml = dir.join("_site.yml");
        let index = dir.join("index.tmd");
        let agents = dir.join("AGENTS.md");
        let site_schema = dir.join(".taliesin").join("tali-site.schema.json");
        let fm_schema = dir.join(".taliesin").join("tali-frontmatter.schema.json");
        assert!(site_yml.exists(), "_site.yml written");
        assert!(index.exists(), "index.tmd written");
        assert!(agents.exists(), "AGENTS.md written");
        assert!(site_schema.exists(), ".taliesin/tali-site.schema.json written");
        assert!(fm_schema.exists(), ".taliesin/tali-frontmatter.schema.json written");
        assert_eq!(
            written,
            vec![
                site_yml.clone(),
                index.clone(),
                agents.clone(),
                site_schema.clone(),
                fm_schema.clone(),
            ]
        );

        // The scaffold is a real, parseable site whose one page previews.
        let cfg = fs::read_to_string(&site_yml).unwrap();
        assert!(cfg.contains("title:"), "config has a title: {cfg}");

        // Load-bearing: the modeline points at a real schema whose body is the bundled one, so
        // the referenced path and the emitted file can never silently drift.
        let first = cfg.lines().next().unwrap_or("");
        assert!(
            first.starts_with("# yaml-language-server: $schema="),
            "first line is the schema modeline: {first}"
        );
        let rel = first.trim_end().rsplit('=').next().unwrap();
        let pointed = dir.join(rel);
        assert!(pointed.exists(), "modeline path resolves to a real file: {rel}");
        assert_eq!(
            fs::read_to_string(&pointed).unwrap(),
            taliesin_core::schema::SITE_SCHEMA,
            "the wired schema is the bundled SITE_SCHEMA"
        );
        assert_eq!(
            fs::read_to_string(&fm_schema).unwrap(),
            taliesin_core::schema::FRONTMATTER_SCHEMA,
        );
        let page = fs::read_to_string(&index).unwrap();
        assert!(
            page.starts_with("---") && page.contains("title:"),
            "index has front matter: {page}"
        );
```

- [ ] **Step 2: Run it — expect FAIL** (`.taliesin/...` not written; modeline absent).

Run: `cargo test -p taliesin-server init_scaffolds_a_previewable_site`
Expected: FAIL (schema files missing / modeline missing).

- [ ] **Step 3: Add the modeline to `INIT_SITE_YML`:**

```rust
/// `_site.yml` for the scaffold: the schema modeline (so an editor's YAML language server
/// validates + autocompletes config keys with zero manual step — the schema is emitted into
/// `.taliesin/` beside it) followed by the minimal flat-native config (just a title).
const INIT_SITE_YML: &str =
    "# yaml-language-server: $schema=.taliesin/tali-site.schema.json\ntitle: My site\n";
```

- [ ] **Step 4: Extend `scaffold_init`'s file list + create parent dirs.** Change the `files` array and the write loop:

```rust
    let files = [
        ("_site.yml", INIT_SITE_YML),
        ("index.tmd", INIT_INDEX_TMD),
        // The agent onramp (edit `.tmd`/`check --format json`/dialect). Generated from the
        // validator vocabulary and golden-locked in core, so it cannot drift from `check`.
        ("AGENTS.md", taliesin_core::agents::AGENTS_MD),
        // The bundled config schemas (same constants `taliesin schema` emits, so they can't
        // drift from the validator), wired into `_site.yml` via the modeline above. In a
        // walker-skipped dot-dir so they never become a page or ship into `_site/`.
        (
            ".taliesin/tali-site.schema.json",
            taliesin_core::schema::SITE_SCHEMA,
        ),
        (
            ".taliesin/tali-frontmatter.schema.json",
            taliesin_core::schema::FRONTMATTER_SCHEMA,
        ),
    ];
```

And in the write loop, create the parent before writing (schema files live in a subdir):

```rust
    let mut written = Vec::new();
    for (name, contents) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("cannot create {}: {e}", parent.display()));
            }
        }
        if let Err(e) = std::fs::write(&path, contents) {
            return Err(format!("cannot write {}: {e}", path.display()));
        }
        written.push(path);
    }
    Ok(written)
```

- [ ] **Step 5: Run the test — expect PASS.**

Run: `cargo test -p taliesin-server init_scaffolds_a_previewable_site`
Expected: PASS.

- [ ] **Step 6: Mutation-check the load-bearing pin.** Temporarily change the modeline in `INIT_SITE_YML` to `.taliesin/tali-NONEXISTENT.schema.json`, re-run → the "modeline path resolves to a real file" assertion FAILS → revert.

- [ ] **Step 7: Integration smoke** (real binary):

```bash
cargo build -p taliesin-server
D=$(mktemp -d); ./target/debug/taliesin init "$D" >/dev/null
head -1 "$D/_site.yml"                              # expect the modeline
ls "$D/.taliesin"                                   # expect both .schema.json
diff <(./target/debug/taliesin schema | sed -n '/tali-site/,$p' | tail -n +2) "$D/.taliesin/tali-site.schema.json" && echo "site schema identical to \`schema\`"
./target/debug/taliesin build "$D" --out "$D/_out" >/dev/null 2>&1
test ! -e "$D/_out/.taliesin" && echo "schemas NOT shipped into _site" || echo "LEAK: .taliesin in output"
rm -rf "$D"
```
Expected: modeline present, both schemas listed, site schema byte-identical to `schema`, no `.taliesin/` in the built output.

- [ ] **Step 8: Full gate + commit.**

```bash
cargo test -p taliesin-core -p taliesin-server 2>&1 | grep -E "test result:|FAILED" | tail
cargo fmt --check && cargo clippy -p taliesin-server --all-targets -- -D warnings
git add crates/server/src/cli.rs && git commit -m "feat(init): auto-wire config JSON Schema (.taliesin/ + _site.yml modeline) (DX3)"
```

---

## Self-Review

- **Spec coverage:** schema emit + modeline + dotdir + guard + DRY reuse → Task 1 Steps 3-4; no-drift pin → Step 1/6; dotdir-not-shipped → Step 7.
- **Placeholder scan:** none.
- **Type consistency:** `SITE_SCHEMA`/`FRONTMATTER_SCHEMA` (the real const names) used in both impl and test; paths `.taliesin/tali-{site,frontmatter}.schema.json` consistent throughout.
