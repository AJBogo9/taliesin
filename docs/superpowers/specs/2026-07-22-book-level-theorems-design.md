# Book-level `theorems:` config (backlog item 16 F-01)

Date: 2026-07-22
Status: approved design, ready for a plan

## Problem

`theorems:` configures theorem-environment numbering (`shared:` counter groups + a
`numbered:` mode). It is a **per-document** front-matter key: `render_internal_impl` parses
it from the doc's own front-matter (`crates/core/src/render/mod.rs:388`,
`parse_theorem_config` -> `TheoremConfig { shared, numbered }`). A book (a `_site.yml`
project with numbered chapters) therefore has to repeat the same `theorems:` block in every
chapter to get one book-wide policy. At `_site.yml` level the key is not recognized (it is
absent from `NATIVE_KEYS`, `crates/core/src/site/config/mod.rs:117`), so a book-level
`theorems:` draws the unknown-key diagnostic and is silently ignored.

Verified 2026-07-22: `theorems` is not in `NATIVE_KEYS`; `SiteConfig` carries `python`/`r`
but no `theorems`; the site renders every page through
`render_document_with_includes_scoped(src, base, chapter)`, which has no book-config channel.

This is backlog **item 16 F-01** (the course-author persona finding). The demand is DRY: set
a theorem policy once for the book, let chapters inherit it.

## Non-goals / scope boundary

- No change to the theorem-numbering core (`number_theorems`), the numbering *scope* rule (a
  numbered chapter scopes its theorems; flat elsewhere), the BibTeX/CSL path, or the
  `MAX_WARM_PAGES` freeze.
- No new configuration *surface*: this exposes an existing knob (`theorems:`) at book scope,
  it does not invent a knob. (Minimal-config lens: a DRY win, the same pattern proposed for a
  future site-level `bibliography:`.)
- Not per-field merge (see Merge semantics): a chapter's `theorems:` replaces the book's
  wholesale, it does not inherit `numbered` while overriding `shared`. YAGNI until asked.
- The public core render API (`render_document_with_includes*`) stays byte-compatible; a
  standalone document renders identically to today.

## Design

Three parts, each independently testable.

### 1. Recognize + parse `theorems:` in `_site.yml`

- Add `"theorems"` to `NATIVE_KEYS` (`site/config/mod.rs:117`), so a book-level `theorems:`
  is honored rather than warned as unknown.
- Add a field `theorems: Option<TheoremConfig>` to `SiteConfig`. It is `Some` **only when
  `_site.yml` declares a `theorems:` key** (so the render fallback can tell "book set a
  policy" from "book said nothing"); `None` otherwise.
- Factor the existing `parse_theorem_config(front_matter: &str)` so the value-level logic is
  shared: extract `parse_theorem_config_value(value: &serde_yaml::Value) -> TheoremConfig`
  (the current `value.get("theorems")...` body), and have `parse_theorem_config` parse the
  front-matter string then delegate. `parse_native` sets
  `theorems: value.get("theorems").is_some().then(|| parse_theorem_config_value(value))`.
- `TheoremConfig` derives `Clone` (currently only `Default`) so it can be owned by
  `SiteConfig` and cloned into a render. Add `Debug, PartialEq` for test assertions.
- Validate the book-level block with the **existing** `frontmatter::validate_theorem_values`
  (`frontmatter.rs:325`), so a typo (`theorems: { numbred: true }`) or a bad `numbered:` value
  in `_site.yml` warns exactly as it does per-document. That validator takes
  `&serde_yaml::Mapping` + `&mut Vec<Warning>`; `parse_native` accumulates `Vec<String>`, so
  bridge by draining the `Warning`s into their message strings. (`validate_theorem_values`
  becomes `pub(crate)`.)

### 2. Merge semantics: whole-config override

The effective theorem config for a page is:

- the page's own `theorems:` block if its front-matter declares one, **else** the book's
  `_site.yml` `theorems:` (when set), **else** the default (per-kind, always-numbered).

"Declares one" = the page front-matter has a `theorems` key at all (not whether it parsed to
non-default), so `theorems: {}` still counts as an explicit page override to the default.
This is a clean whole-config override: predictable, and it covers the DRY use case (book sets
the policy; a chapter that needs a different one declares its own).

### 3. Threading (public API unchanged)

- Add `pub(crate) fn render_document_scoped_with_theorems(src, base_dir, chapter,
  book_theorems: Option<&TheoremConfig>) -> RenderedDoc`. The existing public
  `render_document_with_includes_scoped` delegates to it with `None` (byte-identical).
- Thread `book_theorems: Option<&TheoremConfig>` through the private chain:
  `render_doc_with_includes_impl` -> `render_internal` -> `render_internal_impl`. In
  `render_internal_impl`, replace `theorem_config = parse_theorem_config(fm)` with: if the
  page front-matter has no `theorems` key and `book_theorems` is `Some`, clone the book
  config; else parse the page's own. (`std::thread::scope` in `render_internal` carries the
  borrowed `&TheoremConfig` safely.)
