# Instrument theme, Plan 1 of 4: the owned type and colour system

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Taliesin's borrowed type and colour with an owned system — two vendored
faces, two scored palettes, one owned syntax palette — and add the three static gates that
stop it drifting back.

**Architecture:** Everything here lands in the token layer and the five bundled stylesheets,
plus the tests that guard them. No component markup changes and no deletions of features: this
plan makes the *material* right, and Plans 2-4 reshape what is built from it. The page after
this plan looks different (serif headings, warm ground, new code colours, 67-character
measure) but has exactly the same anatomy.

**Tech Stack:** Rust (edition 2024), `include_str!`-bundled CSS, `crates/core/build.rs` woff2
data-URI inlining, `pyftsubset` from the repo `.venv` for font subsetting.

**Spec:** [docs/superpowers/specs/2026-08-14-instrument-theme-design.md](../specs/2026-08-14-instrument-theme-design.md)

## Global Constraints

- **Rust edition 2024, workspace resolver 3.** Shared deps go in the root `[workspace.dependencies]`.
- **Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before the change shows up.**
  They are `include_str!`-compiled; rebuilding only the site re-emits the *old* CSS.
- **Run `cargo fmt --all` LAST**, after every `.rs` edit — a `PostToolUse` hook runs `rustfmt`
  per-file and fights a mid-stream `cargo fmt`.
- **Never run two `cargo test` invocations against this workspace concurrently** — the second
  hangs the first. Kill stale runs before starting.
- **`./tools/gates.sh` must be green before and after.** Take the gate count from the script's
  own verdict line; never trust a count written in prose.
- **A push runs the pre-push hook, which needs `TALIESIN_PYTHON`.** Use
  `TALIESIN_PYTHON="$PWD/.venv/bin/python" git push …` or it hangs.
- **No browser-based gate.** A browser smoke test was decided against with evidence on
  2026-08-13 and is recorded in `notes/DO-NOT-REBUILD.md`. Every gate in this plan is static
  analysis of the bundled sources. This is why Task 3 pins the measure via the font binary
  rather than by rendering a page.
- **Never publish a number about the tool without a committed instrument.** A number without
  one carries its measured-on date.
- **Branch first.** `main` is the default branch; do not commit to it directly.
- Values are copied verbatim from the spec: paper `#FBF9F5`, ink `#22201A`, muted `#5F5C54`,
  code ground `#F4F1EB`, dark paper `#14130F`, dark ink `#EAE7E0`, dark muted `#D0CCC3`, dark
  code ground `#1C1A15`, body `1.25rem/1.55`, measure `32em`, `U = 1.9375rem`, radius `2px`.

---

## File structure

| File | Responsibility after this plan |
|---|---|
| `tools/subset-fonts.sh` | **new.** The reproducible instrument that produces the vendored woff2 from upstream. Without it the font bytes are unexplainable magic. |
| `crates/core/assets/fonts/literata-latin-wght-{normal,italic}.woff2` | **new.** Body + headings. |
| `crates/core/assets/fonts/jetbrains-mono-latin-wght-normal.woff2` | **new.** Code + the machine voice. |
| `crates/core/assets/fonts/newsreader-*.woff2` | **deleted.** |
| `crates/core/assets/css/fonts.css` | `@font-face` for the three faces. Consumed by `build.rs`'s data-URI inliner, which keys on `url(fonts/<name>.woff2)`. |
| `crates/core/assets/css/tokens.css` | The light palette + type + geometry + spacing tokens. One definition of each. |
| `crates/core/assets/css/tokens-dark.css` | The dark palette, designed independently. |
| `crates/core/assets/css/base.css` | Body type, leading, heading faces, the light syntax palette. |
| `crates/core/assets/css/dark.css` | The dark syntax palette only. |
| `crates/core/assets/css/site.css` | Chrome type; loses its `--tali-font-head` reads. |
| `crates/core/src/site/mod.rs` | Loses its one `--tali-font-head` read. |
| `crates/core/src/render/tests.rs` | The palette, measure and anti-drift gates. |
| `THIRD_PARTY.md` | Literata + JetBrains Mono in, Newsreader out. |

---

## Task 1: Vendor the two faces, reproducibly

**Files:**
- Create: `tools/subset-fonts.sh`
- Create: `crates/core/assets/fonts/literata-latin-wght-normal.woff2`, `literata-latin-wght-italic.woff2`, `jetbrains-mono-latin-wght-normal.woff2`
- Delete: `crates/core/assets/fonts/newsreader-latin-wght-normal.woff2`, `newsreader-latin-wght-italic.woff2`
- Modify: `crates/core/assets/css/fonts.css`, `THIRD_PARTY.md`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: the three font filenames above, referenced verbatim by `fonts.css` as
  `url(fonts/<name>.woff2)`; `crates/core/build.rs`'s `inline_woff2` rewrites those to
  `data:` URIs at build time and emits `$OUT_DIR/fonts-inlined.css`.

**Verified facts this task rests on** (the byte counts re-measured 2026-08-15 against the
files actually on disk after the `rvrn` re-vendor, which moved both Literata faces; the
2026-08-14 figures they replace — 47,700 / 48,732 / 116,200 / 6,404 — described the earlier
subset and were never corrected): Literata roman 48,072 B, italic 48,868 B, JetBrains Mono
19,768 B = **116,708 B**, against Newsreader's 122,604 B. The tool gains a monospace it has
never owned and ships **5,896 bytes less**. Dropping `calt` halves JetBrains Mono
(37,828 → 19,768 B).

- [ ] **Step 1: Write the failing test**

In `crates/core/src/render/tests.rs`:

```rust
/// The bundled faces are the two the theme owns, and nothing else. `fonts.css` must name
/// each one exactly as it sits on disk, because `build.rs`'s inliner matches the literal
/// `url(fonts/<name>.woff2)` and SILENTLY leaves an unmatched reference uninlined — which
/// ships a page that fetches a font that is not there.
#[test]
fn the_bundled_faces_are_literata_and_jetbrains_mono() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("assets/fonts")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "woff2").then(|| p.file_name()?.to_str().map(String::from))?
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "jetbrains-mono-latin-wght-normal.woff2".to_string(),
            "literata-latin-wght-italic.woff2".to_string(),
            "literata-latin-wght-normal.woff2".to_string(),
        ],
        "the bundled woff2 set changed"
    );
    for n in &names {
        assert!(
            FONTS_CSS_LINKED.contains(&format!("url(fonts/{n})")),
            "fonts.css does not reference {n} in the exact form build.rs inlines"
        );
    }
    assert!(
        !FONTS_CSS_LINKED.to_ascii_lowercase().contains("newsreader"),
        "Newsreader is retired; it must not be referenced"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_bundled_faces_are_literata_and_jetbrains_mono
```

