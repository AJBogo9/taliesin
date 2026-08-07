//! `taliesin lsp` as a real process, which is the only way the editor ever runs it: the
//! in-process tests in `lsp.rs` drive `run()` over an in-memory connection and never touch
//! the command that wraps it. What that wrapper owns is the exit code — an editor decides
//! from it whether the language server died or finished — and the rule that stdout carries
//! the protocol and nothing else.

use std::io::Write;
use std::process::{Command, Stdio};

/// One `Content-Length`-framed LSP message.
fn frame(body: serde_json::Value) -> String {
    let s = body.to_string();
    format!("Content-Length: {}\r\n\r\n{s}", s.len())
}

/// The `result` of the response to request `id`, parsed out of the framed stdout stream.
///
/// Asserting with `stdout.contains(…)` is a trap here: the same stream carries
/// `publishDiagnostics` notifications whose messages quote the document's own text, so a
/// substring check can pass on a diagnostic while the response it claims to test is empty.
fn response(stdout: &str, id: u64) -> serde_json::Value {
    for chunk in stdout.split("Content-Length: ") {
        let Some(body) = chunk.split_once("\r\n\r\n").map(|(_, b)| b) else {
            continue;
        };
        // Each frame's body is one JSON value; the next frame's header follows it.
        let mut de = serde_json::Deserializer::from_str(body).into_iter::<serde_json::Value>();
        let Some(Ok(value)) = de.next() else { continue };
        if value["id"] == id {
            return value["result"].clone();
        }
    }
    panic!("no response for id {id} in:\n{stdout}");
}

/// Run `taliesin lsp`, write `input` to its stdin, close it, and return
/// (exit code, stdout, stderr).
fn lsp_session(input: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn taliesin lsp");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).expect("write");
        // Dropping stdin closes it; the reader thread then sees EOF.
    }
    let out = child.wait_with_output().expect("wait lsp");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A clean initialize → shutdown → exit session exits 0 and answers on stdout.
#[test]
fn a_completed_session_exits_success() {
    let input = format!(
        "{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(
        code,
        Some(0),
        "clean session should exit 0 (stderr: {stderr})"
    );
    assert!(
        stdout.contains("\"capabilities\""),
        "the initialize result should reach stdout, got {stdout:?}"
    );
}

/// A client that dies before saying `initialize` (here: stdin closed straight away) is a
/// protocol failure, and the command must report it as one. Exiting 0 would tell the editor
/// the server shut down cleanly, so it would not surface the crash or restart it — and the
/// diagnosis has to go to stderr, because stdout is the protocol wire.
#[test]
fn a_protocol_failure_exits_nonzero_and_never_writes_to_stdout() {
    let (code, stdout, stderr) = lsp_session("");
    assert_eq!(code, Some(1), "a failed session should exit 1");
    assert!(
        stdout.is_empty(),
        "nothing but protocol may reach stdout, got {stdout:?}"
    );
    assert!(
        stderr.contains("lsp:"),
        "the failure should be logged to stderr, got {stderr:?}"
    );
}

