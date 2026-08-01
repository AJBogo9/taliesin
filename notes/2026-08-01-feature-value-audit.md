# Feature value audit — the portfolio lens

**Run 2026-08-01.** The question no prior round has asked: *of everything Taliesin
ships, what earns its keep?* Sixty-odd rounds have audited whether features **work**
(correctness, a11y, chaos, concurrency, mobile, determinism) and whether the tool is
**wanted** (PMF, demand probes, positioning, adoption friction). None has ranked the
**feature portfolio itself** for keep / improve / cut. `AUDITS.md` requires a new lens
rather than one off a list; this is one.

**Scope:** the user-facing surface — what an author writes, what a reader gets, what the
CLI exposes, what the editor provides. Not internal module structure (the 2026-07-17
reduction audit owns that).

**Stance:** this round is willing to name features for deletion. That is what it is for.
Where the evidence says "healthy, leave it alone", it says that too — a cut list padded
to look productive is worse than a short one.

---

## Method

Value was measured, not asserted. Four instruments:

1. **Adoption across all 185 `.tmd` documents**, split into *author documents* (33:
   `corpus/tech-blog` the personal blog, `site/` the marketing site, `corpus/tarn` the
   book) vs *the dogfooded manual* (`docs/`, 40) vs *pin fixtures* (`corpus/`, 112).
   The split is load-bearing: a feature used only in its own pin doc is a feature the
   author who built it has never reached for.
2. **Front-matter keys parsed out of the actual YAML block**, not grepped from body text
   — a reference page *documenting* `doi:` is not a page *using* it.
3. **The author's real shell history** (`~/.zsh_history`, 6,265 lines) for CLI subcommand
   invocation counts.
4. **A real build** (`taliesin build docs/guide`, 26 pages) for shipped-byte cost per
   feature.

**Caveats, stated up front.**

- `target/release/taliesin` reports `0.2.0 (13d95219)` while HEAD is `5f61ef04` — it was
  built from another session's branch. Build-derived numbers are from that binary.
- Shell history undercounts the **machine-facing** commands (`read`, `map`, `skim`,
  `mcp`) — those are meant to be called by an agent, not typed. It also undercounts
  `lsp` (spawned by the companion) and one-shot setup (`completions`, `init`). Absence
  from history is signal for `pdf`/`render`/`blocks`/`symbols`, not for those.
- Adoption is a *lagging* indicator, and corpus-plus-roadmap deliberately ships a feature
  with only its pin. "Pin-only" is expected on day 1 and damning at week 6. Age is
  reported with every adoption number for exactly this reason.

**Two instrument traps hit and corrected during the round** (both already in `LESSONS.md`
in other forms — they recur):

- The first adoption probe returned **every cell zero**. zsh does not word-split
  unquoted parameters, so `grep $FILES` searched stdin. Caught by the backlog's own rule
  ("an all-negative table is a broken probe"); re-run with `${(f)...}` arrays and a
  known-positive control row.
- "Mermaid is loaded by 24 of 26 pages" was **false** — the inlined-asset needle trap.
  Those 24 hits are the word *mermaid* in comments inside the inlined `app.js`. Needling
  the actual `<script src="…/mermaid.<hash>.js">` tag gives **2 of 26**, and the
  conditional-bundling gate is working correctly.

---

## Headline findings

### F1. The front-matter vocabulary is the most expensive surface per unit of value in the tool

`KNOWN_KEYS` holds **33 keys**. The author's 33 real documents set **15** of them.
Eighteen keys are set by exactly one document each — their own pin — or by none.

The cost is not the parse. The backlog records it precisely: **a new front-matter key
trips six drift gates** (`KNOWN_KEYS`, the JSON schema, the editor vocab, the asset
`AGENTS.md`, the guide-reference completeness gate, and the repo-root `AGENTS.md` whose
test lives in the *server* crate). Every key also owes a diagnostic, a did-you-mean
neighbourhood, a docs row and a vocab entry. **That is the highest fixed cost per feature
anywhere in the tool, and it is being spent on keys nobody sets.**

This is the finding to act on first, because it is not about any one key — it is about
the *default answer* to "how should this capability be expressed". The project already
has the better answer written down and proven: **derive, don't declare.** Item 185
derived `citation_arxiv_id` from the `links:` host; 184 derived affiliation numbers from
first appearance. Each is a key that cannot be got wrong because it does not exist. The
rule deserves promotion from a batch note to a standing constraint.

### F2. Build effort and author demand have diverged over the last three weeks

