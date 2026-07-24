# Skimmability audit: making long Taliesin books scannable

*2026-07-24. Method: 8 parallel web-research agents (reading science, structured-writing methodology, competitive docs-tooling sweep, e-reader/annotation prior art, typography, search/index/findability, accessibility, build-time derivation) plus 5 codebase-inventory agents (reader affordances, author tooling, book-scale navigation, rendered-HTML ground truth, notes archaeology). Their findings fed 8 ideation lenses; ~95 raw candidates were consolidated to 30; each survivor was then attacked by 3 independent adversarial verifiers (already-shipped, invariant, efficacy) with permission to kill it. 4 candidates were killed outright. A second adversarial pass re-verified the measurements and the prescribed mechanisms, and corrected several of them; those corrections are folded in below.*

**All "measured" numbers in this document were reproduced at commit `5c25d00` unless the line says otherwise, and each carries the command that produced it.** Claims from the research inputs carry their original confidence. One research lens was never run and is named as a gap: publishing and editorial practice for long-form books.

**Independently re-verified by the main session before this file was written** (this repo's notes rot, so the headline defects were not taken on the agents' word). At `5c25d00`, from a fresh `cargo build -p taliesin-server` and `taliesin build docs/guide --out /tmp/skim-verify-guide`:

1. **Spurious zero section numbers: confirmed.** `grep -rl 'tali-section-number">[0-9]\+\.0\.'` matches **17 of 22** built guide HTML files (the non-matches are `index`, `404` and the decks). Live strings include `1.0.6.1`, `12.0.1.8`, `10.0.7`. Root cause traced end to end: `render/mod.rs:847-857` demotes body headings when a title block is emitted, then `site/chapter.rs:35` computes the counter slot as `level - 2` from the already-demoted level, so `counters[0]` never advances and prints a literal `0`.
2. **Nested part groups are silently dropped: confirmed by code read.** `site/book.rs:84-86` calls `push_chapter_entry(root, c, ...)` inside the part's inner loop and discards the `false` return that means "this is not a chapter, the caller must handle it". The outer loop at `:70-72` checks that return; the inner loop does not.
3. **`BODY_CAP` truncation: confirmed and measured.** `site/search.rs:11` sets the cap, `:172-175` applies it. **32 of 180** entries in the built `search-index.js` sit at the cap (17.8%).
4. **Search dies below the TOC gate: confirmed by targeted repro**, not just by code read. A three-chapter throwaway book was built (`/tmp/skim-verify-mini`) with one chapter above `MIN_TOC_HEADINGS = 3` and two below. The above-gate page emits `id="TOC"` and references `search-index.js`; **both below-gate pages emit neither, while still rendering the `tali-search-btn` Cmd-K button**. The affordance is advertised and the index is absent. Chain: `site/mod.rs:236` -> `page_toc()` at `:965` -> `doc.toc` false -> `toc` empty -> `render/page.rs:483` skips the index and the search JS together.
5. **No section extents: confirmed.** `reference/cli.html` has 0 `<section>` against 5 `<h3>`; `using/code.html` has 0 against 2. Repo-wide, `<section>` is emitted only by `render/deck.rs` (slides) and the footnotes block at `render/mod.rs:905`.
6. **Deepest headings dimmer than body: confirmed.** `assets/css/base.css:334-336` sets both `h5` and `h6` to `color: var(--tali-muted)` while body text inherits `--tali-fg`. This compounds with defect 1: demotion pushes an author's `####` to `<h5>`, so deep reference sections get greyed out without the author writing anything unusual.

One agent claim was **not** reproduced and is flagged where it appears: an early inventory pass reported search wiring missing from every built page. That was a false negative (the index is lazy-loaded from `search-index.js` rather than inlined as `TALIESIN_SEARCH_INDEX`). The real failure is narrower and is item 4 above.

## The headline finding

**The problem is not that Taliesin lacks skimming features. It is that the heading layer, which is the only surface a skimmer actually reads, is broken, incomplete, or invisible in five separate ways, and each one is a small verified defect rather than a missing feature.**

The reading science converges hard on one mechanism. Duggan and Payne's eye-tracking (CHI 2011) shows skimming is *satisficing over structural patches*: a reader reads forward until the marginal information gain drops below a threshold, then jumps to the next section boundary. NN/g's eye-tracking corpus names the efficient version of this the "layer-cake" pattern, where fixations land almost entirely on headings and dip into body text only on a hit, and states plainly that the famous F-pattern is *the failure mode that appears when there are no cues worth landing on*. So the boundary layer is the product. Everything else, the TOC, the drawer, Cmd-K, hover cards, is machinery for surfacing that layer or for compensating when it is thin.

Taliesin's boundary layer is currently damaged in ways that are cheap to fix and expensive to leave. Section numbers render with a spurious zero segment (`4.0.1`) on **31 of the 32 numbered chapters** of the project's own two books, because heading demotion and the numbering counter disagree; the one exception is a chapter with no body headings at all. The two deepest heading levels are rendered at *lower contrast than the body text they head*, inverting the hierarchy exactly where reference documentation nests deepest. Whole-book Cmd-K search silently disappears on any chapter with fewer than three TOC entries, and the live preview hides that failure from the author. **A quarter of the sections in `docs/internals` are truncated by a 1500-character cap, taking roughly 15% of the book's prose out of the search index with no signal.** Text inside a non-active tab panel is invisible to Ctrl-F. None of these is a design question. All five are defects.

A sixth defect is worse than any of them and is not about skimming at all: **a nested `{part:, chapters:}` group inside another part's `chapters:` list is silently dropped along with every chapter under it, and `taliesin check` exits 0.** A 300-page book is the document most likely to want two-level structure, and it loses chapters with no diagnostic.

Above that, one structural gap dominates: **there is no whole-book outline below chapter granularity anywhere a reader can reach.** The drawer is a flat list of chapter titles. The landing Contents is a flat list of chapter titles, and only on the landing page. The in-page TOC covers one chapter. Cmd-K's empty state is, again, a flat list of chapter titles. Meanwhile 161 section-level records already sit in the built search index of `docs/guide`, reachable only by typing a query that happens to match. At 20 chapters that is annoying; at 300 pages it is the difference between a book and a pile of pages.

And one substrate gap blocks four separate proposals: **the emitted HTML has no section extents.** Zero `<section>` elements wrap content headings on 17 of 19 built guide pages; blocks are flat siblings. Nothing in the DOM says where a section begins or ends, even though `lsp_outline.rs` already computes per-section `end_line` in Rust today.

The second-order finding, and the one worth arguing about: the meta-analytic evidence says the leverage is on the *builder* side, not the reader side. Instructor-provided highlighting improves both memory and comprehension (d about 0.44) while learner-generated highlighting improves memory only (d 0.36) and not comprehension (0.20, n.s.). Instructor-provided graphic organizers are the largest-effect text aid measured (g+ about 0.53 comprehension, 0.70 memory). That is a direct argument for what the Rust builder emits by default, and against a reader annotation layer, which this repo already built and deleted once.

The third-order finding, which the tool cannot fix: **roughly half the problem is content, not code.** Zero of 37 dogfood pages set `description:`. Eight `tali-xref` links exist across the entire 19-chapter guide. Zero backlink lines render in either book. `docs/internals` contains 60,208 words and zero `{.definition}` blocks. Several of the strongest recommendations below (glossary, term index, backlinks with context, landing blurbs) are machinery waiting on an authorial pass that no code change can perform. The "Author playbook" section states that pass explicitly.

## What the research says

Organised as claims with evidence and confidence. Anti-patterns are at the end and are as important as the positives.

### Structure is the whole game

**Skimming is satisficing over structural boundaries; the section boundary is the unit of the skip.** *(high)* Duggan and Payne, CHI 2011, eye-tracking under time pressure: readers read until information-gain rate falls below a threshold, then jump to the next section. Their 2009 JEP:Applied work found skimming preserves memory for *important* ideas but not details or inferences. Implication: boundary density is the primary quality metric of a page, and whatever the document *marks* as important is what a skimmer retains, which makes marking the highest-leverage authorial act.

**The layer-cake pattern is the efficient scan; the F-pattern is a symptom of missing cues.** *(high)* NN/g's multi-pattern eye-tracking work calls layer-cake "by far the most effective way in which users can scan pages" and calls the F-pattern "the default pattern when there are no strong cues to attract the eyes towards meaningful information". So "design for the F" is folklore, and any page where the F emerges is a formatting failure.

**Formatting for scanning is a large, measured usability effect.** *(high for direction, medium for magnitude)* Nielsen and Morkes 1997: scannable layout alone +47% measured usability, concise text +58%, all three traits combined +124%. Small 1997 lab study, single site; the *direction* has been replicated by later eye-tracking, the magnitudes are indicative. Their operationalisation is what matters and is directly lintable: highlighted keywords, meaningful sub-headings, bulleted lists, one idea per paragraph, inverted-pyramid order.

**Author-provided signalling beats reader-generated.** *(high)* Ponce, Mayer and Mendez 2022, 36 articles / 85 effect sizes: instructor-provided highlighting improves memory *and* comprehension (both d about 0.44); learner-generated improves memory (0.36) but not comprehension (0.20, n.s.). Separately, instructor-provided graphic organizers reach g+ 0.70 memory / 0.53 comprehension. This is the strongest single argument for putting effort in the builder, and (see the editorial-architecture subsection) the strongest evidence base for a chapter-opening summary block.

**Signalling has a real but modest effect and depends on distinctiveness.** *(high)* Richter, Scheiter and Eitel 2016 meta-analysis, 27 studies, N=2464: r = 0.17, moderated by signal distinctiveness and by reader prior knowledge (helps low-prior-knowledge readers most). Consequence: emphasis is a zero-sum attention budget. Over-signalling cancels itself.

**Headings measurably improve recall, search and retrieval; heading FORM does not matter.** *(medium)* Hartley and Trueman 1983; Sanchez, Lorch and Lorch 2001. Readers with headings before every subsection remembered topics and their organisation better. But question-form vs statement-form headings showed no differential effect for typical readers. So lint for *presence and distinctiveness*, never for grammatical form.

**Front-loading is the rule that makes the layer cake work.** *(medium)* NN/g's "First 2 Words" and "Headings Are Pick-Up Lines": in a layer-cake or F scan the reader consumes only the leading words of a heading or link. Practitioner eye-tracking, not a controlled experiment, but consistent across their corpus and consistent with the rotor-sort behaviour of screen readers.

**Working-memory capacity is about 4 chunks, not 7.** *(high)* Cowan 2001. Caveat that must travel with it: this governs items held *without external support*. A persistently visible, scannable list is the canonical case where the limit does not bind (this is the same error as "Miller's 7+/-2 caps menu length", which NN/g rejects). Applied correctly it argues for breadcrumb depth and preview-sentence count, not for capping how many entries a visible TOC may show.

### Book editorial architecture

**This lens was not run as a research pass and is a known gap in the method.** Eight lenses covered reading science, structured writing, docs tooling, e-readers, typography, search, accessibility and build-time derivation. None covered how 300-page technical books are actually made scannable by their publishers: front matter and back matter as structural page kinds (preface, "how to use this book", appendices, colophon), chapter openers with objectives or a summary, end-of-chapter recaps, running heads, part title pages, and the O'Reilly / Manning / Pragmatic house conventions. What follows is the defensible fragment that the run lenses do support; treat the rest as unexamined.

**Instructor-provided advance organizers are the strongest evidence for a chapter-opening summary.** *(high)* The graphic-organizer meta-analysis cited above (g+ 0.53 comprehension, 0.70 memory, instructor-provided) is precisely a measurement of "a structured overview supplied before the text". A chapter opener stating what the chapter establishes is that intervention in its cheapest form, and it is author-written so it carries no generation hazard. This is a materially stronger warrant for `summary-div` than the DITA `<shortdesc>` precedent and the Hartley structured-abstract work that the rest of that proposal leans on, and it should be cited there.

**Front/back matter are structural page kinds, not just more chapters.** *(low, practitioner consensus only)* Every trade technical-book house distinguishes preface, "how to use this book", appendices and colophon from numbered chapters. Taliesin's `chapters:` list has exactly one kind: `.unnumbered` on an `# H1` opts a page out of the count and that is the whole vocabulary. The honest position is that this is a real modelling gap at 300 pages and that nothing in this audit measured its cost, so no recommendation here proposes a `front-matter:` key. `grow-tarn` (below) should include at least one unnumbered appendix so the behaviour is pinned before anyone designs for it.

**Structured abstracts are more informative but longer, and the retrieval-speed advantage did not replicate.** *(high)* Hartley, J Med Libr Assoc 2004: 21% (simplified) to 35% (independently published) longer; a 22-item checklist scored traditional 6.4 vs structured 9.1; but "studies with the more complicated 'real-life' abstracts presented in MEDLINE have not shown an advantage for search speed". Ship the honest claim ("more findable facts"), never "faster lookup", and never make it mandatory.

### Readers, screens and attention

**Screen reading is measurably worse than paper for expository text, and readers are overconfident about it.** *(high)* Delgado et al. 2018 (54 studies, g = -0.21) and Clinton 2019 (g = -0.25 overall, -0.32 expository, -0.04 n.s. narrative; calibration g = +0.20 favouring paper). The gap grows under time pressure and is genre-specific to informational text, i.e. exactly Taliesin's genre. The mechanism usually offered is weakened spatial encoding, which argues for the renderer supplying externally the structure the reader is failing to build internally. Honest limit: neither meta-analysis tests an intervention, so this establishes a problem, not a fix.

**Dwell time has strong negative aging: the first ~10 seconds decide.** *(high)* Liu, White and Dumais, SIGIR 2010, 205,873 pages and >2bn dwell times, Weibull with negative aging on 99% of pages. The screening phase is itself a skim, so the first viewport should be optimised for scannability rather than for the best opening sentence. Corollary that cuts against several proposals: tall hero blocks and stacked sticky chrome spend exactly that budget.

**Readers read at most 20-28% of the words on a page.** *(medium)* Weinreich et al. 2008 (25 users, instrumented browsers) with Nielsen's extrapolation. Small N, 2008, and the 20% is derived not measured. Useful as a budgeting frame: decide *which* 20% is seen, and it skews toward headings, ledes and captions.

**Orienteering dominates teleporting.** *(high)* Teevan, Alvarado, Ackerman and Karger, CHI 2004: 61% of directed search is orienteering (small contextual steps from a known start), 39% teleporting, because small steps are less cognitively demanding and provide context for interpreting results. Their title is literally "The Perfect Search Engine Is Not Enough". Implication: a book that invests everything in Cmd-K has under-served the majority behaviour. Note the same finding cuts *against* adding one more teleport destination.

**Information foraging: navigation is driven by proximal cue scent.** *(high)* Pirolli and Card 1999; the SNIF-ACT models predict navigation from semantic overlap between cue words and the reader's goal vocabulary. Every navigational surface Taliesin emits (TOC entry, drawer row, search snippet, backlink, prev/next) is a scent cue and should be audited as one. A bare page title is a weak cue; the citing sentence is a strong one.

### Platform mechanisms worth using

**`hidden="until-found"` makes collapsed content findable by Ctrl-F and by text fragments.** *(high, with a caveat)* MDN and Chrome for Developers: implemented as `content-visibility: hidden`, so the element still participates in layout and is reachable by find-in-page, firing `beforematch` before reveal. Native `<details>` gets the same auto-expand for free. Support: Chrome 102+, Firefox 139+, Safari 26.2. **Verified caveat from the adversarial pass**: MDN browser-compat-data flags both Firefox 139-148 and Safari 26.2 as `partial_implementation` with the note "the browser does not correctly scroll to the matching text", and in Safari older than 26.2 the value is an invalid enumerated value that falls back to plain `hidden` (i.e. `display:none`). So it is a genuine improvement for Chrome and a graceful no-op elsewhere, but it is *not* a cross-browser guarantee and must not be used as the sole safety argument for hiding content. Second caveat, load-bearing for the tabset fix: because the element still generates a box, a collapsed panel occupies layout unless it is explicitly given zero intrinsic size.

**`content-visibility: auto` is the one performance lever that keeps content in the a11y tree and in find-in-page.** *(high)* The CSS Contain spec is explicit that for `auto`, skipped content "must still be available as normal to user-agent features such as find-in-page, tab order navigation, etc." (unlike `content-visibility: hidden`, which removes the subtree from the accessibility tree). It requires `contain-intrinsic-size` or scroll position jumps. Reported caveat in the wild: Safari's find-in-page does not always locate text inside `auto` subtrees. Repo-wide grep for `content-visibility` in the bundled CSS returns **zero** matches, so this lever is entirely unused; see the ruling under `section-extents`.

**URL text fragments (`#:~:text=`) deep-link to an exact sentence with no ids.** *(high)* Chrome 80+, Edge 83+, Safari 16.1+, Firefox 131+. Syntax `#:~:text=[prefix-,]start[,end][,-suffix]`, styleable via `::target-text`, feature-detectable via `document.fragmentDirective`, silently no-ops on a miss. Restrictions: user-initiated navigation, main frame only, each of prefix/start/end/suffix must lie wholly within one block-level element.

**Chunked search indexes make index size independent of corpus size.** *(high)* Pagefind: "Rather than build one large search index, Pagefind splits the search space into ordered chunks", serving a 10,000-page site under 300 kB total network payload. Material for MkDocs independently measured a 42-47% index shrink from indexing sections rather than pages twice, and the same ~1.5 bytes-gzipped-per-source-word constant this repo exhibits.

