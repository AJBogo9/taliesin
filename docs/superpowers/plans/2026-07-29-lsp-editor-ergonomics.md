# LSP Editor Ergonomics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `.tmd` authors four new LSP capabilities (inlay hints, folding, document highlight, selection ranges) plus visible math delimiters, without adding any TypeScript language logic.

**Architecture:** Everything lands in `crates/server/src/lsp*.rs` and reaches every LSP editor, not just VS Code. The one exception is Task 1, a `package.json` theme contribution. Tasks 2 and 3 are shared substrate (a render memo and a `didChange` debounce) that Task 4 onward depend on for responsiveness.

**Tech Stack:** Rust edition 2024, `lsp-server` 0.7, `lsp-types` 0.95.1, `taliesin-core` for rendering. VS Code companion is a `vscode-languageclient` thin client.

**Spec:** `docs/superpowers/specs/2026-07-29-lsp-editor-ergonomics-design.md`. Read it first. Backlog items 177 and 178.

## Global Constraints

- **`stdout` is the JSON-RPC wire.** Never `println!` from LSP code. Use `crate::log` (stderr).
- **Positions on the wire are UTF-16; the scanners use scalar (`char`) offsets.** Convert at the boundary with `crate::lsp_pos::{char_to_utf16, utf16_to_char, nth_line}`, exactly as `resolve_definition` does. Getting this wrong breaks astral characters only, so it will pass a careless test.
- **A provider returns an empty result on a malformed buffer, never an error.** A half-typed document is the normal case for anything firing on every edit.
- **A missing include, bib entry or anchor is an absent hint, not a diagnostic.** Diagnostics already report the genuinely wrong ones; a second report is a double report.
- **`crate::serve::guarded` already wraps `f` in `AssertUnwindSafe`**, so capturing `&mut` state in a request closure is sound. No extra wrapping needed.
- **A shared working tree.** A parallel session holds uncommitted work under `crates/core/` and `corpus/reactive/`. **Stage by explicit pathspec on every commit. Never `git add -A`, never `git add .`, never `git checkout --`, never `git restore`.**
- **Do not branch.** `git checkout -b` moves HEAD for the whole tree and would disrupt the parallel session. Commit to the current branch by pathspec.
- **Rust formatting is automatic.** A `PostToolUse` hook runs `rustfmt` on every edited `.rs`.
- **Verify each fix by mutation.** After a test passes, reintroduce the bug by inverse edit, watch the *named* test fail, then undo the inverse edit. Never `git checkout` a file to undo a mutation.
- **No new corpus document.** The walker renders every corpus doc on every `cargo test`. Reuse `corpus/diagnostics/refs.tmd`.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `editor/vscode/package.json` | Math delimiter token-colour default | 1 |
| `editor/vscode/src/test/manifest.test.ts` | Pins that contribution | 1 |
| `crates/server/src/lsp_memo.rs` (new) | `RenderMemo`: text-keyed render cache | 2 |
| `crates/server/src/lsp.rs` | Capabilities, dispatch, debounce, handlers | 2-8 |
| `crates/server/src/lsp_hints.rs` (new) | Inlay hint construction | 4, 5 |
| `crates/server/src/lsp_fold.rs` (new) | Folding range construction | 6 |
| `crates/server/src/lsp_nav.rs` | Targeted id scan for highlight; span chain for selection | 7, 8 |
| `crates/server/tests/lsp_stdio.rs` | Wire-level capability tests | 4, 6, 7, 8 |

New modules are small and single-purpose because `lsp.rs` is already 3,580 lines and is the dispatch layer, not a feature home. Follow the existing convention: pure functions over text in a submodule, I/O and dispatch in `lsp.rs`.

---

### Task 1: Math delimiter visibility

