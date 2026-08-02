# Feature adoption report, and two retirements it measures

Date: 2026-08-02
Backlog items: **202** (build), **203** + **204** (retire), **207** policy half + **201** (riders)

## Why these three together

The 2026-08-01 [feature-value audit](../../../notes/2026-08-01-feature-value-audit.md) ranked
F5 (`taliesin features`) as its highest finding and named two cuts, C1 (`columns`) and C3
(`datasets:`). The three belong in one batch because 202 is the instrument and 203/204 are the
first two things it measures: building the report first turns the removals' evidence from a
session of `grep` into a command anyone can re-run.

The batch also settles a correction. **Item 204's filed cause is false.** The audit records
"the card already derives what it needs from the file on disk, which is why nobody fills the
block in", but `render/extension/dataset.rs` already prefers measured over declared, and
`frontmatter.rs`'s `KNOWN_KEYS` comment already scopes the key to "only what a file cannot say
about itself". Measured on 2026-08-02:

| sub-key | in-tree file | remote URL |
| --- | --- | --- |
| `bytes:`, `sha256:` | measured; the declaration is ignored, except that a declared `sha256:` becomes a **drift check** with its own tested diagnostic | the only possible source |
| `licence:`, `source:`, `title:`, `description:` | not derivable | not derivable |

There is nothing left to derive. The zero adoption is real but means something else: no remote
dataset has been cited yet and no licence has been needed. This is the backlog's own "trust an
item's symptom, never its cause" pattern, and the item is re-aimed accordingly (below).

## Item 202: `taliesin features`

### Purpose

Answer *what does this document use* and, in the other direction, *which documents use this
feature*. The second half is what makes corpus-plus-roadmap self-checking: the policy says every
capability ships pinned by a corpus document, and until now nothing could list the capabilities
that are not.

It is a **reporting command, not a gate.** A successful scan always exits 0. Only an unreadable
target or a missing path is a failure exit.

### Architecture: off the render path

The item's framing is "the render pipeline already knows every construct it expanded", which
suggests threading a recording sink through render. Rejected, for two reasons:

1. **The validator walk is not the free recording point it looks like.** `validate_div_class` is
   called only in `build_container`'s *generic/fallback* arm (`render/divs.rs:852`), so it never
   sees a div that matched a feature class. Recording would need a new sink at every dispatch
   arm, and each arm is a place to forget one.
2. **Warm-server, block-level incremental render is the tool's moat.** Taxing every render to
   serve a report nobody runs while editing is the wrong trade.

So `features` is a **separate pass that reuses the existing parsers**. It re-implements no
parsing and instruments no render:

