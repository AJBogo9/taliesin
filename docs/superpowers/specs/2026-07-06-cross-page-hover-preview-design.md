# F2a: hover preview for cross-page cross-reference links

**Date:** 2026-07-06 · **Status:** approved, ready for plan · **Backlog:** Tier-1 (P3)

## Problem

The hover-preview card (`crates/core/assets/js/code-enhance/12-link-preview.js`)
only fires for **same-page** links: `eligible()` requires `href` to start with `#`,
and it locates the target with `document.getElementById`, cloning the live element
into a singleton `#tali-link-preview` card. A **cross-page** cross-reference link
(`<a href="methods.html#thm-1" class="tali-xref">Theorem&nbsp;2.1</a>`, produced by
`crates/core/src/site/xref.rs:228`) is rejected by construction — its target lives in
a different HTML file the client cannot reach (no `fetch`, no pre-fetched content).

Cross-page refs are the one class of link where a reader most wants a preview (they
can't just scroll to see the target), so today's card silently does nothing for them.

## Goals

- Hovering a cross-page `.tali-xref` link shows the **same** card, with the rendered
  content of the referenced figure / theorem / table / equation / listing / section.
- Works in **live preview and static build**, and from `file://` (no `fetch`).
- No new config key. The affordance is automatic wherever cross-page refs exist.
- Pinned by a corpus doc (a multi-page project with cross-page refs) plus a browser
  check.

## Non-goals

- **No keyboard/focus trigger.** The existing card is mouse-only (a pre-existing a11y
  gap); this change does not widen or fix it. Left for a separate focused pass.
- **No same-page behavior change.** The `href`-starts-with-`#` path is untouched.
- **No new reactive/live-edit invalidation guarantees** beyond what the search index
  already provides (see Staleness).
- Does not touch the exec/kernel zone or the single-editing-surface invariant.

## Design overview

Mirror the Cmd-K **search-index** pattern end to end — it is the project's precedent
for serving per-site data to the client in a `file://`-safe way.

```
Site::discover
  └─ hover::build_index(pages)  → Site.hover_index_json  (anchor → snippet HTML)
       served as hover-index.js (window.TALIESIN_HOVER_INDEX = {...})
         · static build: build.rs writes _site/hover-index.js
         · preview:      GET /hover-index.js reads Site.hover_index_json
       pointer window.TALIESIN_HOVER_URL injected via SiteCtx / page_chrome
                                   │
client 12-link-preview.js  ◄───────┘
  · eligible-cross-page: a.tali-xref with href NOT starting with '#'
  · lazy-load hover-index.js on first cross-page hover (<script src>, file://-safe)
  · split href → "#anchor"; look up snippet; inject into the same #tali-link-preview card
```

### 1. Server: harvest the snippet index

New module `crates/core/src/site/hover.rs`, sibling to `search.rs`/`xref.rs`.

`build_index_json(...) -> String` (taking the same page slice + book-chapter scoping
`search::build_index_json` takes), called once from `Site::discover` alongside it and
stored on a new `Site.hover_index_json: String` field.

Per page: re-render the source (`render_document_with_includes[_scoped]`, **no code
execution** — same as `search.rs`), giving a `RenderedDoc` with its block list and its
`xref_numbers` map (whose keys are exactly the xref anchors defined on that page).

**Capture is block-level** (reuse the block model — no HTML slicing): for each anchor
defined on the page, find the block whose HTML contains `id="<anchor>"` and take that
block's rendered HTML as the snippet. For a **heading** anchor, also append the next
1–2 blocks (stopping at a block that is itself a heading or bears its own `id`) — this
mirrors the same-page card's "heading + up to 2 following siblings" behavior so a
`@sec-` preview shows the section intro, not a bare title. For figure / theorem /
table / equation / listing anchors, the single defining block is already the whole
element.

**Clean** each snippet the way the card's `cleanClone` does server-side: drop
`.tali-anchor` (permalink chrome) and `.tali-copy` (code-copy buttons). Cap total
snippet length defensively (a large cap; content is already the server's own safe
output). Because the snippet is embedded in a `<script>`, escape `</script>` exactly as
`search.rs`'s `json_str` does.

**JSON shape** — a flat object keyed by anchor (anchors are project-unique;
first-definition-wins with a duplicate warning, per `xref_targets`):

```json
{ "thm-1": "<div class=\"tali-theorem …\">…</div>",
  "fig-plot": "<figure id=\"fig-plot\" …>…</figure>",
  "sec-methods": "<h2 …>Methods</h2><p>…</p>" }
```

The client already knows the target page from the href, so the value needs only the
snippet HTML (no url).

### 2. Server: relative-asset rewriting (the cross-page wrinkle)

A figure snippet defined on `chapters/methods.html` may contain
`<img src="figs/plot.png">` — relative to *its* directory. Shown in a card on the root
`index.html`, that path resolves against the wrong base. At harvest time, rewrite
relative `src` / `href` values in the snippet HTML to **site-root-relative** by
prefixing the defining page's directory (reuse the site's existing link-rewrite
helper; do not touch absolute `http(s)://`, protocol-relative `//`, `data:`, `mailto:`,
or in-snippet `#fragment` links). Client-side, prefix a root-relative asset URL with
the existing `window.TALIESIN_SITE_ROOT` global (the current page's up-path to root,
already emitted for search). Net: an image renders correctly in the card no matter
which page the reader is on. Sections / theorems / equations carry no external assets,
so this affects only image figures.