Ranked by adoption, the **most-used novel capability in the tool** is the
interactive/explorable surface: `{{< input >}}` (50 uses), `{js}` cells (40 documents),
`{{< embed >}}` (33), `{{< video >}}` (27). Ranked by *recent effort*, the last three
weeks went almost entirely elsewhere:

| Shipped | Feature | LOC | Documents that use it |
|---|---|---|---|
| 2026-08-01 | resource row / venue / award (185) | 321 | 1 (its pin) |
| 2026-08-01 | appendix / contributions (187) | 306 | 1 (its pin) |
| 2026-07-31 | `{pyodide}` cells | 530 + 12.9 MB | 1 (a guide showcase page) |
| 2026-07-31 | print / PDF track | 1,496 + 503 KB | 1 (its pin) |
| 2026-07-29 | `{glsl}` cells | 207 | 1 (a guide showcase page) |
| 2026-07-29 | `num` / numerics | 420 | 1 (a guide showcase page) |

Every one of those is defensible individually and pinned as the policy requires. The
finding is not that any is wrong — it is the **portfolio shape**: ~3,300 LOC and 13.4 MB
of payload landed in three weeks into the part of the surface the author's own writing
has never touched, while the surface the author actually writes with got none of it.

The research-publishing cluster is the sharpest case. It is well-built and it shipped
today, so it is genuinely too early to judge — but it is worth naming plainly that
**the author has no papers**, and the cluster's remaining item (188) is the one the
backlog already marks lowest-conviction. This audit's answer to "188 against 164/167",
which the backlog records as an unmade owner call, is: **neither first** — see F5.

### F3. Asset conditionality is healthy. Do not "fix" it

Measured on a real 26-page build, counting actual `<script>`/`<link>` tags:

| Asset | Size | Pages that load it |
|---|---|---|
| `mermaid.js` | 3.5 MB | **2 / 26** |
| `jslibs.js` (d3 + Plot) | 503 KB | **1 / 26** |
| `katex.css` | 369 KB | **4 / 26** |
| `deck.js` | 133 KB | **2 / 26** |
| `app.js` (the base bundle) | 92 KB | 24 / 26 |

The gate works. A reader of a prose page pays 92 KB of JS and nothing else. Item 137's
verdict stands and is unchanged by this round: what remains is a *deploy/storage* cost
(unreferenced files still copied), not a transfer cost, and it is not worth much.

**The real payload cost is in the binary, not the page.** `pyodide.rs` `include_bytes!`s
a 12.9 MB WASM runtime into the executable; `target/release/taliesin` is **101 MB**. Every
user downloads that for a capability exercised by one showcase page. The premortem audit
already named "the evaluation funnel has one very expensive step" as a top-three failure
mode; this is now the largest single contributor to it.

### F4. Nineteen CLI subcommands; the author invokes eight

From 6,265 lines of shell history: `preview` 62 (+ `dev` 2), `build` 7, `check` 5,
`read` 3, `publish` 2, `vocab` 1, `schema` 1, `mcp` 1. **Zero** hand-invocations of
`pdf`, `render`, `map`, `skim`, `new`, `symbols`, `blocks`, `init`, `completions`,
`doctor`, `lsp`.

Discounting the ones history structurally cannot see (`lsp`, `mcp`, `map`, `skim`,
`read`, `completions`, `init`, `new`) and the one whose value is on the bad day
(`doctor`), three remain that are genuinely near-dead: **`render`** (superseded by
`build`), **`blocks`** (a debug dump, and the only subcommand with *zero* documentation
pages), and **`symbols`**.

Also: `preview` already has **three** spellings (`preview | dev | serve`), and the
history shows the author twice typed a **fourth** (`tali view`) that does not exist. More
aliases is not the fix; it is evidence that aliasing does not solve recall.

### F5. The tool cannot answer "what does this document use" — and that is the highest-value gap

This audit took a working session of grep to produce, and it is the kind of question the
tool should answer about itself in a second. The render pipeline already knows every
construct it expanded — every shortcode, div class, cell language, front-matter key,
theorem kind. Nothing surfaces it.

A `taliesin features <dir>` report (feature → documents using it) would:

- make the **corpus-plus-roadmap policy self-checking** — the policy says every
  capability ships pinned by a corpus doc, and this round found keys documented in the
  guide with no corpus pin at all (`include-in-header`, `include-before-body`,
  `include-after-body`, `logo`; all four *are* unit-tested, so this is a pin gap, not a
  coverage gap);
