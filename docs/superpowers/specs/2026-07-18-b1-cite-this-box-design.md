# B1 — Reader-facing "Cite this" box (BibTeX / CSL-JSON / RIS)

Date: 2026-07-18. Backlog item **B1** (PMF audit; = revived D70). Branch `b1-cite-this-box`.

## Goal

Render a small **"Cite this"** box near the end of an article that shows *how to cite this
page itself* and lets the reader **copy** or **download** the citation as **BibTeX**,
**CSL-JSON**, and **RIS**. The machine-readable half already ships (Google-Scholar
`citation_*` meta + ScholarlyArticle/BlogPosting JSON-LD in `site/meta.rs`). B1 is the
reader-facing widget, built from the *same* front-matter fields so the two cannot drift.

This closes the academic-trust gap (PMF persona D1): a reviewer opening a `pages.dev` link
can cite it without asking for a PDF.

## Data source (reuse, no new required front-matter)

All inputs already exist; no new front-matter is introduced (DOI is **deferred to B5** per
owner ruling 2026-07-18):

- `title`  — `Page.title` (required, non-empty)
- `authors` — resolved: `Page.authors` if non-empty, **else `SiteConfig.authors`**
  (owner ruling 2026-07-18: *site-author fallback*). The chain stops there — it does **not**
  fall back to `SiteConfig.title`, so the byline is always a real author name or the box is
  absent. This is a strict subset of the JSON-LD author chain in `meta.rs:140-144`.
- `date`   — `Page.date` (required, `Some`); the year is required, month/day used when the
  value parses as a full ISO date.
- `url`    — canonical page URL (as `meta.rs` computes `page_url` from `SiteConfig.url`);
  **optional** enrichment. Included when the site sets `url:`, omitted otherwise.
- `venue`  — `SiteConfig.title` as the container/"published on" venue; optional.

### The render gate

The box renders **iff** `resolved_authors` is non-empty **and** `date.is_some()` **and**
`title` is non-empty. URL is *not* required for the gate.

Consequence, verified against corpus: today **no** tech-blog post nor `corpus/tech-blog/
_site.yml` sets `author:`, so the box renders **nothing** on the current blog — it degrades
to nothing, not an empty box. Adding one line (`author:`) to `_site.yml` lights up every
dated post. The positive path is pinned by a new fixture (below).

## Outward serializers (new code in `taliesin-core`)

No outward serializer exists today — `cite/format.rs` is *inbound* (`.bib` → IEEE HTML). Add
a new site-layer module `crates/core/src/site/cite_this.rs` (it needs `Page` + `SiteConfig`
in scope) that turns the resolved fields into the three formats. All output is **deterministic** (no build
timestamp / "Accessed:" date — that would break byte-identical builds and the freeze cache).

**Author name split** (shared helper): if a name contains a comma it is already
`Family, Given`; otherwise the last whitespace-token is the family name and the rest is
given. Single-token names are family-only. Names inside one `authors` entry joined by
` & `, ` and `, or `;` are split into multiple authors. Documented as a best-effort
heuristic (matches how the existing `cite/author.rs` reasons about `and`-lists).

**Cite key**: `<family-lowercased><year><first-title-word-lowercased>`, ASCII-folded,
non-alphanumerics stripped (e.g. `bogossian2026em`). Deterministic and stable.

**BibTeX** — `@misc` entry:
```
@misc{bogossian2026em,
  author       = {Bogossian, Andreas},
  title        = {{The EM-algorithm}},
  year         = {2026},
  howpublished = {\url{https://andreasbogossian.com/posts/em-algorithm/}},
  note         = {Andreas Bogossian}      % venue, only if distinct/present
}
```
Title is brace-protected (`{{…}}`) to preserve case. `howpublished`/`url` lines omitted when
no site `url:`. BibTeX special chars escaped.

**CSL-JSON** — a one-element array:
```json
[{ "id": "bogossian2026em", "type": "post-weblog", "title": "The EM-algorithm",
   "author": [{ "family": "Bogossian", "given": "Andreas" }],
   "issued": { "date-parts": [[2026, 4, 14]] },
   "URL": "https://…", "container-title": "Andreas Bogossian" }]
```
`date-parts` carries only the components present. JSON-escaped.

