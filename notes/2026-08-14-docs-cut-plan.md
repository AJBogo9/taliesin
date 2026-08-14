# Docs cut implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `docs/guide` and `docs/internals` to content that clears the spec's
importance bar, fix seven verified stale claims, and gate the vocabulary so the result
cannot rot silently.

**Architecture:** Three new tests are added to the existing
`crates/core/tests/stale_docs.rs` (whose stated purpose is already "gates that compare
shipped prose against shipped behaviour"), written first so they fail against today's
prose. The seven stale claims are then fixed, turning them green. The content cut follows,
merge-before-delete so inbound links never dangle, each task ending in a green
`build --check-only` on both books.

**Tech Stack:** Rust 2024 (tests only, no production change), `.tmd` prose, `taliesin
build --check-only`, `./tools/gates.sh`.

**Spec:** `notes/2026-08-14-docs-cut-design.md`

## Global Constraints

- **No production Rust changes.** The gate reads sources as text or calls existing `pub`
  API. `taliesin-server` is binary-only (no `lib.rs`), so `COMMANDS` is parsed from
  `crates/server/src/main.rs` as text, the idiom `gate_script.rs` and `stale_docs.rs`
  already use.
- **`docs/guide/using/getting-started.tmd` must keep its path.** `tools/build-site.sh`
  (gate 5) asserts `docs/guide/using/getting-started.html` resolves. Moving it without
  updating `site/` turns gate 5 red while `cargo test` stays green.
- **`choosing.tmd`'s census figures are byte-frozen.** Gate 11
  (`tools/portability-census.py --verify`) asserts the document count, line count,
  beyond-CommonMark count, percentage, its complement, and all six per-family
  `| n | share |` pairs still appear in `docs/guide/using/choosing.tmd` and `README.md`.
  Trim around them; never through them.
- **`stale_docs.rs` has an anti-vacuity guard:** `assert!(out.len() > 25)` over the walked
  docs, currently ~38. This plan deletes 4 files, landing at ~34. Still above the floor,
  but do not delete a fifth without re-reading that assertion.
- **Both books are gated.** `.githooks/pre-push` steps 4 and 5 run
  `build docs/guide --check-only`, `build docs/internals --check-only` and
  `tools/build-site.sh --check`. Run the first two per task.
- **A retirement costs one register entry.** Do not write a tombstone test for anything.
- **No em dashes or en dashes in any prose written.** Project author's standing rule.
- **Commit per task on branch `docs-cut`.** The project's convention is one commit per unit
  of work on `main`; whether to squash before merging is the author's call, raised at the
  end rather than decided here.

---

### Task 1: The truth gate, written to fail

**Files:**
- Modify: `crates/core/tests/stale_docs.rs` (append three tests + two helpers)

**Interfaces:**
- Consumes: existing helpers in that file: `repo()`, `read(rel)`, `shipped_docs() ->
  Vec<(String, String)>`, `backticked(text) -> Vec<String>`, `const_block(src, name) ->
  String`, `string_literals(block) -> Vec<String>`.
- Consumes: `taliesin_core::render::executes_to_kernel(lang: &str) -> bool` (public).
- Produces: `live_verbs() -> Vec<String>`, used by no later task but kept for future gates.

**What this gate does and does not cover.** It catches claims 1, 4 and 6 mechanically.
Claims 2, 3, 5 and 7 are prose assertions about behaviour (a deleted server, which book the
hook lints, chrome nothing emits, "R has none") and are not mechanically catchable without
false positives. They are fixed by hand in Task 2. The gate asserts **vocabulary**, not
truth in general, and its doc comments must say so.

- [ ] **Step 1: Write the three failing tests**

Append to `crates/core/tests/stale_docs.rs`:

```rust
/// Every `taliesin <verb>` a shipped doc teaches must be a verb the binary answers.
///
/// `CLAUDE.md` names this hole explicitly: a new subcommand has four registration sites in
/// `main.rs`, each drift-gated, and a **fifth** in `docs/guide/reference/cli.tmd`'s table
/// that nothing gates. Wave 13 left the retired `run` row standing through several edits;
/// what eventually caught it was `documented_cli_flags_exist_in_the_cli` noticing the
/// *flags* inside the row, so a verb with no flags would have shipped a documented command
/// the binary does not answer with every gate green. This closes that.
///
/// `COMMANDS` is read as text because `taliesin-server` is a binary-only crate with no
/// `lib.rs`, so a test crate cannot import it. That is the same approach `gate_script.rs`
/// takes to the same problem, and it needs no production change.
#[test]
fn shipped_docs_do_not_name_a_verb_the_binary_does_not_answer() {
    let main_rs = read("crates/server/src/main.rs");
    let live = string_literals(const_block(&main_rs, "COMMANDS"));
    assert!(
        live.contains(&"preview".to_string()) && live.contains(&"build".to_string()),
        "COMMANDS parsed as {live:?} — the shape changed, update this gate rather than \
         deleting it"
    );

    // Words that follow `taliesin ` but are flags, paths or prose rather than verbs.
    let not_a_verb = |w: &str| {
        w.is_empty()
            || w.starts_with('-')
            || w.contains('/')
            || w.contains('.')
            || w.contains('<')
            || !w.chars().all(|c| c.is_ascii_lowercase())
    };

    let mut hits = Vec::new();
    for (rel, text) in shipped_docs() {
        for (n, line) in text.lines().enumerate() {
            for (idx, _) in line.match_indices("taliesin ") {
                let rest = &line[idx + "taliesin ".len()..];
                let word: String = rest
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '`' && *c != ')')
                    .collect();
                if not_a_verb(&word) || live.contains(&word) {
                    continue;
                }
                hits.push(format!("{rel}:{}: taliesin {word}", n + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "shipped doc(s) name a subcommand the binary does not answer (live: {live:?}):\n{}",
        hits.join("\n")
    );
}