- make **every future version of this audit free**, turning a one-off session into a
  standing signal;
- tell an author porting a document which constructs it depends on.

It is small (the data exists), it rides `read`/`map`'s existing projection machinery, and
it is the only new feature in this round that makes the *other* findings cheaper to act
on. **Rank it above 188, 164 and 167.**

---

## The ranking

Five tiers by value, with the evidence each rests on.

### T0 — load-bearing. The moat. Never touch

Not features so much as the architecture features hang off. The ATAM round measured
click-to-source at 90–97% on eight real documents; the value-stream round measured a
prose edit reaching the DOM in **90 ms** with zero cells re-run.

- The block model: `data-block-id` (content hash) + `data-sourcepos` on every block
- Block-level diff and incremental swap (`diff.rs`)
- Click-to-source (preview → editor cursor, navigate-only)
- Warm dev server + warm Jupyter kernel
- The cumulative-hash freeze cache — the positioning round found this is the one
  capability *nobody else has and users are actively complaining about elsewhere*
  (Quarto's tracker carries stale-`freeze` bugs; the cumulative key makes a stale hit
  structurally impossible)

### T1 — core. Used constantly, in real documents

| Feature | Evidence |
|---|---|
| `preview` | 64 invocations, the daily loop |
| Markdown → HTML + server-side highlighting | every document |
| KaTeX math | 4/26 pages on the guide; the blog is ML/stats |
| Callouts (note/tip/warning/important) | note 29 docs, tip 10, warning 6, important 6 |
| `{{< include >}}` | 59 uses |
| `{python}` / `{js}` / `{r}` cells | 40 / 29 / 16 documents |
| `build` | 7 invocations; ships everything |
| `check` + the diagnostics engine | 5 invocations, 27 docs pages; the pre-publish gate |
| Site model: nav, listings, feeds, books, search | every real project turns them on |
| Citations + bibliography | 12 real documents |
| The LSP + companion | invisible to history, but it is the editing surface |

### T2 — earning its keep, narrower

Real use, clearly below T1, all worth keeping.

`{{< embed >}}` (33) · `{{< video >}}` (27) · `{{< input >}}` (50) · `{{< dataset >}}`
(11) · decks · `panel-tabset` (4 real docs) · `magic-move` (4) · `code-walkthrough` (3) ·
`scrolly` (2) · theorem environments (2 real, including a genuine blog post,
`corpus/tech-blog/posts/em-algorithm/`) · the reader affordances (all 13 enhancers
verified registered in `09-register.js`; `04-focus-trap` and `16-scroll-a11y` are shared
helpers / self-initialising IIFEs, **not** dead code — checked) · `image_opt` · `minify` ·
backlinks · link-preview · `doctor` (zero invocations, but its value is on the day
nothing works, and the first-contact round called it *exceptional*).

### T3 — provisional. Shipped ≤3 days ago; verdict deferred, not granted

Judging these now would be judging them on their ship date. Each gets a **review date**
rather than a verdict, and each should be re-measured then with the same adoption probe.

| Feature | Shipped | Review on | The question |
|---|---|---|---|
| Research-publishing cluster (185/186/187) | 2026-08-01 | 2026-09-15 | Has any real document set `doi:`/`links:`/`venue:`/`award:`? |
| `{pyodide}` cells | 2026-07-31 | 2026-09-15 | Does anything beyond the showcase page use it, given 12.9 MB of binary? |
| print / PDF track | 2026-07-31 | 2026-09-15 | Has `pdf` been invoked once outside a test? |

### T4 — cut, fold, or freeze

The candidates, each with its measurement. See the verdicts below for what to do.

| Feature | Age | Documents using it |
|---|---|---|
| `columns` fenced div | ~6 wk | **1** genuine (`corpus/media/gallery.tmd` — which uses `layout-ncol` too) |
| `caution` callout | ~6 wk | **0** outside `corpus/callouts/kinds.tmd` (the all-kinds pin) |
| `prose-lint:` | 5 wk | **0** (its pin + the reference page) |
| `datasets:` provenance keys | — | **0** (its pin + the reference page), while `{{< dataset >}}` has 11 uses |
| `{glsl}` cells | 3 days | **1** (guide showcase) |
| `num` / numerics | 3 days | **1** (guide showcase) |
| `render`, `blocks`, `symbols` | — | 0 invocations; `blocks` has 0 docs pages |
| `include-in-header` / `-before-body` / `-after-body` | — | **0** — documented, unit-tested, **no corpus pin** |

