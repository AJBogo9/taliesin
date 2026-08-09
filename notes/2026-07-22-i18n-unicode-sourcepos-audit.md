# Audit: internationalization / Unicode / multibyte-offset correctness (perspective AP5)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-22. Perspective: AP5 from the backlog "Audit perspectives" section
(Unicode / multibyte-sourcepos correctness). Run as a single-perspective, code-read
session alongside two other live sessions (a feature session on `polish/a11y-holes`
and a separate audit), so it touches no source, builds nothing, and writes only this
file. Evidence came from reading the offset data flow plus one render probe against the
frozen `taliesin-stable` binary (`/home/bogo/.local/bin/taliesin-stable`, dated Jul 7),
which needs no `cargo build` and does not contend with the other sessions.

## Why this perspective

Click-to-source and editor navigation are load-bearing: the browser preview is a
read-only view, and the ONLY bridge back to the source is a position (file, line,
column). Every prior audit (UI, polish, deck, DX, security, simplification) looked at
behavior with ASCII fixtures. Nothing checked what happens when the source contains
non-ASCII text, which for a "typographic" authoring tool is the common case (curly
quotes, accents, CJK, emoji). This perspective follows the offset unit (byte vs Unicode
scalar vs UTF-16 code unit) across every hop from parse to editor and finds where the
unit silently changes.

## Executive summary

There is no single column convention across the tool's three editor-facing surfaces,
and none of the three is the UTF-16 that both the `vscode://file` URL handler and the
LSP protocol actually expect:

1. **comrak `data-sourcepos` columns are BYTE-based** (proven below). The browser
   Alt-click-to-source path reads these and builds `vscode://file...:line:col`.
2. **The stdio LSP server + diagnostics `to_lsp` use Unicode-SCALAR (char) columns**
   (`Vec<char>` indexing, `chars().count()`), and the server never negotiates
   `positionEncoding`, so the client interprets its output as UTF-16.
3. **The VS Code companion (TypeScript) uses UTF-16** (native JS string indexing),
   which is correct for VS Code, and which the Rust LSP was meant to mirror but does
   not.

Practical severity is bounded and honest: scalar equals UTF-16 across the entire Basic
Multilingual Plane, so all realistic natural-language text (accents, CJK, Greek,
Cyrillic, Arabic, Hebrew) navigates correctly. The defect surface is (a) ASTRAL
characters (emoji, mathematical alphanumerics, rare CJK extensions) sitting on the same
line before a navigable token, and (b) any block whose start column is not 1 on a line
with non-ASCII before it (rare for block-level nodes). The single most consequential
slice is LSP **rename**, because it emits scalar-based edit ranges into a WRITE path.

A large part of this audit's value is the honest NEGATIVES in the last section: the core
scanners and text-truncation are already multibyte-safe by construction, so a future
robustness/fuzzing pass (AP2) can skip them.

## Evidence: comrak sourcepos columns are byte-based

Probe file (rendered with `taliesin-stable render`, no build):

```
你好 world

- 汉字 item
```

`你好 world` is 8 Unicode scalars but 12 UTF-8 bytes. The emitted attribute:

```
data-sourcepos="1:1-1:12"
```

The end column is 12, the BYTE length, not 8 (the scalar length). Confirmed: comrak
reports columns in bytes. So the `data-sourcepos` contract Taliesin emits is byte-based.

## Findings

### I18N-1 (medium): three disagreeing column conventions at the editor boundary

The tool has no single answer to "what unit is a column," and the editor tools that
consume columns (`vscode://file:line:col`, the LSP protocol) both want UTF-16.

- Browser Alt-click: `web-client/client.js:1455-1462` parses `^(\d+):(\d+)` out of
  `data-sourcepos` (byte-based, per the probe above) and builds
  `vscode://file<abs>:<line>:<col>`. VS Code treats that column as a 1-based CHARACTER
  column, so a byte column drifts for any non-ASCII before the block's start column on
  its line. Block-level nodes almost always start at column 1, so in practice this is
  LOW impact, but the emitted contract is nonetheless wrong-unit.
- LSP + diagnostics: `crates/server/src/lsp.rs` and `crates/server/src/check.rs`
  (`to_lsp`, `crates/server/src/check.rs:89`) speak Unicode scalars.
