# Cmd-K search relevance (multi-term / prefix / fuzzy) — design

> Status: building (2026-06-25, branch `feat/search-relevance`, ultracode). From a 4-lens
> design judge-panel. Addresses the audit's "one clear quality win": the Cmd-K matcher is a
> single whole-query `indexOf`, so "block diff" matches nothing unless contiguous and a typo
> never matches.

## Decision: hand-roll, do NOT vendor MiniSearch

Unanimous across all four lenses:

- **Payload.** `web-client/search.js` is `include_str!`'d as `SEARCH_JS`
  (`render/mod.rs`) and inlined into *every* site/book/TOC page. MiniSearch (~7KB gz, ~25KB
  raw) would ship + re-parse on every such page; the hand-rolled matcher replaces the current
  `score`/`highlight`/`snippet` (~70 lines) with ~120 lines that gzip to well under 1KB.
- **No scale to justify it.** The index is a few hundred to low-thousands of short
  `{title, body}` entries scanned linearly per keystroke (sub-ms). MiniSearch's inverted
  index + BM25 buy nothing here, and the single-doc path rebuilds from the DOM on every open
  anyway (its persisted-index advantage is unused).
- **It doesn't retire the real work.** The snippet/highlight token-span rework is needed
  either way; MiniSearch only gives match positions.
- **Discipline + blast radius.** "Hand-roll small things"; vendoring under
  `crates/core/assets/js/` trips `tests/third_party.rs` (`vendored_js_is_attributed`),
  forcing a `THIRD_PARTY.md` edit + a second `include_str!`. Hand-roll keeps it to one file.
- **No regression net.** `search.js` isn't in the type-checked client bundle and corpus tests
  are Rust-side; an auditable hand-rolled matcher beats an opaque dependency here.

The change is confined to `web-client/search.js`. **No Rust / `search.json` / `THIRD_PARTY.md`
change.** `search.json` stays byte-identical; `buildIndex()` already normalizes both the site
(`{i,t,l,b,u,p}`) and single-doc (DOM) inputs to `{id,title,level,body,url,page}`.

## Matcher

Tokenize **once per `render()`**: `terms = q.trim().toLowerCase().split(/\s+/).filter(Boolean)`.
Empty `terms` → the existing empty-query branch is untouched (book → `level===0` entries;
single doc → full outline). `buildIndex()` memoizes `tLow`/`bLow` (lowercased title/body) per
entry so the hot loop is `indexOf` + a bounded scan.

`score(item, terms)` → number (`0` rejects). For each term, `termHit(term, tLow, bLow)` returns
a per-term contribution, first win:

1. `tLow.indexOf(term) >= 0` → title/exact (**6**); record `pos` for the leading-prefix bonus.
2. `bLow.indexOf(term) >= 0` → body/exact (**3**).
3. `term.length >= 4` and a title word is within edit-distance 1 → title/fuzzy (**2**).
4. `term.length >= 4` and a body word is within edit-distance 1 → body/fuzzy (**1**).
5. else `null` → the item is rejected (**AND** semantics: every term must hit some field).

`score = Σ term contributions` + bonuses: all terms hit the title **+3**; some term at title
`pos 0` **+2**; the full original `q` is a contiguous substring of `tLow` **+2** / of `bLow`
**+1** (exact-phrase reward). Tie-break: `level` ascending, then index order. (Single-term
substring degenerates to today's prefix>contains>body ordering.)

`within1(a, b)`: bounded edit-distance-1 (two cursors, one allowed skip — a substitution OR an
insert/delete), `O(len)`, gated to `term.length >= 4`, run **only after exact misses**, with a
first-character-match prune. Tested per **word** of the field, not whole-field Levenshtein.
Substring already covers prefix-of-word (e.g. "diff" ⊂ "difference"), so there is no separate
"prefix" kind — multi-term + substring delivers the prefix behavior; fuzzy adds typo tolerance.

## Highlight + snippet (multiple terms)

`highlight(el, text, terms)` and `snippet(el, body, terms)` both take the **token array** (not
the old whole-query string) and share one `emitRanges(el, sourceText, ranges)` helper that
sorts + **merges overlapping/adjacent** ranges and emits alternating `textNode` / `<mark>` in
one pass (DOM-built with `textContent`/`createTextNode`, **never `innerHTML`**; matched against
lowercased text, sliced from the original-case source for display).

- **highlight:** collect every substring occurrence of each term in `text` (case-insensitive,
  `indexOf`-all), merge, emit. Fuzzy-only terms (no substring) are left unmarked — honest, and
  avoids marking the wrong letters.
- **snippet:** find each term's offsets in `bLow`; pick the ~140-char window covering the most
  *distinct* terms (slide over the sorted term offsets, tie-break earliest), mark every term
  occurrence inside it via `emitRanges`. Fall back to `body.slice(0,120)` when no term is in
  the body. `itemEl` suppresses the snippet when **every** term is already in the title
  (generalizing today's `title.indexOf(q) < 0` guard).

## Preserved exactly

`search.json` + `site/search.rs`; the `buildIndex()` normalization; the empty-query branch;
arrow/Enter/Esc nav; cross-page `go()` (`url`/`QMD_PAGE_URL`/`QMD_SITE_ROOT`); the loading row;
title-vs-snippet structure; deck/no-TOC pages (search.js only acts where wired); all block-model
invariants (search.js touches none).

## Verification (no ranking regression net → enumerated manual cases)

- **Rust test** (`render/tests.rs`): a TOC/site page ships the new matcher (a marker symbol,
  e.g. `emitRanges` / the tokenizing matcher) in `SEARCH_JS`.
- **Browser (chrome-devtools MCP) over a served book** (`docs/internals` built + localhost
  http): (1) multi-term out-of-order ("diff block" matches a section with both, non-contiguous);
  (2) prefix ("diff" matches "difference"); (3) single-typo ("blcok" matches "block"); (4)
  multi-term highlighting in title + a density-chosen snippet marking both terms; (5)
  empty-query chapter list unchanged; (6) arrow/Enter cross-page nav still works; (7) a query
  that must NOT match returns "No matches"; (8) a short term ("fn") does not fuzzily explode
  results; (9) overlapping terms ("data"/"database") merge into one mark.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc` (search.js is not in the tsc
  bundle, but run it for the workspace).

## Files

`web-client/search.js` (the matcher + `emitRanges` + snippet/highlight rework), a test in
`crates/core/src/render/tests.rs`. No corpus pin (web-client behavior, verified against the
existing `docs/internals` book).