/// A `.tmd` buffer is served whatever `languageId` the editor sends.
///
/// Only the VS Code companion declares the `taliesin` language. Every other editor the
/// CLI reference wires up (`cmd = { "taliesin", "lsp" }` for Neovim, Helix, Zed) sends
/// its own filetype, and nothing in this repo registers one for `.tmd` — so the id
/// arrives as `""` or `"markdown"`. Gating admission on the id alone made the server
/// advertise hover, completion, symbols, rename and diagnostics and then answer null to
/// all of them, with nothing on stderr to say why.
///
/// This has to live here rather than beside the in-process tests in `lsp.rs`: those all
/// open documents through a helper that hard-codes `language_id: "taliesin"`, so they
/// are structurally unable to observe the gate.
#[test]
fn a_tmd_buffer_is_linted_whatever_language_id_the_editor_sends() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-lang-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = dir.join("b.tmd");
    // `tittle:` is an unknown front-matter key, so a served buffer must publish at
    // least one diagnostic. A buffer that is dropped publishes none.
    let text = "---\ntittle: oops\n---\n\n# H\n";
    std::fs::write(&doc, text).expect("fixture");
    let uri = format!("file://{}", doc.display());

    for language_id in ["taliesin", "markdown", "", "tmd"] {
        let input = format!(
            "{}{}{}{}{}",
            frame(serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "capabilities": {} }
            })),
            frame(serde_json::json!({
                "jsonrpc": "2.0", "method": "initialized", "params": {}
            })),
            frame(serde_json::json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": { "textDocument": {
                    "uri": uri, "languageId": language_id, "version": 1, "text": text
                }}
            })),
            frame(serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null
            })),
            frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
        );
        let (code, stdout, stderr) = lsp_session(&input);
        assert_eq!(
            code,
            Some(0),
            "languageId {language_id:?}: stderr:\n{stderr}"
        );
        assert!(
            stdout.contains("textDocument/publishDiagnostics"),
            "languageId {language_id:?}: a .tmd buffer must be linted whatever the \
             editor calls it, but no diagnostics were published.\nstdout:\n{stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `textDocument/documentLink` answers over the real wire with a link on an existing
/// `{{< include >}}` target, and nothing on a missing one.
///
/// The in-process tests can assert the scan; only a real session proves the capability is
/// advertised, the request reaches a handler (rather than the `MethodNotFound` every
/// unhandled method gets), and the response serializes into something a client can use.
#[test]
fn document_link_answers_for_an_existing_include_and_skips_a_missing_one() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-links-{}", std::process::id()));
    let _ = std::fs::create_dir_all(dir.join("parts"));
    std::fs::write(dir.join("parts/real.tmd"), "# Part\n").expect("fixture part");
    let doc = dir.join("index.tmd");
    let text = "---\ntitle: T\n---\n\n\
                {{< include parts/real.tmd >}}\n\n\
                {{< include parts/absent.tmd >}}\n";
    std::fs::write(&doc, text).expect("fixture doc");
    let uri = format!("file://{}", doc.display());

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
            "params": { "textDocument": { "uri": uri } }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "documentLink must be handled, not rejected.\nstdout:\n{stdout}"
    );
    // Scope every claim to the documentLink RESPONSE. The stream also carries a
    // `publishDiagnostics` notification that names `parts/absent.tmd` (check's
    // TAL-SHORTCODE "include not resolved"), so a whole-stdout `contains` would pass on
    // the diagnostic and prove nothing about the links.
    let links = response(&stdout, 2);
    let links = links.as_array().expect("documentLink returns an array");
    assert_eq!(
        links.len(),
        1,
        "only the existing target should be linked; a missing one promises a jump to \
         nothing (check already reports it): {links:?}"
    );
    assert!(
        links[0]["target"]
            .as_str()
            .unwrap_or_default()
            .ends_with("parts/real.tmd"),
        "the link should point at the included file: {links:?}"
    );
    assert_eq!(links[0]["tooltip"], "Open parts/real.tmd");
    // The span covers the path token only, not the whole `{{< … >}}` directive.
    assert_eq!(links[0]["range"]["start"]["line"], 4);
    assert_eq!(links[0]["range"]["start"]["character"], 12);
    assert_eq!(links[0]["range"]["end"]["character"], 12 + 14);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Hovering an `{{< include >}}` path names the file it resolves to. This answered `None`
/// until now even though the target was classified and go-to-definition resolved it, so
/// hover — the first place an author looks — was the one surface that said nothing.
#[test]
fn hovering_an_include_path_names_the_target() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-inchover-{}", std::process::id()));
    let _ = std::fs::create_dir_all(dir.join("parts"));
    std::fs::write(dir.join("parts/real.tmd"), "# Part\n").expect("fixture part");
    let doc = dir.join("index.tmd");
    let text = "---\ntitle: T\n---\n\n{{< include parts/real.tmd >}}\n";
    std::fs::write(&doc, text).expect("fixture doc");
    let uri = format!("file://{}", doc.display());

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
            // line 4 (0-based) is the include; column 15 sits inside `parts/real.tmd`.
            "params": { "textDocument": { "uri": uri },
                        "position": { "line": 4, "character": 15 } }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        stdout.contains("parts/real.tmd"),
        "hover should name the included file.\nstdout:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Math completion answers over the real wire, with the KaTeX-authoritative vocabulary and a
