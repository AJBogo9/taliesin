# De-template the branding: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans to implement
> this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the AI-generated-looking traits the 2026-09-23 branding audit confirmed, without
touching layout or UX: one reading face for all words, no colours nobody chose, one copy of the
mark in live tokens, a plain 404, and copy that states facts.

**Architecture:** Every change is a token, a CSS rule, an asset or a sentence. Each task writes or
rewrites the gate that pins its outcome first (red), makes the change (green), and leaves the
gate behind so the tell cannot come back. No new configuration, no new token, no layout change.

**Tech stack:** CSS compiled into the binary via `include_str!` (a `cargo build` is needed before
a change shows in a page), Rust tests in `crates/core`, vanilla JS (`mermaid.js`, `search.js`),
`.tmd` sources for the four sites.

**Spec:** the audit's findings and the judged remedies, summarised in the 2026-09-23 session
report. Workflow run `wf_4d300b74-cf4`: 21 confirmed tells, 13 refuted, remedies judged 2 to 1.

## Global constraints

- No layout or UX change. Line boxes of chrome bars stay within 0.1rem of today's.
- No new token, no new knob. Small serif text follows the existing byline idiom:
  `font: var(--tali-font-body); font-size: .85rem; line-height: 1.2;`
- ONE small size for every label: `.85rem/1.2`. It matches the old mono label's x-height
  (`.78rem x 0.5625 / 0.5156 = .851rem`), so bars keep their optical size (line box
  `1.02rem` against the old `1.014rem`).
- JetBrains Mono stays, for code (`pre`, `code`, `kbd`) and nothing else on a built page.
- Colour on a page means data. The palette, the ink and the three callout hues do not change.
- The dev UI (`crates/server/src/serve/mod.rs`, `web-client/client.js`) is preview-only and
  out of scope; `web-client/search.js` IS reader-facing and in scope.
- No commits and no branch switching: the author commits (global CLAUDE.md). Run
  `cargo fmt --all` as the LAST action after the final `.rs` edit (CLAUDE.local.md).
