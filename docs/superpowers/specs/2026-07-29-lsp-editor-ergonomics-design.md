# LSP editor ergonomics: the doc-local semantic layer

**Date:** 2026-07-29
**Backlog:** items 177 (this work) and 178 (its prerequisite).
**Idea pool:** `notes/FEATURE-IDEAS.md` Session 3, ideas 67-72 (Cluster A). Clusters B-F
(ideas 73-86) are parked there and are **out of scope here**.

## Why this exists

The author asked for a deep pass over the VS Code extension API and a brainstorm of what would
"supercharge the development experience", and separately asked whether LaTeX `$` delimiters should
be highlighted. Twenty ideas came out of that pass. This spec covers only the slice that is a pure
function of the open buffer, lands entirely in Rust, and therefore reaches every editor that
speaks LSP rather than VS Code alone.

## Ground truth

Eight facts were verified in source during the design pass and are recorded with `file:line` in
`notes/FEATURE-IDEAS.md` Session 3. Four of them decide this spec:

- **`RenderedDoc.xref_numbers` already exists** (`crates/core/src/render/model.rs:266`) and is
  already read by hover (`crates/server/src/lsp.rs:1323`). The resolved figure/section number is
  computed today, so the inlay hint that sounds hardest is nearly free.
- **`documentSymbol` already computes whole-section ranges** (`lsp.rs:1453`: `range` is the whole
  section, `selection_range` the heading line). Folding is a re-projection, not new analysis.
- **Unresolved xrefs already produce diagnostics** (`crates/core/src/cite/validate.rs`,
  `validate_xrefs_known_elsewhere` at `crates/server/src/check.rs:311`), published on every change.
- **`didChange` → `publish` is synchronous and undebounced** (`lsp.rs:273-283`), and that path runs
  a full `render_single_doc` **plus** `site::anchors_defined_elsewhere_in_project`
  (`crates/core/src/site/xref.rs:111`), which walks every page in the project, reads each from
  disk, and resolves its includes.

**Two corrections were made mid-design and are load-bearing.** They are recorded here so the
superseded versions are not reintroduced:

1. "The LSP is document-local" is **false** for the diagnostic path. `_site.yml` discovery
   (`enclosing_site_root`) and a project-wide anchor set already exist and already run.
2. "Semantic tokens make a dangling ref red as you type" is **redundant**. Diagnostics already do
   exactly that, and they do it correctly.

## What is in scope

Five features, in build order. Each is independently shippable and each has its own pin.

| Phase | Feature | LSP capability | Depends on |
|---|---|---|---|
| 0 | Math delimiter visibility | none (theme contribution) | nothing |
| 1 | Debounce + render memo (item 178) | none | nothing |
| 2 | Inlay hints | `inlayHintProvider` | phase 1 |
| 3 | Folding ranges | `foldingRangeProvider` | nothing |
| 4 | Document highlight | `documentHighlightProvider` | nothing |
| 5 | Selection ranges | `selectionRangeProvider` | nothing |

### Explicitly cut, with the reason

- **Semantic tokens (idea 67).** Its stated value was error surfacing, which diagnostics already
  own. The residue (distinguishing a locally-defined anchor from one defined on another page, both
  valid, neither warning) is real but unevidenced, and the math-visibility case it might have
  carried is served by phase 0 far more cheaply. Deferred to the idea pool, not deleted.
- **Document colour provider (idea 72).** Lowest value in the cluster, no demand.
- **Taliesin does not ship a VS Code colour theme.** An editor theme is the user's own choice; the
  project's minimal-config convention says perfect the default rather than seize the setting. Phase
  0 contributes narrowly-scoped token-colour *defaults* the user can override instead.

## Architecture

Everything lands in `crates/server/src/lsp*.rs`. **Zero new TypeScript.** The companion is already
a thin `vscode-languageclient`, and the client registers inlay hints, folding, document highlight
and selection ranges automatically the moment the server advertises those capabilities. The only
`editor/vscode` change is phase 0's `package.json` contribution.

This matters beyond tidiness: it is the project's standing rule that editor features go in Rust,
and it means the Zed / Neovim / Helix setups the CLI reference documents get all four features
with no extra work.

### Phase 0: math delimiter visibility