---

## Verdicts

### Cut

**C1. `columns` — remove; keep `layout-ncol`.** Two mechanisms for one job. Across 185
documents `columns` has one genuine authored use, in a document that *also* uses
`layout-ncol`; the other two hits are a typo fixture and the generated tour deck. This is
inherited Quarto vocabulary that never took. Removing it needs a retired-key diagnostic
(the `about:`/`number-within:` precedent) so a leftover says "removed", not "misspelled".

**C2. `prose-lint:` — cut, or fold into `check`.** 282 LOC, five weeks, zero adoption by
an author who writes daily. A prose linter that is opt-in per document is a linter nobody
turns on. If the capability is wanted it belongs behind `check --prose`, where the author
already goes; as a front-matter key it costs six drift gates for a feature with no user.

**C3. `datasets:` provenance keys — cut the keys, keep the shortcode.** `{{< dataset >}}`
has 11 uses and works; the `datasets:` annotation block has zero. The card already derives
what it needs from the file. This is the clearest available instance of F1: a declared key
where the derived value was already sufficient.

### Fold / alter

**C4. `render`, `blocks`, `symbols` — fold into two flags and a namespace.** `render` is
`build` to stdout. `blocks` and `symbols` are debug dumps; `blocks` is the only subcommand
with no documentation at all. Nineteen top-level subcommands is not a single-purpose
tool's surface. Fold `render` into `build --stdout`, move `blocks`/`symbols` under one
`inspect` namespace or drop them.

**C5. `caution` callout — keep, and this is deliberate.** Zero real use, but it is one
line, it completes a vocabulary readers of Pandoc/Quarto/GitHub already know, and removing
it would make the did-you-mean machinery answer `caution` with a wrong neighbour. The
`csl:` precedent applies: recognising something costs less than mis-correcting it. Listed
here so a later round does not re-derive it as a cut.

**C6. Do not add a fourth `preview` alias.** The author reaching for `tali view` is real,
but three spellings already failed to catch it. If anything, `dev` and `serve` are the
ones to consider retiring — one name, documented, beats four.

### Freeze

**C7. `{glsl}` and `num` — freeze, do not extend.** Three days old, one showcase page
each, cheap (207 + 420 LOC, no reader cost when unused). Not cut — too new, and they cost
almost nothing. But they should acquire no further investment until a real document asks,
and they go on the same 2026-09-15 review as T3.

**C8. `{pyodide}`'s 12.9 MB belongs outside the binary.** Not a cut — the feature is good
and its *page*-level conditionality is already correct. But `include_bytes!` puts it in
every executable, and a 101 MB binary is a direct hit on the evaluation funnel the
premortem round ranked in its top three. Download-on-first-use, or a build feature flag.

---

## Improve (existing features that under-deliver relative to their cost)

**I1. The front-matter surface needs a standing rule, not another key.** Promote
"derive, don't declare" from a batch note to the standing constraints in `backlog.md`,
with the six-gate cost stated as the reason. Every proposed key should have to answer:
what on the page already implies this?

**I2. `skim` (934 LOC) overlaps `read` and `map`.** Three machine-facing projections plus
`llms.txt` plus the search index — the backlog already calls this "the four-projection
sweep" and treats it as a tax every new block pays. That is a lot of surface for one
consumer class. Worth one design pass asking whether it is three projections or one with
three renderings; not worth building anything until then.

**I3. `{{< dataset >}}` should derive its provenance** rather than read `datasets:`
(follows from C3).

**I4. Four documented keys have no corpus pin.** `include-in-header`,
`include-before-body`, `include-after-body`, `logo`. All are unit-tested, so this is a
policy gap rather than a correctness risk — but corpus-plus-roadmap says the corpus is
the arbiter of done, and these were never arbitrated. Either pin them or drop them.

---

## Build (new features, ranked)

**N1. `taliesin features <dir>` — the adoption report.** See F5. Highest value in this
round: it is small, the data already exists in the pipeline, it makes the project's own
policy self-checking, and it makes every future portfolio audit free. Should rank above
188, 164 and 167.

**N2. `check --online` (already item 167).** Independently corroborated here: the R11
round found 118 of 123 errors on a real external book were link breakage. The docs books
cross-link heavily and nothing checks them. This audit raises its priority; it does not
re-file it.