- TS companion: `editor/vscode/src/hover.ts` / `definition-provider.ts` speak UTF-16.

Recommendation: choose UTF-16 as the single editor-facing column unit and convert at
each boundary (byte-to-UTF-16 for the `vscode://` link; scalar-to-UTF-16 for the LSP).
See I18N-2/I18N-3 for the specific conversions.

### I18N-2 (medium): the stdio LSP treats UTF-16 positions as scalar indices

`crates/server/src/lsp.rs:server_capabilities()` (lines 20-52) does not set
`position_encoding`. Per the LSP specification, absent negotiation the encoding is
UTF-16, so an editor sends `Position.character` as a UTF-16 code-unit offset and
interprets returned positions the same way. Every handler then does
`pos.character as usize` and feeds it to scalar logic:

- `lsp.rs:239` and `:309` pass `pos.character as usize` into
  `crate::lsp_nav::classify_target`, which indexes `lt: Vec<char>` (scalars) at
  `crates/server/src/lsp_nav.rs:68-70`.
- `lsp.rs:402` and `:425` pass it into `anchor_at`, same scalar indexing.
- Returned columns are scalar-based: `lsp.rs:254` (`col + id.chars().count()`),
  `:428-429` (rename edit ranges from `anchor_occurrences`, which uses
  `offset_to_line_col` counting scalars at `lsp_nav.rs:248-259`).

`lsp_nav.rs:7-8` documents the assumption ("Offsets are char-based (equal to UTF-16 for
ASCII ...)"). The claim understates the safe range (scalar equals UTF-16 across the whole
BMP, not just ASCII) and, more importantly, hides that scalar and UTF-16 DIVERGE on
astral characters.

Worked example. Line: `Ship it 🚀 in @fig-1`. The emoji 🚀 is 1 scalar but 2 UTF-16 code
units. VS Code places the cursor on `@fig-1` at a UTF-16 offset that is 1 greater than
its scalar index. `classify_target` indexes `Vec<char>` with that too-large value, so it
reads one scalar to the right of where the user clicked, and can misclassify or miss the
token. Go-to-definition, hover ranges, and completion replace-ranges are all affected the
same way.

Severity is medium, not high, because it requires an astral character before the token on
the same line. All BMP text is correct.

Recommendation: negotiate `position_encoding` explicitly and convert. Either advertise
UTF-16 and add a small `utf16_to_scalar(line, u16col) -> scalar` on the way in plus
`scalar_to_utf16` on the way out, or advertise `positionEncoding: utf-8` and move the
internals to byte offsets. UTF-16 conversion is the smaller change given the current
scalar code.

### I18N-3 (medium, write path): LSP rename can misplace edits on astral lines