**Section-scoped search results with per-heading sub-results are the converged docs-search shape.** *(high)* Pagefind returns `sub_results` scoped to individual headings, each with title, anchored url, and a `<mark>`-highlighted excerpt. Algolia DocSearch does the same via lvl0-lvl5 records with `attributeForDistinct: url`. Material for MkDocs groups sections under a parent page row and surfaces missing query terms with a struck-through "Missing: X" line, added because silently dropped terms were unrecoverable for users.

**Query-biased snippets beat leading-text snippets, and length should follow query type.** *(high)* Tombros and Sanderson 1998 (n=20) and White et al. 2003 replicate the precision/recall advantage; Cutrell and Guan 2007 (n=22) found paragraph-length snippets help informational tasks but *hurt* navigational ones; Clarke et al. 2007 (10,000 summary pairs by clickthrough inversion) found summaries over 100 characters significantly raise clicks.

### Structured writing, condensed

**DITA `<shortdesc>` is the one-source-many-consumers design worth copying.** *(high)* The OASIS spec: a <=50-word purpose statement rendered as the topic's first paragraph *and* reused as link preview *and* as the search-result snippet. Taliesin already has the key (`description:`) and already consumes it in four places; it just does not derive one when absent.

**rustdoc supplies the correct failure mode for derived summaries.** *(high)* The rustdoc book: the first sentence is used as the index summary, "there is an arbitrary limit on the length of a summary", and "if the first sentence is too long then rustdoc will not create a summary". Silence beats a truncation. Important caveat surfaced by verification: rustdoc's rule works partly because Rust's API guidelines *instruct* authors to write a standalone summary line. Prose sections have no such convention.

**The LEAD baseline is hard to beat.** *(high)* See et al. 2017: lead-3 scores ROUGE-1 40.34 on CNN/DM against 39.53 for their best pointer-generator. Extractive graph methods sit well below both. So a deterministic first-sentence extraction is not a poor man's summariser, it is close to the ceiling for the genre, and it has the decisive advantage of being the author's own words.

**A real back-of-book index is not a concordance.** *(high)* Wu, Li, Mitra and Giles, CIKM 2013 (213 books, 22.3M words): "A good index is more than just an alphabetical list of all proper nouns (which is properly called a concordance)". Their example: "hidden Markov model" appears on 20+ pages of Bishop's PRML while the real index entry lists 2 locators. Measured index-to-book ratio: 0.42% of word count. Unsupervised keyphrase extraction tops out near F1@10 = 0.30 (YAKE 0.309, plain TF-IDF sometimes higher at 0.352).

**Kindle X-Ray proves the offline precomputed term index.** *(medium)* Preloaded into a small sidecar file on the device, People/Terms with mention counts and jump-to-mention, explicitly no internet needed. Product behaviour, not a study. Note Amazon's own pipeline uses ML *plus manual review plus author override*.

**Line length is preference, not speed.** *(medium)* Dyson and Haselgrove 2001: 55 cpl rated easiest and comprehended best, but longer lines are often read *faster*; roughly 100 cpl optimises raw speed while readers prefer 45-72. A real speed/preference dissociation. Consequence: defend the measure on comfort, do not treat 66 characters as a discovered constant, and note that this dissociation argues for a good default (readers pick the width they read slower at).

**Letter spacing is the one typographic intervention with real dyslexia evidence.** *(medium)* Zorzi et al., PNAS 2012, 54 Italian + 40 French dyslexic children: +2.5pt letter spacing gave about 20% faster reading with roughly half the errors, no training. Published methodological objections exist (Skottun and Skoyles letters) which the authors rebutted.

### Anti-patterns and debunked items

**Bionic Reading and all word-level fixation bolding: measurably negative.** *(high)* Readwise, paired counterbalanced, 1,916 analysed participants: Bionic 325.3 WPM vs plain 327.9 WPM (2.6 WPM *slower*, t = -1.92, p = 0.055), comprehension identical at 88% in both arms. Three independent studies now: Snell 2024 (Acta Psychologica, "No, Bionic Reading does not work", null reading times, no eye tracking); Beelders 2025 (J. Eye Movement Research: no difference in fixation durations, fixation counts or reading speed, and fixations spread across the whole word, so readers cannot auto-complete from the bolded prefix); Spear et al. 2025 (Attention, Perception & Psychophysics: *costs* relative to unbolded, and no benefit for any individual-difference subgroup, which closes the "but it helps dyslexic/ADHD readers" escape hatch).

**RSVP / speed-reading modes.** *(high)* Rayner et al. 2016 (Psych Science in the Public Interest): at normal speed comprehension matches static text; overclocked, comprehension and memory suffer. A marketing feature, not a comprehension feature.

**"Dyslexia-friendly" fonts.** *(high)* Wery and Diliberto 2016, single-subject alternating treatment, 12 students with confirmed dyslexia: OpenDyslexic produced *negative* improvement-rate differences on fluency (-88.65% to -49.65%) and accuracy (-73.53% to -63.62%). A 2018 study found the same for Dyslexie.

