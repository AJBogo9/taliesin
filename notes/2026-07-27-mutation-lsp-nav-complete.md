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

## Closed (item 69): 25 of 25, and one campaign rule needed sharpening

All 24 survivors plus the unmeasured mutant are now detected — **22 killed by a named assertion, 3
by hanging** — each verified by restoring the mutant by hand, running one *named* test, and
restoring by inverse edit. Five tests, all in `lsp_nav.rs`'s own module:

| test | kills |
|---|---|
| `bib_entry_offset_skips_whitespace_and_stops_at_the_end_of_a_truncated_bib` | 17 |
| `frontmatter_bib_paths_scans_forwards_and_only_inside_the_front_matter` | 4 |
| `anchor_at_needs_the_cursor_on_the_token_and_a_sigil_before_it` | 2 |
| `only_include_and_embed_shortcodes_are_navigable` | 1 |
| `a_bibliography_list_stops_at_the_first_non_item` | 1 |

**Item 69's own framing was wrong in the same way item 61's was, and for the same reason.** It said
`bib_entry_offset` "has NO test at all", measured as zero occurrences of the name in the test
module. The name is absent; the function is not untested — `bib_entry_site_finds_the_entry_header`
and `bib_entry_text_is_brace_balanced` both drive it through its two wrappers. What was missing was
not a test but a *fixture shape*: every existing one is a canonical, complete `@type{key,` header,
so neither whitespace-skipping loop ever ran a single iteration and no bounds check was ever
evaluated at the end of the buffer. **Counting occurrences of a private function's name measures
whether it is called directly, not whether it is tested** — for a helper reached through a wrapper
the count is always zero and always says nothing.

Eleven of the seventeen are killed only by the malformed axis (item 58's lesson, third batch
running): six truncations, one per part of the header, each of which makes some widened bound read
one char past the end. The rest are the whitespace arms, which need `@article {key,` and
`@article{ key,` — both legal BibTeX, both absent from every fixture in the tree.

### The sharpening: a `*=` in the *missed* column is not a counter-example, it is a dead loop

The campaign's standing rule is that a `+= → *=` on a scan cursor hangs, and the hang is the
detection — 62 of 62 with no counter-example. **Two of this batch's survivors looked like exactly
that counter-example:** `401:23 += → *=` and `406:27 += → *=` were recorded as MISSED, not as
timeouts. They are cursor increments in scan loops, so by the rule they should have hung.

They did not hang because **nothing entered the loop.** Both sit in the whitespace skips that no
fixture ever reached, and a loop with zero iterations cannot spin. Adding the fixture flipped both
from SURVIVED to hang, measured here (120 s wall, killed by timeout). The previously unmeasured
`503:11 += → *=` behaves the same way, so the tally is **65 of 65, still with no counter-example**.

**The transferable form: "a hang is a detection" presupposes a fixture that enters the loop.** When
a `*=` cursor mutant appears in the missed column rather than the timeout column, the finding is
not "the rule has an exception" — it is "this loop is never executed by any test", which is a
strictly *worse* gap than the boundary mutants next to it and is invisible from the survivor list
alone. Both readings produce the same list; only one of them explains it.
