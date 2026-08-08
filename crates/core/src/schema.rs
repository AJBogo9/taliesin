//! JSON Schema for `_site.yml`, the one YAML config surface no language server covers.
//!
//! The committed `assets/schema/tali-site.schema.json` file, bundled here as a static string,
//! is generated from the SAME closed-set const the validator uses (`site::NATIVE_KEYS`), so
//! the schema cannot drift from what the validator enforces. It is regenerated ONLY via the
//! bless path in this module's tests (`TALIESIN_BLESS=1 cargo test -p taliesin-core --lib
//! schema`), never hand-edited. `taliesin init` writes it into `.taliesin/` and points the
//! scaffolded `_site.yml` at it with a `# yaml-language-server:` modeline; the companion
//! wires the same file through `yamlValidation`.
//!
//! **Front matter has no schema here, and needs none.** It lives inside a `.tmd` file, where
//! no YAML language server ever looks — `taliesin lsp` is what completes and validates it,
//! from `vocab.rs` and `frontmatter::KNOWN_KEYS`. The generated front-matter schema was an
//! on-ramp written before that server existed and was withdrawn with `taliesin schema` in
//! Wave 2.

/// The Draft-2020-12 JSON Schema for a project's `_site.yml`.
pub const SITE_SCHEMA: &str = include_str!("../assets/schema/tali-site.schema.json");

#[cfg(test)]
mod generate {
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
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Taliesin _site.yml",
            "type": "object",
            "additionalProperties": false,
            "properties": properties(NATIVE_KEYS, &[("chapters", chapters)]),
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
    use super::SITE_SCHEMA;
    use super::generate::{site_config_schema, to_pretty_json};
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
    fn site_schema_matches_committed() {
        bless_or_assert(
            to_pretty_json(&site_config_schema()),
            SITE_SCHEMA,
            "assets/schema/tali-site.schema.json",
        );
    }

    #[test]
    fn the_schema_is_structurally_sane() {
        let site = site_config_schema();
        assert_eq!(
            site["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "draft id"
        );
        assert_eq!(site["type"], "object", "type");
        assert_eq!(
            site["additionalProperties"],
            Value::Bool(false),
            "the schema must be closed, or an unknown key validates"
        );
        assert!(site["properties"].is_object(), "has properties");
        // Every closed-set key appears as a property, so a future key the validator gains but
        // the schema forgets is caught here (not just by the golden file).
        for k in crate::site::NATIVE_KEYS {
            assert!(
                site["properties"].get(k).is_some(),
                "site schema missing `{k}`"
            );
        }
        // The committed bundle parses as JSON (catches an empty or corrupt committed file).
        serde_json::from_str::<Value>(SITE_SCHEMA).expect("site bundle is valid JSON");
    }
}
