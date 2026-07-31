# Research-publishing survey — what to take from the academic-web ecosystem

**2026-07-31.** Author-raised: "scrape the Academic Project Page Template, see what they do well,
take inspiration for publishing research." Widened on request to the whole ecosystem of academic
web-publishing projects that are *actually used*.

Backlog items **181-187** point here (plus **189** in Tier 3). This file carries the survey and a
per-item implementation brief so a session can go straight to a spec under
`docs/superpowers/specs/` without redoing the research.

> **Standing warning, same as every other notes file:** the seams named below were measured on
> 2026-07-31 and will rot. Grep the named symbol before pricing the work. Every "Taliesin does not
> have X" claim below is a grep result, not a memory — re-run the grep, it is recorded with each
> item.

---

## 1. The landscape, with evidence of use

Measured 2026-07-31 via the GitHub API (stars / forks / last push). Fork-to-star ratio is the
signal that matters: it separates "starred" from "used".

| Project | ★ | Forks | Last push | Job |
| --- | --- | --- | --- | --- |
| academicpages | 17,389 | 8,009 | 2026-07-27 | personal homepage |
| al-folio | 15,944 | 13,094 | 2026-07-31 | personal homepage |
| HugoBlox (ex-Wowchemy) | 9,608 | 2,947 | 2026-07-29 | personal homepage |
| tufte-css | 6,531 | 483 | 2026-06-24 | article typography |
| quarto-cli | 5,895 | 450 | 2026-07-31 | article framework |
| **Academic-project-page-template** | **5,111** | **1,142** | 2025-09-04 | **project page** |
| nerfies.github.io | 4,307 | 1,939 | 2024-06-21 | project page (the ancestor) |
| jupyter-book | 4,263 | 732 | 2026-07-24 | article framework |
| observablehq/framework | 3,574 | 201 | 2026-05-15 | data-app framework |
| idyll | 2,036 | 87 | 2023-02-04 | explorable articles (dormant) |
| minimal-light | 1,014 | 871 | 2026-07-21 | personal homepage |
| distillpub/template | 991 | 160 | 2022-12-05 | article framework (archived, still copied) |
| mystmd | 516 | 166 | 2026-07-28 | article framework |
| APPT-vue | 331 | 27 | 2025-07-08 | project page, config-driven |
| curvenote | 161 | 10 | 2026-07-30 | commercial platform |

**Three distinct jobs, and only one of them is Taliesin's.**

- **Article frameworks** (Distill, Tufte, MyST/Jupyter Book, Curvenote, Idyll, Quarto) — the
  document itself, web-native. This is Taliesin's category.
- **Project pages** (APPT, nerfies) — a landing page *advertising* a paper that lives on arXiv.
  Taliesin does not enter this category at all.
- **Personal homepages** (al-folio, academicpages, HugoBlox) — biggest numbers here, different
  job. Only one transferable piece; see §5.

---

## 2. What Taliesin already leads on — do NOT rebuild these

Measured, not assumed. Stated here because it changes what is worth building.

- **Reproducibility.** `crates/core/src/render/repro.rs` assembles the whole program *including*
  `#| echo: false` cells and ships it as a `data:` URL; `{{< dataset >}}` (item 176) covers data
  provenance. MyST and Jupyter Book offer a Binder/Colab launch button, which needs a live
  service. **Do not build launch buttons.** Item 158 (`{pyodide}`) is the stronger version of the
  same idea: run it in the reader's browser, no service at all.
- **Citation export.** `crates/core/src/site/cite_this.rs` emits BibTeX + CSL-JSON + RIS in tabs
  with a copy button (`tali-cite-copy`). Every template surveyed ships BibTeX alone.
- **Reactive prose.** Idyll's entire reason for existing is variables bound to text. Taliesin has
  `{{< range >}}`, `tali.state`, `animate`/`point` (items 153-157, shipped 2026-07-29). Idyll's
  last push was 2023-02-04.
- **Offline.** Distill loads from a CDN. The APPT pulls from six external hosts (googleapis,
  gstatic, jsdelivr, documentcloud.adobe.com, ajax.googleapis, youtube). Taliesin bundles.
- **Social card.** `site/card.rs` *generates* the card at build; the APPT makes you hand-author a
  1200x630 PNG.