Expected: FAIL — the directory still holds the two Newsreader files.

- [ ] **Step 3: Write the subsetting instrument**

Create `tools/subset-fonts.sh`:

```bash
#!/usr/bin/env bash
# Produce the vendored woff2 faces from upstream, reproducibly.
#
# The font bytes in assets/fonts/ are a MEASURED artifact, not a download: they are
# subset to Latin, stripped of hinting, and — for the mono — stripped of `calt`, which
# halves it (37,828 -> 19,768 B measured 2026-08-14) AND makes code ligatures
# impossible to re-enable by a stray CSS rule. Code ligatures misrepresent the source
# characters, so this is a correctness choice as much as a size one.
#
# Needs fontTools + brotli. fontTools is in .venv; brotli is not, so it is installed
# into a scratch dir rather than mutating the project venv.
#
#   tools/subset-fonts.sh            # rebuild assets/fonts/ from upstream
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

OUT=crates/core/assets/fonts
WORK=$(mktemp -d -t tali-fonts-XXXXXX)
trap 'rm -rf "$WORK"' EXIT

LATIN="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,\
U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD"

.venv/bin/pip install --quiet --target="$WORK/pylibs" brotli

fetch() { curl -sSLf -o "$WORK/$1" "$2"; }
sub() { # <in> <out> <layout-features>
    PYTHONPATH="$WORK/pylibs" .venv/bin/pyftsubset "$WORK/$1" \
        --output-file="$OUT/$2" --flavor=woff2 --unicodes="$LATIN" \
        --layout-features="$3" --no-hinting --desubroutinize
}

CDN=https://cdn.jsdelivr.net/npm
fetch lit.woff2      "$CDN/@fontsource-variable/literata/files/literata-latin-wght-normal.woff2"
fetch lit-it.woff2   "$CDN/@fontsource-variable/literata/files/literata-latin-wght-italic.woff2"
fetch jbm.woff2      "$CDN/@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2"

# onum/tnum/smcp are kept: the theme sets table figures tabular and uses small caps.
sub lit.woff2    literata-latin-wght-normal.woff2      "kern,ccmp,mark,mkmk,onum,tnum,smcp,liga"
sub lit-it.woff2 literata-latin-wght-italic.woff2      "kern,ccmp,mark,mkmk,onum,tnum,liga"
# NO calt, NO liga on the mono. See the header.
sub jbm.woff2    jetbrains-mono-latin-wght-normal.woff2 "kern,ccmp,mark,mkmk"

echo "vendored:"
for f in "$OUT"/*.woff2; do printf "  %8s  %s\n" "$(stat -c%s "$f")" "$(basename "$f")"; done
```

Then:

```bash
chmod +x tools/subset-fonts.sh
git rm crates/core/assets/fonts/newsreader-latin-wght-normal.woff2 \
       crates/core/assets/fonts/newsreader-latin-wght-italic.woff2
tools/subset-fonts.sh
```

- [ ] **Step 4: Rewrite `fonts.css`**

Replace the whole file with:

```css
/* The two owned faces. Both SIL OFL 1.1 with NO Reserved Font Name (checked 2026-08-14),
   so they may be subset and vendored without renaming. Produced by tools/subset-fonts.sh;
   do not hand-edit the binaries. The `url(fonts/…)` refs are rewritten to inlined `data:`
   URIs at build time by build.rs. `font-weight: 200 900` exposes the real wght axis, so
   bold is a genuine weight and never a synthesized faux-bold.

   Literata is the body AND the headings: this theme owns no sans. It was chosen on a
   measurement, not taste — KaTeX renders at a fixed 1.21em, so the body face's x-height
   decides whether math towers over the prose. Literata's math/body ratio is 1.027 against
   a 1.08 ceiling; the Newsreader this replaces was 1.168. */
@font-face {
  font-family: "Literata";
  font-style: normal;
  font-weight: 200 900;
  font-display: swap;
  src: url(fonts/literata-latin-wght-normal.woff2) format("woff2");
}
@font-face {
  font-family: "Literata";
  font-style: italic;
  font-weight: 200 900;
  font-display: swap;
  src: url(fonts/literata-latin-wght-italic.woff2) format("woff2");
}
/* Subset WITHOUT `calt`, so code ligatures cannot be re-enabled by a stray rule. */
@font-face {
  font-family: "JetBrains Mono";
  font-style: normal;
  font-weight: 100 800;
  font-display: swap;
  src: url(fonts/jetbrains-mono-latin-wght-normal.woff2) format("woff2");
}
```

- [ ] **Step 5: Update `THIRD_PARTY.md`**

Remove the Newsreader entry; add both faces with their exact copyright lines, verified
2026-08-14:

