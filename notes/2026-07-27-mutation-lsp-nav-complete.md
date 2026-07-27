# `lsp_nav.rs` measured end to end (item 68)

**2026-07-27.** The last compute the mutation campaign owed: `lsp_nav.rs` was the one file measured
only in part (338 of 444 on 2026-07-26, banked in
[2026-07-26-mutation-server-half-partial.md](2026-07-26-mutation-server-half-partial.md)). This run
covers it whole, and it was deliberately scheduled *after* item 58's pins landed so the same job
answers two questions at once: did those pins kill what they were written for, and what is in the
106 mutants nobody had ever tested?

Run against a `git archive` snapshot of `717e76d`'s parent (`6e677f0`), output outside the tree at
`~/.cache/taliesin-mut68-out/`. Package-default scoping, which is sound for a `crates/server` file
(no `crates/core` test reaches server code), so **every MISSED here is real** — no workspace
recheck is owed.

## Result

| | |
|---|---|
| measured | **443 of 444** |
| caught | 394 |
| **missed** | **24** |
| timeout | 21 |
| unviable | 4 |
| rate | ~9.0 mutants/min at `-j 4` |

**One mutant never ran** (`frontmatter_bib_paths:503:11 += → *=`): the machine died with it in
flight. **Do not record it as a timeout on the strength of the campaign's rule** — its own
same-line sibling (`+= → -=`) *survived* rather than hanging, so this one is genuinely unmeasured
and belongs in the next batch's list rather than in the detections column.

## Answer 1: the cursor-walk pins did what they were written for

Of the **36 survivors** the partial run found, **35 are now caught.** The eight functions that held
them are complete, and seven are clean:

| function | mutants | survivors now |
|---|---|---|
| `classify_target` | 58 | **0** |
| `classify_include` | 61 | 1 (`150:38 == → !=`) |
| `classify_frontmatter_key` | 32 | **0** |
| `definition_site` | 38 | **0** |
| `is_anchor_site` | 29 | **0** |
| `anchor_occurrences` | 22 | **0** |
| `nested_parent_of` | 9 | **0** |
| `is_cite_key_char` | 6 | **0** |

That is the confirmation the item was filed for, and it also confirms the *method*: the second pass
that item 58 paid for — malformed input as an axis separate from cursor position — is what closed
the guards that reject, and those are the ones that are now zero.

## Answer 2: the untested tail was NOT the same shape, and that is the lesson

The partial run's findings doc concluded that **"all 36 survivors are one shape"** — a boundary
comparison or cursor operator inside a click-to-source position classifier — and that one
table-driven cursor walk would kill most of them at once. **That was true of the 338 it measured
and wrong as a description of the file.** 23 of the 24 remaining survivors are in the tail it never
reached, and the biggest hole there is not a classifier at all:

- **`bib_entry_offset` — 17 survivors, and the function has NO test whatsoever.** It appears three
  times in the whole file (its definition and two call sites) and **zero times in the test module.**
  Not an unpinned edge: an unpinned *function*, the same shape as item 59's `server_capabilities`
  and item 61's `runtime_dirs.rs`. It is the scanner that locates a BibTeX entry by key — the thing
  that makes `[@key]` go-to-definition land on the right entry — and every boundary in its
  brace/whitespace walk is free to move.
- **`frontmatter_bib_paths` — 4** (`479:13`, `481:23`, `495:39` match guard, `503:11`). This one
  *is* exercised by the test module, so these are the finer-grained inside-the-function kind.
- **`anchor_at` — 2** (`376:15`, `376:43`, both `|| → &&`). Also exercised; same kind.
- **`classify_include` — 1** (`150:38 == → !=`), the single pre-pin survivor left.

**The transferable rule: a partial run's shape-conclusion describes the part it measured, not the
file.** Generalising it is how a whole subsystem stays invisible — and the cost of finding out was
48 minutes, against the 3 hours the old estimate would have argued for.

## Timeouts: 21, all cursor arithmetic

`+= → *=` (17), `-= → /=` (3), `+= → -=` (1), spread across `classify_target`, `classify_include`,
`classify_frontmatter_key`, `definition_site`, `is_anchor_site`, `anchor_occurrences`, `anchor_at`
and `bib_entry_offset`. A stalled scan loop spins instead of returning a wrong answer, so the hang
**is** the detection. That is now **62 of 62** across the whole campaign, with no counter-example.

## The 24 survivors, exact locations

Banked in-repo so the next batch does not depend on a `mutants.out` outside the tree.

```
lsp_nav.rs:150:38: replace == with != in classify_include
lsp_nav.rs:376:15: replace || with && in anchor_at
lsp_nav.rs:376:43: replace || with && in anchor_at
lsp_nav.rs:396:21: replace < with <= in bib_entry_offset
lsp_nav.rs:399:18: replace > with >= in bib_entry_offset
lsp_nav.rs:400:25: replace < with <= in bib_entry_offset
lsp_nav.rs:400:25: replace < with == in bib_entry_offset
lsp_nav.rs:400:25: replace < with > in bib_entry_offset
lsp_nav.rs:401:23: replace += with *= in bib_entry_offset
lsp_nav.rs:401:23: replace += with -= in bib_entry_offset
lsp_nav.rs:403:22: replace < with <= in bib_entry_offset
lsp_nav.rs:403:26: replace && with || in bib_entry_offset
lsp_nav.rs:405:29: replace < with <= in bib_entry_offset
lsp_nav.rs:405:29: replace < with == in bib_entry_offset
lsp_nav.rs:405:29: replace < with > in bib_entry_offset
lsp_nav.rs:406:27: replace += with *= in bib_entry_offset
lsp_nav.rs:406:27: replace += with -= in bib_entry_offset
lsp_nav.rs:410:33: replace < with <= in bib_entry_offset
lsp_nav.rs:413:30: replace < with <= in bib_entry_offset
lsp_nav.rs:413:34: replace && with || in bib_entry_offset
lsp_nav.rs:479:13: replace < with <= in frontmatter_bib_paths
lsp_nav.rs:481:23: replace || with && in frontmatter_bib_paths
lsp_nav.rs:495:39: replace match guard !item.trim().is_empty() with true in frontmatter_bib_paths
lsp_nav.rs:503:11: replace += with -= in frontmatter_bib_paths
```

Plus the unmeasured `lsp_nav.rs:503:11 += → *=`.
