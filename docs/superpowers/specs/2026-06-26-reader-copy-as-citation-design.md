# Reader copy-as-citation — design

> Status: shipped (2026-06-26, `feat/reader-copy-as-citation` → main). A **Cite**
> action on the selection toolbar copies a BibTeX entry that deep-links to the
> selected passage. Browser-verified (chrome-devtools): exact `@misc` output,
> single-pass LaTeX escaping (stress-tested), empty-title fallback, and the live
> toolbar (Copy/Quote/Share/Cite) flashing "Cited" on click; no console errors.

## Problem

Readers who cite technical content — the kind qmd-fast produces — work in reference
managers (Zotero, JabRef) and LaTeX. The selection toolbar already offers **Copy** (raw
text), **Quote** (Markdown blockquote), and **Share link** (a bare deep URL), but nothing
that drops straight into a `.bib`. A plain citation string is terminal; BibTeX is a
structured, lossless source that round-trips through any tool into any style — the
"most universal" choice for this audience. The four actions then divide cleanly with no
overlap: Copy = raw, Quote = Markdown attribution, Share = URL, Cite = BibTeX.

## Goal

A fourth selection-toolbar action, **Cite**, that copies a BibTeX `@misc` entry:

```bibtex
@misc{a-longer-read-2026,
  title        = {{A longer read}},
  howpublished = {\url{https://example.com/long-read.html#:~:text=the%20selected%20passage}},
  note         = {Accessed June 26, 2026}
}
```

- **title** = `document.title`, LaTeX-escaped, double-braced (`{{…}}`) to preserve casing.
- **howpublished** = `\url{…}` wrapping the text-fragment deep link `qmdBuildTextFragmentUrl`
  builds — `\url` (url/hyperref) makes its argument verbatim, so the LaTeX-hostile
  `# : ~ % &` in the deep link survive untouched.
- **note** = `Accessed <Month D, YYYY>` — today, client-side, explicit English month names
  (deterministic + offline; no locale dependence).
- **cite key** = title-slug + year (`a-longer-read-2026`); falls back to `qmd-citation` when
  the title slugs to empty.

Reader-side, read-only, one-shot clipboard action (no `localStorage`, no source writes).
Flashes **Cited** / **Copy failed**, announced through the toolbar's existing `aria-live`.

**Entry type: `@misc` only.** It is the genuinely universal type — classic BibTeX, biblatex,
and Zotero import all accept it. `@online` + `urldate` are semantically nicer but
biblatex-only (classic BibTeX errors on them).

**Out of scope (YAGNI):** author, publication date, the quoted snippet (the deep link
already points at the passage; Quote covers blockquoting), a style switcher, `@online`,
new CSS (reuse `.qmd-hl-action`).

## Invariants honored

- Reader-side, read-only, offline, additive; no block-model change.
- **Decks excluded** implicitly: the toolbar is built inside `qmdInitHighlights`, which
  already returns early on `.qmd-deck`.
- **Pure builder:** `qmdBuildBibtex(title, url, date)` does no DOM/clipboard work and is
  deterministic for a given `date` — it sits beside `qmdBuildTextFragmentUrl`. The action
  wires it to `document.title` + `qmdBuildTextFragmentUrl(pending.text)` + `new Date()`.

## Mechanism

All in `crates/core/assets/js/code-enhance.js`.

**`qmdBuildBibtex(title, url, date)`** (new top-level function, beside
`qmdBuildTextFragmentUrl`):

- `name = (title || 'Untitled').trim()`
- `key = slug(name) + '-' + date.getFullYear()`, where `slug` lowercases and maps
  `[^a-z0-9]+ → '-'` then trims leading/trailing `-`; if the slug is empty the key base
  falls back to `qmd-citation`.
- `accessed = MONTHS[date.getMonth()] + ' ' + date.getDate() + ', ' + date.getFullYear()`
  with `MONTHS = ['January', …, 'December']`.
- `latexEsc(s)` escapes the LaTeX specials in text fields, backslash first:
  `\\ { } & % $ # _ ~ ^`. Applied to the **title only** (the note is controlled text:
  month name + digits + comma, all LaTeX-safe; the URL is verbatim inside `\url{}`).
- Returns:
  ```
  @misc{<key>,
    title        = {{<latexEsc(name)>}},
    howpublished = {\url{<url>}},
    note         = {Accessed <accessed>}
  }
  ```

**The action**, in the selection toolbar (after `shareBtn`):

```js
var citeBtn = action('Cite', function (done) {
  var url = qmdBuildTextFragmentUrl(pending.text) || location.href;
  qmdCopyText(qmdBuildBibtex(document.title, url, new Date()), function () {
    done('Cited');
    if (location.protocol === 'file:') announce('Citation copied; the deep link opens when served over http or https');
  }, function () { done('Copy failed'); });
});
```

Append `citeBtn` to the `extras` array so it joins Copy/Quote/Share before the Highlight
button. `placeBtn` already centers + viewport-clamps the wider bar; no layout change.

## Verification

- **Corpus pin:** reuse `corpus/reader/share.qmd` (the selection-toolbar fixture; Cite joins
  the same toolbar). Add one sentence to its prose noting the Cite action.
- **Rust test** (`render/tests.rs`): `assembled_page_ships_cite_action` — `render_html_page`
  contains `qmdBuildBibtex` (the pure helper is a unique discriminator token).
- **Browser (chrome-devtools MCP):** on the served fixture, call `window.qmdBuildBibtex(
  'A longer read', '<deeplink>', <fixed date>)` and assert the exact `@misc{…}` shape
  (key, `{{title}}`, `\url{}`, `note`); then select prose, confirm the **Cite** button is in
  the toolbar and clicking flashes **Cited**; no console errors.
- **Gates:** `cargo test -p qmd-fast-core`, `cargo clippy -D warnings`, `cargo fmt`,
  `cd web-client && tsc`.

## Files

- `crates/core/assets/js/code-enhance.js` — `qmdBuildBibtex` + the Cite action.
- `crates/core/src/render/tests.rs` — `assembled_page_ships_cite_action`.
- `corpus/reader/share.qmd` — one sentence noting Cite (pin only).
