# R11 — a real external document

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Date:** 2026-07-28
**Round:** Wave 3 / R11 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** What breaks on a document the project did not write?

**Why novel here.** All four demand probes used fixtures written for the probe. LESSONS.md already
documents three corpus shape gaps and **each one hid a real bug**. A real document carries shapes
nobody thought to invent. This lens was previously listed as **L6, BLOCKED** on "a repository that is
not on this machine"; it is no longer blocked.

**Two real documents, not one.**

1. **`rust-lang/book`** (public, shallow clone) — an mdBook: **112 Markdown files, 25,962 lines**.
   For scale: that is larger than Taliesin's entire corpus (125 documents, 10,118 lines).
   Reproducible by anyone.
2. **A real 8-chapter Quarto book with a `_quarto.yml`** — a multi-author university project on this
   machine, used read-only. Only structural facts are recorded here; no content is reproduced.

New items are numbered from **127**.

---

## Headline

**Both documents failed in the same two ways, and neither failure is a bug in the renderer.**

```
$ taliesin check .                    # rust-lang/book, 112 pages
457 problems (123 errors, 329 warnings, 5 suggestions)

  329 warning[TAL-CODE-LANG]     ← 72% of everything, ONE shape
  118 error[TAL-LINK]            ←  96% of all errors, ONE shape
    5 error[TAL-LINK-ANCHOR]
```

**Everything else worked.** `taliesin build .` produced **112 pages in 427 ms** (3.8 ms/page), exit 0,
28 assets, a search index and a 404 page, on the largest document set this tool has ever been pointed
at. The engine is not the problem. **The two blockers are both about meeting a document where it
already is.**

---

## Items

### 127. Comma-separated code-fence attributes are unsupported, so 329 real code blocks ship as plain text and the diagnostic misdiagnoses it. (HIGH for adoption)

**Measured.** The 329 `TAL-CODE-LANG` warnings are all one shape:

| Count | Fence |
|---|---|
| 127 | ` ```rust,ignore ` |
| 82 | ` ```rust,ignore,does_not_compile ` |
| 74 | ` ```rust,noplayground ` |
| 9 | ` ```rust,no_run ` |
| 37 | six further `rust,*` combinations |

**The consequence is not a warning, it is lost rendering.** Verified in a browser on
`ch09-02-recoverable-errors-with-result.html`, reading the live DOM:

```json
{"totalBlocks":18, "highlighted":7, "plainText":11,
 "sample":[{"cls":"language-rust","spans":20},
           {"cls":"language-rust,should_panic","spans":0},
           {"cls":"language-rust,ignore","spans":0}]}
```

**Eleven of eighteen code blocks on one real chapter render unstyled**, and the emitted class carries
a comma (`class="language-rust,ignore"`), which is not a single valid class token.

**The message is actively wrong, which is the worse half.**

> unknown code language `rust,ignore`: this block renders as plain text (check the spelling, or use
> `text` if that is intended)

The language is `rust` and it is spelled correctly. `ignore` is an *attribute*, and this syntax is the
near-universal convention across **mdBook, rustdoc, Pandoc, Docusaurus and GitHub**. The tool tells a
new user their spelling is wrong when they have written the ecosystem's standard form. Wave 1's
adoption round measured that anxiety, not appeal, is what blocks switching; being told your correct
input is a typo, 329 times, on your first `check`, is an anxiety event.

**Fix.** Split the info string on `,` (and whitespace), treat the first token as the language and the
rest as attributes: highlight on the first token, and either ignore the rest or map the known ones.
Taliesin already parses `{python}`-style braces and `#|` cell options, so an info-string parser
exists; this is a widening, not a new subsystem.

**Refuted if** a `rust,ignore` block highlights (measured: 0 spans against 20 for plain `rust`).

**Pin it with** a corpus fixture carrying `lang,attr` fences — a shape the corpus has **nowhere**
today, which is exactly why nothing caught this.

### 128. Every internal link in a migrated document is a hard error, in both real documents, and a did-you-mean would close it. (HIGH for adoption)

**Measured, and the shape is identical in two unrelated documents:**

| Document | Link errors | Example |
|---|---|---|
| `rust-lang/book` (mdBook) | **118 of 123 errors** | `broken link: 'ch17-02-concurrency-with-async.md' resolves to 'ch17-02-concurrency-with-async.md', which is no page in this site` |
| the Quarto book | **10 of 11 errors** | `broken link: 'creators.qmd' resolves to 'creators.qmd', which is no page in this site` |

A stranger's first move is to rename their files so Taliesin can read them. **Every internal link then
points at the old extension**, and Taliesin — which already rewrites `.tmd` → `.html` — reports each
one as broken with no suggestion.

**The information needed to fix it is already in the page registry.** `creators.qmd` is broken, and
`creators.tmd` is a page in the same site. The tool knows both facts and connects neither.

