# FV-5 instrument, and the portfolio re-checked at HEAD

**Run 2026-08-04**, against `main` = `253fa173` (the visual-minimalism merge). Two
outputs: the **FV-5 instrument is built** (the lens every prior round recorded as
blocked on method), and the feature portfolio is **re-measured at HEAD** rather than
carried over from the 2026-08-01 / 2026-08-02 rounds, both of which pre-date Waves 0-6
and the minimalism pass.

**This round did not cut anything.** Its one proposed cut was refuted by its own
evidence (§4), which is recorded here so a later round does not re-derive it.

---

## 1. The FV-5 instrument

`lsp*.rs` is the largest single feature in the tool and the only one no adoption round
has been able to see: shell history cannot observe a process the editor spawns. Both
prior rounds name it as their biggest blind spot and rank the measurement as blocked on
**method, not will**. `crates/server/src/lsp_trace.rs` is the method.

```sh
TALIESIN_LSP_TRACE=~/lsp-tally.json code .
```

```json
{ "sessions": 2, "methods": { "textDocument/completion": 812, "textDocument/hover": 91 } }
```

**The shape was set by the no-telemetry stance**, which is a product position rather
than an oversight: nothing leaves the machine, unset is off, and "off" is a branch on
`None` rather than a disabled-logger apparatus. The record is a method name and a count
— no ids, no document text, no timing — because that is exactly the FV-5 question.

**Why it counts rather than logs.** The window is a week of real writing, which spans
many editor restarts and would be millions of append-lines at `didChange` rates. The
tally is seeded from the file at startup and rewritten in place, so restarts accumulate
instead of overwriting. `sessions` is what distinguishes *never invoked* from *never
armed* — without it, a zero is unreadable. A flush every 25 records makes a SIGKILL
(which skips `Drop`) survivable.

**Reading it back is deliberately not a subcommand.** The round this serves exists
because surface must earn its place; a dev instrument does not earn a nineteenth verb.
It is JSON.

**Arming it in VS Code is the fragile step, so it announces itself.** The extension host
inherits its environment from however VS Code was launched, so `TALIESIN_LSP_TRACE=…
code .` from a terminal works and the desktop launcher does not see a `.zshrc` export.
The server logs `lsp: capability trace armed → <path>` to stderr on startup, visible via
the companion's existing **Show Language Server Log** command. Check that line before
starting the week; an instrument you cannot tell is running is one you find out was off
afterwards.

**Verified against the real binary over stdio, not only by unit test** (the
[verify-by-running](LESSONS.md) rule): six capabilities driven through a real
`taliesin lsp`, all eight methods tallied; a second run against the same file gave
`sessions: 2` with counts accumulated; an unset environment created no file and logged
nothing. Plus 6 unit tests, 757 `--bin taliesin`, 11 `lsp_stdio`, `fmt` + `clippy` clean.

**What to do with the result.** After a week, a capability at or near zero is a
deletion candidate with evidence — which is the first time anything in `lsp*.rs` has
had any. A capability in the thousands is load-bearing and the question closes.

---

## 2. The portfolio at HEAD

Measured, not carried over. Prior rounds' numbers pre-date ~6,900 lines of wave cuts and
the minimalism pass's ~4,119.

| | measured |
|---|---|
| Rust | **122,861** lines — implementation **63,993 (52%)**, test **58,868 (48%)** |
| Bundled JS / CSS | 5,371 / 3,138 |
| web-client | 3,032 |
| Document constructs | 126 known, 112 used, **14 unused** |
| `_site.yml` native keys | 14 |
| CLI verbs | 18 |

**Cost of the largest features**, LOC including their dedicated tests:

| feature | LOC | note |
|---|---|---|
| LSP | **15,592** (impl 6,441) | largest by 2.8x; value unmeasured until §1 runs |
| deck engine | 5,592 | ruled frozen 2026-08-02, not reopened here |
| citations | 2,550 | 20 documents |
| `{pyodide}` | 2,119 | already a non-default cargo feature |
| `{js}` reactive | 1,778 | most-adopted novel construct |
| `run` | 1,676 | shipped Wave 6 |
| print / PDF | 1,621 | **zero invocations, ever** |
| search (Cmd-K) | 1,459 | |
| publish | 1,100 | the only feature with a real successful deploy trace |

### The finding neither prior round measured

**Test code is 48% of the Rust.** Every feature kept carries roughly a line of test per
line of implementation, on top of its drift gates (a front-matter key trips five; a
retired one, eight). This is the multiplier on every keep/cut decision and it means the
executed waves bought roughly double the LOC they booked. It is also why "average
utility per feature" is a real quantity and not a figure of speech.

### Adoption, split by authorship

From the tool's own `taliesin features . --format json` over 190 documents. Real writing
= `docs/` + `site/` + `corpus/tech-blog` + `corpus/tarn`.

`title` 71 real · `description` 63 · `{python}` 24 · `note` callout 24 · `echo` 15 ·
`{js}` 14 · `@fig-` 13 · `label`/`fig-cap` 13 · mermaid 11 · `{{< include >}}` 6 ·
theorem kinds **1** (one document, one of eight kinds) · `{glsl}` / `{pyodide}` / `num`
**0**.

Twelve constructs are used by nothing anywhere; per the 2026-08-03 sheet, five of those
are deliberate aliases or catalogue entries and cost nothing.

### The CLI, re-measured

