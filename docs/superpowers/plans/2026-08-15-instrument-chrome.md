# Instrument theme, Plan 3 of 4: chrome, brand and the dev UI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the surfaces a reader meets *around* the prose onto the same system the prose
already sits on — the preview dev UI (its status colours, its geometry, its voice, and the five
panels spec §8 deletes), the site chrome (two sticky bars that are one bar, a listing that is a
grid of cards and should be a ruled list, the book drawer), the marketing landing page (an
editorial masthead, not a centred hero with a feature grid), and the CLI banner — and delete,
in the same commits, the two gates Plan 1 built to fail when this plan arrived.

**Architecture:** Plan 1 made the material right (two owned faces, two scored palettes, one
geometry scale). Plan 2 changed the anatomy of the reading page. This plan changes the anatomy
of everything else a person sees, and it is the last plan before spec §13's verification
protocol has a finished surface to run against. The feature subtractions and the three
additions are Plan 4. Most of the work is in `site.css` and `serve/mod.rs`'s `STATUS_CSS`; the
Rust changes are the listing/drawer emitters, one caption-free `render/mod.rs` cut, and the
banner.

**Tech Stack:** Rust (edition 2024), `include_str!`-bundled CSS/JS, Rust-embedded CSS
(`STATUS_CSS`), vanilla browser JS (`web-client/client.js`, `// @ts-check`, no build step),
`crates/core/src/render/tests.rs` for the static gates, chrome-devtools MCP for the render
gates that static analysis cannot supply.

**Spec:** [docs/superpowers/specs/2026-08-14-instrument-theme-design.md](../specs/2026-08-14-instrument-theme-design.md)
— **amended twice by Plan 2, and both amendments bind here**: §4 now opens with the rule that
generates every machine-voice exception (the voice attaches to a label the TOOL generates,
never to a container that may hold AUTHOR text), and §3's spacing row is `{0.5U, U, 1.5U, 2U,
3U}` between flow blocks with object padding explicitly scoped out.

**Predecessor:** [docs/superpowers/plans/2026-08-15-instrument-reading-surface.md](2026-08-15-instrument-reading-surface.md)
(Plan 2; its rulings R1–R6 and its self-review carry forward). Plan 1 is
[2026-08-14-instrument-foundation.md](2026-08-14-instrument-foundation.md).

---

## Global Constraints

- **`TALIESIN_PYTHON` is inherited POISONED** in this environment — it points at another
  project's `.venv`, which does not exist. Prefix **every** `cargo test --workspace`, **every**
  `./tools/gates.sh` and **every** `git push` with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`.
  Without it four kernel tests "FAIL" without ever reaching an assertion, and the failure reads
  exactly like "my change broke the kernel".
- **Never run two `cargo test` invocations against this workspace concurrently** — the second
  hangs the first. If a suite seems slow, check `ps` for a long-`etime`, low-CPU
  `target/debug/deps/taliesin-*` before blaming the code.
- **Every task runs `cargo test --workspace`, not `-p taliesin-core`.** Half of this plan is in
  `crates/server`, and a crate-only run cannot see it at all.
- **Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before any test observes
  it.** They are `include_str!`-compiled. `STATUS_CSS` is a Rust `const`, so it needs one too.
- **Run `cargo fmt --all` LAST**, after every `.rs` edit — a `PostToolUse` hook runs `rustfmt`
  per file and fights a mid-stream `cargo fmt`. **An edit made with `python`/`sed` bypasses the
  hook entirely**, so the tree can be unformatted in a file you never opened with an editor
  tool. Plan 2 hit this twice.
- **Use `git commit -F <file>`, never `git commit -m` with backticks in the message.** The
  shell executes them. Plan 2 lost two message fragments and created a file literally named `:`.
- **`./tools/gates.sh` runs once, after the final task**, and its verdict line reports its own
  gate count. Never copy a gate count out of prose.
- **Branch first.** Work on `instrument-theme-chrome`; do not commit to `main`.
- **Never publish a number about the tool without a committed instrument.** A number with no
  instrument carries its measured-on date instead.
- **The ordering rule binds.** A corpus document, a docs page and a pin die in the *same*
  commit as the feature they guard, never in an earlier one.
- **A retirement is one register line plus stopping the parser reading the key.** The register
  note is ONE sentence — the date, then the successor or an explicit "nothing" — and may never
  be phrased as a did-you-mean. Do not write a tombstone test; the register derives it.
- **Two line coordinate systems.** If any diagnostic is added, a `source_file` may only be
  paired with a *mapped* line, never a buffer line.
- **`stdout` is the LSP's JSON-RPC wire.** Nothing this plan touches may print to it; console
  output goes through `crate::log` to stderr.
- Values copied verbatim from the spec: radius `2px` on interactive objects and `0` on
  structure, no shadows, no backdrop blur, one duration `--tali-dur: .1s`, machine voice
  `0.78rem` uppercase `letter-spacing: .053em` weight 400, hover may change an underline or a
  ground but **may not move anything**, text dimming is never `opacity` — every text colour an
  explicit scored hex, colour in chrome **none**.

### The rule this plan is most likely to break, stated once

**Static analysis of a stylesheet cannot see layout.** Plan 2 shipped six defects that every
static gate was green through, three of them in the single most-reviewed CSS block on the
branch. Every task here that moves a box ends with a **render** step: serve the page, measure
with `getBoundingClientRect`, and check `scrollWidth - clientWidth` at more than one width.
`resize_page` fails while the browser window is maximized and will not go below ~500px wide;
use device emulation for anything narrower.

---

## Decisions taken before this plan was written

**Measured on the branch tip (`9d6a5391`) on 2026-08-15**, with `scratchpad/palette.py`'s
sRGB relative-luminance definition — the same instrument §5 used:

| Dev-UI status hex | Role | On light `#FBF9F5` | Floor | |
|---|---|---|---|---|
| `#3fb950` | live dot | **2.42:1** | 3:1 non-text | FAIL |
| `#2bb673` | cell `done` border | **2.48:1** | 3:1 non-text | FAIL |
| `#d9a23a` | warming/warn — **and the alert label + count badge, which are TEXT** | **2.18:1** | 4.5:1 text | FAIL |
| `#e5534b` | error | 3.52:1 | 3:1 non-text | pass |
| `#cc3333` | cell `error` border | 4.88:1 | 3:1 non-text | pass |

Three of five fail their own floor, and the one that fails worst is the one used as text. Every
one of them is a dark-UI colour that met a light ground for the first time when Plan 1 made the
paper warm, and none was ever scored — which is precisely why spec §8 asks for **scored**
status tokens rather than merely named ones. **Task 2 is a contrast fix, not a tidy-up.**

`#3fb950` and `#e5534b` are additionally GitHub Primer's `success` and `danger`. They are not
on `BANNED` and so were never caught, but they are the same tell §12.1 widened the vendor-hex
ban for.

---

## Rulings taken while writing this plan

Each is one or two lines to reverse, and each is recorded because the spec did not settle it.

- **R1: the four status names collapse to THREE tokens.** Spec §8 says "four hardcoded status
  hexes become named, scored status tokens (live / warming / warn / error)". Two of those four
  are the *same literal* today (`warming` and `warn` are both `#d9a23a`), so a fourth token
  would be a second name for one value — the exact shape this theme keeps deleting. The
  coloured set is `live`, `warn` (covering "kernel starting" and "reconnecting": both mean *not
  ready yet, nothing is broken*), and `error`. `busy`/`running` already resolve to `--tali-fg`
  and `queued`/`idle` to `--tali-muted` (Plan 1), so nothing else needs a name. *Cost if
  wrong:* one token and two selectors. **Task 2.**
- **R2: the three status tokens are ALIASES of the three callout tokens, declared in
  `tokens.css`.** Spec §8 asks for tokens "shared with the diagnostic surfaces"; the diagnostic
  palette already exists, is already scored in both sheets, and maps one-to-one
  (`tip`→live, `warning`→warn, `important`→error). An alias gets the dark palette for free
  because `tokens-dark.css` redefines the callout tokens on the *same element* (`:root` is
  `html`, and so is `html[data-theme="dark"]`), so the substitution reads the cascaded dark
  value. **This looks like Plan 2's `:root` track-list trap and is not**: that one baked in a
  zero because the property it read was engaged on a *descendant*. Same element is safe,
  different element is not — and Task 2 verifies it in a dark render rather than resting on
  this paragraph. *Cost if wrong:* three literal pairs instead of three aliases.
- **R3: the two sticky bars become ONE rule with two garnishes, and the bar goes opaque.**
  `.tali-site-nav` and `.tali-book-topbar` are byte-identical apart from a padding value, and
  both carry `background: color-mix(in srgb, var(--tali-bg) 88%, transparent)` — a 12%
  translucent bar, which is the residue of the glassmorphism whose `backdrop-filter` Plan 1
  deleted. Without the blur, translucency is not a style: it is body text visibly scrolling
  through the chrome. The gate that banned the blur states the replacement in its own message
  ("an opaque ground and a 1px rule read the same"); this applies it. *Cost if wrong:* one
  `background` value. **Task 4.**
- **R4: a listing is a ruled list, not a grid of cards.** Spec §15.2 rules the landing page's
  three-card feature grid out and Plan 1 scoped "listing-cards-to-a-ruled-list" to this plan,
  but neither says what replaces the card. A card is a box (`border: 1px`, a radius, a ground)
  and spec §3 puts radius `0` on cards and §1 puts no colour in the furniture, so a "card" with
  neither is a bordered rectangle around a date and a title. The replacement is the anatomy the
  rest of the theme already uses: a hairline rule between rows, the date in the machine voice,
  the title in the serif at the prose size. *Cost if wrong:* one selector block; the emitted
  markup is unchanged except for cut #12. **Task 5.**
- **R5: `hero.eyebrow` keeps its key and loses its voice.** Spec §15.2 bans "letterspaced
  all-caps eyebrow"; §4's amended rule bans the machine voice on any container that may hold
  author text. `.hero-eyebrow` is `font: 600 .8rem/1 var(--tali-font-mono); letter-spacing:
  .12em; text-transform: uppercase` applied to `_site.yml`'s authored `eyebrow:` string — a
  *fourth* voice (not even the machine voice's `400`/`.78rem`/`.053em`), on author text. It
  becomes the serif. Retiring the key instead would cost four drift gates to remove a word the
  author wrote and may still want. *Cost if wrong:* one register line, later. **Task 7.**
- **R6: spec §9's cut #12 lands whole, in one task.** Its four items (category chips, the
  monogram placeholder, reading time, chapter word counts) sit in three different files across
  the listing, the drawer and the title block. Splitting a numbered cut across tasks is how a
  cut half-lands and the leftover half is defended later by the pin that survived it. *Cost if
  wrong:* Task 5 is the largest task here. **Task 5.**
- **R7: the emoji leave in two pieces, and the seam is deliberate.** Spec §8 says the emoji
  "leave the dev chrome with" the deleted panels. Three of the eight (`✗ ⚠ ♿`) exist only
  inside panels Task 1 deletes and go with them; the other five (`⚡ ⏳ ✓ ✕ ●`) are on cell
  badges and tab titles that survive, and replacing those with words is a *typographic* change,
  which is Task 3's subject. The gate asserting the dev surface is emoji-free lives in Task 3,
  where the last one dies. *Cost if wrong:* one task boundary.

---

## File structure