**RIS**:
```
TY  - BLOG
TI  - The EM-algorithm
AU  - Bogossian, Andreas
PY  - 2026
DA  - 2026/04/14
UR  - https://…
T2  - Andreas Bogossian
ER  -
```
Optional lines omitted when their source is absent.

## Render path (generated block, site layer)

Follow the `attach_backlinks` precedent exactly: append one generated `Block` in
`finish_blocks` (`site/mod.rs`), *after* xref/backlink work, so the static build and the live
preview inject identically. The block has `id: "qmd-cite-this"`, empty `sourcepos`,
`source_file: None`, `cell: None`. It lands inside `content`, ahead of `post_nav_html`.

The block HTML embeds the three serialized strings server-side (each in its own element the
JS reads via `.textContent`), so there is **no client-side citation logic to drift**:

```html
<aside class="tali-cite-this" data-block-id="…" aria-label="Cite this page">
  <h2>Cite this</h2>
  <div class="tali-cite-tabs" role="tablist">…BibTeX · CSL-JSON · RIS…</div>
  <pre class="tali-cite-out" data-format="bibtex">@misc{…}</pre>
  <pre class="tali-cite-out" data-format="csl" hidden>[ … ]</pre>
  <pre class="tali-cite-out" data-format="ris" hidden>TY  - BLOG …</pre>
  <div class="tali-cite-actions"><button …>Copy</button><a download …>Download</a></div>
</aside>
```

Styling: a new `--tali-*`-token block in the site CSS (Marginalia identity, theme-aware,
light + dark). No hard-coded colors, no vendor hexes.

## Client behavior (bundled, offline)

Add one ordered fragment `assets/js/code-enhance/17-cite-box.js` (next after
`16-scroll-a11y.js`) and register it in the
`CODE_ENHANCE_JS` concat in `render/mod.rs` (updating the
`code_enhance_bundle_matches_fragments_in_order` guard test). It only:
1. switches the visible `<pre>` when a tab is clicked (ARIA tab semantics, keyboard-navigable),
2. copies the active format to the clipboard (`navigator.clipboard`, with a
   `document.execCommand` fallback), showing a transient "Copied",
3. wires each Download link to a `Blob` of the active format's text with the right
   extension (`.bib` / `.json` / `.ris`) and MIME.

No network, no external libs. Progressive enhancement: without JS the box still shows the
three citations as readable text (the `<pre>`s), just without one-click copy/download.

## Pin (corpus) + tests

- **New fixture** `corpus/refs/cite-this.tmd` (a page that sets `title:` + `author:` +
  `date:`, and cites a `.bib` so it is a ScholarlyArticle) — the positive pin: box renders,
  all three formats present + well-formed, byline is the page author.
- **Negative pin**: an existing authorless dated tech-blog post renders **no**
  `tali-cite-this` block (asserts "degrade to nothing").
- **Site-author fallback pin**: a page with `date:`+`title:` but no page `author:`, under a
  site whose `_site.yml` sets `author:`, renders the box with the site author as byline.
- **Serializer unit tests** (`taliesin-core`): BibTeX/CSL-JSON/RIS golden output for a
  known input incl. multi-author, comma-name, missing-url, missing-month; cite-key
  generation; author-name split. Each mutation-checked (mutate serializer → named test fails).
- **JS guard**: the fragment-order test updated so the bundle can't silently drop the fragment.
- **Browser verify** (chrome-devtools MCP): the box renders on the fixture at the three
  viewport sizes; tab switch + copy + download work; light + dark themes both legible;
  absent on the authorless post.

## Out of scope (deferred)

- **DOI / publisher / journal front-matter** → B5 (Zenodo DOI on-ramp). B1 uses only
  existing fields.
- **Per-page opt-out knob** — not built (minimal-config: the metadata gate *is* the control;
  a page/site without `author:` shows nothing). Revisit only on demand.
- **Book chapters / decks** — B1 targets website article pages. Extending to book chapters is
  a follow-up if wanted (the same generated-block hook exists in the book path).

## Invariants honored

No CDN / all offline; no preview write-back; no new output format; `--tali-*` tokens only;
generated block carries `data-block-id`, no sourcepos (matches `attach_backlinks`);
deterministic output (byte-identical builds preserved).
