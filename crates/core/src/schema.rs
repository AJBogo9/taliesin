//! JSON Schema for taliesin's YAML config surfaces (document front matter + `_site.yml`).
//!
//! The committed `assets/schema/*.schema.json` files, bundled here as static strings, are
//! generated from the SAME closed-set consts the validator uses (`frontmatter::KNOWN_KEYS`
//! plus the nested `EXECUTE`/`LISTING`/`ABOUT`/`HERO` sets, and `site::NATIVE_KEYS`), so the
//! schema cannot drift from what the validator enforces. They are regenerated ONLY via the
//! bless path in this module's tests (`TALIESIN_BLESS=1 cargo test -p taliesin-core --lib
//! schema`), never hand-edited. The `taliesin schema` CLI emits these strings so an editor's
//! YAML language server can validate config: the in-scope single-editing-surface on-ramp,
//! with no taliesin language server to build.

/// The Draft-2020-12 JSON Schema for a document's YAML front matter.
pub const FRONTMATTER_SCHEMA: &str = include_str!("../assets/schema/tali-frontmatter.schema.json");

/// The Draft-2020-12 JSON Schema for a project's `_site.yml`.
pub const SITE_SCHEMA: &str = include_str!("../assets/schema/tali-site.schema.json");

#[cfg(test)]
mod generate {
    use crate::frontmatter::{
        ABOUT_KEYS, EXECUTE_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, PROSE_LINT_KEYS,
        THEOREM_KEYS,
    };
    use crate::site::{NATIVE_KEYS, PUBLISH_KEYS};
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
        let listing_item = closed_object(
            LISTING_KEYS,
            &[("max-items", integer()), ("categories", boolean())],
        );
        // listing: a single mapping or a sequence of mappings (cv.tmd shape).
        let listing = json!({
            "oneOf": [listing_item.clone(), { "type": "array", "items": listing_item }]
        });
        let about = closed_object(ABOUT_KEYS, &[]);
        let hero = closed_object(HERO_KEYS, &[]);
        // prose-lint: `true` (built-in rules) or `{ banned: [strings] }`.
        let prose_lint = json!({
            "oneOf": [
                boolean(),
                closed_object(
                    PROSE_LINT_KEYS,
                    &[("banned", json!({ "type": "array", "items": { "type": "string" } }))],
                )
            ]
        });
        // theorems: `shared` is a list of kind names sharing one counter.
        let theorems = closed_object(
            THEOREM_KEYS,
            &[
                (
                    "shared",
                    json!({ "type": "array", "items": { "type": "string" } }),
                ),
                (
                    "number-within",
                    json!({ "type": "string", "enum": ["chapter"] }),
                ),
                (
                    "numbered",
                    json!({ "oneOf": [{ "type": "boolean" }, { "type": "string", "enum": ["unless-unique"] }] }),
                ),
            ],
        );
        let overrides = [
            ("toc", boolean()),
            ("execute", execute),
            ("listing", listing),
            ("about", about),
            ("hero", hero),
            ("prose-lint", prose_lint),
            ("theorems", theorems),
            // An extension owns `format:`'s sub-keys, so leave it fully permissive.
            ("format", json!({})),
        ];
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Taliesin document front matter",
            "type": "object",
            "additionalProperties": false,
            "properties": properties(KNOWN_KEYS, &overrides),
        })
    }

    pub fn site_config_schema() -> Value {
        // A book chapter is either a bare path (`- intro.tmd`) or a `{ file:, text: }`
        // mapping whose `text:` overrides the sidebar label.
        let chapter = json!({
            "oneOf": [
                { "type": "string" },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["file"],
                    "properties": { "file": { "type": "string" }, "text": { "type": "string" } },
                },
            ]
        });
        // `chapters:` is a list of chapters and/or `{ part:, chapters: }` group headers
        // (each group's inner list takes the same chapter shapes).
        let chapters = json!({
            "type": "array",
            "items": {
                "oneOf": [
                    chapter.clone(),
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["part"],
                        "properties": {
                            "part": { "type": "string" },
                            "chapters": { "type": "array", "items": chapter },
                        },
                    },
                ]
            }
        });
        // publish: a closed { provider, project } block. `provider` is an enum (only
        // cloudflare today); `project` is the Cloudflare Pages project name.
        let publish = closed_object(
            PUBLISH_KEYS,
            &[
                (
                    "provider",
                    json!({ "type": "string", "enum": ["cloudflare"] }),
                ),
                ("project", json!({ "type": "string" })),
            ],
        );
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Taliesin _site.yml",
            "type": "object",
            "additionalProperties": false,
            "properties": properties(
                NATIVE_KEYS,
                &[("toc", boolean()), ("chapters", chapters), ("publish", publish)],
            ),
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

    /// Assert the generated schema equals the committed file, OR (under `TALIESIN_BLESS=1`)
    /// rewrite the committed file from the generator. `rel_path` is relative to the core
    /// crate root (`CARGO_MANIFEST_DIR`).
    fn bless_or_assert(generated: String, committed: &str, rel_path: &str) {
        if std::env::var("TALIESIN_BLESS").is_ok() {
            let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel_path);
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed {rel_path}");
        } else {
            assert_eq!(
                generated, committed,
                "schema drift in {rel_path}; regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib schema`"
            );
        }
    }

    #[test]
    fn frontmatter_schema_matches_committed() {
        bless_or_assert(
            to_pretty_json(&front_matter_schema()),
            FRONTMATTER_SCHEMA,
            "assets/schema/tali-frontmatter.schema.json",
        );
    }

    #[test]
    fn site_schema_matches_committed() {
        bless_or_assert(
            to_pretty_json(&site_config_schema()),
            SITE_SCHEMA,
            "assets/schema/tali-site.schema.json",
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
            assert_eq!(
                v["additionalProperties"],
                Value::Bool(false),
                "{name}: closed"
            );
            assert!(v["properties"].is_object(), "{name}: has properties");
        }
        // Every closed-set key appears as a property, so a future key the validator gains but
        // the schema forgets is caught here (not just by the golden file).
        let fm = front_matter_schema();
        for k in crate::frontmatter::KNOWN_KEYS {
            assert!(
                fm["properties"].get(k).is_some(),
                "frontmatter schema missing `{k}`"
            );
        }
        let site = site_config_schema();
        for k in crate::site::NATIVE_KEYS {
            assert!(
                site["properties"].get(k).is_some(),
                "site schema missing `{k}`"
            );
        }
        // The committed bundles parse as JSON (catches an empty or corrupt committed file).
        serde_json::from_str::<Value>(FRONTMATTER_SCHEMA)
            .expect("frontmatter bundle is valid JSON");
        serde_json::from_str::<Value>(SITE_SCHEMA).expect("site bundle is valid JSON");
    }
}
