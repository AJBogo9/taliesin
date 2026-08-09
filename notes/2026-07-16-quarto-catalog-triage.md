# Quarto design-decisions catalog: triage (2026-07-16)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Triage of `docs/superpowers/specs/2026-07-03-quarto-design-decisions-catalog.md` (branch
`quarto-decisions-catalog` @ `535b4e1`, 165 decisions), per backlog §E: rule each by **"is this the
right design for Taliesin"**, now that the 2026-07-07 repositioning retired Quarto as the reference.

**Status: 34 of 165 triaged (wave 1 COMPLETE: crossref, citations, slides, config).** Method: 4 parallel
agents, one per area, each verifying every "Taliesin today" claim against source and returning a
recommended verdict + evidence. Every headline claim below was re-verified by hand before landing here.

## The catalog has three layers, and they deserve very different trust

**Read this before using the catalog.** It is not uniformly reliable, and the unreliable part is the
part you read first.

1. **The entries are the real asset, and mostly well-grounded *as of 2026-07-03*.** Each carries a
   verified "Taliesin today" and a skeptic-revised recommendation. D140 cites `diagnostics/reactive.rs`,
   which landed 2026-06-29, four days before the catalog: the grounding worked.
2. **The heading status is degenerate. Ignore it.** The skeptic returned `revise` on **162 of 165**
   entries, which mechanically produced 94 headings reading "Proposed (revised)" regardless of what the
   entry concluded. D2 concludes "keep as-is, decline the proposal" and D140 concludes "already built";
   both are labelled the same as a genuine proposal. The real verdict lives in the entry's
   **"Revised recommendation"** prose.
3. **The executive summary is misleading and should be rewritten or discarded.**
   - It claims "Every entry is tagged taliesin-already-better, proposed-change, or open-question."
     **No such tags exist** anywhere in the file.
   - It claims "about forty are concrete proposed-changes, and around thirty are genuine open
     questions." The headings say **94 and 1**.
   - Its **"Highest-impact decisions to rule on first"** list is 11 verbatim entry titles, and all 11
     carry the same degenerate status. So it mixes genuinely open questions (D131 committed `_freeze/`,
     D141 interactive-kernel-output stance) with **already-shipped work** (D140), and nothing
     distinguishes them. Two of its 11 are config items that wave 1 refutes (see D33/D34 below).

**Plus: the entries are 13 days stale wherever the repo moved**, which since 2026-07-03 is: the deck
rework (F), the AI-native batch (G), the blog/site batch (A), publish hardening (B), and the a11y
batch (C). Staleness is **not** evenly spread: it clusters exactly where work landed.

### Measured staleness, wave 1

| Area | Entries | Outright STALE/SUPERSEDED | Contain >=1 false statement about today's source |
|---|---|---|---|
| Slides | 9 | 5 | 6 |
| Cross-references and numbering | 9 | 3 | 5 |
| Citations and bibliography | 9 | 2 | 5 |
| Project and configuration model | 7 | 2 | 4 |
| **Total** | **34** | **12 (35%)** | **20 (59%)** |

In citations the staleness concentrates in the **skeptic's** revisions, the layer that gives the
catalog its authority: three of them are wrong against current source (D73's "the duplicate-key
warning is unlocated" is itself stale, D71's "the enhancer is mouseover only" is stale, D70's CFF
rationale is dead).

The rot is not random. In slides, **every** entry framed as "Taliesin already better" or "add a knob"
was overtaken, because the 2026-07-12 ruling was systematically *remove and default*, the opposite
axis from the one the catalog reasoned on.

## Verdicts

Vocabulary: **STALE** (the "Taliesin today" claim is factually wrong now) · **SUPERSEDED** (a later
owner ruling kills or inverts the premise) · **KEEP-AS-WIN** (already better, still true) ·
**ADOPT** (right for Taliesin, becomes a backlog item) · **DECLINE** · **OWNER-RULING**.

### Cross-references and numbering (9/9)

