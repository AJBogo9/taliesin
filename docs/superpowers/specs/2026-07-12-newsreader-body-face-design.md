# Newsreader as the owned body typeface (backlog #13)

**Status:** approved design, 2026-07-12 (owner decisions collected via AskUserQuestion). Target: a TDD implementation plan.

## Problem

The blog's body text uses a generic system-serif stack
(`--tali-font-body: 1.125rem/1.7 ui-serif, Georgia, "Times New Roman", serif`,
`base.css:35`). "Assembled from defaults" is the biggest remaining identity tell in the
2026-07-11 website audit. The project already bundles the **Newsreader** variable font
(OFL) Regular-only as a TTF, but only the OG-card rasterizer (`site/card.rs`, via
`ab_glyph`) uses it; the browser never sees it. Owner ruling: **promote Newsreader to the
body face with real bold + italic + bold-italic (never synthesize weights).**

## Goal

Make Newsreader the body typeface for HTML pages, with genuine weights and a true italic,
delivered fully offline (inlined, no network request), without disturbing the heading
face (sans), the OG-card path (TTF), math (KaTeX), or single-file self-containment.

## Owner decisions (2026-07-12)

- **Faces:** variable woff2, two files (roman + italic). Two `@font-face` rules cover
  regular / bold / italic / bold-italic via the real `wght` axis. No synthesized weights.
- **Delivery:** inline as `data:` URIs via the same `build.rs` step KaTeX uses, so every
  page stays self-contained/offline. The deferred item **#17** (already ruled) externalizes
  the whole shared bundle (KaTeX + body font + app CSS) for `build <dir>` later; #13 does
  not build a served-font path.

## Assets

Fetch from `@fontsource-variable/newsreader@5.2.10` (OFL, LICENSE shipped in-package),
already Latin-subset variable woff2 with a `wght` axis (no local subsetting tooling needed):

| file | bytes | axis |
|---|---|---|
| `newsreader-latin-wght-normal.woff2` | 58 084 | wght (roman) |
| `newsreader-latin-wght-italic.woff2` | 64 520 | wght (italic) |

Both saved to `crates/core/assets/fonts/`. The existing
`assets/fonts/Newsreader[opsz,wght].ttf` **stays** — `ab_glyph` (OG cards) needs a TTF and
cannot read woff2; the woff2 pair is additional, for the browser only. ~122 KB inlined
total, well under KaTeX's ~347 KB already inlined per page.

## Mechanism

1. **A dedicated, generated font stylesheet** (mirrors `katex-inlined.css`, keeps
   `base.css` a plain static file):
   - New source `crates/core/assets/css/fonts.css` holding the two `@font-face` rules that
     reference `url(fonts/newsreader-latin-wght-normal.woff2)` /
     `url(fonts/newsreader-latin-wght-italic.woff2)`:

     ```css
     @font-face {
       font-family: "Newsreader";
       font-style: normal;
       font-weight: 200 800;           /* the real wght axis range: no faux-bold */
       font-display: swap;
       src: url(fonts/newsreader-latin-wght-normal.woff2) format("woff2");
     }
     @font-face {
       font-family: "Newsreader";
       font-style: italic;
       font-weight: 200 800;
       font-display: swap;
       src: url(fonts/newsreader-latin-wght-italic.woff2) format("woff2");
     }
     ```
   - `build.rs` gains a second pass (generalizing its existing KaTeX pass): read
     `fonts.css`, replace each `url(fonts/<name>.woff2)` with
     `url(data:font/woff2;base64,<...>)` read from `assets/fonts/<name>`, and write
     `fonts-inlined.css` into `OUT_DIR`.
   - `render/mod.rs` gains `const FONTS_CSS: &str = include_str!(concat!(env!("OUT_DIR"),
     "/fonts-inlined.css"));` beside `KATEX_CSS`.

2. **Emit the font CSS before body CSS.** In `render/page.rs`, prepend `FONTS_CSS` to the
   page's inline `<style>` (before `base`), so the `@font-face` is defined before
   `--tali-font-body` uses it. The `@font-face` is inlined, so no page ships a bare
   `url(fonts/...)` that would 404.

3. **Point the body variable at Newsreader.** `base.css`:
   `--tali-font-body: 1.125rem/1.7 "Newsreader", ui-serif, Georgia, "Times New Roman", serif;`
   The system stack stays as the fallback if the face ever fails to load. Headings keep
   `--tali-font-head` (sans): the serif/sans pairing is an audit KEEP, so this change is
   body-only.

4. **THIRD_PARTY.md**: extend the existing Newsreader (OFL) entry — now the body face,
   italic added, source `@fontsource-variable/newsreader@5.2.10` (Latin subset, woff2), the
   Regular TTF retained for OG cards.

## Invariants preserved

- **Offline / self-contained:** the font is a `data:` URI in the inline `<style>`; zero
  network requests, single-file `build file.tmd` stays self-contained. Same property KaTeX
  already relies on.
- **OG cards unchanged:** `site/card.rs` keeps rasterizing from the TTF; woff2 is a
  separate, browser-only asset. The card corpus pins are untouched.
- **Math unchanged:** KaTeX has its own `KaTeX_*` families; the body-font change does not
  touch `--tali-font-*` used by `.katex`.
- **No new config knob:** the body face is automatic and correct-by-default ("perfect the
  default"). Reader text-size/spacing controls were already declined; this is not a knob.

## Testing

- **Unit (render/tests.rs or page test):** render a page to full HTML and assert the head
  `<style>` contains `@font-face` with `font-family:"Newsreader"`, both a `font-style:italic`
  and a normal rule, and an inlined `url(data:font/woff2;base64,` (proving the build-time
  inline ran, not a bare `url(fonts/`). Assert `--tali-font-body` names `"Newsreader"`.
- **Guard:** assert the emitted page contains **no** literal `url(fonts/newsreader` (every
  font url is a data URI; nothing to 404).
- **OG-card regression:** existing `site::card` tests stay green (TTF path untouched).
- **Browser:** on a real post, computed `font-family` of body prose is Newsreader; a
  `<strong>` renders a genuinely heavier weight and an `<em>` a true italic (spot-check
  `font-synthesis` is not doing the work); light + dark; desktop + mobile; KaTeX math still
  renders in its own face. Console clean.
- Full `cargo test -p taliesin-core` + clippy clean.

## Out of scope / non-goals

- No heading-face change (sans stays), no optical-size or weight config knob.
- No served-font path (that is #17's externalization, ruled separately).
- No change to the OG-card TTF or its rasterizer.