- **Margin notes.** `::: {.column-margin}` with `.sidenote` / `.marginnote` / `.aside` aliases
  already ships (`assets/css/base.css` ~line 745, `vocab.rs` `DIV_CLASS_NAMES`).

## 3. What to reject, and why

Recorded so it is not re-proposed.

- **The carousel.** The APPT ships two elements with `id="results-carousel"` — invalid HTML, and
  it only works because the library selects by class. More importantly a carousel hides n-1 of
  your results behind a timer, which is the wrong default for research results. Take the need
  (N captioned figures side by side), reject the mechanism. See item 188.
- **Multi-format export.** MyST's pitch is one source to site + PDF + JATS + Word. Out of scope
  per `CLAUDE.md` (HTML is the only output target; the print track renders *from* built HTML).
- **The fixed floating "More Works" dropdown.** On mobile the APPT's own CSS moves it to
  `bottom: 2rem; right: 1rem`, which is exactly where its `.scroll-to-top` also sits. They
  overlap. The *idea* (cross-link the group's other papers) survives as a listing, not a
  floating widget.
- **Distill as a stack.** Archived 2022-12-05, journal on indefinite hiatus since 2021. Copy the
  component vocabulary and the layout ladder; do not adopt the framework.
- **The APPT's CSS habits.** 40 `!important` declarations fighting Bulma, `text-align: justify`
  on the abstract (rivers, no hyphenation), an unguarded `fadeInUp` on every `.hero`/`.section`
  with **zero** `prefers-reduced-motion` rules in 753 lines, and **zero** `prefers-color-scheme`
  rules (light only). Gradient under-borders on `h2` and animated link underlines would fight the
  owned iron-gall accent. Colour lives in `assets/css/tokens.css` + `tokens-dark.css`; use
  `--tali-*` tokens only, and note an invented token renders nothing (that is gated).

## 4. The five ideas worth taking

### A. Distill's layout-escape ladder — the biggest single gap

Distill ships a named width ladder any element can opt into: `.l-body` (text column),
`.l-middle`, `.l-page`, `.l-screen`, `.l-screen-inset`, `.l-gutter`, plus `.outset` and `.side`
(right-floated ~50%) modifiers. Tufte solves the same problem with `figure.fullwidth` and
`.fullwidth` on `div`, `table` and `pre` — confirming the need spans **all** block types, not
just figures.

Taliesin has **only the gutter end**. A figure, wide table, or 6-panel result plot that needs to
be *wider* than the prose column has no escape hatch. → **item 181**.

### B. Hover previews on citations and cross-references

Distill ships a dedicated `d-hover-box` component. MyST states it as their headline feature:
*"Only MyST references support rich features like hover-previews."*

Taliesin has `[@key]` citations and `@fig-`/`@sec-` cross-refs and **no hover machinery** —
grepped `crates/core/src/cite/`, `crates/core/src/site/xref.rs`, `web-client/*.js` and
`crates/core/assets/js/*.js` for `hover|popover|tooltip` on 2026-07-31: zero hits outside deck
and code-enhance. The data is already in the page. → **item 182**.

### C. Footnotes that render in the margin

Tufte's argument: a footnote forces the eye to the bottom of the page; a sidenote does not. His
implementation is pure CSS — `counter-reset: sidenote-counter`, a `.sidenote-number` that
increments it, and below the breakpoint the note goes `display: none` behind a
`label.margin-toggle` that reveals it inline via `:checked`. **No JavaScript at all.**

Taliesin has both halves and they do not meet: footnotes exist (`render/emit.rs`, gathered at the
bottom, ids `fnref-*`) and margin divs exist, but a `[^note]` cannot *become* a sidenote.
→ **item 183**.

### D. The appendix block

Distill's `d-appendix` standardises what a serious web paper owes its reader: **acknowledgments**,
**author contributions** (a first-class statement of who did what), reviewers, DOI, and how to
cite. Distill treating author-contributions as a section rather than prose was genuinely
influential. Taliesin has the citation half and nothing else. → **item 187**.

### E. Config-driven project pages are validated demand

The Vue rewrite of the APPT (331 stars, 27 forks) exists because people do not want to edit HTML.
That is Taliesin's front-matter thesis, independently confirmed. It is also the opening nobody in
this survey fills: **every project page in the wild is hand-maintained HTML, divorced from the
paper it advertises.** Generating it from the same `.tmd` is a differentiated position, not
catch-up. → **items 184-187**.

## 5. Deliberately not filed

- **The publications list** (al-folio / academicpages): a `.bib` rendered as a page with per-entry
  PDF / code / bibtex / abstract badges. Plausible for Taliesin's multi-page sites and distinct
  from `cite_this` (which is outbound, one page's own citation). **Not filed** — it is the
  personal-homepage job, and filing it would widen scope on speculation. Revisit if the author
  wants Taliesin to host their own academic homepage.
- **`citation_*` completeness beyond §6's list.** Anything requiring a network call (DOI
  existence, ORCID resolution) is item 167's declined half; do not revive it here.

---

## 6. Per-item implementation brief

Seams verified 2026-07-31. **Re-grep before pricing.**

### 181 — layout escapes (`.column-page`, `.column-screen`)

**Seam.** `.column-margin` is *pure CSS* — there is no `divs.rs` dispatch arm for it. It is a
generic fenced div whose class `assets/css/base.css` styles (~line 745). Registration lives in
two lists that a test pins together:

- `crates/core/src/render/validate.rs` → `DIV_FEATURE_CLASSES` (the did-you-mean anchor)
- `crates/core/src/vocab.rs` → `DIV_CLASS_NAMES` + the `named(...)` description table
- pinned by `vocab.rs::div_classes_are_a_subset_of_the_validator_vocab`

So the work is CSS + two list entries + a description, **not** a new render path.

**The trap that makes this bigger than it looks: there are THREE container modes**, and an escape
must work in all of them (measured in `assets/css/base.css` and `assets/css/site.css`):

1. single-doc — `body { max-width: var(--tali-maxw) }` (base.css ~line 201)
2. site page — `.tali-site-main { max-width: var(--tali-maxw) }` (site.css ~line 101), plus
   `.tali-site-main.tali-wide { max-width: 60rem }`
3. site page with TOC — `.tali-site-main.has-toc { max-width: 62.5rem; display: grid }`
   (site.css ~line 108) — **this one is a grid**, so a negative-margin escape behaves differently
   from the two flow containers. Browser-verify all three.

**Do not** reach for `width: 100vw` + negative margins as the first attempt: it breaks when a
scrollbar is present. Prefer a grid-column or `margin-inline` approach and measure.

**Pin.** `corpus/layout/` already exists and `corpus/layout/panels.tmd` already uses
`column-margin` — extend there rather than minting a doc. Verify that is still true.

**Verify.** Browser at all three standing viewport sizes — mobile 390x844, laptop landscape
1440x900, and **laptop portrait 900x1440, the band that gets forgotten** — and in all three
container modes. Use viewport *emulation*, never window resize ([LESSONS.md](LESSONS.md): probes
reported `innerWidth: 500` while the operator believed they were at 390, silently across the
40rem breakpoint the audit was about). Print/PDF (item 159 shipped) must not
regress: a `.column-screen` block on paper needs a rule.

### 182 — hover previews for citations and cross-references

**Two link shapes, both already emitted:**

- citations — `crates/core/src/cite/render.rs` emits `<a href="#ref-{key}">{n}</a>`
- cross-references — `class="tali-xref"`, carrying `data-tali-xref="{target}"` before the
  cross-page rewrite; after rewrite `crates/core/src/site/xref.rs` emits
  `<a href="{up}{page}#{anchor}" class="tali-xref">…</a>`

So the client selector is `a[href^="#ref-"], a.tali-xref`.

**Design fork — brainstorm before coding.** A *same-page* target can be read out of the DOM. A
*cross-page* target cannot, and that is the common case in a book. Options: (a) same-page only,
degrade silently; (b) server-side emit a `data-tali-preview` attribute carrying the target's
title/caption text at render time (the block model already has it; this is the
`repro.rs`-style "read the block model, not the DOM" answer). **(b) is almost certainly right** —
see the trap below.

**Trap, recorded from item 173.** A client-side scrape of the rendered page is the wrong source
for anything derived from the document. It was wrong for cells (`#| echo: false` runs and emits
no listing) and it is wrong here for cross-page targets. Read the block model.

**Trap.** Any new generated attribute or block owes the text-projection sweep: `taliesin read`,
`skim.rs`, the search index, and `llms-full.txt` — **four projections in three modules**. A
preview attribute containing caption text will leak into the search index and double-count.

**Pin.** `corpus/refs/` (cross-references + theorems) and a citation user such as
`corpus/posts/cite-coverage/`.

**Verify.** Browser. Keyboard focus must show the preview too, not just mouse hover, and it needs
a `prefers-reduced-motion` path. Tap target on mobile — item 173 shipped a 15px link because
inline text inherits the line box; WCAG 2.5.8 floor is 24px.

### 183 — footnotes as margin sidenotes

**Seam.** `render/emit.rs` — `[^name]` renders a superscript link (~line 114) and
`footnote_def_li` (~line 241) builds the gathered `<ol>`. `assets/css/base.css` styles
`.footnotes li:target` and `[id^="fnref-"]:target`.

**Design fork — this is a DEFAULT question, not a knob question.** Under the minimal-config
convention ("perfect the default before adding a knob"), the case for making margin placement the
*default* on a wide screen is strong: it is strictly better reading, and the gathered list can
stay as the narrow-screen fallback. Adding `footnotes: margin` as a front-matter key is the
lazy answer. **Get an owner ruling before coding.**

**Take Tufte's mechanism, it is pure CSS.** `counter-reset` / `counter-increment` on a
sidenote counter, and below the breakpoint `display: none` + `label.margin-toggle` + `:checked`
to reveal in flow. No JS means no client-side regression surface.

**Trap.** The existing `.column-margin` float and a numbered sidenote will collide in the same
right margin if a document uses both. Decide the stacking rule and pin it.

**Trap.** Footnotes appear in the print track (item 159) and in all four text projections. A
margin footnote on paper must still print; verify `taliesin pdf` output.

### 184 — structured authors + affiliations (substrate for 185/186/187)

**Today.** `author:` is a flat string list — `crates/core/src/site/frontmatter.rs` line ~54
(`authors: string_list(val.get("author"))`), consumed only by `cite_this.rs`, which splits
`Family, Given` and ` and `/` & `/`;`-joined names (`cite_this::parse_authors`).

**Target shape** (scalars stay valid — this must be backward-compatible or every corpus doc
breaks):

```yaml
author:
  - name: Ada Lovelace
    url: https://example.org/ada
    affiliation: 1
    equal: true
  - name: Charles Babbage
    affiliation: [1, 2]
    orcid: 0000-0002-1825-0097
affiliations:
  - Analytical Engine Institute
  - Somewhere Else
```

**Three consumers, one source** — that is the whole argument for doing this before 185/186/187:
the visible byline, `citation_author` + `citation_author_institution`, and JSON-LD `affiliation`.

**Where to register the keys.** `crates/core/src/frontmatter.rs` → `KNOWN_KEYS` (a new
`affiliations` key). **`KNOWN_KEYS` is not the only place a new key must be registered, and the
count is higher than it looks** — adding `datasets:` also touched `vocab.rs` in three separate
places (the key list, the `named(...)` description table, and a second list at ~line 586). **Work
the last added key backwards before starting**: `git log -S '"datasets"'` shows every site that
one key had to touch. Do not trust a remembered count, including this sentence.

**Trap.** `cite_this.rs` documents its render gate carefully (page author, falling back to *site*
author, never to site title). A structured author list must not silently change which pages emit
a cite box. Pin the gate with a test before touching the parser.

**Pin.** `corpus/cite-this/` exists — extend it, and it is also the natural home for the byline.

### 185 — resource-links row + venue/award badges

**The element every fork of the APPT keeps**: a row of pill buttons — Paper, Supplementary, Code,
arXiv — directly under the byline, above the fold.

**Minimal-config shape: infer everything from the URL.** `arxiv.org` → arXiv, `github.com` →
Code, `*.pdf` → Paper, `huggingface.co` → Data. `{text:, href:}` overrides.

```yaml
links:
  - https://arxiv.org/abs/2401.12345
  - https://github.com/me/repo
  - paper.pdf
venue: "CVPR 2026"
award: "Oral"
```

**`hero:` is close but wrong-shaped, do not overload it.** `site/frontmatter.rs::parse_hero`
takes `eyebrow / headline / lead / actions / image / image-alt`; `actions` is `{text, href,
primary}`. It **replaces** the title block and has no icon concept, so it cannot sit under a
byline. Filing this as a `hero` sub-key would fight `hero`'s existing job.

**Trap.** Icons must be bundled, not FontAwesome/academicons from a CDN — that is the offline
guarantee. Inline SVG or nothing.

**`venue:` doubles as the source for `citation_conference_title` in 186.**

### 186 — complete the Scholar + social meta

**Today** (`crates/core/src/site/meta.rs`, grepped 2026-07-31) Taliesin emits `citation_title`,
`citation_author`, `citation_publication_date`, `citation_pdf_url`, `citation_public_url`,
`citation_journal_title`, the OG/Twitter block, and JSON-LD `ScholarlyArticle`.

**Missing and cheap:**

- `citation_conference_title` (from `venue:`, item 185) — only `citation_journal_title` exists
- `citation_doi`
- `citation_arxiv_id`
- `citation_author_institution` (needs item 184)
- `citation_abstract_html_url`
- `og:image:width` / `og:image:height` — the generated card's dimensions are known at build, and
  LinkedIn in particular only renders a large card when they are present

**Trap.** `emit_social` is deliberately shared between the page path (`social_head`) and the
embedded-deck path (`deck_social_head`) "so both emit the same tag shape by construction". Add to
the shared function, not next door.

**Trap: the inlined-asset needle trap** ([LESSONS.md](LESSONS.md)). Every page inlines the whole
CSS+JS payload, so a whole-page `contains()` for a new meta name is a claim about the bundle too —
**and it fires on negative assertions as well**. Needle the full emitted tag.

### 187 — the appendix block

Acknowledgments, **author contributions**, DOI, and how-to-cite, gathered after the body — the
`d-appendix` idea. Contributions key off item 184's author list.

**Seam.** `cite_this.rs` already appends a generated block at the end of the page and documents
the pattern (empty sourcepos, like References and the gathered footnotes). Follow it — and note
its determinism rule: **no "Accessed:" date, no build timestamp**, or the static build stops being
byte-identical and busts the freeze cache.

**Trap.** Another generated block means another `REPRO_BLOCK_ID`-style constant and the same
four-projection sweep (item 173's lesson: `taliesin read`, `skim.rs`, search index,
`llms-full.txt`).

### 188 — results gallery + image-comparison slider

**Evidenced need, rejected mechanism.** Every project page surveyed shows N result figures; the
APPT and nerfies both reach for a carousel. Build instead:

- `::: {.gallery}` — a responsive grid of numbered figures, reusing `render/figure.rs` so
  `@fig-` cross-references still resolve. Pairs with item 181 (a gallery usually wants
  `.column-page`).
- an **image-comparison slider** (drag divider between two images) — the interaction CV project
  pages actually want and that a carousel substitutes for badly. `assets/js/scrolly.js` and the
  magic-move machinery are the right neighbourhood.

**Trap.** Scroll/drag features have a documented false-negative pattern in browser tests (the
`.scrolly` / `.code-walkthrough` work): force `scroll-behavior: auto`, settle rAF, and floor
`innerWidth` around 500px, or the test fails for reasons unrelated to the feature.

---

## 7. Sources

Repo metrics via the GitHub API 2026-07-31; template source read from raw.githubusercontent.

- <https://github.com/eliahuhorwitz/Academic-project-page-template> (README, `index.html`,
  `static/css/index.css`, `static/js/index.js` all read in full)
- <https://distill.pub/guide/> and `distillpub/template` `src/components/` listing (the `d-*`
  component set, incl. `d-hover-box`, `d-appendix`, `d-byline`, `d-toc`)
- <https://distill.pub/2021/distill-hiatus/>
- <https://edwardtufte.github.io/tufte-css/> (`tufte.css` read in full)
- <https://gwern.net/sidenote>
- <https://mystmd.org/guide/external-references>
- <https://proceedings.scipy.org/articles/hwcj9957> (Jupyter Book 2 / MyST stack)
- <https://curvenote.com/products/reader>, <https://www.nature.com/articles/d41586-024-02577-1>
- <https://arxiv.org/abs/2605.16562> (arXiv HTML + MathML 4, 2026)