**Readability grades (Flesch-Kincaid, Gunning Fog, SMOG).** *(high)* W3C amended its own SC 3.1.5 Understanding doc after criticism (w3c/wcag#4022): the formulas count only syllables-per-word and words-per-sentence, ignore vocabulary familiarity, different implementations disagree on identical text (Zhou, Jeong and Green 2017), and revising text to improve the score has been shown to *decrease* comprehension (Duffy and Kabance; Olsen and Johnson). Meaningless over code identifiers and KaTeX.

**Reading-time estimates.** *(low, i.e. the supporting evidence is bad)* The ubiquitous "+40% engagement" traces to an unpublished Simpleview marketing claim reported in trade press. Medium's famous 7-minute result (Mike Sall, Data Lab 2013) measured which post length accumulates the most total attention, and says nothing about the effect of *displaying* an estimate. No controlled study surfaced. The 265 wpm constant is badly wrong for prose containing code and math.

**Accordions and collapse-by-default for content readers need.** *(high)* NN/g: collapsing "compromises the discoverability of the collapsed content", valuable content "may be missed altogether", and on desktop "it is easier to scroll down the page than to decide which heading to click on". Their explicit avoid-list includes complex multi-level content and uninterrupted reading flow, i.e. a technical book.

**Fuzzy text-quote anchoring for durable reader state.** *(high)* Aturban et al., TPDL 2015: about 27% of Hypothesis text annotations orphaned against live pages, only ~3.5% recoverable from web archives, 61% of survivors at risk. Hypothesis needs three selectors and four fallback strategies with a Bitap matcher, which has been slow enough to block page execution in production. A content-hash block id owned by the renderer is strictly better *and* fails honestly.

**Auto-extracted term lists presented as an index.** *(high)* See Wu et al. above. F1@10 about 0.30 means roughly two in three suggested terms are wrong: acceptable for a dismissible editor hint, unacceptable as rendered output.

**Also ruled out by the research:** designing an "F-shaped layout" (optimising for the symptom); a reader-side highlighter (comprehension null); read-time or build-time generated summaries (determinism plus "a wrong abstract is worse than none"); infinite scroll or virtualised long documents (breaks find-in-page, text fragments, and the scrollbar as a length cue); multi-column or justified body text; "Was this page helpful?" (needs a backend, and the binary format is low-signal anyway).

## Current state: what a reader of a long Taliesin book actually has

All file:line references verified against the working tree at `5c25d00`. All numbers measured against a fresh build of `docs/guide` (19 pages, 32,600 source words) and `docs/internals` (15 pages, 30,104 words) unless stated. Reproduction command for every build-derived number below:

```sh
target/release/taliesin build docs/guide     --out /tmp/skimrev
target/release/taliesin build docs/internals --out /tmp/skimint
```

### What ships and works

| Surface | Where | Scope |
| --- | --- | --- |
| In-page TOC | `render/mod.rs:2180` `toc_html`, gated `site/mod.rs:958` `page_toc` | One page, three heading levels relative to the shallowest |
| TOC scrollspy + read ticks | `web-client/toc-spy.js:80-153`, read set at `:24` | One page, `localStorage` keyed by pathname |
| Mobile pull-up TOC sheet | `web-client/toc-sheet.js`, markup `render/page.rs:337-340` | One page, static builds |
| Book chapter drawer | `site/chrome.rs:224-327` `sidebar_html` | Flat chapter titles only |
| Book landing Contents | `site/book_toc.rs:22-104` | Chapter titles + `description:` blurb, landing page only |
| Prev/next + arrow keys | `site/chrome.rs:330-372`, `code-enhance/07-keyboard.js:77-82` | Linear, chapter granularity |
| Cmd-K cross-page search | index `site/search.rs:61-112`, client `web-client/search.js` | Per-section records, lazily loaded |
| Hover preview cards | `site/hover.rs`, `code-enhance/12-link-preview.js` | Figures/theorems/tables/equations only |
| Backlinks ("Referenced by") | `site/backlinks.rs:14-126` | Cross-page, explicit `@`-refs only |
| Reading progress + resume | `code-enhance/15-reading-progress.js`, key `:43` | One page, block-id anchored |
| Focus mode, theme, shortcuts | `03-focus-mode.js`, `14-reader-prefs.js`, `07-keyboard.js` | Reader-local |
| Skip link + `<main>` landmark | `render/page.rs:173-177, 516-526` | Server-rendered, works JS-off |
| Cross-references + numbering | `site/xref.rs`, `site/chapter.rs` | Project-wide registry |

The keyboard shortcut set is exactly five keys (`?`, `/`, `f`/`F`, arrows, Esc), and it already ships a WCAG 2.1.4 on/off switch (`07-keyboard.js:33-46`), which is what makes adding single-key bindings legitimate.

### Measured numbers

Each line names its command. Anything not reproducible from a command is labelled as such.

- **Build.** `docs/guide`: 19 pages + 2 decks + 3 assets in **599 ms**. `docs/internals`: 15 pages in 470 ms. (`taliesin build …`, stderr summary line.)
- **Search index, guide.** `search-index.js` = **173,306 B raw / 59,014 B gzipped**, **180 entries**. (`ls -l /tmp/skimrev/search-index.js; gzip -c … | wc -c`.) Density is about 1.8 bytes gzipped per source word, the same order as the constant Material for MkDocs measured on a King James Bible corpus. A 150k-word book projects to roughly 800 KB raw / 270 KB gzipped.
- **Search truncation.** `BODY_CAP = 1500` at `site/search.rs:11`. **33 of 180** guide entries (**18.3%**) and **40 of 158** internals entries (**25.3%**) sit at the cap. (Python pass over the built index counting `len(entry['b']) >= 1490`.) Separately, an inventory pass that diffed indexed text against section text extracted from the built HTML measured **84% (guide) / 85% (internals)** of section prose actually indexed; that method is not reproducible from a one-line command and is quoted as an inventory measurement, not re-derived here. Verified misses: "viewport meta tag" and "broken placeholder" both appear in the built `reference/cli.html` and return zero hits in the index.
- **Outline-sidecar ratio.** Stripping the `b` (body) field from the guide index leaves 21,723 B raw / 4,569 B gzipped against 172,201 B raw / 59,029 B gzipped for the same array: **the body field is 87% of raw bytes and 92% of gzipped bytes**. Use the ratio, not the absolute byte counts, when justifying a separate outline sidecar; the ratio is stable across builds, the byte counts are not.
- **TOC entries per built guide page:** 0, 4, 5, 5, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 12, 13, 14, 16, 17. Exactly one page (`reference/shell-completion.html`, 0 body headings) falls below `MIN_TOC_HEADINGS = 3`, and it consequently ships **no search wiring at all** while still rendering a Search button.
- **Section numbers.** 18 built guide pages and 14 built internals pages carry `tali-section-number` spans. **31 of those 32 emit a zero segment** (17 of 18 guide, 14 of 14 internals). The exception is `reference/shell-completion.html`, which has no body headings and so emits only the bare chapter number `17`. (`grep -lE 'tali-section-number">[0-9]+\.0' *.html */*.html`.) Concrete strings today: `using/reading.html` reads `6`, `6.0.1`, `6.0.2`; `reference/cli.html` reads `16`, `16.0.0.1`, `16.0.0.2`, `16.0.0.3`, `16.0.1`.
- **Heading density:** `reference/cli.tmd` is 4,077 words behind 9 headings; `docs/internals/architecture.tmd` is 1,891 words behind 3 (about 630 words per heading). The longest true prose run (tables, lists, code, figures as breakers) across all 36 dogfood pages is 429 words in `using/reading.tmd`; the headline "1,832-word run" in `cli.tmd` is 1,021 words of *table cells* plus code.
- **Page weight:** a typical built chapter is 60-68 KB gzipped, about 81% inline script and style. `index.html` is 810 KB gzipped and `using/writing.html` is 1.12 MB gzipped, both dominated by 2.57 MB of inlined mermaid. The entire search index (59 KB gz, fetched once, on demand) is a small fraction of what every navigation already costs.
- **Reading time:** `render/mod.rs:915` gates the estimate on `is_article` (a non-empty `date:`). Zero of 36 dogfood pages set one, so `grep -c tali-read-time` returns 0 on every built page of both books.
- **`description:`** zero of 37 dogfood pages set it in their own front matter (the six grep hits are all inside fenced examples). The landing Contents therefore renders 18 bare titles and **0** `tali-btoc-desc` blurbs, and no chapter page has a `<meta name="description">`.
- **Semantics:** **zero** `<section>` elements wrap content headings on 17 of 19 built guide pages. Blocks are flat siblings. `using/code.html` has 47 `data-block-id` blocks, all siblings.
- **Deep headings:** exactly **4** `#### ` headings and **0** `##### ` headings exist across all of `docs/` and `corpus/`. (`grep -rn '^#### ' docs corpus --include='*.tmd' | wc -l`.)
- **Cross-reference density:** 8 `class="tali-xref"` links across the entire 19-chapter guide; 6 distinct `{#sec-}` labels across both books; **0** `tali-backrefs` lines in either book. `docs/internals` (60,208 words) contains zero `.definition`/`.theorem` divs.
- **Corpus scale:** the largest corpus book is `corpus/tarn` at 6 chapters / 1,135 words, with two parts (`Guide`, `Reference`). `corpus/course` has 6 listed chapters of which `problems.tmd` is `draft: true`, so only 5 publish; total 1,058 words. `corpus/demo-book` is 288 words. Nothing in the regression net is within two orders of magnitude of a 300-page book.
- **`content-visibility`** appears **zero** times in the bundled CSS. (`grep -rn content-visibility crates/core/assets/`.)
- **Preview vs built asymmetry (search):** the live preview injects `TALIESIN_SEARCH_URL` unconditionally (`serve_site/mod.rs:686-690`) while the build injects it only inside the TOC-gated block (`render/page.rs:483-497`), so the author never sees the search failure.
- **Preview vs built asymmetry (TOC), corrected:** the earlier draft diagnosed this as absolute-vs-relative levels. That is **false**. `client.js:850` computes `const base = Math.min(...heads.map(lvl))`, so nesting is relative in both. The real divergence is the **selector**: `client.js:847` queries `h1[id], h2[id], h3[id]` by tag, while the build's `render::toc_items` filters `level - base <= 2` over *all* levels. On a title-demoted page (rendered h3/h4/h5) the preview TOC lists only the h3 rows and silently drops the h4/h5 rows the built TOC includes. An engineer following the old diagnosis would go "fix" a `base` that is already correct.

### What changes at 150k words

Almost every measurement above is taken at 32.6k words. Several surfaces change *character*, not just size, at book scale. This subsection projects each and names which ones change in kind.

- **Chapter drawer: changes in kind.** It is a flat `<ul>` inside a `min(20rem, 86vw)` panel with `overflow-y: auto`, no filter, no type-ahead, no disclosure. At 19 rows it is fine. At 60+ rows it is a scroll list whose only orientation cue is the reader's own scrollbar position. Either the drawer gains type-ahead or `book-outline-artifact` Ship B must subsume it. See the `drawer-typeahead` recommendation.
- **Search index: changes in degree, then in kind.** 59 KB gzipped at 32.6k words projects to ~270 KB gzipped at 150k. That is the measured trigger for `unbounded-index` Stage 2 and nothing else.
- **Preview warm-page pool: changes in degree only, and is not to be touched.** `MAX_WARM_PAGES` plus the deterministic LRU order in `serve_site/exec_pool.rs` is the one standing freeze. At 60+ chapters a reader navigating the preview will evict constantly. **State the consequence plainly: a slower cold chapter, not a correctness bug.** Nothing in this audit proposes touching it, and no recommendation depends on its behaviour.
- **Whole-book zip: linear.** 2.98 MB at 19 pages, dominated by per-page inlined assets. At 300 pages this is tens of MB and the offline download becomes a different product decision. Not addressed here.
- **Build time and freeze cache: linear and unmeasured at scale.** 599 ms for 19 pages. Nothing in this audit measured a 300-page build; treat any claim about it as unverified.
- **Section numbering, backlinks, xref registry: linear.** These are per-anchor and the dogfood books barely exercise them.

## The gaps

Ranked by severity, all verified.

1. **Section numbers carry a spurious zero on 31 of 32 numbered dogfood chapters.** `render/mod.rs:851-858` demotes every body heading one level when a visible title block is emitted; `site/chapter.rs:35` computes the counter slot as `level - 2` from the *demoted* level, so `counters[0]` is never filled and a literal `0` is printed. Guide chapters read `4.0.1`; `reference/cli.html` reads `16.0.0.1`. Worse, the two *other* numbering sites (`render/mod.rs:533` and `site/xref.rs:87`) read pre-demotion source levels, so a reader clicking a link that reads "Section 6.1.1" lands on a heading labelled "6.0.1.1". The existing corpus pin (`corpus.rs:1046`, `corpus/demo-book/methods.tmd`) passes vacuously because that chapter uses `# Methods` with no front-matter `title:`, so demotion never fires. The in-code comment asserting "books never satisfy it" is false.

2. **A nested part silently deletes its chapters, and `check` exits 0.** `site/book.rs::build_book` handles the `{part:, chapters:}` shape only in the OUTER loop; the inner loop calls `push_chapter_entry(root, c, …)` and **discards its `false` return**, which is exactly the "this is a part, caller must handle it" signal. A two-level `chapters:` tree therefore drops the nested part header and every chapter under it, with no build warning and no diagnostic. Compounding it, `_site.yml` warnings carry no line number at all (`check.rs` wraps them as `Diagnostic::new("_site.yml", None, …)`), so `chapters:`, the one place a book's structure is authored, has no click-to-source.

3. **No whole-book outline below chapter granularity, on any reader surface.** Drawer, landing Contents, and Cmd-K's empty state are three renderings of the same flat chapter list; the in-page TOC is one chapter. The 161 section records in the index are query-only. A reader in chapter 4 cannot see or jump to a named section of chapter 12 without loading chapter 12.

4. **The emitted HTML has no section extents.** Zero `<section>` elements wrap content headings on 17 of 19 built guide pages; `using/code.html` has 47 flat sibling blocks. This blocks at least four proposals (reading-density fold, sticky current-section heading, section-scoped read/change state, per-section length in the DOM) and it is not a small thing to add: a wrapper changes the parent/child shape the incremental diff mounts. `lsp_outline.rs:16/154/182` already computes per-section `end_line` in Rust, so the *data* exists and is simply never emitted.

5. **A fifth of the prose is unsearchable, silently.** `BODY_CAP = 1500` truncates 18.3% of guide sections and 25.3% of internals sections, taking roughly 15% of each book's prose out of the index. There is no signal to the reader or to the author.

6. **Whole-book search disappears on short chapters, invisibly to the author.** The search-index global rides inside the TOC-gated script block. On `reference/shell-completion.html` the Search button renders and the palette degrades to a DOM scan of that one page. `corpus/demo-book` is worse: all six built pages lack the wiring while `search-index.js` is emitted and referenced by nothing.

7. **The two deepest heading levels render at lower contrast than body text.** `base.css:334-336` sets `h5`/`h6` to `--tali-muted` (#555) while body is `--tali-fg` (#1a1a1a). This inverts the layer-cake hierarchy exactly where `docs/guide/reference/` nests deepest. Note the accessibility framing is *wrong*: all three themes' muted colour passes WCAG AA (7.46:1 light, 6.75:1 dark, 5.38:1 sepia) and `base.css:843-845` already remaps muted to fg under `prefers-contrast: more`. This is a typographic hierarchy defect, not a contrast defect.

8. **Text in non-active tab panels is invisible to Ctrl-F.** `assets/js/tabset.js:28` does `panel.hidden = !on`. Repo-wide grep for `until-found`/`beforematch` returns zero. Compounding it, `crates/core/tests/tarn.rs:42` actively asserts that hidden-tab content **is** in the Cmd-K index, so the project promises findability with one tool and breaks it with the other. Cmd-K's own arrival path is worse than "does nothing": `firstTermRange` (search.js:608-637) walks a TreeWalker with no visibility filter, finds the term inside the hidden panel, gets an all-zero `getBoundingClientRect`, so the off-screen scroll test never fires and the highlight is painted on invisible content.

9. **No length or cost signal at any decision point.** Chapters in the guide span 259 to 4,077 words with nothing shown in the drawer, the landing Contents, or the pager. The one length measure the tool computes is structurally unavailable to books.

10. **The scrollspy activation line is wrong on book pages, and on website pages.** `toc-spy.js:64-67` measures `.tali-site-nav`, which books never emit (they emit `.tali-book-topbar`), so the line falls back to 16px against a 64px `scroll-margin-top` and the spy highlights the *previous* section after a click. Verified failing on websites too, by a smaller margin (fractional offset flips the strict `> 0` comparison).

11. **`description:` is load-bearing and universally unset, with no derived fallback and no warning.** The landing Contents blurb, `<meta name="description">`, og/twitter and JSON-LD all read it. Nothing warns; nothing derives.

12. **`taliesin check` has 27 diagnostic families and none concerns document structure.** `check docs/guide` prints "no problems found" on a 32,600-word book with a 4,077-word chapter behind 9 headings and a broken section-number scheme on every page. The only heading rule (`a11y.rs:243`) fires solely on a mid-document level skip of >= 2. This is also a genuine market gap: measured from source, Vale's Google style is 2 of 31 rules structural, Microsoft 4 of 39, proselint 0 of 26, and markdownlint's structural rules are syntactic (none knows how many words sit between two headings).

13. **Machine projections are blind to structure.** `taliesin map docs/guide` prints 19 rows of url + title and "10 cross-reference target(s)". `read --json` returns `{path, executed, cells, text}` with `text` as one 24,598-character blob. The LSP outline types every node `SymbolKind::STRING` with `detail: None` at `lsp.rs:806` and `:809` in `to_document_symbol`, even though `lsp_outline.rs:16` already carries `end_line` per node. `llms.txt` is gated behind a `url:` neither book sets.

14. **The preview and the build disagree about which headings are in the TOC.** `client.js:847` selects headings by absolute tag (`h1[id], h2[id], h3[id]`) while the build filters relative to the shallowest, so a title-demoted page's h4/h5 rows appear in the built TOC and not in the preview. The author therefore tunes navigation against a TOC readers never see.

15. **The printed TOC shows only the active branch.** `base.css:725` sets `#TOC ul ul { display: none }` and the print block (`base.css:850-881`) overrides `#TOC`'s position, max-height, overflow, z-index, background, margin, padding and `::before`, but **not** the nested-list collapse. Measured: a printed chapter's TOC shows 2 of 8 entries. For a 150k-word book, print is a real consumption path.

16. **No book-level reading position.** Progress, resume and read-state are all keyed by `location.pathname`, so a 19-chapter book has 19 independent progress bars and no memory of which chapter you were in.

17. **Nothing pins book-scale behaviour.** The regression net's largest book is about 2% of the target scale.

## Recommendations

30 candidates survived; 4 were killed. Each entry folds in every correction from the adversarial pass. Effort is relative: small = under a day, medium = a few days, large = a week-plus, epic = a project.

**Fragment numbers are assigned centrally here so two items cannot claim the same slot.** The `code-enhance/` directory currently ends at `18-media.js`. New fragments: `19-book-outline.js` (`book-outline-artifact` Ship B), `20-heading-keystep.js` (`heading-keystep`). Note `CODE_ENHANCE` in `render/mod.rs:1432-1448` is filename-ordered with `09-register.js` sitting **before** the fragments it registers, so a new fragment relies on function-declaration hoisting inside the single concatenated script.

**`prose::word_count` (`prose.rs:69`) is `pub(crate)` today and two items need it public.** Ownership: whichever of `chapter-cost-signal` or `machine-shape-projections` lands first performs the visibility change; the other references it as a dependency and does not restate it.

---

### Cluster: corpus substrate (session 0)

#### grow-tarn: one real book at book-ish scale

**Problem.** Gap 17. Invariant 6 requires every capability to ship pinned by a corpus doc, but the regression net's largest book is `corpus/tarn` at 6 chapters / 1,135 words. Seven recommendations below currently propose their own book fixture, three of them naming `corpus/course`, which has only 5 published chapters (`problems.tmd` is `draft: true`) and 1,058 words. Without one grown book they will each mint a fixture and the net will still pin nothing at scale.

**How it works.** Grow `corpus/tarn` (already a book with two parts) to an acceptance criterion, in one change, before anything downstream lands:

- at least 12 chapters, at least 3 parts;
- at least one nested `{part:, chapters:}` group (so `fix-nested-parts` has something to pin);
- at least one chapter below `MIN_TOC_HEADINGS` (so `search-on-every-page` has something to pin);
- at least one `###`-rooted titled chapter and one titled chapter carrying a body `# H1` (so `fix-book-section-numbers` has both hard cases);
- at least one section over `BODY_CAP` whose distinctive term sits in its final paragraph (so `unbounded-index` has something to pin);
- at least two `{.definition}` blocks and one unnumbered appendix (so `glossary-autolink`, `book-term-index` and the front/back-matter question have a substrate).

**Do not mint `corpus/longbook`.** The corpus walker renders every doc on every `cargo test` run, and `corpus/README.md` frames the corpus as real documents, not synthetic scale tests. Growing an existing real book is both cheaper and honest.

**Value** high (it is the precondition for every book-scale pin). **Effort** medium (it is writing, not code). **Risk** low, but note the whole corpus test suite gets slower and `body_html_snapshots` for `corpus/tarn` will need re-blessing.

**Corpus pin.** It *is* the pin. Every downstream item's "Corpus pin" line below references `grow-tarn` rather than proposing its own book.

---

### Cluster: correctness fixes (do these first)

#### fix-book-section-numbers: section numbers read 4.1, not 4.0.1

Number a chapter's sections from its own shallowest body-heading level instead of absolute level 2, at all three numbering sites at once.

**Problem.** See gap 1. The real defect is a render/registry *divergence*: the link text and the heading disagree, and both are wrong in different ways.

**Author surface:** nothing. **Reader surface:** headings, TOC rows and `@sec-` link text all read `4`, `4.1`, `4.1.1` and agree with each other.

**How it works, corrected.** The earlier draft prescribed a one-call-site fix in `site/chapter.rs` and forbade touching `site/xref.rs:87` and `render/mod.rs:533` on the grounds that they read undemoted source levels and are "already correct". **That is only true when a chapter's shallowest source heading is `##`.** For a `###`-rooted titled chapter with no `##` at all, rendered levels are h4, so a chapter-local base gives `N.1` while `xref.rs` and `render/mod.rs:533` still see source level 3 and produce `N.0.1` via `counters[1]`. The two diverge, and the item's own lockstep pin fails.

So the fix is a **shared signature change**, applied to all three call sites together:

1. Change `crate::site::section_number(chapter, level, counters)` to `section_number(chapter, level, counters, base)`.
2. Replace the `level <= 1` guard with `if level < base { return chapter.to_string(); }`, and index `let i = (level - base).min(counters.len() - 1);`. Because `base` is always >= 2 and the guard rejects anything shallower, **underflow is impossible**; this is the explicit resolution of the `counters[2 - 3]` panic path.
3. At each call site, compute `base` in *that site's own level space* as the minimum heading level >= 2 over the chapter's headings, defaulting to 2 when there are none. `site/chapter.rs::number_chapter_headings` does a pre-pass over rendered blocks (excluding the `tali-title-block` header, which keeps the bare chapter number). `site/xref.rs::scan_page_anchors` and `render/mod.rs:533` do a pre-pass over source heading levels. Demotion is a uniform +1, so the two bases shift together and `level - base` is identical on both sides. That is what makes them agree.

Consequences worth stating explicitly:

- A titled chapter that *also* carries a body `# H1` now numbers that H1 as `N.1` and its `##` sections as `N.1.1`. That is coherent (the H1 is a real top-level section under the title) and it removes the special case rather than adding one.
- An untitled `# H1` chapter (`corpus/demo-book/methods.tmd`) is unchanged: base is still the minimum level >= 2, the H1 is below base, so it prints the bare chapter number and `##` prints `N.1`. The existing pin keeps passing.

**Honest residual.** `reference/cli.html` does **not** become clean. Its first heading is a source `###` appearing *before* any `##`, so base = 3, that heading gets depth 1, and it prints `16.0.1`: still a zero segment. That is arguably correct (the heading has no parent section) and matches Quarto's behaviour. Post-fix, the real strings are `using/reading.html` → `6`, `6.1`, `6.2` (clean) and `reference/cli.html` → `16`, `16.0.1`, `16.0.2`, `16.0.3`, `16.1` (a residual zero on the pre-`##` headings). Drop `cli.html` from the exemplar list in any commit message, and consider a TOC-DROP-style suggestion lint for a chapter whose first heading is deeper than its shallowest heading.

**Value** high. **Effort** small. **Risk** low. Block ids hash *source*, not HTML (`make_id`, `render/mod.rs:2249`), so `data-block-id` is unaffected; the earlier "ids will churn" worry was false. `body_html_snapshots` for every numbered corpus chapter will need re-blessing.

**Invariants.** This *does* touch the numbering-scanner zone named in the do-not-touch list. Say so plainly and justify it: all three sites already call one shared `section_number`, so this is a parameter added to one function plus three pre-passes, not a rewrite of the scanners. Zero config, offline, HTML-only.

**Corpus pin.** `grow-tarn`, asserting the **lockstep** property in four cases: a `##`-rooted titled chapter, a `###`-rooted titled chapter, a titled chapter carrying a body `# H1` plus `##` sections, and the existing untitled `# H1` chapter in `corpus/demo-book` (unchanged). In each case the rendered heading number equals the TOC row equals the resolved `@sec-` link text. **Default-on.**

---

#### fix-nested-parts: a nested part stops deleting its chapters

**Problem.** See gap 2. Verified in source: `site/book.rs::build_book`'s outer loop handles `{part:, chapters:}`, but the inner loop over that part's `chapters:` calls `push_chapter_entry(root, c, &mut entries, &mut num, mode, excluded);` and drops the `bool`. `push_chapter_entry` returns `false` precisely to mean "this is some other shape, caller must handle it", so a nested group and every chapter beneath it vanishes.

**Author surface:** either the nesting works or the author gets a located warning. **Reader surface:** the chapters exist.

**How it works.** Two acceptable resolutions, pick one and pin it:

- **(a) Flatten.** Treat a nested group as a flat continuation: emit its chapters into the same entry list, either dropping the inner part label or rendering it as a sub-divider. Cheapest, no schema change, no reader surprise.
- **(b) Reject loudly.** Keep one level of nesting as the supported model and emit a `_site.yml` warning naming the offending part when the inner loop sees a non-chapter shape.

Either way, **also give `_site.yml` warnings a line number**. `check.rs` currently wraps config warnings as `Diagnostic::new("_site.yml", None, …)`, so the file where a book's structure is authored is the only file in the project with no click-to-source and no LSP squiggle. `site/config/mod.rs` already walks the YAML; carrying the offending node's line through the warning is a small independent fix that makes every future `_site.yml` diagnostic navigable.

**Value** high (this is silent data loss). **Effort** small. **Risk** low.

**Corpus pin.** `grow-tarn` includes a nested group; assert every chapter under it appears in `book.entries` (flatten) or that `check` reports a located warning naming the part (reject). Either assertion fails today. **Default-on.**

---

#### find-in-page-honesty: collapsed content stops lying to Ctrl-F

Emit inactive tab panels with `hidden="until-found"`, and open collapsed `<details>` on programmatic arrival.

**Problem.** See gap 8.

**Author surface:** nothing. **Reader surface:** Ctrl-F finds a string in a non-active tab and the browser reveals it; a Cmd-K hit or `#anchor` inside a closed disclosure opens it instead of stranding the reader.

**How it works.** **Four** edits, not two. (1) `render/divs.rs:622` emits `hidden="until-found"` for inactive panels. (2) **`base.css:582` must be narrowed** to `.tabset-panel[hidden]:not([hidden="until-found"]) { display: none }` or the attribute is a silent no-op (the author rule beats the UA `content-visibility`). (3) **Add `.tabset-panel[hidden="until-found"] { contain-intrinsic-size: 0 }`** (or an explicit zero block size). `hidden="until-found"` keeps the element in layout, so on `corpus/tarn/install.tmd` (two tabsets, six panels, pinned at `tarn.rs:30-38`) five panels would go from `display: none` to zero-content-but-boxed elements stacked under the visible one, and the existing `.tabset-panel > :first-child { margin-top: 0 }` rule at `base.css:583` no longer guarantees the visible panel's top margin, because the visible panel is no longer the only one generating a box. (4) `tabset.js` must write `panel.setAttribute('hidden','until-found')` rather than `panel.hidden = true`, since the boolean IDL setter erases the value on the first tab click and permanently kills the feature; add a `beforematch` listener calling the existing `select(tab)` so `aria-selected` and the roving tabindex follow, feature-detected via `'onbeforematch' in HTMLElement.prototype`.

Scope the `<details>` half to the **programmatic** arrival path only (`search.js::go()`, `flashFromSession`, hash/xref jumps). Chrome and Firefox already auto-expand closed `<details>` for native find-in-page, so the genuinely new Ctrl-F win is tab panels. Hook the reveal to the Range that `firstTermRange` already finds, walking `closest('details')` / `closest('[role=tabpanel]')` upward *before* measuring and scrolling.

**Verification step, not optional.** Per the project's viewport-matrix rule, chrome-devtools-check `corpus/tarn/install.tmd` at 390x844, 1440x900 and 900x1440 and confirm the visible panel's `offsetTop` is unchanged from today.

**Value** high. **Effort** small. **Risk** low.

**Invariants.** Read-only, offline, block model untouched (the attribute rides an element inside a block; block attrs sit on the `.panel-tabset` container). Note honestly that `hidden="until-found"` does *not* put the subtree in the accessibility tree until revealed, so this fixes find-in-page, not screen-reader skimming.

**Corpus pin.** Extend `corpus/tarn/install.tmd` (already dual-tabset, already pinned for hidden-tab indexing): assert the second panel carries `hidden="until-found"`, that the zero-intrinsic-size rule exists, and that the visible panel's first-child margin rule still applies; plus a collapsed callout whose anchor is a cross-reference target. **Default-on.**

---

#### search-on-every-page: un-couple whole-book search from the TOC gate

**Problem.** See gap 6.

**Author surface:** nothing. **Reader surface:** Cmd-K searches the whole book from every page.

**How it works, corrected.** This is not just moving a global. `render/page.rs:483-497` bundles the index global together with `toc_scripts()`, and `toc_scripts()` (`render/mod.rs:1392`) is a single string concatenating `TOC_SPY_JS` + `TOC_SHEET_JS` + `SEARCH_JS`. The built guide ships everything inline (no page references `app.js`), so on a TOC-less page today the Search button renders with **no palette code at all**. Un-gating only the global leaves the button dead. The change is:

1. Split `toc_scripts()` into `toc_scripts()` (spy + sheet, TOC-gated) and a new `search_scripts()` (SEARCH_JS, gated on a non-empty `site.search_index`).
2. Emit them independently from `render/page.rs:483-497`. Frame it as decoupling from `doc.toc` entirely, not as relaxing `MIN_TOC_HEADINGS`: the heading count is only one of four ways `doc.toc` goes false.
3. Fix `render/tests.rs:707`, which asserts on `toc_scripts()`'s current composition. `toc_scripts` is `pub` API.
4. Do **not** move the global into `page_chrome`'s head: `serve_site/mod.rs:686` already injects it separately in preview and would double-assign.

**Value** high. **Effort** small. **Risk** low (was "none"; it is a `pub` signature change plus a test edit).

**Corpus pin.** `grow-tarn`'s below-`MIN_TOC_HEADINGS` chapter, plus the existing `corpus/demo-book` (all six built pages currently lack the wiring): assert the built page carries `TALIESIN_SEARCH_URL` **and** `SEARCH_JS`, and no `<nav id="TOC">`. Note in the same change that 404 and deck pages still render a Search button with no `app.js` at all; this fix does not close that. **Default-on.**

---

#### scrollspy-activation-line: the TOC highlight tells the truth

**Problem.** See gap 10.

**How it works.** Derive the activation line from `getComputedStyle(heading).scrollMarginTop`, which the browser resolves to px on every page kind, rather than from `getPropertyValue('--tali-nav-h')` (which returns the unresolved token `"3rem"` and would need rem math plus a second `+1rem` constant). Fall back to `|| 16` when it computes to 0, which is exactly what `base.css:654`'s standalone `scroll-margin-top: 1rem` already equals, proving that was the intended semantic.

**Specify where the read happens.** `line()` (`toc-spy.js:64-67`) is called on every scroll event and `getComputedStyle` forces a style flush, so it must not sit on the scroll path. **Sample once in `collect()`, and re-run `collect()` on a debounced `resize` or a `matchMedia` change listener keyed to the same breakpoint `site.css` uses.** A once-only sample goes stale exactly when the `--tali-nav-h` media query flips, which is the 900x1440 narrow-tall band the project's own viewport matrix insists on testing.

Add a 1-2px tolerance to the `heading.top - ln > 0` comparison so fractional offsets do not flip the highlight. Correct the scope claim: this fixes **websites too**, verified failing.

**Value** medium. **Effort** small. **Risk** low.

**Corpus pin.** A manual browser check is not a regression pin, and this is precisely the bug class that regressed silently. Pin the derivable half in Rust: assert that a built book page emits the `scroll-margin-top` token the activation line reads (so a future CSS rename fails a test rather than silently re-breaking the highlight), using `corpus/demo-book` plus `grow-tarn`. Then browser-verify at 390x844, 1440x900 and 900x1440 as the acceptance check, not as the pin. **Default-on.**

---

#### print-toc-expand: the printed TOC shows the whole chapter

**Problem.** See gap 15. One line, verified missing from the print block.

**How it works.** Add `#TOC ul ul, #TOC li > ul { display: block !important; }` inside `base.css`'s `@media print` block (`:850-881`), beside the existing `#TOC` un-sticking rules.

**Value** small but real at book scale (print is a genuine consumption path for a 150k-word book). **Effort** trivial. **Risk** none.

**Corpus pin.** A CSS-rule assertion in the existing `render/tests.rs` style, asserting the print-media rule exists (assert on the rule *body*, not the selector text, per the `heading-contrast` note below). **Default-on.**

---

### Cluster: typography (default output)

#### heading-contrast: un-mute h5/h6

Drop `color: var(--tali-muted)` from the `h5`/`h6` rules at `base.css:334-336`; depth stays carried by size, h5's uppercase + `.04em` letterspacing, and the existing ~3:1 space-above:space-below ratio. Add a guard test in the existing `render/tests.rs` style (asserting on rule *bodies*, not selector text, so it does not false-positive on `.tali-anchor`).

**Argue it on typographic hierarchy alone.** Delete the WCAG framing: all three themes pass AA and `prefers-contrast: more` already handles the low-vision case. Frame it explicitly as reversing a documented decision (`notes/ROADMAP.md:295` records "uppercase/muted h5/h6" as an intended element of the hand-tuned scale).

**Value** small correctness polish, not high. **Effort** small. **Risk** low. Reach is honest: exactly 4 `####` headings and 0 `#####` exist across `docs/` and `corpus/` (measured), so this matters for future reference pages more than current ones, and **after this change the corpus fixture is the sole exerciser of h5/h6 styling.**

**Corpus pin.** `corpus/layout/heading-scale.tmd`, authored in the same change, with a front-matter `title:` and **both** a `####` (which demotes to `<h5>`) and a `#####` (which demotes to `<h6>`), so the guard test covers both edited rules. **Default-on.**

**Cut from the original proposal:** the `:is(h2,h3) + p` "section lede lift". Two of its three declarations are already global no-ops (`text-wrap: pretty` at `base.css:171`; paragraphs already inherit `--tali-fg`), leaving a lone `font-size: 1.04em` at or below the just-noticeable difference, and its selector misfires under heading demotion. If revisited, it must key off *source* heading depth via a block-model marker, not the emitted tag.

---

### Cluster: substrate

#### section-extents: decide whether the DOM knows where a section ends

**Problem.** See gap 4. This is not a feature, it is the substrate four features need, and it deserves an explicit decision rather than a parenthetical in two other items' risk sections.

**Three options, pick one and record the reasoning.**

- **(a) Wrap.** Emit `<section data-section-level="N">` around each heading-to-next-heading run. Gives every downstream feature a real element to fold, mark, measure and observe. **This is the risky one and the reason the item exists:** a wrapper changes the parent/child shape the incremental diff mounts. `diff.rs` and `client.js` currently mount blocks as flat siblings of one root; introducing a nesting level means the diff has to reason about a container whose children change without the container changing, and about a heading edit that re-parents every following block. That is a real design question, not an implementation detail.
- **(b) Mark.** Emit `data-section-end="<block-id>"` (or `data-section-blocks="N"`) on each heading block, computed at build time from the same walk `lsp_outline.rs` already performs. Purely additive attribute on an existing block, invisible to the diff, invisible to the corpus invariants, and sufficient for every consumer that only needs to *enumerate* a section's blocks (per-section length, section-scoped read state, section-scoped change marks, a JS-driven fold). Insufficient only for CSS-only folding and for `content-visibility: auto` on a real container.
- **(c) Decline** and record why, so the next audit does not rediscover it.

**Recommendation: (b), with (a) explicitly deferred.** The marker unblocks three of the four dependent proposals at near-zero risk; only `reading-density-dial` (which this audit already says not to schedule) actually needs a container. Revisit (a) only if a scheduled feature demonstrably needs one.

**Ruling on `content-visibility: auto`, which (a) would enable.** It is the one platform performance lever that keeps content in the accessibility tree and in find-in-page, and the block model can supply a `contain-intrinsic-size` nobody else can guess (per-section block and line counts are known at build time). It is currently unused: zero matches in the bundled CSS. **Deferred behind a measured trigger, not ruled out:** revisit when a single built chapter exceeds, say, 250 KB of HTML or a measured interaction-latency problem exists, and only after option (a) lands. The Safari find-in-page caveat means it must be validated in three engines, not assumed.

**Value** medium (as an enabler). **Effort** small (b), large (a). **Risk** low (b), medium (a).

**Corpus pin.** `corpus/layout/structure.tmd`, which `notes/FEATURE-IDEAS.md` #26 already names as a pin and which does not exist. Assert every heading block carries its section-extent marker, that the marker points at a real `data-block-id` in the same document, and that a section ending at end-of-document is handled. **Default-on.**

---

### Cluster: build-time derivation

#### section-gist: one gated first sentence, two consumers

Extract each section's and chapter's first sentence at build time under rustdoc's rule, emitting **nothing** when the gate fails.

**Problem.** See gap 11 plus the general scent-starvation of bare headings.

**Author surface:** nothing, and specifically not a new key. An explicit `description:` still wins (DITA precedence); the derivation is the fallback.

**How it works.** A pure `render::gist(section_blocks) -> Option<String>` in Rust: first prose paragraph of the section, through `render::indexable_text` (the shared HTML-to-text projection behind search and `taliesin read`, so preview and Cmd-K cannot disagree), cut at the first terminal punctuation with an abbreviation guard, **rejected** if it exceeds about 30 words / 200 chars, has no terminal punctuation, ends in a colon, or is dominated by math/code spans.

**Do not** emit `data-lede` on the heading block. Every consumer already holds `&[Block]` at build time, and putting block N+1's prose into block N's HTML would force a full `BlockOp::Update` (rather than the cheap `SetMeta`) on every edit to a section's opening paragraph, churning `body_html_snapshots` on every heading in the corpus for no gain.

**Consumers, trimmed from five to two:** (1) `book_toc.rs::desc_of` as the fallback under `description:` (the styled-but-always-empty `.tali-btoc-desc`); (2) `<meta name="description">` / og / twitter / JSON-LD via `meta.rs:116`. **Cut the sidebar TOC sub-line** (a 14rem rail with `max-height: 92vh` and 0.12rem link padding turns an 8-entry page into ~50 wrapped lines). **Cut the Cmd-K `g` field** (search.js already renders a query-centred marked KWIC window; the bare-prefix fallback it would replace is essentially unreachable). Optionally feed the chapter opener if `summary-div` ships.

**On the gate's hit rate.** The earlier draft asserted "about 50-64% of real sections pass" as a measurement. **It is not reproducible: no script, no corpus scope, no definition of "section" was recorded.** Treat it as an *estimate, low confidence*, and make measuring it the first step of implementation: write the gate, run it over `corpus/` and `docs/`, and report pass rate plus a count of passes that are tautological or backward-referencing ("This section explains that."). The item's value rating and its "silence is the correct failure" framing both depend on that number, so it should be produced before the consumers are wired, not after.

**Value** medium (down from high once the consumer list is honest, and pending the gate measurement). **Effort** medium. **Risk** low.

**Invariants.** Deterministic Rust, no model, no network, reproducible. Derivation, not generation.

**Corpus pin.** `grow-tarn`'s landing page with chapters that have no `description:`: assert a well-formed lede fills the Contents blurb and the meta tag, a 60-word opener yields neither, and an explicit `description:` still wins. **Default-on.**

---

#### long-output-fold: bound runaway cell output and oversize tables

**Problem.** The biggest density offender in a computational document is a 400-line stdout, a 200-row dataframe dump, or a 4000px matplotlib PNG: content the author cannot shorten without losing the artefact. `.tali-output` (`base.css:622-626`) has no height cap of any kind; the existing `code-fold` is author-opt-in and folds the *source*. The kernel's guard rails cap at 4096 items / 512 KB, two orders of magnitude above a screenful.

**How it works, corrected.** Ship it as **CSS, not a disclosure**: give `.tali-output > pre` and oversize tables a `max-height` (bounded by both a rem cap and a vh cap so the box never exceeds the viewport) plus `overflow-y: auto`, with a `max-height: none !important` companion in the print block (`base.css:872-873`, which already forces `overflow: visible` for print). Both target boxes are already scroll containers (`pre` is `overflow: auto`, tables are `display:block; overflow-x:auto`), so the bound itself is genuinely two lines and works in every browser, keeping text copyable, findable and in the a11y tree.

**The fade is not a reuse; it is new.** Verified: `base.css:625` is `.tali-output > pre { background: var(--tali-code-bg); border-left: 3px solid var(--tali-border); }`, a `background` **shorthand**, which already resets the 4-layer scroll-shadow gradients declared on `pre` at `base.css:349-363`. So there is no existing shadow on `.tali-output > pre` to reuse. And the pattern that does exist (bare `pre`, `table`, `base.css:412-419`) is horizontal-only (`to right` / `to left`, `left center` / `right center`, `background-attachment: local, local, scroll, scroll`). A vertical fade means writing a **new** top/bottom 4-layer set into that same shorthand, which `base.css:887`'s own comment already flags as fragile (`the background shorthand above drops the generic pre rule's shadow`). Budget accordingly.

**Decide `.tali-output img` explicitly.** It is neither a `pre` nor a table (`base.css:626`: `display: block; max-width: 100%`), so a 4000px plot is left unbounded and the stated problem is only half addressed. Bound it with the same rem+vh cap, or state why not (e.g. a plot is a single glanceable artefact where a scrollbox is worse than a tall image). Recommendation: bound it, because a matplotlib default-dpi figure at book width is the single most common density offender in the corpus.

**Drop `hidden="until-found"` here entirely.** Its reveal-and-scroll promise is Chrome-only today, and in older Safari the value falls back to `display:none`, which would make a traceback silently un-copyable and absent from print. Jupyter's own answer to this exact problem is a scrolled output container, not a collapse.

**Value** high. **Effort** small (bound) to medium (bound + vertical fade + image case). **Risk** low.

**Invariants.** Explicitly avoids exec/freeze/kernel by bounding in CSS, not at output-emit time. Zero config: the threshold is a constant, and if it ever needs to vary that signals the default is wrong.

**Corpus pin.** `corpus/layout/dense-output.tmd` with a static long `<pre>`, a 200-row table and a tall image (kernel-free so the corpus test runs without Jupyter), asserting the CSS bound, the print reset, **and that the horizontal scroll shadow on bare `pre` is unchanged by the edit**. **Default-on.**

---

### Cluster: search index

#### unbounded-index: delete BODY_CAP, stage the rest

**Problem.** See gap 5.

**Stage 1 (do this).** Stop truncating; split a long section into several records sharing the heading anchor but each carrying its own body window, split on **block** boundaries (which also strengthens the block-model story). Measured, uncapping grows indexed text only about 1.17x on the real books, and a Node benchmark of the real `score()` gives 0.75-1.01 ms per keystroke on the guide index and 6.6-11.6 ms at 5x scale, so the matcher is not the constraint.

**Stage 2 (defer behind a measured trigger).** A chunked inverted index (term -> postings, alphabetically ordered chunk files plus a boundary manifest, Pagefind's design, loaded as `<script>` subresources so `file://` keeps working). **Do not schedule this yet.** The real byte problem in this project is the 810 KB-gzipped inlined-mermaid page shell, not the 59 KB lazy index, and the dev-loop question is unsolved: `install_search_fragment` works because the index is a concatenation of per-page JSON fragments, and a term-keyed index has no per-page fragment. Trigger: a real document whose index exceeds 250 KB gzipped or whose keystroke cost exceeds 16 ms, plus a design for incremental chunk re-emission.

**Drop the proposed `check` truncation warning.** It would fire on 18-25% of sections, blame the author for a hidden constant, and Stage 1 deletes the condition anyway.

**Value** high (Stage 1). **Effort** small (Stage 1), epic (Stage 2). **Risk** low / medium.

**Corpus pin.** `grow-tarn`'s over-`BODY_CAP` section, whose distinctive term appears only in its final paragraph, asserted present in the built index (today it is not). If a `corpus/course` fixture is used instead, **it must also be registered in `corpus/course/_site.yml` under `chapters:` in the same change**, or it is not a chapter and the pin is inert. **Default-on.**

---

#### book-shaped-search-results: group by chapter, survive partial matches

**Problem.** `search.js:379` sorts flat by score with heading level as a bare tiebreak, so one dense chapter monopolises the visible rows; `score()` hard-ANDs (`else return 0`), so one mistyped word annihilates the result set and the reader gets a bare "No matches"; and the record `{u,p,i,l,t,b}` knows nothing about which chapter it is in.

**How it works.** Producer: add **two** genuinely free fields, `c` (chapter number, already an argument to `page_fragment` and discarded) and `h` (ancestor heading path, derivable from the level sequence `headings_with_pos` already returns). Client: group by `it.u`, synthesising the parent row from the level-0 entry, with the top 3 sections and a "+N more in this chapter" expander; keep items that match >= 1 term, sorted below full matches, with a struck-through `Missing: <term>` line; make `within1` Damerau-aware so transpositions (the most common typo class) are tolerated, which captures most of what a did-you-mean vocabulary would.

**Cut from the original:** the `k` content-kind bitmask and kind facets. `Block` is `{id, sourcepos, source_file, html, cell}` with no kind field, so deriving it is HTML class-scanning wearing an invariant-4 costume, and its 10-value enum has no bucket for `div.tali-embed`, `details.tali-code-fold`, `nav.tali-book-landing-toc` or `div.tali-cell-hidden`. **Also cut:** the Pagefind h1=7..h6=2 quadratic weighting. Those numbers are a *per-word* weight inside a page-granular record; applied as a per-record multiplier over Taliesin's already section-granular records it would rank every chapter-title record above every h3 record for every query, which is where the answers actually live. Keep the existing +6/+3 with level as a mild tiebreak.

**Two regressions to guard.** Command-palette actions are scored by the same `score()` and pinned above content, so relaxing AND needs an explicit AND carve-out for actions; and the single-doc DOM branch produces items with no `url`, so grouping must be book-only. Add a result cap (AND semantics is currently the only bound on multi-term queries).

**Value** high. **Effort** medium. **Risk** low.

**Invariants.** Read-only overlay, three additive fields on a lazily-loaded same-origin file, zero keys (chapter scoping, if added, is a transient keystroke; Material's `search.boost` and per-page `search.exclude` are exactly the sprawl to avoid).

**Corpus pin.** `grow-tarn` (12 chapters, so grouping has something to do): a term occurring in three chapters, plus a four-term query yielding two partial matches and zero full matches. Do **not** pin this on `corpus/course`, which publishes only 5 chapters (`problems.tmd` is `draft: true`), where "a term in three chapters" covers most of the book and the grouping assertion is near-vacuous. **Default-on.** Pairs with `find-in-page-honesty`: grouped results are worse than useless if selecting one lands on collapsed content.

---

#### text-fragment-landing: ship the ?h= half, gate the :~:text= half

**Problem.** A Cmd-K hit hands its highlight to the destination through `sessionStorage`, so it is invisible in the URL: copying the address bar, reloading, or sharing all lose it, and only the first occurrence in the landed section flashes.

**Ship first:** replace the sessionStorage handoff with a `?h=<terms>` query parameter, re-marking on arrival with the existing CSS Custom Highlight API path. Small, validated by Material for MkDocs (`search.highlight` + `search.share`), works for 100% of hits, and makes the highlight survive copy, reload and back.

**Gate the `:~:text=` half.** It cannot ship reliably from the current index: `indexable_text` is `decode(strip_tags_separated(html))`, and `strip_tags_separated` inserts a space at **every** tag boundary, so `<p>Use <code>--out</code>.</p>` indexes as "Use --out ." while the page reads "Use --out." A text fragment containing a space the document lacks does not match. Measured on the dogfood books: 669 of 876 prose paragraphs contain inline code, and 472 contain an inline element immediately followed by punctuation, and the query on a docs site is usually the identifier itself, i.e. centred exactly on the hazard. Add block-boundary markers or a fragment-safe per-section projection in Rust first, plus a deck flag (`flashTermsIn` deliberately no-ops on `.tali-deck`, and a native fragment cannot be suppressed at the destination).

Also correct the problem framing: `flashTermsIn` already scrolls the match into view, so the honest gap is durability and shareability, not "lands you at the top".

**Value** medium. **Effort** small (`?h=`), medium (fragments). **Risk** low.

**Corpus pin.** The `?h=` round-trip is a **cross-page** Cmd-K handoff, so it needs a real site project. `corpus/reader/` has no `_site.yml` (the corpus site projects are exactly: bayesian-website, cite-this, course, demo-book, descent, embed, scaffold-book, scaffold-site, tarn, tech-blog, theorem-book), so a `corpus/reader/deep-links.tmd` cannot exercise it. Pin it in `corpus/demo-book` or `grow-tarn`, asserting the `?h=` round-trip and, if fragments land, that `-` and `,` inside a matched sentence are escaped correctly. **Default-on.**

---

### Cluster: wayfinding

#### book-outline-artifact: whole-book section outline, one build-time artifact

The single largest structural win. **Split into two ships.**

**Ship A (small, no new artifact): make the Cmd-K empty state structural.** `search.js:373` currently does `index.filter(it => it.level === 0)`, showing the same flat chapter list the drawer shows. The index already carries every section with its level and anchor. Grouping by page instead of filtering is a pure client change with zero Rust and zero new build artifact, and it is the highest value-per-token piece of this whole audit.

**Ship B (large): a lazily-loaded outline sidecar for the drawer.** Emit a per-page fragment beside `search::page_fragment` carrying only `{url, anchor, level, number, title}`. Measured justification, as a **ratio** so it stays true across builds: the body field is 87% of the index's raw bytes and 92% of its gzipped bytes, so an outline-only sidecar is roughly 8x smaller raw and 13x smaller gzipped than the search index for the same records. Assemble at the output root, load via a `<script>` element on first drawer expand, exactly the pattern `hover-index.js` and `search-index.js` already use so it works under `file://`. `code-enhance/19-book-outline.js` hydrates nested `<ul>`s into the existing `<ul class="tali-book-chapters">`; `chrome.rs` emits a `<button aria-expanded>` plus an empty slot per row. With JS off the drawer stays exactly the flat list it is today.

**Invalidation.** The sidecar is per-page but its *assembly* is whole-project, and a heading edit on page A changes only A's fragment. Copy `Site::refresh_search_for_page` verbatim as `refresh_outline_for_page`; that is the existing precedent and it keeps the preview incremental.

**Explicitly cut:** gists and length bars in the drawer (137 auto-generated sentences stacked in a modal nav list is the "auto-summary too weak to trust" plus "chrome competing with content" failure). Number + title only. **Drop the `hidden=until-found` claim**: the whole drawer is emitted `hidden`, so nothing inside is find-in-page reachable either way (this does not make things worse than today, but stop citing it as mitigation).

**Honest scoping note.** mdBook, Docusaurus, GitBook and Starlight sidebars list author-declared *pages*, not harvested in-page headings; Material nests a page's TOC under the *active* page only. Expanding any chapter to see its sections is beyond all the cited precedent, and that restraint is likely deliberate at scale. Ship A is unambiguous; Ship B is the genuinely new bet.

**Value** transformational (the pair). **Effort** small (A), large (B). **Risk** low (A), medium (B).

**Dependencies, corrected.** **Both ships depend on `fix-book-section-numbers`.** The search index's heading `t` field is built from rendered HTML *after* `number_chapter_headings` runs, so a structural Cmd-K empty state grouped by page renders `4.0.1` rows exactly like the drawer would. Ship B additionally depends on `search-on-every-page`, or short chapters have no index to read.

**Corpus pin.** `grow-tarn`. Ship B: assert the outline lists every anchored heading of every chapter with its correct section number, and that coverage includes chapters below the TOC gate. **Ship A is not unpinnable and should not be sold as such:** pin it the way the project pins other client JS, with a Rust test asserting the built index carries the `l` (level) and `i` (anchor) fields the grouping keys off for every section of a multi-chapter book, plus the `web-client` jsconfig type-check. That is a real regression net for the producer side; the grouping render itself stays unpinned and that should be stated, not glossed. **Default-on.**

---

#### drawer-typeahead: a filter box in the chapter drawer

**Problem.** See "What changes at 150k words". The drawer is the only always-available cross-chapter navigation and it is a flat unfilterable scroll list. At 19 rows this is invisible; at 60 rows it is the drawer's only usable entry point.

**How it works.** A single `<input type="search">` at the top of `#tali-book-drawer`, filtering the existing `<li>` rows by substring on the visible label (and, once Ship B lands, on section titles too), with a live-region count. It rides the existing drawer focus trap; the input becomes the trap's initial focus target instead of the active chapter row, so opening the drawer and typing is one gesture. Reader-local, no persistence, no config.

**Cheaper alternative to consider first:** Ship B of `book-outline-artifact` plus Cmd-K arguably subsume it, and adding a second search-like box next to a Search button is a real discoverability smell. Decide between them rather than shipping both.

**Value** low at 19 chapters, medium at 60. **Effort** small. **Risk** low.

**Corpus pin.** `grow-tarn`: assert the input is emitted server-side (so it exists with JS off, inert) and that every chapter row carries a filterable text label. **Default-on for books with more than N chapters, or unconditional; do not add a key.**

---

#### chapter-cost-signal: per-chapter length where the decision is made

**Problem.** See gap 9.

**How it works.** Roll a per-chapter prose length into `BookEntry` (an additive struct that has grown a field before, for `draft`), rendered as **text** (words, or "~N min") in the book drawer and the landing Contents row. `prose::word_count` already exists, is markdown-aware (skips front matter, fences, `:::` lines, inline code/math/links) and is include-expanded; make it `pub` (see the central ownership note above). `chapter_heading` already does a `read_to_string` per chapter at discovery, so the cost is near-zero, but it must resolve `{{< include >}}` there or include-built chapters under-count.

**Cut from the original:** the per-section length bars in the TOC, and the unitless proportional-bar form. A bar normalised per page at section level and per book at chapter level means a full-width bar is 80 words in one row and 4,077 in another, which is misleading at exactly the moment it claims to help; and it discards the units that make the number budgetable while inheriting the identical prose-only bias. Every shipped precedent (Kindle "time left in chapter", O'Reilly, Medium) uses absolute units. Also note honestly: `word_count` excludes code and math, so a code-heavy chapter is *understated*; label it prose length or fold in a code-extent term.

**Do not** change `is_article` or add a page-level badge. That gate also selects `og:type` article-vs-website and the `<article>` landmark, and its absence on undated pages is test-pinned (`tech_blog.rs:374-392`).

**Value** medium. **Effort** small. **Risk** low. Zero keys.

**Corpus pin.** `corpus/demo-book` with a deliberately short chapter, a long one, and a code-heavy one (asserting fences and math are excluded). **Default-on.**

---

#### book-breadcrumb: static "Part, Chapter" under the topbar

**Problem.** A reader arriving mid-book from Cmd-K or a shared link has no idea where they landed. The topbar shows the *book* title; the part name renders only as a drawer divider inside a `hidden` drawer.

**How it works.** A compact non-animated line of at most 2.5rem emitted server-side from `BookEntry`, correct with JS off. **Note the data model:** `BookEntry.part` is `Some` only on the synthetic header row; real chapter entries carry `part: None`, so derive the part by back-scanning `book.entries` to the nearest preceding `part: Some(_)`. Bump `--tali-nav-h` inside the same media query that shows the ribbon so `scroll-margin-top` stays honest (both `#TOC { top }` and `[data-block-id] { scroll-margin-top }` derive from that one variable, and `scrollspy-activation-line` reads the resolved value, so the two changes must land in a compatible order).

**Cut the live section leaf.** On desktop the sticky `#TOC` already highlights the current section via scrollspy; below 60rem `toc-sheet.js` already writes the current section title into a fixed `#tali-toc-cur` handle that never scrolls away. A leaf would be a third copy on desktop and is withheld by design on mobile. Also drop the "try `position: sticky` on the h2 first" path: blocks are flat siblings with no per-section wrapper, so sticky headings would pin and never release (this is a `section-extents` option (a) dependency, not a quick win).

**Value** medium (down from high: this is "cheap and mildly orienting", not transformational; the breadcrumb-specific literature reports low click-usage and null-to-marginal task-time effects, with location *awareness* as the honest benefit). **Effort** medium. **Risk** medium (it spends screening viewport, worst on the 900x1440 narrow-tall band).

**Note for the record.** `docs/superpowers/specs/2026-07-03-quarto-design-decisions-catalog.md:1429` (D114) chose "no breadcrumbs" while conceding they answer "where am I in the tree", and deferred to "derive from the book's real part/chapter hierarchy, pinned by a corpus doc" with a standing caution that any breadcrumb re-adds chrome the reading-first redesign stripped. This proposal is that derived version; it should be argued as a reversal, not as an unexamined gap.

**Corpus pin.** `corpus/demo-book` (already has `part: "Core"` and `toc: true`) plus `grow-tarn` for the multi-part case. **Default-on for books.**

---

#### heading-keystep: n/p move focus between headings

**Problem.** Arrow keys move a whole chapter; there is no way to step section by section inside a 4,000-word chapter without the mouse.

**How it works.** One new `code-enhance` fragment (`20-heading-keystep.js`, per the central assignment above) plus one line in `09-register.js` and one `include_str!` in the drift-locked concat. Server-side: emit `tabindex="-1"` on headings inside the existing heading + `DocFormat::Html` arm, after the id (deck `<h2 data-block-id=` must stay byte-identical), and add a `--tali-focus` ring for the stepped heading, since `base.css:183-192` deliberately scopes the ring away from programmatically-focused `tabindex="-1"` elements. Moving **focus** rather than merely scrolling is the load-bearing detail.

**Blast radius, stated honestly.** Emitting `tabindex="-1"` on every heading in every HTML document changes the emitted HTML of every heading block across the entire corpus, so expect a **large blessed-snapshot diff** in `body_html_snapshots`, not the localised churn the "Effort small" rating suggests. Second, the same open tag feeds `search.rs::headings_with_pos`, which parses the raw `<hN …>` open tag by string-scanning for `id="`; confirm (do not assume) that it still extracts the id with the extra attribute present, and add a test if it does not already have one.

**Rescope the justification.** The WebAIM 71.6% figure is about *screen-reader* users navigating with their AT's own H key in browse mode, where the keystroke never reaches page JS, and `n`/`p` are themselves NVDA/JAWS quick-nav letters (and Vimium binds both). Justify this on sighted keyboard-only and low-vision/zoomed readers, and pick keys that do not collide. Note also a real adjacent hole this partly works around: `#TOC` is emitted *after* `</main>` in DOM order with only a "Skip to content" skip link, so reaching the TOC by keyboard means tabbing the whole chapter; a second skip target may be the cheaper fix.

**Value** medium. **Effort** small to medium (the snapshot re-bless is the bulk of it). **Risk** low. Rides the existing WCAG 2.1.4 shortcuts switch, so it adds no knob.

**Corpus pin.** `corpus/reader/long-read.tmd` plus `corpus/demo-book`, asserting HTML headings carry `tabindex="-1"`, deck heading bytes are unchanged, `headings_with_pos` still resolves ids, and the cheatsheet self-censors the keys when a page has fewer than two headings. **Default-on.**

---

#### book-resume: book-scoped reading position

**Problem.** See gap 16.

**How it works.** One book-scoped record `{url, blockId, frac, ts}` (max by `ts`, oldest-page eviction) aggregated from the existing `tali-pos:<path>`, surfaced as an inert "Continue -> 7 Execution model" affordance on the book landing page. Derive the book scope from the resolved book-root URL (the drawer brand's `index.html` href), **never** from `book.title` / `archive_name`: a rename silently orphans all state and untitled books collide, which is precisely the data-loss class that got a previous feature cut. Resolution by `data-block-id` only, which is a content hash and so survives reflow and theme changes while correctly invalidating when the block's content changes; when the id is gone, fall back to the chapter and say so rather than guessing.

**Cut the per-chapter read ticks and the "8 of 19 chapters" counter.** Their input is `qmd-read:`, a forward-only scroll high-water mark whose own source comment notes that a TOC click or resume jump marks every skipped section read. Pressing End marks a whole chapter. Worse, it self-poisons: clicking the Resume pill scroll-jumps forward and inflates the count. Kindle, Kobo and Pocket track *position* and never assert "read"; Coursera checkmarks and GitHub's PR "Viewed" come from an explicit action. If a completion ledger is ever wanted it needs an explicit "mark read" control, not scroll inference.

**Value** medium. **Effort** medium. **Risk** low.

**Invariants.** The crisp single-editing-surface case: reader-local `localStorage`, no backend, no sync, no account, nothing written to source. Explicitly scoped away from the deleted annotation cluster (commit `882addd`): no highlights, no bookmarks, no notes.

**Corpus pin.** `grow-tarn`'s landing page, asserting the Continue slot is emitted inert with no stored state, and that the drawer carries a stable book identity (`data-tali-book` on the sidebar nav, since an untitled book emits no brand link at all). **Reader-local, always on and inert.**

---

### Cluster: author tooling

#### preview-skim-signals: put the structural feedback where the author already is

**Problem.** Question 1 of this audit's brief was "how do you build a tool that helps writers create skimmable documents", and every other author-facing answer here is a CLI lint or a CLI projection. The live preview is the author's actual working surface and the one place feedback arrives while the prose is still warm, and it gets nothing. The audit also records two facts that make this pointed: word count and reading time are *already computed* in the preview dev panel and never surfaced usefully, and the preview's TOC lists different headings than the built page (gap 14), so the author is tuning navigation against a view readers never see.

**How it works, three pieces, all read-only.**

1. **Fix the preview/build TOC divergence.** Widen `client.js:847`'s selector from `h1[id], h2[id], h3[id]` to `h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]` and reuse the existing `base = Math.min(...)` math, so the client filter matches `render::toc_items`' `level - base <= 2` window. The `base` computation is already correct and must not be touched. Alternative, if a divergence is preferred: record the ruling. Do not leave it measured and unaddressed.
2. **Surface the shape numbers in the dev panel.** Per-section prose length and the page's longest unbroken prose run, both already derivable from `prose::word_count` plus the block model, next to the existing word count. No new UI surface, no new keys.
3. **A preview-only "skim view" toggle** rendering the exact projection `taliesin-skim` prints (headings, ledes, captions, callout titles, theorem statements), as an overlay panel beside the preview rather than a transformation of it. This is a *display* of a read-only projection: it navigates and shows, it never writes, so invariant 1 is untouched. It is also the fastest possible feedback loop for "do my headings carry the argument", which is the one question the whole audit says decides skimmability.

**Value** high for the author loop, and it is the only recommendation that answers the brief's authoring half at the point of authoring. **Effort** small (1), small (2), medium (3). **Risk** low.

**Invariants.** Preview-side, read-only, no source writes, no config.

**Corpus pin.** Piece 1 is pinnable in Rust: assert that for a title-demoted corpus page, the set of headings `render::toc_items` returns matches the set the client selector would return (encode the selector's level set as a constant both sides share, or assert the client file contains the widened selector, in the same style as the existing `TOC_SHEET_JS.contains(...)` assertion at `render/tests.rs:711`). Pieces 2 and 3 are preview-only and pin through `taliesin-skim`'s own golden output. **Default-on (1, 2); toggle, off by default (3).**

---

#### skim-suggestion-severity: a third severity that reports without gating

**Problem.** `check.rs:806` exits non-zero on ANY diagnostic; `codes.rs:12-13` states plainly that "check still exits non-zero on ANY diagnostic regardless of severity". Every structural lint would turn a green gate red for advice.

**How it works, corrected.** Severity-based gating **already partly exists** and the earlier draft did not name the function that has to change. `crates/server/src/check.rs:818` defines `fn at_severity_floor(diags: Vec<Diagnostic>, errors_only: bool)`, filtering on `codes::ERROR` for both the printed output and the exit decision, driven by the DX18 `--errors-only` flag; `notes/AUDITS.md:463-465` records the deliberate scope call that folded `--min-severity` into it "because there are exactly two severities today".

So:

1. Add `pub const SUGGESTION` beside `ERROR`/`WARNING` in `codes.rs:14-15`.
2. Turn `at_severity_floor` from a boolean into a **three-state floor**: errors only / errors+warnings / everything. `--errors-only` keeps its exact current meaning (ERROR only). The **exit decision's** default floor moves from "everything" to "errors+warnings"; the **printed output's** default floor stays "everything", so suggestions are always visible and never gating.
3. Teach `human_summary` a third bucket.
4. **Widen beyond `check`:** `build.rs:649` and `:1102` both do a bare `problems += statics.len()` from the same `page_static_diagnostics`, and `publish.rs:58` is strict by default, so all three gates must learn the split or a default-on suggestion still fails `--strict` and blocks publish.
5. LSP mapping is free (`check.rs:100-103` already has `"info" => INFORMATION` with a `_ => HINT` catch-all).

Precedent for advisory tiers: Vale ships prose rules at `level: suggestion`; rustc has note/help.

**Drop the symmetric `--suggestions` gate flag** from v1: non-gating with no new knob is the doctrine-pure default.

**Value** high (it is the precondition for the lint family). **Effort** small. **Risk** low.

**Important prior.** The owner already SKIPPED TODO/FIXME surfacing on 2026-07-10 for exactly this blocker ("no `level` concept exists, so a TODO warning would fail `check` on every draft"), preferring a preview-only `Diagnostic::info` over "re-plumbing a real level". That ruling was scoped to TODO surfacing and is outweighed by `notes/AUDITS.md:463-465` ("Noted for a future third severity"), but it should be answered, not ignored.

**Corpus pin.** Ship bundled with at least one real SUGGESTION-level code so `corpus/diagnostics/check-superset.tmd` has something to trip; a severity tier with no diagnostics at it changes no golden output. **Extend the existing DX18 exit-code tests in `check_cli.rs`** (which pin `--errors-only` behaviour) with the three-state cases: default exits 0 with suggestions present, `--strict` still exits 0 with only suggestions, and a warning still exits non-zero. **Default-on.**

---

#### skim-shape-lints: structural diagnostics, heavily trimmed

**Problem.** See gap 12.

**Ship only the threshold-free, binary rules**, as one ordinary `TAL-*` family after suggestion-severity lands:

- **HEADING**: exact-duplicate headings on a page, empty headings, contentless headings (a heading immediately followed by another with no content between), a subheading echoing the page title, near-duplicate first-two-words across siblings (stop-word guarded). Google's developer style guide is verbatim on unique headings and empty headings; a heading appears alone in a screen-reader rotor and in search results.
- **CAPTION**: empty, label-only ("Figure 3", "Results"), or an uncaptioned float that a cross-reference points at. Keshav's second pass is figure-driven; a caption is a free second heading level.
- **NO-DESC**: a chapter with neither a `description:` nor a derivable gist, which degrades the landing Contents to a bare title. **This rule depends on `section-gist`** and cannot ship before it; without the gist half it degenerates into "you did not set `description:`", which fires on 100% of the dogfood corpus and is noise.
- **TOC-DROP**: a heading the shared `toc_items` filter silently discards. Requires making `render::toc_items` `pub(crate)`/`pub`; note its filter is *relative* (`level - base <= 2`), so it fires rarely on title-demoted pages. Natural home for the `fix-book-section-numbers` residual too: a chapter whose first heading is deeper than its shallowest heading will print a zero segment, and that is exactly a TOC-DROP-adjacent structural smell.

**Cut RUN, DENSITY, EMPHASIS, FANOUT, SKELETON and FORWARD.** Measured against the corpus, none has a defensible threshold: the flagship RUN rule fires on exactly **one** page across all 36 dogfood pages (429 words in `using/reading.tmd`) and that page opens two of its paragraphs with bolded run-in lead-ins, i.e. the one firing is a false positive; the headline "1,832-word run" in `cli.tmd` is 1,021 words of table cells plus code, and tables are one of the strongest scan landmarks in the very study being cited. FANOUT's >9-siblings rule is the only high-volume one (8 hits) and its basis is wrong twice over (Cowan's ~4 is working memory, not a visible-list limit; Horn's 9 is house style, not a finding), and "split your 12-section CLI reference" would make it worse. SKELETON is undefined at the real N: `docs/guide/reference/` holds 7 siblings with pairwise-disjoint h2 sets and one with no headings at all. FORWARD is self-admittedly near-silent on today's sparsely-labelled books.

**Also cut:** anything resembling a readability grade, and any rule about heading *form*.

**How it works.** `crates/core/src/diagnostics/skim.rs`, validators shaped `fn(&[Block], ..) -> Vec<Warning>`, added by one `out.extend(...)` in `check::page_static_diagnostics` (the single check-superset definition, which reaches check, `build --strict`, publish, the preview panel and the LSP squiggle at once). Two rows each in the drift-locked `codes.rs` TABLE and EXPLANATIONS, which regenerates `docs/DIAGNOSTICS.md`. Beware: `classify` matches by *ordered substring* with needles as generic as `("category ", …)` and `("math", …)`, so each new code needs a non-colliding needle plus a `check_cli.rs` message pin, or a reworded message silently becomes a gating error.

**Value** medium (down from "flagship"). **Effort** medium. **Risk** low.

**Invariants.** The finding lands in the CLI or editor and the AUTHOR edits the `.tmd`: no preview gesture, no auto-fix. Zero YAML.

**Corpus pin.** `corpus/diagnostics/skim-shape.tmd` tripping each surviving code exactly once, plus `skim-shape-clean.tmd` as a well-shaped control asserted to produce **zero** skim diagnostics so the rules cannot pass vacuously. Asserted through `check --format json` by code and line. **Default-on** (as suggestions).

---

#### link-text-self-describing: fix the builder's own links, lint the collisions

**Ship the default-output half as the feature.** `backlinks.rs::render_backrefs_line` currently emits only "Referenced by <page title>". Store and render the sentence containing the reference alongside the title, as **visible** muted text **adjacent** to the anchor, never inside it (the citing sentence contains the reference's own `<a>`, and nested anchors are invalid HTML). Truncate, and fall back to title-only past two or three referrers so the quiet one-line whisper stays quiet. Obsidian and Roam linked-references and Semantic Scholar's "cited by with context" are the shipped precedent, and Pirolli and Card explain why: a bare title is a weak proximal cue, the citing sentence is the strongest available one.

**Ship only the collision rule as a lint**: two links on one page whose accessible name is identical but whose hrefs differ, compared **modulo fragment** so same-page deep links do not fire (measured: the naive version produces 4 findings on the dogfood books, 3 of them benign same-page-different-anchor links).

**Cut the "here"/"read more" stop-list from default-on.** It fires zero times across all 139 corpus + docs files, and `crates/core/src/prose.rs` is the project's existing style linter and is deliberately opt-in behind `prose-lint:`. If wanted, it belongs there.

**Cut the sr-only heading text on cross-page xrefs.** `.tali-sr-only` content survives tag-stripping, so it would inject duplicated heading titles into the Cmd-K index, `llms.txt` and the machine projections; it targets WCAG 2.4.9 (AAA) rather than the level-A criterion that inline context already satisfies; it adds stutter to linear screen-reader reading; and the author already ruled that section-heading links "carry no useful extra context beyond their title".

**Value** medium. **Effort** medium (the backlink index shape changes; a sentence extractor over rendered HTML is new; snapshot churn).

**Invariants.** Half is default-output with zero author action; half is an author-side lint. One trap to respect: do **not** use `aria-label` to replace a link's visible text (breaks WCAG 2.5.3 Label in Name for voice-control users).

**Corpus pin.** Extend `crates/core/tests/xref_backlinks.rs` / `corpus/demo-book` for the citing sentence (`corpus/diagnostics/` has no `_site.yml`, and backlinks are strictly cross-page, so a single `.tmd` cannot pin it), plus `corpus/diagnostics/link-text.tmd` for the collision rule. **Default-on.**

---

#### machine-shape-projections: give map, read and the LSP the shape

**Problem.** See gap 13.

**How it works.** Add `words` and a `headings` array (level, text, anchor, number, words) to `map --format json`, `read --json` and the MCP wrappers; set LSP `DocumentSymbol.detail` to the section word count. **Correct file pointers:** the typing to change is `lsp.rs:806` (`detail: None`) and `lsp.rs:809` (`kind: SymbolKind::STRING`) inside `to_document_symbol`, not `lsp_outline.rs`; what `lsp_outline.rs` supplies is the per-node `end_line` at `:16/:154/:182`. Grepping `lsp_outline.rs` for `SymbolKind` returns nothing, so an engineer following the old pointer would conclude the claim was stale and drop the item.

**Count from the markdown source using `lsp_outline`'s line extents**, not from `search::section_text` (which is `BODY_CAP`-truncated and counts code and math via `indexable_text`), so the "agrees with `prose::word_count`" assertion can actually hold. `prose::word_count` and a heading-extraction entry point both need to become public (see the central ownership note).

**Cut `gist` from this item** (no such helper exists yet; if `section-gist` lands, wire it later). **Defer** `longest_prose_run` until a consumer exists (`preview-skim-signals` is that consumer, so sequence them together if both land). **Defer** typed figure/table/theorem outline symbols to a separate change: `document_symbols` receives only `&HashMap<Url, String>` and the registry lives behind a full include-expanding `render_buffer`, so typing symbols would put a full render on the per-keystroke `documentSymbol` path and break `lsp_outline.rs`'s stated pure-scanner design. Cite that constraint once and reuse it.

**Un-gate `llms.txt` only, with two explicit link variants.** With `url:` set, keep absolute rows exactly as today (byte-identical). Without `url:`, emit **document-relative** rows (`using/formats.html`) plus a one-line header stating that the links are relative to this file. Rationale and its limit, stated honestly: root-relative would 404 because both books are mounted at `/docs/guide` and `/docs/internals`, so the no-`url:` case *is* the deploy-at-unknown-subpath case; but llms.txt's purpose is to be handed to an agent that may hold only the file contents, and a relative row is unresolvable in that case, so **this is a deliberate local deviation from the convention, not a reading of it.** The header line is what makes it honest. Leave `llms-full.txt` (a whole-book plaintext dump), `sitemap.xml` and `robots.txt` gated: the latter two require absolute URLs by protocol. Also correct the problem statement: the missing `<meta name="description">` is an unset `description:` key, not the `url:` gate.

**Value** medium. **Effort** small to medium. **Risk** low.

**Corpus pin.** `corpus/demo-book` for a `map --format json` golden asserting the heading tree and per-section word counts agree with `prose::word_count`; an LSP outline test asserting the long section's node reports its word count; a book with no `url:` asserting the relative `llms.txt` form plus its header line; and `corpus/tech-blog` (which sets `url:`) asserting the absolute form is byte-identical to today. **Default-on.**

---

#### taliesin-skim: the reader's-eye pass-1 projection

A read-only CLI projection printing exactly what a layer-cake skimmer sees: title, lede, every heading with its number, every caption, every callout title, every theorem statement, each section's first sentence, and nothing else.

**Problem.** An author has no way to test the one question that decides whether a 300-page book is usable: read alone, do the headings and captions carry the argument? Keshav's first pass *is* this projection, which gives "skimmable" a testable definition.

**How it works.** Put the collector in `taliesin-core` beside `render/text.rs`, exported publicly like `RenderedDoc::body_text()`, with `cmd_skim` and an MCP row as thin wrappers. (`indexable_text` is `pub(crate)`, `read` actually uses `body_text`, and `Block` exposes no semantic kind, so the server crate cannot do this itself without duplicating private helpers.) Use `scoped_site_doc` so a chapter projects with its book numbering, and `DraftMode::Include` to match `cmd_read_dir`.

**Critical design correction: always print the raw first sentence.** The original spec had a failed gist gate print a bare heading, which renders two opposite states identically (a genuinely weak section vs a heuristic misfire) and teaches the author to ignore bare headings, killing the instrument. Show any gate judgement as a visible annotation, never as suppression. It must also work standalone if `section-gist` never ships.

**Value** medium, but it is also the **evaluation instrument** for this whole audit (see "How you would know"), which is a stronger argument for it than its standalone utility. **Effort** medium. **Risk** low. Restate the problem narrowly: the LSP outline, the book TOC and the Cmd-K index already surface heading skeletons; what does not exist is headings + first sentence + captions + callout/theorem titles + per-section length as one linear stream across a whole multi-chapter book.

**Corpus pin.** `corpus/demo-book` + `grow-tarn` (both real books) plus one small fixture exercising the boundary classifier's edge cases (figcaption, callout title, theorem head, heading with no following prose). **Default-on** as a CLI verb.

---

### Cluster: authoring primitives (lower priority, opt-in)

#### summary-div: ::: {.summary}, harvested

The one place in this audit proposing real new syntax. Justification: the signal cannot be inferred (invariant 3 forbids an LLM; extractive summarization on prose is a correctness hazard; a generated blob presented as the author's summary is a lie in a tool whose premise is that the `.tmd` is truth).

**The strongest evidence for it is the advance-organizer literature, not DITA.** Instructor-provided graphic organizers measure g+ 0.53 comprehension / 0.70 memory, the largest effect of any text aid in this research set, and an authored chapter-opening summary is exactly that intervention. The DITA `<shortdesc>` precedent supplies the *reuse* design (one string, many consumers) and Hartley supplies the honest caveat (longer, more informative, no replicated retrieval-speed gain). Cite all three, and lead with the organizer effect.

**How it works.** One arm in `divs.rs::build_container` emitting `<aside class="tali-summary" role="doc-abstract">` with its own `data-block-id` and `data-sourcepos` (so click-to-source works for free), plus a build-time collector recording rel -> summary text. Precedence: authored `.summary` > front-matter `description:` > derived gist. **Never mandatory:** `check` must not error on absence.

**Trim the "five ways" to what is real.** `search.rs:101` already indexes the pre-first-heading intro section as the page snippet, so a top-of-chapter `.summary` is harvested into Cmd-K for free; `book_toc.rs` and `llms.rs` already have blurb slots. Only the hover card is genuinely new work (`hover.rs` keys anchors to defining blocks and excludes headings, so a page-level payload is a new index shape). Truncate the hover payload to the first sentence or the reader meets the identical paragraph three times.

**Value** medium. **Effort** medium. **Risk** low. **Corpus pin.** `grow-tarn` or `corpus/theorem-book`: assert `role="doc-abstract"`, that its text appears as that chapter's landing-Contents blurb, and that a sibling without one shows no blurb. **Opt-in** (it is content, not config).

Note the adoption evidence cuts *for* this: zero of 37 dogfood pages set `description:`, and only 19 of 102 corpus pages do, exclusively where a listing card makes it visible. The existing one-line affordance fails precisely because it has no in-page reader payoff.

---

#### glossary-autolink: define once, link every first mention

**Ship v1 with ZERO new syntax.** Harvest the already-registered, already-hover-indexed titled `::: {.definition}` theorem environments as the term registry, auto-link the first mention per section, and generate the A-Z page from those. Defer `::: {.glossary}` until the definition-env path is demonstrably insufficient. (Comrak's `description_lists` extension is off, so "an ordinary definition list" is not free syntax anyway: enabling it re-parses every existing corpus paragraph beginning with `:`.)

**Two corrections to the reader surface.** (1) Make **every** occurrence a hover/focus target and give only the first per section the visible dotted underline. "First occurrence per section only" is the opposite of what ScholarPhi did (its contribution was *position-sensitive* definitions at every occurrence) and it removes the affordance at exactly the moment the feature exists for: a reader landing mid-book is statistically not looking at a section's first occurrence. Decoration density solves MOS:OVERLINK; hover-target density serves the reader. (2) Specify the matcher properly: word-boundary required, term plus naive plural, case-insensitive only at sentence-initial position, terms >= 4 chars, and a hard cap of about 30 registered terms per book. Measured on `docs/internals`: "block" appears 289 times singular and 75 plural, and a substring matcher wrongly hits `blocks`, `BlockOp`, `blockquote`, `blocking`.

**Also:** drop "Used in" (unbounded by construction; the terms most worth glossarying get the longest walls of links), or cap it at 5 sections. The hover snippet needs a new sub-block dt/dd extractor: `extract_snippet` is block-granular and would return the entire glossary for every term.

**Mechanism honesty.** The autolink pass is NEW prose-scanning machinery at the `finish_blocks` stage, not an existing seam: `resolve_blocks` only rewrites blocks already containing a `data-qmd-xref=` marker, and `attach_backlinks`/`attach_book_toc` splice whole new blocks. It must extend the quote-aware, depth-tracked `<math>`-skipping discipline already proven in `strip_tags_inner` (KaTeX output embeds raw TeX as text; attribute values contain `>`), plus skip `<pre>`/`<code>`, existing `<a>`, and headings, and match against HTML-escaped text.

**Invalidation honesty (the part the earlier draft missed entirely).** The term registry is a **cross-page input to a per-page render**, and block ids hash *source*. That stability, which is a virtue everywhere else, is the problem here: adding a definition in chapter 2 must re-link every later mention in chapter 7, but chapter 7's blocks are byte-identical in source, so their ids do not change and nothing signals the diff to re-emit them. The block diff itself is safe (`diff.rs::anchor_op` falls through to a full `Update` whenever the html changes), so the failure mode is **stale-until-restart**, not a missed op. Two acceptable resolutions, and the item must pick one before it is scheduled:

- **(a)** A `refresh_glossary_for_page`-style invalidation modelled on `Site::refresh_search_for_page`: when the definition set changes, dirty every page carrying a mention and re-render those. This is the correct behaviour and the cost is a whole-project re-render on a definition edit, which in the preview is the expensive case.
- **(b)** Declare that preview autolinks refresh only on server restart, and say so in the user-facing docs.

The same class of issue, in a milder form, applies to `book-outline-artifact` Ship B (a per-page fragment must invalidate when a *different* page's headings change), which is why that item names `refresh_search_for_page` as its precedent. Raise this item's effort accordingly, or downgrade it.

**Value** medium-high. **Effort** medium-large (large if resolution (a)). **Risk** medium (the matcher plus the invalidation; block ids are safe since they hash source).

**Corpus pin.** `grow-tarn`'s definition blocks plus a `glossary.tmd` chapter **registered in its `_site.yml` `chapters:` list in the same change**: three terms (one multi-word, one also appearing inside a code span, one appearing in a later chapter's heading) with mentions in two later chapters; assert exactly one *decorated* link per term per section, no link inside the code span or the heading, and that the generated page lists every term. **Default-on** once the matcher is calibrated.

---

#### book-term-index: structure-derived defined-term index

**Heavily trimmed.** Ship only the structure-derived index: entries from `theorem_meta` kinds (definition/theorem/…), `{#sec-}`/`{#fig-}` xref anchors and figure captions. Emit as **`terms.html`, never `index.html`** (which is every book's landing page, verified in all 10 corpus site fixtures), and only when the book yields more than a handful of terms.

**Cut from v1:** the mention-count integer and the slide-over mention panel (literal matching misses morphology and over-matches short terms; a displayed integer reads as authoritative and will be wrong in both directions; `search.js` already gives section-ranked results plus `flashTermsIn`); the "concepts discussed in 3+ chapters but never defined" report check (undeliverable without the keyphrase extractor the proposal itself bans); the MCP tool; and the `;`-subentry / `see=` / `to=` grammar (Sphinx mimicry, each a knob before the default is proven).

**Defer** the `{{< index >}}` escape hatch plus its LSP code action to a follow-up once the derived half is proven. It is the only thing extraction provably cannot do (posting under a term the prose never uses), so it earns its place eventually, but it must be inserted by an editor command, never a preview gesture.

**Be honest about scope.** `docs/internals` is 60,208 words with zero definition/theorem divs and 5 `{#sec-}` anchors: structure-only sourcing yields about 9 entries where Wu et al.'s 0.42% yardstick implies about 250. The flagship dogfood book gets no index until it grows definition blocks. That is acceptable; pretending otherwise is not, and it is the same content-not-code point the Author playbook makes.

**Value** medium. **Effort** large. **Risk** medium. **Corpus pin.** `grow-tarn` and `corpus/theorem-book` (which already carry `{.definition}`/`{.theorem}` blocks with ids). **Default-on** where terms exist. Sequence after `glossary-autolink`, which supplies the densest registry.

---

#### float-digest: List of Figures / Tables / Theorems for a book

Raised by three research lenses and by the repo's own FEATURE-IDEAS, then lost between consolidation and output in the first draft. Recorded here so it is decided rather than rediscovered.

**Problem.** Captions are read far out of proportion to their length (the ~20% of words that get seen skews to headings and captions), and a caption index is a high-scent, low-cost patch that often answers a question without entering the chapter. LaTeX `\listoffigures` and O'Reilly front matter are the shipped precedent.

**How it works.** A generated page assembled from the registry that already exists: `xref.rs` holds anchor -> url + number + kind, and `figure.rs` owns numbered figures and captions. One page per book listing every numbered float in reading order with its number, caption and jump link, grouped by kind. Emit as `floats.html` (never `index.html`), and only when the book yields more than a handful of entries. It is the same generated-block-plus-generated-page shape as `book_toc.rs`.

**Honest reason it might not be worth it now:** the dogfood books contain almost no numbered floats, so the page would be near-empty, exactly as with `book-term-index`. If `grow-tarn` does not add floats, this stays parked.

**Value** low now, medium once a book has floats. **Effort** small (the registry is the hard part and it exists). **Risk** low.

**Corpus pin.** `corpus/theorem-book` or `grow-tarn`: assert every numbered float appears once with its correct number and a resolvable anchor, and that a book with fewer than the threshold emits no page. **Default-on where floats exist.**

---

### Cluster: reader-local (speculative)

#### reading-density-dial: fold prose to headings, on demand

**Cut to two states, not three.** A reader-local, off-by-default "Fold prose" toggle in the existing Settings popover: Full (today's page) and Folded (headings, gists, captions, callout titles kept; prose bodies folded in place with scroll position preserved). Drop Outline as a third level: measured across the built guide, 12 of 15 pages have zero `<figure>`, callouts run 0-5 with zero on 6 pages, and real theorem markup is zero, so Skim and Outline render identically on most real pages.

**Drop the kind-lens chips entirely.** "Dim non-matching blocks to near-invisible while keeping headings at full contrast" is deliberately shipping sub-threshold text contrast, in a project that advertises WCAG compliance; dimming-not-removing preserves the a11y tree while producing text a low-vision reader cannot read but a screen reader still announces.

**Hard prerequisites, all real.** (1) `hidden="until-found"` cannot be set from CSS, so this needs JS stamping per block and **re-stamping after every incremental-diff block replacement**. (2) It needs section extents, which do not exist: `Block` is flat with no children field. `section-extents` option (b) is not enough for a CSS-only fold; this is the one dependent proposal that actually needs option (a), the container. (3) It needs `section-gist` to be good enough that a folded section reads as more than its heading. (4) `find-in-page-honesty` must land first, or folded prose becomes unfindable and the page is strictly worse than today.

**Correct the premise.** The `problem` statement's "side rail that shows 2 of 8 entries" reproduces on exactly one page; across all 16 built pages 6 show 100% of their TOC entries at load and the median is about 5 of 7, and the entries hidden at load are precisely the h3s a fold view would not list either.

**Value** moderate (down from transformational). **Effort** very large with prerequisites. **Risk** medium. **Reader-local preference, off by default.** **Corpus pin.** `corpus/reader/density.tmd` plus `grow-tarn`'s longest structure-rich chapter, asserting every level keeps all text in the DOM, marks folded regions `hidden="until-found"`, and never removes a block.

**Recommendation: do not schedule this yet.** It is the item most likely to look impressive and land badly.

---

#### changed-since: "what changed since you last read this"

**Pick one pitch and keep it.** The earlier draft opened by claiming this is "the capability the block model uniquely enables and no fuzzy-anchoring competitor can offer offline" and then, three paragraphs later, prescribed computing the reader-side digest from normalised `textContent` rather than `data-block-id`. Those cannot both be true: if the digest is textContent-based, any renderer with a stable DOM could do the same and the uniqueness claim is false.

**Resolution: keep the textContent digest and drop the uniqueness claim.** The reason is empirical, not rhetorical. `make_id` hashes the block's *source*, so a theme or highlighting change already cannot perturb an id (the "mitigation" the earlier draft proposed is the status quo), and ids are already neighbour-stable (editing a callout body changed exactly 1 of 15 ids). The real noise runs the other way: re-wrapping a paragraph across lines, reindenting, or renaming a `#| label:` changes the id while the rendered paragraph is byte-identical, which is exactly the case a reader must not be told about. So the digest must be content-of-render, not id-of-source. That makes this an ordinary reader-local nicety, and given the item's own "no evidence readers benefit" verdict, that demotes it further.

**Scope it to section/page granularity**, not per-block marks. Reuse the existing `qmd-read:<path>` heading-id set to answer "this page has new or changed material since you read it", mark whole sections in the TOC/drawer, count new sections as changes when the reader reached the page bottom (the dogfood git history is overwhelmingly pure addition: +43/0, +65/0, +112/0, +219/0), treat a changed heading id as itself a change signal rather than an un-read (rewriting a section usually touches its heading, which would otherwise drop it from the read set and suppress the marks on exactly the case the feature is for), and **suppress the banner entirely when more than about 40% of a page's blocks changed** so a rename sweep never lights the whole book (measured: `df394c6` is +34/-34 on one guide page).

**Value** low and explicitly **speculative**: no study was found showing that surfacing change marks improves comprehension or navigation, and the shipped-product precedents (GitHub, Wikipedia, Notion) are all backend-mediated authoring contexts. Note also that WebKit's ITP caps script-writable storage at about 7 days without interaction, so the long-gap returning reader often has no baseline. The cheap alternative the docs market converged on is a build-time per-page "Last updated" line.

**Effort** medium. **Risk** medium. **Corpus pin.** Split the substrate pin out and land it regardless: a test asserting a one-word edit changes exactly that block's id and no other's. **Reader-local, off by default until dogfooded.**

---

#### read-aloud: the verdict the annotation rule-out deferred

The annotation rule-out below explicitly carves read-aloud out of its own ruling ("must be judged on an accessibility lens") and the earlier draft then never judged it, leaving a reader unable to tell whether it is ruled out, open, or recommended. Judging it here.

**Facts.** It was deleted in the same commit as the annotation cluster (`882addd`, 2026-07-02, `05-read-aloud.js`, 389 lines), so its removal was a bundling decision, not a considered a11y ruling. Unlike highlights and bookmarks it keeps **no store**, has **no anchoring problem**, needs **no backend**, and sits squarely inside invariant 5's reader-local accessibility exemption. The Web Speech API is a browser primitive, so there is no network fetch. The `notes/FEATURE-IDEAS.md` entry that still marks it "SHIPPED 2026-06-26" with a pin at `corpus/reader/read-aloud.qmd` is stale: grep for `speechSynthesis` returns zero and the file does not exist.

**Verdict: leave it out, on cost rather than on principle, and record that reason.** It is a genuine accessibility affordance with no invariant conflict, but (a) `speechSynthesis` voice quality and availability are wildly inconsistent across platforms, so the default experience is not controllable from the build; (b) OS-level and browser-level reader modes already provide it for any well-structured page, and this audit's whole thesis is that improving the structure improves those tools for free; (c) it does nothing for skimming, which is the brief. If it is ever revived it should be argued on an accessibility lens with a named user need, not folded back into a reading-features bundle. **Not scheduled. Not ruled out on principle.**

---

## Author playbook for an existing book

Every recommendation above is Rust, CSS or JS work on the tool. **Roughly half the skimmability problem in the dogfood books is content, and no code change can perform this pass.** The measurements are unambiguous: 0 of 37 pages set `description:`, 8 cross-reference links exist across 19 chapters, 0 backlink lines render, `docs/internals` has 60,208 words and 0 definition blocks. This is the ordered pass an author runs against their own 150k-word manuscript, with an honest note on which steps pay off with today's binary and which wait on a recommendation.

**Step 1: read the projection, not the pages.** Run `taliesin read <dir>` today (or `taliesin-skim` once it ships) and read the output alone. Do the headings, in sequence, carry the argument? This is Keshav's first pass, and it is the only test that tells you whether your book is skimmable before a reader tries. *Works today, better after `taliesin-skim` and `preview-skim-signals`.*

**Step 2: write a `description:` for every chapter.** Measured payoff with today's binary: the book landing Contents goes from 18 bare titles to 18 blurbs, and every chapter page gains a `<meta name="description">`, an og/twitter description, and a JSON-LD description. This is the single highest content-to-payoff ratio action available and it costs one sentence per chapter. *Works today. `section-gist` later provides a fallback for chapters you never get to, but an authored line beats a derived one every time.*

**Step 3: split or head the headingless preambles.** The audit located them: `reference/cli.tmd` opens with 1,021 words of table cells and prose before its first heading, which means it has no TOC entry, no anchor, no permalink, and only its first 1,500 characters in the search index. A heading in front of each such run fixes all four at once. *Works today; `skim-shape-lints`' TOC-DROP rule will point at them for you.*

**Step 4: label the sections other chapters actually refer to.** Add `{#sec-…}` to the headings you find yourself saying "as discussed in chapter 4" about, and write `@sec-…` at each mention. This is what turns on cross-reference numbering, cross-page hover previews, and the "Referenced by" backlink line, all three of which currently render nothing in either book because the anchors do not exist. Aim for the sections a reader would want to jump *back* to, not every section. *Works today; `link-text-self-describing` makes the backlink line worth reading.*

**Step 5: write `::: {.definition}` blocks for your load-bearing vocabulary.** Cap it around 30 terms for a whole book. This is the registry that `glossary-autolink` and `book-term-index` both consume, and until it exists both features have nothing to index. It is also independently useful today: definitions are numbered, anchored, cross-referenceable and hover-previewable. *Partly pays off today; fully pays off after `glossary-autolink`.*

**Step 6: put a `::: {.summary}` at the top of each long chapter.** *Waits on `summary-div`.* Of everything on this list it has the strongest experimental warrant (instructor-provided advance organizers, g+ 0.53 comprehension / 0.70 memory) and the least existing adoption evidence, which is exactly the tension open question 5 asks you to resolve.

**What this pass does not do.** It does not fix the section numbers, the truncated index, the invisible tab text, the missing whole-book outline, or the silently-deleted nested parts. Those are tool work and they are session 1 through 3 below.

## How you would know

Twenty-nine recommendations and no success criterion is a plan without a test. These are the properties this audit already measured, their current dogfood values at `5c25d00`, a target, and the instrument that produces the number on demand. Nothing here requires a new metrics subsystem: every number falls out of `taliesin-skim` plus `machine-shape-projections`' per-section word counts.

| Property | Current (measured) | Target after sessions 1-3 | Instrument |
| --- | --- | --- | --- |
| Chapters emitting a spurious section-number zero | 31 of 32 | 0, except headings deeper than their chapter's shallowest (documented residual) | build + `grep 'tali-section-number">[0-9]*\.0'` |
| Indexed-text coverage | 84% guide / 85% internals; 18.3% / 25.3% of sections truncated | 100% of sections, 0 truncated | index parse (`BODY_CAP` deleted) |
| Chapters with whole-book search wiring | 18 of 19 (guide) | 19 of 19, and every corpus book page | `grep -c TALIESIN_SEARCH_URL` per page |
| Nested-part chapters silently dropped | all of them, `check` exits 0 | 0, or a located warning | `grow-tarn` build + `check --format json` |
| Words per heading, worst chapter | 630 (`internals/architecture.tmd`) | author-set; the instrument is the point, not a threshold | `machine-shape-projections` per-section words |
| Chapters with a `description:` or derivable gist | 0 of 37 | 37 of 37 (playbook step 2 + `section-gist`) | `taliesin map --format json` |
| Cross-reference links per book | 8 across 19 chapters | author-set (playbook step 4) | `taliesin map` xref count |
| Whole-book section outline reachable without typing | no surface | Cmd-K empty state (Ship A), drawer (Ship B) | manual + Ship A's producer-side pin |

Two of these (words per heading, xref density) deliberately have no numeric target. The research is explicit that no defensible threshold exists for either, which is why `skim-shape-lints` cut every threshold rule. What the instruments buy is the ability to *see* the number for a 300-page manuscript at all, which is currently impossible.

## Ruled out

Blunt, so a future session does not re-derive these.

### Killed by verification

**`section-hover-preview` (lift the heading exclusion from hover cards).** KILLED. This was built, shipped, and deliberately deleted 13 days ago: commit `318f22f` (2026-07-11), "feat(site): drop hover preview for section-heading links", with the stated reason "It added no context beyond the heading title already in the link, and was noise while reading a book." The deleted version already showed heading + first lines on both paths (client `buildPreview` appended up to two following blocks; `hover.rs::extract_snippet` did the same server-side with `number_chapter` applied first), so the proposal argues against a rationale that was never the real one. The exclusion is pinned by three tests (`corpus.rs:1104`, `tarn.rs:124`, `hover.rs:134`). **One real bug survives and should be fixed as a one-line docs edit:** `docs/guide/using/reading.tmd:68` still promises a hover card showing "the equation, the bibliography entry, or a heading with its first lines". Delete the substring `, or a heading with its first lines`. (Line 68, not 66 as the first draft said.)

**`toc-entry-budget` (replace the TOC depth cap with a total-entry budget).** KILLED. The motivating bug does not exist: `toc_items` computes `base` as the *minimum* heading level present, so the three-level window is relative and moves with demotion. Verified by rendering `reference/configuration.tmd` (a titled chapter with two source `####`): base = 3, and 12 of 12 headings are inside the window, including both `####`. Two tests already pin this (`toc_filter_is_relative_to_the_shallowest_heading`, `a_demoted_post_still_lists_all_its_sections_in_the_toc`). The proposed fold is also already the default (`base.css:725` `#TOC ul ul { display: none }` plus active-branch scrollspy expansion, i.e. Quarto's `toc-expand: 1`). Measured: zero documents across 129 heading-bearing `.tmd` lose a heading to the cap, and zero would exceed a 25-entry budget. Note the print-side consequence of that same rule is a real defect and is fixed by `print-toc-expand`.

**`margin-footnotes` (auto-promote short footnotes to the gutter).** KILLED. Three independent reasons. (1) Footnote references are already in-page fragment links, so the shipped hover card (`12-link-preview.js`) already previews them at all widths, with no length limit, with keyboard parity, pinned by `corpus/reader/hovercards.tmd`. (2) There are exactly TWO real footnotes in the entire repository (`docs/guide/using/writing.tmd:186` and `corpus/reader/hovercards.tmd:6`), both demo footnotes; there is no footnote-heavy document to rescue. (3) The proposal's own mitigation for the sticky-TOC collision ("suppress on `body.has-toc`") disables it on 8 of 13 site projects including both dogfooded books, i.e. on 100% of its target pages. **Salvage:** the underlying `.column-margin` vs sticky-TOC collision is a real open bug (`notes/AUDITS.md`) worth a two-line CSS fix on its own.

**`taliesin split` (split a chapter, rewrite `_site.yml`, repair references).** KILLED. (1) It cannot cut where the defect is: the motivating pathology in `cli.tmd` is a *headingless* preamble, and `--at sec-build` needs a heading; once headings exist, the drawer, TOC, scrollspy and Cmd-K already index them, so the skim win is banked before `split` runs. (2) "Expensive and unverifiable" is false: `docs/guide` has **zero** real `@sec-` cross-references and **zero** inbound `cli.html#anchor` links, so the flagship reference-repair capability would repair 0 references on the chapter it was designed for; and `check` already reports broken links, broken anchors and undefined xrefs. (3) The corpus pin is self-contradictory ("every `@`-ref resolves to the same displayed number" AND "chapter numbering renumbers consistently" cannot both hold, since section numbers are chapter-prefixed). (4) `_site.yml` is read with `serde_yaml` and every real config carries load-bearing comments, so a round-trip write destroys them. **Salvage:** a `check` lint for a long headingless preamble (folded into `skim-shape-lints`), and at most an LSP "extract section to new page" code action leaning on the existing `check` gate.

### Ruled out on evidence (record these so they stay ruled out)

**Reader-side highlighter, bookmarks, margin notes, annotation layer.** Built here and deleted at commit `882addd` (2026-07-02), removing about 1,350 lines plus five corpus documents, on the ruling that they "served a published audience, not the single author's dev loop", plus a real data-loss incident (the rename silently re-keyed `qmd-hl:`/`qmd-bm:`). Independently: learner-generated highlighting is comprehension-null (Ponce et al.); durable text-quote anchoring orphans ~27% (Aturban et al.); an annotation store is one product decision from a second write path. **The revival bar is the one the owner set: demand-driven, if the tool gains a published audience.** Read-aloud was deleted in the same commit and is judged separately above (verdict: not scheduled, on cost, not on principle).

**Document minimap, overview rail, structure map, reference graph.** The whole-site cross-reference graph was built and deleted at `6ff59b0` (2026-07-06), 583 deletions, recorded as a standing ruling. A scrollbar minimap is the same category: VS Code's minimap works because code has visual shape (indentation, block structure) that prose does not. Scope this ruling to the **reading view only**: the deck overview is a pannable/zoomable spatial map that ships and was deliberately kept by the same 2026-07-06 ruling, and the deck's corner minimap was separately cut (deck-audit C-CUT-3), making this a two-precedent rule. It does **not** cover a list-form structure panel (FEATURE-IDEAS #26, pin `corpus/layout/structure.tmd`), which stays open and is now the pin for `section-extents`. Note also that browser-native Ctrl-F already paints scrollbar tick marks for free, and the page already carries two ambient position signals.

**Per-chapter "N min read" badge.** The engagement evidence is unpublished vendor marketing; the Medium 7-minute result measures a different thing; `prose::word_count` deliberately excludes code and math, so the estimate systematically understates exactly the pages that take longest; and "45 min read" on a reference chapter plausibly causes the abandonment it aims to prevent. The dynamic "~N min left / X% read" readout was removed at `0feb565` (as part of stripping the whole Aa menu). *Correction for the record:* the static `tali-read-time` post badge survived that commit and still ships, pinned by `tech_blog.rs:373-395` and `corpus.rs:791-795`; leave it alone rather than extending it. The defensible version is per-chapter length in the drawer and Contents (`chapter-cost-signal`).

**An index extracted from prose by keyword extraction.** Wu et al.: an alphabetical list of every occurrence is a *concordance*, not an index, and it will look authoritative while being systematically wrong. Unsupervised keyphrase extraction tops out near F1@10 = 0.30. **Close the carve-out too:** allow an "index health" report only over deterministic structural facts (a marked term never referenced, duplicate markers, an entry exceeding N locators), never a suggested-terms list, or the ruled-out work re-enters as "just a `check` hint". Also note `notes/FEATURE-IDEAS.md:444` and `:552` say "auto index" without distinguishing structural aggregation from prose extraction; annotate those lines.

**Bionic Reading, fixation bolding, RSVP, and all content-blind auto-emphasis.** Three independent studies plus a 1,916-participant paired experiment, all null-to-negative, including one that specifically tested individual-difference moderators and found no benefit for any subgroup. **Scope the generalised rule carefully**, because stated absolutely it condemns three shipped features: the rule is *no content-blind mechanical emphasis of PROSE BODY TEXT absent a reader-supplied query*. Syntax highlighting and Cmd-K `<mark>`s are fine, the latter precisely because the reader's query supplies the isolation that makes signalling work. Also correct the citations before quoting them: Snell 2024 (Acta Psychologica) = null reading times, no eye tracking; Beelders 2025 = null fixation measures plus the mechanism kill; Spear et al. 2025 = costs vs unbolded plus no subgroup benefit. The GP-TSM-style deterministic de-emphasis of parentheticals falls under the same rule (their pipeline used LLM sentence compression; a Rust syntactic approximation is materially weaker, and a bad de-emphasis hides content the reader cannot tell is hidden).

**Generated TL;DRs, chapter abstracts and section summaries, at read time OR build time.** Read-time is dead on invariant 3. Build-time is dead on two grounds the proposal under-stated: byte-identical build output is an actively pinned contract (`build_reproducibility.rs`, `parallel_build_determinism.rs`), and invariant 3's bundling rule (`include_str!` into the binary) is unsatisfiable by model weights, so output would become machine-dependent. Also rule out making an authored `::: {.summary}` mandatory. The honest substitute is the gated first sentence.

**A `kind: how-to | reference | explanation` front-matter key.** Fails minimal-config on its own terms: its entire value proposition is "the author can now configure something". Note the repo already derives document type from content (`scholarly` is inferred from `has_bibliography`), so this would be a regression from an established pattern. **Also strike the three substitute lints** originally proposed here: the title-shape lint cannot separate a tutorial from a how-to on step density (Diataxis gives tutorials the same numbered-step shape and no title convention) and fires on zero real pages; the modal-h2-skeleton lint is undefined at the real sibling counts; the table-density split lint would target `reference/frontmatter.tmd` and make it *less* Ctrl-F-skimmable.

**Reader text-size / width / spacing controls.** These were built and deleted at `0feb565`, and the removal is recorded three ways: `notes/backlog.md` under "Decided against / do-not-re-litigate", `notes/backlog.md` "Reading-first defaults, research-validated keeps (do NOT fix)" naming `--tali-maxw: 46rem` explicitly, and `docs/guide/using/reading.tmd:46-49` as a published product promise. The proposal's technical premise is also false: `--tali-maxw` and `--tali-font-body` are both root-relative, so characters-per-line is invariant under zoom, and the deleted control was an `html { font-size: calc(...) }` root multiplier that scaled both in lockstep. The `ch` conversion would additionally make column width depend on webfont load state (`font-display: swap`) and desync two hardcoded `46rem` TOC grids. **If any of this is ever revived, the narrow defensible slice is a single Spacing detent at the WCAG 1.4.12 values** (letter spacing has real evidence; the rest does not), and it must restore the prose-scoped `code, pre, kbd, .katex { letter-spacing: normal }` reset that came with the deleted version.

**Cross-book search (federating `docs/guide` with `docs/internals`).** KILLED as designed. The headline example is false: the guide's own index contains "freeze" 15 times and "cache" 52 times across 7 pages with two dedicated headings, and the design self-defeats (neighbour indexes load only when local hits fall below N, so for its own example federation would never fire). The zero-config derivation is refuted by `corpus/`, where 11 unrelated projects share a parent; the `mounts:` fallback is directionally impossible (mounts live one level up, in the mounting project). **The real gap underneath is different and smaller:** Cmd-K on the marketing site finds nothing from either manual. The fix is for the *mounting* project to emit one unified index covering its own pages plus every mounted project's pages, tagged by mount prefix, sequenced after the build learns to build mounts into `<out>/<at>/`.

### Dropped during consolidation

- **`pullquote-div`.** Low value in a technical reference book, competes for the same emphasis budget and the same right gutter as margin notes, and creates a screen-reader duplicate-content problem requiring `aria-hidden` plus exclusion from search and machine projections.
- **`overview-rail`** folded into the minimap rule-out, with its strongest counter-argument recorded verbatim (ambient and zero-interaction, versus the deleted graph's modal and opt-in), so the ruling can be overturned on merits rather than rediscovered.
- **`gist-fade`** folded into the word-level-rendering rule-out.
- About 65 further raw entries were near-duplicates and were merged. The largest collapses: 6 independent proposals of `hidden="until-found"`, 5 of the section-number defect, 5 of per-section length at the decision point, 5 of the whole-book outline, 4 of section hover previews, 4 of the derived first sentence, and 13 separate lint proposals into one family.

## Sequencing

The smallest slice a reader would actually feel is **session 1 alone**: correct numbers, honest search, findable tabs, and chapters that stop disappearing.

**Session 0: the corpus and the notes.** `grow-tarn` (12 chapters, 3 parts, one nested group, one below-gate chapter, one `###`-rooted titled chapter, one titled chapter with a body `# H1`, one over-`BODY_CAP` section, two `{.definition}` blocks, one unnumbered appendix). Bundled with it, one housekeeping commit reconciling the notes, whose scope is already enumerated: mark `notes/FEATURE-IDEAS.md` items #9 (read-aloud, falsely "SHIPPED", pin file does not exist), #4, #5, #6, #7, #10, moonshots 1 and 3, and the line 575 "Big bets" entry as removed; annotate `:444` and `:552` to distinguish structural aggregation from prose extraction; and add `notes/backlog.md` "Decided against" lines for `079a30d` (Ask-AI selection popover) and `318f22f` (section hover previews). This is not optional bookkeeping: the notes are stale enough that a future session will rediscover deleted features as gaps, and this audit already tripped over that once.

**Session 1: the defects.** `fix-book-section-numbers` (base threaded through all three numbering sites, with the four-case lockstep pin), `fix-nested-parts` (plus line numbers on `_site.yml` diagnostics), `search-on-every-page` (split `toc_scripts`, fix `render/tests.rs:707`), `scrollspy-activation-line` (computed `scroll-margin-top`, sampled in `collect()`, re-sampled on a debounced resize), `heading-contrast` (drop the muted h5/h6, pin both), `print-toc-expand` (one print-media rule), and the one-line `docs/guide/using/reading.tmd:68` correction. All small, all independent, all default-on, all verifiable. Browser-verify the scrollspy at three viewport sizes.

**Session 2: honest search and honest output.** `unbounded-index` Stage 1 (delete `BODY_CAP`, split long sections on block boundaries), `find-in-page-honesty` (four edits: `divs.rs`, two `base.css` rules, `tabset.js`, plus the programmatic reveal path in `search.js`, plus the three-viewport layout check), and `long-output-fold` (CSS max-height + print reset + the `.tali-output img` decision; budget for a *new* vertical fade, not a reuse). This is where the reader stops getting false negatives from two different tools.

**Session 3: the whole-book outline, cheap half first.** `book-outline-artifact` Ship A (group the already-loaded index by page for Cmd-K's empty state, pure client change, **depends on session 1's numbering fix** because the index's heading text carries the rendered numbers), plus `book-shaped-search-results` (chapter grouping, `c`/`h` fields, partial matches with "Missing:", Damerau-aware fuzzy). Together these convert Cmd-K from a query box into the book's structural surface, with no new build artifact.

**Then, in rough dependency order.**

- `preview-skim-signals` piece 1 (the preview/build TOC selector) is small and independent; slot it anywhere after session 1.
- `section-extents` option (b) before anything that needs to enumerate a section's blocks.
- `skim-suggestion-severity` gates the whole lint family and must land before `skim-shape-lints`.
- `section-gist` before `skim-shape-lints`, because the NO-DESC rule literally depends on "neither a `description:` nor a derivable gist"; and `section-gist` feeds `summary-div`'s fallback chain.
- **`taliesin-skim` and `machine-shape-projections` before `skim-shape-lints`**, not after. The earlier draft claimed they "unblock the calibration of every lint" and then listed them later; inverted here, because you cannot calibrate a structural lint against a corpus you cannot measure.
- `book-outline-artifact` Ship B (the drawer sidecar) after session 1's numbering fix and session 2's search decoupling; it also decides whether `drawer-typeahead` is still wanted.
- `chapter-cost-signal` and `book-breadcrumb` are independent after session 1; `book-breadcrumb` must land in a compatible order with `scrollspy-activation-line` because both touch `--tali-nav-h`.
- `glossary-autolink` before `book-term-index`, which it feeds, and only after its invalidation resolution is chosen.
- `book-resume`, `link-text-self-describing`, `heading-keystep` and `float-digest` are independent medium items.

**Do not schedule.** `reading-density-dial` (three unbuilt prerequisites including `section-extents` option (a), and the premise it rests on is measurably overstated). `unbounded-index` Stage 2 (no measured trigger yet, and the dev-loop design is unsolved). `text-fragment-landing`'s `:~:text=` half (needs a fragment-safe text projection first; ship `?h=` alone). `changed-since` (demoted). `read-aloud` (verdict recorded, not scheduled). `content-visibility: auto` (deferred behind a measured trigger and `section-extents` option (a)).

## Open questions for the author

1. **Is the whole-book section outline in the drawer worth the bet?** Ship A (Cmd-K structural empty state) is cheap and unambiguous. Ship B is a genuine differentiator with no precedent: every comparable tool lists author-declared pages, not harvested headings, and that restraint may be deliberate at scale. Appetite question. It also decides whether `drawer-typeahead` is needed at all.

2. **Does `skim-suggestion-severity` get built at all?** It is the precondition for the lint family, and it is smaller than the earlier draft implied (`at_severity_floor` already exists and becomes a three-state floor). But you declined the same plumbing once (2026-07-10, TODO surfacing) in favour of a preview-only info diagnostic. If the answer is no, the trimmed lint family can still ship as ordinary warnings, but only the four binary rules, and only if you accept a red gate until the corpus is clean.

3. **`section-extents`: marker, wrapper, or decline?** The recommendation is the marker (option b), because it unblocks three of the four dependents at near-zero diff risk and only `reading-density-dial` needs the wrapper. But the wrapper is the one that would also unlock `content-visibility: auto` and sticky section headings, and it is the kind of change that is much cheaper now than after another year of block-model consumers. Is the diff risk worth pricing now or later?

4. **How much reader chrome is too much?** A book chapter already carries a sticky topbar, an ambient progress bar, a resume pill, a sticky TOC and a mobile sheet. `book-breadcrumb` adds a fourth persistent top element. The dwell-time evidence says the first viewport is the screening surface and that chrome spends it. Is "Part, Chapter" worth about 2.5rem of every chapter's fold, especially on the 900x1440 band?

5. **Does `::: {.summary}` earn new syntax?** It is the one proposal introducing an authoring primitive, it now carries the strongest experimental warrant in the whole set (advance organizers at g+ 0.53/0.70), and the adoption evidence is genuinely mixed: zero of 37 dogfood pages set `description:` today, which is both the argument *for* (the existing one-line affordance fails because it has no reader payoff) and the argument *against* (you may simply not write summaries). The Author playbook makes it step 6 for exactly that reason.

6. **How much of the Author playbook are you willing to run?** Steps 2, 4 and 5 are unglamorous manuscript work with no code in them, and three recommendations (`glossary-autolink`, `book-term-index`, `float-digest`) produce near-empty output until they happen. If the answer is "not much", those three should be deferred rather than built into an empty registry.

7. **What is the real target scale?** The audit assumed "300 pages" from the prompt, but the largest real document is 32,600 words and the largest corpus book is 1,135. Several recommendations (chunked index, cross-book search, term index, drawer type-ahead) only earn their cost at scales that do not exist yet. If the real near-term ceiling is 60k words, the sequencing above is right, `drawer-typeahead` is unnecessary, and Stage 2 of the index never happens.