/// snippet for a command that takes arguments.
///
/// `$…$` was the one place in a `.tmd` where the editor knew nothing: the grammar colorized
/// math, so it looked supported and behaved unsupported.
#[test]
fn completion_offers_math_commands_inside_math_and_nothing_outside_it() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-math-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = dir.join("m.tmd");
    // line 4 is inside inline math; line 6 is prose with the same backslash.
    let text = "---\ntitle: T\n---\n\nLet $\\fra\n\nProse \\fra\n";
    std::fs::write(&doc, text).expect("fixture");
    let uri = format!("file://{}", doc.display());

    let completion = |id: u64, line: u32, character: u32| {
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "textDocument/completion",
            "params": { "textDocument": { "uri": uri },
                        "position": { "line": line, "character": character } }
        }))
    };
    let input = format!(
        "{}{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        completion(2, 4, 9),  // end of `Let $\fra`
        completion(3, 6, 10), // end of `Prose \fra`
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");

    let in_math = response(&stdout, 2);
    let items = in_math.as_array().expect("completion returns items");
    let frac = items
        .iter()
        .find(|i| i["label"] == "\\frac")
        .unwrap_or_else(|| panic!("`\\frac` should be offered inside math: {items:?}"));
    assert_eq!(
        frac["insertTextFormat"], 2,
        "a command with arguments must insert a SNIPPET so the cursor lands in the first \
         placeholder: {frac:?}"
    );
    assert_eq!(
        frac["textEdit"]["newText"], "\\frac{$1}{$2}",
        "the edit must insert the argument shape: {frac:?}"
    );
    // The edit REPLACES the typed `\fra`, so accepting cannot leave `\fra\frac`.
    assert_eq!(frac["textEdit"]["range"]["start"]["character"], 5);
    assert_eq!(frac["textEdit"]["range"]["end"]["character"], 9);
    assert!(
        items
            .iter()
            .any(|i| i["label"] == "\\frac{}{}" || i["label"] == "\\frac"),
        "labels should read as the command itself"
    );

    assert_eq!(
        response(&stdout, 3),
        serde_json::Value::Null,
        "a backslash in PROSE must offer nothing: math commands there would render as \
         literal text, not math"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Div attribute keys over the real wire, and — the half that matters — the NARROWING.
///
/// `render/divs.rs` dispatches on class, so an attribute is not a property of divs in
/// general. A `collapse="true"` on a `.lemma` renders exactly like a bare `.lemma`, and
/// offering it there would be the editor recommending a no-op. This was the last of the 21
/// probe positions in the companion audit still answering nothing.
#[test]
fn completion_offers_div_attributes_narrowed_to_the_classes_on_the_fence() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-divattr-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = dir.join("d.tmd");
    // Line 4 is a callout's attribute slot; line 7 is a numbered theorem's.
    let text = "---\ntitle: T\n---\n\n::: {.callout-note \n:::\n\n::: {.lemma \n:::\n";
    std::fs::write(&doc, text).expect("fixture");
    let uri = format!("file://{}", doc.display());

    let completion = |id: u64, line: u32, character: u32| {
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "textDocument/completion",
            "params": { "textDocument": { "uri": uri },
                        "position": { "line": line, "character": character } }
        }))
    };
    let input = format!(
        "{}{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        completion(2, 4, 19), // end of `::: {.callout-note `
        completion(3, 7, 12), // end of `::: {.lemma `
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");

    let labels = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .unwrap_or_else(|| panic!("completion returns items: {v:?}"))
            .iter()
            .filter_map(|i| i["label"].as_str().map(str::to_string))
            .collect()
    };

    let callout = response(&stdout, 2);
    let got = labels(&callout);
    for expected in ["title", "collapse", "icon", "appearance"] {
        assert!(
            got.iter().any(|l| l == expected),
            "`{expected}=` should be offered on a callout: {got:?}"
        );
    }
    for absent in ["state", "lines", "name", "ncol", "layout-ncol"] {
        assert!(
            !got.iter().any(|l| l == absent),
            "`{absent}=` is inert on a callout (the callout arm never reads it) and must \
             not be offered: {got:?}"
        );
    }
    // The value half comes with the key, and as a snippet, so `appearance` lands on a
    // value the renderer recognizes instead of an empty pair of quotes.
    let items = callout.as_array().expect("items");
    let appearance = items
        .iter()
        .find(|i| i["label"] == "appearance")
        .expect("appearance is offered");
    assert_eq!(appearance["insertTextFormat"], 2, "{appearance:?}");
    assert_eq!(
        appearance["insertText"], "appearance=\"${1|simple,minimal|}\"",
        "the closed value set should be offered as a snippet choice: {appearance:?}"
    );

    let theorem = labels(&response(&stdout, 3));
    assert!(
        theorem.iter().any(|l| l == "title"),
        "`title=` reaches every theorem kind: {theorem:?}"
    );
    assert!(
        !theorem.iter().any(|l| l == "collapse"),
        "`collapse=` has no branch in the NUMBERED theorem arm (only `proof`), so offering \
         it on a `.lemma` would recommend a no-op: {theorem:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Table formatting over the real wire, including the promise the feature rests on: the edits
/// name ONLY the table's lines, so nothing else in the document can move.
#[test]
fn formatting_rewrites_a_table_and_names_no_other_line() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-fmt-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = dir.join("f.tmd");
    // Prose above and below, and a paragraph containing a pipe that is NOT a table.
    let text = "---\ntitle: T\n---\n\nA | pipe in prose.\n\n|a|long|\n|-|-:|\n|1|2|\n\nAfter.\n";
    std::fs::write(&doc, text).expect("fixture");
    let uri = format!("file://{}", doc.display());

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
            "params": { "textDocument": { "uri": uri },
                        "options": { "tabSize": 2, "insertSpaces": true } }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");

    // The initialize response must advertise the capability, or no editor will ever ask.
    assert_eq!(
        response(&stdout, 1)["capabilities"]["documentFormattingProvider"],
        serde_json::json!(true),
        "formatting must be advertised"
    );

    let edits = response(&stdout, 2);
    let edits = edits.as_array().expect("formatting returns edits");
    assert_eq!(edits.len(), 1, "one table, one edit: {edits:?}");
    let e = &edits[0];
    assert_eq!(
        e["range"]["start"]["line"], 6,
        "the table starts on line 6: {e:?}"
    );
    assert_eq!(e["range"]["end"]["line"], 8, "and ends on line 8: {e:?}");
    assert_eq!(
        e["newText"], "| a   | long |\n| --- | ---: |\n| 1   |    2 |",
        "columns padded, right-alignment kept: {e:?}"
    );
    // Line 4 is `A | pipe in prose.` — a paragraph with a pipe. If the range ever reaches it,
    // ordinary prose is being rewritten as a table.
    assert!(
        edits
            .iter()
            .all(|e| e["range"]["start"]["line"].as_u64().unwrap() > 5),
        "no edit may touch the prose above the table: {edits:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Inlay hints reach the editor over the real wire, carrying the resolved number.
///
/// `corpus/diagnostics/refs.tmd` is the right fixture because it holds one reference that
/// resolves (`@fig-results`) and three that do not, so a provider that annotated everything
/// and a provider that annotated nothing would both fail here.
#[test]
fn inlay_hints_number_the_one_reference_that_resolves() {
    let doc = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/refs.tmd"),
    )
    .expect("corpus fixture");
    let text = std::fs::read_to_string(&doc).expect("read fixture");
    let uri = format!("file://{}", doc.display());
    let last = text.lines().count() as u64;

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
            "params": {
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": last, "character": 0 }
                }
            }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "inlayHint must be handled, not rejected.\nstdout:\n{stdout}"
    );

    // Scoped to the inlayHint RESPONSE: the same stream carries publishDiagnostics
    // notifications that quote the document's own near-miss anchors.
    let hints = response(&stdout, 2);
    let hints = hints.as_array().expect("inlayHint returns an array");
    assert_eq!(
        hints.len(),
        1,
        "only `@fig-results` resolves in this document; the three near-misses are valid \
         diagnostics, not numbers: {hints:?}"
    );
    assert_eq!(
        hints[0]["label"], " ⟨1⟩",
        "the sole figure is Figure 1: {hints:?}"
    );
    // Line 15 (0-based) is `…never warns: see @fig-results.`, and the hint sits just past
    // the id rather than at the start of the line.
    assert_eq!(
        hints[0]["position"]["line"], 15,
        "hint on the right line: {hints:?}"
    );
    let col = hints[0]["position"]["character"]
        .as_u64()
        .expect("a column");
    let line15 = text.lines().nth(15).expect("line 15");
    assert_eq!(
        col as usize,
        line15.find("@fig-results").expect("the reference") + "@fig-results".len(),
        "the hint must sit just past the id it annotates"
    );
}