**N3. Invest the next capability slot in the explorable/reactive surface.** Follows
directly from F2: `{{< input >}}` (50) and `{js}` (40 documents) are the most-adopted
novel things Taliesin has, they are what the marketing site leads with, and they have
received no investment while ~3,300 LOC went to surfaces with one user apiece. This is a
*direction*, not an item — it needs the normal brainstorm to become one.

**Explicitly not proposed:** no new output formats, no preview write-back, no CDN, no
reader-side backend. Every one of those is a standing guardrail, and nothing in this
round's evidence argues against any of them.

---

## Healthy — measured, leave alone

Recorded so a later round does not spend a session rediscovering them.

- **Conditional asset bundling** (F3). Working correctly; measured per-tag, not per-grep.
- **The reader-affordance registry.** All 13 built-ins registered; the two unregistered
  fragments are a shared focus-trap helper and a self-initialising IIFE. No dead code.
- **The callout family, `{{< include >}}`, `{{< embed >}}`, `{{< video >}}`,
  `{{< input >}}`.** The five highest-adoption authored constructs in the tool. Nothing to
  do.
- **`doctor` and the front-matter typo suggestions.** The first-contact round called them
  exceptional and this round found nothing better to say.
- **The freeze cache's cumulative key.** The single most defensible differentiator the
  tool has, per the positioning round.

---

## Successor rounds — the feature-importance family

This round measured **adoption**. Adoption is one axis of value and the cheapest to get,
but it is not the strongest. Six successor lenses, ranked by what they would add that
this one structurally could not.

`AUDITS.md`'s rule applies to every one of them: **a round whose only output would be to
rebuild rows that already exist is not worth running.** Each entry below therefore names
its *kill condition* — the thing that would make it not worth a session.

### FV-2. The ablation round *(strongest successor; run this next)*

**Adoption measures use. Ablation measures dependence, and dependence is what "earns its
keep" actually means.** For each T4 / T3 candidate: remove it, run the corpus, count what
breaks. A feature whose deletion breaks exactly one document — its own pin — is a feature
nothing depends on, and that is a far harder number than "one document uses it".

*Method.* Branch per feature; delete the emission path; `cargo test -p taliesin-core`;
record failures by document. Cheap and mechanical. `git restore` at the end — but **commit
first**, per the mutation-testing footgun: a `git checkout` on uncommitted work restores
from HEAD and silently eats the implementation.

*Why it is strong here.* It converts every "cut?" verdict in this document from a
judgement into a measurement, and the corpus already exists to make it free.

*Kill condition.* If it is only run against the three features already named for cutting
(203/204/209), it rebuilds this round's conclusions. Run it across **all** of T2–T4 or not
at all — the interesting result is a T2 feature that turns out to be load-bearing, or a
T1 feature nothing actually depends on.

### FV-3. The cost-to-carry round

This round priced features by **LOC and shipped bytes**, which is the cost to *build*, not
the cost to *keep*. The keeping cost is: how many gates a feature trips, how often its code
is touched by unrelated changes, and how many defects it has produced.

*Method.* `git log --numstat` per feature file to get churn; count backlog items
historically filed against each area (`AUDITS.md` + git history are the source); count
drift gates per feature (front matter = 6, a `--tali-*` token or `data-*` attribute = the
`token_contract.rs` census, a new generated block = the four-projection sweep).

*Why it matters.* A feature can be genuinely used and still be a net loss if it taxes every
unrelated change. That is the one shape this round is blind to — `columns` is cheap to
carry and unused; something used and expensive would rank worse and this round would miss
it.

*Kill condition.* If churn turns out to concentrate in `render/mod.rs` and `build.rs`
regardless of feature (likely — they are the hubs), the per-feature signal is noise and the
round ends after the first measurement. Check that first, in ten minutes, before committing
a session.

### FV-4. The cognitive-surface round *(closest to "fits the hand like a glove")*

The stated goal is a tool that fits the hand. This round counted what features *cost the
project*; it never counted what they **cost the author to hold in their head**. Total
vocabulary today: 33 front-matter keys + ~25 `_site.yml` keys + 6 shortcodes + ~20 div
classes + cell options + cross-ref prefixes + 19 subcommands.

*Method.* Count the vocabulary a user must know to reach each of the four formats, then
measure how much of it the tool *teaches at the moment of need* (completion, did-you-mean,
`new` scaffolds, hover) versus how much must be recalled cold. The first-contact round has
the walkthrough method; this is that method aimed at breadth rather than the first two
minutes.

*Why it matters.* Bloat is felt as *recall load*, not as binary size. The `tali view`
observation is a data point of exactly this kind: the author reached for a command that
does not exist, in a tool they wrote.