**Root cause, verified.** `editor/vscode/syntaxes/tmd.injection.tmLanguage.json` already scopes the
delimiters as `punctuation.definition.math.begin.tmd` / `.end.tmd`, the spans as
`markup.math.inline.tmd` / `markup.math.display.tmd`, and the body as `meta.embedded.math.tmd`.
That is the *same shape* VS Code's own `markdown-math` extension uses, differing only in the
trailing `.markdown` vs `.tmd`. **No bundled theme defines any rule for these scopes** (checked
across `dark_plus`, `light_plus`, `dark_vs`, `light_vs`, `hc_black`, `hc_light`, `2026-dark`,
`2026-light`: the only "math" scope any of them paints is `support.constant.math`, which is a
JavaScript `Math.*` rule and unrelated). Built-in Markdown math is invisible in the default themes
for exactly this reason. **The grammar is correct and ecosystem-aligned; the themes are empty.**

**Fix.** Contribute `editor.tokenColorCustomizations` → `textMateRules` in
`contributes.configurationDefaults`, targeting only the `.tmd`-suffixed math scopes. The suffix is
Taliesin's own, so although `editor.tokenColorCustomizations` is a window-level setting and cannot
be nested inside the existing `"[taliesin]"` block, the rules are inert in every other language.
The user overrides them like any other default.

**Colour choice, decided rather than left open.** Contribute exactly **one** rule, on
`punctuation.definition.math.begin.tmd` and `punctuation.definition.math.end.tmd` only, setting
`"fontStyle": "bold"` and **no foreground**. Rationale:

- The author's stated need is *seeing where math starts and ends*, which is a delimiter concern.
  Recolouring `markup.math.*` or `meta.embedded.math.tmd` would restyle the LaTeX body, which the
  `#math_body` patterns already colour correctly.
- Omitting a foreground means the rule inherits whatever the active theme uses for surrounding
  text, so it is legible in light, dark and both high-contrast themes without Taliesin picking a
  colour. This sidesteps the vendor-hex discipline entirely rather than trying to honour it in a
  file the banning test cannot reach.
- One rule is trivially overridable by a user who wants something else.

If bold alone proves too subtle in practice, the escalation is a foreground drawn from the
project's own OKLCH accent, **not** a vendor hex, and it needs the phase 0 editor check re-run.

### Phase 1: debounce + render memo (item 178)

Two independent changes, both prerequisites for phase 2 being pleasant, and both defects on their
own terms.

**Memo.** A one- or two-entry cache of `(buffer_text, RenderedDoc)` behind `render_buffer`
(`lsp.rs:1311`), shared by `publish`, hover, completion and the new providers. **Keyed on the text
itself**, which is the whole point: a different buffer is a different key, so there is no
invalidation logic, no version tracking and no staleness class to get wrong. The same memo should
cover `anchors_defined_elsewhere_in_project`, keyed on the project root plus a cheap generation
signal.

**Debounce.** `main_loop` is `for msg in &connection.receiver` (`lsp.rs:139`), a blocking iterator,
so coalescing `didChange` requires either `recv_timeout` in a hand-rolled loop or a publish thread.
**This restructures the main loop and is the riskiest change in the spec**, which is why it is
priced as its own phase rather than folded into a feature.

**Do not "fix" this by deleting the project walk.** It exists so a valid cross-page
`@sec-`/`@fig-`/`@tbl-` is not reported as an error (`check.rs:305-310`); removing it regresses a
fixed bug.

**Measure first.** No number has been taken. The cost estimate above is reasoning from the code
shape, not from a benchmark, and the fix should be chosen against a real measurement on the largest
book in the tree.

### Phase 2: inlay hints

`textDocument/inlayHint` is **range-scoped**: the request carries the visible range. That means a
per-line scan over that range suffices and **no full-document scanner is needed**, which is what
decouples this phase from the cut semantic-tokens work.

Hints:

- `@fig-`/`@tbl-`/`@sec-`/`@eq-` refs render their resolved number from the memoized
  `RenderedDoc.xref_numbers`.
- `[@key]` renders author-year, from the front-matter `.bib` the LSP already reads.
- `{{< include path >}}` renders the included file's line count.

`resolveInlayHint` fills tooltips lazily so the common path stays cheap.

**Known limitation, by design.** `xref_numbers` is page-local, so a reference to an anchor defined
in another chapter has no number to show. Such a reference is *valid* (the diagnostic path knows
it, via the project anchor set) but unnumbered here. **Omit the hint rather than render a
placeholder**: a missing hint reads as "no information", whereas `⟨elsewhere⟩` reads as a claim.
Revisit if the Cluster C index (idea 74) ever lands.

### Phase 3: folding ranges