ADOPT 5 · STALE 3 · DECLINE 1

- **D49 chapter-scoped float numbering: ADOPT.** Real and unfixed: `fig_count`/`eq_count`/`lst_count`
  are flat per-page counters (`render/mod.rs:258-261`); `number_theorems` (`mod.rs:1583`) is the only
  numbering fn taking a `chapter`. Two chapters each get a "Figure 1". Auto-scoping in a numbered book
  is a better default, not a knob. First step: thread `chapter` into the float emit path mirroring
  `number_theorems`. Pin: extend `corpus/demo-book`. **Sub-fork for the owner:** floats would become
  auto-scoped while theorems stay opt-in (`number-within`), yielding "Figure 2.3" beside "Theorem 5"
  in one book.
- **D50 `@sec-` hierarchical numbers: STALE.** Shipped in `1132df3` (2026-07-07), four days after the
  catalog. `render/mod.rs:411-413` computes `site::section_number` at render time and registers *that*.
  *Live residual:* outside a chapter the flat counter still applies (`mod.rs:421-424`), so a single
  doc's `@sec-x` says "Section 3" while its heading shows no number. There is still **no
  `number-sections` key**, yet `site/mod.rs:989` documents one.
- **D51 configurable ref prefixes: DECLINE.** Minimal-config is dispositive; document config gets no
  a11y exemption. *Separate finding:* `lang:` is wired only to `<html lang>` (`render/page.rs:239`), so
  `lang: fr` silently promises French and delivers English labels. Lint it or document it.
- **D52 cross-page refs under warm preview: ADOPT (retargeted).** The proposed optimization targets
  work that never happens: `serve_site/mod.rs` re-discovers only on `_site.yml` change (:1116) or a
  structural add/remove (:1139), so a content edit leaves `xref_targets` **stale mid-session**. The
  real payload is a freshness fix, not an optimization.
- **D53 unresolved/duplicate refs: STALE.** The did-you-mean shipped (`c687fcc`,
  `cite/validate.rs:33-35`). *Live residual, the real item:* duplicate-label warnings are still
  **unlocated** (`render/mod.rs:1538` builds a `Warning` with no `.at(...)`; `site/xref.rs:56` pushes a
  bare `String`), so Taliesin half-reproduces the exact Quarto flaw D53 critiques.
- **D54 static code listing as a numbered float: ADOPT (now a correctness fix).** `code_lang`
  (`cell_extract.rs:170-185`) splits naively, so ```` ```{.python code-line-numbers="1|2-3"} ````
  routes to the **executable** path. That form is authored as display-only in `corpus/deck.tmd:46`,
  `docs/guide/using/formats.tmd:66`, and `docs/guide/demo.tmd:33`. The kernel-free corpus can never
  catch it. *Unverified:* traced by code path, not reproduced against a live kernel.
- **D55 custom xref types: STALE.** `XREF_LABELS` (`cite/render.rs:14-27`) is now the single source of
  truth and the parallel array is drift-guarded by `xref_anchor_recognizes_every_cite_prefix`
  (`xref.rs:317`). Custom kinds stay declined (minimal-config + strict validation).
- **D56 `#| label:` vs `{#id}`: ADOPT (narrow).** `table_wrap` (`exec.rs:1072`) drops the `#tbl-`
  anchor when output has no `<table>`; `exec.rs:379` skips empty output, orphaning a registered anchor.
  Do the two cheap guards, not output-kind sniffing. **Execution-gated, so it cannot ride the
  kernel-free corpus:** weaker net than a static check, flag per corpus-plus-roadmap.
- **D57 `-@fig-x` prefix suppression: ADOPT (floor version).** The `-` is **silently ignored** today
  (`cite/render.rs:350`). This project treats silent no-ops as defects (`a6cf810`, the `page-layout`
  value-lint), so the floor is "stop ignoring it": honor or warn. Sequence after D49/D50.

### Slides and presentations (9/9)

SUPERSEDED 3 · STALE 2 · DECLINE 2 · ADOPT 1 · KEEP-AS-WIN 1. **Two-thirds rot.**