`preview` **58** · `build` 7 · `check` 5 · `read` 3 · `vocab`/`schema`/`mcp` 1 · all
others **0**, including `pdf`, `run`, `new`, `doctor`.

> **The probe lied first, exactly as recorded.** The first table came back all zeros.
> `~/.zsh_history` is *Non-ISO extended-ASCII*, so grep treated it as binary and
> suppressed output; `grep -a` plus a control row (84 launcher invocations total) fixed
> it. This is the third recurrence of the all-negative-table trap in this family. Always
> carry the control row.

---

## 3. The books and the site are in good condition — measured

**The docs are not stale.** Sixteen names of things the waves deleted were grepped
across `docs/` and `site/`; eleven files hit, and **every hit is a false positive**:

| grep | what it really was |
|---|---|
| `taliesin render` | the prose "taliesin **renders** one author's own `.tmd` files" |
| `taliesin serve` | "taliesin **serves** one author's own documents" |
| `taliesin dev` | the noun "the taliesin **dev server**" in a `description:` |
| `skim` | "the right flag for **skim**ming a stranger's document" |
| `lightbox` | the deliberate **removal register** explaining why it is gone |
| `.aside` / `marginnote` | the documented alias list (2026-08-03 sheet §2) |
| `prose-lint` / `datasets:` / `columns` | `from-quarto.tmd`, where a retired key **must** tell a migrating reader what to do |

`crates/core/tests/stale_docs.rs` is green on all 7 gates, including
`shipped_docs_do_not_name_a_file_that_does_not_exist` and
`documented_cli_flags_exist_in_the_cli`.

**A grep hit is not a finding.** This is the fifth recorded instance of a confident-wrong
"does this exist" answer from an unread grep; the cost of reading eleven contexts was
about a minute.

### Why the tool's cut logic does not transfer unchanged to docs

An unused **feature** costs code, drift gates, test surface and recall load forever, so
cutting it raises average utility. An unread **doc page** costs a reader one glance at a
nav item — the whole manual is 6,116 lines against 122,861 of Rust — and deleting it buys
strangers arriving with questions the manual used to answer, while shrinking the dogfood
that pins the book format. The analogous lever for docs is *delete what documents things
that no longer exist* (already enforced, and green) and *merge pages that answer the same
question* (attempted below, and refuted).

---

## 4. The one proposed cut, and why it was withdrawn

**Proposed:** fold `site/formats.tmd` (69 lines) into `site/index.tmd`, on the reading
that five marketing pages answer one question ("what can it do") and that `index.tmd`
already carries a *One source, many outputs* section — the spec's own new rule, "no
second mechanism for a job that already has one", applied to attention rather than code.

**Refuted by the inbound-link graph.** `index.tmd`'s section is a four-card grid whose
cards link **into** `formats.tmd#blog-posts`, `#slide-decks` and `#websites`. It is
hub-and-spoke, not duplication. `formats.tmd` also carries:

- 3 deep anchor links from `index.tmd`, 3 from `features.tmd`, 4 from the User Guide
  (`getting-started.tmd`, `recipes.tmd` ×3) — **10 inbound links**;
- a nav slot in `site/_site.yml`;
- the live `{{< embed demo.tmd >}}` deck;
- a named entry in `stale_docs.rs:235`.

The proposal was derived from **headings alone**. Reading the two pages and the link
graph took ten minutes and reversed it. Recorded so a later round does not re-derive
"four pages for one job" from a nav listing: **each of `features` / `formats` /
`showcase` / `gallery` answers a different question** (the capability list, the four
outputs, interactive demos, whole real projects), and `index` deliberately teases into
them.

**Structural inventories keep undercounting and mis-reading cross-layer things.** The
minimalism pass recorded the same failure in the other direction (17 of 17 tasks found
its file lists incomplete). A page's role is in its inbound links, not its headings.

---

## 5. Publish readiness

Checked, not assumed. Two block; two are housekeeping.

| | state |
|---|---|
| Repo | **private** |
| CI | **inert** — every job guarded on `private != true`, so nothing has been certified by Actions; only `.githooks/pre-push` has gated anything |
| `taliesin.dev` | **does not resolve**, and `site/_site.yml` sets it as canonical `url:` |
| Releases | none; one local non-`v*` tag, unpushed |
| LICENSE / README / THIRD_PARTY | present. Root `AGENTS.md` is *not* missing — it is a scaffolded asset (`taliesin init`), gated and green |

**Blocking, and both are owner calls:**

1. **The canonical URL is a domain that does not exist.** It feeds Atom feeds,
   `llms.txt`, SEO canonical tags and social-card metadata, so every shared link at
   launch resolves to nothing. Register it, or point `url:` at where the site will live.
2. **Seven pages, including the marketing homepage, load Three.js from `esm.sh` at view
   time.** The first page a stranger opens makes a third-party request, against the
   repository's own stated offline guarantee, and it is declared nowhere a reader sees —
   only in the build log, which still exits 0. Defensible as a trade; currently an
   *undeclared* one.

---

## 6. What this round did not measure

- **The LSP's actual value.** §1 builds the instrument; it has not been run. That is a
  week of writing, not a session.
- **Reader-side value** (FV-6). Still needs an outside human; still should not be run as
  a desk exercise.
- **Architectural accuracy of `docs/internals/`.** `stale_docs.rs` gates file-path claims
  and retired vocabulary, not whether a prose description of a subsystem still matches it
  after six waves rewrote parts of it. `deck-engine.tmd` documents a frozen feature.
  Worth one pass; not attempted here.