**Fix, and the smaller option is better.** Not silent rewriting: a `.md` link could legitimately point
at a real shipped `.md` file. **A did-you-mean is the right shape** — "broken link: `creators.qmd`;
did you mean `creators.tmd`?" — matching the front-matter typo rule that R2 measured as the tool's
best diagnostic. `xref_didyoumean.rs` shows the pattern already exists in this codebase.

**This is the concrete version of Wave 1's item 93** ("`taliesin check` already *is* a Quarto migration
assistant and nothing says so"). It is now measured on two real corpora rather than argued.

**Refuted if** `check` already suggests the same-stem `.tmd` (measured: it does not, in either
document).

### 129. Shape inventory: what two real documents contain that `corpus/` has nowhere. (the durable artefact)

Per the slate, this may outlast the defects. **Enumerated from the two documents, not imagined.**

| Shape | Real-document use | In `corpus/`? |
|---|---|---|
| ` ```lang,attr,attr ` fences | **734 occurrences** across 11 distinct forms | **no** → item 127 |
| ` ```console ` (shell session) | 209 | no |
| links carrying a non-`.tmd` extension | 128 across both documents | **no** → item 128 |
| a `SUMMARY.md`-driven chapter order (mdBook's book spine) | the whole book's structure | no (Taliesin uses `_site.yml`) |
| 112 pages in one flat directory | the whole book | no (largest corpus project is 14 pages) |
| chapter files with **no front matter at all** | all 112 mdBook files | partially |
| `_quarto.yml` as the live project config | the Quarto book | **yes**, and it is handled — see below |

**Do not grow `corpus/` toward these wholesale** — the walker renders every corpus doc on every
`cargo test`. **Pin only the two that earned it**: a `lang,attr` fence (item 127) and a stale-extension
link (item 128). The rest are recorded so a future round does not re-derive them.

### 130. `CLAUDE.md` names a retired class prefix, and it cost this round a probe. (LOW)

`CLAUDE.md:67` says `src/highlight.rs server-side syntax highlighting (syntect → `qhl-` scope
classes)`. The emitter says otherwise: `highlight.rs:23` is
`ClassStyle::SpacedPrefixed { prefix: "tali-hl-" }`, and the rendered output confirms
`class="tali-hl-source tali-hl-rust"`.

A probe grepping for `qhl-` returned **0 on a page that was fully highlighted**, which read as
"highlighting is broken" until a known-positive sanity check caught it. `retired_names.rs` polices
live `qmd` tokens but evidently not this one, and `CLAUDE.md` is the file every session reads first.

**Fix:** one word. Filed because the cost lands on every future session, not because the line matters
on its own.

---

## Measured healthy

- **112 pages, 25,962 lines, built in 427 ms, exit 0.** No crash, no hang, no memory event, on
  roughly 2.5× the corpus's total volume. This is the strongest scale evidence the project has, and it
  came free.
- **The `_quarto.yml` detection works on a real Quarto book**, and the message is good:
  > found `_quarto.yml` at ., but the project config is now `_site.yml`: rename it, or its settings go
  > on being ignored

  The L4 finding that a pre-rename `_quarto.yml` was *invisible* is **fixed and now verified against a
  real document**, not a fixture.
- **Front matter is optional and the tool does not care.** All 112 mdBook files have none; every page
  built with a title derived from content.
- **The build is honest about failure**: `built with 452 problems (run with --strict to fail the
  build)`.
- **Rendering quality on a real page is high.** Screenshot of the built
  `ch09-02-recoverable-errors-with-result.html`: correct typography, working nav, inline `code` spans
  in headings, a syntax-highlighted `rust` block. Nothing about it reads as a foreign document.

---

## Not measured

- **Only two documents**, and one of them is not public.
- **No mdBook-specific syntax was exercised deliberately** — `{{#include}}`, `{{#playground}}` and
  `SUMMARY.md` ordering were present in the tree but not isolated and tested. Item 129 lists
  `SUMMARY.md` as an unhandled shape without measuring what Taliesin does with it.
- **No Docusaurus, no MkDocs, no loose-Markdown folder**, all of which the slate names as valid.
- **The 5 `TAL-LINK-ANCHOR` errors were not investigated** individually.
- **Whether `console` should highlight** — it produces no spans but also no warning, so syntect
  recognises it while styling nothing. Not chased.
- **No `check` was run against the two documents in their original form** (`.md` / `.qmd`), because
  Taliesin reads `.tmd` only. Whether it *should* offer a read-only pass over foreign extensions is an
  adoption question for the author, not a finding.

## Round bookkeeping

This round wrote only this file. Items 127-130 follow R9's 124-126. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**Remaining slate:** R8 (author value stream), R10 (demand and positioning), R13 (green software,
optional), and R12 (real-device mobile), which needs the author's phone.
