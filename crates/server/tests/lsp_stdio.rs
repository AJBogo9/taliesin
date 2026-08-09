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
/// advertise hover, completion, symbols and diagnostics and then answer null to
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
    // Line 4 is a callout's attribute slot; line 7 is a plain (custom-class) div's.
    let text = "---\ntitle: T\n---\n\n::: {.callout-note \n:::\n\n::: {.my-thing \n:::\n";
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
        completion(3, 7, 15), // end of `::: {.my-thing `
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
    for absent in ["ncol", "layout-ncol"] {
        assert!(
            !got.iter().any(|l| l == absent),
            "`{absent}=` is inert on a callout (the callout arm is tested FIRST and never \
             reads it) and must not be offered: {got:?}"
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

    // The other side of the narrowing: a div carrying no callout class falls through to
    // the generic arm, which reads `layout-ncol=` and nothing else.
    let generic = labels(&response(&stdout, 3));
    assert!(
        generic.iter().any(|l| l == "layout-ncol"),
        "`layout-ncol=` reaches a plain div: {generic:?}"
    );
    for absent in ["title", "collapse", "icon", "appearance"] {
        assert!(
            !generic.iter().any(|l| l == absent),
            "`{absent}=` is a callout attribute; the generic arm never reads it, so \
             offering it here would recommend a no-op: {generic:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
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

/// `textDocument/codeLens` reaches the editor over the real wire, saying what a cell will do.
///
/// Answered `-32601` when the 2026-08-07 audit probed the release binary. Until Wave 13 this
/// pinned a ▶ Run Cell **command**; that verb is gone, so what a lens carries now is a label
/// with no command, and the contract is where it sits and what it says. Driven with a
/// `#| cache: false` cell because a fresh cacheable cell is deliberately silent: without it
/// the provider correctly returns nothing and the test would pass while proving nothing.
#[test]
fn code_lens_labels_a_cell_over_the_wire() {
    let dir = std::env::temp_dir().join(format!("tali-lsp-lens-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let doc = dir.join("cells.tmd");
    let text = "---\ntitle: T\n---\n\n# A\n\n```{python}\n#| cache: false\nx = 1\n```\n\n```bash\nls\n```\n";
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
    assert_eq!(
        lenses.len(),
        1,
        "one label per executable cell, and the `bash` fence is not one: {lenses:?}"
    );
    // Line 6 (0-based) is the ```{python} fence: the label hangs on the cell it describes.
    assert_eq!(lenses[0]["range"]["start"]["line"], 6, "{lenses:?}");
    assert_eq!(
        lenses[0]["command"]["command"], "",
        "a label is not a button: naming a command here would give a client something to \
         invoke that nothing implements: {lenses:?}"
    );
    assert!(
        lenses[0]["command"]["title"]
            .as_str()
            .is_some_and(|t| t.contains("always re-runs")),
        "the label must say what this cell does next run: {lenses:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A diagnostic reaches the hover over the real wire (backlog item 220).
///
/// The message used to be joined by a catalogued cause + fix for the diagnostic's `TAL-*`
/// code; that catalogue went on 2026-08-08 and the message is the whole answer now, so what
/// this pins is that the squiggle's own text arrives at the pointer at all. That is the half
/// worth a wire test: the hover reads `published`, not a fresh lint, so a regression in the
/// publish path shows up here and nowhere else.
#[test]
fn hover_on_a_squiggle_carries_the_diagnostic_message() {
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
        md.contains("unknown front-matter key `treme`"),
        "the diagnostic under the pointer: {hover:?}"
    );
    assert!(
        md.contains("did you mean `theme`"),
        "the fix travels with it, inline in the message, which is the whole reason the \
         separate catalogue could go: {md:?}"
    );
}

/// `taliesin/siteMap` answers where each page of a project is served, and
/// `taliesin/mathCommands` answers with the symbol picker's table.
///
/// Both are Wave 2 re-homings: the companion used to spawn `taliesin map --format json` and
/// `taliesin vocab` for exactly these two answers, and both verbs went with the
/// machine-facing cut. The capabilities did not — one decides which chapter the preview
/// opens at, the other fills the Insert Math Symbol quick-pick — so they moved onto the wire
/// the companion already holds open. This is what is left of `map_cli.rs`'s coverage, kept
/// because the answer is load-bearing for the preview and TypeScript must never re-derive it.
///
/// `corpus/demo-book` pins both halves in one list: `chapters:` fixes the order (the
/// `.tmd`→`.html` mapping and book numbering that live in Rust), and `appendix.tmd` is
/// `draft: true`, so a map that leaked it would open the preview at a page no build writes.
#[test]
fn site_map_and_math_commands_answer_over_the_wire() {
    let root = std::fs::canonicalize(format!(
        "{}/../../corpus/demo-book",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the demo-book fixture exists");
    let uri = format!("file://{}", root.display());
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
            "jsonrpc": "2.0", "id": 2, "method": "taliesin/siteMap",
            "params": { "uri": uri }
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "taliesin/mathCommands", "params": null
        })),
        frame(serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null
        })),
        frame(serde_json::json!({ "jsonrpc": "2.0", "method": "exit", "params": null })),
    );
    let (code, stdout, stderr) = lsp_session(&input);
    assert_eq!(code, Some(0), "stderr:\n{stderr}");

    let map = response(&stdout, 2);
    let pages = map["pages"].as_array().expect("a pages array: {map}");
    let urls: Vec<&str> = pages.iter().filter_map(|p| p["url"].as_str()).collect();
    assert_eq!(
        urls,
        [
            "index.html",
            "intro.html",
            "methods.html",
            "results.html",
            "summary.html"
        ],
        "pages follow `chapters:`, and the `draft: true` appendix is not one: {map}"
    );
    // The companion looks its own document up by `rel`, so the pairing is the whole answer.
    assert_eq!(
        pages
            .iter()
            .find(|p| p["rel"] == "methods.tmd")
            .map(|p| p["url"].clone()),
        Some(serde_json::json!("methods.html")),
        "a chapter's source path must resolve to its url: {map}"
    );

    let math = response(&stdout, 3);
    let commands = math.as_array().expect("a mathCommands array");
    assert!(
        commands
            .iter()
            .any(|c| c["name"] == "\\frac"
                || c["snippet"].as_str().is_some_and(|s| s.contains("frac"))),
        "the picker's table came back empty or unrecognizable: {math}"
    );
}