/// A `theme:` value the manual teaches must not draw a warning from the renderer.
///
/// This checks *behaviour*, not a register, and deliberately so: `theme: light|dark|default`
/// is a retired **value** of a **live** key, and all three retirement registers key on the
/// key. `shipped_docs_do_not_use_a_retired_front_matter_key` therefore cannot see it, which
/// is exactly how `formats.tmd` came to teach `theme: dark` while `theming.tmd` correctly
/// called it retired, in the same book, for a day short of a year's worth of gates.
///
/// Only bare-word values are checked. A `theme: custom.scss` in a shipped example names a
/// file that does not exist next to a synthetic render and would warn for an unrelated
/// reason.
#[test]
fn shipped_docs_do_not_teach_a_theme_value_the_renderer_warns_about() {
    let mut taught: Vec<(String, usize, String)> = Vec::new();
    for (rel, text) in shipped_docs() {
        for (n, line) in text.lines().enumerate() {
            let Some(v) = line.trim().strip_prefix("theme:") else {
                continue;
            };
            let v = v.trim().trim_matches('"').trim_matches('\'');
            // Bare words only: no path, no extension, no placeholder.
            if v.is_empty()
                || v.contains('.')
                || v.contains('/')
                || v.contains('<')
                || !v.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            {
                continue;
            }
            taught.push((rel.clone(), n + 1, v.to_string()));
        }
    }

    let mut hits = Vec::new();
    for (rel, line, value) in taught {
        let src = format!("---\ntitle: \"t\"\ntheme: {value}\n---\n\nBody.\n");
        let doc = taliesin_core::render_document(&src);
        if let Some(w) = doc.warnings.iter().find(|w| w.message.contains(&value)) {
            hits.push(format!("{rel}:{line}: theme: {value} — {}", w.message));
        }
    }
    assert!(
        hits.is_empty(),
        "shipped doc(s) teach a `theme:` value the renderer warns about:\n{}",
        hits.join("\n")
    );
}

