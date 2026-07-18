# DX10 — Teaching scaffolds: Implementation Plan

> REQUIRED SUB-SKILL: superpowers:executing-plans. Checkbox steps. Scope: sub-parts 1-3 (paper
> worked example, init→new pointer, `new post --draft`); `--tour` deferred (see spec Non-goals).

**Goal:** scaffolds teach the format — a worked `{python}` figure in `paper`, an `init` index that
points at `taliesin new`, and `new post --draft`.

**Files:** `crates/server/src/cli.rs` (templates + `NewOpts` plumbing), `crates/server/tests/new_cli.rs`
(assertions), `corpus/scaffold/posts/my-paper/index.tmd` (regenerated mirror).

## Global Constraints
- Default-off `--draft` → today's scaffolds stay byte-identical (mirror + existing tests unchanged).
- All literal `{`/`}` in `format!` templates escaped (`{{python}}`, `{{#sec-methods}}`).
- Every scaffold stays `check`-clean (new_cli.rs runs the real binary + `taliesin check`).
- `cargo fmt`-clean; the `corpus/scaffold/` mirror regenerated from the real `new` output.

---

### Task 1: `paper` worked example

- [ ] **Step 1 (failing test):** extend `new_cli.rs` `a_paper_ships_its_bibliography_so_citations_resolve` with:
```rust
    assert!(src.contains("```{python}"), "paper shows a runnable code cell");
    assert!(src.contains("#| label: fig-"), "paper labels a figure the Quarto way");
    assert!(src.contains("@fig-"), "paper cross-references its figure");
    assert!(src.contains("$$"), "paper shows display math");
```
- [ ] **Step 2:** run → FAIL. `cargo test -p taliesin-server --test new_cli a_paper_ships_its_bibliography`
- [ ] **Step 3:** rewrite the `NewKind::Paper` `index` template in `new_files` (cli.rs ~L254) to the worked example (intro cites `[@knuth1984literate]` + refs `@sec-methods`; a `## Methods {{#sec-methods}}`; a `{{python}}` matplotlib cell `#| label: fig-demo` + `#| fig-cap:`; `@fig-demo` ref; a `$$ y = x^2 $$`; `# References`). Escape all literal braces.
- [ ] **Step 4:** run → PASS.
- [ ] **Step 5:** regenerate the corpus mirror from the real output:
```bash
cargo build -p taliesin-server
D=$(mktemp -d); ./target/debug/taliesin new paper my-paper --dir "$D" >/dev/null
cp "$D/posts/my-paper/index.tmd" corpus/scaffold/posts/my-paper/index.tmd
diff "$D/posts/my-paper/references.bib" corpus/scaffold/posts/my-paper/references.bib   # expect no change
rm -rf "$D"
```
- [ ] **Step 6:** `cargo test -p taliesin-core` (renders + lints the new mirror) + `cargo test -p taliesin-server --test new_cli`. Both green.
- [ ] **Step 7:** integration: `./target/debug/taliesin check <scaffolded paper>` → `0 problems` (ignore the informational Environment/ipykernel line).
- [ ] **Step 8:** commit `feat(new): paper scaffolds a worked {python} figure + math + cross-refs (DX10)`.

---

### Task 2: `init` index points at `taliesin new`

- [ ] **Step 1:** append a bullet to `INIT_INDEX_TMD` (cli.rs ~L24) Next-steps list:
  `- Start a post with \`taliesin new post my-first-post\` (add \`--draft\` to hold it back).\n`
- [ ] **Step 2:** `cargo build -p taliesin-server`; `init` into a temp dir; `serve`/`check` it → index parses, no new warning. (The init test already asserts the index has front matter.)
- [ ] **Step 3:** commit `feat(init): index.tmd points new users at \`taliesin new\` (DX10)`.

---

### Task 3: `new post --draft`

- [ ] **Step 1 (failing test):** add to `new_cli.rs`:
```rust
#[test]
fn new_post_draft_marks_it_a_draft_and_stays_clean() {
    let dir = tmp("draft");
    let (ok, _, stderr) = run(&["new", "post", "wip", "--draft", "--dir", dir.to_str().unwrap()]);
    assert!(ok, "stderr: {stderr}");
    let src = std::fs::read_to_string(dir.join("posts/wip/index.tmd")).unwrap();
    assert!(src.contains("draft: true"), "--draft sets draft: true:\n{src}");
    let (clean, diags) = check_is_clean(&dir.join("posts/wip/index.tmd"));
    assert!(clean, "a fresh --draft post must check clean:\n{diags}");
    // Default (no flag) stays draft-free.
    let (ok2, ..) = run(&["new", "post", "pub", "--dir", dir.to_str().unwrap()]);
    assert!(ok2);
    let plain = std::fs::read_to_string(dir.join("posts/pub/index.tmd")).unwrap();
    assert!(!plain.contains("draft:"), "no --draft → no draft key:\n{plain}");
    let _ = std::fs::remove_dir_all(&dir);
}
```
- [ ] **Step 2:** run → FAIL (`--draft` is an unknown flag today).
- [ ] **Step 3:** implement `NewOpts`:
  - `#[derive(Clone, Copy, Default)] struct NewOpts { draft: bool }`.
  - `new_files(kind, slug, today, opts: NewOpts)`: add a `{draft}` slot after each `title:` line, where `let draft = if opts.draft { "draft: true\n" } else { "" };` (escape as `{draft}` in each `format!`). Paper's early-return branch too.
  - `write_new(root, kind, slug, opts)` threads it; `cmd_new` parses `"--draft" => opts.draft = true` and adds `--draft` to `NEW_FLAGS`.
- [ ] **Step 4:** run → PASS; and `cargo test -p taliesin-server --test new_cli` (all cases, incl. the byte-default guard) green.
- [ ] **Step 5:** `cargo test -p taliesin-core` still green (default-off keeps the mirror valid).
- [ ] **Step 6:** commit `feat(new): --draft flag marks a scaffold draft: true (DX10)`.

---

### Task 4: full gate

- [ ] `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`, `cargo clippy -p taliesin-server --all-targets -- -D warnings` — all green.

## Self-Review
- Spec coverage: paper worked example → T1; init pointer → T2; `--draft` → T3; `--tour` explicitly deferred (spec Non-goals).
- Placeholders: none. Type consistency: `NewOpts`/`new_files`/`write_new` signatures updated together (T3); template brace-escaping noted.