| File | Responsibility after this plan |
|---|---|
| `crates/core/assets/css/tokens.css` | Gains `--tali-status-live` / `-warn` / `-error`, three aliases of the callout tokens (R2). Still the one place a token is declared. |
| `crates/server/src/serve/mod.rs` | `STATUS_CSS`: every colour a token, one radius, no shadows, the owned faces, the machine voice, and none of the five deleted panels' rules. |
| `web-client/client.js` | Loses the section-annotations panel, the a11y scanner, the Cache/Sections rows and the canvas favicon dot; its error overlay is rewritten off the token layer (which is where `client.js:474`'s borrowed stack dies); its cell badges speak words. |
| `crates/core/assets/css/site.css` | One sticky-bar rule instead of two; an opaque bar; the listing as a ruled list; the drawer on the token geometry; the chrome margins on the spacing scale; three no-op hovers gone. |
| `crates/core/assets/css/base.css` | The masthead replaces the centred hero; `.feature-grid`/`.feature` become ruled sections; the `.btn` ladder trimmed to what the masthead uses. |
| `crates/core/src/site/mod.rs` | `card_html` loses the category chips and the monogram placeholder (cut #12). |
| `crates/core/src/site/chrome.rs` | The drawer/Contents rows lose `.tali-chap-words` (cut #12). |
| `crates/core/src/render/mod.rs` | The title-block meta line loses the reading-time estimate (cut #12). |
| `crates/server/src/log.rs` | The banner is the third instance of the typographic mark, uncoloured; `keys_hint_body` names the glyph the toggle actually draws. |
| `site/index.tmd` | Rewritten as an editorial masthead and prose (spec §15.2). |
| `crates/core/src/render/tests.rs` | **Deletes `DEV_UI_STATUS_EXEMPT` + `DEV_UI_SURFACE`** (Task 2) and widens the tell probe to the dev UI's two files (Task 3); gains the status-score gate, the one-bar gate, the ruled-list gate and the chrome-scale gate. |

---

## Task 1: The dev menu loses five panels

**Files:**
- Modify: `web-client/client.js`
- Modify: `crates/server/src/serve/mod.rs` (the deleted panels' CSS)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing later tasks read. It *removes* `renderSections`, `collectSections`,
  `scanA11y`, `a11yLoc`, `contrastRatio`, `parseRgb`, `bgColor`, `setFaviconDot` and the
  module-level `sectionsEl` / `sectionsStale` / `lastDiagnostics` / `a11yCount` / `origFavicon`
  state, so Tasks 2 and 3 restyle a smaller surface.

**Why.** Spec §8 names five deletions and gives each its own reason: the section-annotations
panel (three of its four columns are duplicated three rows below in the same panel), the static
"Cache" prose row, the empty "Sections" label row, the client-side a11y scanner (it duplicates
the server, **re-implements a check wave 9 deliberately cut**, double-counts its own findings
into the alert badge, and renders unstyled because every `.tali-diag` rule is scoped to a
different id), and the canvas favicon dot.

**Verified facts** (read on the branch tip, 2026-08-15):

- The panel is assembled in one `append` at `client.js:633`:
  `devRow("Status", …), devRow("Words", …), devRow("Source", …), kernelBtn, devRow("Cache", …),
  devRow("Sections", document.createElement("span")), sectionsEl` — the "Sections" row's value
  really is a bare empty `<span>`, and `sectionsEl` follows it as a sibling.
- The a11y scanner's check 3 ("Link/Button has no accessible name", `client.js:404-414`) is the
  accessible-name lint wave 9 cut server-side. Check 5 (body-text contrast) re-derives at
  runtime what `every_text_colour_is_scored_in_both_palettes` already gates on the token sheet.
- `a11yCount` is summed into `refreshAlert`'s `total` alongside `diagCount`, so a page with one
  missing `alt` shows `2` when the server has also reported it — the double-count §8 names.
- `setFaviconDot` is called at four sites (`client.js:863, 882, 898, 919`) and is the ONLY
  reason `client.js` contains a status hex at all. Deleting it is what lets Task 2 delete the
  gate's exemption.
- `lastDiagnostics` exists solely to let `collectSections` badge diagnostics per section
  (`client.js:162-166`); nothing else reads it. `sectionsStale` likewise.

- [ ] **Step 1: Write the failing test**

In `crates/core/src/render/tests.rs`:

```rust
/// Spec §8's five dev-menu deletions, gated on the two files that render the dev UI.
///
/// Each had its own reason and none of them is style: the section-annotations panel duplicated
/// three of its four columns three rows above itself; the "Cache" row was static prose; the
/// "Sections" row was a label with an empty `<span>` for a value; the client-side a11y scanner
/// duplicated the server, re-implemented the accessible-name check wave 9 deliberately cut,
/// double-counted its own findings into the alert badge, and rendered unstyled because every
/// `.tali-diag` rule is scoped to a different id; and the canvas favicon dot drew a coloured
/// circle nobody asked for over a mark spec §7 had just made deliberate.
#[test]
fn the_dev_menu_lost_the_five_panels_spec_8_deletes() {
    let root = repo_root();
    let client = std::fs::read_to_string(root.join("web-client/client.js")).expect("client.js");
    let serve = std::fs::read_to_string(root.join("crates/server/src/serve/mod.rs"))
        .expect("serve/mod.rs");

    for (needle, why) in [
        ("collectSections", "the section-annotations panel is deleted"),
        ("renderSections", "the section-annotations panel is deleted"),
        ("sectionsEl", "the section-annotations panel is deleted"),
        ("scanA11y", "the client-side a11y scanner duplicated the server"),
        ("a11yCount", "the a11y scanner double-counted into the alert badge"),
        ("contrastRatio", "runtime contrast re-derived what the token gates already score"),
        ("setFaviconDot", "the canvas favicon dot is deleted"),
        ("tali-cache-hint", "the static Cache prose row is deleted"),
        ("devRow(\"Sections\"", "the Sections row was a label with an empty value"),
    ] {
        assert!(!client.contains(needle), "client.js still has `{needle}`: {why}");
    }
    // The CSS for a deleted panel is deleted in the same commit, or it becomes a rule set
    // nothing can reach and the next reader restyles it.
    for (needle, why) in [
        ("tali-dev-sections", "the section panel's rules go with the panel"),
        ("tali-section-row", "the section panel's rules go with the panel"),
        ("tali-section-meta", "the section panel's rules go with the panel"),
        ("tali-section-empty", "the section panel's rules go with the panel"),
    ] {
        assert!(!serve.contains(needle), "STATUS_CSS still styles `{needle}`: {why}");
    }
    // The three emoji that lived only inside the deleted panels go with them. The other five
    // are on surviving badges and titles and are Task 3's (see ruling R7).
    for glyph in ['✗', '♿'] {
        assert!(
            !client.contains(glyph),
            "`{glyph}` lived only in a panel this commit deletes"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_dev_menu_lost_the_five_panels_spec_8_deletes
```

Expected: FAIL on `client.js still has collectSections`.

- [ ] **Step 3: Delete the section-annotations panel**

In `web-client/client.js`, delete:

- `collectSections` (the whole `const collectSections = () => { … };`, currently ~`:130-181`)
  and its doc comment;
- `renderSections` (~`:183-228`);
- the `sectionsEl` and `sectionsStale` declarations wherever they are declared at module scope
  (grep for both names — each has exactly one declaration and the rest are uses);
- the `lastDiagnostics` declaration and its assignment inside `setDiagnostics`. Read
  `setDiagnostics` before cutting: the two lines
  ```js
      lastDiagnostics = list;
      if (sectionsEl && !document.getElementById("tali-dev-panel")?.hidden) renderSections();
      else sectionsStale = true;
  ```
  go together with the comment above them; everything after `diagEl.textContent = "";` stays.
- the `if (!panel.hidden && sectionsStale) renderSections();` clause inside the toggle's click
  handler (~`:572`), leaving the handler as:
  ```js
      toggle.addEventListener("click", (e) => {
        e.stopPropagation();
        panel.hidden = !panel.hidden;
        toggle.setAttribute("aria-expanded", panel.hidden ? "false" : "true");
      });
  ```
- the second `renderSections()` call site (~`:1131`), whose surrounding `if` guard goes with it;
- `sectionsEl = document.createElement("div"); sectionsEl.className = "tali-dev-sections";`
  (~`:630-631`).

- [ ] **Step 4: Delete the a11y scanner**

Delete the whole `--- accessibility audit of the rendered output ---` section
(`client.js:328-455`): the banner comment, `a11yLoc`, `contrastRatio`, `parseRgb`, `bgColor`
and `scanA11y`. Then:

- delete the `a11yEl` / `a11yCount` declarations (~`:245-250`) and the comment above them;
- in `refreshAlert`, drop the `a11yCount` term:
  ```js
    const refreshAlert = () => {
      const diagCount = diagEl.style.display === "none" ? 0 : diagEl.children.length;
      const total = diagCount + cellErrCount;
  ```
- in the panel assembly, drop `a11yEl`: `panel.append(diagEl, cellErrEl);`
- delete the `scanA11y();` call site (~`:1134`) and repair the comment two lines below it,
  which currently reads "word count deep-clones `#tali-root` + a11y/code scans" — after this
  commit it deep-clones for the word count alone.

- [ ] **Step 5: Delete the Cache row, the Sections row, and the canvas favicon dot**

The `cacheHint` element and its comment (~`:621-628`) go; the panel assembly becomes:

```js
    panel.append(devRow("Status", statusEl), devRow("Words", wordCountEl), devRow("Source", srcHint), kernelBtn);
```

Delete `setFaviconDot` and the `origFavicon` declaration above it (~`:794-819`), the comment
block that introduces them, and all four call sites:

- `setFaviconDot(null);` in the `idle` branch (~`:863`) — the comment two lines above says
  "Restore tab title and favicon"; it becomes "Restore the tab title";
- `setFaviconDot("#e5534b");` in the `error` branch (~`:882`);
- `setFaviconDot("#d9a23a");` in the `warming` branch (~`:898`);
- in the `executing` branch, the last three lines of `updateProgress` (~`:916-919`), including
  the `busyColor` read and the comment above it — that `getComputedStyle` call existed only to
  hand `--tali-fg` to a canvas `fillStyle`.

Trim the section banner at `:775` — it currently promises "tab-title/favicon" — to
`// --- progress chip: idle/busy dot, k/N bar, click-to-scroll, tab title ---`.

- [ ] **Step 6: Delete the panels' CSS**

In `crates/server/src/serve/mod.rs`'s `STATUS_CSS`, delete the five
`.tali-dev-sections …` rules (`:401-413`) and `#tali-wordcount`'s neighbour comment if it names
the section panel. **Keep `#tali-wordcount`** — the Words row survives.

- [ ] **Step 7: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_dev_menu_lost_the_five_panels_spec_8_deletes
cd web-client && npx -y -p typescript tsc -p jsconfig.json; cd -
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS, and `tsc` clean. The type-check is not optional here and it is the only thing
that will catch a dangling reference to a deleted symbol — there is no JS test suite.

- [ ] **Step 8: Render it**

The dev menu has no static gate for "the panel still opens". Serve a document with a
diagnostic and a code cell, open the menu, and confirm four rows and no console error:

```bash
cargo run -p taliesin-server -- preview corpus/callouts/kinds.tmd 4388
```

Then via the chrome-devtools MCP: navigate to `http://127.0.0.1:4388`, click
`#tali-dev-toggle`, screenshot the panel, and `list_console_messages`. Expected: rows
`Status` / `Words` / `Source`, the `Restart kernel` button, the theme toggle, and **zero**
console errors. A `ReferenceError` here is the failure mode `tsc` cannot see (it type-checks,
it does not execute).

- [ ] **Step 9: Commit**

Write the message to a file — backticks in `-m` are executed by the shell.

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the dev menu loses the five panels spec §8 deletes

The section-annotations panel duplicated three of its four columns three rows
above itself in the same panel. The "Cache" row was static prose. The "Sections"
row was a label with an empty <span> for a value. The canvas favicon dot drew a
coloured circle over a mark §7 had just made deliberate.

The a11y scanner is the one worth stating in full: it duplicated the server's own
diagnostics, re-implemented the accessible-name check wave 9 deliberately cut,
double-counted its findings into the alert badge (one missing alt read as 2), and
rendered unstyled the whole time because every .tali-diag rule is scoped to a
different id than the one it mounted into.

Deleting setFaviconDot is what leaves client.js with no status hex, which is the
precondition for the next commit deleting the vendor-hex gate's dev-UI exemption.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 2: The status colours become named, scored tokens — and the exemption dies

**Files:**
- Modify: `crates/core/assets/css/tokens.css`
- Modify: `crates/server/src/serve/mod.rs`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-callout-tip` / `-warning` / `-important` (Plan 1, scored in both sheets).
- Produces: `--tali-status-live`, `--tali-status-warn`, `--tali-status-error`. Task 3 reads all
  three; nothing else does.

**Why.** Spec §8: the dev UI's hardcoded status hexes "become **named, scored** status tokens
(live / warming / warn / error) shared with the diagnostic surfaces". The measurement at the
top of this plan is why *scored* is the load-bearing word: three of the five fail their own
WCAG floor on the light ground, and the worst of them (`#d9a23a`, **2.18:1**) is used as text.

Per **R1** the four names are three tokens; per **R2** they are aliases of the diagnostic
palette, which is what "shared with the diagnostic surfaces" means and what gets the dark
palette right for free.

**This task deletes `DEV_UI_STATUS_EXEMPT` and `DEV_UI_SURFACE`.** They were built to fail here
and the failure message says so: *"if you just gave this a real token, delete this exemption
instead of leaving it stale"*. Deleting the exemption is the job. **Do not weaken it, do not
add a hex to it, and do not add a fourth file to `DEV_UI_SURFACE`.**

**Verified facts:**

| Token | Alias of | Light | on `#FBF9F5` | Dark | on `#14130F` |
|---|---|---|---|---|---|
| `--tali-status-live` | `--tali-callout-tip` | `#3F6152` | 6.56:1 | `#8FBBA3` | 8.68:1 |
| `--tali-status-warn` | `--tali-callout-warning` | `#7A4A18` | 7.08:1 | `#D0A67C` | 8.34:1 |
| `--tali-status-error` | `--tali-callout-important` | `#8B3A2E` | 7.28:1 | `#DC8B78` | 7.09:1 |

All six clear 4.5:1 as text and 3:1 as non-text on the ground they sit on, measured 2026-08-15.
Six literals collapse to three tokens: `#3fb950` and `#2bb673` were two different greens for
`live` and `done`, and `#e5534b` and `#cc3333` two different reds for `error` and cell `error`
— two designers' worth of status colour on one surface.

- [ ] **Step 1: Write the failing test**

```rust
/// The dev UI's status colours are named, scored tokens (spec §8) rather than literals picked
/// for a dark UI and never measured against the paper.
///
/// MEASURED 2026-08-15 on the branch tip, before this commit: `#3fb950` (live) scored 2.42:1
/// on `#FBF9F5`, `#2bb673` (cell done) 2.48:1, and `#d9a23a` 2.18:1 — the last of which is
/// used as TEXT (the alert label and the count badge), where the floor is 4.5:1. Three of the
/// five failed their own floor. That is why §8 says *scored* and not merely *named*.
///
/// Three tokens and not four: `warming` and `warn` were the same literal, so a fourth name
/// would be a second spelling of one value (ruling R1).
#[test]
fn the_dev_ui_paints_status_from_scored_tokens_only() {
    let serve = std::fs::read_to_string(repo_root().join("crates/server/src/serve/mod.rs"))
        .expect("serve/mod.rs");
    let css = serve
        .split_once("pub(crate) const STATUS_CSS")
        .expect("STATUS_CSS exists")
        .1;

    // No literal colour of any kind on the dev UI's own surface. A `#` followed by three or
    // six hex digits is a colour wherever it appears in a stylesheet; there is no legitimate
    // one left once the four status hexes are tokens.
    let mut literals: Vec<String> = Vec::new();
    for (i, _) in css.match_indices('#') {
        let tail: String = css[i + 1..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        // `#tali-progress` and friends are ids, not colours: an id is followed by more
        // identifier characters, a colour by a delimiter.
        let after = css[i + 1 + tail.len()..].chars().next().unwrap_or(' ');
        if (tail.len() == 3 || tail.len() == 6) && !after.is_alphanumeric() && after != '-' {
            literals.push(format!("#{tail}"));
        }
    }
    literals.sort();
    literals.dedup();
    assert!(
        literals.is_empty(),
        "STATUS_CSS still paints literal colours {literals:?}. Every one of them was chosen \
         for a dark UI and none was ever scored against the paper — see the ratios in this \
         test's doc comment. Route them through --tali-status-* or an existing token"
    );

    // The three tokens exist, are declared exactly once, and each derives from the diagnostic
    // colour it shares a meaning with, which is what gets the dark palette right (R2).
    let tokens = std::fs::read_to_string(
        repo_root().join("crates/core/assets/css/tokens.css"),
    )
    .expect("tokens.css");
    for (name, from) in [
        ("--tali-status-live", "--tali-callout-tip"),
        ("--tali-status-warn", "--tali-callout-warning"),
        ("--tali-status-error", "--tali-callout-important"),
    ] {
        assert_eq!(
            tokens.matches(&format!("{name}:")).count(),
            1,
            "{name} must be declared exactly once, in tokens.css"
        );
        assert!(
            tokens.contains(&format!("{name}: var({from})")),
            "{name} must derive from {from}: spec §8 says the status tokens are SHARED with \
             the diagnostic surfaces, and an alias is what makes tokens-dark.css's override \
             reach both without a second dark block"
        );
        assert!(
            css.contains(&format!("var({name})")),
            "{name} is declared but the dev UI never reads it"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_dev_ui_paints_status_from_scored_tokens_only
```

Expected: FAIL listing `["#2bb673", "#3fb950", "#cc3333", "#d9a23a", "#e0e0e0", "#e5534b",
"#f5f5f5", "#fff", "#111", "#222", "#888", "#aaa"]` or a subset — the `var(--x, fallback)`
greys are literals too and go in this commit with the status hexes. That is deliberate: a
fallback grey is a colour nobody scored, sitting behind a token that always resolves.

- [ ] **Step 3: Declare the three tokens**

In `tokens.css`, immediately after the `--tali-callout-important` declaration:

```css
    /* The preview dev UI's three status colours, which are the diagnostic colours under
       names that say what they mean (spec §8: "named, scored status tokens ... shared with
       the diagnostic surfaces"). Aliases and not literals, so tokens-dark.css's override of
       the callout tokens reaches these too — both are declared on `:root`, which IS `html`,
       so the substitution reads the cascaded dark value on the same element. (That is what
       makes this safe where Plan 2's `:root` track list was not: THAT one read a property
       engaged on a DESCENDANT and baked in the root's zero.)

       Scored 2026-08-15 on the ground each sits on: live 6.56 light / 8.68 dark, warn 7.08 /
       8.34, error 7.28 / 7.09. All six clear 4.5:1 as text. What they replace did not — the
       four literals here were picked for a dark UI and met the warm paper for the first time
       when Plan 1 landed: live scored 2.42:1, cell-done 2.48:1, and the amber 2.18:1 while
       being used as the alert LABEL. `warming` and `warn` shared one literal, which is why
       there are three names here and not §8's four. */
    --tali-status-live: var(--tali-callout-tip);
    --tali-status-warn: var(--tali-callout-warning);
    --tali-status-error: var(--tali-callout-important);
```

- [ ] **Step 4: Route every colour in `STATUS_CSS` through a token**

In `crates/server/src/serve/mod.rs`, replace the literals. The status ones:

| Was | Becomes |
|---|---|
| `#3fb950` (live dot) | `var(--tali-status-live)` |
| `#2bb673` (cell `done` border) | `var(--tali-status-live)` |
| `color-mix(in srgb, #2bb673 40%, transparent)` | `color-mix(in srgb, var(--tali-status-live) 40%, transparent)` |
| `#d9a23a` (alert border+label, warn dot, warming border+dot, diag-warning rule) | `var(--tali-status-warn)` |
| `color-mix(in srgb, #d9a23a 55%, transparent)` | `color-mix(in srgb, var(--tali-status-warn) 55%, transparent)` |
| `#e5534b` (error dot, diag-error rule, cellerr rule + hover, progress error) | `var(--tali-status-error)` |
| `#cc3333` (cell `error` border) | `var(--tali-status-error)` |

And the fallback greys, which are the same defect in a quieter form — drop the fallback, keep
the token:

| Was | Becomes |
|---|---|
| `var(--tali-bg, #fff)` | `var(--tali-bg)` |
| `var(--tali-fg, #111)`, `var(--tali-fg, #222)` | `var(--tali-fg)` |
| `var(--tali-muted, #888)`, `var(--tali-muted, #aaa)` | `var(--tali-muted)` |
| `var(--tali-border, #e0e0e0)` | `var(--tali-border)` |
| `var(--tali-code-bg, #f5f5f5)` | `var(--tali-code-bg)` |
| `background: #d9a23a; color: #fff` (the count badge) | `background: var(--tali-status-warn); color: var(--tali-bg)` |

Dropping the fallbacks is safe and is *why* they must go: `every_tali_custom_property_read_is_defined_somewhere`
**exempts any `var(--x, fallback)` read**, which is exactly the hole that let
`var(--tali-mono, monospace)` name a token that never existed for as long as it shipped. A
bare `var(--tali-bg)` is checked by that gate; `var(--tali-bg, #fff)` is not. Every one of
these six tokens is defined in `tokens.css` and `STATUS_CSS` is only ever injected into a page
that inlines it (`serve_site/mod.rs:821` appends it to `extra_head`).

- [ ] **Step 5: Delete the exemption — this is the job, not a side effect**

In `crates/core/src/render/tests.rs`, inside
`no_vendor_default_colours_remain_anywhere_that_emits_colour`:

- delete `const DEV_UI_STATUS_EXEMPT` and `const DEV_UI_SURFACE`;
- delete `let mut exempt_seen = [false; DEV_UI_STATUS_EXEMPT.len()];`, the
  `let on_dev_ui_surface = …` line and the whole `for (i, (hex, what)) in
  DEV_UI_STATUS_EXEMPT.iter().enumerate()` block inside the file loop;
- delete the trailing `for … exempt_seen[i]` assertion block;
- in the `.tmd` sweep, `BANNED.iter().chain(DEV_UI_STATUS_EXEMPT)` becomes `BANNED.iter()`;
- replace the ~14-line comment introducing the exemption with what actually happened, and add
  the two Primer hexes to `BANNED` so they cannot come back under a different name:

```rust
    const BANNED: &[(&str, &str)] = &[
        // … the thirteen existing entries, unchanged …
        ("#3fb950", "GitHub Primer's success green — the dev UI's old `live` dot"),
        ("#e5534b", "GitHub Primer's danger red — the dev UI's old `error` colour"),
    ];
```

and above the file list:

```rust
    // The dev UI's status literals had a bounded exemption here until 2026-08-15, when they
    // became `--tali-status-live` / `-warn` / `-error`. The exemption is DELETED rather than
    // emptied: an exemption nobody removes is how the gap this test closes quietly reopens,
    // and its own message said to delete it in the commit that gave the colours real tokens.
    // Two of the four were GitHub Primer's success/danger and are now on BANNED above, so
    // the ban is wider than it was, not narrower.
```

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_dev_ui_paints_status_from_scored_tokens_only \
                            no_vendor_default_colours_remain_anywhere_that_emits_colour
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. `every_tali_custom_property_read_is_defined_somewhere` (in
`crates/core/tests/token_contract.rs`) now checks six reads it was previously exempting; if it
fires, a token name is misspelled and the browser was silently dropping that whole declaration.

- [ ] **Step 7: Render it in BOTH palettes — R2 rests on this**

R2's alias claim is a statement about CSS custom-property substitution, and a wrong one paints
the light green in dark mode with every gate green. Verify it:

```bash
cargo run -p taliesin-server -- preview corpus/callouts/kinds.tmd 4388
```

Via the chrome-devtools MCP, with the page open:

```js
// light
getComputedStyle(document.documentElement).getPropertyValue('--tali-status-live').trim()
// → "#3F6152"
document.documentElement.setAttribute('data-theme', 'dark');
getComputedStyle(document.documentElement).getPropertyValue('--tali-status-live').trim()
// → "#8FBBA3"   <-- if this is still #3F6152, R2 is wrong: replace the three aliases with
//                    six literals across tokens.css + tokens-dark.css and re-run.
```

Screenshot the dev menu in both palettes and confirm the dot is legible on each ground.

- [ ] **Step 8: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the dev UI's status colours become scored tokens

Measured on the branch tip before this commit, on the warm paper Plan 1 landed:
#3fb950 (live) 2.42:1, #2bb673 (cell done) 2.48:1, #d9a23a 2.18:1 — and the amber
is used as TEXT, where the floor is 4.5:1. Three of the five failed. Every one was
chosen for a dark UI and none was ever scored, which is why spec §8 asks for SCORED
status tokens and not merely named ones.

Six literals become three tokens: live/done were two different greens and
error/cell-error two different reds, so the surface carried two designers' worth of
status colour. The tokens alias the diagnostic palette, which is what §8 means by
"shared with the diagnostic surfaces" and what makes tokens-dark.css reach them
without a second dark block. All six values clear 4.5:1 on the ground they sit on.

Deletes DEV_UI_STATUS_EXEMPT and DEV_UI_SURFACE, which is what they were built for:
the exemption's own message said to delete it in the commit that gave these colours
real tokens. Two of the four were GitHub Primer's success and danger and join BANNED,
so the ban is wider after this than before it.

The var(--x, #fallback) greys go with them. A fallback is exempt from the
never-read-an-undefined-token gate — the same hole that let var(--tali-mono, monospace)
name a token that never existed for as long as it shipped.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 3: The dev UI's geometry and voice

**Files:**
- Modify: `crates/server/src/serve/mod.rs`
- Modify: `web-client/client.js`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-status-*` (Task 2), `--tali-radius`, `--tali-font-mono`, `--tali-u`.
- Produces: nothing.

**Why.** Spec §8: "its radii collapse to the one radius, its shadows go". Spec §3: no shadows
anywhere, no backdrop blur, one radius. Spec §12.2's tell probe already asserts all of this —
**on the five bundled stylesheets only**, which is the same "a ban that only looks where you
already cleaned" defect §12.1 called out for colour. `STATUS_CSS` carries `border-radius: 999px`
twice, `9px`, `6px` four times, `4px` twice and three `box-shadow`s; `client.js`'s error overlay
carries `border-radius: 10px`, a `box-shadow`, a `backdrop-filter: blur(3px)` (the third
verbatim blur, after the two §3 named) and **the borrowed `ui-sans-serif, system-ui, sans-serif`
stack at `:471` and `:474`** — the stack Plan 1 removed from the sheets and gates against,
surviving in a file the gate does not scan.

`client.js:474` is the second gate this plan is required to retire, and it retires by the file
becoming scannable rather than by the line being patched: **widening the probe is the fix; the
line is a symptom.**

Per **R7** this task also takes the five surviving emoji (`⚡ ⏳ ✓ ✕ ●`), because replacing an
emoji with a mono word is a typographic change and this is the typographic task.

**Verified facts:** the error overlay (`client.js:461-484`) hardcodes `#1b1d23`, `#5a2a2a`,
`#e5534b`, `#ff8c82`, `#f2d5d5` — a dark card that ignores the reader's palette entirely — and
is the only place in the dev UI that draws its own `<style>` element rather than living in
`STATUS_CSS`. `#e5534b` is on `BANNED` as of Task 2, so this file will not compile past the
colour gate until the overlay is rewritten; that is the gate doing its job, not an obstacle.

- [ ] **Step 1: Write the failing test**

```rust
/// Spec §12.2's tell probe, applied to the two files that render the preview dev UI.
///
/// The probe has always asserted these things — on the five bundled stylesheets, which is the
/// set where the doctrine had already been applied. That is the same shape of hole §12.1
/// widened the vendor-hex ban for, and it is how a `ui-sans-serif, system-ui` stack survived
/// in `client.js` after Plan 1 removed it from every sheet and gated against it: the gate did
/// not scan the file.
#[test]
fn the_dev_ui_carries_no_generated_design_tells_either() {
    let root = repo_root();
    for rel in ["crates/server/src/serve/mod.rs", "web-client/client.js"] {
        let text = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
            .to_ascii_lowercase();
        for (needle, why) in [
            ("box-shadow", "separation is whitespace, then a ground shift, then a hairline"),
            ("backdrop-filter", "an opaque ground and a 1px rule read the same"),
            ("system-ui", "the theme owns its faces"),
            ("ui-sans-serif", "the theme owns its faces"),
            ("border-radius: 999px", "a pill badge is a tell; set the label as text"),
            ("sfmono-regular", "the mono is JetBrains Mono, through --tali-font-mono"),
        ] {
            assert!(!text.contains(needle), "{rel}: {needle} — {why}");
        }
        // One radius, and it is the token. `50%` (a circle) is the one shape exclusion, and
        // it is bounded by being a single literal value — the same carve-out the sheet probe
        // makes, for the same reason.
        let mut radii: Vec<&str> = Vec::new();
        for seg in text.split("border-radius:").skip(1) {
            let v = seg.split(';').next().unwrap_or("").trim();
            if v != "50%" && !v.starts_with("var(") && !v.is_empty() {
                radii.push(v);
            }
        }
        radii.sort_unstable();
        radii.dedup();
        assert!(
            radii.iter().all(|v| *v == "0"),
            "{rel}: the radius scale drifted: {radii:?}. One token, 2px, objects only"
        );
    }

    // The machine voice replaces the emoji (spec §8). The three that lived inside deleted
    // panels went with them; these five were on badges and titles that survive.
    let client = std::fs::read_to_string(root.join("web-client/client.js")).expect("client.js");
    for glyph in ['⚡', '⏳', '✓', '✕', '●', '⚠'] {
        assert!(
            !client.contains(glyph),
            "`{glyph}` is still in the dev chrome; the machine voice replaces it (spec §8)"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_dev_ui_carries_no_generated_design_tells_either
```

Expected: FAIL on `crates/server/src/serve/mod.rs: box-shadow`.

- [ ] **Step 3: Put `STATUS_CSS` on the geometry and the voice**

In `crates/server/src/serve/mod.rs`, edit `STATUS_CSS` in place:

- the three `box-shadow: …` declarations (on `.tali-dev-toggle`, `.tali-dev-panel`,
  `#tali-progress`) are **deleted**. Each already has a `1px solid var(--tali-border)` beside
  it, which is the separation the theme uses;
- every `border-radius` becomes `var(--tali-radius)` on an interactive object (the toggle, the
  panel, `.tali-dev-ctl`, `.tali-diag`, `.tali-cellerr`, `#tali-progress`, `.tali-diag-frame`,
  `.tali-prog-bar`, `.tali-prog-fill`) and stays `50%` on the two dots. The two `999px` pills
  (`.tali-dev-toggle`, `.tali-dev-count`) take `var(--tali-radius)` — §12.2's own message is
  "a pill badge is a tell; set the label as text";
- the three font declarations take the owned faces. The dev UI is *entirely* the machine
  speaking — there is no author text anywhere in it, which is the one place §4's rule permits
  the voice on a whole container:

```rust
    #tali-controls.tali-dev { position: fixed; bottom: .6rem; left: .6rem; z-index: 9999; \
      font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase; \
      letter-spacing: .053em; } \
```

  `.tali-cellerr`'s `font: 12px ui-sans-serif, system-ui, sans-serif` and `#tali-progress`'s
  become `font: inherit;` — they are inside `#tali-controls` and `<body>` respectively, so
  read the surrounding rule before choosing: `#tali-progress` is appended to `document.body`
  (`client.js:833`) and is NOT inside `#tali-controls`, so it needs the declaration written
  out, not `inherit`;
- `.tali-diag-frame`'s and `.tali-dev-glyph`'s `ui-monospace, SFMono-Regular, Menlo, monospace`
  become `var(--tali-font-mono)`;
- `.tali-diag-loc::after`'s `content: "  \2192 source"` keeps its arrow **as a deliberate
  exception**: it is a `content` string in an owned-mono context and the vendored subset has no
  `U+2192` (Plan 4 re-vendors), so it falls back to a system mono for one glyph. Leave a
  comment saying so, or it will read as an oversight. It is not an emoji and the gate above
  does not test for it.

- [ ] **Step 4: Rewrite the error overlay against the token layer**

In `web-client/client.js`, replace the `errorEl` IIFE's `style.textContent` (`:463-474`) with:

```js
    style.textContent =
      "#tali-error{position:fixed;inset:0;z-index:2147482500;display:none;flex-direction:column;" +
      "align-items:center;justify-content:center;padding:2rem;box-sizing:border-box;" +
      "background:var(--tali-scrim);}" +
      "#tali-error.tali-show{display:flex;}" +
      "#tali-error .tali-error-card{max-width:min(680px,92vw);width:100%;max-height:74vh;overflow:auto;" +
      "background:var(--tali-bg);border:1px solid var(--tali-border);" +
      "border-left:2px solid var(--tali-status-error);border-radius:var(--tali-radius);" +
      "padding:1rem 1.2rem;}" +
      "#tali-error .tali-error-title{font:400 .78rem/1.3 var(--tali-font-mono);" +
      "text-transform:uppercase;letter-spacing:.053em;color:var(--tali-status-error);" +
      "margin-bottom:.55rem;}" +
      "#tali-error pre{margin:0;padding:0;background:transparent;white-space:pre-wrap;word-break:break-word;" +
      "font:var(--tali-mono-size)/1.5 var(--tali-font-mono);color:var(--tali-fg);}" +
      "#tali-error .tali-error-hint{font:400 .78rem/1.3 var(--tali-font-mono);" +
      "text-transform:uppercase;letter-spacing:.053em;margin-top:.85rem;color:var(--tali-muted);}";
```

and the card's markup loses its emoji:

```js
    el.innerHTML =
      '<div class="tali-error-card"><div class="tali-error-title">Render failed</div><pre></pre>' +
      '<div class="tali-error-hint">Fix the source and save; this clears on the next successful render. (Esc to dismiss)</div></div>';
```

Add a line to the comment above the IIFE recording what changed and why it is not in
`STATUS_CSS`:

```js
  // Styled off the token layer like every other dev surface, and it is the one that carried
  // a whole second design system: a hardcoded #1b1d23 card with its own red, its own shadow
  // and a backdrop blur, drawn identically whatever palette the reader chose. It keeps its
  // own <style> because it must render when the page's own render FAILED, which is exactly
  // when assuming STATUS_CSS arrived is a bad bet — the tokens it reads are in the inlined
  // token sheet, which is in <head> and not in the render that failed.
```

- [ ] **Step 5: The cell badges speak words**

In `applyCellState` and its ticker, replace the five emoji with the machine voice's own
vocabulary — the badge is already `font: … var(--tali-font-mono)` and uppercase by inheritance
once Step 3 lands:

```js
      badge.textContent = "running 0.0s";
```
```js
      if (msg.state === "done") badge.textContent = msg.source === "cache" ? "cached" : (msg.duration_ms != null ? fmtElapsed(msg.duration_ms) : "done");
      else if (msg.state === "error") badge.textContent = "failed";
      else badge.textContent = "queued";
```
and in the 200 ms ticker: `b.textContent = "running " + fmtElapsed(now - runningTimers[id]);`

Then the three tab titles (`client.js:881, 897, 915`) lose their `⚠`/`●`:

```js
      document.title = "error — " + baseTitle;
```
```js
      document.title = "starting kernel… — " + baseTitle;
```
```js
    document.title = "building… — " + baseTitle;
```

Update the comment at `:706-708`, which explains the `⚡ cached` / `✓ 1.2s` distinction it no
longer describes: a cache replay carries no duration, so it says `cached` where a fresh run
says its elapsed time — the distinction survives, only the glyph goes.

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_dev_ui_carries_no_generated_design_tells_either \
                            the_dev_ui_paints_status_from_scored_tokens_only
cd web-client && npx -y -p typescript tsc -p jsconfig.json; cd -
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. `log.rs`'s `keys_hint_names_the_corner_the_stylesheet_puts_the_menu_in` slices
`STATUS_CSS` on `#tali-controls.tali-dev {` and reads `bottom`/`left` out of the rule; Step 3
keeps both, so it should stay green — if it reddens, the rule was reflowed and the test is
telling you the truth.

- [ ] **Step 7: Render it — the dev UI has no other instrument**

```bash
cargo run -p taliesin-server -- preview corpus/analyst/index.tmd 4388
```

Via the chrome-devtools MCP: open the dev menu, screenshot it in both palettes, and force the
error overlay to confirm it is theme-aware for the first time:

```js
document.getElementById('tali-error').classList.add('tali-show');
document.querySelector('#tali-error pre').textContent = 'demo';
```

Expected: a paper-coloured card in light mode, an ink-coloured one in dark, a 2px error rule,
no shadow, no blur, and a title in the mono voice. Also confirm a code cell's badge reads
`RUNNING 0.4s` → an elapsed time or `CACHED`, with no glyph.

- [ ] **Step 8: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the dev UI takes the theme's geometry and its voice

Three box-shadows, two 999px pills, five other radii and three borrowed font stacks
go. Spec §12.2's tell probe has asserted every one of these since Plan 1 — on the five
bundled stylesheets, which is the set where the doctrine had already been applied.
That is the same hole §12.1 widened the vendor-hex ban for, and it is exactly how a
ui-sans-serif/system-ui stack survived in client.js after Plan 1 removed it from every
sheet and gated against it. The probe now scans the two files that render the dev UI,
so the stack dies because the file became scannable rather than because a line was
patched.

The error overlay was the worst of it: a hardcoded #1b1d23 card with its own red, its
own shadow and a backdrop blur — the third verbatim blur after the two §3 named —
drawn identically whatever palette the reader had chosen. It is now the reader's own
paper with a 2px status rule.

The five surviving emoji become words. A cache replay still reads differently from a
fresh run (CACHED against an elapsed time); it just no longer needs a lightning bolt
to say so.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 4: Two sticky bars become one

**Files:**
- Modify: `crates/core/assets/css/site.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-chrome-maxw`, `--tali-u`, `--tali-font-mono`.
- Produces: the shared selector list `:is(.tali-site-nav, .tali-book-topbar)` and
  `:is(.tali-nav-inner, .tali-book-topbar-inner)`. Task 6 reads neither; Task 8 puts their
  spacing on the scale.

**Why.** Plan 1 scoped "navbar/topbar merge" to this plan. The two rules are byte-identical:

```css
.tali-site-nav   { position: sticky; top: 0; z-index: 50;
  background: color-mix(in srgb, var(--tali-bg) 88%, transparent);
  border-bottom: 1px solid var(--tali-border); }
.tali-book-topbar{ position: sticky; top: 0; z-index: 50;
  background: color-mix(in srgb, var(--tali-bg) 88%, transparent);
  border-bottom: 1px solid var(--tali-border); }
```

and their inner boxes differ in exactly one value (`padding: .6rem 1rem` against
`.45rem 1rem`). Two copies of a rule that must agree is the same defect the reading grid fixed
at a larger scale, and the copies have already drifted once — the book bar is 3px shorter than
the website bar for no recorded reason, while `--tali-nav-h` (the scroll-margin offset both
`[data-block-id]` rules read) is declared as `3.25rem` on one body and `3rem` on the other.

Per **R3** the merged bar is also **opaque**. `color-mix(… 88%, transparent)` is what is left
of a glassmorphic bar after Plan 1 deleted its `backdrop-filter`, and without the blur it is
not a style: it is body text visibly scrolling through the chrome. §12.2's own message for the
blur ban states the replacement — "an opaque ground and a 1px rule read the same".

- [ ] **Step 1: Write the failing test**

```rust
/// The website navbar and the book topbar are ONE bar with two garnishes.
///
/// They were two byte-identical rules whose inner boxes differed by one padding value, and
/// they had already drifted: the book bar is 3px shorter than the website bar for no recorded
/// reason, while `--tali-nav-h` — the offset every `[data-block-id]` scroll-margin and the
/// sticky TOC read — was declared 3.25rem on one body and 3rem on the other, so a jumped-to
/// heading cleared the bar by a different amount in a book than on a site.
///
/// And the bar is OPAQUE. `color-mix(… 88%, transparent)` is the residue of a glassmorphic bar
/// after Plan 1 deleted its backdrop-filter; without the blur it is not a translucency effect,
/// it is body text scrolling visibly through the chrome.
#[test]
fn the_two_sticky_bars_are_one_rule_and_the_bar_is_opaque() {
    assert!(
        !SITE_CSS.contains("88%, transparent"),
        "a sticky bar over scrolling prose is opaque: the blur that made translucency \
         legible was deleted in Plan 1 and only the transparency was left behind"
    );
    // One `position: sticky; top: 0` bar rule, shared.
    assert!(
        SITE_CSS.contains(":is(.tali-site-nav, .tali-book-topbar)"),
        "the two bars share one rule"
    );
    assert!(
        SITE_CSS.contains(":is(.tali-nav-inner, .tali-book-topbar-inner)"),
        "the two inner boxes share one rule"
    );
    // Neither may re-declare the shared properties on its own afterwards.
    for sel in [".tali-site-nav {", ".tali-book-topbar {"] {
        assert!(
            !SITE_CSS.contains(sel),
            "`{sel}` is a second definition of a bar that now has one"
        );
    }
    // One bar, one height. The token both `scroll-margin-top` rules read must agree.
    let heights: std::collections::BTreeSet<&str> = SITE_CSS
        .split("--tali-nav-h:")
        .skip(1)
        .map(|s| s.split(';').next().unwrap_or("").trim())
        .collect();
    assert_eq!(
        heights.len(),
        1,
        "--tali-nav-h has {} values {heights:?}; a jumped-to heading would clear the bar by \
         a different amount depending on which one it is under",
        heights.len()
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_two_sticky_bars_are_one_rule_and_the_bar_is_opaque
```

Expected: FAIL on the `88%, transparent` assertion.

- [ ] **Step 3: Merge the two bars**

In `site.css`, replace `.tali-site-nav` (`:22-24`) with the shared rule, and delete
`.tali-book-topbar`'s copy (`:275-277`) where it sits further down:

```css
  /* ONE sticky bar, worn two ways: the website navbar and a book's topbar were two
     byte-identical rules whose inner boxes differed by a single padding value — and they had
     already drifted, the book bar sitting 3px shorter with no recorded reason while
     `--tali-nav-h` said 3.25rem on one body and 3rem on the other. That token is what every
     `[data-block-id]`'s scroll-margin and the sticky TOC's `top` read, so a heading jumped to
     from a TOC cleared the bar by a different amount in a book than on a site.

     OPAQUE, deliberately. This carried `color-mix(in srgb, var(--tali-bg) 88%, transparent)`,
     which is what is left of a glassmorphic bar once its `backdrop-filter` is deleted — and
     without the blur that is not a translucency effect, it is body text scrolling visibly
     through the chrome. The tell probe's own message for the blur ban states the replacement:
     an opaque ground and a 1px rule read the same. */
  :is(.tali-site-nav, .tali-book-topbar) { position: sticky; top: 0; z-index: 50;
    background: var(--tali-bg); border-bottom: 1px solid var(--tali-border); }
  :is(.tali-nav-inner, .tali-book-topbar-inner) { position: relative;
    max-width: var(--tali-chrome-maxw); margin: 0 auto; padding: calc(.25 * var(--tali-u)) 1rem;
    display: flex; align-items: center; gap: 1.1rem;
    font: 400 .78rem/1.3 var(--tali-font-mono); text-transform: uppercase;
    letter-spacing: .053em; }
  /* The one thing a book's bar really does differ in: it holds a drawer button and a title
     rather than seven links, so its items sit closer. */
  .tali-book-topbar-inner { gap: .7rem; }
```

Then set one height. `body.tali-site` (`:9`) and `body.tali-book-body` (`:269`) each declare
`--tali-nav-h`; both take `3.25rem`, which is the value the *taller* of the two bars had and
therefore the one that cannot under-clear:

```css
    /* Height of the sticky bar; the TOC and jumped-to headings clear it. ONE value for both
       bars, because there is one bar (above): the site said 3.25rem and the book 3rem, so a
       heading jumped to in a book cleared the bar by 3px less than the same heading on a
       site. The larger of the two is the safe merge. */
    --tali-nav-h: 3.25rem;
```

Delete `.tali-book-topbar-inner`'s now-duplicated declarations, keeping only the `gap` above.

- [ ] **Step 4: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_two_sticky_bars_are_one_rule_and_the_bar_is_opaque
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. `book_topbar_title_truncates_instead_of_wrapping_the_sticky_bar_taller` and
`touch_nav_tap_target_grows_without_growing_the_sticky_bar` both slice `site.css` for topbar
rules — read either failure rather than adjusting it; both are measured MOB findings.

- [ ] **Step 5: Render it — a sticky bar is pure layout**

```bash
cargo run -p taliesin-server -- preview docs/guide 4388
```

Via the chrome-devtools MCP, on a book page and a site page:

```js
// The bar is opaque: nothing shows through it.
getComputedStyle(document.querySelector('.tali-site-nav, .tali-book-topbar')).backgroundColor
// The two bars are the same height, and --tali-nav-h describes it.
const bar = document.querySelector('.tali-site-nav, .tali-book-topbar');
[bar.getBoundingClientRect().height,
 getComputedStyle(document.documentElement).fontSize,
 getComputedStyle(document.body).getPropertyValue('--tali-nav-h')]
// No horizontal overflow at either width.
[document.documentElement.scrollWidth, document.documentElement.clientWidth]
```

Then scroll to a mid-page heading via a TOC link and screenshot: the heading must clear the
bar, not sit under it. Repeat at 1280px and 700px.

- [ ] **Step 6: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): two sticky bars become one, and the bar goes opaque

The website navbar and a book's topbar were byte-identical rules whose inner boxes
differed by one padding value — and they had already drifted. --tali-nav-h, the token
every [data-block-id] scroll-margin and the sticky TOC's own `top` reads, said 3.25rem
on one body and 3rem on the other, so a heading jumped to from a book's TOC cleared
the bar by 3px less than the same heading on a site. One bar, one height, the larger
of the two.

The bar is opaque now. color-mix(… 88%, transparent) is what is left of a glassmorphic
bar once Plan 1 deleted its backdrop-filter, and without the blur that is not a
translucency effect: it is body text scrolling visibly through the chrome. The tell
probe's own message for the blur ban already stated the replacement.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 5: A listing is a ruled list, and spec §9's cut #12 lands whole

**Files:**
- Modify: `crates/core/assets/css/site.css`
- Modify: `crates/core/src/site/mod.rs`, `crates/core/src/site/chrome.rs`,
  `crates/core/src/render/mod.rs`
- Modify: the corpus and docs pins the cut features carry (found in Step 5)
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-u`, `--tali-border`, `--tali-font-mono`, `--tali-measure`.
- Produces: nothing later tasks read. `card_html`'s signature is unchanged; it emits fewer
  children.

**Why.** Per **R4** a card is a box, and spec §3 puts radius `0` on cards while §1 puts no
colour in the furniture — a card with neither is a bordered rectangle around a date and a
title, which is a rule with extra steps. Per **R6** spec §9's cut #12 (category chips, monogram
placeholder, reading time, chapter word counts, ~92 lines) lands here, whole, because three of
its four items are exactly this chrome and splitting a numbered cut is how half of one survives.

Two carried-forward defects are fixed by the same change: `.tali-card-title` is `1.08rem` =
**17.28 px**, under the 20 px prose — the exact defect `the_serif_reading_scale_never_drops_below_the_body`
gates for elsewhere and cannot see here because the selector is not on its list. And
`.tali-site-main.tali-wide > main { --tali-measure: 60rem; }` is a local token override Plan 2
left standing for these pages; a ruled list at the reading measure does not need it.

**Verified facts:** the emitted markup is `<li class="tali-listing-item"><a class="tali-card"
…>{img}<div class="tali-card-body">{draft_badge}{date}<h3 class="tali-card-title">{title}</h3>
{desc}{cats}</div></a></li>` (`site/mod.rs:1438`). Cut #12 removes `{cats}` (`:1426`) and the
`.tali-card-noimg` monogram branch (`:1391`); the `<ul role="list">` wrapper, the `<li>`, the
`<a>` and the block ids are all untouched, so `crates/core/tests/corpus.rs`'s invariants and
the eight `class=\"tali-card\"` assertions in `site/mod.rs`'s own tests keep their subject.

- [ ] **Step 1: Write the failing test**

```rust
/// A listing is a ruled list. A "card" in this theme is a box with radius 0, no ground and no
/// colour — which is a hairline rule with extra steps (ruling R4) — and its title was 17.28px
/// against a 20px body, the exact defect `the_serif_reading_scale_never_drops_below_the_body`
/// gates for on every selector it knows about and could not see on this one.
///
/// Spec §9's cut #12 lands here whole: category chips, the monogram placeholder, the reading
/// time and the chapter word counts. Splitting a numbered cut across tasks is how half of one
/// survives and is then defended by the pin that outlived it.
#[test]
fn a_listing_is_a_ruled_list_and_cut_12_landed() {
    for (needle, why) in [
        (".tali-card-cats", "category chips: spec §9 cut #12"),
        (".tali-cat ", "category chips: spec §9 cut #12"),
        (".tali-card-noimg", "the monogram placeholder: spec §9 cut #12"),
        (".tali-chap-words", "chapter word counts: spec §9 cut #12"),
        (".tali-listing-grid", "a listing is one ruled list, not two layouts"),
    ] {
        assert!(!SITE_CSS.contains(needle), "site.css still styles `{needle}`: {why}");
    }
    // The row is a rule, not a box.
    assert!(
        SITE_CSS.contains(".tali-card { display: flex; flex-direction: column;"),
        "the card keeps its flex column"
    );
    assert!(
        !SITE_CSS.contains("border: 1px solid var(--tali-border); border-radius: var(--tali-radius); overflow: hidden; background: var(--tali-bg);"),
        "the card's box is a rule now"
    );
    // A listing title is prose and may not sit under the prose it lists.
    let title = declaration_in(SITE_CSS, ".tali-card-title {", "font-size")
        .expect(".tali-card-title sets a font-size");
    assert!(
        rem_px(title) >= 20.0,
        "a listing title is {}px against a 20px body; the small register in this theme is \
         the MONO voice, not a shrunken serif",
        rem_px(title)
    );
    // The measure override Plan 2 left for these pages is gone with the grid that needed it.
    assert!(
        !SITE_CSS.contains("--tali-measure: 60rem"),
        "a ruled list reads at the reading measure; the local token override went with the \
         card grid it was widening"
    );

    // The cut is in the EMITTERS too, not only the sheet: a register entry or a deleted rule
    // leaves the markup shipping.
    let root = repo_root();
    for (rel, needle, why) in [
        ("crates/core/src/site/mod.rs", "tali-card-cats", "chips"),
        ("crates/core/src/site/mod.rs", "tali-card-noimg", "the monogram"),
        ("crates/core/src/site/chrome.rs", "tali-chap-words", "chapter word counts"),
        ("crates/core/src/render/mod.rs", "min read", "the reading-time estimate"),
    ] {
        let text = std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(!text.contains(needle), "{rel} still emits {why} (`{needle}`): spec §9 cut #12");
    }
}
```

`declaration_in` and `rem_px` are the helpers `the_serif_reading_scale_never_drops_below_the_body`
already uses; both are in this file.

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core a_listing_is_a_ruled_list_and_cut_12_landed
```

Expected: FAIL on `site.css still styles .tali-card-cats`.

- [ ] **Step 3: Make the listing a ruled list**

In `site.css`, replace the block from `.tali-listing {` through `.tali-cat { … }`
(`:150-204`) with:

```css
  /* A LISTING IS A RULED LIST. It was a grid of bordered cards, and a "card" in this theme is
     a box with radius 0 (spec §3), no ground and no colour (spec §1) — which is a hairline
     rule with extra steps. So it is the rule: a row per post, a hairline between rows, the
     date in the machine voice above a serif title at the prose size, and the description
     under it. The `<ul role="list">` / `<li>` / `<a>` markup is unchanged; only its dress is.

     One layout, not two. `.tali-listing-grid` and `.tali-listing-default` were a thumbnail
     grid and a stacked row, and the grid's whole reason was to give a 16:9 image somewhere to
     sit — which is also what needed the monogram placeholder cut #12 deletes, since a mixed
     listing left a text-only post floating beside its imaged neighbours. A ruled list has no
     such alignment to keep: a post with an image shows it, a post without simply does not. */
  .tali-listing { margin: calc(2 * var(--tali-u)) 0; padding: 0; list-style: none;
    border-top: 1px solid var(--tali-border); }
  .tali-listing > .tali-listing-item { display: flex; margin: 0; padding: 0;
    border-bottom: 1px solid var(--tali-border); }
  .tali-listing > .tali-listing-item > .tali-card { flex: 1 1 auto; min-width: 0; }
  .tali-card { display: flex; flex-direction: column; text-decoration: none;
    color: var(--tali-fg); padding: var(--tali-u) 0; }
  /* Hover changes a ground and nothing else — it may not move anything (spec §3), and the
     row has no border of its own left to tint. Keyboard focus gets the same affordance. */
  .tali-card:hover, .tali-card:focus-visible { background: var(--tali-code-bg); }
  .tali-card-img { width: 100%; max-width: 12rem; aspect-ratio: 16 / 9; object-fit: cover;
    display: block; margin-bottom: calc(.5 * var(--tali-u)); background: var(--tali-code-bg); }
  .tali-card-body { display: flex; flex-direction: column; gap: calc(.25 * var(--tali-u)); }
  .tali-card-date { display: block; font: 400 .78rem/1.3 var(--tali-font-mono);
    color: var(--tali-muted); text-transform: uppercase; letter-spacing: .053em; }
  /* A listing title is the author's own headline, at the size the prose it lists is set in.
     It was 1.08rem = 17.28px against a 20px body — the defect the reading-scale gate catches
     everywhere it can see, on a selector that gate does not know about. */
  .tali-card-title { font-weight: 600; font-size: 1.25rem; line-height: 1.25; margin: 0; }
  .tali-card-desc { margin: 0; font-size: .92rem; color: var(--tali-muted); line-height: 1.5; }
```

Delete `.tali-listing-grid`, `.tali-listing-default`, `.tali-listing-default .tali-card`,
`.tali-listing-default .tali-card-img`, `.tali-card-noimg` and its `.tali-listing-default`
twin, `.tali-card-cats`, `.tali-cat`, and the `.tali-card:hover .tali-card-title` rule (a
no-op: `--tali-link` and `--tali-fg` are the same value in both palettes, and the title already
inherits `--tali-fg` from `.tali-card`). Delete the `@media (max-width: 40rem)` block's two
`.tali-listing-default` rules; keep the `.tali-search-kbd` and nav rules that share the block.

Delete `.tali-site-main.tali-wide > main { --tali-measure: 60rem; }` (`:101-104`) and its
comment, and the `.tali-site-main.tali-wide > main:has(.tali-listing) > :where(…)` rule
(`:159-171`) whose entire job was clamping the intro prose while the card grid stayed wide.

- [ ] **Step 4: Land cut #12 in the emitters**

`crates/core/src/site/mod.rs`:
- delete the `cats` binding (the `.tali-card-cats` `format!` around `:1420-1426`) and drop
  `{cats}` from the `card_html` template at `:1439`;
- delete the `.tali-card-noimg` monogram branch (`:1388-1393`), leaving the `img` binding as
  the image case and an empty string otherwise. Read the surrounding `let img = …` before
  editing: the two arms share one `match`/`if let`.

`crates/core/src/site/chrome.rs:517`: delete the
`.map(|w| format!(" <span class=\"tali-chap-words\">{w}</span>"))` link in the chain and
whatever now-unused `words` binding feeds it. `book.rs`'s `word_count` call
(`book.rs:217-223`) and the `Chapter::words` field go **only if nothing else reads them** —
grep first; `lint` and the outline use `prose::word_count` for their own purposes and must not
lose it.

`crates/core/src/render/mod.rs:1241-1249`: delete the `read_time` binding and its slot in the
title-block meta line. Read the `format!` that consumes it before editing — the separator
between the date and the reading time goes with it, or the meta line ships a dangling `·`.

- [ ] **Step 5: Delete the pins, in this commit (the ordering rule)**

Find them rather than assuming:

```bash
grep -rln "categories:\|category:" corpus/ docs/ site/ --include=*.tmd
grep -rn "reading time\|min read\|word count\|chapter length" docs/guide docs/internals
grep -rn "tali-cat\|tali-card-noimg\|tali-chap-words" crates/ editor/ --include=*.rs --include=*.ts --include=*.json
```

For each hit: a corpus document that exists to witness a cut feature loses that section; a docs
page that documents it loses the row or the sentence. **A corpus document deleted ahead of the
code it guards leaves that code silently unguarded while every gate still passes** — so this is
the same commit, not an earlier one and not a later one.

`categories:` is front matter. If cut #12 removes the only thing that *renders* it while the
key is still parsed, that is the half-retirement CLAUDE.md warns about (`listing: sort:`
answered "deleted" for eleven days while `parse_listing_spec` still honoured it). Decide
explicitly: either the key keeps a use (search facets, the feed) — in which case say so in the
commit — or it takes a `RETIRED_KEYS` line **and** the parser stops reading it, with a
parser-side pin. Check what reads it before choosing:

```bash
grep -rn "categories" crates/core/src/site/ crates/core/src/frontmatter.rs
```

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core a_listing_is_a_ruled_list_and_cut_12_landed
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. Several `site/mod.rs` tests assert on `class=\"tali-card\"` counts and ordering
— those must stay green untouched, because the markup they pin is unchanged. Any test asserting
on `tali-card-noimg`, `tali-cat` or a reading-time string was pinning the cut anatomy and its
assertion goes with the feature.

- [ ] **Step 7: Render it — this is the largest layout change in the plan**

```bash
cargo run -p taliesin-server -- preview site 4388
```

Via the chrome-devtools MCP, on the blog/projects index:

```js
// Rows are rules, not boxes: no border except the hairline separators.
[...document.querySelectorAll('.tali-card')].slice(0,3).map(c => {
  const s = getComputedStyle(c);
  return [s.borderTopWidth, s.borderRadius, s.backgroundColor, c.getBoundingClientRect().width];
})
// Titles clear the prose size.
getComputedStyle(document.querySelector('.tali-card-title')).fontSize
// No overflow at either width.
[document.documentElement.scrollWidth, document.documentElement.clientWidth]
```

Screenshot at 1280px and at 700px. Watch specifically for: a listing whose intro paragraph now
sits at a different width than the rows (the `:has(.tali-listing)` rule that clamped it is
gone, so both should be the measure), and a post *with* an image — the image is now a 12rem
block above the title rather than a 16:9 cover, and it must not be stretched.

- [ ] **Step 8: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): a listing is a ruled list, and cut #12 lands whole

A card in this theme is a box with radius 0 (§3), no ground and no colour (§1), which
is a hairline rule with extra steps. So it is the rule: a row per post, a hairline
between rows, the date in the machine voice above a serif title, the description under
it. The <ul role="list">/<li>/<a> markup is untouched; only its dress changed.

The title was 1.08rem = 17.28px against a 20px body — the defect
the_serif_reading_scale_never_drops_below_the_body catches on every selector it knows
about, on one it did not know about. It is the prose size now.

Spec §9's cut #12 lands whole rather than in pieces: category chips, the monogram
placeholder, the reading-time estimate and the chapter word counts, with their pins in
the same commit. The monogram existed to keep a text-only post aligned beside its
imaged neighbours in a grid; a ruled list has no such alignment to keep.

Two layouts become one. .tali-listing-grid existed to give a 16:9 thumbnail somewhere
to sit, and .tali-site-main.tali-wide > main's local --tali-measure: 60rem override
existed to widen the container that held it. Both go with it.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 6: The book drawer

**Files:**
- Modify: `crates/core/assets/css/site.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-scrim`, `--tali-radius`, `--tali-dur`, `--tali-u`, `--tali-font-mono`.
- Produces: nothing.

**Why.** The drawer is a book's only in-chapter outline (item 76) and is the last chrome
surface still carrying its own design decisions: `.tali-book-part` is `font-size: .76rem;
font-weight: 700; letter-spacing: .04em; text-transform: uppercase` — a *fifth* voice, neither
the machine voice (`400`/`.78rem`/`.053em`) nor the serif, on a label the tool generates from
`_site.yml`'s `parts:`; `.tali-book-part-nested` is a sixth; and
`animation: tali-book-drawer-in .16s ease` is a second motion duration against §3's single
`--tali-dur: .1s` (which `--tali-dur-slow` having zero consumers is the same finding about).

The part label is worth care: `parts:` names are **authored**, so under §4's amended rule the
container may not take the machine voice. What the tool generates there is nothing — the whole
row is the author's word. It goes to the serif.

- [ ] **Step 1: Write the failing test**

```rust
/// The book drawer stops carrying its own type scale and its own clock.
///
/// `.tali-book-part` was `.76rem/700/.04em/uppercase` and `.tali-book-part-nested` a variation
/// on it — a fifth and sixth voice in a theme that owns two. And they hold `parts:` names,
/// which the author wrote: under spec §4's rule the machine voice attaches to a label the TOOL
/// generates and never to a container that may hold the AUTHOR's text, so the answer is not
/// "make it the machine voice" but "make it the serif".
#[test]
fn the_drawer_speaks_in_the_themes_two_voices_and_one_clock() {
    // One duration. `.16s` was a second clock on the one surface that animates.
    let durations: std::collections::BTreeSet<&str> = SITE_CSS
        .split("animation:")
        .skip(1)
        .filter_map(|s| s.split(';').next())
        .flat_map(|s| s.split_whitespace())
        .filter(|w| w.ends_with('s') && w.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.'))
        .collect();
    assert!(
        durations.is_empty(),
        "site.css animates for {durations:?}; the one duration is var(--tali-dur)"
    );
    // A `parts:` name is the author's word, so it is the serif — not a sixth voice, and not
    // the machine voice either (spec §4's rule).
    for (sel, banned) in [
        (".tali-book-part {", "text-transform: uppercase"),
        (".tali-book-part {", "letter-spacing: .04em"),
    ] {
        let rule = SITE_CSS
            .split_once(sel)
            .unwrap_or_else(|| panic!("{sel} exists"))
            .1
            .split_once('}')
            .expect("the rule is closed")
            .0;
        assert!(
            !rule.contains(banned),
            "`{sel}` still carries `{banned}`, on a label the AUTHOR wrote (`parts:`)"
        );
    }
    assert!(
        !SITE_CSS.contains(".tali-book-part-nested"),
        "a nested part is a part; a second rule for it was a sixth voice"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_drawer_speaks_in_the_themes_two_voices_and_one_clock
```

Expected: FAIL on `site.css animates for {".16s"}`.

- [ ] **Step 3: Put the drawer on the theme**

In `site.css`:

```css
  .tali-book-drawer-panel { position: absolute; top: 0; left: 0; bottom: 0;
    width: min(20rem, 86vw); overflow-y: auto; overscroll-behavior: contain;
    padding: var(--tali-u) calc(.5 * var(--tali-u)) calc(2 * var(--tali-u));
    background: var(--tali-bg); border-right: 1px solid var(--tali-border);
    animation: tali-book-drawer-in var(--tali-dur) ease; }
```

and the part headings, one rule instead of two:

```css
  /* A `parts:` name is a word the AUTHOR wrote, so it is the serif — spec §4's rule attaches
     the machine voice to labels the TOOL generates and never to a container that may hold
     author text. It was `.76rem/700/.04em/uppercase`, which is neither of this theme's two
     voices; the nested variant was a second one of those. Depth is expressed by indent, which
     is what depth means, rather than by a second type treatment. */
  .tali-book-part { margin: var(--tali-u) 0 calc(.25 * var(--tali-u));
    font-size: .92rem; font-weight: 600; color: var(--tali-muted); }
  .tali-book-part[data-depth] { margin-left: .9rem; }
```

Read `chrome.rs`'s emission of `.tali-book-part-nested` before writing `[data-depth]`: if the
nested case is a distinct class rather than an attribute, keep the class in the selector list
rather than inventing an attribute nothing emits — `every_emitted_attribute_has_a_runtime_consumer`
runs the other direction and a `data-depth` nothing sets is a rule nothing reaches. Grep:

```bash
grep -rn "tali-book-part" crates/core/src/site/
```

Also delete `.tali-book-drawer-close:hover`'s `background: var(--tali-code-bg)` **only if** it
also changes the colour — it does (`--tali-muted` → `--tali-fg`), so it stays. Leave it.

- [ ] **Step 4: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_drawer_speaks_in_the_themes_two_voices_and_one_clock
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
```

Expected: PASS. `book_drawer_close_button_clears_the_wcag_tap_target_floor` is a measured MOB
finding and must stay green untouched.

- [ ] **Step 5: Render it**

```bash
cargo run -p taliesin-server -- preview docs/internals 4388
```

Via the chrome-devtools MCP: click the Chapters button, screenshot the open drawer, and check
the part headings and chapter rows read as one list rather than three type treatments. Confirm
the panel does not overflow: `document.querySelector('.tali-book-drawer-panel').scrollWidth`
against its `clientWidth`. Then re-check with `prefers-reduced-motion: reduce` emulated — the
`@media (prefers-reduced-motion: reduce)` rule sets `animation: none` and must still win.

- [ ] **Step 6: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the drawer speaks in the theme's two voices and one clock

.tali-book-part was .76rem/700/.04em/uppercase and .tali-book-part-nested a variation
on it: a fifth and a sixth voice in a theme that owns two. They hold `parts:` names,
which the author wrote — so under spec §4's rule the answer is not to give them the
machine voice but to give them the serif. Depth is an indent now, which is what depth
means.

The drawer's .16s slide was the last second clock in the tree; it takes --tali-dur.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 7: The landing page becomes an editorial masthead and prose

**Files:**
- Modify: `site/index.tmd`
- Modify: `crates/core/assets/css/base.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-u`, `--tali-measure`, the heading ladder (Plan 2).
- Produces: nothing.

**Why.** Spec §15.2, an author decision taken 2026-08-14 and recorded so no future session
reopens it: *"The marketing landing page becomes an editorial masthead and prose. No centred
hero, no letterspaced all-caps eyebrow, no three-card feature grid, no repeated bottom CTA. The
feature grid becomes a definition list or ruled sections. This is a rewrite of `site/index.tmd`,
not only a CSS change."*

Per **R5** `hero.eyebrow` keeps its key and loses its voice: `.hero-eyebrow` is
`font: 600 .8rem/1 var(--tali-font-mono); letter-spacing: .12em; text-transform: uppercase`
applied to an authored string — a *fourth* voice (not even the machine voice's
`400`/`.78rem`/`.053em`), on author text, which §4's amended rule forbids outright.

The masthead form already exists in the tree: `base.css:150-154` gives a hero on a
reading-measure page a left-aligned treatment with a short ink hairline. **This makes that form
the only form** and deletes the centred `.tali-wide` branch — which is also the branch whose
`clamp(2rem, 6vw, 3.2rem)` is the last viewport-relative type size on the site.

- [ ] **Step 1: Write the failing test**

```rust
/// Spec §15.2: the landing page is an editorial masthead and prose. The masthead form already
/// existed for reading-measure pages (`base.css`'s `:not(.tali-wide)` branch); this makes it
/// the ONLY form, which is what deletes the centred hero, the viewport-scaled headline and
/// the three-card feature grid in one go.
#[test]
fn the_landing_page_is_a_masthead_and_prose() {
    for (needle, why) in [
        ("text-align: center", "no centred hero (spec §15.2)"),
        ("clamp(", "the last viewport-relative type size went with the centred hero"),
        ("justify-content: center", "the hero's actions sit where the prose starts"),
        (".feature-grid", "the three-card feature grid becomes ruled sections"),
    ] {
        assert!(!BASE_CSS.contains(needle), "base.css still has `{needle}`: {why}");
    }
    // The eyebrow is the AUTHOR's word (`_site.yml` / front-matter `hero.eyebrow:`), so it may
    // not wear the machine voice — spec §4's rule, which this shipped against in a FOURTH
    // voice: 600 weight, .8rem, .12em tracking, uppercase.
    let eyebrow = BASE_CSS
        .split_once(".hero-eyebrow {")
        .expect(".hero-eyebrow exists")
        .1
        .split_once('}')
        .expect("the rule is closed")
        .0;
    for banned in ["text-transform: uppercase", "var(--tali-font-mono)", "letter-spacing: .12em"] {
        assert!(
            !eyebrow.contains(banned),
            "`.hero-eyebrow` still carries `{banned}` on text the author wrote"
        );
    }

    // And the page itself, not only its stylesheet: §15.2 calls this a rewrite of index.tmd.
    let index = std::fs::read_to_string(repo_root().join("site/index.tmd")).expect("index.tmd");
    assert!(
        !index.contains("{.feature-grid}") && !index.contains("{.feature}"),
        "the three-card feature grid is still authored in the landing page"
    );
    assert_eq!(
        index.matches("{.btn").count(),
        0,
        "the repeated bottom CTA and its button ladder go with the centred hero; the \
         masthead's own actions come from the `hero:` front matter"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p taliesin-core the_landing_page_is_a_masthead_and_prose
```

Expected: FAIL on `base.css still has text-align: center`.

- [ ] **Step 3: The masthead becomes the only hero**

In `base.css`, replace `:136-154` (the `.hero` block and its `:not(.tali-wide)` overrides) with:

```css
  /* THE MASTHEAD. Spec §15.2, an author decision: a landing page is an editorial masthead and
     prose — no centred hero, no letterspaced all-caps eyebrow, no three-card feature grid, no
     repeated bottom CTA. The left-aligned form already existed here for reading-measure pages
     and is now the only form, which is what deletes the centred branch, its
     `clamp(2rem, 6vw, 3.2rem)` (the last viewport-relative type size on the site) and the
     `justify-content: center` on its actions.

     The eyebrow is the AUTHOR's word — `hero.eyebrow:` in front matter — so it takes the
     serif, not the machine voice: spec §4's rule attaches the voice to labels the TOOL
     generates and never to a container that may hold author text. It shipped in a FOURTH
     voice (600 weight, .8rem, .12em tracking, uppercase), which is neither of the two this
     theme owns. */
  .hero { padding: calc(2 * var(--tali-u)) 0 var(--tali-u); }
  .hero h1, .hero h2 { font-size: 2.25rem; line-height: 1.1; letter-spacing: -.008em;
    margin: 0 0 calc(.5 * var(--tali-u)); }
  .hero p, .hero .hero-lead { font-size: 1.25rem; color: var(--tali-muted);
    max-width: none; margin: 0 0 var(--tali-u); line-height: 1.5; }
  .hero-eyebrow { font: var(--tali-font-body); font-size: .92rem; font-style: italic;
    color: var(--tali-muted); text-transform: none; letter-spacing: normal;
    margin: 0 0 calc(.5 * var(--tali-u)); }
  /* The hairline under the eyebrow is the masthead's one mark, and it is the ink: a rule the
     width of a word, not a coloured band. */
  .hero-eyebrow::after { content: ""; display: block; width: 2.5rem; height: 2px;
    background: var(--tali-fg); margin-top: calc(.25 * var(--tali-u)); }
  .hero-actions { display: flex; flex-wrap: wrap; gap: .6rem;
    margin-top: var(--tali-u); }
  .hero a.btn, .hero-actions a.btn { margin: 0; }
```

Then replace `.feature-grid` / `.feature` / `.feature h3` / `.feature p` (`:155-163`) with
ruled sections — the "definition list or ruled sections" §15.2 names:

```css
  /* Ruled sections, not cards (spec §15.2). A `.feature` was a bordered, tinted box; the
     theme's structure is square, unboxed and separated by a hairline, exactly as a callout is
     a rule and a listing is a rule. `.feature h3` keeps taking the h3 size — the gate that
     pins it against the body reads this selector by name. */
  .feature-list { display: flex; flex-direction: column; margin: calc(2 * var(--tali-u)) 0;
    border-top: 1px solid var(--tali-border); }
  .feature { padding: var(--tali-u) 0; border-bottom: 1px solid var(--tali-border); }
  .feature h3 { margin: 0 0 calc(.25 * var(--tali-u)); font-size: 1.3rem; }
  .feature p { margin: 0; color: var(--tali-muted); font-size: .95rem; }
```

**`the_serif_reading_scale_never_drops_below_the_body` reads `"\n  .feature h3 {"` by name and
asserts it equals `h3`'s size.** Keep both the selector and the value or that gate reddens, and
it is right to.

- [ ] **Step 4: Trim the button ladder to what the masthead uses**

`a.btn-primary:hover { filter: brightness(1.08); }` is a hover that changes a *ground* by
computation rather than by a chosen colour — the theme's rule is that every colour is an
explicit scored value. And `a.btn-lg` exists only for the CTA §15.2 deletes. Read what survives
before cutting:

```bash
grep -rn "\.btn" site/ docs/ corpus/ --include=*.tmd
```

Delete only the variants with no surviving author, and say in the commit which those were. A
`.btn` class is an **open authoring vocabulary** — there is no register for it and no
did-you-mean — so anything still written in a document must keep working.

- [ ] **Step 5: Rewrite `site/index.tmd`**

The masthead comes from the existing `hero:` front matter, restated as prose rather than as a
pitch. Replace the front matter's `hero:` block and the two `:::` sections:

```yaml
hero:
  eyebrow: "A dev server for technical writing"
  headline: "Taliesin"
  lead: "One .tmd file becomes a live post, a book, or a website — and only the block you changed re-renders."
  actions:
    - { text: "Read the docs", href: "docs/guide/", primary: true }
    - { text: "Explore the features", href: "features.tmd" }
```

The `## One source, many outputs` section's three `::: {.feature}` blocks become
`::: {.feature-list}` wrapping three `::: {.feature}` blocks — same authored content, one class
renamed, so the prose is untouched and only the container changes. **Keep the `{js}` scene and
the video figure**: both are live demonstrations, which is data, and §1 carves out room for
exactly that.

Delete the closing `::: {.hero}` block entirely (`site/index.tmd:127-134`) — it is the repeated
bottom CTA §15.2 names, and it restates the two links the masthead already carries. The section
above it (`## Built the way you would build yours`) already closes the page with the argument
the CTA was decorating.

- [ ] **Step 6: Run everything**

```bash
cargo build
cargo test -p taliesin-core the_landing_page_is_a_masthead_and_prose \
                            the_serif_reading_scale_never_drops_below_the_body
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/build-site.sh --check
```

Expected: PASS, and the composition gate green. `build-site.sh --check` is the gate that exists
because this project's own call-to-action once shipped 404ing (item 149) — a rewrite of the
landing page is precisely the change it was built for, so run it here and not only at the end.

- [ ] **Step 7: Render it**

```bash
cargo run -p taliesin-server -- preview site 4388
```

Via the chrome-devtools MCP: screenshot the landing page at 1280px and 700px. Check the
masthead's headline is the `2.25rem` ladder value and not a viewport-scaled one
(`getComputedStyle(document.querySelector('.hero h1, .hero h2')).fontSize` must be stable
across the two widths), that the feature sections are ruled and not boxed, and that
`document.documentElement.scrollWidth === clientWidth` at both. Confirm the `{js}` scene still
mounts and the console is clean.

- [ ] **Step 8: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the landing page becomes an editorial masthead and prose

Spec §15.2, an author decision taken 2026-08-14: no centred hero, no letterspaced
all-caps eyebrow, no three-card feature grid, no repeated bottom CTA. The masthead
form already existed in the tree for reading-measure pages; making it the only form is
what deletes the centred branch, its clamp(2rem, 6vw, 3.2rem) — the last
viewport-relative type size on the site — and the centred actions in one go.

The eyebrow keeps its key and loses its voice. It was 600 weight, .8rem, .12em
tracking and uppercase: a FOURTH voice, in a theme that owns two, applied to a string
the author wrote in their own front matter. Spec §4's rule attaches the machine voice
to labels the tool generates and never to a container that may hold author text, so
this is the serif.

The feature grid becomes ruled sections, which is the anatomy a callout and a listing
already have here. The authored prose inside them is untouched; one class is renamed.
The live {js} scene and the screencast stay: both are demonstrations, and §1 carves
out room for exactly that.
EOF
git commit -F /tmp/msg.txt
```

---

## Task 8: The banner, the glyph, and the chrome on the spacing scale

**Files:**
- Modify: `crates/server/src/log.rs`
- Modify: `crates/core/assets/css/site.css`
- Test: `crates/server/src/log.rs` (its own `mod tests`),
  `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `--tali-u`.
- Produces: nothing. This is the last task.

**Why.** Three leftovers that share one commit because each is a line or two:

1. **The CLI banner is the third instance of the typographic mark** (spec §7: "the favicon, the
   CLI banner glyph and the VS Code icon become one mark"). Plan 1 did the two SVGs; the banner
   still prints `taliesin` in `\x1b[1;32m` — bright green bold. Spec §3: colour in chrome,
   none. A terminal has no paper to score against, so the answer is not a different colour but
   **no colour**: the mark is the letterform, and the banner's own emphasis is weight.
2. **`keys_hint_body` names a glyph the UI does not draw.** It says "open the ◇ dev menu" while
   the toggle renders `</>` (`client.js:560`). Its test asserts the hint contains `◇` — pinning
   the prose against itself rather than against the thing it describes, which is the exact
   failure mode the *neighbouring* test (`keys_hint_names_the_corner_…`) was written to fix
   after the hint named the wrong corner for as long as it shipped. Same fix: derive it.
3. **The chrome's spacing.** `tokens.css` says the scale "lands with the components it
   measures: Plan 2 for the reading surface, **Plan 3 for the chrome**". Plan 2 landed its half.
   This is the other half, plus the two remaining no-op hovers from Plan 1's ledger
   (`.tali-nav-brand:hover` and `.tali-book-brand:hover` both set `--tali-link` on an element
   already at `--tali-fg`, and the two tokens are the same value in both palettes — the third,
   `.tali-card:hover .tali-card-title`, died with the card in Task 5).

- [ ] **Step 1: Write the failing tests**

In `crates/server/src/log.rs`'s `mod tests`, replace the `◇` assertion in the existing hint test
and add the banner test:

```rust
    /// The hint must name the glyph the dev-menu toggle actually DRAWS, not one chosen when
    /// the hint was written. It said `◇` while `client.js` rendered `</>` — the same failure
    /// as the corner (see the test below it, which was added after the hint named the wrong
    /// corner for as long as it shipped): prose pinned against itself rather than against the
    /// thing it describes. So derive it, and moving the glyph reddens this instead of rotting
    /// the sentence.
    #[test]
    fn keys_hint_names_the_glyph_the_toggle_actually_draws() {
        let client = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web-client/client.js"),
        )
        .expect("client.js");
        let glyph = client
            .split_once("class=\"tali-dev-glyph\">")
            .expect("the toggle draws a glyph")
            .1
            .split_once("</span>")
            .expect("the glyph span is closed")
            .0;
        // The toggle writes it HTML-escaped; the terminal hint writes it plain.
        let plain = glyph.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        let body = keys_hint_body();
        assert!(
            body.contains(&plain),
            "hint must name the {plain:?} glyph the toggle draws: {body:?}"
        );
    }

    /// The banner is the third instance of the one typographic mark (spec §7), and a mark in
    /// this theme carries no colour (spec §3: colour in chrome, none). It printed in
    /// `\x1b[1;32m` — bright green bold — which is a chrome accent in a place with no ground
    /// to score it against.
    #[test]
    fn the_banner_carries_no_colour() {
        let src = std::fs::read_to_string(file!()).or_else(|_| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/log.rs"),
            )
        })
        .expect("log.rs");
        let body = src
            .split_once("pub fn banner(")
            .expect("banner exists")
            .1
            .split_once("\n}")
            .expect("banner is closed")
            .0;
        for code in ["3\u{31}m", "32m", "33m", "34m", "35m", "36m", "37m"] {
            assert!(
                !body.contains(code),
                "the banner sets SGR colour {code}; the mark is a letterform and the theme \
                 puts no colour in chrome. Weight and dim are the available emphasis"
            );
        }
    }
```

In `crates/core/src/render/tests.rs`:

```rust
/// The chrome's half of the spacing scale. `tokens.css` says the scale "lands with the
/// components it measures: Plan 2 for the reading surface, Plan 3 for the chrome"; Plan 2
/// landed its half and gated `base.css`. This is the other half.
///
/// Same scope as its sibling: MARGINS between flow blocks are on `{0.5U, U, 1.5U, 2U, 3U}`,
/// while the internal padding of a small object stays on a quarter-unit sub-multiple, because
/// 0.5U is 15.5px and triples a table row (spec §3's amended row).
#[test]
fn the_chrome_margins_are_on_the_spacing_scale_too() {
    const SCALE: &[&str] = &[".25", ".5", "1.5", "2", "3"];
    for seg in SITE_CSS.split("calc(").skip(1) {
        let expr = seg.split(')').next().unwrap_or("");
        if !expr.contains("var(--tali-u)") {
            continue;
        }
        let factor = expr.split('*').next().unwrap_or("").trim();
        assert!(
            SCALE.contains(&factor) || factor.starts_with('-'),
            "`calc({expr})` uses {factor}U, which is not on the scale {SCALE:?}"
        );
    }
    // A hover that sets a colour the element already has is a rule that does nothing. Both of
    // these set `--tali-link` on an element already at `--tali-fg`, and the two tokens are
    // the same value in BOTH palettes — so they were silent no-ops in every theme.
    for sel in [".tali-nav-brand:hover", ".tali-book-brand:hover"] {
        assert!(
            !SITE_CSS.contains(sel),
            "`{sel}` sets --tali-link on an element already at --tali-fg, and the two tokens \
             are the same value in both palettes: it changes nothing"
        );
    }
}
```

- [ ] **Step 2: Run all three and watch them fail**

```bash
cargo test -p taliesin-server keys_hint_names_the_glyph_the_toggle_actually_draws \
                              the_banner_carries_no_colour
cargo test -p taliesin-core the_chrome_margins_are_on_the_spacing_scale_too
```

Expected: three failures — the hint says `◇` and the toggle draws `</>`; the banner sets `32m`;
`site.css` carries raw rem margins and both no-op hovers.

- [ ] **Step 3: The banner and the hint**

In `crates/server/src/log.rs`:

```rust
/// The opening banner: the mark, then the version.
///
/// The mark is the WORDMARK and carries no colour — spec §7 makes the favicon, this banner and
/// the VS Code icon one purely typographic mark, and spec §3 puts no colour in chrome. It
/// printed in bright green bold, which is a chrome accent in the one place with no ground to
/// score it against. Bold is the emphasis a terminal actually has; the version stays dim.
pub fn banner(version: &str) {
    eprintln!();
    eprintln!(
        "  {} {}",
        paint("taliesin", "\x1b[1m"),
        paint(version, "\x1b[2m")
    );
}
```

and the hint, which now names what the toggle draws:

```rust
fn keys_hint_body() -> &'static str {
    "controls live in the browser — open the </> dev menu (bottom-left)"
}
```

- [ ] **Step 4: The chrome on the scale**

In `site.css`, put every flow margin on the scale and drop the two no-op hovers. The
substitutions, all of which are the nearest scale member to the value they replace (`U` is
31 px):

| Was | Becomes |
|---|---|
| `.tali-foot-inner { padding: 1.4rem 1rem }` | `padding: calc(.5 * var(--tali-u)) 1rem` |
| `.tali-book-postnav { margin-top: 2.5rem; padding-top: 1.2rem }` | `margin-top: calc(2 * var(--tali-u)); padding-top: calc(.5 * var(--tali-u))` |
| `.tali-listing-backnav { margin-top: 2.5rem; padding-top: 1.2rem }` | the same two |
| `.tali-book-sidebar-head { margin: 0 0 1.1rem }` | `margin: 0 0 calc(.5 * var(--tali-u))` |
| `.tali-book-chapter { padding: .3rem 0 }` | `padding: calc(.25 * var(--tali-u)) 0` |

Then delete `.tali-nav-brand:hover { color: var(--tali-link); }` and
`.tali-book-brand:hover { color: var(--tali-link); }`, replacing the pair with one comment at
the first site:

```css
  /* No `:hover` on either brand: both set `--tali-link` on an element already at
     `--tali-fg`, and the two tokens hold the same value in both palettes, so the rules were
     silent no-ops in every theme. A wordmark that is already the ink has nowhere to lift to;
     it is a link and the cursor says so. */
```

Leave `.tali-book-prev:hover`, `.tali-back-link:hover` and `a.tali-foot-item:hover` alone —
each lifts `--tali-muted` to `--tali-fg`, which is a real change.

- [ ] **Step 5: Run everything, then every gate**

```bash
cargo build
cargo test -p taliesin-server keys_hint_names_the_glyph_the_toggle_actually_draws \
                              the_banner_carries_no_colour
cargo test -p taliesin-core the_chrome_margins_are_on_the_spacing_scale_too
cd web-client && npx -y -p typescript tsc -p jsconfig.json; cd -
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json; cd -
cargo fmt --all
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo test --workspace
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Expected: the workspace suite green, and `gates.sh` reporting `PASSED — every gate ran and
passed (N gates)` with **N taken from the script's own verdict line**. If a document gate fails
it is `site/index.tmd` or a docs page this plan edited; read the message and fix the document,
not the gate.

- [ ] **Step 6: The whole-surface render pass**

Every prior task rendered its own component. This is the first look at all of them together,
and it is what spec §13 will run against. Serve each of the three projects and screenshot in
both palettes at 1280px and 700px:

```bash
cargo run -p taliesin-server -- preview site 4388           # masthead, listing, footer
cargo run -p taliesin-server -- preview docs/guide 4388     # navbar + TOC rail
cargo run -p taliesin-server -- preview docs/internals 4388 # book topbar + drawer
```

For each, via the chrome-devtools MCP:

```js
[document.documentElement.scrollWidth, document.documentElement.clientWidth]  // must be equal
[...document.querySelectorAll('*')].filter(e => getComputedStyle(e).boxShadow !== 'none').length  // 0
new Set([...document.querySelectorAll('*')].map(e => getComputedStyle(e).fontFamily)).size        // 2
```

Record what you saw in the commit. **Do not report this task complete on the static gates
alone** — the handover's one lesson is that six Plan 2 defects were invisible to every static
gate and only a render found them.

- [ ] **Step 7: Commit**

```bash
git add -A
cat > /tmp/msg.txt <<'EOF'
feat(theme): the banner is the mark, the hint names the real glyph, the chrome
lands on the scale

The CLI banner is the third instance of spec §7's one typographic mark, and it printed
in bright green bold — a chrome accent in the one place with no ground to score it
against. A terminal's emphasis is weight.

keys_hint_body said "open the ◇ dev menu" while the toggle draws </>, and its test
asserted the hint contains ◇: prose pinned against itself rather than against the
thing it describes. That is the same failure the neighbouring test was written to fix
after the hint named the wrong corner for as long as it shipped, so it gets the same
answer — derive the glyph from client.js, and moving it reddens the test instead of
rotting the sentence.

tokens.css said the spacing scale "lands with the components it measures: Plan 2 for
the reading surface, Plan 3 for the chrome". This is the chrome's half. Two silent
no-op hovers go with it: both set --tali-link on an element already at --tali-fg, and
the two tokens are the same value in both palettes.
EOF
git commit -F /tmp/msg.txt
```

---

## Self-review

**Spec coverage.**

| Spec | Where |
|---|---|
| §1 no colour in chrome | Task 4 (the bar), Task 7 (the eyebrow, the masthead rule), Task 8 (the banner) |
| §3 one radius, no shadows, no blur, one duration | Task 3 (the dev UI), Task 6 (the drawer's second clock) |
| §3 spacing scale, chrome half | Task 8 |
| §4 the machine voice never on a container that may hold author text | Task 6 (`parts:`), Task 7 (`hero.eyebrow`) |
| §5 the diagnostic surfaces keep their own named colours | Task 2 (the status tokens ARE those colours) |
| §7 the CLI banner glyph, third instance of the mark | Task 8 |
| §8 four status hexes → named, scored tokens | Task 2 (as R1: three) |
| §8 the five dev-menu deletions | Task 1 |
| §8 the emoji | Task 1 (three) + Task 3 (five, as R7) |
| §8 `serve/mod.rs:472` reads `--tali-mono`, a token that never existed | **already fixed by Plan 1**; verified 2026-08-15 that no sibling survives — the nine `--tali-*` names read by `serve/mod.rs` + `client.js` are all defined, and Task 2 removes the `var(--x, fallback)` form that made the gate blind to this class of defect in the first place |
| §9 cut #12 chips, monogram, reading time, chapter word counts | Task 5 (as R6, whole) |
| §12.1 the vendor-hex ban's widened scope | Task 2 (deletes the exemption; adds Primer's success + danger to `BANNED`) |
| §12.2 the tell probe | Task 3 (widened to the dev UI's two files, which is where `client.js:474` dies) |
| §15.2 the landing page | Task 7 |

**The two gates built to fail, and how each dies.** `DEV_UI_STATUS_EXEMPT`/`DEV_UI_SURFACE` are
**deleted** in Task 2 Step 5, not emptied and not extended — and `BANNED` gains two hexes in the
same edit, so the ban is strictly wider afterwards. `client.js:474`'s borrowed stack dies in
Task 3 by the tell probe learning to scan the file, which also takes `:471` (the same stack, one
line the handover did not name) and the `backdrop-filter`, the `box-shadow` and the `10px`
radius in the same block. Neither is weakened; both trade an exemption for a wider rule.

**Explicitly out of scope, and where each goes.** Spec §9's cut #6 (the search-hit whole-page
flash and its `<mark>` fallback, ~137 lines in `search.js`) is **Plan 4**, with the Cmd-K work
cut #2 already assigns there — this plan does not otherwise touch `search.js`, and §9's rule is
that a cut lands in the commit that restyles its neighbours. Structured `author:` and
`.tali-appendix` (17.1 px, still under the prose), the remaining §10 knobs, KaTeX face
subsetting, fonts-as-files, the orphan-page diagnostic and the `theming.tmd` rewrite are Plan 4.
The mono subset's missing `U+2318` and `U+2192` need a re-vendor that invalidates **both** font
hash pins: Plan 4. Task 3 leaves one `\2192` in a `content` string with a comment saying so
rather than pretending the glyph exists.

**Carried into spec §13, which runs after this plan.** Two items are known-unverified from
Plan 2 and must be picked up there rather than assumed: **print was never render-verified**
(Plan 2 added `#tali-main { display: block !important }` to the print block, reasoned but not
seen), and **the ≤30rem mobile block was never rendered** (the Chrome window has a ~500px floor,
so it needs device emulation). This plan adds a third: the dev UI is preview-only, so §13's
protocol — which runs against built pages — will not see any of Tasks 1-3 unless it is run
against a live `preview` as well.

**Placeholder scan.** No `TBD`, no "similar to Task N", no step that says what without showing
how. Four steps deliberately say "read the surrounding code before editing", each because the
edit depends on a shape this plan did not want to transcribe and risk being wrong about: where
`setDiagnostics`'s deleted lines end (Task 1 Step 3), the `let img = …` match arms in
`card_html` (Task 5 Step 4), whether `.tali-book-part-nested` is a class or an attribute
(Task 6 Step 3), and which `.btn` variants still have an author (Task 7 Step 4). Three steps are
**discovery** steps with the exact `grep` written out — Task 5 Step 5's pin hunt, Task 5's
`categories:` decision, Task 6's part-heading emission — because a plan that guesses which
corpus documents pin a cut feature is how the ordering rule gets broken.

**Type consistency.** `--tali-status-live` / `-warn` / `-error` are introduced in Task 2 Step 3
and read with those exact names in Task 2 Step 4 and Task 3 Step 4. The shared selector lists
`:is(.tali-site-nav, .tali-book-topbar)` and `:is(.tali-nav-inner, .tali-book-topbar-inner)` are
written in Task 4 Step 3 and asserted with the same spelling in Task 4 Step 1. `.feature-list`
is introduced in Task 7 Step 3's CSS and authored in Task 7 Step 5's `.tmd`. `.feature h3` keeps
both its selector and its value in Task 7 because
`the_serif_reading_scale_never_drops_below_the_body` reads `"\n  .feature h3 {"` by name — that
dependency is stated in the step, not left to be discovered by a red test.

**One risk worth naming.** Task 2's R2 rests on a claim about CSS custom-property substitution:
that `--tali-status-live: var(--tali-callout-tip)` declared on `:root` picks up
`tokens-dark.css`'s override because both target the same element. It is the correct reading of
the cascade, and it is one element away from Plan 2's `--tali-prose-cols` trap, where the same
idiom baked in a zero because the property being read was engaged on a *descendant*. **Task 2
Step 7 verifies it in a dark render rather than resting on the paragraph**, and states the
fallback (six literals across two sheets) in the step itself. If any single thing in this plan
is going to be wrong in a way no gate can see, it is this.
