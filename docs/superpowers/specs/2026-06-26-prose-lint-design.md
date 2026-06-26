# Prose-lint diagnostics (design)

Date: 2026-06-26
Status: approved (brainstorm), pre-implementation
Feature branch: `feat/prose-lint`
Pillar: BEYOND-QUARTO.md Pillar I (authoring intelligence — the validation moat) +
FEATURE-IDEAS.md #29.

## Summary

An opt-in, markdown-aware **prose linter** that emits located, click-to-source warnings into
the existing diagnostics channel — the Vale/Hemingway loop without a second tool or a network
call. A doc turns it on with `prose-lint: true` in front-matter; the scanner flags three
high-precision rules — **doubled words**, **weasel words**, and a **custom banned-terms
list** — skipping code, math, links, inline code, and HTML so only prose text is checked. It
is diagnostic-only: rendering is unaffected.

This extends qmd-fast's rigorously-linted-config moat to prose, in a way a batch compiler
cannot match: live, located, offline.

## Goals

- `prose-lint: true` (or `prose-lint: { banned: [...] }`) enables the linter for a doc; absent
  / `false` leaves it off.
- Three rules, all low-false-positive: doubled words (`the the`), a curated weasel-word list,
  and the author's custom banned terms.
- Markdown-aware: never flags text inside fenced code, inline code, `$…$`/`$$` math, link/image
  URLs, autolinks, or HTML tags.
- Each finding is a located `Warning` (file + source line) flowing through the same channel as
  broken xrefs / unknown shortcodes, so it jumps to source in the dev panel and prints in
  `build`.