/// A `{lang}` cell language a shipped doc names must still be one the tool handles.
///
/// `{r}` was the second kernel language and was cut in Wave 6, leaving two mentions in
/// `internals/execution.tmd`. This reads backticked tokens, so it catches `` `{r}` `` in
/// prose and in a fence info string alike. It does **not** catch a bare prose "R", which is
/// why claim 7 in the spec is fixed by hand.
#[test]
fn shipped_docs_do_not_name_a_cell_language_the_tool_dropped() {
    // Handled at render time rather than by a kernel, so `executes_to_kernel` is false for
    // each and each is still legitimate in the manual.
    let render_side = ["js", "mermaid", "=html"];

    let mut hits = Vec::new();
    for (rel, text) in shipped_docs() {
        for (n, line) in text.lines().enumerate() {
            for tok in backticked(line) {
                let Some(lang) = tok.strip_prefix('{').and_then(|t| t.strip_suffix('}'))
                else {
                    continue;
                };
                if lang.is_empty()
                    || lang.contains(' ')
                    || render_side.contains(&lang)
                    || taliesin_core::render::executes_to_kernel(lang)
                {
                    continue;
                }
                hits.push(format!("{rel}:{}: {{{lang}}}", n + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "shipped doc(s) name a cell language the tool no longer handles:\n{}",
        hits.join("\n")
    );
}
```

- [ ] **Step 2: Run the three tests and confirm each fails for the right reason**

Run:

```sh
cargo test -p taliesin-core --test stale_docs -- --nocapture \
  shipped_docs_do_not_name_a_verb_the_binary_does_not_answer \
  shipped_docs_do_not_teach_a_theme_value_the_renderer_warns_about \
  shipped_docs_do_not_name_a_cell_language_the_tool_dropped
```

Expected: three FAILs, naming at minimum
`docs/internals/architecture.tmd` (`taliesin run`),
`docs/guide/using/formats.tmd` (`theme: dark`), and
`docs/internals/execution.tmd` (`{r}`).

**If any test passes, stop.** A gate that is green before the fix is not testing what it
claims. Read its output, correct the extractor, and re-run before continuing.

**If a test fails on a doc this plan does not touch**, that is a real eighth defect. Record
it in the spec's stale-claims table and fix it in Task 2.

- [ ] **Step 3: Verify `render_document` and `Warning` are reachable as written**

Run: `cargo test -p taliesin-core --test stale_docs --no-run`

Expected: compiles. If `render_document` is not re-exported at the crate root, use
`taliesin_core::render::render_document`; if `Warning.message` is private, match on the
`Display` output instead. Fix inline; do not weaken the assertion.

- [ ] **Step 4: Commit the failing gate**

```sh
git add crates/core/tests/stale_docs.rs
git commit -m "test: gate the docs' verb, theme-value and cell-language vocabulary

Three tests in the file whose stated job is already comparing shipped prose
against shipped behaviour. Each fails against today's books, which is the
point: the manual names a `run` verb cut in Wave 13, teaches a `theme: dark`
retired on 2026-08-13, and names an `{r}` kernel cut in Wave 6.

The theme test checks behaviour rather than a register on purpose. All three
retirement registers key on the KEY, and a retired *value* of a *live* key is
invisible to them, which is how formats.tmd came to contradict theming.tmd
inside the same book with every gate green.

Fails until the next commit."
```

---

### Task 2: Fix the seven stale claims

**Files:**
- Modify: `docs/internals/architecture.tmd:273` (claim 1), `:112-135` (claim 2)
- Modify: `docs/internals/extending.tmd:31` (claim 3)
- Modify: `docs/guide/using/formats.tmd:141-146` (claim 4)
- Modify: `docs/guide/using/preview.tmd:155-157` (claim 5)
- Modify: `docs/internals/execution.tmd:25` (claim 6), `:249` (claim 7)

**Interfaces:**
- Consumes: the three tests from Task 1.
- Produces: a green `stale_docs` suite, which every later task must keep green.

**Line numbers are from 2026-08-14 and will move as you edit.** Grep the quoted string, do
not trust the number.

- [ ] **Step 1: Claim 1, the subcommand list**

In `docs/internals/architecture.tmd`, the `main.rs` row of the `crates/server/src` table
reads:

> `main.rs` | CLI: the `preview` / `build` / `run` / `lsp` / `init` subcommands, plus the retired-verb register.

Replace with:

> `main.rs` | CLI: the seven subcommands (`COMMANDS`) plus the retired-verb register, which answers a cut verb with its successor rather than a did-you-mean.

Naming the count and the const rather than the list is deliberate: the list is what rotted.

- [ ] **Step 2: Claim 2, the deleted second server**

In `docs/internals/architecture.tmd`, delete the whole `### One door, two server paths`
subsection (the intro sentence, the two-column table, and the paragraph beginning "Both
walk the same re-render"), and keep the paragraph that follows it, which already states the
truth ("One server handles a project and a single document alike"). Promote that paragraph
to sit directly under `## How a save flows`.

Verify the deletion is total:

```sh
grep -rn 'DocState\|two server paths\|serve/mod.rs.*single' docs/
```

Expected: no hits.

- [ ] **Step 3: Claim 3, the pre-push book list**

In `docs/internals/extending.tmd`, `build docs/guide --check-only` becomes
`build docs/{guide,internals} --check-only`, matching `.githooks/pre-push:94`.

- [ ] **Step 4: Claim 4, the retired theme values**

In `docs/guide/using/formats.tmd`, the `## Themes` section opens:

> `theme:` selects the built-in `light` (default) or `dark`, a `.css`/`.scss` file, or an installed `_extensions/<name>/` bundle.

Replace with:

> `theme:` points at a `.css`/`.scss` file or an installed `_extensions/<name>/` bundle. There is no built-in mode name to select: both palettes ship in every page and the reader's device picks between them.

In the same section, "the light/dark toggle just swaps the values: nothing re-renders"
becomes "switching palettes just swaps the values: nothing re-renders".

- [ ] **Step 5: Claim 5, the navbar theme toggle**

In `docs/guide/using/preview.tmd`, the dev-menu bullet reads:

> A **theme toggle** (light/dark), but only when the page has no theme toggle of its own. A site puts one in its navbar, so the dev menu adds one only for a document previewed outside any project, whose chrome has none.

Replace with:

> A **theme toggle** (light/dark). It is the only theme control anywhere in Taliesin and it belongs to the preview alone, so a built page never carries one: what a reader sees is their own device setting.

- [ ] **Step 6: Claims 6 and 7, the R kernel**

In `docs/internals/execution.tmd`, the quoted `Executor` struct's comment
`// "python" / "r" → warm kernel + ran-list` becomes `// "python" → warm kernel + ran-list`.

Add one sentence after the struct, because a one-key map invites a reviewer to
"simplify" it, and `CLAUDE.md` records that it must stay a map:

> `langs` holds one key today. It stays a `HashMap` on purpose: `{r}` was the second kernel language until Wave 6, and the shape is what makes a third cheap.

In the `start()` list, step 5 ends "R has none." Delete those three words and end the
sentence at the matplotlib hook.

- [ ] **Step 7: Run the gate and confirm it is now green**

```sh
cargo test -p taliesin-core --test stale_docs
```

Expected: PASS, all tests in the file.

- [ ] **Step 8: Confirm both books still build**

```sh
cargo run -q -p taliesin-server -- build docs/guide     --check-only --no-exec
cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec
```

Expected: exit 0 for both.

- [ ] **Step 9: Commit**

```sh
git add docs/ && git commit -m "docs: fix seven claims that outlived the code they describe

Four were enumerations a const already holds, which is the pattern the gate
in the previous commit exists to catch: a `run` verb cut in Wave 13, a
two-server split whose DocState no longer exists, a `theme: dark` retired on
2026-08-13, and an `{r}` kernel cut in Wave 6 named twice.

The other three are prose the gate cannot see and that this commit fixes by
hand: extending.tmd named one of the two books the pre-push hook lints,
preview.tmd described a navbar theme toggle nothing emits, and execution.tmd
said 'R has none' in prose.

architecture.tmd's `main.rs` row now names the count and the const instead of
the list, since the list is what rotted."
```

---

### Task 3: Internals structure, fold `server.tmd` into `architecture.tmd`

**Files:**
- Modify: `docs/internals/architecture.tmd` (receive; delete three module tables)
- Delete: `docs/internals/server.tmd`
- Modify: `docs/internals/_site.yml` (drop the chapter)
- Modify: any chapter linking to `server.tmd`

**Interfaces:**
- Consumes: Task 2's corrected `architecture.tmd`.
- Produces: a five-chapter internals book. Later tasks must not re-add a sixth.

- [ ] **Step 1: Find every inbound link before deleting anything**

```sh
grep -rn 'server\.tmd' docs/
```

Record the hits. Each becomes a link to `architecture.tmd` in Step 4.

- [ ] **Step 2: Move the three sections worth keeping**

From `docs/internals/server.tmd`, move into `architecture.tmd` after "How a save flows":

- `### The watcher` (the `notify` thread, the inotify-exhaustion reason for not watching
  recursively, and why the skip list is matched relative to the project root rather than
  against the whole path).
- `### Panic isolation` (`build_page_guarded`, `catch_unwind`, why `parking_lot::Mutex`
  matters here).
- `## Binding, and the two guards` (loopback-only, `ws_origin_ok` vs `with_host_guard`, and
  why a rebind defeats the origin check).

Do **not** move: the save-loop mermaid diagram (duplicates `fig-save-flow`), `## What the
target means`, or `## The server (serve_site/mod.rs)` beyond a sentence, since Task 2's
surviving paragraph already covers the one-server point.

Do **not** move `## Why one server, not two` (the whole section). It is archaeology, and
decision 1 in the spec cuts that layer.

- [ ] **Step 3: Delete the three module/file-map tables**

In `architecture.tmd`, delete the `## The codebase map` tables for `crates/core/src`,
`crates/server/src` and the `web-client/` paragraph, along with the "here is the
file-by-file detail" lead-in. Keep `@fig-modules` and `@fig-data-model` and their
explanatory prose: a diagram of dependency shape is not a list that rots row by row.

Replace the deleted tables with:

> The file-by-file detail is not restated here, because a table of module responsibilities is a second copy of something each file's own header already says, and a second copy is what goes stale. Start at `crates/core/src/render/mod.rs` for the pipeline, `crates/core/src/site/mod.rs` for multi-page projects, and `crates/server/src/serve_site/mod.rs` for the dev server. Each opens with a header describing what it owns.

- [ ] **Step 4: Delete the file and repoint links**

```sh
git rm docs/internals/server.tmd
```

Remove `- server.tmd` from `docs/internals/_site.yml`, and repoint every hit from Step 1.

- [ ] **Step 5: Verify**

```sh
cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec
cargo test -p taliesin-core --test stale_docs
```

Expected: exit 0, and PASS. A dangling `server.tmd` link fails the first; a path claim that
no longer resolves fails `shipped_docs_do_not_name_a_file_that_does_not_exist` in the second.

- [ ] **Step 6: Commit**

```sh
git add -A docs/internals && git commit -m "docs: fold the dev-server chapter into architecture, and cut the module maps

server.tmd's save-loop diagram duplicated fig-save-flow and its 'why one
server, not two' section is archaeology. What it uniquely held (the watcher's
inotify reasoning, panic isolation, the two security guards) moves into
architecture.tmd, which is where a reader looking for the save path already is.

The three module/file-map tables go with it. A table of module
responsibilities is a second copy of what each file's header already states,
and it is precisely the shape that rotted: two of the seven claims fixed in
the previous commit were rows in these tables."
```

---

### Task 4: Internals, `extending.tmd` to an appendix and `index.tmd` trim

**Files:**
- Modify: `docs/internals/extending.tmd` (315 lines to ~90)
- Modify: `docs/internals/index.tmd` (drop the chapter-count paragraph, repoint the reading list)

- [ ] **Step 1: Cut `extending.tmd` to what an evaluator needs**

Keep, in this order: `## The guiding principle: a lean core`, `## Conventions`,
`## The two extension points`, `### The client enhancer contract` including its code
example, and `### Byte-identity for refactors`.

Delete: `## Where a change goes` (a table of file paths, same failure mode as Task 3),
`## Editor integration` in full (the LSP capability table plus both
capability-removal paragraphs at `:141-159`, which are the rationale layer), `## Recipes`
(`### Add a new cell option`, which is contributor material), `## The dev loop`, and
`### The corpus records; it does not lead` (rationale).

Replace the editor section with two sentences:

> An editor gets everything from `taliesin lsp`, an offline kernel-free language server over stdio, so `cmd = { "taliesin", "lsp" }` is the entire setup in any LSP editor. The VS Code companion in `editor/vscode/` implements no language features of its own; it adds only what the protocol has no concept of, which is the preview webview and the source sync.

- [ ] **Step 2: Trim `index.tmd`**

Delete the closing paragraph beginning "**Six chapters, and that is deliberate.**" It is
archaeology, and after Task 3 its count is wrong anyway.

Update the `## How to read this book` list: drop the `server.tmd` bullet (folded into
Architecture in Task 3), and reword the `extending.tmd` bullet to "the conventions and the
two seams meant to be extended".

- [ ] **Step 3: Verify**

```sh
cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec
cargo test -p taliesin-core --test stale_docs
```

Expected: exit 0, PASS.

- [ ] **Step 4: Commit**

```sh
git add -A docs/internals && git commit -m "docs: cut the extending chapter to what an evaluator reads

The book is for a reader deciding whether the design is real, plus the
author's future self. It was written for a contributor who does not exist
yet: a file-path table, a full LSP capability table, two paragraphs on
capabilities removed in August, and a recipe for adding a cell option.

What survives is the part an evaluator uses: the lean-core principle, the
conventions, the two seams, and the enhancer contract with its example.
The index's chapter-count paragraph goes too; it was archaeology, and the
previous commit made its count wrong."
```

---

### Task 5: Internals, trim the three dense chapters

**Files:**
- Modify: `docs/internals/rendering.tmd` (211 to ~195)
- Modify: `docs/internals/block-model.tmd` (261 to ~240)
- Modify: `docs/internals/execution.tmd` (368 to ~330)
- Modify: `docs/guide/using/choosing.tmd` (donate the payload analysis)

**These three chapters are nearly all keepers.** The spec revised its own estimate up after
reading them. Trim asides, not mechanism. If a cut would remove something not
reconstructible from source in less time than reading it, do not make the cut.

- [ ] **Step 1: Move the payload-shape analysis out of the Guide**

Cut the paragraph in `docs/guide/using/choosing.tmd` beginning "That payload row deserves
its detail" (the 55 ops, the 53 metadata-only patches, the 29 KB callout, the collapse
callout closing on an edit above). It is internals content sitting in an adoption page.

**Do not touch the census table or the speed table above it.** Gate 11 asserts the census
figures byte-for-byte.

Paste it into `docs/internals/block-model.tmd`, after `### Why the diff is O(n log n)`,
where the `SetMeta` mechanism it depends on has just been explained.

In `choosing.tmd`, leave the one sentence that carries the point ("The number that matters
is not any single row: it is that the warm edit is a **diff**, so its cost tracks what you
changed rather than how large the document is") and link to the internals chapter.

- [ ] **Step 2: Trim the asides in all three**

Remove the dated or archaeological asides only:

- `rendering.tmd`: none found on read; verify with `grep -n '2026-0' docs/internals/rendering.tmd` and remove any hit.
- `block-model.tmd`: keep everything mechanical, including the protocol table (a stable
  at-a-glance contract) and `## Trust model` (a real constraint on use).
- `execution.tmd`: keep the cumulative-hash chain, the `_freeze/` file properties, the
  `packages:` digest reasoning, the silence cap, the dual-theme matplotlib rendering, and
  `## How generated outputs reach the build`. Tighten prose only.

- [ ] **Step 3: Verify both books, since one edit crossed between them**

```sh
cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec
cargo run -q -p taliesin-server -- build docs/guide     --check-only --no-exec
python3 tools/portability-census.py --verify
```

Expected: exit 0 for all three. The census check is here specifically because this task
edits `choosing.tmd`.

- [ ] **Step 4: Commit**

```sh
git add -A docs && git commit -m "docs: move the payload analysis to internals, trim the rest

choosing.tmd is the page a reader opens to decide whether to adopt. A
paragraph breaking a 32 KB warm-edit payload into 55 ops, 53 of them
metadata-only patches, is internals content: it belongs beside the SetMeta
mechanism that explains it, which is block-model.tmd.

The three internals chapters are otherwise nearly untouched, and that is the
finding rather than an omission. The spec's estimate for this book was
revised up from 750 lines to 1,100 after reading them: the LIS reason the
diff is O(n log n), the removes-before-inserts ordering rule, the 256 MB
render stack, and footnote definitions folded into a block's hash input are
none of them reconstructible from source faster than reading them here.

The census figures in choosing.tmd are untouched; gate 11 re-verified."
```

---

### Task 6: Guide, delete `reading.tmd`

**Files:**
- Modify: `docs/guide/reference/accessibility.tmd` (receive ~35 lines)
- Delete: `docs/guide/using/reading.tmd`
- Modify: `docs/guide/_site.yml`

- [ ] **Step 1: Find inbound links**

```sh
grep -rn 'reading\.tmd' docs/ site/
```

Repoint each to `../reference/accessibility.tmd` or delete the sentence, whichever reads
better. **Check `site/` too:** gate 5 fails on a broken cross-project link.

- [ ] **Step 2: Move the keyboard and accessibility section**

Move `## Keyboard and accessibility` from `reading.tmd` (the skip link, the three keyboard
interactions, the WCAG 2.1.4 note, the focus ring, landmarks and dialogs, high contrast,
and the typographic-polish closer) into `reference/accessibility.tmd`, after
`## Reader controls`.

Also move the first two paragraphs of `## Theme` (the device-decides behaviour and the
no-flash pre-paint), which are how the page actually behaves, into
`reference/accessibility.tmd`'s `## Reader controls`. That section currently describes
controls; it now describes the absence of them and why.

- [ ] **Step 3: Delete the rest**

`git rm docs/guide/using/reading.tmd`, and remove it from `docs/guide/_site.yml`.

What goes: `## No focus mode, and no fullscreen`, `## What else was tried, and cut` (all
eleven bullets), the sepia paragraph, and the no-text-size-knob paragraph. This is the
rationale layer the author cut.

- [ ] **Step 4: Verify**

```sh
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
tools/build-site.sh --check
```

Expected: exit 0 for both. `build-site.sh --check` is run here because Step 1 touched
`site/` if there were inbound links.

- [ ] **Step 5: Commit**

```sh
git add -A docs/guide site && git commit -m "docs: delete the reader-experience chapter, keep its accessibility half

Thirty-six percent of it was a catalogue of eleven reader features that were
built and then removed: hover cross-reference cards, reading-position resume,
TOC checkmarks, anchor copy-links, the mobile contents sheet, the image
lightbox, the show-code toggle, the code-download box, the theme picker, the
listing filter chips, and two character-key shortcuts. Every entry is a good
answer to a question a reader of a user guide is not asking.

What was load-bearing moves to the conformance report, where a reader looking
for keyboard access already goes: the skip link, the three keyboard
interactions, the WCAG 2.1.4 reasoning, the focus ring, landmarks, and
forced-colors. The device-decides theme behaviour goes with it."
```

---

### Task 7: Guide, delete `shortcodes.tmd`

**Files:**
- Modify: `docs/guide/using/writing.tmd` (receive `{{< include >}}`)
- Modify: `docs/guide/using/interactive.tmd` (receive `{{< input >}}`)
- Delete: `docs/guide/reference/shortcodes.tmd`
- Modify: `docs/guide/_site.yml`

- [ ] **Step 1: Find inbound links**

```sh
grep -rn 'shortcodes\.tmd' docs/ site/
```

- [ ] **Step 2: Move `{{< include >}}` into `writing.tmd`**

Add a `## Reusing a partial` section near the end of `writing.tmd`, before `## Raw HTML
passthrough`, carrying the `{{< include >}}` syntax, the `_includes/` leading-underscore
convention (a `_`-prefixed file is not a page), and the one sentence that matters most:
click-to-source still resolves to the partial's own file and line.

- [ ] **Step 3: Move `{{< input >}}` into `interactive.tmd`**

`interactive.tmd` already has `## Inputs without boilerplate: {{< input >}}`. Merge the
reference page's type table into it rather than duplicating the prose.

- [ ] **Step 4: Delete and repoint**

`git rm docs/guide/reference/shortcodes.tmd`, remove it from `_site.yml`, repoint Step 1's
hits.

- [ ] **Step 5: Verify**

```sh
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
```

Expected: exit 0.

- [ ] **Step 6: Commit**

```sh
git add -A docs/guide && git commit -m "docs: retire the shortcode reference into the two chapters that use them

There are two shortcodes. A reference page for two entries is a page a reader
has to find, and both were already explained where they are used: `{{< input >}}`
in the interactive chapter, and `{{< include >}}` nowhere obvious, which is
the actual gap this closes."
```

---

### Task 8: Guide, fold `formats.tmd` into `recipes.tmd` and trim to two recipes

**Files:**
- Modify: `docs/guide/using/recipes.tmd` (333 to ~200)
- Delete: `docs/guide/using/formats.tmd`
- Modify: `docs/guide/_site.yml`

- [ ] **Step 1: Find inbound links**

```sh
grep -rn 'formats\.tmd' docs/ site/
```

There are several: `index.tmd` and `getting-started.tmd` both link to it.

- [ ] **Step 2: Keep what `formats.tmd` uniquely holds**

Most of it restates `recipes.tmd` and `configuration.tmd`. Three things do not, and move
into `recipes.tmd` as a short `## Books, sites, and the difference` preamble:

- A book is a directory whose `_site.yml` lists `chapters:`; their presence is what makes
  it a book, with no `type:` needed.
- A page with `toc: true` gets a rail; a book has no rail, it has a Chapters drawer.
- The per-chapter word count in the drawer, and why it is a word count rather than a
  reading time (it excludes fenced code and math, so a minutes label would carry that
  error into a promise about the reader's time).

Its `## Themes` section is already covered by `theming.tmd` and was claim 4. Drop it.

- [ ] **Step 3: Cut two of the four recipes**

Keep recipe 1 (personal blog, exercises `listing:` and dated posts) and recipe 3
(documentation book, exercises `chapters:` and `part:`). These are the two shapes that
differ structurally.

Delete recipe 2 (project/portfolio site) and recipe 4 (single long-form post). Recipe 2 is
recipe 1 with `hero:` instead of `listing:`, both of which are documented in the front
matter reference; recipe 4 is a single document, which `getting-started.tmd` already walks.

- [ ] **Step 4: Delete and repoint**

`git rm docs/guide/using/formats.tmd`, remove from `_site.yml`, repoint Step 1's hits to
`recipes.tmd`.

- [ ] **Step 5: Verify**

```sh
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
tools/build-site.sh --check
```

Expected: exit 0 for both.

- [ ] **Step 6: Commit**

```sh
git add -A docs/guide site && git commit -m "docs: fold books-and-sites into recipes, and keep the two recipes that differ

formats.tmd restated recipes.tmd and configuration.tmd almost entirely. Three
things it held uniquely survive as a preamble: what makes a directory a book,
why a book has a drawer instead of a rail, and why the drawer shows a word
count rather than a reading time.

Two of the four recipes go. The portfolio site is the blog recipe with
`hero:` instead of `listing:`, and the single long-form post is one document,
which getting-started already walks end to end. What is left are the two
shapes that differ structurally."
```

---

### Task 9: Guide, merge the two key references

**Files:**
- Modify: `docs/guide/reference/frontmatter.tmd` (becomes the merged page, ~330)
- Delete: `docs/guide/reference/configuration.tmd`
- Modify: `docs/guide/_site.yml`

**This task carries a drift gate.** `CLAUDE.md`: a new front-matter key trips four gates,
one of which is `the_reference_page_documents_every_known_key`, pointed at
`docs/guide/reference/frontmatter.tmd`. **That file must keep its path and keep documenting
every `KNOWN_KEYS` entry.** Merge `configuration.tmd` *into* it, never the reverse.

- [ ] **Step 1: Confirm which file the gate names**

```sh
grep -rn 'frontmatter.tmd\|configuration.tmd' crates/core/tests/ crates/core/src/
```

Expected: `frontmatter.tmd` named by the reference-page gate. If `configuration.tmd` is
also named by a gate, both paths are frozen and this task becomes a trim of each in place
rather than a merge. Adjust and note it in the spec.

- [ ] **Step 2: Retitle and restructure**

`frontmatter.tmd` becomes "Configuration reference", covering both vocabularies in two
parts: `# Document front matter` (its current content, unchanged in coverage) and
`# Project _site.yml` (from `configuration.tmd`).

Keep from `configuration.tmd`: the `_site.yml` key table, the editor-autocomplete schema
line (`# yaml-language-server: $schema=...`), and `## Project structure & reserved names`.

Drop from `configuration.tmd`: `### Code-cell options` (a third copy; `cell-options.tmd` is
the reference and `code.tmd` is the explanation) and `## See also`.

- [ ] **Step 3: Delete and repoint**

```sh
grep -rn 'configuration\.tmd' docs/ site/
git rm docs/guide/reference/configuration.tmd
```

Remove from `_site.yml` and repoint every hit.

- [ ] **Step 4: Verify, including the reference-page drift gate**

```sh
cargo test -p taliesin-core
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
```

Expected: PASS and exit 0. A dropped key fails
`the_reference_page_documents_every_known_key` specifically; read its output rather than
guessing which key went missing.

- [ ] **Step 5: Commit**

```sh
git add -A docs/guide && git commit -m "docs: one configuration reference instead of two

frontmatter.tmd and configuration.tmd both answered 'which key goes where',
and a reader with a key in hand had to guess which page held it. They are now
one page in two parts, document front matter then project _site.yml.

frontmatter.tmd keeps the path because a drift gate names it: adding a
KNOWN_KEYS entry without documenting it there fails
the_reference_page_documents_every_known_key. Merging the other direction
would have removed one of the four gates a new key trips.

configuration.tmd's cell-options section was a third copy of a table that
lives in cell-options.tmd and is explained in code.tmd. It goes."
```

---

### Task 10: Guide, trim `code.tmd` and `cli.tmd`

**Files:**
- Modify: `docs/guide/using/code.tmd` (396 to ~260)
- Modify: `docs/guide/using/interactive.tmd` (receive the 3-D section)
- Modify: `docs/guide/reference/cli.tmd` (496 to ~260)
- Modify: `docs/guide/reference/cell-options.tmd` (239 to ~120)

- [ ] **Step 1: Move 3-D out of `code.tmd`**

`code.tmd` has `### 3D graphics (Three.js / WebGL)` and `interactive.tmd` has
`## Interactive 3-D with a {js} cell`. Keep the one in `interactive.tmd`, which is where a
reader looking for interactivity goes, and delete the one in `code.tmd`, leaving a
one-sentence pointer.

Also move `## Live interactive content` and `### The Python → JS bridge` from `code.tmd` to
`interactive.tmd`, for the same reason: `code.tmd` is about kernel execution, and these are
about the browser.

- [ ] **Step 2: Consolidate the echo/include/cache explanation**

Both `code.tmd` (`### echo, include, and cache: three different switches`) and
`cell-options.tmd` (`## echo vs include vs cache`, `## The cache key sees your code, not
your data`, `## include: false keeps state`) explain the same three options.

Keep the *explanation* in `code.tmd` and merge the best of both there, including the
cache-key-sees-your-code point, which is the one people get wrong. Reduce
`cell-options.tmd` to the `## Every option` table, `## How options are written`, the
leading-block rule, and `## {js} reactive options`.

- [ ] **Step 3: Trim `cli.tmd`**

Delete `## Publishing & sharing` (it restates `getting-started.tmd`'s Publish section,
which is where a reader is when they need it) and the two diagnostics-code archaeology
passages at roughly `:130` and `:158` ("...on 2026-08-08. What an author needs from a
diagnostic is the fix..." and the cut-lint-family aside).

Keep every flag and every verb row: this page is the CLI's reference and two drift gates
read it.

- [ ] **Step 4: Verify, including the CLI flag gate**

```sh
cargo test -p taliesin-core
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
```

Expected: PASS and exit 0. `documented_cli_flags_exist_in_the_cli` fails if a trim removed
a flag's context in a way that breaks its extractor; and Task 1's new verb gate fails if a
verb row was dropped.

- [ ] **Step 5: Commit**

```sh
git add -A docs/guide && git commit -m "docs: put browser-side content in the interactive chapter, and de-duplicate

code.tmd is about cells that run against a warm kernel. Three of its sections
were about cells that run in the reader's browser, one of them a full second
copy of the Three.js walkthrough in interactive.tmd. They move.

echo/include/cache was explained twice, in code.tmd and cell-options.tmd. The
explanation stays in code.tmd, where a reader meets the options; cell-options
keeps the lookup table and the leading-block rule.

cli.tmd loses its publishing section, which restated getting-started, and two
passages of diagnostics-code archaeology. Every flag and verb row is intact:
two drift gates read them."
```

---

### Task 11: Guide, trim the remaining chapters

**Files:**
- Modify: `docs/guide/index.tmd`, `using/getting-started.tmd`, `using/preview.tmd`,
  `using/writing.tmd`, `using/theming.tmd`, `using/choosing.tmd`,
  `reference/troubleshooting.tmd`, `reference/licensing.tmd`

- [ ] **Step 1: `index.tmd`, fold the two openings together**

`## Three things it gets right` and `## The 60-second version` say the same thing twice,
and the internals index says it a third time. Keep one: the three named bullets
(click-to-source, block-level incremental updates, no per-edit startup cost), since those
are the three load-bearing goals the whole project is organized around. Fold the 60-second
paragraph's one distinct sentence ("That loop is the whole product; everything below is
what you can put inside it") into it.

Repoint the closing paragraph's link to `formats.tmd` (deleted in Task 8) to `recipes.tmd`.

- [ ] **Step 2: `preview.tmd`, compress the Sections view**

`### Sections: the shape of the draft` is 27 lines for a dev-menu panel. Reduce to about
six: what it lists, that a section is a heading down to the next of equal or shallower
level (the same definition folding and the outline use), that badges place diagnostics
where they happened, and that it is a revision view a build never ships.

- [ ] **Step 3: `writing.tmd`, cut two asides**

Delete the unhighlighted-fence aside ("Taliesin no longer reports it: the lint that did was
cut on 2026-08-08..."), keeping the actionable half: an unrecognised language renders
unhighlighted, `` ```pyton `` warns, and `text`/`console` is how you mean it.

Delete the `.sidenote`/`.marginnote`/`.aside` retirement sentence, keeping "`.column-margin`
is the only spelling."

- [ ] **Step 4: `theming.tmd`, cut the closing rationale**

Delete `## Which palette a reader sees is not yours to set` in full and the
`theme: dark`-was-retired aside at roughly `:23`. Keep `## Two built-in palettes, and the
device picks between them`, which states the behaviour without arguing for it.

Keep the whole `--tali-*` variable reference, the Mermaid colour section, and the three
customization routes.

- [ ] **Step 5: `choosing.tmd`, `troubleshooting.tmd`, `licensing.tmd`**

- `choosing.tmd`: no further cuts. Task 5 already moved the payload paragraph, and the
  census figures are frozen by gate 11.
- `troubleshooting.tmd`: delete the `TAL-*` code history at roughly `:32`, keeping how to
  read a diagnostic's file:line. Every symptom section stays.
- `licensing.tmd`: reduce to the question, the answer (the Output Exception grants
  unrestricted rights over your own built pages with nothing to attribute), what the AGPL
  still covers (the tool's own source), and the one-sentence version. Drop the extended
  framing.

- [ ] **Step 6: Verify**

```sh
cargo test -p taliesin-core
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
python3 tools/portability-census.py --verify
```

Expected: PASS, exit 0, exit 0.

- [ ] **Step 7: Commit**

```sh
git add -A docs/guide && git commit -m "docs: cut the remaining rationale asides and one duplicated opening

The guide index opened twice, with three named bullets and then a 60-second
paragraph saying the same thing; the internals index said it a third time.
One opening survives, the three load-bearing goals.

The rest is the last of the 'X was retired on 2026-08-XX' layer: the
unhighlighted-fence lint, the four margin-note aliases, the theme values, and
the TAL-* diagnostic codes. In every case the actionable half is kept and the
history dropped. theming.tmd's closing argument for why the reader's device
decides goes with them; the section stating that it does, stays."
```

---

### Task 12: Add the cheat sheet

**Files:**
- Create: `docs/guide/reference/cheatsheet.tmd` (~90 lines)
- Modify: `docs/guide/_site.yml` (add it first in the Reference part)

**This page documents only constructs that already ship.** Every row must be verifiable
against a validator const or a corpus document. Do not invent a construct, and do not
document a spelling a retirement register holds.

- [ ] **Step 1: Derive the vocabulary from the consts, not from memory**

```sh
grep -n 'CELL_OPTION_KEYS\|CALLOUT_KINDS\|INPUT_TYPES\|DIV_FEATURE_CLASSES\|XREF_LABELS' \
  crates/core/src/render/validate.rs crates/core/src/cite/render.rs
grep -n 'KNOWN_KEYS' crates/core/src/frontmatter.rs
```

Use what these return. `CLAUDE.md` is explicit that `vocab.rs` is the *offered-completions*
subset and under-reports, so answer "what does the tool support" from the validator consts.

- [ ] **Step 2: Write the page**

Front matter:

```yaml
---
title: "Cheat sheet"
description: "Every construct on one page: front matter, cells, divs, references and shortcodes, with a link to the chapter that explains each."
---
```

Six tables, each row `| syntax | what it does | chapter |`:

1. **Front matter**: the `KNOWN_KEYS` a reader actually sets (`title`, `subtitle`,
   `description`, `date`, `author`, `toc`, `theme`, `bibliography`, `image`, `draft`,
   `execute`, `listing`, `hero`).
2. **Cells**: ` ```{python} `, ` ```{js} `, ` ```{mermaid} `, ` ```{=html} `, and the `#|`
   / `//|` / `%%|` option prefixes.
3. **Cell options**: from `CELL_OPTION_KEYS`.
4. **Divs**: `::: {.callout-note}` with the `CALLOUT_KINDS`, plus the three
   `DIV_FEATURE_CLASSES` width escapes and `.column-margin`.
5. **References**: `[@key]`, `@fig-`, `@sec-`, `@tbl-`, `@eq-`, `@lst-`, `[^footnote]`,
   `{#anchor}`.
6. **Shortcodes**: `{{< include >}}`, `{{< input >}}`.

Open with one sentence: this is the whole surface, and anything not here is either plain
CommonMark or not supported.

- [ ] **Step 3: Verify the page renders and the gate accepts it**

```sh
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
cargo test -p taliesin-core --test stale_docs
```

Expected: exit 0, PASS. Task 1's cell-language gate fires if a table names a `{lang}` the
tool dropped, which is exactly the check this page needs.

- [ ] **Step 4: Commit**

```sh
git add -A docs/guide && git commit -m "docs: add a cheat sheet, every construct on one page

The one page a reader who will not spend time on a manual will actually
open, and the guide did not have it. Six tables covering front matter,
cells, cell options, divs, references and shortcodes, each row linking to
the chapter that explains it.

Every row is derived from the validator consts rather than from vocab.rs,
which CLAUDE.md records as the offered-completions subset that under-reports:
xrefPrefixes offers 5 of the 12 XREF_LABELS. The new cell-language gate
covers this page like any other."
```

---

### Task 13: Full verification and the read-through

**Files:**
- Modify: `notes/2026-08-14-docs-cut-design.md` (record the achieved figures)

- [ ] **Step 1: Run every gate**

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Expected: green, with the script's own verdict line reporting that every gate ran. **Take
the count from that line, never from prose.** A `SKIPPED` line means the run certifies less
than it appears to.

- [ ] **Step 2: Measure the result against the spec**

```sh
find docs -name '*.tmd' | xargs wc -l | tail -1
find docs/guide -name '*.tmd' | xargs wc -l | tail -1
find docs/internals -name '*.tmd' | xargs wc -l | tail -1
```

Spec targets: guide ~2,750, internals ~1,100, total ~3,850, from 5,952. Record the actual
figures in the spec. **If the total is off by more than 10%, say so plainly rather than
adjusting the target to match** (the spec was already corrected once, in that direction).

- [ ] **Step 3: Read both books end to end in a browser**

```sh
cargo run -p taliesin-server -- preview docs/guide 4388
```

Then `docs/internals`. No gate measures whether the result reads well, which is the actual
goal. Look for: a section that now opens mid-thought because its lead-in was cut, a
"see the chapter on X" pointing at something deleted, a chapter whose first paragraph
promises content that moved.

- [ ] **Step 4: Confirm the composed deploy still resolves**

```sh
tools/build-site.sh --check
```

Expected: exit 0. This is the one gate that fails on a broken link from `site/` into the
books, including `docs/guide/using/getting-started.html`.

- [ ] **Step 5: Commit the measurements**

```sh
git add notes/2026-08-14-docs-cut-design.md
git commit -m "notes: record what the docs cut actually achieved

Measured, not projected. Both books read end to end in a browser, since no
gate measures whether the result reads well."
```

- [ ] **Step 6: Raise the merge question, do not decide it**

Report to the author: the branch holds one commit per task. The project's convention is one
commit per unit of work on `main`. Ask whether to squash `docs-cut` into a single commit or
fast-forward the series, and do not merge without an answer.

---

## Self-review

**Spec coverage.** Every spec section maps to a task: the criterion drives Tasks 3 to 12;
the seven stale claims are Task 2; the Guide disposition table is Tasks 6 to 12; the
Internals table is Tasks 3 to 5; the truth gate is Task 1; hazard 1 (gate 5) is checked in
Tasks 6, 8 and 13; hazard 2 (census) in Tasks 5 and 11; hazard 3 (dangling links) by the
grep-first step opening every deletion task; hazard 4 (`_site.yml`) in each deletion task;
hazard 5 (internals under-exercised) by running `--check-only` on it in Tasks 3, 4 and 5.

**Deviation from the spec, recorded rather than silently taken.** The spec left open whether
the gate moves to `crates/server/tests/` or `COMMANDS` is re-exported. Neither: the gate
goes in the existing `crates/core/tests/stale_docs.rs` and reads `main.rs` as text.
`taliesin-server` is binary-only, so `crates/server/tests/` could not have imported
`COMMANDS` anyway, and `stale_docs.rs` already holds `const_block`, `string_literals` and
`retired_keys`, which do exactly this. This is strictly better than both options recorded
and needs no production change.

**Placeholder scan.** No TBD or TODO. Every prose replacement is written out in full. Two
steps carry conditional branches (Task 1 Step 3 on `render_document`'s export path, Task 9
Step 1 on which reference page a gate names); both state the check, the likely answer, and
what to do with each outcome, rather than deferring the decision.

**Type consistency.** The three new tests use only helpers verified present in
`stale_docs.rs` on 2026-08-14 at the signatures quoted (`read`, `shipped_docs`,
`backticked`, `const_block`, `string_literals`) plus one public function verified in
`crates/core/src/render/mod.rs` (`executes_to_kernel`). `const_block` expects the literal
shape `NAME: &[&str] = &[`, which `COMMANDS` in `main.rs:101` matches exactly.
