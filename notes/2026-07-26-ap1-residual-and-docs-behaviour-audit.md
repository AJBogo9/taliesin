# AP1's unchased residuals, and the behavioural half of the docs lens (2026-07-26)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspectives:** the last two audit angles left unstarted after 2026-07-25 — **AP1's
unchased residuals** (kernel RSS drift, multi-hour warm RSS) and the **behavioural half
of the docs-vs-behaviour lens** (what a key *does*, not whether it exists). Run together,
against `3f3a6bb`, release build. Findings **and** fixes; both fixes ship in this batch.

## Headline

**Both entries were wrong about where their own defect was, in opposite directions.**

AP1's residual predicted a *kernel* leak ("the warm kernel is reused across edits, so a
per-execution leak would compound"). Measured over 1,000 real executions the kernel is
**fine** — it saturates. The unbounded growth is in **Taliesin's own process**: the freeze
cache is capped by entry *count* (1024) and never by *bytes*, and an entry is a whole
rendered cell output. 150 edits to one matplotlib cell in a single warm session wrote a
**6.71 MB** `_freeze/<page>.json`, growing strictly linearly.

The docs lens's existing gates tie **code → `--help` → the guide** for flags and env vars,
so "does this knob exist" is well covered. Nothing tied the *front-matter vocabulary* to
the guide, and there the drift was total: **`about:` was removed from the code on
2026-07-17 and the User Guide kept documenting it for nine days** — a dedicated reference
section, a sub-key table, a worked recipe, and a `formats.tmd` subsection. A reader
following any of them got an `unknown front-matter key` warning, which fails `check`,
`build --strict` and `publish`.

---

# Lens 1: AP1's unchased residuals

## Method

The residual needed "a live kernel + a scripted execute-many loop", which nothing in
`tools/` does (`live-edit-bench` is in-process and never starts a server or a kernel). So:
a real `taliesin preview` on a temp doc, the **cell body** rewritten per iteration, and
each sample taken only after the executed marker appeared in the served HTML.

Two traps shaped the harness, both already recorded in the backlog's standing constraints:

- **Editing prose measures nothing.** A cell's freeze key is its own code plus upstream
  same-language code, so only a body edit re-executes. Every iteration rewrites the body
  and every sample is confirmed post-execution (`misses=0` on all runs).
- **RSS is allocator noise at this resolution.** The RSS series is real but ragged (it
  dropped 12 MB at one sample). The decisive measurement is the **on-disk
  `_freeze/<page>.json`**, which is a deterministic function of what the cache retains.

Runs: 400 trivial-cell executions, 200 plot executions, 600 plot executions, plus a
freeze-size probe of 150 plot executions.

## Refuted: the kernel does not leak

| Run | Kernel RSS | Verdict |
|---|---|---|
| 400 trivial executions | 68.8 MB → 69.4 MB (+536 KB, 1.3 KB/exec, decelerating) | no leak |
| 600 plot executions | 115 MB → ~143 MB, **flat after ~40 executions** | saturates |

Matplotlib figures are being closed and the warm kernel reaches a steady state. The
residual's stated premise is refuted, and this is the half worth keeping: the warm-kernel
moat does not degrade over a long session.

## AP1-R1 (medium): the freeze cache is capped by entry count, never by bytes

**Measured.** One `taliesin preview` session, one matplotlib cell, edited N times:

| Edits | `_freeze/<page>.json` | Entries | Server RSS |
|---|---|---|---|
| 1 | 68 KB | 2 | 20.6 MB |
| 50 | 2.05 MB | 51 | 31.2 MB |
| 100 | 4.32 MB | 101 | 33.1 MB |
| 150 | **6.71 MB** | 151 | 56.9 MB |

Perfectly linear, ~44.7 KB per entry, **zero evictions** — because `MAX_ENTRIES` is 1024
and 151 is nowhere near it. The 600-edit RSS run agrees: 19 MB → 116 MB, still climbing at
the last sample.