- `prose-lint` (and its `banned` subkey) are themselves validated (added to the known-key sets,
  so a typo'd key is flagged with did-you-mean).

## Non-goals (v1, YAGNI)

- **No passive-voice rule** (its `is/was + -ed` heuristic is noisy/opinionated; deferred).
- **No on-by-default.** Prose voice is subjective; off unless a doc opts in (keeps the corpus +
  every existing doc quiet).
- **No rendering change.** Diagnostic-only, exactly like the schema validators.
- **No external wordlist / dictionary / network.** The weasel list is a small inline constant.
- **No per-rule severity, autofix, or column-precision** (line precision matches the rest of
  the diagnostics channel; whole-word match).
- **No new client/JS, no browser surface of its own** — it rides the existing warning display.

## Invariants honoured

- **HTML-only**, **offline** (inline wordlist; no download).
- **Diagnostic-only / read-only:** never mutates blocks, ids, sourcepos, or rendered HTML.
- **Rides the diagnostics channel** (`Vec<Warning>` + `map_origin` located warnings) and the
  front-matter known-key machinery. Do-NOT-touch machinery (`cite.rs`, `includes.rs`,
  numbering, exec/freeze/kernel, the `:::` scanner) untouched.
- **Single editing surface:** it only *navigates* to source (located warning), never writes.

## Configuration (front-matter)

```yaml
prose-lint: true                       # built-in rules on
# or
prose-lint:
  banned: [utilize, leverage, synergy] # on + custom banned terms
# absent / false                       # off
```

- `prose-lint` added to `frontmatter::KNOWN_KEYS`.
- `PROSE_LINT_KEYS = &["banned"]`; `validate_front_matter` calls
  `validate_nested(map, "prose-lint", "prose-lint option", PROSE_LINT_KEYS, block, out)` so
  `prose-lint: { bnned: [...] }` is flagged (did-you-mean), mirroring `execute:`/`listing:`.

### Parsing

`prose::config(front_matter: &str) -> Option<ProseLint>`:
- parse the front-matter YAML; read `prose-lint`.
- `Value::Bool(true)` → `Some(ProseLint { banned: vec![] })`.
- `Value::Mapping` → `Some(ProseLint { banned: <string items of `banned`> })`.
- `Bool(false)` / absent / other → `None` (linter off).

```rust
pub(crate) struct ProseLint {
    pub banned: Vec<String>,
}
```

## The scanner (`crates/core/src/prose.rs`)

`pub(crate) fn lint(src: &str, cfg: &ProseLint) -> Vec<(usize, String)>` — returns
`(1-based source line, message)` pairs. Pure (no IO), so unit-testable.

Walk `src` line by line (1-based), tracking state:
- **Front-matter:** skip the leading `---` … `---`/`...` block.
- **Fenced code:** track open/close fences (reuse the `next_code_state`/`code_fence` helpers
  from `divs.rs`, made `pub(crate)`); skip lines inside a fence and the fence lines.
- **Per prose line — strip before matching** (replace each span with spaces to keep it simple;
  only the line number matters): inline code spans (`` `…` `` / ` ``…`` `), `$$…$$` and `$…$`
  math, markdown link/image targets `](…)`, autolinks `<…>`, and HTML tags `<…>`. The
  remaining text is the prose to lint.
- **Rules** on the stripped text (case-insensitive, word-boundary):
  - **doubled words**: regex `(?i)\b([a-z']+)\s+\1\b` → `repeated word \`the\``. (One match per
    occurrence.)
  - **weasel words**: each occurrence of a word in `WEASEL_WORDS` → `weasel word \`very\`
    (consider cutting)`.
  - **banned terms**: each occurrence of a `cfg.banned` term → `banned term \`utilize\``.

`WEASEL_WORDS` (curated, conservative): `very, really, quite, just, actually, basically,
simply, clearly, obviously, essentially, fairly, somewhat, rather`.

Multiple findings on one line are all emitted (in column order); the message names the
offending word so the author can act without column precision.

## Wiring (`render/mod.rs`)

In `render_internal_impl`, after the front-matter block is in hand and after the existing
`validate_front_matter` call:

```rust
if let Some(cfg) = crate::prose::config(fm) {
    for (line, msg) in crate::prose::lint(src, &cfg) {
        let (file, l) = map_origin(origins, line);
        warnings.push(Warning::new(msg).at(file, l));
    }
}
```

(`fm` = the doc's front-matter block; `src` = the include-expanded source already used for the
other passes; `origins` maps expanded lines back to their origin file/line. So included prose
is linted and located correctly.)

## Tests

1. **`prose.rs` unit tests** (pure): doubled / weasel / banned each fire with the right
   message + line; code-fence, inline code, `$math$`, and a link URL are NOT flagged; an empty
   `banned` list yields no banned warnings; `config()` returns `None` for absent/false and
   `Some` with the right `banned` for the map form.
2. **`crates/core/tests/prose_lint.rs`**: render `corpus/diagnostics/prose.qmd` and assert the
   exact set of located prose warnings (each carries a line), and that a code/math/link line is
   not among them. Mirrors `nested_validation.rs`.
3. **Corpus invariants** (auto): the pin doc renders; `corpus/diagnostics/` is exempt from the
   clean-front-matter / unknown-key guards.

No browser test — the feature is entirely server-side and Rust-testable; the dev panel/build
surface it through the existing warning-printing path (unchanged).

## Corpus pin

`corpus/diagnostics/prose.qmd`:

```markdown
---
title: "Prose lint"
prose-lint:
  banned: [utilize]
---

We we should fix this doubled word.   <!-- doubled: we -->

This is very fast and really clever.  <!-- weasel: very, really -->

Please utilize the new API.           <!-- banned: utilize -->

`utilize` in code and $very$ in math and [very](https://very.example) in a link
must NOT be flagged.                   <!-- markdown-awareness -->

```python
# utilize very very   <-- inside a fence: NOT flagged
```
```

(The exact trip-words are finalized in the plan to match the asserted warning set.)

## Risks & mitigations

- **False positives** — mitigated by shipping only the three high-precision rules; passive
  voice (the noisy one) is deferred; weasel list is conservative + small.
- **Markdown-stripping gaps** (a span the stripper misses → a false flag) — covered by the pin
  doc's code/math/link line, asserted NOT to warn; the stripper replaces spans with spaces
  (line-preserving) and is unit-tested.
- **Linting included files under the host's setting** — intended (the prose is the author's);
  located via `map_origin` to the include's file.
- **`next_code_state` visibility** — make the two `divs.rs` fence helpers `pub(crate)` (small,
  additive) rather than duplicating the fence logic.

## Out of scope follow-ups (recorded, not built)

- Passive-voice rule (behind its own opt-in sub-flag).
- Readability score / sentence-length / reading-grade.
- A `qmd-fast check` CLI emitting these as JSON/SARIF (FEATURE-IDEAS #39).
- Repeated-sentence-start / adverb-density / cliché rules.
- Surfacing prose warnings inline in the rendered output (kept author-facing only).