| counted | parser reused | why not the alternative |
| --- | --- | --- |
| front-matter keys, top level plus the `execute` / `listing` / `hero` / `prose-lint` / `theorems` children | `frontmatter::front_matter_block` + `serde_yaml` | counted **inside** the YAML block. Grepping body text would score the reference page that documents a key as a page that uses it (item 208's recorded trap) |
| div classes, callout kinds, theorem kinds | `divs::scan_div_spans` + `divs::parse_attrs` | emitted HTML is lossy: `.columns` becomes `tali-layout`, and `.sidenote` / `.marginnote` / `.aside` collapse into `.column-margin` |
| cell languages, cell options | the block model's `Block.cell` | already parsed |
| shortcode names, `{{< input type= >}}` types | `extension::tokenize_args` | render expands shortcodes away |
| cross-reference prefixes actually written | `cite` | |

**Consequences of this choice, stated so they are not discovered later.** `RenderedDoc` does not
grow a field, the block model does not change, and the four-projection sweep
(`read` / `skim` / search index / `llms-full.txt`) is untouched because `features` emits no
block. The report answers *what the author wrote*, which can differ from *which arm won the
dispatch*: a div carrying both `layout-ncol=` and `.columns` counts as both, since
`layout-ncol` is tested first and shadows the class. For an adoption report that is the correct
answer, and it is recorded here so nobody reads it as a bug.

### The catalogue is read, never re-declared

Denominators come from the authoritative validator consts, not from `vocab.rs`.

**`vocab.rs` is the wrong source and this is the trap worth writing down.** It is the
*offered-completions* projection, not the implemented set: `vocab::DIV_CLASS_NAMES` holds 11
entries while `render::validate::DIV_FEATURE_CLASSES` holds 23, because `columns`, `column`,
`fragment`, `incremental`, `notes`, `fade-out` and `highlight` are implemented and deliberately
not offered. `vocab`'s shortcode list is likewise short: `SHORTCODE_SPECS` has two entries
(`embed`, `video`) while `input` and `dataset` are dispatched ahead of it in
`extension/mod.rs`. A report built on `vocab.rs` would under-count and, worse, would report a
feature as unused when it is merely unsuggested.

Sources, in one place:

- `frontmatter::KNOWN_KEYS` (with `UNSUPPORTED_KEYS` marked, not dropped: `csl` is recognized
  and ignored, which the report should say rather than hide), plus `EXECUTE_KEYS`,
  `LISTING_KEYS`, `HERO_KEYS`, `PROSE_LINT_KEYS`, `THEOREM_KEYS`
- `render::validate::DIV_FEATURE_CLASSES`, `CALLOUT_KINDS`, `THEOREM_KINDS`,
  `CELL_OPTION_KEYS`, `INPUT_TYPES`
- `SHORTCODE_SPECS` plus the two names dispatched ahead of it (`input`, `dataset`)
- the cell-language registry behind `vocab::CELL_LANGUAGES` / `render::executes_to_kernel`
- `cite::XREF_LABELS`

One drift test pins that every construct name the scanner can emit appears in the catalogue, so
a construct added later cannot be silently invisible to the report. The test must be
mutation-checked against exactly that shape, per the standing "gate the gate" rule.

**Out of scope for v1, deliberately:** `_site.yml` keys. They are per-project, not per-document,
so they do not fit the per-feature-per-document cut. Recorded here so the omission reads as a
decision.

### Output: shape follows the target, no flag

```
$ taliesin features corpus                     # a directory: feature-first adoption table
corpus, 195 documents

front-matter keys              33 known · 15 used · 18 unused
  title                       171
  datasets                      1  corpus/datasets.tmd
  include-in-header             0  (no document)
  logo                          0  (no document)

div classes                    23 known · 19 used · 4 unused
  column-margin                11
  columns                       3  corpus/media/gallery.tmd,
                                   corpus/scaffold/deck-tour.tmd,
                                   corpus/diagnostics/typos.tmd

26 of 96 features are used by no document

$ taliesin features corpus/media/gallery.tmd   # a file: document-first, "what does this use"
```

- **Three or fewer documents are named inline; above three, the count only.** The low-adoption
  tail is exactly what the audit cares about, so it should be readable without a second command.
  `--json` always carries the full list.
- **Target.** Any directory, walked. A directory holding an `_site.yml` reports pages in
  `chapters:`/nav order via `Site::discover` and handles drafts as a build does; a bare
  directory walks in sorted path order. A single `.tmd` file is also accepted. This diverges
  from `read`/`map`/`skim`, which refuse a bare directory, and the divergence is deliberate:
  those project a document for a reader, this inventories a tree for an auditor, and `corpus/`
  (the single most useful target) has no `_site.yml` at its root.
- **Formats.** Human by default, `--format json` / `--json` as on `read`/`map`/`skim`.
  Feature-first JSON, which a consumer can invert; no second flag for the inverse.
- **Placement.** `cmd_features` in `crates/server/src/query.rs`, beside `cmd_read` / `cmd_map` /
  `cmd_skim`. The scanner is `crates/core/src/features.rs`, because only core knows what it
  dispatches on.

### Corpus pin

Per corpus-plus-roadmap the capability ships pinned. `features` emits no HTML, so the pin is not
a rendered corpus document: it is an integration test in `crates/server/tests/` over a temp-dir
fixture, matching the standing rule that a pin exercising a non-render behaviour does not belong
in `corpus/` (the walker renders every corpus doc on every `cargo test` and would pay the cost
for nothing). The positive control is a fixture document that uses a known set of constructs and
must report exactly that set, plus a known-zero row, so an all-zero or all-full table cannot pass.

## Item 203: remove `::: {.columns}`, keep `layout-ncol`

### Measured before starting

`::: {.columns}` has three authored occurrences across all 195 `.tmd` files:
`corpus/media/gallery.tmd:27` (which documents it), `corpus/scaffold/deck-tour.tmd:37` (the
generated tour), `corpus/diagnostics/typos.tmd:32` (the fixture for `validate_column_width`).
`docs/guide/using/from-quarto.tmd:89` mentions it as a migration note. Two mechanisms for one
job, and six weeks of daily writing adopted the other one.

**`.column-margin`, `.column-page` and `.column-screen` are a different feature** (the layout
escapes, item 181), are used widely, and do not move. Nothing in this item touches them.

### Surface

Smaller than it looks. `.column` has no emitter of its own: it falls through to the generic-div
arm, and the `.columns` arm merely sniffs children whose HTML starts with `<div class="column"`
to pick a count. Neither class has any CSS, because the grid is inline-styled.

- `columns` and `column` out of `DIV_FEATURE_CLASSES`
- the `columns` arm out of `build_container` (`render/divs.rs:585-607`)
- `ncol` out of `DIV_ATTRIBUTES` (it is scoped `DivScope::Class("columns")`)
- `validate_column_width` deleted: it fires on `.column` and nothing else
- `layout-ncol` is a generic attribute tested *ahead* of the `columns` arm and is unaffected

### The diagnostic, which is the real work

`RETIRED_KEYS` is front-matter-only, so this needs a sibling **`RETIRED_DIV_CLASSES`** register
consulted by `validate_div_class` ahead of its did-you-mean search. Without it a leftover
`::: {.columns}` is answered by silence (nothing survives within edit distance 2 of `columns`
once `column` is also gone), which is the failure the `about:` / `number-within:` precedent
exists to prevent.

Phrased as a **removal, never a rename**, for the reason `RETIRED_KEYS` already documents:
`codes::extract_suggestion` lifts a did-you-mean phrase into a structured fix an agent applies
mechanically, and `.columns` to `{layout-ncol=N}` is not a mechanical rename (the class sits on
the parent and the attribute replaces it while the `.column` children go away).

### Documents that move in the same change

- `corpus/media/gallery.tmd:19-35`, rewritten to `{layout-ncol=2}`, which the same document
  already uses higher up
- `corpus/scaffold/deck-tour.tmd:37-43` **and the `new deck --tour` generator that emits it**,
  which `crates/server/tests/new_cli.rs:337` pins byte-for-byte against the fixture
- `corpus/diagnostics/typos.tmd:32-36`, repurposed from the `validate_column_width` pin to the
  retired-class pin
- `docs/guide/using/from-quarto.tmd:89` and the guide's div-class reference page

## Item 204: move the annotations onto the shortcode

Re-aimed per the correction above. The key is not redundant, so the answer is not "derive it";
it is to put the annotation where the author is already typing and delete a `KNOWN_KEYS` entry
worth six drift gates.

### Shape

```
# before
---
datasets:
  - url: https://example.org/full.parquet
    licence: ODbL-1.0
    source: https://example.org/data
    sha256: 9f86d0...
    bytes: 2400000000
---
{{< dataset https://example.org/full.parquet >}}

# after
{{< dataset https://example.org/full.parquet
   licence=ODbL-1.0 source=https://example.org/data
   sha256=9f86d0... bytes=2400000000 >}}
```

Inline arguments are already the house idiom: `{{< video clip.mp4 poster= caption= dark= >}}`.

### Surface

- **`dataset` joins `SHORTCODE_SPECS`.** It is currently dispatched ahead of that table and so
  has never been argument-linted at all; joining it buys the closed-vocabulary did-you-mean for
  free, including the `licence=` / `license=` pair the front-matter parser already accepted.
- `Declared` / `declared()` read tokenized args instead of front matter.
- **`path:` and `url:` disappear entirely.** Their only job was matching an entry back to the
  shortcode's target, which the positional argument already identifies.
- `datasets` leaves `KNOWN_KEYS` and enters `RETIRED_KEYS`.

**Every capability survives**, including the `sha256=` drift check ("the file changed since it
was recorded, so any figure computed from it is stale") and the remote no-digest warning. Both
keep their existing tests with a changed input shape; neither test is deleted.

### The six gates come back

Removing a front-matter key trips `KNOWN_KEYS`, the JSON schema, the editor vocab golden file,
`crates/core/assets/agents/AGENTS.md`, the guide-reference completeness gate, and the repo-root
`AGENTS.md` **whose test lives in the server crate**, so `cargo test -p taliesin-core` is green
while it is stale. The first four bless; the last is a `cp` from the asset.

Documents: `corpus/datasets.tmd` (the pin), `docs/guide/reference/frontmatter.tmd`,
`docs/guide/reference/shortcodes.tmd`, and the comment at `crates/server/src/lsp_insert.rs:250`
that explains why there is no `datasets:` scaffold.

## Riders

**207, policy half only.** One line into the backlog's *Standing constraints*: promote "derive,
don't declare" from a batch note to a standing rule, with the six-gate-per-key cost as its
stated reason, so every proposed key has to answer *what on the page already implies this?*
Its other half (the four unpinned keys `include-in-header`, `include-before-body`,
`include-after-body`, `logo`) is **deliberately left open** for the owner to rule on after
seeing `features` print them, rather than pre-judged here. That is the self-checking loop this
batch exists to build, and using it once before deciding is the point.

**201.** Two lines. `crates/server/src/exec.rs:255-257` still calls `--no-exec` "the safe way to
preview a document you don't trust", which the shipped user-facing wording (`cli.tmd:184`,
`cli.tmd:198-201`) already corrected. The behaviour is right and unchanged; only the comment
contradicts the rest of the codebase about a security property.

## Order and verification

1. **202 first.** It is the instrument, and running it produces the before/after evidence for
   both removals.
2. **203, then 204.** Independent of each other; 203 is the smaller surface.
3. **Riders last**, 201 alongside whichever change touches `exec.rs`, or on its own.

Every fix verified by **mutation** (restore the bug, watch the *named* test fail), not by a green
suite. `./tools/gates.sh` before anything is called done, with
`TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python` and the workspace suite at
`--test-threads=1`, because a plain `cargo test` skips the four interpreter-gated suites
silently. `cargo test` aborts remaining binaries at the first failure, so totals are re-run
before they are trusted.

## Explicitly not in this batch

- `_site.yml` keys in the `features` catalogue (per-project, not per-document).
- A `--unused` or `--by-document` flag: the target decides the shape, and minimal config says
  perfect the default before adding a knob.
- Any change to `.column-margin` / `.column-page` / `.column-screen`.
- Deleting the `datasets` capability, as opposed to relocating its annotations. The sha256 drift
  diagnostic is the reproducibility story the tool is positioned on.
- Ruling on 207's four unpinned keys.
