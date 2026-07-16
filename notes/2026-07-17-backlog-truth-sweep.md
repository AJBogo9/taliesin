# Backlog truth sweep, 2026-07-17

Every open item in [backlog.md](backlog.md) re-derived from today's source, after the file was
caught advertising already-built work. Verdicts below; the corrections are already applied to
`backlog.md`, so this file is the **evidence**, not a second to-do list.

## Why it rotted (the part worth keeping)

Not the usual cause. The backlog warns that the author pushes mid-session, so entries go stale with
no signal. **That is not what happened here.** `2368e4a` is titled *"prune the machine-facing items
that landed"* and did exactly that: it pruned the **machine-facing audit's** items. D37 came from the
Quarto catalog and the display-only fence from elsewhere, so neither was in the prune's scope and
neither was re-checked. The start-here block was then refreshed 90 minutes **after** both had landed
and still named D37 "the cleanest build on the list".

**A scoped prune leaves the unscoped half looking freshly reviewed.** A reader cannot see a prune's
scope from its result: pruned and never-examined entries are byte-identical on the page. So: prune
the whole list, or write down which slice you pruned.

## Verdicts

**LANDED (were listed as open):**

| Item | Landed | Proof |
|---|---|---|
| **D37** lint `format:` sub-keys | `515fbd7` | `frontmatter.rs:286-308` `validate_format_subkeys`, emits `located(...)` |
| **#3** `{.python}` display-only fence executed | `371b060` | `render/cell_extract.rs:181` `is_executable_fence`, gated `render/mod.rs:348` |

**ROT (symptom real, entry's cause wrong):**

- **#5 `lang: fr`** — the entry named `render/page.rs:239`, which is **correct code**
  (`<html lang="{lang}">` fed by `doc.lang`, doing its job). True site: `cite/render.rs:15-21`, a
  hardcoded English const table; `lang` appears **zero** times in that file, so there is no
  localization seam at all. And the "promise" is unsupported: the docs and `vocab.rs` only ever say
  `lang:` sets `<html lang>`. **It is an absent i18n feature with a scope question, not a defect.**
  Wrong *layer*, not merely a wrong line.
- **#11 "the `reload()` lever … is unused"** — **false**. Three live senders
  (`serve_site/mod.rs:835`, `:1216`, `serve/mod.rs:1086`) plus a protocol test. It is unwired *to the
  boot-mismatch path* only, which makes the fix narrower than the entry implies.

**Mispriced:**

- **#2 duplicate-label warnings** reads as one small fix and is **two**. `render/mod.rs:1568` is on
  the `Vec<Warning>` channel, which already has `.at(file, line)` (`render/model.rs:166`) — cheap.
  `site/xref.rs:23` and `site/mod.rs:172` are `Vec<String>`: **no location field exists in that
  channel**, so that half is a channel type change.

**Double-filed:**

- **M4** appears twice at two priorities with two severities: §4's "`fork_kernel` PID desync" and
  Tier 2's "`fork_kernel` cross-call edge (low)". Same bug. Whoever fixes the M3/M4/M5 bundle deletes
  the Tier-2 line.

**Mis-labelled blast radius:**

- **M6 is two items.** `MAX_WARM_PAGES` (`serve_site/exec_pool.rs:14`, a file the entry never named)
  **needs sign-off**: eviction at `:87` drops the executor, killing its kernel children (`:17`) and
  destroying that page's variable state; `:3` says the order must stay deterministic because the
  build relies on it. The `/proc` probe (`build_budget.rs:36-46`) is **free-standing**. The one-line
  summary ("a constant and a `/proc` probe") is what bundled a kernel-lifecycle policy with a file
  read. **The label travels with the summary, not the code.**

**False premise (was blocking verification):**

- **"this sandbox has no `ipykernel`"** (on #3 and #8) is **wrong**.
  `~/.local/share/qmd-venv/bin/python` has it and the warm forkserver boots (`preloaded: numpy,
  matplotlib`). #8 is verifiable now. An unchecked premise had quietly marked two items unverifiable.

**Confirmed REAL** (symptom + cause hold; several line numbers drifted, corrected in `backlog.md`):
#1, #2, #4, #6, #7, #8, #10, #11, #13, #14, #15, D34, D70, M2, M3, M5, D72, D69, B3-18.

Notable strengthenings found while checking:
- **#7** the entry's grep claim verified exactly; `docs/` matches only `docs/internals/sites.tmd`.
  Corpus has **9** matching docs, so the entry's "6" was an undercount.
- **#8** the correct shape already exists in the same function: the `CellRole::Listing` arm gates
  (`:572`) *before* it registers (`:578`), while the cell arm registers (`:525`) before the `match
  lang` (`:527`).
- **#6** `site/llms.rs:241` already has the `&nbsp;` replace that `site/search.rs:163-171` lacks —
  the "helper exists next door, reimplemented weaker" pattern for the fourth time.
- **M3** is stronger than filed: refill is *permanently* dark, not dark-until-next-take. `take` only
  re-triggers refill at `:483` behind `if kernel.is_some()` (`:476`), and an empty `ready` queue can
  never satisfy that guard.

## Line numbers corrected in `backlog.md`

#2 `1538→1568`, `56→50` · #14 `358-361→400`, `399-409→443-453`, `411-414→454-457` ·
#15 `102-110→133-145`, `252-283→287-318` · #8 `~523→525` · #4 `1148-1199→1186-1204`.
`minify.rs` is `crates/server/src/minify.rs`; M6's constant is `serve_site/exec_pool.rs`.

## Method

The sweep was told to trust the **symptom** and re-derive every cause, and to treat a
name-matched test in `crates/*/tests/` as near-proof a feature exists. Traps that fired during the
2026-07-16 round and were avoided here: a bare word matching English prose, `grep | head` reporting
**head's** exit code (so `|| echo absent` never fires), `grep -c` counting lines, and assets living
under `crates/core/assets/`. A branch name proves nothing: `d37-format-subkey-lint` existing is not
evidence, its **content being in main's source** is.
