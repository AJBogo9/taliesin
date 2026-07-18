# DX11 — Friendly `.pdf`/wrong-target rejection

Date: 2026-07-18. Backlog item **DX11** (§6 DX audit batch, Tier 1 — the second
silent-failure trap, after DX5). Branch `dx11-friendly-pdf-rejection`. Detail source:
`notes/2026-07-18-dx-audit.md` (row 11; persona 🎓 Priya's abandonment moment).

> **Autonomy note:** author asked me to continue without the interactive gate. The design
> fork (denylist vs. allowlist, which extensions, where to validate) is resolved below with
> documented defaults, not an `AskUserQuestion`.

## Goal

Kill the *silent-wrong-output* trap: `taliesin build methods.tmd methods.pdf` today writes
**HTML bytes into `methods.pdf`, logs `built methods.pdf`, and exits 0**. The academic opens
a "PDF" full of `<!DOCTYPE html>` and concludes the tool is broken (the audit's single
worst abandonment moment). Reject a format-implying output extension with a friendly message
that (a) states Taliesin renders HTML only, (b) hands them the concrete `.html` fix, (c)
offers the browser Print-to-PDF escape hatch, and (d) notes a real print track is planned
(ROADMAP Pillar IV / Wave 5). Write nothing; exit non-zero.

## Ground truth (grepped + measured against the running product 2026-07-18)

- **Reproduced the bug** on `target/debug/taliesin`:
  `build methods.tmd methods.pdf` → `built methods.pdf`, **exit 0**, and `file methods.pdf`
  reports `HTML document, UTF-8 text`. The `.pdf` is HTML.
- **The output path is written verbatim, no extension check**
  ([`build.rs:231-234`](../../../crates/server/src/build.rs)): the second positional
  `out_html` becomes a `PathBuf` and is handed straight to `std::fs::write`. The default
  (no second positional) is `base.join("{stem}.html")` — HTML beside the source.
- **Arg parsing is a pure, unit-testable function**
  ([`parse_build_args`, `build.rs:71`](../../../crates/server/src/build.rs)) that already
  returns `Err(ready-to-print message)` for a value-less `--out`, a bad `--jobs`, an unknown
  `--flag` (did-you-mean), and a missing path. `cmd_build` prints that `Err` to stderr and
  returns `ExitCode::FAILURE` ([`build.rs:155-159`](../../../crates/server/src/build.rs)).
  This is the natural home for one more validation.
- **Precedent for the message + a curated non-HTML list.**
  [`frontmatter.rs:330`](../../../crates/core/src/frontmatter.rs) already carries
  `NON_HTML_FORMATS = [pdf, typst, docx, latex, beamer, epub, pptx, odt, rtf, jats, docbook]`
  and warns `format \`{n}\` is not supported (Taliesin renders HTML only)` when a carried-over
  `format:` field names one. **DX11 is the CLI-output-path analog of that front-matter rule.**
  (The two lists differ: front-matter matches format *names* [typst, beamer, jats]; DX11
  matches output-path *file extensions* [typ, tex, .md], so they stay separate consts in
  their own crates. Same spirit, honest phrasing shared.)
- **Nothing depends on writing a non-HTML output.** `grep` for `.pdf`/`.docx`/`.tex` output
  targets across `crates/*/tests` finds only `fig-export: figures/x.pdf` (a figure path,
  unrelated). No test builds a page into a denylisted extension, so the guard breaks nothing.
- **The end-to-end pin has a home.**
  [`crates/server/tests/strict_robustness.rs`](../../../crates/server/tests/strict_robustness.rs)
  already exercises the real binary for the *sibling* guardrails ("unknown `--flag` is a hard
  error", "value-less `--out` is a hard error"). DX11's ".pdf output is a hard error" slots in
  beside them.

## Resolved decisions (autonomous, documented)

1. **Denylist, not allowlist.** Reject a *curated set of format-implying extensions*, not
   "anything that is not `.html`". An allowlist would also reject harmless, deliberately-named
   targets — an extensionless `build doc.tmd draft`, a `build doc.tmd page.htm`, a `.xhtml` for
   a picky host — none of which are format-expectation traps (the user named the file; they get
   HTML content in it, as asked). The abandonment moment is specifically *"I asked for a PDF/DOCX
   and got garbage."* Target that. **Accepted tradeoff:** a rare format-implying extension not in
   the list slips through (writes HTML, exit 0, as today). Those are low-frequency and low-harm,
   and the const is one line to extend.
2. **Extension set** (case-insensitive), the pandoc/Quarto refugee's muscle memory, mirroring
   `NON_HTML_FORMATS` translated to file extensions plus the source-format round-trip attempts:
   `pdf, docx, doc, odt, rtf, tex, latex, typ, epub, pptx, ppt, md, markdown`.
   - `md`/`markdown`: a `build doc.tmd doc.md` round-trip attempt (Taliesin is `.tmd`→HTML;
     there is no HTML→md path) writes HTML into a `.md` — same trap, same message.
   - **Not** included: `.txt`, `.xml`, `.json`, extensionless. `.txt` is not a
     format-conversion expectation (HTML-in-a-txt is odd but the user named it); the rest are
     not doc-compiler targets. Kept permissive on purpose (decision 1).
3. **Where: a pure helper called from `parse_build_args`.** Add
   `fn non_html_output_error(out_html: Option<&str>) -> Option<String>` (returns the
   ready-to-print message when the second positional's extension is denylisted). Call it in
   `parse_build_args` on `positionals.get(1)` and return its `Some` as the parse `Err`.
   - Purely unit-testable (mirrors `parse_jobs_value`), and consistent with the other
     Err-returning validations already living there.
   - Validated *whenever a denylisted second positional is present*, even alongside `--out`
     (where `out_html` is ignored today — a contradictory `build doc.tmd x.pdf --out dist`).
     A `.pdf` second positional always signals a misunderstanding of the output model, so
     surfacing it is more correct than silently honoring `--out`.
4. **Behavior: hard error, write nothing.** Exit `ExitCode::FAILURE` via the existing
   `cmd_build` `eprintln!(Err)` path; no file is created. Message is `error: `-prefixed to match
   the other parse errors.
5. **Message** (no em/en dashes, per house style; names the extension + the `.html` fix +
   the escape hatch + the planned track). For `out = "methods.pdf"`:
   > `error: `build` renders HTML only, but the output path `methods.pdf` ends in `.pdf`.`
   > `Write `methods.html` instead (or omit it to build `methods.html` beside the source).`
   > `For a rough PDF, open the built page and use your browser's Print to PDF; a real`
   > `print/PDF track is planned (ROADMAP Pillar IV).`

   The `.html` suggestion is the out path with its extension swapped
   (`Path::with_extension("html")`), so it is correct for `dist/methods.pdf` →
   `dist/methods.html` too.

## Changes

### `crates/server/src/build.rs`
- Add `const NON_HTML_OUTPUT_EXTS: &[&str]` (the 13 extensions above), near `BUILD_FLAGS`.
- Add `fn non_html_output_error(out_html: Option<&str>) -> Option<String>`: lower-cases the
  second positional's extension; if in `NON_HTML_OUTPUT_EXTS`, build the friendly message
  (with the `with_extension("html")` suggestion) and return `Some`; else `None`.
- In `parse_build_args`, after computing `path`, run the helper on `positionals.get(1)` and
  `return Err(msg)` on a hit (before constructing `BuildArgs`).

### `crates/server/src/build.rs` (tests, `#[cfg(test)]`)
- Unit test `non_html_output_error`: `Some("out.pdf")` → message naming `.pdf`, `HTML only`,
  `out.html`, and `Print`; `Some("out.PDF")` (case) → `Some`; `Some("out.html")` /
  `Some("out.htm")` / `Some("draft")` (extensionless) / `Some("out.txt")` / `None` → `None`.
- Unit test `parse_build_args(["taliesin","build","doc.tmd","out.pdf"])` → `Err` naming the
  extension and `HTML only`; the same argv with `out.html` → `Ok` with `out_html = Some("out.html")`.

### `crates/server/tests/strict_robustness.rs`
- End-to-end pin `build_into_pdf_is_rejected`: `build doc.tmd doc.pdf` on the real binary →
  non-zero exit, **`doc.pdf` is not created**, stderr contains `.pdf` + `HTML only` +
  `ROADMAP` (or `Print`). Mirrors the sibling unknown-flag / value-less-`--out` tests.

## Non-goals

- **Producing a real PDF.** Sanctioned separately as ROADMAP Pillar IV / Wave 5
  (`print-pdf-track`, *derived from* the built HTML). DX11 is only the interim guardrail.
- **Validating `--out <dir>` values.** `--out` is a directory; a `.pdf` there is a strange
  dirname, not the write-into-a-file trap. Left alone.
- **Allowlist / rejecting every non-HTML extension.** Explicitly rejected (decision 1): would
  police harmless deliberately-named targets (extensionless, `.htm`, `.xhtml`).

## Invariant safety

CLI arg-validation only. No output-format change, no rendering change, no CDN, no preview
write-back. `data-block-id`/`data-sourcepos`, `MAX_WARM_PAGES` + `exec_pool.rs` LRU freeze all
untouched. A pure function + one `Err` branch in an already-`Err`-returning parser.