/// Folding ranges reach the editor over the real wire.
///
/// The fixture is deliberately one of each construct, because the failure this guards is a
/// provider that folds only the easy one: a heading section, a `:::` div, a code fence whose
/// `#` comment must not be read as a heading, and the front matter.
#[test]
fn folding_ranges_cover_heading_div_fence_and_front_matter() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-fold-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let doc = dir.join("fold.tmd");
    let text = "---\ntitle: T\n---\n\n\
                # One\n\n\
                ```{python}\n# not a heading\nx = 1\n```\n\n\
                ::: {.callout-note}\ninside\n:::\n";
    std::fs::write(&doc, text).expect("fixture doc");
    let uri = format!("file://{}", doc.display());

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/foldingRange",
            "params": { "textDocument": { "uri": uri } }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "foldingRange must be handled, not rejected.\nstdout:\n{stdout}"
    );

    let folds = response(&stdout, 2);
    let folds = folds.as_array().expect("foldingRange returns an array");
    let spans: Vec<(u64, u64)> = folds
        .iter()
        .map(|f| {
            (
                f["startLine"].as_u64().expect("startLine"),
                f["endLine"].as_u64().expect("endLine"),
            )
        })
        .collect();
    assert!(spans.contains(&(0, 2)), "front matter: {spans:?}");
    assert!(spans.contains(&(6, 9)), "the code fence: {spans:?}");
    assert!(spans.contains(&(11, 13)), "the ::: div: {spans:?}");
    assert!(
        spans.contains(&(4, 13)),
        "`# One` runs to the end of the document; the `#` inside the cell is a comment, \
         not a heading that would cut it short at line 7: {spans:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Document highlight reaches the editor over the real wire, and distinguishes the anchor's
/// definition (a write) from its references (reads).
///
/// The cursor is put on a REFERENCE, because the answer must not depend on which site it sits
/// on: an implementation that only searched forwards from the cursor would still pass with the
/// cursor on the definition.
#[test]
fn document_highlight_marks_the_anchor_definition_apart_from_its_references() {
    let doc = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/refs.tmd"),
    )
    .expect("corpus fixture");
    let text = std::fs::read_to_string(&doc).expect("read fixture");
    let uri = format!("file://{}", doc.display());
    // Line 15 holds `@fig-results`; put the cursor inside the id.
    let line15 = text.lines().nth(15).expect("line 15");
    let col = line15.find("@fig-results").expect("the reference") + 3;

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentHighlight",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 15, "character": col }
            }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "documentHighlight must be handled, not rejected.\nstdout:\n{stdout}"
    );

    let hits = response(&stdout, 2);
    let hits = hits.as_array().expect("documentHighlight returns an array");
    // `fig-results` is defined on line 11 (`{#fig-results}`) and referenced once, on line 15.
    assert_eq!(hits.len(), 2, "definition + one reference: {hits:?}");
    // DocumentHighlightKind on the wire is Text=1, Read=2, Write=3.
    assert_eq!(
        hits[0]["range"]["start"]["line"], 11,
        "the definition: {hits:?}"
    );
    assert_eq!(
        hits[0]["kind"], 3,
        "the definition is the write site: {hits:?}"
    );
    assert_eq!(
        hits[1]["range"]["start"]["line"], 15,
        "the reference: {hits:?}"
    );
    assert_eq!(hits[1]["kind"], 2, "a reference is a read: {hits:?}");
}

