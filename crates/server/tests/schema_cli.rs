use std::process::Command;

/// `qmd-fast schema --out <dir>` writes both schema files, each a closed Draft-2020-12 schema.
#[test]
fn schema_subcommand_writes_both_files() {
    let dir = std::env::temp_dir().join(format!("qmd-schema-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let status = Command::new(env!("CARGO_BIN_EXE_taliesin"))
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
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("schema")
        .output()
        .expect("run qmd-fast schema");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Taliesin document front matter"),
        "prints the front-matter schema"
    );
    assert!(
        stdout.contains("Taliesin _site.yml"),
        "prints the site schema"
    );
}