**Files:**
- Modify: `editor/vscode/package.json` (the `contributes.configurationDefaults` object)
- Test: `editor/vscode/src/test/manifest.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks. Fully independent.

**Background:** the grammar already scopes delimiters as `punctuation.definition.math.begin.tmd` / `.end.tmd`. No bundled VS Code theme defines a rule for that scope, which is why they render as plain text. One narrowly-scoped default fixes it. The `.tmd` suffix is Taliesin's own, so a window-level setting cannot leak into Markdown.

- [ ] **Step 1: Write the failing test**

Append to `editor/vscode/src/test/manifest.test.ts`:

```typescript
test("contributes a math-delimiter token colour scoped to .tmd only", () => {
  const defaults = pkg.contributes.configurationDefaults;
  const rules = defaults["editor.tokenColorCustomizations"]?.textMateRules;
  assert.ok(Array.isArray(rules), "expected textMateRules in configurationDefaults");

  const scopes = rules.flatMap((r: { scope: string | string[] }) =>
    typeof r.scope === "string" ? [r.scope] : r.scope
  );
  assert.deepStrictEqual(scopes.slice().sort(), [
    "punctuation.definition.math.begin.tmd",
    "punctuation.definition.math.end.tmd",
  ]);

  // The `.tmd` suffix is what keeps a window-level setting from restyling Markdown.
  for (const s of scopes) {
    assert.ok(s.endsWith(".tmd"), `scope must be .tmd-suffixed, got ${s}`);
  }
  // No foreground: the rule inherits the active theme's colour, so it is legible in
  // light, dark and both high-contrast themes without Taliesin picking one.
  for (const r of rules) {
    assert.strictEqual(r.settings.foreground, undefined, "must not hardcode a colour");
    assert.strictEqual(r.settings.fontStyle, "bold");
  }
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd editor/vscode && npm test -- --grep "math-delimiter"`
Expected: FAIL, "expected textMateRules in configurationDefaults"

- [ ] **Step 3: Write minimal implementation**

In `editor/vscode/package.json`, replace the `configurationDefaults` value with:

```json
"configurationDefaults": {
  "[taliesin]": {
    "editor.wordWrap": "on",
    "editor.quickSuggestions": {
      "other": true,
      "comments": false,
      "strings": true
    }
  },
  "editor.tokenColorCustomizations": {
    "textMateRules": [
      {
        "scope": [
          "punctuation.definition.math.begin.tmd",
          "punctuation.definition.math.end.tmd"
        ],
        "settings": { "fontStyle": "bold" }
      }
    ]
  }
}
```

`editor.tokenColorCustomizations` is window-level and cannot nest inside `"[taliesin]"`; the `.tmd` scope suffix is what scopes it in practice.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd editor/vscode && npm test -- --grep "math-delimiter"`
Expected: PASS

- [ ] **Step 5: Verify in the editor (this cannot be unit-tested)**

Open `corpus/diagnostics/refs.tmd` (or any doc with `$x$`) in a VS Code window running the companion. Confirm the `$` delimiters render bold while the LaTeX body keeps its existing colours. Record what you saw. If bold is too subtle, the escalation named in the spec is a foreground from the project's OKLCH accent, **not** a vendor hex, and this step re-runs.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/package.json editor/vscode/src/test/manifest.test.ts
git commit -m "editor: make math delimiters visible in themes that define no math rule"
```

---

### Task 2: RenderMemo (item 178, half one)

**Files:**
- Create: `crates/server/src/lsp_memo.rs`
- Modify: `crates/server/src/lsp.rs` (add `mod` declaration is in `main.rs`; see step 3)
- Modify: `crates/server/src/main.rs` (module declaration)

**Interfaces:**
- Consumes: `taliesin_core::RenderedDoc`, `crate::lsp::render_buffer`.
- Produces:
  - `pub(crate) struct RenderMemo` with `RenderMemo::default()`
  - `pub(crate) fn RenderMemo::get(&mut self, uri: &lsp_types::Url, text: &str) -> Option<std::sync::Arc<taliesin_core::RenderedDoc>>`

**Background:** `render_buffer` (`lsp.rs:1311`) re-runs the full `render_single_doc` on every call. Keying the cache on `(uri, text)` means a different buffer is a different key, so there is no invalidation logic and no staleness class. The `uri` is part of the key because `render_buffer` derives the render base directory from it, so identical text in two directories is genuinely two different renders.

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/lsp_memo.rs` containing only the test module for now:

```rust
//! A text-keyed memo of the last rendered buffer.

#[cfg(test)]
mod tests {
    use super::RenderMemo;
    use lsp_types::Url;

    fn uri(name: &str) -> Url {
        Url::parse(&format!("file:///tmp/{name}")).unwrap()
    }

    #[test]
    fn repeated_identical_text_reuses_one_render() {
        let mut memo = RenderMemo::default();
        let u = uri("a.tmd");
        let a = memo.get(&u, "# Hi\n").unwrap();
        let b = memo.get(&u, "# Hi\n").unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "identical text must return the very same render, not an equal one"
        );
    }

    #[test]
    fn changed_text_renders_again() {
        let mut memo = RenderMemo::default();
        let u = uri("a.tmd");
        let a = memo.get(&u, "# Hi\n").unwrap();
        let b = memo.get(&u, "# Bye\n").unwrap();
        assert!(!std::sync::Arc::ptr_eq(&a, &b), "changed text must re-render");
    }

    #[test]
    fn same_text_in_a_different_file_renders_again() {
        // The render base directory comes from the URI, so the URI is part of the key.
        // Keying on text alone would serve one directory's render for another's buffer.
        let mut memo = RenderMemo::default();
        let a = memo.get(&uri("a.tmd"), "# Hi\n").unwrap();
        let b = memo.get(&uri("sub/b.tmd"), "# Hi\n").unwrap();
        assert!(!std::sync::Arc::ptr_eq(&a, &b), "a different URI must re-render");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin lsp_memo`
Expected: FAIL to compile, "cannot find type `RenderMemo`"

- [ ] **Step 3: Write minimal implementation**

Add to `crates/server/src/main.rs` beside the other `mod lsp*` declarations:

```rust
mod lsp_memo;
```

Prepend to `crates/server/src/lsp_memo.rs`, above the test module:

```rust
use std::sync::Arc;

/// A one-entry memo of the last rendered buffer, keyed on `(uri, text)`.
///
/// Keying on the text itself is the whole design: a different buffer is a different
/// key, so there is no invalidation to write and no staleness class to get wrong. The
/// URI is part of the key because [`crate::lsp::render_buffer`] derives the render base
/// directory from it, so the same text in two directories is two different renders.
///
/// One entry, not an LRU: the access pattern is many reads of the buffer the author is
/// currently typing in, and a second entry would only help when two documents are being
/// edited in strict alternation.
#[derive(Default)]
pub(crate) struct RenderMemo {
    last: Option<(lsp_types::Url, String, Arc<taliesin_core::RenderedDoc>)>,
}

impl RenderMemo {
    /// The render for `text` at `uri`, from cache when the key is unchanged. `None` when
    /// the buffer cannot be rendered (`render_buffer` is panic-guarded and returns `None`).
    pub(crate) fn get(
        &mut self,
        uri: &lsp_types::Url,
        text: &str,
    ) -> Option<Arc<taliesin_core::RenderedDoc>> {
        if let Some((u, t, doc)) = &self.last
            && u == uri
            && t == text
        {
            return Some(doc.clone());
        }
        let doc = Arc::new(crate::lsp::render_buffer(uri, text)?);
        self.last = Some((uri.clone(), text.to_owned(), doc.clone()));
        Some(doc)
    }
}
```

Change `render_buffer` in `crates/server/src/lsp.rs:1311` from `fn` to `pub(crate) fn` so the memo can call it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin lsp_memo`
Expected: PASS, 3 tests

- [ ] **Step 5: Verify by mutation**

Delete the `&& t == text` clause from the cache-hit guard. Run the tests. `changed_text_renders_again` must FAIL. Restore the clause by inverse edit (retype it), do not `git checkout`.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_memo.rs crates/server/src/main.rs crates/server/src/lsp.rs
git commit -m "lsp: memoize the buffer render, keyed on (uri, text)"
```

---

### Task 3: Debounce didChange (item 178, half two)

**Files:**
- Modify: `crates/server/src/lsp.rs` (`main_loop` at line 133, the `DidChangeTextDocument` arm at 273-283)

**Interfaces:**
- Consumes: `RenderMemo` from Task 2.
- Produces: `main_loop` gains a `debounce: std::time::Duration` parameter so tests can pass a short interval. `run()` passes `DEFAULT_DEBOUNCE`.

**Background:** `didChange` currently calls `publish` synchronously with no coalescing, and `publish` runs a full render **plus** `site::anchors_defined_elsewhere_in_project`, which walks and reads every page in the project. This is the riskiest change in the plan because it restructures the main loop's blocking receive.

**Design:** block on `recv()` when nothing is pending; block on `recv_timeout(debounce)` when a publish is pending. That avoids spinning on an idle server.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/server/src/lsp.rs`:

```rust
#[test]
fn rapid_edits_coalesce_into_one_publish_of_the_final_text() {
    let (client, server) = Connection::memory();
    let handle = std::thread::spawn(move || {
        let _ = super::main_loop(&server, Duration::from_millis(40));
    });
    handshake(&client);

    let uri = Url::parse("file:///tmp/debounce.tmd").unwrap();
    did_open(&client, &uri, "---\ntittle: a\n---\n".to_owned());
    let _ = recv_publish(&client); // the didOpen publish is not debounced

    // Five edits inside one debounce window. Only the last text may be reported on.
    for n in 0..5 {
        client
            .sender
            .send(Message::Notification(Notification {
                method: DidChangeTextDocument::METHOD.to_owned(),
                params: serde_json::json!({
                    "textDocument": { "uri": uri, "version": n + 2 },
                    "contentChanges": [{ "text": format!("---\ntittle{n}: a\n---\n") }],
                }),
            }))
            .unwrap();
    }

    let published = recv_publish(&client);
    assert_eq!(published.uri, uri);
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.message.contains("tittle4")),
        "the coalesced publish must describe the LAST edit, got: {:?}",
        published.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // Nothing further arrives: the other four edits were dropped, not queued.
    assert!(
        client.receiver.recv_timeout(Duration::from_millis(400)).is_err(),
        "five edits in one window must produce exactly one publish"
    );

    shutdown(&client);
    handle.join().unwrap();
}
```

If a `shutdown` helper does not already exist in the test module, send a `Shutdown` request followed by an `exit` notification, matching whatever the neighbouring tests do.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin rapid_edits_coalesce -- --test-threads=1`
Expected: FAIL to compile, `main_loop` takes 1 argument not 2

- [ ] **Step 3: Write minimal implementation**

In `crates/server/src/lsp.rs`, add near the top:

```rust
/// How long `didChange` edits are coalesced before diagnostics are published.
///
/// `publish` runs a full render **plus** `site::anchors_defined_elsewhere_in_project`,
/// which walks and reads every page in the project, so an undebounced keystroke costs a
/// whole-book pass. 120 ms is below the threshold at which an author notices a lag in
/// squiggles and well above a fast typist's inter-key interval.
const DEFAULT_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(120);
```

Change the signature and receive loop:

```rust
fn main_loop(
    connection: &Connection,
    debounce: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut docs: std::collections::HashMap<lsp_types::Url, String> =
        std::collections::HashMap::new();
    let mut memo = crate::lsp_memo::RenderMemo::default();
    // The document whose diagnostics are owed but not yet published. At most one: a
    // second edit to the same buffer supersedes the first, which is the whole point.
    let mut pending: Option<lsp_types::Url> = None;

    loop {
        let msg = if pending.is_some() {
            match connection.receiver.recv_timeout(debounce) {
                Ok(m) => m,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // The window closed with no further edit: publish the latest text.
                    if let Some(uri) = pending.take()
                        && let Some(text) = docs.get(&uri)
                    {
                        publish(connection, &uri, text)?;
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            // Idle: block, so an untouched server costs nothing.
            match connection.receiver.recv() {
                Ok(m) => m,
                Err(_) => break,
            }
        };

        // ... existing `match msg { ... }` body, unchanged except as below ...
    }
    Ok(())
}
```

Replace the body of the `DidChangeTextDocument` arm (currently `lsp.rs:273-283`) so it records instead of publishing:

```rust
} else if method == DidChangeTextDocument::METHOD {
    let mut p: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
    let uri = p.text_document.uri;
    if docs.contains_key(&uri)
        && let Some(change) = p.content_changes.pop()
    {
        docs.insert(uri.clone(), change.text);
        // Coalesced: the timeout arm above publishes once the edits stop. Publishing
        // here re-walked the whole project on every keystroke (item 178).
        *pending = Some(uri);
    }
}
```

`handle_notification` needs `pending: &mut Option<Url>` threaded in; `didClose` must clear it when it names the closed document, otherwise a close-then-timeout publishes diagnostics for a buffer that is gone.

Update `run()` to call `main_loop(connection, DEFAULT_DEBOUNCE)`, and thread `&mut memo` into `handle_request` (its signature gains `memo: &mut crate::lsp_memo::RenderMemo`); `guarded` already asserts unwind safety so the closure capture is sound.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin rapid_edits_coalesce -- --test-threads=1`
Expected: PASS

Then the full LSP suite, which has the most regression risk in this task:
Run: `cargo test -p taliesin-server --bin taliesin lsp -- --test-threads=1`
Expected: PASS, no test left behind

- [ ] **Step 5: Measure, and decide the open question with the number**

The spec leaves one question open: whether the anchor walk also needs memoizing, or whether
debouncing alone makes it cheap enough. **Answer it with a measurement, not a guess.**

Time one `publish` on the largest book in the tree, before and after this task:

```rust
// A temporary #[test] in lsp.rs, deleted before committing. Not a benchmark harness:
// one number on one real book is enough to decide between "debounce is sufficient" and
// "the anchor scan also needs a memo".
#[test]
#[ignore = "measurement, run by hand"]
fn measure_publish_cost() {
    let path = std::path::Path::new("../../docs/guide/using/formats.tmd");
    let text = std::fs::read_to_string(path).unwrap();
    let t = std::time::Instant::now();
    let _ = crate::check::buffer_diagnostics(path, &text);
    eprintln!("buffer_diagnostics: {:?}", t.elapsed());
}
```

Run: `cargo test -p taliesin-server --bin taliesin measure_publish_cost -- --ignored --nocapture`

Record the number in the commit message. **If a single `publish` is comfortably under the
120 ms debounce window, stop here**: debouncing alone has solved item 178 and an anchor-scan
memo would be unjustified complexity. If it is not, open a follow-up rather than growing this
task. Delete the temporary test before committing.

- [ ] **Step 6: Verify by mutation**

Change `pending.is_some()` to `false` so the loop always blocks on `recv()`. The coalescing test must FAIL (it will hang until its own timeout and report no publish). Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp.rs
git commit -m "lsp: coalesce didChange so a keystroke stops re-walking the whole book

One publish on <book> measured at <N> ms, against a 120 ms debounce window."
```

---

### Task 4: Inlay hints for cross-reference numbers

**Files:**
- Create: `crates/server/src/lsp_hints.rs`
- Modify: `crates/server/src/main.rs` (module declaration), `crates/server/src/lsp.rs` (capability + dispatch)
- Test: inline in `lsp_hints.rs`, plus `crates/server/tests/lsp_stdio.rs`

**Interfaces:**
- Consumes: `RenderMemo::get` (Task 2), `taliesin_core::RenderedDoc::xref_numbers`, `crate::lsp_pos::char_to_utf16`.
- Produces: `pub(crate) fn inlay_hints(text: &str, doc: &taliesin_core::RenderedDoc, range: lsp_types::Range) -> Vec<lsp_types::InlayHint>`

**Background:** `RenderedDoc.xref_numbers` (`crates/core/src/render/model.rs:266`) is a `HashMap<String, String>` from anchor id to rendered number, already populated and already read by hover. `textDocument/inlayHint` is range-scoped, so only the visible lines need scanning and no full-document tokenizer is required.

`xref_numbers` is page-local: a reference to an anchor on another page has no entry. **Omit the hint in that case** rather than rendering a placeholder, because a missing hint reads as "no information" while `⟨elsewhere⟩` reads as a claim.

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/lsp_hints.rs`:

```rust
//! Inlay hints: the resolved number beside a cross-reference.

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range};

    fn render(text: &str) -> taliesin_core::RenderedDoc {
        taliesin_core::render_single_doc(text, std::path::Path::new("."))
    }

    const DOC: &str = "\
# Results {#sec-results}

