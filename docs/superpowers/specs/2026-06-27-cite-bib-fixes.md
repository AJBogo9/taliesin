# `.bib` rendering fixes (Lane C — cite.rs)

Date: 2026-06-27
Scope: `crates/core/src/cite.rs` + a focused corpus bib/doc + cite tests.
Authorization: cite.rs is normally Do-NOT-touch; this lane is explicitly greenlit
to fix `.bib` rendering bugs, under a HARD merge gate: existing IEEE corpus
citation output must stay BYTE-IDENTICAL except for the specific fixes below.

## Hard merge gate

- The existing corpus citation HTML (IEEE style) must not change except where a
  bug is fixed. A byte-stable guard test snapshots a current-correct corpus doc's
  References section and asserts it is unchanged.
- `cargo test -p qmd-fast-core` must pass.

## Bugs fixed

### 1. LaTeX accents → Unicode

A `latex_accents()` pass runs inside `clean()` (so it applies to every name/title
part, journal, publisher, note, etc.). It resolves the common TeX accent macros to
their composed Unicode characters BEFORE the brace-stripping step, so brace-grouped
forms like `M{\"u}ller`, `Erd{\H{o}}s`, `\v{s}` resolve correctly.

Supported macros (the common set):

- Accent macros taking a letter, in both `\"o` / `\"{o}` forms, for the vowels and
  common consonants:
  - `` \` `` grave → à è ì ò ù …
  - `\'` acute → á é í ó ú ý ć ń ś ź ŕ ĺ …
  - `\^` circumflex → â ê î ô û …
  - `\"` diaeresis/umlaut → ä ë ï ö ü ÿ …
  - `\~` tilde → ã ñ õ …
  - `\=` macron → ā ē ī ō ū …
  - `\.` dot-above → ż ė ċ ġ …
  - `\u` breve → ă ğ …
  - `\v` caron/háček → š č ž ř ě ň ľ ť …
  - `\H` double acute → ő ű …
  - `\c` cedilla → ç ş ţ …
  - `\k` ogonek → ą ę …
  - `\r` ring → å ů …
  - `\b`/`\d` (bar-under/dot-under) → pass the base letter through
  - A nested control sequence as the base (`\"\i` → ï) is resolved first.
- Special / no-argument macros:
  - `\AA`→Å `\aa`→å `\AE`→Æ `\ae`→æ `\OE`→Œ `\oe`→œ `\O`→Ø `\o`→ø
  - `\ss`→ß `\L`→Ł `\l`→ł `\DH`→Ð `\dh`→ð `\TH`→Þ `\th`→þ `\i`→ı `\j`→ȷ
- Literal-escape macros are UNescaped (kept, not dropped): `\&`→& `\%`→% `\_`→_
  `\#`→# `\$`→$ (these handle real titles like "AT\&T", "50\% off", "C\#").

Unknown/unsupported macros degrade gracefully (the backslash + name are dropped,
the argument letter is kept), so no entry renders worse than before.

### 2. Corporate / brace-protected authors

The BibTeX corporate marker is the DOUBLE brace `{{World Health Organization}}`:
`read_value` strips exactly one brace level, so a double-brace value retains its
inner braces and `format_one_author` (which already keeps a value starting with `{`
literal) renders it WHOLE, never split into "W. H. Organization". A SINGLE-brace
`{First Last}` keeps no braces and is initialized normally — that is the standard
convention, and the distinction is load-bearing: single-brace `{First Last}` authors
appear in the existing corpus (`{Umar Jamil}`) and must keep initializing.

### 3. `@string` macro support

`@string{ key = "value" }` definitions are collected during parsing and substituted
wherever an entry references `key` as a bare (unquoted, unbraced) value, including
inside `#`-concatenations (`mar # " and " # apr`). Case-insensitive keys (BibTeX is).

### 4. `@inbook` / `@incollection`

These now render `booktitle` (italic, "in <booktitle>") and `pages` (pp. N–M),
which `fmt_book` previously dropped. Chapter/section titles in `title` are quoted
like an article.

### 5. Auto-References duplication

If the document already contains a manual heading whose text is "References" (or
"Bibliography"), the auto-generated `.qmd-references` section omits its own
`<h2>References</h2>` so the reference list renders under the author's heading
instead of producing a second "References" heading. The list `<div>`s and the
`#ref-<key>` anchors are unchanged, so existing citation links still resolve.

## Tests

- Unit tests in `cite.rs` for each fix (failing-first).
- Byte-stable guard: `ieee_corpus_reference_output_is_byte_stable` snapshots the
  pca-geometry post's rendered References block and asserts it is unchanged.
- Corpus pin: `corpus/posts/cite-coverage/` — a focused doc + bib citing a Müller
  (accents), an Erdős (`\H`), a corporate author, a `@string`-using entry, and an
  `@inbook` entry, with a manual `# References` heading (dedup). A corpus test
  asserts the rendered strings.