`resolve_rename` (`crates/server/src/lsp.rs:413-442`) builds `TextEdit` ranges from
`anchor_occurrences` scalar columns and returns them as UTF-16 `Position`s. On a line
with an astral character before the anchor, each such edit range is 1 UTF-16 unit short
of where it should be, so the editor applies the rewrite starting inside the preceding
astral character's surrogate pair. This is the same root cause as I18N-2 but lands in a
WRITE path (the one place the tool's data flows back into the source through the editor),
so it is the sharpest edge of the finding even though the trigger is niche. The
single-editing-surface invariant intends the editor, not the preview, to own writes;
this does not violate that, but it can make an editor-owned write land wrong.

Recommendation: fixed by the same boundary conversion as I18N-2; call it out separately
in a test so the rename range is pinned against an astral fixture.

### I18N-4 (low): the "port of the companion" parity claim is now false for astral input

`lsp_nav.rs:1-8` describes itself as a Rust port of the companion's `hover.ts` /
`complete.ts` and says it matches them. The companion indexes JS strings (UTF-16:
`.length`, regex `.index`, `bibText[i]` at `editor/vscode/src/hover.ts`), while the Rust
port indexes `Vec<char>` (scalars). For astral input the two return different columns, so
the LSP server and the companion disagree on the same document. Whichever encoding fix is
chosen, restore the stated parity (or update the comment to state the intended
divergence, though matching is the better outcome).

### I18N-5 (low): no non-ASCII coverage anywhere near these paths

Every fixture in `crates/server/src/lsp.rs` tests (lines 809-1647), the `lsp_nav.rs`
unit tests, and the diagnostics tests is pure ASCII. There is no test that a curly quote,
a CJK character, or an emoji before a token leaves navigation correct. This is why the
byte/scalar/UTF-16 divergence was never caught. Independent of the fix, add fixtures that
pin the intended encoding: one BMP case (must stay correct today) and one astral case
(the regression guard for I18N-2/I18N-3).

## Verified safe (honest negatives, so later audits can skip them)

- **Core scanning loops do not panic on multibyte.** `render/extension/mod.rs:59-82`,
  `prose.rs:122-168`, and `site/xref.rs:111-138` all drive with `as_bytes()`, compare
  only against ASCII bytes, and slice on byte offsets derived from those byte scans, so
  every slice endpoint lands on a char boundary regardless of multibyte content. A
  continuation byte simply matches no branch and falls through.
- **Heading-attribute slicing is guarded.** `render/mod.rs:1644` and `:1669`
  (`&line[open+1..line.len()-1]`) look risky but are safe: the `line.ends_with('}')`
  guard forces the final byte to ASCII `}`, and `{` at `open` is ASCII, so both endpoints
  are boundaries. Same for `lsp_outline.rs:31` and `emit.rs:244`.
- **Text truncation is char-aware.** `site/hover.rs:21-22` and `site/search.rs:170` use
  `char_indices().nth(CAP)` before truncating, so they cut on a boundary. `render/mod.rs:2204`
  slices `&hex[..12]` of a hex hash (ASCII).
- **Front-matter diagnostic columns are ASCII-safe.** `frontmatter.rs:512-556` computes
  `col`/`end_col` from indentation width plus key length, and front-matter keys and
  indentation are always ASCII, so the columned quick-fix diagnostics do not drift even
  though `to_lsp` would forward a wrong-unit column if one were ever non-ASCII.
- **Block identity and incremental update are unaffected.** `data-block-id` is a content
  hash over bytes and block-to-source mapping keys on LINE only (`map_origin(origins,
  sp.start.line)` at `render/mod.rs:338/393`), so multibyte content does not disturb the
  block model, the diff, or live state preservation.
- **The primary Alt-click line target is correct.** comrak line numbers are unaffected by
  within-line multibyte, and `client.js:171-179`'s locator uses the line only; only the
  finer `client.js:1455` path adds the (byte-based) column.

## Build-ready items to fold into backlog.md "Open work"

Not filed into `notes/backlog.md` yet: the feature session is actively editing that file,
so folding these in now would collide. Fold in when it settles.

- **I18N-2/I18N-3 (one change):** make the stdio LSP encoding-correct. Advertise
  `position_encoding` and convert UTF-16 <-> scalar at the boundary (in) and when
  emitting `Position`s (out), covering go-to-definition, hover, completion replace-range,
  prepareRename, rename, document symbols, and `to_lsp` diagnostics. Pin with I18N-5
  fixtures. Size: M.
- **I18N-1 (browser link):** convert the byte-based `data-sourcepos` start column to a
  character column before building the `vscode://file:line:col` URL in
  `client.js:1455-1462`, or emit a second char-based column attribute for this consumer.
  Low practical impact (block start column is usually 1) but removes the wrong-unit
  contract. Size: S.
- **I18N-4:** after the encoding fix, re-assert or correct the `lsp_nav.rs` "port of the
  companion" parity claim. Size: XS.
- **I18N-5:** add BMP and astral fixtures to the LSP and diagnostics tests. Can land
  first, as a failing guard, before the fix. Size: S.

## Method notes for the next AP5-style run

- The byte/scalar/UTF-16 discriminator probe (render `你好 world`, read the end column)
  is the fastest way to nail comrak's unit; reuse it if comrak is ever upgraded.
- To turn I18N-2/I18N-3 from code-traced into executable-proven without an editor, drive
  `taliesin lsp` over stdio with LSP framing and a document containing an astral char
  before an `@id`, then assert the go-to-definition and rename ranges. Deferred here only
  because it wants a fresh build and the tree was owned by the feature session.
