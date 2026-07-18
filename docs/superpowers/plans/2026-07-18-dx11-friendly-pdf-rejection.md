# DX11 — Friendly `.pdf`/wrong-target rejection — TDD plan

Spec: `docs/superpowers/specs/2026-07-18-dx11-friendly-pdf-rejection-design.md`.
Branch `dx11-friendly-pdf-rejection`. One S item: a pure guard in `parse_build_args`.

## Step 0 — reproduce (done, recorded in spec)

`target/debug/taliesin build methods.tmd methods.pdf` → `built methods.pdf`, exit 0,
`file` says HTML. This is the failing real-world behavior the tests below lock down.

## Step 1 — RED: failing unit tests in `crates/server/src/build.rs`

Add a `#[cfg(test)] mod dx11_tests` (or extend `jobs_tests`) with:

1. `non_html_output_error_flags_format_extensions`
   - `non_html_output_error(Some("methods.pdf"))` is `Some(m)` where `m` contains
     `.pdf`, `HTML only`, `methods.html`, and `Print`.
   - `non_html_output_error(Some("out.PDF"))` is `Some` (case-insensitive).
   - `non_html_output_error(Some("slides.pptx"))`, `("paper.docx")`, `("x.tex")`,
     `("x.typ")`, `("x.md")` are each `Some`.
   - `non_html_output_error(Some("page.html"))`, `("page.htm")`, `("draft")`
     (extensionless), `("notes.txt")`, and `None` are each `None`.
2. `parse_build_args_rejects_pdf_output`
   - `parse_build_args(&argv("build doc.tmd out.pdf"))` is `Err(m)` with `m` naming
     `.pdf` and `HTML only`.
   - `parse_build_args(&argv("build doc.tmd out.html"))` is `Ok(a)` with
     `a.out_html == Some("out.html")` (regression: a valid HTML target still parses).

`argv(s)` helper: `std::iter::once("taliesin").chain(s.split(' ')).map(String::from).collect()`.

Run `cargo test -p taliesin-server --lib dx11` (or the module name) → RED (helper does not
exist / parse succeeds).

## Step 2 — GREEN: implement in `crates/server/src/build.rs`

- `const NON_HTML_OUTPUT_EXTS: &[&str] = &["pdf","docx","doc","odt","rtf","tex","latex","typ","epub","pptx","ppt","md","markdown"];`
- `fn non_html_output_error(out_html: Option<&str>) -> Option<String>`:
  - `let out = out_html?;`
  - `let ext = Path::new(out).extension()?.to_str()?.to_ascii_lowercase();`
  - `if !NON_HTML_OUTPUT_EXTS.contains(&ext.as_str()) { return None; }`
  - `let html = Path::new(out).with_extension("html");`
  - `Some(format!("error: `build` renders HTML only, but the output path `{out}` ends in \
     `.{ext}`. Write `{}` instead (or omit it to build `{}` beside the source). For a rough \
     PDF, open the built page and use your browser's Print to PDF; a real print/PDF track is \
     planned (ROADMAP Pillar IV).", html.display(), html.display()))`
    - Prefix `error: ` inside the helper so `cmd_build`'s `eprintln!("{msg}")` matches the
      other parse errors. (The `--jobs`/`--out` errors are `error: `-prefixed; the missing-path
      usage line is not — this is an error, so prefix it.)
- In `parse_build_args`, right after `let path = positionals.first()...?;`:
  ```rust
  if let Some(msg) = non_html_output_error(positionals.get(1).copied()) {
      return Err(msg);
  }
  ```

Run the Step 1 tests → GREEN.

## Step 3 — RED→GREEN: end-to-end pin in `crates/server/tests/strict_robustness.rs`

Add `build_into_pdf_is_rejected`:
- `tmp_dir("pdftarget")`; write `post.tmd` (`---\ntitle: P\n---\n\nProse.\n`).
- Run `taliesin build post.tmd post.pdf` (cwd or absolute paths).
- Assert: `!status.success()`; `!post.pdf.exists()` (nothing written); stderr contains
  `.pdf`, `HTML only`, and `ROADMAP` (and, defensively, `Print`).
- Cleanup `remove_dir_all`.

`cargo test -p taliesin-server --test strict_robustness` → GREEN.

## Step 4 — full gate

- `cargo test -p taliesin-core -p taliesin-server` (whole net, not just the new tests).
- `cargo fmt --check` (the PostToolUse hook keeps edited files clean; confirm the tree).
- `cargo clippy -p taliesin-server -- -D warnings` (the const + helper must not trip an
  unused/format-string lint — DX5 hit a cfg(test)-only re-export clippy error, so watch this).
- **Manual real-binary check** (the audit is about human-facing UX): rebuild
  `cargo build -p taliesin-server`, then `taliesin build methods.tmd methods.pdf` prints the
  friendly message, exits non-zero, and leaves no `methods.pdf`; `taliesin build methods.tmd
  methods.html` still works. No browser UI in this change, so no chrome-devtools loop.

## Step 5 — land + record

- `git commit` (spec + plan + code + tests) on the branch; fast-forward `main` to it.
- Delete the DX11 backlog entry in `notes/backlog.md` §6; update the "Pick up here" /
  suggested-order lines so the next item is the DX10-followup (`new deck --tour`).
- Append a DX11 closure note to `notes/AUDITS.md` (matching the DX5 note's shape).
- Do **not** push (author pushes on request).

## Risk / rollback

Tiny surface (one const, one pure fn, one `Err` branch, tests). If the denylist proves noisy,
it is one line to trim. No invariant touched; `git revert` the single commit fully undoes it.