/// `textDocument/references` and `textDocument/selectionRange` reach the editor over the real
/// wire.
///
/// Both answered `-32601` when the 2026-08-07 devtooling audit probed the release binary over
/// stdio, and that probe is what this test replaces: an in-process test of the resolver would
/// still pass with the dispatch arm deleted, because the *dispatch* is the half that was
/// missing. Asserting a value (not merely "no MethodNotFound") is the other half — an arm that
/// answered `null` to everything would satisfy the negative check alone.
#[test]
fn references_and_selection_range_answer_over_the_wire() {
    let doc = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/refs.tmd"),
    )
    .expect("corpus fixture");
    let text = std::fs::read_to_string(&doc).expect("read fixture");
    let uri = format!("file://{}", doc.display());
    let line15 = text.lines().nth(15).expect("line 15");
    let col = line15.find("@fig-results").expect("the reference") + 3;

    let input = format!(
        "{}{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 15, "character": col },
                "context": { "includeDeclaration": true }
            }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "textDocument/selectionRange",
            "params": {
                "textDocument": { "uri": uri },
                "positions": [{ "line": 15, "character": col }]
            }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "both methods must be handled, not rejected.\nstdout:\n{stdout}"
    );

    let refs = response(&stdout, 2);
    let refs = refs.as_array().expect("references returns an array");
    // `fig-results` is defined on line 11 and referenced once, on line 15; the declaration is
    // included, so both are here.
    let lines: Vec<u64> = refs
        .iter()
        .map(|l| l["range"]["start"]["line"].as_u64().expect("a line"))
        .collect();
    assert_eq!(lines, vec![11, 15], "definition + reference: {refs:?}");

    let chains = response(&stdout, 3);
    let chains = chains.as_array().expect("selectionRange returns an array");
    assert_eq!(chains.len(), 1, "one chain per position: {chains:?}");
    // The innermost rung is the id under the cursor, and the chain must actually nest —
    // a flat answer is what a client cannot expand.
    assert_eq!(
        chains[0]["range"]["start"]["line"], 15,
        "the innermost rung is on the cursor's line: {chains:?}"
    );
    assert!(
        chains[0]["parent"].is_object(),
        "the chain must have a parent to expand to: {chains:?}"
    );
}