### 3. Server: serve the index

Exactly like `search-index.js`, gated on a non-empty index:

- **Static build** (`crates/server/src/build.rs`, next to the search-index write):
  write `_site/hover-index.js` = `window.TALIESIN_HOVER_INDEX={json};`.
- **Preview** (`crates/server/src/serve_site/mod.rs`): route `GET /hover-index.js`
  returning `window.TALIESIN_HOVER_INDEX={json};` as `text/javascript`; add the
  mounted-sub-site branch in the fallback handler, mirroring the search route.
- **Pointer**: `window.TALIESIN_HOVER_URL` set in the same inline chrome snippet that
  already sets `TALIESIN_SEARCH_URL` (`site/mod.rs` `page_chrome` → `SiteCtx`), so it is
  available on every page that gets site chrome (not just TOC pages — cross-page refs
  can appear on any page).

### 4. Client: extend `12-link-preview.js`

- Broaden eligibility: in addition to the existing same-page (`href[0] === '#'`) path,
  accept `a.tali-xref` whose `href` does **not** start with `#` (still excluding TOC /
  the card's own contents).
- On the first eligible cross-page hover, lazy-load `window.TALIESIN_HOVER_URL` via an
  injected `<script src>` (mirroring `search.js`'s `loadIndexThen`; sets a loaded flag
  so it fires once; degrades silently on error / `file://` miss). When it arrives, if
  the pointer is still on the link, show the card.
- On show for a cross-page link: split the href at `#` → the anchor; look it up in
  `window.TALIESIN_HOVER_INDEX`; if found, set the card body to the snippet HTML,
  rewrite root-relative asset URLs by prefixing `window.TALIESIN_SITE_ROOT`, and reuse
  the existing `place()` / open / dismiss / pin logic verbatim. If the anchor is absent
  (stale index / broken ref), show nothing (current no-op behavior).
- Keep `taliInitLinkPreview` and the idempotency guard intact (a corpus test asserts
  the symbol ships).

## Staleness

`Site.hover_index_json`, like `search_index_json`, is rebuilt only on a full
`Site::discover` (config change or structural page add/remove), not on an in-place
content edit. So during a live-edit session a cross-page snippet can lag until the next
structural change — identical to the Cmd-K index, and acceptable for a preview
affordance (a slightly stale snippet, never a wrong link). Not worth special-casing in
the per-page rebuild path for v1.

## Testing / corpus pin

- **Unit (core):** given a small two-page fixture with a cross-page figure + theorem +
  section ref, `hover::build_index_json` produces an index containing each anchor with
  non-empty snippet HTML; asset URLs are rewritten root-relative; `</script>` is
  neutralized.
- **Corpus/invariant:** the existing cross-page-ref corpus (e.g. the `b.html#fig-plot`
  / `methods.html#sec-methods` fixtures) builds with a `hover-index.js` emitted and the
  referenced anchors present. Keep the `assembled_page_ships_hover_cards` guard green.
- **Browser (chrome-devtools MCP):** preview a multi-page corpus, hover a cross-page
  ref, confirm the card opens with the right rendered content (incl. an image figure
  resolving), 0 console errors, at the three viewport sizes.

## Risks / mitigations

- **Index size** — HTML snippets are heavier than the search plaintext. Mitigate with a
  per-snippet length cap and the fact that the file is lazy-loaded once, only when a
  reader hovers a cross-page ref.
- **Asset-URL edge cases** — be conservative: only rewrite clearly-relative URLs; leave
  absolute/protocol-relative/`data:`/`mailto:`/`#` untouched. Cover with the unit test.
- **Block-anchor detection** — matching `id="<anchor>"` as a substring could in
  principle match an unrelated attribute value; guard by matching the `id="..."`
  attribute form and preferring the block the renderer's registry attributes the anchor
  to. Verified against the corpus.