- Every test run prefixes `TALIESIN_PYTHON="$PWD/.venv/bin/python"`.
- No em or en dashes in any new prose (author's global writing rule).

## Review focus

1. **Dark mode legibility of labels.** Dark muted `#D0CCC3` stays (the `>= 9:1` pin holds);
   with the mono gone, only size marks the secondary register. Check a dark screenshot of the
   nav, TOC and footer; expected: labels read as secondary, not as body.
2. **Mobile nav and TOC at 375px.** Lowercase serif at 13.6px must not wrap the bar or clip.
3. **Callout kind word** (`Note`) now reads as a small bold serif label; an authored title
   stays body size. Both must still be distinguishable in one screenshot.
4. **Search hit visibility.** Ink at 16% replaces yellow; the hit must still be findable in a
   1.5 s flash, light and dark (body stays >= 9:1 on it, computed in Task 2).
5. **A `.tmd` with h4-h6** (the guide has some): they become body-size bold serif and must not
   read as prose paragraphs; check one guide page.

---

### Task 1: One reading face for every word

**Files:**
- Modify: `crates/core/assets/css/base.css` (skip link ~88-91, `a.btn` ~120-121, heading
  comment ~176-192, `h4, h5, h6` ~206-207, `.tali-title-meta` ~221-223, byline ~225-237,
  affiliations ~249-256, `th code` ~423-428, captions ~429-441, `thead th` ~442-451,
  callouts ~462-481, code-fold ~490-502, sidenote back-link ~573-577, sidenote reset ~610-613,
  mobile comment ~1037-1041, `#TOC` ~1004-1005)
- Modify: `crates/core/assets/css/site.css` (nav bar ~39, nav brand ~43-57, search kbd comment
  ~104-107, mobile TOC label ~222, footer ~243, listing comment ~259, card date ~350-351,
  book brand ~480-487, book part comment ~523, draft badge ~567-568, draft banner ~580)
- Modify: `crates/core/assets/css/tokens.css:20` (muted comment) and `:67-75` (the voice rule)
- Modify: `crates/core/assets/css/tokens-dark.css:3-8` (muted rationale)
- Modify: `web-client/search.js:60-61, 70-71`
- Modify: `site/_includes/three-scene.tmd:98-102` (fullscreen button style)
- Test: `crates/core/src/render/tests.rs` (new gate; rewrite ~6426-6505 and ~6785-6820; doc
  comment of `the_dark_muted_tier_is_not_a_lightness_mirror` ~3400)
- Modify: `crates/core/tests/tech_blog.rs:181` (comment only)

**Interfaces:** Produces the literal `font: var(--tali-font-body); font-size: .85rem; line-height: 1.2;`
that the gate below searches for. Consumes nothing.

- [ ] **Step 1: Write the failing gates.** In `tests.rs`, replace
  `a_table_header_speaks_in_the_machine_voice` with the two tests below, and rewrite the two
  machine-voice tests as shown.

```rust
/// Labels are set in the case they should read. A label the tool generates (a nav link, a
/// date, a column head, `Figure 1`, the callout kind) is the reading face, set small and
/// muted; it was tracked uppercase mono, which the 2026-09-23 audit confirmed as the
/// generated "technical minimal" label. Negative tracking on display sizes stays legal.
#[test]
fn no_reader_facing_sheet_uppercases_or_tracks_a_label() {
    for (name, css) in [("tokens.css", TOKENS_CSS), ("base.css", BASE_CSS), ("site.css", SITE_CSS)] {
        let l = css.to_ascii_lowercase();
        assert!(
            !l.contains("text-transform: uppercase"),
            "{name}: text-transform: uppercase. Set the string in the case it should read"
        );
        for seg in l.split("letter-spacing:").skip(1) {
            let v = seg.split(&[';', '}'][..]).next().unwrap_or("").trim();
            assert!(
                v == "normal" || v == "0" || v.starts_with('-'),
                "{name}: letter-spacing: {v}. Positive tracking only exists to open up caps"
            );
        }
    }
    let search = std::fs::read_to_string(repo_root().join("web-client/search.js"))
        .expect("search.js")
        .to_ascii_lowercase();
    assert!(
        !search.contains("text-transform:uppercase"),
        "search.js is reader-facing chrome on every site; its labels are not uppercased"
    );
}

/// Every label the tool writes is ONE small size of the reading face. `.85rem` matches the
/// retired mono label's x-height (.78rem x 0.5625 / 0.5156), so bars keep their height.
#[test]
fn the_tools_labels_are_the_reading_face_set_small() {
    const LABEL: &str = "font: var(--tali-font-body); font-size: .85rem; line-height: 1.2;";
    for (name, css, sel) in [
        ("base.css", BASE_CSS, ".tali-skip {"),
        ("base.css", BASE_CSS, "a.btn {"),
        ("base.css", BASE_CSS, ".tali-title-block .tali-title-meta {"),
        ("base.css", BASE_CSS, "thead th {"),
        ("base.css", BASE_CSS, ".callout-title.callout-kind {"),
        ("base.css", BASE_CSS, "details.tali-code-fold > summary {"),
        ("base.css", BASE_CSS, ".tali-affiliations {"),
        ("base.css", BASE_CSS, "#TOC {"),
        ("site.css", SITE_CSS, ":is(.tali-nav-inner, .tali-book-topbar-inner) {"),
        ("site.css", SITE_CSS, ".tali-foot-inner {"),
        ("site.css", SITE_CSS, ".tali-card-date {"),
    ] {
        let b = rule_block(css, sel);
        assert!(b.contains(LABEL), "{name} `{sel}` is not the label size of the reading face: `{b}`");
        assert!(!b.contains("--tali-font-mono"), "{name} `{sel}` still sets a label in the mono");
    }
    let h4 = rule_block(BASE_CSS, "h4, h5, h6 {");
    assert!(
        !h4.contains("--tali-font-mono") && h4.contains("font-size: 1em;"),
        "h4-h6 are authored headings: the serif at body size, bold from the shared rule. Got `{h4}`"
    );
}
```

  Rewrite `the_machine_voice_is_only_on_generated_labels_never_on_authored_text` as:

```rust
/// A callout title is the author's in two of its three branches (`divs.rs`: `title=`, then a
/// leading heading, then the capitalized kind word), so only the generated branch,
/// `.callout-kind`, is set as a label. An authored title is the body size.
#[test]
fn only_a_generated_callout_kind_is_set_as_a_label() {
    let base = rule_block(BASE_CSS, "\n  .callout-title {");
    assert!(
        base.contains("var(--tali-font-body)") && !base.contains(".85rem"),
        "an authored callout title is the serif at the body size. Got: `{base}`"
    );
    let kind = rule_block(BASE_CSS, ".callout-title.callout-kind {");
    assert!(
        kind.contains("font-size: .85rem;") && kind.contains("font-weight: 600;"),
        "the generated kind word is the label size, bold. Got: `{kind}`"
    );
    // The code-fold summary is one style now: `#| code-summary:` and the "Code" fallback read
    // the same, so the fallback has no rule of its own to drift.
    assert!(
        !BASE_CSS.contains("summary.tali-code-label {"),
        "the \"Code\" fallback needs no rule of its own"
    );
}
```

  Rewrite `a_caption_number_is_the_machine_voice_and_the_caption_is_not` as
  `a_caption_number_is_upright_inside_an_italic_caption`: keep its `render_html_page` span
  assertion and both `figcaption, table caption` assertions unchanged, and replace the two
  `.tali-caption-label { font: 400 .78rem/1.3 var(--tali-font-mono);` assertions with:

```rust
    assert!(
        BASE_CSS.contains(".tali-caption-label { font-style: normal; }"),
        "the generated number is the caption's own face, upright against the italic"
    );
```

- [ ] **Step 2: Run the gates and see them fail.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core --lib -- no_reader_facing_sheet the_tools_labels only_a_generated_callout_kind a_caption_number_is_upright`
  Expected: 4 FAILED (uppercase present, labels in mono, kind in mono, caption label in mono).

- [ ] **Step 3: Change the sheets.** Every rule below replaces
  `font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase; letter-spacing: .053em;`
  (or its near-copies) with `font: var(--tali-font-body); font-size: .85rem; line-height: 1.2;`,
  keeping every other declaration of the rule as it is:
  - base.css: `.tali-skip`, `a.btn`, `.tali-title-block .tali-title-meta`, `thead th` (then
    `font-weight: 600;` after it, replacing the deliberate 400; rewrite its comment: a column
    head is a label, set small and bold), `#TOC`.
  - base.css `.callout-title.callout-kind { font: var(--tali-font-body); font-size: .85rem; line-height: 1.2; font-weight: 600; }`
  - base.css `details.tali-code-fold > summary`: its `font: var(--tali-font-body); font-size: .78rem; font-weight: 400; line-height: 1.3; text-transform: none; letter-spacing: normal;`
    becomes the label literal; DELETE the `details.tali-code-fold > summary.tali-code-label { ... }` rule.
  - base.css `.tali-affiliations`: `font: var(--tali-font-body); font-size: .78rem; font-weight: 400; line-height: 1.3;` becomes the label literal.
  - base.css `.tali-title-block .tali-byline`: becomes `{ display: inline; }` (it inherits the
    meta line, which is now the serif); shorten its comment to the flex-item sentence.
  - base.css `h4, h5, h6 { font-size: 1em; line-height: 1.3; margin: var(--tali-u) 0 calc(.5 * var(--tali-u)); }`
    (family and 600 come from `h1, h2, h3, h4, h5, h6 { font-family: inherit; font-weight: 600; }`).
    Rewrite the heading-scale comment's two mono sentences: h4 is where the serif sizes stop,
    so h4-h6 are the body size in bold; their 2:1 spacing stays the scale's one stated exception.
  - base.css `.tali-caption-label { font-style: normal; }` and delete the comment's inheritance clause.
  - base.css `.tali-sidenote:target .tali-sidenote-back { display: inline; margin-left: .5em; }`
    (it inherits the sidenote's serif).
  - base.css: delete `th code { text-transform: none; letter-spacing: normal; }` and its comment.
  - base.css: delete the now-dead `text-transform: none; letter-spacing: normal;` pair in
    `.hero-eyebrow`, `figcaption, table caption`, `.callout-title`, `.tali-sidenote, .column-margin`,
    and trim each adjacent comment clause that explains inheriting tracked uppercase.
  - base.css mobile comment (~1040): "The label size does NOT move: .85rem is the label at every
    width, and 0.8 of it would be under 11px."
  - site.css: nav bar and footer rules as above; `.tali-card-date` as above; `.tali-nav-toc-label`
    loses `letter-spacing: .05em; text-transform: uppercase;` and takes `font-size: .85rem;`;
    `.tali-nav-brand` and `.tali-book-brand` lose `text-transform: none; letter-spacing: normal;`
    and their comments shrink to "Authored text (the site's `title:`) at the wordmark size.";
    `.tali-draft-badge` loses `letter-spacing: .04em; text-transform: uppercase;`;
    `.tali-draft-banner` loses `letter-spacing: .03em;`.
  - Replace every remaining "machine voice"/"machine-voice" phrase in the two sheets
    (`grep -n machine crates/core/assets/css/*.css`, excluding base.css:757 "machine-shaped
    content") with "label", and the search-kbd comment with "every other code glyph on the page".
  - tokens.css:20 comment `/* 6.68:1  — the machine voice */` becomes `/* 6.68:1, secondary text */`.
  - tokens.css:67-75 becomes:

```css
    /* TWO owned faces with separate jobs. Literata sets every WORD on the page, whoever wrote
       it: prose, headings, and the tool's own labels (nav, TOC, footer, dates, table heads,
       figure numbers, callout kinds), which are the same face at .85rem, muted, in the case
       the string already has. JetBrains Mono sets CODE and nothing else. Nothing is
       uppercased or tracked: the secondary register is carried by SIZE and position.
       (Until 2026-09-23 labels were the mono at .78rem, uppercase and tracked; the branding
       audit confirmed that as the generated "technical minimal" label, and it also broke the
       rule that the author's own words never take the tool's voice.) */
```

  - tokens-dark.css:3-8: replace "carried by face, size and tracking (it is the mono voice)
    rather than by lightness. So muted stays bright and still reads as secondary. That is a
    dividend of the theme's one rule, and it is why the usual dark-mode muted-grey trap does not
    apply." with "carried by SIZE rather than by lightness: every muted surface (labels,
    captions, affiliations) is also smaller than the body, so muted stays bright and still
    reads as secondary, and the usual dark-mode muted-grey trap does not apply."
  - tests.rs doc of `the_dark_muted_tier_is_not_a_lightness_mirror`: "carried by face, size
    and tracking" becomes "carried by size".
  - search.js:60-61: `...{color:var(--tali-link);font-size:.66rem;font-weight:700}` (drop
    `text-transform:uppercase;` and `letter-spacing:.05em;`); search.js:70-71:
    `...{color:var(--tali-muted);font-size:.78rem}` (drop the same pair).
  - three-scene.tmd:98-102: the button comment says "in the theme's own label face" and the
    three entries become `"font:var(--tali-font-body)", "font-size:.85rem", "line-height:1.2",`.
  - tech_blog.rs:181 comment: "and keeps the uppercase mono" becomes "and carries the marker class".

- [ ] **Step 4: Run the gates and the whole core suite.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core`
  Expected: PASS. Fix any other pin that named the old literal (search for `.78rem/1.3 var(--tali-font-mono)`).

---

### Task 2: Remove the colours nobody chose

**Files:**
- Modify: `crates/core/assets/css/tokens.css:22` (code ground)
- Modify: `crates/core/assets/css/base.css:10-40` (search highlight)
- Modify: `crates/core/assets/js/mermaid.js:9-47, 86-91` (theme, dead bridge, error banner)
- Modify: `docs/guide/using/theming.tmd:25-32`
- Test: `crates/core/src/render/tests.rs` (`no_vendor_default_colours_remain_anywhere_that_emits_colour`, new mermaid test)

- [ ] **Step 1: Extend the colour ban and add the diagram pin.** Append to `BANNED`:

```rust
        ("#f4f1eb", "the generated cream the white ground left behind in the code ground"),
        ("#fbf9f5", "the retired paper ground"),
        ("250, 204, 21", "Tailwind yellow-400, the search hit's old colour"),
        ("#6e5414", "the search hit's old hand-picked dark amber"),
        ("#c0392b", "Flat UI's pomegranate red, the old diagram error banner"),
```

  Add `"crates/core/assets/js/mermaid.js",` to the swept file list and change
  `assert_eq!(checked, 11, ...)` to `12`. Add:

```rust
/// Diagrams draw in Mermaid's own greyscale theme, so they read as ink on the page. They drew
/// Mermaid's stock lavender (`#ECECFF` nodes, `#9370DB` borders) because the CSS-variable
/// bridge they read had no definitions anywhere, left over from the cut `theme:` key.
#[test]
fn diagrams_draw_in_greyscale_with_no_dead_token_bridge() {
    let js = std::fs::read_to_string(repo_root().join("crates/core/assets/js/mermaid.js"))
        .expect("mermaid.js");
    assert!(js.contains("theme: dark ? 'dark' : 'neutral',"), "diagrams use Mermaid's greyscale theme");
    assert!(!js.contains("--tali-mermaid"), "the unread --tali-mermaid-* bridge is gone");
    let doc = std::fs::read_to_string(repo_root().join("docs/guide/using/theming.tmd"))
        .expect("theming.tmd");
    assert!(!doc.contains("--tali-mermaid"), "the guide no longer documents the dead knob");
}
```

- [ ] **Step 2: Run and see both fail.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core --lib -- no_vendor_default_colours diagrams_draw_in_greyscale`
  Expected: 2 FAILED (tokens.css ships #f4f1eb; mermaid.js still reads --tali-mermaid).

- [ ] **Step 3: Make the changes.**
  - tokens.css:22: `--tali-code-bg: #F4F4F4;            /* ink 5% over the white ground */`
    (0.05 x #22201A + 0.95 x #FFFFFF = 243.95, 243.85, 243.55, i.e. #F4F4F4).
  - base.css search highlight: light `rgba(250, 204, 21, .5)` (twice) and `rgba(250, 204, 21, .55)`
    become `rgba(34, 32, 26, .16)`; dark `#6e5414` (twice) and `rgba(250, 204, 21, .36)` become
    `rgba(234, 231, 224, .16)`; the `/* body 5.55:1 */` note becomes `/* body 9.8:1 */`; the two
    amber comments become: "A search hit is the ink at 16%: it is page state, not data, so it
    takes no hue. Body text stays 11.6:1 on it in light (#DCDBDA) and 9.8:1 in dark (#363531)."
  - mermaid.js: header comment becomes "mermaid bakes colours into the SVG at run() time, so a
    theme flip re-renders from the stashed source (dataset.src). Diagrams use Mermaid's own
    greyscale themes, `neutral` and `dark`, so they read as ink on either palette."; set
    `theme: dark ? 'dark' : 'neutral',`; delete `cs`, `get`, `map`, `vars`, the loop and the
    `themeVariables` line. Error banner:
    `'border:1px solid var(--tali-callout-important,#8B3A2E);border-radius:var(--tali-radius,2px);padding:.5em .75em;margin:.5em 0;' +`
    `'color:var(--tali-callout-important,#8B3A2E);background:color-mix(in srgb,var(--tali-callout-important,#8B3A2E) 8%,transparent);font-size:.9em';`
  - theming.tmd: the paragraph becomes "Mermaid bakes its colours into the SVG as it renders, so
    CSS cannot restyle a finished diagram. Taliesin draws diagrams in Mermaid's greyscale
    themes and re-renders them on a light/dark switch, so they read as ink on either palette
    with nothing to set."

- [ ] **Step 4: Run the core suite and the assets type-check.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core` then
  `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
  Expected: PASS, and tsc exits 0.

---

### Task 3: One mark, painted in live tokens

**Files:**
- Modify: `editor/vscode/icons/icon.png` (PLTE entry 0)
- Delete: `site/favicon.svg`; Modify: `site/_site.yml:13`
- Modify: `crates/core/src/render/page.rs:757` (comment)
- Test: `crates/core/src/render/tests.rs` (`the_marketplace_icon_is_the_mark_in_two_owned_colours`, sweep list, new favicon gate)

- [ ] **Step 1: Make the icon gate read the palette, and add the SVG gate.** In the icon test,
  replace the two consts with:

```rust
    let bytes = |hex: &str| -> [u8; 3] {
        let h = hex.trim_start_matches('#');
        [0, 2, 4].map(|i| u8::from_str_radix(&h[i..i + 2], 16).expect("hex"))
    };
    // READ, never spelled: a literal here is how #FBF9F5 outlived the ground it named.
    let paper = bytes(color_after(TOKENS_CSS, "--tali-bg:"));
    let ink = bytes(color_after(TOKENS_CSS, "--tali-fg:"));
```

  and the comparison with `*rgb == paper || *rgb == ink`. Add:

```rust
/// Every copy of the mark paints only a palette's ground and ink, read from the token files.
/// A deny list cannot catch a colour that was once valid and then retired.
#[test]
fn every_copy_of_the_mark_paints_only_a_palettes_ground_and_ink() {
    let allowed: Vec<String> = [
        (TOKENS_CSS, "--tali-bg:"),
        (TOKENS_CSS, "--tali-fg:"),
        (TOKENS_DARK_CSS, "--tali-bg:"),
        (TOKENS_DARK_CSS, "--tali-fg:"),
    ]
    .iter()
    .map(|(css, tok)| color_after(css, tok).to_ascii_lowercase())
    .collect();
    for rel in ["web-client/favicon.svg", "editor/vscode/icons/tmd.svg"] {
        let text = std::fs::read_to_string(repo_root().join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
            .to_ascii_lowercase();
        let mut seen = 0;
        for (i, _) in text.match_indices('#') {
            let h = text.get(i..i + 7).unwrap_or("");
            if h.len() == 7 && h[1..].chars().all(|c| c.is_ascii_hexdigit()) {
                seen += 1;
                assert!(allowed.iter().any(|a| a == h), "{rel} paints {h}, not a palette's ground or ink");
            }
        }
        assert!(seen >= 2, "{rel}: found {seen} colours, the scan broke");
    }
}
```

  In the vendor sweep, remove `"site/favicon.svg",` and set the count back to `11`.

- [ ] **Step 2: Run and see the icon gate fail.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core --lib -- the_marketplace_icon every_copy_of_the_mark`
  Expected: icon gate FAILS on `#FBF9F5`; the SVG gate passes (a regression pin). The sweep fails
  until the file is deleted.

- [ ] **Step 3: Repaint, delete the duplicate, fix the comment.**

```python
# run with system python3 from the repo root
import struct, zlib
p = "editor/vscode/icons/icon.png"
b = bytearray(open(p, "rb").read())
i = 8
while i < len(b):
    n = struct.unpack(">I", b[i:i+4])[0]; t = bytes(b[i+4:i+8])
    if t == b"PLTE":
        assert bytes(b[i+8:i+14]) == bytes([251,249,245,34,32,26]), b[i+8:i+14]
        b[i+8:i+11] = bytes([255,255,255])
        b[i+8+n:i+12+n] = struct.pack(">I", zlib.crc32(b[i+4:i+8+n]) & 0xffffffff)
    i += 12 + n
open(p, "wb").write(b)
```

  Delete `site/favicon.svg` (byte-identical to `web-client/favicon.svg`, which the build
  already inlines as the default favicon) and the `favicon: favicon.svg` line of
  `site/_site.yml`. page.rs:757: "(the block-model glyph)" becomes "(the T mark)".

- [ ] **Step 4: Run the core suite.** Expected: PASS.

---

### Task 4: The default 404 is a plain document

**Files:**
- Modify: `crates/core/src/site/mod.rs:942-963`
- Test: `crates/core/tests/tech_blog.rs:533-535, 584`

- [ ] **Step 1: Pin the absence.** tech_blog.rs:535 becomes
  `page.contains("Page not found") && !page.contains("tali-404-code"),` with message
  `"a plain 404 body: the heading, no decorative numeral"`; :584 becomes `page.contains(".tali-404{"),`.
- [ ] **Step 2: Run it and see it fail.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test -p taliesin-core --test tech_blog -- 404`
  Expected: FAIL (the numeral is present).
- [ ] **Step 3: Strip the stamp.** In `NOT_FOUND_STYLE` delete the `.tali-404-code{...}` rule,
  make `.tali-404 h1{margin:.4rem 0 0}` and `.tali-404-home{display:inline-block;margin-top:1.4rem}`.
  In the body delete `<div class=\"tali-404-code\">404</div>\n\` and replace the stock sentence
  with `<p>Nothing is published at this address.</p>\n\`. Above `NOT_FOUND_STYLE`, replace the
  comment with: "A special page is a short document, set by the reading surface's own elements
  and nothing else: no type scale of its own. The centred box is its only layout."
- [ ] **Step 4: Run the core suite.** Expected: PASS.

---

### Task 5: Copy states facts

**Files:** `site/index.tmd`, `docs/internals/index.tmd`, `docs/guide/index.tmd`,
`gallery/index.tmd`, `CLAUDE.md` (Conventions).

- [ ] **Step 1: site/index.tmd.**
  - Lines 14-17 become: "When you save, the page does not reload. The scene below keeps turning
    where you left it, and the Python kernel keeps the variables it has already computed."
  - `## One source, three shapes` becomes `## What it builds`; its paragraph becomes: "One `.tmd`
    file is a post. A directory with a `_site.yml` is a website, and a `chapters:` list in it
    makes a book. They share one renderer, so what you learn for one applies to the others."
  - Drop ` →` from the three feature links.
  - `## Edit the code. The output re-runs in place.` becomes `## Executed code`.
  - `## Three more things worth knowing` becomes `## Editing and publishing`.
  - `### Bundled, not fetched` becomes `### Works offline`; `### Output you can host anywhere`
    becomes `### Static output`; "It is the one bridge back: the `.tmd` stays the only place you
    edit." becomes "The `.tmd` stays the only place you edit."
  - The closing paragraph becomes: "This page and the [documentation book](https://guide.taliesin.sh/)
    are built by Taliesin from plain `.tmd` files in one repository."
- [ ] **Step 2: docs/internals/index.tmd.** "No flash, no reload, no cold start. **This book is
  how that works.**" becomes "This book explains how."; `## The one idea` becomes
  `## A Rust core with thin clients`; "Everything here follows from a single architectural
  choice: **all of the" becomes "**All of the"; delete "Nothing clever lives in the client.";
  "Three properties fall out of that design, and they are what the rest of this book explains in
  detail:" becomes "The rest of this book explains what follows from that design:".
- [ ] **Step 3: docs/guide/index.tmd.** ", and makes one trade: do one thing, author HTML, and make
  the edit loop instant." becomes " and outputs HTML only."; `## Three things it gets right` becomes
  `## The edit loop`; delete "That loop is the whole product; everything below is what you can put
  inside it."
- [ ] **Step 4: gallery/index.tmd.** "No screenshots, no mockups: open a demo and poke at the real
  thing. Even this index is dogfood: the cards below are a `listing:` block, built from each
  page's own front matter." becomes "The cards below are a `listing:` block, built from each
  page's own front matter."
- [ ] **Step 5: CLAUDE.md Conventions**, a new last bullet:
  "- **Copy states facts; it does not perform.** On the four sites and in scaffolded text: no
  "No X, no Y, no Z" closers, no "one idea" thesis reveals, no count in a heading ("Three
  things..."), no "→" appended to link text. A heading or a link names its subject or its
  destination. Deliberately ungated: prose linting was ruled out on 2026-09-23, so this holds by
  review."
- [ ] **Step 6: Lint the books.**
  Run: `TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo run -q -p taliesin-server -- build docs/guide --check-only`,
  the same for `docs/internals`, then `cargo test -p taliesin-core --test cross_site_links`.
  Expected: exit 0 each (no dangling anchor from a renamed heading).

---

### Task 6: Retire stale design sources

**Files:** `notes/superpowers/specs/2026-08-14-instrument-theme-design.md:4`, `notes/backlog.md:106-110`,
`site/README.md:38-40`, `crates/core/assets/css/tokens.css` (header).

- [ ] **Step 1:** Spec status becomes `**Status:** superseded. The live design is tokens.css,
  tokens-dark.css and the gates in crates/core/src/render/tests.rs; where this file disagrees
  with them, they win.`
- [ ] **Step 2:** backlog "Website / brand" bullet: replace "the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit
  KEEPs live in that file." with "that audit's \"Marginalia\" direction is superseded; the live
  design is `tokens.css` plus its gates in `render/tests.rs`."
- [ ] **Step 3:** site/README.md: delete the `**Closing CTA**` bullet (the page has none) and
  make line 40 "The theme is the Taliesin default: Literata for every word, JetBrains Mono for
  code, and the reader's OS picks light or dark."
- [ ] **Step 4:** tokens.css header, after "Keep in sync: nothing else should declare these
  tokens.": "Comments that cite \"spec §N\" point at
  notes/superpowers/specs/2026-08-14-instrument-theme-design.md, which is superseded: this file
  and the gates in render/tests.rs are the design."

---

### Task 7: Verify end to end

- [ ] `cargo fmt --all`, then `git diff --stat` and `git diff -- '*.rs'` to confirm no hunk
  outside the intended tests and `site/mod.rs`.
- [ ] `cargo build --release -p taliesin-server` (assets are compiled in).
- [ ] `cd web-client && npx -y -p typescript tsc -p jsconfig.json` and the assets tsc: exit 0.
- [ ] `ps -eo etimes,args | grep '[c]argo test'` is empty, then run
  `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` in the background and quote its
  verdict line.
- [ ] Build `site`, `docs/guide` and `gallery` to the scratchpad with the release binary, serve
  them, and screenshot through the chrome-devtools CLI: the landing page (hero, feature rows),
  a guide page with a table, a callout, h4 and the TOC, at 1440px light and dark, plus the
  landing page at 375px. Compare against the audit's before-shots for bar heights.