![A curve](a.png){#fig-results}

See @fig-results and @sec-results and @fig-nowhere.
";

    fn hints_on_last_line(text: &str) -> Vec<String> {
        let doc = render(text);
        let last = text.lines().count() as u32 - 1;
        super::inlay_hints(text, &doc, Range::new(Position::new(0, 0), Position::new(last, 0)))
            .into_iter()
            .map(|h| match h.label {
                lsp_types::InlayHintLabel::String(s) => s,
                other => panic!("expected a string label, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_resolving_xref_gets_its_number() {
        let labels = hints_on_last_line(DOC);
        assert!(
            labels.iter().any(|l| l.contains('1')),
            "expected a number hint for @fig-results, got {labels:?}"
        );
    }

    #[test]
    fn an_unresolvable_xref_gets_no_hint() {
        let labels = hints_on_last_line(DOC);
        assert!(
            !labels.iter().any(|l| l.contains("nowhere")),
            "an anchor this document does not define must produce no hint, got {labels:?}"
        );
    }

    #[test]
    fn hints_outside_the_requested_range_are_not_returned() {
        let doc = render(DOC);
        // Range covering only line 0, which holds no reference.
        let hints = super::inlay_hints(
            DOC,
            &doc,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
        );
        assert!(hints.is_empty(), "expected no hints outside the range, got {hints:?}");
    }

    #[test]
    fn a_malformed_buffer_yields_no_hints_and_does_not_panic() {
        // Unterminated fenced div and unterminated display math: the normal half-typed case.
        let text = "::: {.callout}\n$$\n\\frac{1}{";
        let doc = render(text);
        let _ = super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(2, 0)),
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin lsp_hints`
Expected: FAIL to compile, "cannot find function `inlay_hints`"

- [ ] **Step 3: Write minimal implementation**

Add `mod lsp_hints;` to `crates/server/src/main.rs`. Prepend to `lsp_hints.rs`:

```rust
use lsp_types::{InlayHint, InlayHintLabel, Position, Range};

/// One inlay hint per cross-reference in `range` whose anchor this document numbers.
///
/// `xref_numbers` is page-local, so a reference to an anchor defined in another chapter
/// has no entry here. Such a reference is *valid* (the diagnostic path knows the project's
/// anchors) but unnumbered, and we omit the hint rather than render a placeholder: a
/// missing hint reads as "no information", `⟨elsewhere⟩` reads as a claim.
pub(crate) fn inlay_hints(
    text: &str,
    doc: &taliesin_core::RenderedDoc,
    range: Range,
) -> Vec<InlayHint> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line_no = line_no as u32;
        if line_no < range.start.line || line_no > range.end.line {
            continue;
        }
        for (id, end_char) in xrefs_on_line(line) {
            let Some(number) = doc.xref_numbers.get(&id) else {
                continue;
            };
            out.push(InlayHint {
                // The wire wants UTF-16; the scan works in scalar offsets.
                position: Position::new(
                    line_no,
                    crate::lsp_pos::char_to_utf16(line, end_char) as u32,
                ),
                label: InlayHintLabel::String(format!(" ⟨{number}⟩")),
                kind: None,
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }
    out
}

/// Every `@anchor-id` on one line, with the scalar offset just past the id. Reuses
/// `lsp_nav`'s cursor classifier rather than adding a second scanner: walking the line and
/// asking "what is here" keeps one definition of what an xref is.
fn xrefs_on_line(line: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for col in 0..line.chars().count() {
        if let crate::lsp_nav::Target::Xref { id, end, .. } =
            crate::lsp_nav::classify_target(line, 0, col)
            && out.last().map(|(prev, _)| prev != &id).unwrap_or(true)
        {
            out.push((id, end));
        }
    }
    out
}
```

`lsp_nav::Target` and `classify_target` are `pub(crate)`; if `Target`'s variants are not visible outside the module, widen them to `pub(crate)` in `lsp_nav.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin lsp_hints`
Expected: PASS, 4 tests

- [ ] **Step 5: Advertise the capability and dispatch it**

In `server_capabilities()` in `lsp.rs`, beside `document_symbol_provider`:

```rust
// Range-scoped, so only the visible lines are scanned and there is no
// full-document tokenizer behind this.
inlay_hint_provider: Some(OneOf::Left(true)),
```

In `handle_request`, add to the `use lsp_types::request::{...}` list `InlayHintRequest`, and a dispatch arm following the existing shape:

```rust
} else if req.method == InlayHintRequest::METHOD {
    let params: lsp_types::InlayHintParams = serde_json::from_value(req.params)?;
    let hints = docs
        .get(&params.text_document.uri)
        .and_then(|text| {
            memo.get(&params.text_document.uri, text)
                .map(|doc| crate::lsp_hints::inlay_hints(text, &doc, params.range))
        })
        .unwrap_or_default();
    lsp_server::Response {
        id: req.id,
        result: Some(serde_json::to_value(hints)?),
        error: None,
    }
}
```

Extend the existing capability assertion test in `lsp.rs` (the one whose comment says a dropped field "is its own silent feature loss"):

```rust
assert_eq!(caps["inlayHintProvider"], true);
```

- [ ] **Step 6: Add the wire-level test**

Append to `crates/server/tests/lsp_stdio.rs`, following that file's existing helper style, a test that opens `corpus/diagnostics/refs.tmd` over stdio, sends a `textDocument/inlayHint` request covering the whole file, and asserts the response contains a hint whose label mentions the number for `fig-results`. Its purpose is to prove the capability is reachable over the real wire, which a unit test cannot.

- [ ] **Step 7: Run the full suite**

Run: `cargo test -p taliesin-server -- --test-threads=1`
Expected: PASS. **Grep the output for `FAILED`**: the totals line has been observed to mislead.

- [ ] **Step 8: Verify by mutation**

Delete the `let Some(number) = ... else { continue; }` guard and unwrap instead. `an_unresolvable_xref_gets_no_hint` must FAIL or panic. Restore by inverse edit.

- [ ] **Step 9: Commit**

```bash
git add crates/server/src/lsp_hints.rs crates/server/src/main.rs \
        crates/server/src/lsp.rs crates/server/src/lsp_nav.rs \
        crates/server/tests/lsp_stdio.rs
git commit -m "lsp: inlay hints showing the resolved number beside a cross-reference"
```

---

### Task 5: Inlay hints for citations and includes

**Files:**
- Modify: `crates/server/src/lsp_hints.rs`, `crates/server/src/lsp.rs`

**Interfaces:**
- Consumes: `inlay_hints` from Task 4; `crate::lsp_nav::frontmatter_bib_paths`, `crate::lsp_nav::bib_entry_text`.
- Produces: `inlay_hints` gains a `dir: Option<&std::path::Path>` parameter for bib and include resolution. Task 4's call site in `lsp.rs` passes the document's parent directory.

**Background:** `resolve_definition` already reads front-matter `.bib` files relative to the document directory (`lsp.rs:482`), and `lsp_nav::bib_entry_text` already extracts an entry. Author-year comes from that entry's `author` and `year` fields.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `lsp_hints.rs`:

**`tempfile` is not a dependency of `taliesin-server`.** The house pattern for a test that
needs files on disk is a manually managed directory under `std::env::temp_dir()`, keyed on the
process id (see `lsp.rs:2478`). Follow it:

```rust
/// A scratch directory holding a `refs.bib`, following the pattern the existing
/// cross-file LSP tests use (`lsp.rs:2478`): `tempfile` is not a dependency here.
fn bib_dir(name: &str, bib: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-hints-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("refs.bib"), bib).unwrap();
    dir
}

fn labels_of(hints: Vec<lsp_types::InlayHint>) -> Vec<String> {
    hints
        .into_iter()
        .map(|h| match h.label {
            lsp_types::InlayHintLabel::String(s) => s,
            other => panic!("expected a string label, got {other:?}"),
        })
        .collect()
}

#[test]
fn a_citation_shows_author_and_year() {
    let dir = bib_dir(
        "authoryear",
        "@book{bishop2006pattern,\n  author = {Bishop, Christopher M},\n  year = {2006},\n}\n",
    );
    let text = "---\nbibliography: refs.bib\n---\n\nSee [@bishop2006pattern].\n";
    let doc = render(text);
    let labels = labels_of(super::inlay_hints(
        text,
        &doc,
        Range::new(Position::new(0, 0), Position::new(5, 0)),
        Some(&dir),
    ));
    assert!(
        labels.iter().any(|l| l.contains("Bishop") && l.contains("2006")),
        "expected an author-year hint, got {labels:?}"
    );
}

#[test]
fn a_citation_with_no_bib_entry_gets_no_hint() {
    let dir = bib_dir("nobibentry", "@book{other,\n}\n");
    let text = "---\nbibliography: refs.bib\n---\n\nSee [@nosuchkey].\n";
    let doc = render(text);
    let hints = super::inlay_hints(
        text,
        &doc,
        Range::new(Position::new(0, 0), Position::new(5, 0)),
        Some(&dir),
    );
    assert!(hints.is_empty(), "an absent entry must produce no hint, got {hints:?}");
}
```

Every existing call in this module's tests gains a trailing `None` argument.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin lsp_hints`
Expected: FAIL to compile, `inlay_hints` takes 3 arguments not 4

- [ ] **Step 3: Write minimal implementation**

Add the `dir` parameter to `inlay_hints` and, inside the per-line loop, a second pass over citations:

```rust
for (key, end_char) in cites_on_line(line) {
    let Some(dir) = dir else { continue };
    let Some(label) = author_year(text, dir, &key) else {
        continue;
    };
    out.push(InlayHint {
        position: Position::new(line_no, crate::lsp_pos::char_to_utf16(line, end_char) as u32),
        label: InlayHintLabel::String(format!(" ⟨{label}⟩")),
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    });
}
```

with `cites_on_line` mirroring `xrefs_on_line` against `Target::Cite`, and:

```rust
/// "Bishop 2006" for a key defined in one of the front matter's `.bib` files, or `None`
/// when nothing defines it. An absent entry is not an error here: the diagnostic pass
/// already reports an unresolvable citation, and a second report is a double report.
fn author_year(text: &str, dir: &std::path::Path, key: &str) -> Option<String> {
    for rel in crate::lsp_nav::frontmatter_bib_paths(text) {
        let Ok(bib) = std::fs::read_to_string(dir.join(&rel)) else {
            continue;
        };
        if let Some(entry) = crate::lsp_nav::bib_entry_text(&bib, key) {
            let surname = bib_field(&entry, "author")
                .and_then(|a| a.split(&[',', ' '][..]).next().map(str::to_owned))?;
            let year = bib_field(&entry, "year")?;
            return Some(format!("{surname} {year}"));
        }
    }
    None
}

/// The value of `name = {...}` in one BibTeX entry body.
fn bib_field(entry: &str, name: &str) -> Option<String> {
    let at = entry.find(name)?;
    let open = entry[at..].find('{')? + at;
    let close = entry[open + 1..].find('}')? + open + 1;
    Some(entry[open + 1..close].trim().to_owned())
}
```

If `bib_entry_text`'s real signature differs from `(bib: &str, key: &str) -> Option<String>`, adapt the call; `lsp_nav.rs:920` shows a `bib_entry_site` and a `bib_entry_text` in its tests, so confirm which returns text before writing this.

In `lsp.rs`, the dispatch arm passes the directory:

```rust
let dir = params.text_document.uri.to_file_path().ok();
let dir = dir.as_deref().and_then(std::path::Path::parent);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin lsp_hints`
Expected: PASS, 6 tests

- [ ] **Step 5: Add the include hint**

Same shape, against `Target::Include`: read the resolved path, count its lines, label `⟨N lines⟩`. Omit when the file does not exist. Add one test asserting a hint for a real temp file and none for a missing path.

- [ ] **Step 6: Verify by mutation**

Make `author_year` return `Some("x".into())` when the entry is absent. `a_citation_with_no_bib_entry_gets_no_hint` must FAIL. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp_hints.rs crates/server/src/lsp.rs
git commit -m "lsp: inlay hints for citation author-year and include line counts"
```

---

### Task 6: Folding ranges

**Files:**
- Create: `crates/server/src/lsp_fold.rs`
- Modify: `crates/server/src/main.rs`, `crates/server/src/lsp.rs`
- Test: inline, plus `crates/server/tests/lsp_stdio.rs`

**Interfaces:**
- Consumes: the existing document-symbol section walk in `lsp_outline.rs`.
- Produces: `pub(crate) fn folding_ranges(text: &str) -> Vec<lsp_types::FoldingRange>`

**Background:** folding is currently indentation-based, which is wrong for a Markdown-derived format: there is no `folding` key in `editor/vscode/language-configuration.json` and no server capability. Sections are a re-projection of ranges `documentSymbol` already computes (`lsp.rs:1453`: `range` is the whole section).

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/lsp_fold.rs` with a test module:

```rust
//! Folding ranges: sections, fenced divs, front matter, code fences.

#[cfg(test)]
mod tests {
    fn lines_of(text: &str, kind: Option<lsp_types::FoldingRangeKind>) -> Vec<(u32, u32)> {
        super::folding_ranges(text)
            .into_iter()
            .filter(|r| r.kind == kind)
            .map(|r| (r.start_line, r.end_line))
            .collect()
    }

    const DOC: &str = "\
---
title: T
---

# One

text

## Two

more

::: {.callout-note}
inside
:::
";

    #[test]
    fn front_matter_folds() {
        // Lines 0..2 inclusive of the closing `---`.
        assert!(
            lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region)).contains(&(0, 2)),
            "expected the front matter to fold, got {:?}",
            super::folding_ranges(DOC)
        );
    }

    #[test]
    fn a_section_folds_to_the_next_heading_of_its_level_or_above() {
        let regions = lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region));
        // `# One` starts at line 4 and runs to the end of the document.
        assert!(
            regions.iter().any(|&(s, e)| s == 4 && e >= 14),
            "expected `# One` to fold through the end, got {regions:?}"
        );
        // `## Two` starts at line 8.
        assert!(
            regions.iter().any(|&(s, _)| s == 8),
            "expected `## Two` to fold, got {regions:?}"
        );
    }

    #[test]
    fn a_fenced_div_folds() {
        let regions = lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region));
        assert!(
            regions.iter().any(|&(s, e)| s == 12 && e == 14),
            "expected the ::: div to fold, got {regions:?}"
        );
    }

    #[test]
    fn an_unterminated_div_does_not_panic_and_folds_to_end_of_file() {
        let text = "::: {.callout}\nstill open\n";
        let _ = super::folding_ranges(text);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin lsp_fold`
Expected: FAIL to compile, "cannot find function `folding_ranges`"

- [ ] **Step 3: Write minimal implementation**

Add `mod lsp_fold;` to `main.rs`. Prepend to `lsp_fold.rs`:

```rust
use lsp_types::{FoldingRange, FoldingRangeKind};

/// Fold by document structure rather than by indentation: front matter, headings (each
/// running to the next heading of equal or shallower level), and `:::` fenced divs.
///
/// Indentation folding is what `.tmd` gets today (there is no `folding` key in the
/// language configuration and no server capability), and it is meaningless in a
/// Markdown-derived format where nesting is expressed by fences and heading level.
///
/// An unterminated construct folds to the last line rather than being dropped: a
/// half-typed div is the normal case for a provider that fires while the author types.
pub(crate) fn folding_ranges(text: &str) -> Vec<FoldingRange> {
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.len().saturating_sub(1) as u32;
    let mut out = Vec::new();
    // (start_line, heading_level) for each heading still open.
    let mut headings: Vec<(u32, usize)> = Vec::new();
    // start_line for each `:::` div still open.
    let mut divs: Vec<u32> = Vec::new();
    let mut fm_start: Option<u32> = None;

    for (i, raw) in lines.iter().enumerate() {
        let i = i as u32;
        let line = raw.trim_end();

        // Front matter: only when `---` opens line 0, so a thematic break mid-document
        // is not mistaken for it.
        if line == "---" {
            match fm_start {
                None if i == 0 => fm_start = Some(0),
                Some(start) => {
                    out.push(region(start, i));
                    fm_start = None;
                }
                None => {}
            }
            continue;
        }

        if let Some(level) = heading_level(line) {
            // A heading closes every open heading at its level or deeper.
            while let Some(&(start, open)) = headings.last() {
                if open >= level {
                    out.push(region(start, i.saturating_sub(1)));
                    headings.pop();
                } else {
                    break;
                }
            }
            headings.push((i, level));
            continue;
        }

        if line.starts_with(":::") {
            // A bare `:::` closes; `::: {.x}` or `:::note` opens.
            if line.trim_matches(':').trim().is_empty() {
                if let Some(start) = divs.pop() {
                    out.push(region(start, i));
                }
            } else {
                divs.push(i);
            }
        }
    }

    // Unterminated constructs fold to the end of the document.
    for (start, _) in headings {
        out.push(region(start, last));
    }
    for start in divs {
        out.push(region(start, last));
    }
    if let Some(start) = fm_start {
        out.push(region(start, last));
    }
    // A zero-height range is not foldable and clutters the client's gutter.
    out.retain(|r| r.end_line > r.start_line);
    out
}

fn region(start_line: u32, end_line: u32) -> FoldingRange {
    FoldingRange {
        start_line,
        start_character: None,
        end_line,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    }
}

/// `1` for `# x`, `2` for `## x`, and so on. `None` for anything else, including a `#`
/// with no following space (which is not a heading in CommonMark).
fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}
```

`FoldingRange::collapsed_text` is `Option<String>` in `lsp-types` 0.95.1 (`folding_range.rs:144`),
so the `region` constructor above compiles as written.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin lsp_fold`
Expected: PASS, 4 tests