- `Site` holds `config: SiteConfig` (`site/mod.rs:141`), so every site render call site passes
  `self.config.theorems.as_ref()`. The five call sites that must agree so numbering +
  `@thm-`/`@lem-` cross-refs stay consistent across page render, cross-page discovery, search
  index, and llms projection:
  - `site/mod.rs:649` (`render_page`)
  - `site/mod.rs:1028`, `site/mod.rs:1149` (discovery / build page render)
  - `site/llms.rs:174`
  - `site/search.rs:72` (`page_fragment` gains a `book_theorems` parameter, passed by its
    `Site` caller)

Standalone single-document render (server `render`/`build` of a lone `.tmd`) never sets
`book_theorems`, so it is unchanged.

## Testing (the regression net)

- **Unit (`site/config`)**: `_site.yml` with `theorems: { numbered: unless-unique }` parses to
  `Some`; absent `theorems:` parses to `None`; a typo'd sub-key warns via the shared validator.
- **Unit (`render`)**: `render_document_scoped_with_theorems` with a `book_theorems` and a page
  that has no `theorems:` uses the book config; a page that has its own overrides it; `None`
  book config = default. Assert on `TheoremConfig` (now `PartialEq`) or on rendered numbering.
- **Corpus pin**: a minimal book fixture (`_site.yml` sets a book-wide `theorems:` policy; one
  chapter inherits it, one chapter overrides). A test renders the pages via `Site` and asserts
  the inheriting chapter reflects the book policy (e.g. a lone theorem renders unnumbered under
  `numbered: unless-unique`) and the overriding chapter does not. Exact fixture location chosen
  in the plan (new `corpus/<name>/` book vs. extending an existing theorem fixture), verified
  not to disturb existing pins (`course.rs`, `read_book.rs`).
- **Mutation-check** each: drop the fallback branch -> the inheriting chapter reverts to the
  default policy -> the named test fails.

## Files

- `crates/core/src/render/fm_extract.rs` — `parse_theorem_config_value` factor-out;
  `TheoremConfig` derives `Clone, Debug, PartialEq`.
- `crates/core/src/frontmatter.rs` — `validate_theorem_values` -> `pub(crate)`.
- `crates/core/src/site/config/mod.rs` — `NATIVE_KEYS += "theorems"`; `SiteConfig.theorems`;
  parse + validate in `parse_native`.
- `crates/core/src/render/mod.rs` — `render_document_scoped_with_theorems` +
  `book_theorems` threaded through the private chain + the merge at the parse site.
- `crates/core/src/site/{mod.rs,llms.rs,search.rs}` — pass `self.config.theorems.as_ref()` at
  the five render call sites.
- `corpus/<book>/` + a pin test — the book-level-policy fixture.

## Risks / watch-items

- **Backward compatibility**: existing books have no `_site.yml theorems:`, so `book_theorems`
  is `None` and every current render/snapshot is unchanged. The one behavior change is exactly
  the intended one (a book that *adds* `theorems:` now propagates it).
- **Warning-type bridge**: `validate_theorem_values` emits `Warning`; site config collects
  `String`. Convert at the call boundary; do not change the validator's signature beyond
  visibility.
- **Consistency across surfaces**: if a render call site is missed, that surface (e.g. search
  index) would number theorems differently from the page. The five sites above are the full
  set of `render_document_with_includes_scoped` callers in `site/` (grep-verified 2026-07-22).