**Root cause.** `freeze.rs`'s cap is a count, and its comment reasons about the wrong
quantity: *"a page rarely has more than a few dozen cells, so this holds a deep edit
history while staying small on disk."* True of the **live set**; false of what the cache
actually stores, which is one entry per distinct cell **version**. For a text cell an entry
is a few hundred bytes and 1024 of them are trivial. For a plot cell an entry is a base64
PNG, so the same 1024 entries are ~45 MB — held resident **and** re-serialized to disk on
every save, since `save()` rewrites the whole file.

**Fix (shipped in this batch).** A `MAX_BYTES` budget of 16 MB per page alongside the
count cap, with a running byte total maintained on insert and eviction. 16 MB clears two
floors and stays under a ceiling: at least 2x `kernel::MAX_RICH_BYTES` (8 MB) so one huge
output cannot fill the budget alone; several hundred ordinary rich outputs, more history
than a session revisits; and small enough that the warm set (`MAX_WARM_PAGES` = 6) stays
proportionate to the kernels it already holds. Text-output pages are unaffected — the
entry cap still binds first for them. A single output larger than the whole budget is
still kept (`MAX_RICH_BYTES` permits 8 MB, and a cache that evicted its own newest entry
would re-run that cell forever).

**Verified by mutation:** dropping `|| self.bytes > MAX_BYTES` from the eviction condition
makes `eviction_bounds_total_bytes_not_only_entry_count` fail with *"cache holds 25165886
bytes, over the 16777216-byte cap"*, and `one_output_larger_than_the_budget_is_still_kept`
fail with it.

## Not measured, stated so it is not mistaken for a clean bill

- **Whether the per-edit rewrite costs warm-loop latency.** The probe's per-iteration time
  was flat (0.245 s/edit early, 0.260 s/edit at iteration 600), but it polls the served
  page every 200 ms, so the measurement is quantized far above the effect it would need to
  resolve. **This is not evidence that the rewrite is free.** A real answer needs
  server-side timing around `FreezeCache::save`.
- **Multi-hour wall-clock drift.** Still untested. This round substituted *volume* (1,000+
  executions) for *duration*, which is the better leak amplifier but not the same axis.
- **R kernels.** Only Python was driven.
- **Cold-build RSS peak** at 400+ pages built 16-wide, and `notify` at extreme directory
  counts: both still unchased from the AP1 original.

---

# Lens 2: docs-vs-behaviour drift, the behavioural half

## Method

Existence is already gated in both directions (`env_help_lists_every_runtime_env_var` ties
code to `--help`; `every_documented_env_var_is_in_the_user_guide` ties `--help` to the
guide; `every_parsed_flag_is_documented_in_its_subcommand_help` does the same for flags).
So this lens went after what those cannot see:

1. **Every documented default value** checked against the constant in source.
2. **Every YAML example the two books show a reader**, extracted and actually fed to
   `check` — front-matter blocks into a temp `.tmd`, with stub files minted for any path
   the example names so a missing-asset diagnostic could not be mistaken for a rejected key.

44 YAML blocks were extracted; 25 were front matter.

## DOCS-2 (high): the guide documents `about:`, a key that was removed nine days ago