- [ ] **Step 5: Advertise and dispatch**

In `server_capabilities()`:

```rust
// Replaces indentation-based folding, which is wrong for a Markdown-derived format.
folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
```

Dispatch `FoldingRangeRequest` in `handle_request` following the established shape, reading the buffer from `docs` and returning `Vec::new()` when absent. Add `assert_eq!(caps["foldingRangeProvider"], true);` to the capability test, and a wire test in `lsp_stdio.rs`.

- [ ] **Step 6: Verify by mutation**

Make the heading stack close only on an equal level, never a shallower one. `a_section_folds_to_the_next_heading_of_its_level_or_above` must FAIL. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp_fold.rs crates/server/src/main.rs \
        crates/server/src/lsp.rs crates/server/tests/lsp_stdio.rs
git commit -m "lsp: fold by heading, fenced div and front matter instead of by indentation"
```

---

### Task 7: Document highlight

**Files:**
- Modify: `crates/server/src/lsp_nav.rs` (add the targeted scan), `crates/server/src/lsp.rs`
- Test: inline in `lsp_nav.rs`, plus `crates/server/tests/lsp_stdio.rs`

**Interfaces:**
- Consumes: `classify_target`, `definition_site` (both already in `lsp_nav.rs`).
- Produces: `pub(crate) fn occurrences(text: &str, id: &str) -> Vec<(u32, u32, u32)>` returning `(line, start_char, end_char)` per occurrence, scalar offsets.

**Background:** this needs the id under the cursor plus its occurrences, so it is a targeted single-id scan, not a full-document tokenizer. `definition_site` already locates the declaration, which is what distinguishes `Write` from `Read`.

- [ ] **Step 1: Write the failing test**

Add to `lsp_nav.rs`'s test module:

```rust
#[test]
fn occurrences_finds_the_definition_and_every_reference() {
    let text = "# R {#sec-r}\n\nSee @sec-r and again @sec-r.\n";
    let hits = super::occurrences(text, "sec-r");
    assert_eq!(hits.len(), 3, "one definition + two references, got {hits:?}");
    assert_eq!(hits[0].0, 0, "the definition is on line 0");
    assert!(hits[1..].iter().all(|h| h.0 == 2), "both references are on line 2");
}