Sections come free from the existing symbol tree. Add `:::` fenced divs, YAML front matter, and
code fences, all of which existing scanners already delimit. Kind `region` throughout except front
matter.

This replaces indentation-based folding, which is the current behaviour (there is no `folding` key
in `editor/vscode/language-configuration.json` and no server capability) and is simply wrong for a
Markdown-derived format.

### Phase 4: document highlight

Needs the id under the cursor plus its occurrences, so it is a **targeted single-id scan**, not a
full-document tokenizer. `lsp_nav::classify_target` supplies the id; one pass finds the rest. Mark
the definition site `DocumentHighlightKind::Write` and references `Read`.

### Phase 5: selection ranges

A nesting chain at the position: word → inline construct (math / xref / cite / link) → sentence →
paragraph → `:::` div → section. Assembled from `classify_target` for the inline level, the block
model for paragraph, and the symbol tree for section.

## Data flow

```
didChange ──debounce(P1)──> publish ──> buffer_diagnostics ──> memo(P1) ──┐
                                                                          │
inlayHint(range)  ──> memo(P1) ──> xref_numbers / bib / include ──────────┤
foldingRange      ──> symbol tree + div/fence scan ───────────────────────┤
documentHighlight ──> classify_target(cursor) ──> targeted id scan ───────┤
selectionRange    ──> classify_target + block model + symbol tree ────────┘
                                                                          │
                                                          RenderedDoc (memoized)
```

No provider reaches the kernel, the preview server, or the network. The LSP stays offline and
kernel-free, which is what makes every phase testable without a browser or an interpreter.

## Error handling

- **Every provider returns an empty result rather than an error on a malformed buffer.** The
  existing `render_buffer` already wraps rendering in `crate::serve::guarded`, which converts a
  panic into `None`; new providers inherit that and must not introduce an `unwrap` on buffer-derived
  data. A half-typed document is the *normal* case for a provider that fires on every edit.
- **Position conversion stays at the boundary.** The wire uses UTF-16 columns and the scanners use
  scalar offsets; `lsp_pos` owns the conversion. Every new provider converts at its edge, exactly as
  `resolve_definition` does, so astral characters stay correct.
- **A missing include, bib or anchor is not an error**, it is an absent hint. Diagnostics already
  report the ones that are genuinely wrong; a provider that also complains would double-report.
- **stdout is the JSON-RPC wire.** No provider may print to it; diagnostics about the server itself
  go through `crate::log` to stderr.

## Testing

The corpus is the regression net for *rendering*; an LSP feature's analogue is a wire-level test.
Both are used:

- **Unit tests inline in the `lsp_*` module** that owns each provider, following the existing
  density (`lsp_complete.rs` has 43, `lsp.rs` 37). These pin span arithmetic and the shape of each
  result.
- **Wire-level tests in `crates/server/tests/lsp_stdio.rs`**, driving a real server over stdio, for
  each newly advertised capability. `lsp.rs`'s existing capability test already asserts that a
  dropped capability field is "its own silent feature loss"; extend it for the four new providers so
  a regression cannot be silent.
- **Existing corpus documents as fixtures.** The LSP tests already read `corpus(...)` files, so
  phases 2-5 pin against real documents rather than synthetic strings. **No new corpus document is
  required for this spec**, which matters because the walker renders every corpus doc on every
  `cargo test`.
- **Malformed input is a separate axis from span arithmetic.** A cursor walk pins spans; it does not
  pin the guards. Each provider needs at least one deliberately malformed buffer (unterminated
  `:::`, unterminated `$$`, truncated front matter) asserting an empty result rather than a panic.
- **Verify each fix by mutation**: restore the bug, watch the *named* test fail. A phase is not done
  until that has been observed.
- **Phase 0 is verified in the editor, not by a unit test.** A `tokenColorCustomizations`
  contribution cannot be asserted from Rust; confirm it with the running companion and record what
  was seen.

## Open questions

- **Phase 1's debounce interval.** Pick against a measurement, not a convention.
- **Whether the anchor-scan memo needs a generation signal at all**, or whether debouncing alone
  makes the walk cheap enough. Decide after measuring; the simpler change wins if it suffices.

## Out of scope

Clusters B-F of the idea pool: paste/drop gestures, the project index and the five surfaces gated
on it, task/testing/URI-handler integration, LM tools, and cell CodeLens. Cell CodeLens
additionally depends on backlog item 175(b) (output streaming) and must not be built from two
entries. Three ideas are ruled out with reasons in the pool; do not re-propose them.