`about:` was removed at **`dcf0588`** (2026-07-17, *"remove the retired about: block
(superseded by hero:)"*). That commit correctly scrubbed the code, the JSON schema, the
editor vocabulary, `site.css`, `AGENTS.md` and `crates/core/src/site/CLAUDE.md`. **It never
touched `docs/`.**

So the tool's behaviour and its manual disagreed completely: `validate_front_matter` warns
`unknown front-matter key 'about'` (pinned by `retired_about_key_warns_as_unknown`), while
the User Guide kept teaching it in **28 places across 6 pages**, including three whole
sections:

| Page | What it still claimed |
|---|---|
| `guide/reference/frontmatter.tmd` | a `## about:` section, a 4-row sub-key table, a full example, a key-table row, 3 prose mentions |
| `guide/reference/configuration.tmd` | a `#### about:` section, an example, a sub-key table, 3 prose mentions |
| `guide/using/formats.tmd` | a `### A profile header with about:` section, an example, a "reach for `hero:` vs `about:`" comparison |
| `guide/using/recipes.tmd` | a worked homepage recipe built on `about:` |
| `internals/validation.tmd` | a validation-surface table row naming **`ABOUT_KEYS`**, a constant that does not exist |
| `internals/sites.tmd` | a pipeline diagram node and a two-paragraph description |

**Severity is not cosmetic.** The warning fails `check`, `build --strict` and `publish`, so
a reader copying the guide's own homepage recipe cannot deploy it. And `CLAUDE.md` carried
the same stale line, so the drift was feeding back into how the tool gets worked on.

**Fix (shipped in this batch):** all 28 mentions removed or rewritten onto `hero:` (the
successor), the recipe reworked, `ABOUT_KEYS` corrected to `LISTING_KEYS`, and `CLAUDE.md`
updated. Both books `check` clean.

**Recurrence guard (shipped): `frontmatter::guide_vocabulary_gate`,** the third link in the
front-matter chain, mirroring the flag and env-var gates:

- `every_front_matter_example_in_the_guide_uses_only_real_keys` — every top-level key in
  every front-matter example under `docs/guide/` is in `KNOWN_KEYS`. This is the
  copy-paste surface, and it is the one that catches a removed key fastest.
- `every_nested_block_section_in_the_reference_names_a_real_key` — no `## \`name:\``
  reference section documents a key that no longer exists. A section for a removed key
  reads as a whole supported feature, which is worse than a stale table row.
- `the_reference_page_documents_every_known_key` — the completeness direction, since that
  page calls itself "the full vocabulary".

All three **failed before the fix**, each naming the exact file and line.

## DOCS-3 (low): the completeness gate immediately found two more

`footer:` and `logo:` are real `KNOWN_KEYS` (deck chrome) documented only in
`using/formats.tmd`, never on the reference page that claims to list every top-level key.
Fixed with a "Deck chrome" table.

## DOCS-4 (low): `configuration.tmd` states the wrong `theme` default

The row read ``| `theme` | `light` (default) / `dark`, or a `.css` file |``. An unset
`theme:` does **not** resolve to light — `theme_default_mode` returns `"auto"`, and the
pre-paint script follows the reader's OS `prefers-color-scheme`, falling back to light only
when the OS expresses no dark preference. A reader on a dark-mode OS gets a dark page from
a config the reference says defaults to light.

Two other pages state it correctly (`frontmatter.tmd`'s row and `theming.tmd`'s two
paragraphs), so this was one stale copy of a table, not a misunderstanding. Fixed, and the
same row was missing `.scss` files and `_extensions/` bundles.

## DOCS-5 (low): the guide teaches a pattern its own linter rejects

`frontmatter.tmd` told the reader: *"Leave [`image-alt`] unset and the card's image falls
back to an empty (decorative) `alt`."* The `TAL-A11Y-ALT` lint that shipped 2026-07-25
(PA-M13) warns on exactly that, and says to write `image-alt: ""` if it really is
decorative. The prose predates the lint and PA-M13's batch did not sweep the docs. Two
examples (`configuration.tmd`, `recipes.tmd`) also set `image:` with no `image-alt:`.
Fixed: prose rewritten to match the lint, both examples given real alt text.

*(Note: those examples never warned **as written**, because a fenced YAML block is prose,
not front matter — `an_image_key_inside_a_prose_example_is_not_front_matter` pins that.
They warn when a reader copies them into a real page, which is the point of an example.)*

## Defaults checked and found correct (do not re-check)

`TALIESIN_CELL_TIMEOUT` 120 s, `TALIESIN_RENDER_TIMEOUT` 30 s, `TALIESIN_JS_TIMEOUT` 10 s,
`preview` port 4321, `execute:` children all defaulting true, `listing:` `sort: "date desc"`.

## Method note worth keeping

**A table-scoped key diff needs the right table.** Diffing `NATIVE_KEYS` against
`configuration.tmd` reported nine undocumented site keys (`author`, `description`, `url`,
`head`, `body-start`, `body-end`, `publish`, `r`, `theorems`). **All nine were false
positives** — the `awk` range had swept in the listing sub-key tables and missed the
project table. Every one is documented; one `grep` each killed them. Same lesson the
previous docs round recorded about name-set diffs: a diff is a lead generator, and here the
*extraction*, not the comparison, was the thing that lied. The example-extraction approach
in this round had a 0% false-positive rate by contrast, because it ran the tool instead of
comparing two texts.