#[test]
fn occurrences_does_not_match_a_longer_id_that_starts_the_same() {
    let text = "See @sec-r and @sec-results.\n";
    let hits = super::occurrences(text, "sec-r");
    assert_eq!(hits.len(), 1, "`sec-r` must not match inside `sec-results`, got {hits:?}");
}

#[test]
fn occurrences_on_a_malformed_buffer_does_not_panic() {
    // Guards are a separate axis from span arithmetic: a cursor walk over well-formed
    // text never exercises them. Unterminated div, unterminated math, truncated front
    // matter, and a bare `@` with no id.
    for text in [
        "::: {.callout}\n@sec-r",
        "$$\n\\frac{1}{\n@sec-r",
        "---\ntitle: x\n\n@sec-r",
        "@ @- @sec-\n",
    ] {
        let _ = super::occurrences(text, "sec-r");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin occurrences_`
Expected: FAIL to compile, "cannot find function `occurrences`"

- [ ] **Step 3: Write minimal implementation**

Add to `lsp_nav.rs`:

```rust
/// Every occurrence of anchor `id` in `text`, as `(line, start_char, end_char)` in scalar
/// offsets, definition first when this document defines it.
///
/// Equality on `id`, never a prefix test: `sec-r` must not match inside `sec-results`.
/// The cursor walk asks `classify_target` at each column, so an id spanning several
/// columns reports once, not once per character.
pub(crate) fn occurrences(text: &str, id: &str) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    if let Some((line, col)) = definition_site(text, id) {
        out.push((line, col, col + id.chars().count() as u32));
    }
    for (line_no, line) in text.lines().enumerate() {
        let mut last_end: Option<usize> = None;
        for col in 0..line.chars().count() {
            let Target::Xref { id: found, start, end } = classify_target(line, 0, col) else {
                continue;
            };
            if found != id || last_end == Some(end) {
                continue;
            }
            last_end = Some(end);
            let hit = (line_no as u32, start as u32, end as u32);
            // The definition site is already in `out`; do not list it twice.
            if !out.contains(&hit) {
                out.push(hit);
            }
        }
    }
    out
}
```

`definition_site` returns `(u32, u32)` (`lsp_nav.rs:443`). If its column is the start of the
id rather than of the `{#`, the arithmetic above is correct as written; confirm against that
function before trusting the definition entry's `end`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin occurrences_`
Expected: PASS, 2 tests

- [ ] **Step 5: Advertise and dispatch**

```rust
document_highlight_provider: Some(OneOf::Left(true)),
```

Dispatch `DocumentHighlightRequest`: classify the cursor, and if it is an `Xref`, map `occurrences` to `DocumentHighlight { range, kind }` with `DocumentHighlightKind::WRITE` for the definition and `READ` for the rest, converting scalar columns to UTF-16 at the boundary. Anything else returns an empty vec. Add `assert_eq!(caps["documentHighlightProvider"], true);` and a wire test.

- [ ] **Step 6: Verify by mutation**

Change the id equality to `starts_with`. `occurrences_does_not_match_a_longer_id_that_starts_the_same` must FAIL. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp_nav.rs crates/server/src/lsp.rs crates/server/tests/lsp_stdio.rs
git commit -m "lsp: highlight every occurrence of the cross-reference under the cursor"
```

---

### Task 8: Selection ranges

**Files:**
- Modify: `crates/server/src/lsp_nav.rs`, `crates/server/src/lsp.rs`
- Test: inline, plus `crates/server/tests/lsp_stdio.rs`

**Interfaces:**
- Consumes: `classify_target`, `folding_ranges` from Task 6 (for the div and section levels).
- Produces: `pub(crate) fn selection_chain(text: &str, line: usize, col: usize) -> Vec<(u32, u32, u32, u32)>` returning `(start_line, start_char, end_line, end_char)` innermost first.

**Background:** the chain is word → inline construct (math / xref / cite / link) → paragraph → `:::` div → section. Each level must strictly contain the previous, which is the invariant most worth testing.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn each_selection_level_strictly_contains_the_previous() {
    let text = "# S {#sec-s}\n\n::: {.callout}\nSee @fig-x here.\n:::\n";
    let chain = super::selection_chain(text, 3, 6); // inside `@fig-x`
    assert!(chain.len() >= 3, "expected word, construct, paragraph, div, section: {chain:?}");
    for w in chain.windows(2) {
        let (inner, outer) = (w[0], w[1]);
        let starts_at_or_before = (outer.0, outer.1) <= (inner.0, inner.1);
        let ends_at_or_after = (outer.2, outer.3) >= (inner.2, inner.3);
        assert!(
            starts_at_or_before && ends_at_or_after,
            "level {outer:?} must contain {inner:?}"
        );
    }
}

#[test]
fn a_position_in_empty_space_yields_a_chain_without_panicking() {
    let text = "\n\n\n";
    let _ = super::selection_chain(text, 1, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server --bin taliesin selection_`
Expected: FAIL to compile, "cannot find function `selection_chain`"

- [ ] **Step 3: Write minimal implementation**

Add to `lsp_nav.rs`:

```rust
/// The nesting chain at `(line, col)`, innermost first, as
/// `(start_line, start_char, end_line, end_char)` in scalar offsets.
///
/// Levels are built inside-out and each is kept only if it *contains* the one below it,
/// so the containment invariant LSP requires holds by construction rather than by hope.
/// A level that cannot be determined is skipped, never emitted as a zero-width guess.
pub(crate) fn selection_chain(text: &str, line: usize, col: usize) -> Vec<(u32, u32, u32, u32)> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(src) = lines.get(line) else {
        return Vec::new();
    };
    let l = line as u32;
    let mut chain: Vec<(u32, u32, u32, u32)> = Vec::new();

    // 1. The word under the cursor.
    if let Some((s, e)) = word_span(src, col) {
        chain.push((l, s as u32, l, e as u32));
    }
    // 2. The inline construct enclosing it, when there is one.
    match classify_target(src, 0, col) {
        Target::Xref { start, end, .. }
        | Target::Cite { start, end, .. }
        | Target::Include { start, end, .. } => chain.push((l, start as u32, l, end as u32)),
        Target::Math { start_line, start_char, end_line, end_char, .. } => chain.push((
            start_line as u32,
            start_char as u32,
            end_line as u32,
            end_char as u32,
        )),
        _ => {}
    }
    // 3-5. Paragraph, then every enclosing fold (div, then section), outermost last.
    if let Some((s, e)) = paragraph_span(&lines, line) {
        chain.push((s, 0, e, lines[e as usize].chars().count() as u32));
    }
    let mut folds: Vec<(u32, u32)> = crate::lsp_fold::folding_ranges(text)
        .into_iter()
        .filter(|r| r.start_line <= l && r.end_line >= l)
        .map(|r| (r.start_line, r.end_line))
        .collect();
    // Innermost fold first: the one that starts latest encloses least.
    folds.sort_by_key(|&(s, e)| (std::cmp::Reverse(s), e));
    for (s, e) in folds {
        chain.push((s, 0, e, lines[e as usize].chars().count() as u32));
    }

    // Keep only levels that strictly grow.
    let mut kept: Vec<(u32, u32, u32, u32)> = Vec::new();
    for cand in chain {
        match kept.last() {
            None => kept.push(cand),
            Some(&prev) => {
                let starts_before_or_at = (cand.0, cand.1) <= (prev.0, prev.1);
                let ends_after_or_at = (cand.2, cand.3) >= (prev.2, prev.3);
                let grew = (cand.0, cand.1) < (prev.0, prev.1) || (cand.2, cand.3) > (prev.2, prev.3);
                if starts_before_or_at && ends_after_or_at && grew {
                    kept.push(cand);
                }
            }
        }
    }
    kept
}

/// `[start, end)` of the word at `col`, in scalar offsets, or `None` in whitespace.
fn word_span(line: &str, col: usize) -> Option<(usize, usize)> {
    let cs: Vec<char> = line.chars().collect();
    if col >= cs.len() || !is_word(cs[col]) {
        return None;
    }
    let mut s = col;
    while s > 0 && is_word(cs[s - 1]) {
        s -= 1;
    }
    let mut e = col;
    while e < cs.len() && is_word(cs[e]) {
        e += 1;
    }
    Some((s, e))
}

/// The blank-line-delimited paragraph containing `line`, as inclusive line numbers.
fn paragraph_span(lines: &[&str], line: usize) -> Option<(u32, u32)> {
    if lines.get(line)?.trim().is_empty() {
        return None;
    }
    let mut s = line;
    while s > 0 && !lines[s - 1].trim().is_empty() {
        s -= 1;
    }
    let mut e = line;
    while e + 1 < lines.len() && !lines[e + 1].trim().is_empty() {
        e += 1;
    }
    Some((s as u32, e as u32))
}
```

`is_word` already exists in `lsp_nav.rs:64`. This is the one place `lsp_nav` depends on
`lsp_fold`, which is why Task 6 must land first.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p taliesin-server --bin taliesin selection_`
Expected: PASS, 2 tests

- [ ] **Step 5: Advertise and dispatch**

```rust
selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
```

Dispatch `SelectionRangeRequest`, folding the chain into nested `SelectionRange { range, parent }` values, one per requested position. Add `assert_eq!(caps["selectionRangeProvider"], true);` and a wire test.

- [ ] **Step 6: Verify by mutation**

Remove the containment filter. `each_selection_level_strictly_contains_the_previous` must FAIL. Restore by inverse edit.

- [ ] **Step 7: Run every gate**

Run: `./tools/gates.sh`
Expected: green, with every `TALIESIN_REQUIRE_*` canary reporting `... ok`. **A single ignored test is a failure.** If an interpreter is genuinely unavailable in this environment, say so explicitly rather than reporting the run as clean.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/lsp_nav.rs crates/server/src/lsp.rs crates/server/tests/lsp_stdio.rs
git commit -m "lsp: smart-expand selection from word out to enclosing section"
```

---

## Closing out

- [ ] Delete items **177** and **178** from `notes/backlog.md`. The house rule is to delete a landed item, never leave a `[x]`.
- [ ] In `notes/FEATURE-IDEAS.md` Session 3, mark ideas 68-71 shipped and leave 67 and 72 parked with their existing reasons intact.
- [ ] Update `CLAUDE.md`'s `crates/server` map: `lsp*.rs` gains `lsp_memo.rs`, `lsp_hints.rs`, `lsp_fold.rs`.
- [ ] Do **not** push. The pre-push hook runs the full workspace test suite against a working tree that a parallel session is still editing.