- **D102 own native slide engine: KEEP-AS-WIN.** Zero reveal hits anywhere; `frontmatter.rs:296`
  actively flags `revealjs`. *Dead sub-clause:* its ask for "corpus coverage for slide print/PDF
  pagination" is void (see D105).
- **D103 spatial camera + overview map: STALE.** Four of its six named specifics are deleted
  (minimap, LOD, threads/filter, van Wijk). The camera, rows-as-topics, and pannable overview survive.
  **Live defect, the only one this area found:** `deck.js:1680` renders `key('↑ ↓', 'Vertical slides')`
  but `up()`/`down()` call `moveTopic` (`deck.js:664-666`), so ↑↓ jump topics and vertical slides step
  ←→. The legend survived the 43-bug audit because B7's drift sweep cleaned the `.tmd` docs and never
  read the in-product string.
- **D104 speaker view over postMessage: STALE.** Core win intact (`sameOrigin` `deck.js:861`), but
  three of its four asks already shipped (step-aware Next `0d850e7`; panes are DOM-clone snapshots, not
  iframes, so the skeptic's "3x load on file://" is dead; `sync()` staleness fixed `3e69d6f`).
- **D105 PDF via browser print: SUPERSEDED.** Celebrates a deleted feature. Zero hits for the whole
  print/PDF family; `deck.css:367-373` is an explicit tombstone. *Residual:* `deck.css:374`
  `@page { size: 960px 540px }` is still hardcoded, now only affecting the stray-Cmd-P fallback.
- **D106 deck front-matter config surface: DECLINE.** The owner already ruled this way in-code:
  `deck.js:1955` says `?qmd=feed`/`?qmd=present` are transient escape hatches with **"no config knob"**.
  Phase 1's aspect knob lost its reason when PDF died (its payoff was print letterboxing); on screen the
  camera scales a fixed 960x540 stage to any viewport.
- **D107 fragment effects (`.fade-out`, `.highlight`): ADOPT, scoped hard.** The **only live,
  un-adjudicated capability gap in the area**: fragment effects were never in the deck audit's scope.
  These are Markdown-reachable author capability with zero config, not engine chrome. **Decline** the
  `incremental:` global knob (needs a second `.nonincremental` knob to escape it; `::: {.incremental}`
  is already the right local default) and `data-fragment-index`. Pin: `corpus/deck.tmd`, stepped
  forward **and back**. Use a `--tali-*` token for `.highlight` or it fails on one theme.
- **D108 default deck to reader/scroll: SUPERSEDED, totally inverted.** `deck.js:2029` is the
  tombstone: "Reader/scroll mode and PDF-export mode were removed." Routing is by **aspect**
  (`deck.js:1961-1964`, `isPortrait()` at `:1358`), not width. *Subtlety:* a "Present" button did ship
  (`deck.js:1726`) but escapes the **feed**, not a reader. Same label, opposite topology; do not read it
  as partial vindication.
- **D109 configurable slide-level: DECLINE, stronger than the skeptic.** The one surviving use case
  **buys zero capability**: `deck.rs:247` already accretes headings deeper than level 2, so `slide-level=1`
  only lets an author spell the same structure `#`/`##`. It is a renaming preference that also collapses
  `gridRows` to a 1-D strip and kills ↑↓ in present mode.
- **D110 persist pen annotations: SUPERSEDED.** Pen was cut. `\bpen\b` = 2 hits, both noise (one is a
  `penalty` local in the QR encoder that shipped *after* the pen was removed). *Design lesson worth
  keeping:* `drawKey()` used a positional `i<h>-<v>` key, so any future per-slide reader-state
  persistence must gate on an author-assigned `{#id}`, not an index.

**The catalog has no entry for anything shipped after 2026-07-12**, including some of the strongest
"wider than existing tools" decisions in the project: the **mobile slide-feed** (aspect-routed, reuses
the identical slide DOM so block-ids and live `{js}` state survive), the **offline QR share-link**
(own ~180-line byte-mode encoder, no CDN, works on `file://`), **deep-links carrying live
`{{< input >}}` state**, feed notes-narration, and wake-lock.

### Citations and bibliography (9/9)

ADOPT 4 · STALE 2 · KEEP-AS-WIN 1 · OWNER-RULING 1 · DECLINE 1

The summary calls this "the thinnest area" and "the most leveraged". **"Thinnest" overstates it** (two
of its five supporting examples are dead), but **"most leveraged" is TRUE, for the opposite reason it
claims**: the leverage is three near-free correctness fixes, not a citation-style engine. Author-date
and CSL-JSON, the "invest here" headline, are the *least* urgent things in the area.

**Note: D67, D69, and D72 all edit `crates/core/src/cite/`, a Do-NOT-touch zone, and none is a purely
additive read-only accessor. All three need explicit owner sign-off.**

- **D72 bare `@key` citation form: ADOPT. This is a live production bug, verified end to end (see
  "The a-star bug" below).** Bare `@` routes only through `parse_xref` (`cite/render.rs:250-252`),
  whose prefix scan yields `russell`, not in `XREF_LABELS`, so it falls through to literal text.
  Fix: gate on bibliography membership (`keys()` at `cite/mod.rs:69` already exists, so no new
  accessor is needed) and thread it through the same `cite_key` closure so numbering stays unified.
  **Drop the entry's warn-on-unresolved-bare-`@` clause:** `is_cite_key_char` (`cite/mod.rs:111-113`)
  admits `/ . : +`, so a scanner would greedily eat `@media`, `@types/node`, and `@example.com`.
- **D67 hardcoded IEEE-numeric + silently ignored `csl:`: ADOPT (part 1 only).** Worse than the
  catalog says: `csl:` is advertised on **four** surfaces, not one (`frontmatter.rs:50` allowlist,
  `vocab.rs:75` editor completion described as "Citation Style Language file.",
  `tali-frontmatter.schema.json:18` validates any value, `includes.rs:181` even resolves and watches
  the `.csl`). The tool does not merely ignore the key, it *recommends* it. Ship the located
  "recognized but unsupported" diagnostic and drop the `vocab.rs:75` completion in the same change.
  **Do not just remove it from the allowlist:** `frontmatter.rs:34` is `"css"` and `:50` is `"csl"`,
  edit distance 1, so the did-you-mean would suggest `css`. Put the diagnostic in
  `diagnostics/bibliography.rs` to keep `cite/` untouched. *Owner fork (part 2, deferrable):* native
  author-date as one curated style key (it is what makes `-@key` meaningful) versus IEEE-only forever;
  either way `csl:` must be removed or repurposed in the same commit, never left as a second key for
  one concept.
- **D69 reference-list placement: ADOPT (part 1).** Proven by rendering: with an appendix after
  `# References`, document order is References heading, then appendix, then the headless list. The
  author asked for the list *here* and got an empty heading plus a headless list elsewhere. That is a
  correctness defect, not a preference. Fix: splice after the manual heading block instead of
  `blocks.push` (`cite/render.rs:138`); the diff matches by stable id so it is diff-safe. **Unpinned
  today:** no corpus doc has content after the heading. Decline part 2 (margin references); the
  skeptic correctly demolished the "reuse the sidenote float" premise (a citation is inline,
  `.sidenote` is a block-level authored div).
- **D74 footnotes: ADOPT (narrowed to the reverse-sync fix).** `render/mod.rs:670-681` pushes the
  `qmd-footnotes` block with `sourcepos: String::new()`, and rendering confirms each `<li>` carries no
  `data-sourcepos`: a hole in a **load-bearing invariant**, orthogonal to the margin question the
  drafter tied it to. Fix is outside the Do-NOT-touch zone (`render/mod.rs`, not `cite/`), and
  `client.js` already resolves via `.closest("[data-sourcepos], [data-block-id]")`, so no client
  change. *Owner fork:* `reference-location: margin` as a signature Tufte feature versus decline on
  minimal-config (it cannot be the default, since long notes overflow a 12rem column, so it is a knob).
- **D73 located citation diagnostics: KEEP-AS-WIN, and more true than recorded.** The **skeptic's
  central correction is itself stale**: it claims the duplicate-key warning is an unlocated
  `Vec<String>`, but `render/mod.rs:895` wraps every one through `locate()`, pointing at the
  front-matter `bibliography:` line by deliberate design (`mod.rs:861-867`). *Real gap:*
  `diagnostics/bibliography.rs:20` uses a bare `Warning::new` with no `.at()`.
- **D68 CSL-JSON + inline `references:`: STALE.** The headline bug is fixed: `render/mod.rs:874-878`
  warns on an unsupported suffix, located to the front-matter line. Proven: `check` on a probe emits
  "bibliography 'refs.json' ignored: only BibTeX ('.bib') is supported". Both the drafter's
  "skipped without a warning" and the skeptic's "triple-silent failure is a real bug" are now false.
  Inline `references:` should be a hard DECLINE on minimal-config (a ~50-field open CSL schema either
  bloats the allowlist or punches a hole in strict validation).
- **D71 citation hover preview: STALE.** The claim "no hover/tap preview" is wrong. A citation is
  exactly `<a href="#ref-key">` (`cite/render.rs:364`), which the generic `taliInitLinkPreview`
  (`code-enhance/12-link-preview.js:7`) already previews via `eligibleSame`. The proposed
  per-citation data-attribute payload is strictly worse than the lazy DOM resolution in place.
  *Real scrap:* `corpus/reader/hovercards.tmd` has no `[@cite]` and no `bibliography:`, so working
  behavior is unpinned. Touch/tap is genuinely absent (no `touchstart`/`pointerdown`).
- **D70 `citation:` block + "Cite this" card: OWNER-RULING.** Partially stale: the machine-readable
  half shipped (per-page `.citations.json` at `build.rs:1561-1580`, ScholarlyArticle JSON-LD at
  `meta.rs:144-150`), which kills the CITATION.cff limb twice over. *The fork:* ship a `citation: true`
  card reusing the IEEE formatter, accepting it renders **author-free for every current post**
  (**0 of 8** tech-blog posts set `author:`), versus decline because the machine-readable need is
  already met and no human has asked to cite the blog.
- **D75 `nocite` / `@*`: DECLINE.** Low impact by the catalog's own rating, no corpus doc demands it,
  and it costs a new front-matter key. `@*` additionally forces `Bibliography.entries`
  (`HashMap`, `cite/mod.rs:47`) to become insertion-ordered or any pin is flaky. The project already
  knows this hazard: `cite/mod.rs:88-89` breaks `nearest()` ties lexicographically to compensate.

### Project and configuration model (7/7)

KEEP-AS-WIN 3 · STALE 2 · ADOPT 1 · DECLINE 1

- **D33 keep the strict two-level config model: KEEP-AS-WIN.** No directory walk exists;
  `load_config` (`config/mod.rs:169`) reads exactly one `_site.yml`. *The entry's own revised-rec is
  stale:* it claims `card_image` inherits project-to-page, but that field now has zero readers, so
  `toc` is the **only** project-to-page inheritance, making the headline claim *more* true than when
  written.
- **D34 inheritable project defaults + "the dead `author:`": STALE. The catalog's flagship claim is
  refuted.** `author:` is **live**, with two consumers that landed 2026-07-11, eight days after the
  catalog: `site/meta.rs:137-143` (JSON-LD `Person`, emitted :164/:182, `718e289`/`f80bda1`) and
  `site/feed.rs:112-119` (Atom `<author><name>`, :141-146). Wiring or deleting it would break two
  shipped features. **The entry fingered the wrong field:** the genuinely dead one is
  `SiteConfig.card_image` (`image:`), zero readers, superseded by auto OG cards, and its own doc comment
  at `config/mod.rs:38-43` concedes it. The defaults half (B) has a real premise
  (`bibliography`/`csl`/`execute`/`theme` are absent from the 19-key `NATIVE_KEYS`) but fails
  minimal-config **today**: no corpus doc repeats them across pages. → **OWNER-RULING**, recommendation
  **subtract before adding**: delete the dead `image:` field, defer the defaults until a corpus doc hurts.
  *Latent bug either way:* both consumers call `.as_str()` on an `Option<serde_yaml::Value>`, so
  `author: [A, B]` silently falls back to `title:`. Nothing pins `config.author` in either file.
- **D35 flat `_site.yml` schema: KEEP-AS-WIN.** The skeptic's central downside is **fixed**: it claimed
  nav typos silently drop with no did-you-mean, but `validate_keys` (`config/mod.rs:269-293`) dispatches
  into per-shape validators, pinned by `nested_nav_footer_mount_typos_warn_instead_of_silently_dropping`
  (:544), landed `a6cf810` (2026-07-07). ("16 keys" is stale; it is 19.) *Residual:* `nav`/`footer`/
  `mounts`/`css`/`head`/`body-*` are typed `{}` in `tali-site.schema.json`, an editor-affordance gap only.
- **D36 infer project type from shape: KEEP-AS-WIN.** `is_book: !chapters.is_empty()`
  (`config/mod.rs:210`). Holds only while exactly two multi-page shapes exist; a third reopens it.
- **D37 lint `format:` sub-keys: ADOPT (revised mechanism only).** The hole is open and deliberate
  (`frontmatter.rs:17-18`; the test `format_subkeys_are_not_linted` at :610 pins it). The honored
  `format: deck:` key set is genuinely **empty** (`deck.rs:109` is hardcoded), so whitelisting
  `transition`/`incremental` would validate no-ops as supported. **Shipped precedent that did not exist
  on 2026-07-03:** the from-quarto value-lint (`69c228b`, `frontmatter.rs:311-328`) warns on a
  recognized key carrying an ignored value. **This adds a diagnostic, not a knob**, so minimal-config is
  served, not merely survived. Sequence **after** the deck rework settles.
- **D38 filesystem discovery with `draft:`-only exclusion: STALE.** Draft-aware preview shipped
  2026-07-16 (`DraftMode`, `site/mod.rs:37`, `discovery.rs:28`, build reports "N drafts not published"
  at `build.rs:1121`), and corpus pins now exist, so the keep no longer rests on an unprotected feature.
  *Genuinely open:* the orphan diagnostic. Nothing warns that a stray `.tmd` silently became a page.
- **D39 config profiles: DECLINE (and `--base-url` too, for now).** With `url:` unset, **nothing**
  absolute is emitted, so there is zero preview/prod divergence. *Both the drafter and the skeptic
  misplaced the gate:* it is `Site::canonical_base` (`site/feed.rs:13-19`), not `meta.rs`. Pinned by
  `no_jsonld_without_url`, `no_feeds_without_url`, `no_feed_index_without_url`.

## The a-star bug: a corpus doc is shipping broken, and every gate is green

**This is the most valuable thing the triage found, and it is worth more than the rest of §E combined.**
Verified end to end by hand (render + `check`), not inferred:

`corpus/tech-blog/posts/a-star/index.tmd` is a **real published post** in the author's blog. It:
- declares `bibliography: references.bib` (line 7),
- writes "please refer to @russell2022artificial." (line 32), a **bare** `@key`,
- and that key **exists**, at `references.bib:1` (`@book{russell2022artificial,`).
- It has **zero** bracketed `[@...]` cites, so this is its only citation.

Rendered output:
```
data-sourcepos="32:1-32:73">For a more thorough study of A*, please refer to @russell2022artificial.</p>
...
<h3 id="references" data-block-id="b-31dda0420bcb" data-sourcepos="679:1-679:13">References</h3>
</main>
```
The literal `@russell2022artificial.` ships as prose. `ref-russell2022artificial` occurs **0** times.
`<section class="tali-references">` occurs **0** times. `## References` is line 679 of a 679-line file,
so the post ends with an **empty References heading followed immediately by `</main>`**.

`taliesin check corpus/tech-blog/posts/a-star/index.tmd` prints **"no problems found"**, **exit 0**.
The corpus suite passes. Every gate is green.

**Root cause:** bare `@` routes only through `parse_xref` (`cite/render.rs:250-252`); its prefix scan
yields `russell`, which is not in `XREF_LABELS`, so it falls through to literal text.

**Why nothing caught it (the deeper bug):** a **declared-but-never-used bibliography emits no
diagnostic**. `diagnostics/bibliography.rs` only checks the inverse case (a references block with no
`bibliography:`). So the corpus regression net, the project's arbiter of done, **cannot see this class
of failure at all**.

**Two separable fixes, different risk:**
1. **The diagnostic** (recommended first; **outside** the Do-NOT-touch zone). Warn when a
   `bibliography:` is declared and zero citations resolve. Cheap, static, would have caught this, and
   it is the kind of "stop failing silently" lint the owner has repeatedly accepted (`a6cf810`, the
   `page-layout` value-lint). Lives in `diagnostics/bibliography.rs`.
2. **Bare `@key` support** (D72; **inside** `crates/core/src/cite/`, needs owner sign-off). Gate on
   bibliography membership via the existing `keys()` (`cite/mod.rs:69`); emit no warning for a
   non-matching bare `@word` or it will eat `@media` and `@example.com`.

Fix (1) makes the failure visible; fix (2) makes the post correct. They can ship independently.

## Live defects found (not in the catalog, worth filing regardless of E)

1. **The deck key sheet lies to every presenter.** `deck.js:1680` advertises "↑ ↓ Vertical slides";
   ↑↓ call `moveTopic`. Cheap fix, real user-facing wrongness, survived a 43-bug audit.
2. **Duplicate-label warnings are unlocated.** `render/mod.rs:1538` and `site/xref.rs:56` emit no
   file/line, so there is nothing to click. This is the exact Quarto flaw D53 critiques.
3. **`{.python code-line-numbers=...}` is executed.** Authored as display-only in `corpus/deck.tmd:46`
   and two docs pages; `code_lang` routes it to the executable path. Invisible to the kernel-free corpus.
4. **The xref registry goes stale on a warm content edit** (`serve_site/mod.rs:1148-1199` refreshes only
   the Cmd-K search fragment).
5. **`author: [A, B]` silently falls back to `title:`** (both consumers `.as_str()` an
   `Option<serde_yaml::Value>`).
6. **`lang: fr` promises French and delivers English** cross-ref labels (`render/page.rs:239`).
7. **`site/mod.rs:989` documents a `number-sections` key that does not exist.**

## What is left

131 entries across 14 areas, all unverified and all subject to the same 13-day staleness. Highest
expected rot, by where work landed: **Diagnostics** (D163 proposes severity + stable rule ids that
shipped 2026-07-13; D161's value-domain validation partly shipped as the from-quarto value-lint),
**Publishing** (D135 proposes sitemap/robots/feeds that all exist: `seo.rs:11`/`:35`, `feed.rs:70`,
`llms.rs:95`, plus autodiscovery at `meta.rs:105`), **Interactivity** (D140 is on the summary's "rule
first" list but is built), **Figures** (D58 is on that list), and **Front-matter**.

**D135 is the sharpest warning in the catalog about the catalog.** Its skeptic insisted: "DROP the
RSS/Atom half entirely: it was built and deliberately removed, it is a documented non-goal for a solo
author." Atom feeds shipped anyway, with autodiscovery. (RSS 2.0 does remain a non-goal, per
`docs/superpowers/specs/2026-07-11-auto-seo-artifacts-design.md:41`.) The adversarial layer, the thing
that gives the catalog its authority, was overruled by the owner's own later decision. Treat every
skeptic verdict as evidence, never as a ruling.