*Kill condition.* If the LSP already covers the vocabulary at point-of-need (it may — 14
capability providers), the load is near zero and the round should stop and say so.

### FV-5. The LSP / editor value round *(blocked on method, not on will)*

~11,300 LOC across `lsp*.rs` is **the largest single investment in the tool** and this
round could not see one byte of its usage: shell history cannot observe a
companion-spawned process. Fourteen capability providers are advertised; nothing says
which fire during real writing.

*Method options, none free.* (a) A deliberate observation session — write for an hour,
record which features you actually invoked. (b) Log LSP method names to stderr behind an
env var, local-only, never shipped on. (c) Ablate a provider and see whether the week
feels worse. **(b) is the only one that scales, and it must stay off by default** — the
no-telemetry stance is a product position, not an oversight.

*Kill condition.* If the answer is "completion, hover and diagnostics, everything else is
noise", that is worth knowing once and never again. Run it once.

### FV-6. The reader-side value round *(hardest; needs a person)*

Every number in this document is **author** adoption. Nothing here measures whether the
reader menu, link previews, reading progress, Cmd-K or the lightbox are used, and with the
no-telemetry stance nothing can — without asking someone.

*Method.* Ask two or three real readers of the blog to think aloud, or run the
`FEATURE-IDEAS.md` reader-persona probe against a real post. This is the one lens that
needs an outside human, which is why it keeps not happening.

*Kill condition.* Do not run it as a solo desk exercise. A reader round with no reader is
the author guessing, and this round already has enough of the author's own signal.

### FV-7. The inherited-vocabulary round

`columns` was cut here because it is Quarto vocabulary that never took. **How much more of
the surface is inherited rather than chosen?** Cross the full feature list against
[2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md) and separate
*"we support this because Quarto did"* from *"we chose this"*.

*Why it matters.* The project has already ruled the aggressive shed-Quarto break, keeping
Markdown/Pandoc **syntax** while improving every decision above it. Inherited features are
where that ruling is least likely to have been applied, because nobody ever decided them.

*Kill condition.* The triage doc's own caveat governs: its heading status is degenerate and
its executive summary misleading, and a skeptic verdict is evidence, never a ruling (its
"drop Atom feeds" call was overruled). If the round turns into re-litigating that catalog,
stop.

### FV-8 (filed as item 208). Re-measure the provisional features on 2026-09-15

Not a new lens — the dated re-run of this one's adoption probe, so three deferred verdicts
do not become permanent passes by default.

---

## What this round did NOT measure

- **Reader-side value.** Every adoption number here is *author* adoption. Nothing
  measures whether readers use the reader menu, link previews, or Cmd-K — and with the
  no-telemetry stance, nothing can without asking someone.
- **The LSP surface item by item.** ~11,300 LOC across `lsp*.rs` is the largest single
  investment in the tool and history cannot see it. It deserves its own round, with the
  companion's own telemetry-free instrumentation or a session of deliberate observation.
- **Whether any T4 cut is safe to execute.** Each verdict above is a recommendation with
  its evidence; none was implemented, and each needs the normal retired-key diagnostic
  work before it lands.

---

## Filed to the backlog

| Item | Band | What |
|---|---|---|
| **202** | P1 | `taliesin features <dir>` — the adoption report (F5/N1) |
| **203** | P1 | Remove the `columns` fenced div; keep `layout-ncol` (C1) |
| **204** | P1 | `{{< dataset >}}` derives provenance; retire the `datasets:` keys (C3) |
| **205** | P1 | Take pyodide's 12.9 MB out of the binary (C8) |
| **206** | P2 | Fold `render` / `blocks` / `symbols` off the top level (C4) |
| **207** | P2 | Four unpinned front-matter keys + promote "derive, don't declare" (F1/I4) |
| **208** | P2 | Re-measure the provisional features on 2026-09-15 (T3) |
| **209** | P3 | `prose-lint`: cut, or fold into `check --prose` — owner ruling (C2) |

The T3 review date is carried by item **208** so the provisional verdicts do not quietly
become permanent passes. **209 is a ruling, not a task**: it is the deletion of a working,
tested, documented feature, and this round measured demand, not intent.

Not re-filed, only re-ranked: **167** (`check --online`) — this round independently
corroborates it and raises its priority. **N3** (invest the next capability slot in the
explorable/reactive surface) is deliberately *not* an item: it is a direction, and it owes
the normal brainstorm before it becomes one.