```markdown
- **Literata** — SIL OFL 1.1, no Reserved Font Name.
  `Copyright 2017 The Literata Project Authors (https://github.com/googlefonts/literata)`
  Subset by `tools/subset-fonts.sh`.
- **JetBrains Mono** — SIL OFL 1.1, no Reserved Font Name.
  `Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)`
  Subset by `tools/subset-fonts.sh`, `calt` removed.
```

- [ ] **Step 6: Run the test and the third-party gate**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core the_bundled_faces_are_literata_and_jetbrains_mono
cargo test -p taliesin-core --test third_party
```

Expected: both PASS. If `third_party` fails, it is asserting the manifest against the tree —
read its message and make `THIRD_PARTY.md` match.

- [ ] **Step 7: Commit**

```bash
git add tools/subset-fonts.sh crates/core/assets/fonts crates/core/assets/css/fonts.css \
        THIRD_PARTY.md crates/core/src/render/tests.rs
git commit -m "feat(theme): vendor Literata + JetBrains Mono, retire Newsreader

Literata replaces Newsreader on a measurement, not taste: KaTeX renders at a fixed
1.21em, so the body face's x-height decides whether math towers over the prose.
Measured math/body ratio 1.027 (Literata) against 1.168 (Newsreader), ceiling 1.08.

The tool also gains a monospace it has never owned and still ships 6,404 bytes LESS
font data (116,200 vs 122,604), because tools/subset-fonts.sh drops calt from the
mono — which halves it and makes code ligatures un-re-enableable."
```

> **Correction, 2026-08-15.** The commit above shipped with those two figures and they are
> wrong; the message is left as written because it is what was committed. The `rvrn` fix in
> the following commit re-subset both Literata faces, so the real totals are 116,708 B against
> Newsreader's 122,604 B — **5,896 bytes less**, not 6,404. See the corrected "Verified facts"
> block at the top of this task.

---

## Task 2: The type tokens, and the death of `--tali-font-head`

**Files:**
- Modify: `crates/core/assets/css/tokens.css`
- Modify: `crates/core/assets/css/base.css`, `site.css`, `crates/core/src/site/mod.rs` (the 20 `--tali-font-head` reads)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: the `"Literata"` and `"JetBrains Mono"` family names from Task 1's `fonts.css`.
- Produces: `--tali-font-body`, `--tali-font-mono`, `--tali-measure`, `--tali-u`,
  `--tali-radius`, `--tali-dur`. **`--tali-font-head` no longer exists.**

- [ ] **Step 1: Write the failing test**

```rust
/// The theme owns TWO faces and no more. `--tali-font-head` was `ui-sans-serif, system-ui`
/// with 20 reads across four files: headings and chrome rendered in whatever the reader's OS
/// shipped, so the page had a different voice on every platform and two of its three voices
/// were not the tool's. Headings now take the body serif; labels take the mono.
#[test]
fn the_theme_owns_exactly_two_faces_and_no_system_ui() {
    for (name, css) in [
        ("tokens.css", TOKENS_CSS),
        ("tokens-dark.css", TOKENS_DARK_CSS),
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
        ("site.css", SITE_CSS),
    ] {
        assert!(
            !css.contains("--tali-font-head"),
            "{name} still reads --tali-font-head; headings take the body serif now"
        );
        assert!(
            !css.contains("system-ui"),
            "{name} still names system-ui; the theme owns its faces"
        );
    }
    assert!(TOKENS_CSS.contains(r#"--tali-font-body: 1.25rem/1.55 "Literata""#));
    assert!(TOKENS_CSS.contains(r#"--tali-font-mono: "JetBrains Mono""#));
}

/// The geometry and motion scales must describe what the sheets actually contain. The old
/// token file advertised "three roundness tiers, three elevation shadows, two motion
/// durations" while the sheets held three radii, ONE shadow and ONE duration.
#[test]
fn the_geometry_scale_is_one_radius_no_shadows_one_duration() {
    assert_eq!(TOKENS_CSS.matches("--tali-radius").count(), 1);
    assert!(!TOKENS_CSS.contains("--tali-shadow"));
    assert!(!TOKENS_CSS.contains("--tali-dur-slow"));
    for (name, css) in [("base.css", BASE_CSS), ("site.css", SITE_CSS)] {
        assert!(!css.contains("box-shadow"), "{name} still draws a box-shadow");
        assert!(
            !css.contains("backdrop-filter"),
            "{name} still blurs a sticky bar"
        );
    }
}
```

- [ ] **Step 2: Run and watch both fail**

```bash
cargo test -p taliesin-core the_theme_owns_exactly_two_faces_and_no_system_ui \
                            the_geometry_scale_is_one_radius_no_shadows_one_duration
```

Expected: FAIL — `--tali-font-head` is present, as are shadows and the blur.

- [ ] **Step 3: Rewrite the type, geometry and spacing block of `tokens.css`**

Replace the font/geometry/motion declarations (keep the palette for Task 4):

```css
    /* TWO owned faces, and the rule that generates the whole system: the mono is the
       MACHINE's voice, the serif is the AUTHOR's. Everything the tool says — labels,
       figure and table numbers, table headers, callout kinds, nav, TOC, cell timings —
       is the mono at .78rem uppercase, tracked. Everything the author wrote is Literata.
       There is no third family and no `--tali-font-head`: headings take the body serif.
       Consequence worth stating, because it is load-bearing in dark mode: the secondary
       register is carried by FACE, SIZE and TRACKING, not by lightness, which is why the
       dark muted tier can stay bright without collapsing into the body. */
    --tali-font-body: 1.25rem/1.55 "Literata", Georgia, serif;
    --tali-font-mono: "JetBrains Mono", ui-monospace, monospace;
    /* The mono sits at .92em so its x-height MATCHES Literata's. Derived, not chosen:
       0.5156 / 0.5625, both measured from the binaries 2026-08-14. Re-derive it if
       either face changes. */
    --tali-mono-size: .92em;

    /* The measure is expressed in `em` of the BODY face and never in `ch`: `ch` is the
       advance of the digit `0`, which overshoots real lowercase by 12-55% depending on
       family, so the same `65ch` is 73 characters in one face and 101 in another. At
       1.25rem this is 640px = 67 real characters (measured; see the measure gate). */
    --tali-measure: 32em;
    --tali-chrome-maxw: 56rem;

    /* The vertical unit IS the line box: U = 1.55 x 1.25rem. Every vertical space is
       {0.5U, U, 2U, 3U} and nothing else, replacing 39 ad-hoc rem values. This is also
       why a 4px lattice cannot be reintroduced piecemeal: 31px is not a multiple of 4. */
    --tali-u: 1.9375rem;

    /* ONE radius, on interactive objects only (copy button, search input, kbd, focus
       ring). Structure is square. NOTE, honestly: no experiment has ever tested
       border-radius; this is inference from measured convention among admired
       interfaces, and it is the weakest-evidenced number in the theme. */
    --tali-radius: 2px;
    --tali-focus: 2px solid var(--tali-fg);
    --tali-focus-offset: 2px;
    /* One duration. Hover may change an underline or a ground; it may not MOVE anything. */
    --tali-dur: .1s;
```

Delete outright: `--tali-radius-sm/-md/-lg`, `--tali-shadow-sm/-md/-lg`, `--tali-dur-slow`,
`--tali-maxw` (replaced by `--tali-measure`), `--tali-font-head`.

- [ ] **Step 4: Repoint every `--tali-font-head` read**

```bash
grep -rn "tali-font-head" crates/core/assets/css crates/core/src
```

There are 20. Each is one of two cases, and the choice is not stylistic:

- **A heading or a title** → delete the declaration; it inherits Literata from `body`.
- **A label, caption number, nav item, TOC entry, table header, meta line, chip, badge or
  button** → it is the machine speaking, so:

```css
  font: 400 .78rem/1.3 var(--tali-font-mono);
  text-transform: uppercase;
  letter-spacing: .053em;
```

**Do not apply the machine voice to `figcaption` body text, sidenotes or margin notes.**
Those are authored prose and stay in the serif; only a caption's `Figure N` *number* takes
the mono. (This is a correction from a render: whole captions in mono read as terminal
output.) Plan 2 rebuilds those components; here, just leave them on the serif.

Also delete, in the same pass, every `box-shadow` declaration, both
`backdrop-filter: saturate(1.4) blur(9px)` rules, and `.tali-card:hover`'s
`transform: translateY(-2px)`.

- [ ] **Step 5: Repoint the body type in `base.css`**

```css
  body { max-width: var(--tali-measure); margin: calc(2 * var(--tali-u)) auto; padding: 0 1rem;
         font: var(--tali-font-body); color: var(--tali-fg); background: var(--tali-bg);
         overflow-wrap: break-word;
         font-feature-settings: "liga" 1, "calt" 1, "kern" 1;
         -webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale; }
  body p, body li, body dd, body blockquote, body figcaption { line-height: 1.55; }
  p { margin: 0 0 var(--tali-u); }
  h1, h2, h3, h4, h5, h6 { font-family: inherit; font-weight: 600; }
  h1 { font-size: 2rem; line-height: 1.1; letter-spacing: -.008em; margin: 0 0 calc(.5 * var(--tali-u)); }
  h2 { font-size: 1.35rem; line-height: 1.2; margin: calc(2 * var(--tali-u)) 0 calc(.5 * var(--tali-u)); }
  h3 { font-size: 1.12rem; line-height: 1.25; margin: calc(1.5 * var(--tali-u)) 0 calc(.4 * var(--tali-u)); }
  h4 { font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase;
       letter-spacing: .053em; margin: var(--tali-u) 0 calc(.3 * var(--tali-u)); }
```

Code must not inherit the ligature settings above:

```css
  pre, code { font-family: var(--tali-font-mono); font-size: var(--tali-mono-size);
              font-feature-settings: "kern" 1; font-variant-ligatures: none; }
```

- [ ] **Step 6: Run the tests**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core the_theme_owns_exactly_two_faces_and_no_system_ui \
                            the_geometry_scale_is_one_radius_no_shadows_one_duration
cargo test -p taliesin-core
```

Expected: the two new tests PASS. **`every_tali_custom_property_read_is_defined_somewhere`
will now fail if any sheet still reads a deleted token** — that is the gate doing its job;
fix each read it names. Expect `no_stray_15s_duration_outside_the_motion_scale` and
`overlay_backdrops_share_the_scrim_token` to need updating for the new scale.

- [ ] **Step 7: Commit**

```bash
git add crates/core/assets/css crates/core/src
git commit -m "feat(theme): two owned faces, one geometry scale

Deletes --tali-font-head and its 20 reads. It was ui-sans-serif/system-ui, so
headings and chrome rendered in whatever the reader's OS shipped: the page had a
different voice per platform and two of its three voices were not the tool's.
Headings take the body serif; labels take the mono, which is the rule the whole
theme is generated from.

Also collapses three radius tiers to one 2px, deletes three shadow tokens (two had
zero consumers), deletes --tali-dur-slow (zero consumers), and removes both
backdrop-filter rules. The token file previously advertised three shadows and two
durations while the sheets held one of each."
```

---

## Task 3: Pin the measure in characters, not in CSS

**Files:**
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-measure` from Task 2; the font bytes from Task 1.
- Produces: `LITERATA_MEAN_ADVANCE_EM`, a dated constant other tasks may read.

**Why this exists.** `--tali-measure` is in `em` of the body face, so the *rendered* line
length is a property of the font binary, not of the stylesheet. A face swap moves the layout
~21% with every CSS assertion still green. A browser gate is out (settled 2026-08-13), so this
pins the three things together: the font file's bytes, the advance measured from it, and the
arithmetic. **Swap the face and the hash changes, which forces a re-measurement.**

- [ ] **Step 1: Write the failing test**

```rust
/// Mean advance of English lowercase text in Literata, in `em`.
///
/// MEASURED 2026-08-14 in Chrome 141 at 20px: a 640px column rendered 67 characters per
/// line, so 640 / 67 / 20 = 0.4776 em per character. The sample was ordinary English prose
/// (including its spaces), not an alphabet run — an alphabet run overstates density,
/// because English is rich in narrow letters.
///
/// This is a measurement, so it carries its date. The font hash below is what makes it
/// re-measurable rather than merely asserted: change the face and this test fails, which
/// is the point.
const LITERATA_MEAN_ADVANCE_EM: f64 = 0.4776;

/// The measure is pinned in CHARACTERS, because that is what a reader experiences and it is
/// what WCAG 1.4.8 bounds. Before this theme the column was 46rem: measured 96 characters of
/// capacity with filled paragraphs at 80-92, past the 80-character AAA ceiling and far past
/// the comprehension-optimal band.
#[test]
fn the_measure_is_sixty_to_seventy_characters() {
    let em: f64 = {
        let d = TOKENS_CSS
            .split("--tali-measure:")
            .nth(1)
            .expect("--tali-measure is defined in tokens.css");
        let v = d.split(';').next().unwrap().trim();
        v.trim_end_matches("em")
            .parse()
            .unwrap_or_else(|_| panic!("--tali-measure must be in `em`, got `{v}`"))
    };
    let chars = em / LITERATA_MEAN_ADVANCE_EM;
    assert!(
        (62.0..=72.0).contains(&chars),
        "the measure renders {chars:.1} characters; keep it in 62..=72 \
         (WCAG 1.4.8 caps at 80). Either --tali-measure or the body face moved."
    );
}

/// The advance constant above describes ONE font binary. If the binary changes, the constant
/// is stale and the measure test is measuring nothing. Hash the file so a swap cannot be
/// silent.
#[test]
fn the_body_face_is_the_one_the_measure_was_measured_on() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/fonts/literata-latin-wght-normal.woff2");
    let bytes = std::fs::read(&p).expect("the vendored body face");
    // FNV-1a: no dependency, and collision resistance is irrelevant here — this only has
    // to notice that somebody replaced the file.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    assert_eq!(
        (bytes.len(), h),
        (47_700, 0x0), // <- replace 0x0 with the value the first run prints
        "the body face changed. Re-measure LITERATA_MEAN_ADVANCE_EM in a browser \
         (render a paragraph, divide column width by realized characters per line, \
         divide by font-size), update it and this hash together, and re-date both."
    );
}
```

- [ ] **Step 2: Run it and read the real hash out of the failure**

```bash
cargo test -p taliesin-core the_body_face_is_the_one_the_measure_was_measured_on -- --nocapture
```

Expected: FAIL, printing the actual `(len, hash)` tuple. Paste that tuple into the assertion,
replacing the `0x0` placeholder. (This is the one place a placeholder is correct: the value is
an output of the build, not a decision.)

- [ ] **Step 3: Run both tests**

```bash
cargo test -p taliesin-core the_measure_is_sixty_to_seventy_characters \
                            the_body_face_is_the_one_the_measure_was_measured_on
```

Expected: both PASS. `32.0 / 0.4776 = 67.0` characters.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/render/tests.rs
git commit -m "test(theme): pin the measure in characters, tied to the font binary

--tali-measure is in em of the body face, so the rendered line length is a property
of the font file and not of the CSS: a face swap moves layout ~21% with every CSS
assertion still green. This pins the bytes, the measured advance and the arithmetic
together, so changing the face forces a re-measurement.

No browser is involved: a browser smoke test was decided against 2026-08-13."
```

---

## Task 4: The two palettes

**Files:**
- Modify: `crates/core/assets/css/tokens.css`, `tokens-dark.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `--tali-bg`, `--tali-fg`, `--tali-muted`, `--tali-code-bg`, `--tali-border`,
  `--tali-border-strong`, `--tali-inline-code`, and the three callout tokens. Plans 2-4 read
  all of these.

- [ ] **Step 1: Write the failing test**

```rust
/// Every text colour the theme ships is scored, in both palettes. The floors are WCAG 2.x:
/// 4.5:1 for text, 3:1 for a control boundary. A decorative separator is deliberately below
/// both — it is not a control and not text.
///
/// APCA is deliberately absent. It is a different model on the WCAG 3 research track, and a
/// guessed Lc is worse than an absent one.
#[test]
fn every_text_colour_is_scored_in_both_palettes() {
    for (theme, css, bg, code_bg) in [
        ("light", TOKENS_CSS, "#fbf9f5", "#f4f1eb"),
        ("dark", TOKENS_DARK_CSS, "#14130f", "#1c1a15"),
    ] {
        let on_page = |tok: &str| wcag_contrast(color_after(css, tok), bg);
        for (tok, floor) in [
            ("--tali-fg:", 7.0),
            ("--tali-muted:", 4.5),
        ] {
            let c = on_page(tok);
            assert!(
                c >= floor,
                "{theme}: {tok} is {c:.2}:1 on the page, needs {floor}:1"
            );
        }
        let inline = wcag_contrast(color_after(css, "--tali-inline-code:"), code_bg);
        assert!(inline >= 4.5, "{theme}: inline code is {inline:.2}:1 on the code ground");
        let strong = on_page("--tali-border-strong:");
        assert!(
            strong >= 3.0,
            "{theme}: --tali-border-strong is {strong:.2}:1; a control boundary needs 3:1"
        );
    }
}

/// The dark palette is DESIGNED, not inverted. The tell of an inversion is a muted tier that
/// mirrors the light one's lightness; here muted stays bright on purpose, because in this
/// theme the secondary register is carried by face, size and tracking rather than by
/// lightness. Assert it did not drift dark.
#[test]
fn the_dark_muted_tier_is_not_a_lightness_mirror() {
    let c = wcag_contrast(color_after(TOKENS_DARK_CSS, "--tali-muted:"), "#14130f");
    assert!(
        c >= 9.0,
        "dark muted is only {c:.2}:1. A mirrored muted grey lands near 6.7:1 and reads as \
         dimmed rather than as a different voice."
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p taliesin-core every_text_colour_is_scored_in_both_palettes \
                            the_dark_muted_tier_is_not_a_lightness_mirror
```

Expected: FAIL — `--tali-inline-code` does not exist yet and the grounds are still white.

- [ ] **Step 3: Write the light palette**

In `tokens.css`, replace the palette block:

```css
    /* The ground is a warm off-white and the ink a warm near-black, so nothing on screen
       sits at either end of the luminance range. Stated honestly: this is a brand and
       preference choice. The most recent controlled test of tinted grounds found no
       reading-performance effect in English, so it must never be defended as legibility,
       and never with blue light. Every value carries its measured WCAG ratio.

       There is NO chrome accent. Colour on a Taliesin page means DATA: a plot, a syntax
       token, an error. Links are the ink with a hairline underline. */
    --tali-bg: #FBF9F5;                 /* paper */
    --tali-fg: #22201A;                 /* 15.49:1 */
    --tali-muted: #5F5C54;              /* 6.35:1  — the machine voice */
    --tali-inline-code: #3A362E;        /* 11.43:1 on the code ground */
    --tali-code-bg: #F4F1EB;            /* ink 4% over paper */
    --tali-border: #D9D7D2;             /* 1.37:1 — a decorative separator, not a control */
    --tali-border-strong: #8B887F;      /* 3.37:1 — clears the 3:1 non-text floor */
    --tali-link: #22201A;
    --tali-underline: #A8A49B;
    --tali-flash: rgba(34, 32, 26, .08);
    --tali-scrim: rgba(0, 0, 0, .42);
    color-scheme: light;
    /* Three callout kinds; the left rule and the mono kind-word carry the meaning.
       `caution` is deleted (zero consumers, kept alive only by a test looping over five). */
    --tali-callout-note: #5F5C54;
    --tali-callout-tip: #3F6152;
    --tali-callout-warning: #7A4A18;
```

- [ ] **Step 4: Write the dark palette**

Replace `tokens-dark.css` entirely:

```css
  /* The dark palette is DESIGNED, not inverted.

     Its muted tier is nearly as bright as the body, and that is correct here: a muted grey
     dark enough to LOOK secondary fails perceptual contrast, and in this theme the secondary
     register is carried by face, size and tracking (it is the mono voice) rather than by
     lightness. So muted stays bright and still reads as secondary. That is a dividend of the
     theme's one rule, and it is why the usual dark-mode muted-grey trap does not apply.

     The ground is #14130F rather than #000000: pure black buys almost nothing and adds
     halation. */
  html[data-theme="dark"] {
    --tali-bg: #14130F;
    --tali-fg: #EAE7E0;                 /* 15.05:1 */
    --tali-muted: #D0CCC3;              /* 11.60:1 */
    --tali-inline-code: #DBD7CE;        /* 12.94:1 on the dark code ground */
    --tali-code-bg: #1C1A15;
    --tali-border: #33312B;             /* 1.43:1 decorative */
    --tali-border-strong: #7C7972;      /* 4.28:1 */
    --tali-link: #EAE7E0;
    --tali-underline: #6E6B63;
    --tali-flash: rgba(234, 231, 224, .08);
    --tali-callout-note: #D0CCC3;
    --tali-callout-tip: #8FBBA3;
    --tali-callout-warning: #D0A67C;
    color-scheme: dark;
  }
```

- [ ] **Step 5: Run the tests**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core every_text_colour_is_scored_in_both_palettes \
                            the_dark_muted_tier_is_not_a_lightness_mirror \
                            callout_family_meets_its_contrast_floors_in_every_theme
```

Expected: the two new tests PASS. `callout_family_…` will FAIL because it loops over five
kinds — edit its list to the three that now exist and update its hard-coded `bg`/`fg` pair to
`#FBF9F5`/`#22201A` and `#14130F`/`#EAE7E0`.

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/css/tokens.css crates/core/assets/css/tokens-dark.css \
        crates/core/src/render/tests.rs
git commit -m "feat(theme): two scored palettes, no chrome accent

The ground becomes a warm off-white and the ink a warm near-black; there is no accent
colour in the chrome at all, because colour on a Taliesin page now means data.

The dark palette is designed rather than inverted, and its muted tier stays bright on
purpose: the secondary register here is face, size and tracking, not lightness, so a
dimmed grey would be both harder to read and redundant.

Every text colour ships with a computed WCAG ratio and a test that recomputes it."
```

---

## Task 5: The owned syntax palette

**Files:**
- Modify: `crates/core/assets/css/base.css` (light scopes), `dark.css` (dark scopes)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-code-bg` from Task 4.
- Produces: the six `.tali-hl-*` scope colours in both palettes.

**Why.** The twelve scope colours are GitHub Primer Light/Dark Default, wholesale — inside a
project whose own test bans four borrowed blues as "the single loudest *assembled from
framework defaults* tell". On a page that is half code, the most prominent colour on screen
belongs to another company.

- [ ] **Step 1: Write the failing test**

```rust
/// The syntax palette is OWNED. Four hues in one warm-anchored chroma envelope, mapped onto
/// the six syntect scopes, every one scored on the code ground it actually sits on. Comments
/// are italic as well as coloured, so hue is never the only cue.
#[test]
fn the_syntax_palette_is_owned_and_scored() {
    const GITHUB_PRIMER: &[&str] = &[
        "#646b74", "#0a3069", "#cf222e", "#0550ae", "#8250df", "#953800", // light
        "#8b949e", "#a5d6ff", "#ff7b72", "#79c0ff", "#d2a8ff", "#ffa657", // dark
    ];
    for (name, css) in [("base.css", BASE_CSS), ("dark.css", DARK_CSS)] {
        let lower = css.to_ascii_lowercase();
        for hex in GITHUB_PRIMER {
            assert!(
                !lower.contains(hex),
                "{name} still ships GitHub Primer's {hex}; the syntax palette is owned now"
            );
        }
    }
    for (theme, css, bg) in [("light", BASE_CSS, "#f4f1eb"), ("dark", DARK_CSS, "#1c1a15")] {
        for scope in [
            "tali-hl-comment",
            "tali-hl-string",
            "tali-hl-keyword",
            "tali-hl-constant",
            "tali-hl-entity",
        ] {
            let c = wcag_contrast(color_after(css, scope), bg);
            assert!(
                c >= 4.5,
                "{theme}: .{scope} is {c:.2}:1 on the code ground, needs 4.5:1"
            );
        }
    }
    assert!(
        BASE_CSS.contains(".tali-hl-comment { color: #6E6A60; font-style: italic; }"),
        "the comment scope must stay italic: hue is never the only cue"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p taliesin-core the_syntax_palette_is_owned_and_scored
```

Expected: FAIL on the first Primer hex.

- [ ] **Step 3: Write the light scopes in `base.css`**

Replace the existing `.tali-hl-*` block:

```css
  /* Owned syntax palette: four hues sharing one warm-anchored chroma envelope, replacing
     twelve borrowed GitHub Primer hexes. Every value scored on the code ground it sits on
     (ratios in the comment are WCAG 2.x, computed 2026-08-14). Comments are italic as well
     as coloured, so hue is never the only cue. Unmapped scopes inherit the code colour. */
  .tali-hl-comment { color: #6E6A60; font-style: italic; }   /* 4.78:1 */
  .tali-hl-string { color: #3F6152; }                        /* 6.12:1 */
  .tali-hl-keyword, .tali-hl-storage { color: #7A3B52; }     /* 7.23:1 */
  .tali-hl-constant, .tali-hl-support { color: #3A5578; }    /* 6.77:1 */
  .tali-hl-entity { color: #6B4A2F; }                        /* 7.04:1 */
  .tali-hl-variable { color: #22201A; }                      /* 14.45:1 */
```

- [ ] **Step 4: Write the dark scopes in `dark.css`**

```css
  /* The dark syntax palette is chosen, not lightened: same four roles, re-picked against the
     dark code ground. Ratios computed 2026-08-14. */
  html[data-theme="dark"] .tali-hl-comment { color: #8C877C; font-style: italic; } /* 4.86:1 */
  html[data-theme="dark"] .tali-hl-string { color: #8FBBA3; }                      /* 8.12:1 */
  html[data-theme="dark"] .tali-hl-keyword,
  html[data-theme="dark"] .tali-hl-storage { color: #D99BB0; }                     /* 7.67:1 */
  html[data-theme="dark"] .tali-hl-constant,
  html[data-theme="dark"] .tali-hl-support { color: #9DB4DA; }                     /* 8.26:1 */
  html[data-theme="dark"] .tali-hl-entity { color: #D0A67C; }                      /* 7.80:1 */
  html[data-theme="dark"] .tali-hl-variable { color: #EAE7E0; }                    /* 14.08:1 */
```

- [ ] **Step 5: Run the test**

```bash
cargo build -p taliesin-core
cargo test -p taliesin-core the_syntax_palette_is_owned_and_scored
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/css/base.css crates/core/assets/css/dark.css \
        crates/core/src/render/tests.rs
git commit -m "feat(theme): own the syntax palette

The twelve scope colours were GitHub Primer Light/Dark Default wholesale, inside a
project whose own test bans four borrowed blues as the loudest 'assembled from
framework defaults' tell. On a page that is half code, the most prominent colour on
screen belonged to another company.

Four hues in one warm-anchored chroma envelope, mapped onto the six syntect scopes,
each scored on the ground it actually sits on. The old palette bottomed out at 4.63:1;
this one at 4.78:1."
```

---

## Task 6: Widen the vendor-hex ban to every file that emits colour

**Files:**
- Modify: `crates/core/src/render/tests.rs`
- Modify: `site/favicon.svg`, `web-client/favicon.svg`, `editor/vscode/icons/tmd.svg`,
  `crates/server/src/serve/mod.rs`, and the demo `.tmd` sources that carry `#4c8dff`

**Why.** `no_vendor_default_colours_remain_in_any_bundled_stylesheet` already exists and
already bans `#4c8dff` and `#4c6ef5`. Both ship today — in both favicons, the VS Code icon,
the dev UI (6 reads) and nine demo files — because the test scans only the five stylesheets
where the doctrine had already been applied. **The doctrine was right; only the file list was
too narrow.**

- [ ] **Step 1: Widen the test and watch it fail**

Rename it and replace its source list:

```rust
/// The brand rests on ONE owned palette. These are the vendor defaults it replaced.
///
/// **The file list is the point.** This test used to scan the five bundled stylesheets, which
/// is exactly the set where the doctrine had already been applied — so it passed while
/// `#4c8dff` and `#4c6ef5` shipped in both favicons, the VS Code icon, the dev-menu CSS and
/// nine demo documents. A ban that only looks where you already cleaned is not a ban.
#[test]
fn no_vendor_default_colours_remain_anywhere_that_emits_colour() {
    const BANNED: &[(&str, &str)] = &[
        ("#4c8dff", "the old stock light blue"),
        ("#1f6feb", "GitHub Primer's blue"),
        ("#2563eb", "Tailwind blue-600"),
        ("#4c6ef5", "the retired deck's fourth blue"),
        ("#1e293b", "Tailwind slate-800"),
        ("#e2e8f0", "Tailwind slate-200"),
        ("#f9fafb", "Tailwind gray-50"),
        ("#b00020", "Material Design's error red"),
        ("#0645ad", "the old print link blue"),
    ];
    let root = repo_root();
    let mut checked = 0usize;
    for rel in [
        "crates/core/assets/css/tokens.css",
        "crates/core/assets/css/tokens-dark.css",
        "crates/core/assets/css/base.css",
        "crates/core/assets/css/dark.css",
        "crates/core/assets/css/site.css",
        "crates/server/src/serve/mod.rs",
        "web-client/client.js",
        "web-client/search.js",
        "site/favicon.svg",
        "web-client/favicon.svg",
        "editor/vscode/icons/tmd.svg",
    ] {
        let p = root.join(rel);
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
            .to_ascii_lowercase();
        for (hex, what) in BANNED {
            assert!(
                !text.contains(hex),
                "{rel} still ships {hex} ({what}); route it through the token layer"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 11, "a file dropped out of the vendor-colour sweep");
}
```

```bash
cargo test -p taliesin-core no_vendor_default_colours_remain_anywhere_that_emits_colour
```

Expected: FAIL, naming `site/favicon.svg` first.

- [ ] **Step 2: Replace both favicons and the VS Code icon with the typographic mark**

The mark is a letterform, not a picture: a single `T` in the owned mono, ink on paper, on a
**square** canvas. Write the identical file to `site/favicon.svg` and `web-client/favicon.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" role="img" aria-label="Taliesin">
  <rect width="64" height="64" fill="#FBF9F5"/>
  <path d="M14 16h36v7H36.5v32h-9V23H14z" fill="#22201A"/>
</svg>
```

`editor/vscode/icons/tmd.svg` takes the same mark on the dark ground (`#14130F` field,
`#EAE7E0` letter), since editor icons sit on editor chrome.

- [ ] **Step 3: Recolour the dev UI's hardcoded hexes**

In `crates/server/src/serve/mod.rs`, replace every `#4c8dff` with `var(--tali-fg)` where it
marked liveness, and every `var(--tali-accent, #4c8dff)` with `var(--tali-fg)`. Fix the phantom
token in the same pass — `serve/mod.rs:472` reads `--tali-mono`, which has never existed
(`var(--tali-mono, monospace)` is why no gate caught it); the real name is `--tali-font-mono`.

Full status-token work is Plan 3; here, only clear the banned hexes and the phantom read.

- [ ] **Step 4: Recolour the demo sources**

```bash
grep -rln "4c8dff" site docs corpus
```

Replace the plot/scene colour with the theme's data palette entry. These are *data*, so they
are allowed colour — just not a retired vendor blue. Use `#3A5578`.

- [ ] **Step 5: Run the test and the full suite**

```bash
cargo build
cargo test -p taliesin-core no_vendor_default_colours_remain_anywhere_that_emits_colour
cargo test --workspace
```

Expected: PASS. `retired_names.rs` may also need the new hexes; read its failure if it fires.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "test(theme): ban vendor hexes everywhere colour is emitted, not just in CSS

The existing ban scanned the five bundled stylesheets — exactly the set where the
doctrine had already been applied. So it passed while #4c8dff and #4c6ef5 shipped in
both favicons, the VS Code icon, the dev-menu CSS and nine demo documents. A ban that
only looks where you already cleaned is not a ban.

Replaces the Tailwind-slate rounded-square favicon with the typographic mark, clears
the dev UI's banned hexes, and fixes serve/mod.rs's read of --tali-mono, a token that
has never existed (the var() fallback is why no gate caught it)."
```

---

## Task 7: The static tell probe

**Files:**
- Modify: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing; this is the backstop.

- [ ] **Step 1: Write the test**

```rust
/// The checklist a reviewer would otherwise have to run by eye, as a gate. Each line is a
/// documented tell of generated or templated design; each is cheap to reintroduce by accident
/// and expensive to notice.
///
/// Static analysis of the bundled sheets, deliberately: a browser smoke test was decided
/// against with evidence on 2026-08-13 (notes/DO-NOT-REBUILD.md) and this must not become one.
#[test]
fn the_bundled_stylesheets_carry_no_generated_design_tells() {
    let sheets = [
        ("tokens.css", TOKENS_CSS),
        ("tokens-dark.css", TOKENS_DARK_CSS),
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
        ("site.css", SITE_CSS),
    ];
    for (name, css) in sheets {
        let l = css.to_ascii_lowercase();
        for (needle, why) in [
            ("box-shadow", "separation is whitespace, then a ground shift, then a hairline"),
            ("backdrop-filter", "an opaque ground and a 1px rule read the same"),
            ("system-ui", "the theme owns its faces"),
            ("border-radius: 999px", "a pill badge is a tell; set the label as text"),
            ("linear-gradient(135deg", "no decorative gradients"),
            ("translatey(-2px)", "hover may not move anything"),
            ("scale(1.05)", "hover may not move anything"),
        ] {
            assert!(!l.contains(needle), "{name}: {needle} — {why}");
        }
    }

    // Exactly one radius value, and it is small. `border-radius: 50%` (a circle) and
    // `em`-based inline radii are intentional specials and are excluded by shape.
    let mut radii: Vec<&str> = Vec::new();
    for (_, css) in sheets {
        for seg in css.split("border-radius:").skip(1) {
            let v = seg.split(';').next().unwrap_or("").trim();
            if v != "50%" && !v.ends_with("em") && !v.starts_with("var(") && !v.is_empty() {
                radii.push(v);
            }
        }
    }
    radii.sort_unstable();
    radii.dedup();
    assert!(
        radii.iter().all(|v| *v == "0" || *v == "2px"),
        "the radius scale drifted: {radii:?}. One token, 2px, objects only."
    );
}
```

- [ ] **Step 2: Run it**

```bash
cargo test -p taliesin-core the_bundled_stylesheets_carry_no_generated_design_tells
```

Expected: PASS if Tasks 2-5 were done completely. **If it fails, it is finding real
leftovers** — fix the sheet, do not weaken the test.

- [ ] **Step 3: Run every gate**

```bash
./tools/gates.sh
```

Expected: green, and the verdict line reports its own gate count. If a document gate fails,
it is `docs/guide/using/theming.tmd`, which documents the old tokens — Plan 4 rewrites it, so
for now update only the rows this plan invalidated.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/render/tests.rs
git commit -m "test(theme): gate the generated-design checklist

Static analysis of the bundled sheets: no shadows, no backdrop blur, no system-ui, no
pill radii, no decorative gradients, no hover motion, and exactly one radius value.

Not a browser test, on purpose: a browser smoke test was decided against with evidence
2026-08-13 and is recorded in notes/DO-NOT-REBUILD.md."
```

---

## Plans 2-4 (written after this one lands)

Each produces working, testable software on its own. Deliberately **not** written yet: this
plan changes the material every later plan builds on, and writing their steps against
un-landed tokens would guarantee drift.

- **Plan 2 — the reading surface.** The bleed grid replacing three copies of the escape
  arithmetic (this is what fixes code clipping at the measure, visible in every render so far);
  the permanent margin column plus the sidenote back-link fix — the one component whose reduced
  form is a defect rather than a simplification; and the restyle of code blocks, tables,
  callouts, figures, captions, TOC and title block, with the deletions local to each.
- **Plan 3 — chrome, brand and the dev UI.** Navbar/topbar merge, footer, listing-cards-to-a-
  ruled-list, drawer; the dev UI's status tokens; the dev-menu deletions (section panel, a11y
  scanner, cache/sections rows, canvas favicon dot, emoji).
- **Plan 4 — the subtractions and the three additions.** Structured `author:` and its corpus
  project (ordering rule: the corpus document dies in the *same* commit as the code it guards);
  the knobs a better default kills, each with its register entry; Cmd-K off standalone builds;
  KaTeX face subsetting; fonts-as-files for directory and site builds; the orphan-page
  diagnostic; and the `docs/guide/using/theming.tmd` rewrite plus its drift gate.

---

## Self-review

**Spec coverage.** §3 invariants → Task 2. §4 typography → Tasks 1-3. §5 colour, both palettes
and the syntax palette → Tasks 4-5. §7 brand mark → Task 6 (favicons and the VS Code icon;
the CLI banner glyph is Plan 3, which owns `main.rs`). §12 gates: gate 1 → Task 6, gate 2 →
Task 7, gate 3 → Task 3. §6 layout, §8 dev UI, §9 the cut, §10 knobs, §11 additions → Plans
2-4, listed above. §13 verification is a manual protocol, not a task; it runs after Plan 3,
when there is a finished surface to verify.

**Placeholder scan.** One deliberate placeholder: the FNV hash tuple in Task 3, which is an
output of the build rather than a decision, and Step 2 is the instruction to replace it. No
other TBDs; every code step carries real code.

**Type consistency.** `--tali-measure`, `--tali-u`, `--tali-radius`, `--tali-font-body`,
`--tali-font-mono`, `--tali-mono-size`, `--tali-inline-code`, `--tali-underline` are introduced
in Tasks 2 and 4 and used with those exact names afterwards. `LITERATA_MEAN_ADVANCE_EM` is
defined and read only in Task 3. The renamed test
`no_vendor_default_colours_remain_anywhere_that_emits_colour` replaces the old name in one
place. Test helpers `wcag_contrast`, `color_after` and `repo_root` already exist in
`tests.rs`/`token_contract.rs`; `repo_root` is currently private to `token_contract.rs`, so
**Task 6 Step 1 must either duplicate the four-line helper into `tests.rs` or lift it into a
shared test module** — flagged here rather than discovered mid-task.