/// `textDocument/codeLens` reaches the editor over the real wire, with the command a client
/// binds to.
///
/// Answered `-32601` when the 2026-08-07 audit probed the release binary. The value asserted
/// here is the *contract*: a lens is a command name plus arguments, and a client that binds
/// `taliesin.runCell` to `taliesin run <file> --line <L>` gets the execution loop. Changing
/// either the name or the argument shape silently unbinds every editor that did.
#[test]
fn code_lens_offers_the_run_command_over_the_wire() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-lens-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let doc = dir.join("cells.tmd");
    let text = "---\ntitle: T\n---\n\n# A\n\n```{python}\nx = 1\n```\n\n```bash\nls\n```\n";
    std::fs::write(&doc, text).expect("fixture doc");
    let uri = format!("file://{}", doc.display());

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeLens",
            "params": { "textDocument": { "uri": uri } }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    assert!(
        !stdout.contains("MethodNotFound"),
        "codeLens must be handled, not rejected.\nstdout:\n{stdout}"
    );

    let lenses = response(&stdout, 2);
    let lenses = lenses.as_array().expect("codeLens returns an array");
    let run: Vec<&serde_json::Value> = lenses
        .iter()
        .filter(|l| l["command"]["command"] == "taliesin.runCell")
        .collect();
    assert_eq!(
        run.len(),
        1,
        "one Run per executable cell, and the `bash` fence is not one: {lenses:?}"
    );
    // Line 6 (0-based) is the ```{python} fence; the argument is its 1-based line, which is
    // what `run --line` resolves against.
    assert_eq!(run[0]["range"]["start"]["line"], 6, "{run:?}");
    assert_eq!(
        run[0]["command"]["arguments"][1], 7,
        "the 1-based line of the cell's own fence: {run:?}"
    );
    assert_eq!(
        run[0]["command"]["arguments"][0], uri,
        "the document, as a string a client can parse back: {run:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The `--explain` body reaches the hover over the real wire (backlog item 220).
///
/// The catalogue's cause and fix used to be reachable only from a terminal (`check --explain`)
/// or a browser (`code_description`), which is the trip Barik et al. measured people stop
/// making. `hover` is where the pointer already is.
#[test]
fn hover_on_a_squiggle_carries_the_explain_body() {
    let doc = std::fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/typos.tmd"),
    )
    .expect("corpus fixture");
    let text = std::fs::read_to_string(&doc).expect("read fixture");
    let uri = format!("file://{}", doc.display());
    // Line 2 (0-based) is `treme: darkly`, an unknown front-matter key.
    assert!(
        text.lines().nth(2).unwrap_or("").starts_with("treme:"),
        "fixture moved"
    );

    let input = format!(
        "{}{}{}{}{}{}",
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "capabilities": {} }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": uri, "languageId": "taliesin", "version": 1, "text": text
            }}
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 2 }
            }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");
    let hover = response(&stdout, 2);
    let md = hover["contents"]["value"].as_str().unwrap_or("");
    assert!(
        md.contains("TAL-FM-KEY"),
        "the code under the pointer: {hover:?}"
    );
    let fix = taliesin_core::diagnostics::codes::explain("TAL-FM-KEY")
        .expect("the catalogue documents TAL-FM-KEY")
        .fix;
    assert!(
        md.contains(fix),
        "the canonical fix must travel with it, or this is one more restatement of the \
         message the author already read: {md:?}"
    );
}
