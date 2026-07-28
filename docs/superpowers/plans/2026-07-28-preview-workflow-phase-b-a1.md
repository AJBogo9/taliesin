# Preview workflow: Phase B + Phase A1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give collapsed callouts a visible disclosure affordance, and make the in-editor preview's two navigation directions a deliberate, symmetric, discoverable pair.

**Architecture:** Phase B is a pure CSS change plus a `BASE_CSS` assertion. Phase A1 changes the inverse-search modifier from Alt to Ctrl/Cmd in `web-client/client.js`, adds a `reveal` intent flag to the existing `tali-cursor` postMessage protocol so the preview stops chasing the cursor, and introduces a preview registry in the VS Code companion so panels and servers are reused and a new forward-search command can find them.

**Tech Stack:** Rust (taliesin-core, `include_str!`-bundled CSS), vanilla JS (`web-client/client.js`, `// @ts-check`, no build step), TypeScript (VS Code companion, esbuild), Puppeteer (`tools/ui-audit`), `@vscode/test-electron` (companion e2e).

**Source spec:** [2026-07-28-preview-workflow-design.md](../specs/2026-07-28-preview-workflow-design.md)

**Phase A2 (site-aware preview) is deliberately NOT in this plan.** It depends on the protocol and registry introduced here, and it is a source-resolution redesign rather than a wiring change. It gets its own plan once A1 has landed.

## Global Constraints

- **The preview never writes to source.** Every gesture in this plan navigates. If a step seems to want a write path back to the `.tmd`, stop and re-read the spec.
- **Do NOT touch warm-page eviction:** `MAX_WARM_PAGES` and the deterministic LRU order in `crates/server/src/serve_site/exec_pool.rs`. It is the project's one standing freeze and is not test-guarded, so a reorder breaks the build silently.
- **No new user-facing settings.** The defaults were chosen so no knob is needed.
- **Editing `assets/css/*` or `assets/js/*` requires `cargo build` before the change appears in a built page.** They are `include_str!`-compiled into the binary. A live `preview` hot-swaps CSS, so this bites the build-and-inspect loop, not the dev loop.
- **A `PostToolUse` hook runs `rustfmt` on every edited `.rs` file.** Do not hand-format Rust.
- **Never claim a gate passed without quoting its output.** Several gates in this repo skip silently when an interpreter is absent.
- Rust edition 2024. Shared deps live in the root `[workspace.dependencies]`.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `crates/core/assets/css/base.css` | the trailing caret rule for collapsible callouts; the renamed armed-state cursor rule | 1, 2 |
| `crates/core/assets/css/deck.css` | same caret on slides; the renamed armed-state rule | 1, 2 |
| `crates/core/src/render/tests.rs` | `BASE_CSS` assertions (never page-level `contains`) | 1 |
| `web-client/client.js` | the inverse-search gesture, the armed overlay, the `reveal`-gated highlight, the dev-panel hint | 2, 3 |
| `tools/ui-audit/lib/probe.mjs` | the browser probe that exercises click-to-source | 2 |
| `editor/vscode/src/previews.ts` | **new.** The live-preview registry and `previewFor` lookup. Extracted so both the reuse logic and the command's target resolution are unit-testable without a webview. | 4 |
| `editor/vscode/src/extension.ts` | wires the registry into `openPreview`; sends `reveal: false` on cursor moves | 3, 4, 5 |
| `editor/vscode/src/commands.ts` | the `taliesin.revealInPreview` command | 5 |
| `editor/vscode/package.json` | command, keybinding, palette entry | 5 |
| `editor/vscode/src/test/previews.test.ts` | **new.** Unit tests for the registry. | 4 |
| `crates/core/tests/retired_names.rs` | guard so `tali-alt` cannot return | 6 |

---

## Task 1: Collapsed-callout disclosure chevron

Independent of every other task. Ships first.

**Files:**
- Modify: `crates/core/assets/css/base.css:502-518`
- Modify: `crates/core/assets/css/deck.css:286`
- Test: `crates/core/src/render/tests.rs` (new test function)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing other tasks depend on.

**Background you need.** `divs.rs:573` already emits collapsible callouts as
`<div class="callout callout-{kind} callout-collapse"><details><summary class="callout-title">…`.
A `<summary>` only renders the browser's disclosure marker at `display: list-item`, and
`.callout-title` sets `display: flex` (`base.css:450`), so today there is **no indicator in either
state**. Verified in Chrome: computed `::before` content is `none` and `list-style-type` is
`disclosure-closed`.

**The caret must be `::after`, not `::before`.** In a flex container `::before` is the first item,
and `margin-left: auto` on the first item pushes every following item right too, which would
right-align the kind icon and the title as well. `::after` is the last item, so `margin-left: auto`
consumes the free space to its left and pins the caret right.

- [ ] **Step 1: Write the failing test**

Add to `crates/core/src/render/tests.rs`. Note this asserts on `BASE_CSS` **directly**. Do not
assert on a rendered page: every Taliesin page inlines the whole stylesheet, so a page-level
`contains(".callout-collapse")` passes on a page with no callouts at all.

```rust
/// A `collapse="true"` callout must show a disclosure caret in BOTH states.
///
/// `.callout-title` is `display: flex`, and a `<summary>` renders the browser's own
/// disclosure marker only at `display: list-item` — so before this rule a collapsed
/// callout was a title bar with no indicator whatsoever, and an OPEN collapsible one
/// was indistinguishable from a plain non-collapsible callout.
///
/// Asserted against BASE_CSS, never a rendered page: every page inlines the whole
/// stylesheet, so a page-level `contains` would pass on a page with no callouts.
#[test]
fn collapsible_callouts_carry_a_disclosure_caret() {
    let sel = ".callout-collapse > details > summary.callout-title";
    assert!(
        BASE_CSS.contains(&format!("{sel}::after")),
        "collapsible callouts need a trailing ::after caret"
    );
    assert!(
        BASE_CSS.contains(&format!("{sel}::-webkit-details-marker")),
        "Safari's native marker must be suppressed too"
    );
    // The caret trails, which is what distinguishes it from the leading `::before`
    // carets on folded code and proofs. Without `margin-left: auto` it would sit
    // flush against the title text instead of at the right edge of the tinted bar.
    let i = BASE_CSS
        .find(&format!("{sel}::after"))
        .expect("the callout caret rule");
    let rule = &BASE_CSS[i..i + BASE_CSS[i..].find('}').expect("closing brace")];
    assert!(rule.contains("margin-left: auto"), "the caret must trail: {rule}");
    assert!(rule.contains("rotate(45deg)"), "closed state points right: {rule}");
    // Open rotates to point down, exactly like the proof caret.
    assert!(
        BASE_CSS.contains(&format!(
            ".callout-collapse > details[open] > summary.callout-title::after {{ transform: rotate(135deg); }}"
        )),
        "open state must rotate the caret down"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p taliesin-core collapsible_callouts_carry_a_disclosure_caret
```

Expected: FAIL on the first assertion, "collapsible callouts need a trailing ::after caret".

- [ ] **Step 3: Add the CSS**

In `crates/core/assets/css/base.css`, first replace the comment at lines 502-506. It currently
asserts the opposite decision and must not be left in place.

Replace:

```css
  /* Themed disclosure chevron for folded code (`tali-code-fold`) + collapsible proofs:
     the browser-default triangle is small, inconsistent, and easy to read as plain text,
     so hide it and draw a `currentColor` caret that rotates from a right-pointing to a
     down-pointing chevron when the details opens. (Callouts keep their icon header, which
     is already an affordance.) */
```

with:

```css
  /* Themed disclosure chevron for folded code (`tali-code-fold`), collapsible proofs, and
     collapsible callouts: the browser-default triangle is small, inconsistent, and easy to
     read as plain text, so hide it and draw a `currentColor` caret that rotates from a
     right-pointing to a down-pointing chevron when the details opens.
     Callouts were once excluded here on the theory that the kind icon was already an
     affordance. It is not: the icon says *what kind*, never *that this opens*, and because
     `.callout-title` is `display: flex` the native marker is not rendered either (a
     `<summary>` only draws one at `display: list-item`) — so a collapsed callout had no
     indicator at all, and an open collapsible one was indistinguishable from a plain one.
     The callout caret TRAILS while these two lead, because the callout's left slot is
     already the kind icon; trailing also mirrors `.tali-book-expand` in site.css. That is
     why it is `::after` and not `::before`: `::before` is the first flex item, so pushing it
     right with `margin-left: auto` would drag the icon and title right along with it. */
```

Then append these four rules immediately after the existing line 518
(`.tali-proof-collapse > details[open] > summary.tali-proof-head::before { transform: rotate(135deg); }`).

**Do NOT add the callout selector to the existing rule at 507-509.** That rule also sets
`gap: .45em`, which would silently override `.callout-title`'s own `gap: .45rem` (`base.css:450`)
and shift icon-to-text spacing on every collapsible callout.

```css
  .callout-collapse > details > summary.callout-title { list-style: none; }
  .callout-collapse > details > summary.callout-title::-webkit-details-marker { display: none; }
  .callout-collapse > details > summary.callout-title::after {
    content: ""; flex: 0 0 auto; margin-left: auto; width: .42em; height: .42em;
    margin-bottom: .12em; border-top: 2px solid currentColor; border-right: 2px solid currentColor;
    transform: rotate(45deg); transition: transform var(--tali-dur) ease; opacity: .75; }
  .callout-collapse > details[open] > summary.callout-title::after { transform: rotate(135deg); }
```

In `crates/core/assets/css/deck.css`, replace line 286:

```css
.tali-deck .tali-slides .callout-collapse > details > summary.callout-title { cursor: pointer; list-style: none; }
```

with:

```css
.tali-deck .tali-slides .callout-collapse > details > summary.callout-title { cursor: pointer; list-style: none; }
/* The same trailing caret base.css gives a collapsible callout: a slide's collapsed callout
   is the same silent case by a different route (`list-style: none` here rather than the flex
   display suppressing the marker). */
.tali-deck .tali-slides .callout-collapse > details > summary.callout-title::after {
  content: ""; flex: 0 0 auto; margin-left: auto; width: .42em; height: .42em;
  margin-bottom: .12em; border-top: 2px solid currentColor; border-right: 2px solid currentColor;
  transform: rotate(45deg); transition: transform var(--tali-dur) ease; opacity: .75; }
.tali-deck .tali-slides .callout-collapse > details[open] > summary.callout-title::after { transform: rotate(135deg); }
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p taliesin-core collapsible_callouts_carry_a_disclosure_caret
```

Expected: PASS.

- [ ] **Step 5: Verify in a real browser**

The CSS assertion proves the rule exists, not that it looks right. Build the binary first, or you
will inspect a stale bundled stylesheet.

```bash
cargo build -p taliesin-server --release
cat > /tmp/collapse-check.tmd <<'EOF'
---
title: "Collapsed disclosure affordance check"
---
::: {.callout-note collapse="true"}
## A collapsed note
Hidden body text.
:::

::: {.callout-tip collapse="false"}
## An open, collapsible tip
Visible body text.
:::

::: {.callout-important}
## A plain important
Always-visible body.
:::
EOF
./target/release/taliesin build /tmp/collapse-check.tmd /tmp/collapse-check.html
```

Open `file:///tmp/collapse-check.html` and confirm by screenshot:
- the collapsed note has a right-pointing caret at the **right edge** of its tinted title bar,
- the open tip has a **down**-pointing caret, so it is now distinguishable from the plain callout,
- the plain important has **no** caret,
- the kind icon and title text are still **left**-aligned in all three.

- [ ] **Step 6: Run the full core suite**

```bash
cargo test -p taliesin-core
```

Expected: PASS. Quote the summary line.

- [ ] **Step 7: Commit**

```bash
git add crates/core/assets/css/base.css crates/core/assets/css/deck.css crates/core/src/render/tests.rs
git commit -m "callouts: a collapsed box now says it opens"
```

---

## Task 2: Inverse search becomes Ctrl/Cmd+click

**Files:**
- Modify: `web-client/client.js:425-432` (dev-panel hint), `:1513-1528` (the click handler), `:1530-1577` (the armed overlay)
- Modify: `crates/core/assets/css/base.css:729-735`
- Modify: `crates/core/assets/css/deck.css:1077-1078`
- Modify: `tools/ui-audit/lib/probe.mjs:172-181`

**Interfaces:**
- Consumes: nothing.
- Produces: the armed-state class is now `tali-srcnav` (was `tali-alt`). Task 6 guards the old name.

**Why a clean break rather than an alias.** Decision D1. One gesture to document, test and teach.
The cost is the doc sweep in Task 6.

**Checked before you start, so you do not need to re-derive it:** no other handler in the bundled JS
conflicts with Ctrl/Cmd+click. `code-enhance/11-lightbox.js:158`, `code-enhance/07-keyboard.js:62,68`
and `deck.js:1696` already exclude `ctrlKey` and `metaKey` alongside `altKey`. `deck.js:1084,1094`
uses `ctrlKey` on **wheel** events for trackpad pinch detection, and `search.js:1057` is a Cmd/Ctrl+K
**keydown**, so neither is a click and neither is affected.

- [ ] **Step 1: Change the click handler**

In `web-client/client.js`, replace lines 1513-1522 (the comment through the `preventDefault`):

```js
  // Click-to-source: Alt/Option-click any block to jump to its source line (browser
  // -> vscode://, webview -> host). A plain click browses normally, so there's no
  // mode and no way to land in the editor by accident.
  document.addEventListener("click", (e) => {
    if (!e.altKey) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t || inDevMenu(t)) return;
    const el = locatable(t);
    if (!el) return;
    e.preventDefault(); // suppress text selection / link navigation on the Alt-click
```

with:

```js
  // Inverse search: Ctrl-click (Cmd-click on Mac) any block to jump to its source line
  // (browser -> vscode://, webview -> host). A plain click browses normally, so there's
  // no mode and no way to land in the editor by accident.
  //
  // Ctrl/Cmd rather than Alt because it is the convention every comparable tool already
  // taught the author: LaTeX Workshop's inverse search is Ctrl-click, and Alt-click
  // additionally collides with VS Code's own insert-cursor and, under GNOME, with
  // window dragging. Both modifiers are accepted on every platform so neither habit
  // fails; the docs name the platform-native one.
  document.addEventListener("click", (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t || inDevMenu(t)) return;
    const el = locatable(t);
    if (!el) return;
    e.preventDefault(); // suppress text selection / link navigation on the jump
```

- [ ] **Step 2: Rewire the armed overlay**

Still in `client.js`, in the IIFE at 1530-1577, make these substitutions. The logic is unchanged;
only the trigger key, the class name and the local names move.

- The comment block at 1530-1536: replace `while Alt is held` with `while Ctrl/Cmd is held`, and
  `html.tali-alt` with `html.tali-srcnav`.
- `let altOn = false;` becomes `let navOn = false;`
- `const enterAlt = () => { if (altOn) return; altOn = true; document.documentElement.classList.add("tali-alt");` becomes
  `const enterNav = () => { if (navOn) return; navOn = true; document.documentElement.classList.add("tali-srcnav");`
- `const exitAlt = () => { if (!altOn) return; altOn = false; document.documentElement.classList.remove("tali-alt");` becomes
  `const exitNav = () => { if (!navOn) return; navOn = false; document.documentElement.classList.remove("tali-srcnav");`
- `if (altOn) markEl(...)` in the `mousemove` handler becomes `if (navOn) markEl(...)`

Then replace the two key listeners:

```js
    window.addEventListener("keydown", (e) => { if (e.key === "Alt") enterAlt(); });
    window.addEventListener("keyup", (e) => { if (e.key === "Alt") exitAlt(); });
```

with:

```js
    const isNavKey = (/** @type {KeyboardEvent} */ e) => e.key === "Control" || e.key === "Meta";
    window.addEventListener("keydown", (e) => { if (isNavKey(e)) enterNav(); });
    window.addEventListener("keyup", (e) => { if (isNavKey(e)) exitNav(); });
    // macOS: Ctrl-click IS the secondary click. A Mac author who reaches for Ctrl instead of
    // Cmd would otherwise get a context menu on top of the jump. Suppressed only while the
    // overlay is armed, so an ordinary right-click is untouched.
    document.addEventListener("contextmenu", (e) => { if (navOn) e.preventDefault(); });
```

Finally update the two remaining `exitAlt` references in the blur/visibility handlers to `exitNav`,
and the comment at 1572-1574 that says "an Alt-click that navigates to vscode://" to say
"a jump that navigates to vscode://".

- [ ] **Step 3: Update the dev-panel hint (the signifier)**

Replace `client.js:425-432`:

```js
    // Click-to-source hint: Alt/Option-click any block to open its source. No toggle
    // — a plain click browses normally; the modifier is the whole gesture.
    const srcHint = document.createElement("span");
    srcHint.id = "tali-src-hint";
    srcHint.textContent = "Alt-click a block";
    srcHint.title =
      "Hold Alt (Option on Mac) and click any block to open its source" +
      (inWebview ? " in the editor" : " in your editor");
```

with:

```js
    // Inverse-search hint: Ctrl/Cmd-click any block to open its source. No toggle —
    // a plain click browses normally; the modifier is the whole gesture. A modifier
    // gesture with no signifier is undiscoverable, and this label is that signifier,
    // so it names the platform-native key rather than both.
    const mac = /Mac|iP(hone|ad|od)/.test(navigator.platform || "");
    const navKey = mac ? "Cmd" : "Ctrl";
    const srcHint = document.createElement("span");
    srcHint.id = "tali-src-hint";
    srcHint.textContent = navKey + "-click a block";
    srcHint.title =
      "Hold " + navKey + " and click any block to open its source" +
      (inWebview ? " in the editor" : " in your editor");
```

- [ ] **Step 4: Rename the class in both stylesheets**

`crates/core/assets/css/base.css`, replace lines 729-735:

```css
  /* click-to-source affordance: while Alt is held (html.tali-alt) every source-mapped
     block reads as clickable (pointer), and the block a click would resolve to gets a
     dashed accent outline that tracks the mouse (.tali-src-hover, toggled in client.js).
     Dashed distinguishes this "click to open source" hover from the solid .tali-hl that
     marks the editor-cursor sync. Outline (not border) so tracking never reflows the
     page; no animation, because continuous hover feedback must read as instantaneous. */
  html.tali-alt [data-block-id], html.tali-alt [data-tali-src] { cursor: pointer; }
```

with:

```css
  /* Inverse-search affordance: while Ctrl/Cmd is held (html.tali-srcnav) every source-mapped
     block reads as clickable (pointer), and the block a click would resolve to gets a
     dashed accent outline that tracks the mouse (.tali-src-hover, toggled in client.js).
     Dashed distinguishes this "click to open source" hover from the solid .tali-hl that
     marks the editor-cursor sync. Outline (not border) so tracking never reflows the
     page; no animation, because continuous hover feedback must read as instantaneous. */
  html.tali-srcnav [data-block-id], html.tali-srcnav [data-tali-src] { cursor: pointer; }
```

`crates/core/assets/css/deck.css`, replace lines 1077-1078:

```css
/* Alt-held click-to-source affordance (deck accent; see base.css for the rationale). */
html.tali-alt [data-block-id] { cursor: pointer; }
```

with:

```css
/* Ctrl/Cmd-held inverse-search affordance (deck accent; see base.css for the rationale). */
html.tali-srcnav [data-block-id] { cursor: pointer; }
```

- [ ] **Step 5: Update the browser probe**

In `tools/ui-audit/lib/probe.mjs`, replace lines 172-181:

```js
    // Alt-hover affordance
    await page.keyboard.down('Alt');
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(10, box.height / 2));
    const altHover = await page.evaluate(
      () => document.documentElement.classList.contains('tali-alt'),
    );
    // Alt-click
    const before = cdpFrames.length;
    await handle.click();
    await page.keyboard.up('Alt');
```

with:

```js
    // Ctrl-hover affordance
    await page.keyboard.down('Control');
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(10, box.height / 2));
    const altHover = await page.evaluate(
      () => document.documentElement.classList.contains('tali-srcnav'),
    );
    // Ctrl-click
    const before = cdpFrames.length;
    await handle.click();
    await page.keyboard.up('Control');
```

Then rename the local `altHover` to `navHover` throughout this function, including wherever it is
reported. Run `rg -n 'altHover' tools/ui-audit/lib/probe.mjs` and fix every hit.

- [ ] **Step 6: Type-check the client**

```bash
cd web-client && npx -y -p typescript tsc -p jsconfig.json
```

Expected: no output (clean). If `navigator.platform` is flagged, it is a real deprecation but is
still universally supported; keep it and do not reach for `navigator.userAgentData`, which the
`@ts-check` lib does not declare.

- [ ] **Step 7: Verify the gesture in a real browser**

```bash
cargo build -p taliesin-server --release
cd tools/ui-audit && node probe-run.mjs
```

Expected: the click-to-source probe reports the armed class present and a `click_block` websocket
frame captured. Quote the click-to-source line of its output.

- [ ] **Step 8: Commit**

```bash
git add web-client/client.js crates/core/assets/css/base.css crates/core/assets/css/deck.css tools/ui-audit/lib/probe.mjs
git commit -m "inverse search: the modifier everyone else already taught you"
```

---

## Task 3: Stop the preview chasing the cursor

**Files:**
- Modify: `web-client/client.js:1579-1621`
- Modify: `editor/vscode/src/extension.ts:94-106`

**Interfaces:**
- Consumes: nothing.
- Produces: the `tali-cursor` message gains a `reveal` boolean.
  `{type: "tali-cursor", file: string|null, line: number, reveal: boolean}`.
  `highlightAtLine(file, line, reveal)` marks always and scrolls only when `reveal` is true.
  Task 5 sends `reveal: true`.

**Why.** Decision D2. Today every selection change scrolls the preview, with no gesture and no off
switch, which is strictly more aggressive than any comparable tool. Marking is free and
non-disruptive; scrolling is not, so it becomes something you ask for.

**After this task and before Task 5, nothing scrolls the preview.** That is the intended end state
for cursor movement, but the "take me there" half does not exist until Task 5, so do not stop
between them.

- [ ] **Step 1: Gate the scroll in the client**

In `web-client/client.js`, change the signature at 1582 and the tail of the function. Replace:

```js
  const highlightAtLine = (/** @type {string|null} */ file, /** @type {number} */ line) => {
```

with:

```js
  // `reveal` separates "where am I" from "take me there". Marking is continuous and free,
  // so it always happens; scrolling steals the author's scroll position, so it only happens
  // when they ask (the forward-search command). Without this split, scrolling the preview to
  // compare two figures and then typing one character yanked the page back.
  const highlightAtLine = (
    /** @type {string|null} */ file,
    /** @type {number} */ line,
    /** @type {boolean} */ reveal
  ) => {
```

Then replace the block at 1605-1615:

```js
    if (isDeck && window.TaliesinDeck) {
      const sections = [...root.querySelectorAll(".tali-slides > section")];
      const sec = target.closest(".tali-slides > section");
      const i = sec ? sections.indexOf(sec) : -1;
      if (i >= 0) window.TaliesinDeck.slide(i);
    } else {
      const r = target.getBoundingClientRect();
      if (r.top < 0 || r.bottom > window.innerHeight) {
        target.scrollIntoView({ block: "center", behavior: scrollBehavior() });
      }
    }
```

with:

```js
    if (!reveal) return;
    // Changing slide is the deck's equivalent of scrolling and is just as disruptive,
    // so it is gated identically.
    if (isDeck && window.TaliesinDeck) {
      const sections = [...root.querySelectorAll(".tali-slides > section")];
      const sec = target.closest(".tali-slides > section");
      const i = sec ? sections.indexOf(sec) : -1;
      if (i >= 0) window.TaliesinDeck.slide(i);
    } else {
      const r = target.getBoundingClientRect();
      if (r.top < 0 || r.bottom > window.innerHeight) {
        target.scrollIntoView({ block: "center", behavior: scrollBehavior() });
      }
    }
```

And the listener at 1618-1621:

```js
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (m && m.type === "tali-cursor") highlightAtLine(m.file, m.line, !!m.reveal);
  });
```

- [ ] **Step 2: Send the flag from the companion**

In `editor/vscode/src/extension.ts`, replace lines 94-106:

```ts
  // reverse: editor cursor -> preview (debounced tali-cursor)
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!isSourceFile(f)) return;
    const key = relativeKey(docPath, f);
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(
      () => panel.webview.postMessage({ type: "tali-cursor", file: key, line }),
      80
    );
  });
```

with:

```ts
  // Forward search, passive half: the editor cursor MARKS its block in the preview
  // (debounced), and never moves the page. `reveal: false` is the whole difference —
  // the active half is `taliesin.revealInPreview`, which sends `reveal: true`.
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!isSourceFile(f)) return;
    const key = relativeKey(docPath, f);
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(
      () => panel.webview.postMessage({ type: "tali-cursor", file: key, line, reveal: false }),
      80
    );
  });
```

- [ ] **Step 3: Type-check both sides**

```bash
cd web-client && npx -y -p typescript tsc -p jsconfig.json
cd editor/vscode && npm run compile-tests
```

Expected: both clean.

- [ ] **Step 4: Commit**

```bash
git add web-client/client.js editor/vscode/src/extension.ts
git commit -m "forward search: mark where you are, move only when asked"
```

---

## Task 4: Preview registry (panel and server reuse)

**Files:**
- Create: `editor/vscode/src/previews.ts`
- Create: `editor/vscode/src/test/previews.test.ts`
- Modify: `editor/vscode/src/extension.ts:36-117`

**Interfaces:**
- Consumes: `PreviewServer` from `./server`.
- Produces:
  - `interface LivePreview { panel: vscode.WebviewPanel; server: PreviewServer; docPath: string }`
  - `class PreviewRegistry` with `get(docPath: string): LivePreview | undefined`,
    `set(p: LivePreview): void`, `delete(docPath: string): void`,
    `beginStart(docPath: string): boolean`, `endStart(docPath: string): void`,
    `previewFor(file: string): LivePreview | undefined`, `readonly size: number`.
  - Task 5 calls `previewFor`.

**The bug.** `openPreview` unconditionally allocates a port, spawns `taliesin preview` and creates a
webview. Pressing `Ctrl+Shift+K` twice on one file yields two panels, two servers and two file
watchers.

**Why a separate module.** The lookup rules are the interesting part and they must be unit-testable
without constructing a webview, which a test cannot do outside a real extension host.

- [ ] **Step 1: Write the failing test**

Create `editor/vscode/src/test/previews.test.ts`:

```ts
import * as assert from "node:assert";
import { test } from "node:test";
import { PreviewRegistry, LivePreview } from "../previews";

/** A LivePreview with only the fields the registry actually reads. */
function fake(docPath: string): LivePreview {
  return { docPath, panel: {} as never, server: {} as never };
}

test("a second start for the same document reuses the first", () => {
  const r = new PreviewRegistry();
  const p = fake("/w/a.tmd");
  r.set(p);
  assert.strictEqual(r.get("/w/a.tmd"), p);
  assert.strictEqual(r.size, 1);
});

test("different documents get their own previews", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.size, 2);
  assert.strictEqual(r.get("/w/a.tmd")?.docPath, "/w/a.tmd");
});

test("beginStart is a one-shot latch, so a double keypress spawns one server", () => {
  const r = new PreviewRegistry();
  assert.strictEqual(r.beginStart("/w/a.tmd"), true, "first caller proceeds");
  assert.strictEqual(r.beginStart("/w/a.tmd"), false, "second caller must bail");
  r.endStart("/w/a.tmd");
  assert.strictEqual(r.beginStart("/w/a.tmd"), true, "released after the start settles");
});

test("delete is idempotent, because both disposal paths may fire", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.delete("/w/a.tmd");
  r.delete("/w/a.tmd");
  assert.strictEqual(r.size, 0);
});

test("previewFor prefers the buffer's own preview", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.previewFor("/w/b.tmd")?.docPath, "/w/b.tmd");
});

test("previewFor falls back to the only preview, so an included file resolves", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/book.tmd"));
  assert.strictEqual(r.previewFor("/w/chapter.tmd")?.docPath, "/w/book.tmd");
});

test("previewFor refuses to guess when several previews are open", () => {
  const r = new PreviewRegistry();
  r.set(fake("/w/a.tmd"));
  r.set(fake("/w/b.tmd"));
  assert.strictEqual(r.previewFor("/w/c.tmd"), undefined);
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd editor/vscode && npm test
```

Expected: FAIL, "Cannot find module '../previews'".

- [ ] **Step 3: Write the registry**

Create `editor/vscode/src/previews.ts`:

```ts
import * as vscode from "vscode";
import { PreviewServer } from "./server";

export interface LivePreview {
  panel: vscode.WebviewPanel;
  server: PreviewServer;
  /** The document this preview was started for. Its directory anchors path resolution. */
  docPath: string;
}

/**
 * The previews currently alive, keyed by document path.
 *
 * Without this, `openPreview` allocated a port, spawned `taliesin preview` and created a
 * webview on every invocation, so pressing the shortcut twice on one file left two servers
 * and two file watchers running against it.
 */
export class PreviewRegistry {
  private readonly live = new Map<string, LivePreview>();
  /**
   * Documents whose server is mid-spawn. A start is `await`ed, so two keypresses inside the
   * startup window would both miss `live` and both spawn — the exact leak the registry exists
   * to prevent, reintroduced through the back door.
   */
  private readonly starting = new Set<string>();

  get size(): number {
    return this.live.size;
  }

  get(docPath: string): LivePreview | undefined {
    return this.live.get(docPath);
  }

  set(preview: LivePreview): void {
    this.live.set(preview.docPath, preview);
  }

  /** Idempotent: closing a panel and disposing the extension may both reach this. */
  delete(docPath: string): void {
    this.live.delete(docPath);
  }

  /** True if the caller owns the start; false if one is already in flight. */
  beginStart(docPath: string): boolean {
    if (this.starting.has(docPath)) return false;
    this.starting.add(docPath);
    return true;
  }

  endStart(docPath: string): void {
    this.starting.delete(docPath);
  }

  /**
   * The preview a given buffer belongs to: its own if one is open, else the single open
   * preview if there is exactly one.
   *
   * The fallback is what makes forward search work from an INCLUDED file, whose blocks appear
   * in the parent document's preview but which was never itself previewed. It deliberately
   * declines to guess when several previews are open, because picking the wrong one would
   * scroll a document the author is not looking at.
   */
  previewFor(file: string): LivePreview | undefined {
    const own = this.live.get(file);
    if (own) return own;
    if (this.live.size !== 1) return undefined;
    return [...this.live.values()][0];
  }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd editor/vscode && npm test
```

Expected: all seven tests PASS.

- [ ] **Step 5: Wire it into openPreview**

In `editor/vscode/src/extension.ts`, add to the imports:

```ts
import { PreviewRegistry, LivePreview } from "./previews";
```

Add a module-level instance immediately after the imports:

```ts
/** Module-level, not per-activation: `openPreview` is a free function and both it and the
 *  reveal command must see the same set of live previews. */
const previews = new PreviewRegistry();
```

Then, inside `openPreview`, immediately after the `if (!docPath) { … return; }` block, insert:

```ts
  // Reuse before spawn. A second invocation reveals the panel it already has.
  const existing = previews.get(docPath);
  if (existing) {
    existing.panel.reveal(vscode.ViewColumn.Beside);
    return;
  }
  if (!previews.beginStart(docPath)) return; // a start is already in flight
```

Wrap the existing `try { server = await … } catch { … }` so the latch is always released. Replace:

```ts
  let server: PreviewServer;
  try {
    server = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Starting Taliesin preview…" },
      () => PreviewServer.start(binary, docPath)
    );
  } catch (e) {
    vscode.window.showErrorMessage(String((e as Error).message || e));
    return;
  }
```

with:

```ts
  let server: PreviewServer;
  try {
    server = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Starting Taliesin preview…" },
      () => PreviewServer.start(binary, docPath)
    );
  } catch (e) {
    vscode.window.showErrorMessage(String((e as Error).message || e));
    previews.endStart(docPath); // a failed start must not wedge the document forever
    return;
  }
```

After `panel.webview.html = relayHtml(…)`, register it:

```ts
  previews.set({ panel, server, docPath });
  previews.endStart(docPath);
```

And in the `panel.onDidDispose` callback, add the removal as the first line:

```ts
  panel.onDidDispose(
    () => {
      previews.delete(docPath);
      sel.dispose();
      if (timer) clearTimeout(timer);
      server.dispose();
    },
```

- [ ] **Step 6: Verify the whole companion suite**

```bash
cd editor/vscode && npm test && npm run test:e2e
```

Expected: both PASS. The e2e suite runs headless in this environment despite the README's claim
that it needs a display. Quote the summary lines.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/src/previews.ts editor/vscode/src/test/previews.test.ts editor/vscode/src/extension.ts
git commit -m "preview: one panel and one server per document"
```

---

## Task 5: The forward-search command

**Files:**
- Modify: `editor/vscode/src/extension.ts`
- Modify: `editor/vscode/package.json`

**Interfaces:**
- Consumes: `PreviewRegistry.previewFor` (Task 4); the `reveal` flag (Task 3).
- Produces: command id `taliesin.revealInPreview`.

**Why `Ctrl+Alt+J`.** It is LaTeX Workshop's forward-search key, which is the convention the author
already has. `Ctrl+Alt+M` (insert math symbol) is the only neighbouring binding and does not clash.

- [ ] **Step 1: Register the command**

In `editor/vscode/src/extension.ts`, inside `activate`, add a second `registerCommand` next to the
existing `taliesin.openPreview` registration:

```ts
  context.subscriptions.push(
    // Forward search, active half: put the preview where the cursor is, on request.
    // The passive half (marking, never scrolling) rides the selection listener in
    // `openPreview` and sends `reveal: false`.
    vscode.commands.registerCommand("taliesin.revealInPreview", () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || !isSourceFile(editor.document.fileName)) {
        vscode.window.showWarningMessage("Taliesin: open a .tmd file first.");
        return;
      }
      const target = previews.previewFor(editor.document.fileName);
      if (!target) {
        vscode.window.showWarningMessage(
          previews.size > 1
            ? "Taliesin: several previews are open. Open this document's preview to reveal in it."
            : "Taliesin: open a preview first (Ctrl+Shift+K)."
        );
        return;
      }
      // preserveFocus: revealing must not steal the cursor from the editor the author is
      // typing in — the whole point is to look at the preview without leaving the text.
      target.panel.reveal(vscode.ViewColumn.Beside, true);
      target.panel.webview.postMessage({
        type: "tali-cursor",
        file: relativeKey(target.docPath, editor.document.fileName),
        line: editor.selection.active.line + 1,
        reveal: true,
      });
    })
  );
```

- [ ] **Step 2: Contribute the command, keybinding and palette entry**

In `editor/vscode/package.json`, add to `contributes.commands`:

```json
{
  "command": "taliesin.revealInPreview",
  "title": "Reveal Cursor in Preview",
  "category": "Taliesin"
}
```

Add to `contributes.keybindings`:

```json
{
  "command": "taliesin.revealInPreview",
  "key": "ctrl+alt+j",
  "mac": "cmd+alt+j",
  "when": "editorLangId == taliesin && editorTextFocus"
}
```

Add to `contributes.menus.commandPalette`:

```json
{
  "command": "taliesin.revealInPreview",
  "when": "resourceExtname == .tmd"
}
```

- [ ] **Step 3: Compile and run the companion suite**

```bash
cd editor/vscode && npm run build && npm test && npm run test:e2e
```

Expected: all PASS. Quote the summary lines.

- [ ] **Step 4: Verify by hand in a real editor**

**This step is not optional and cannot be replaced by the automated suite.** Click-to-source has no
automated end-to-end coverage in this repo: the harness stops at the relay. Every gate above can be
green while the gesture is broken in VS Code.

1. Open a `.tmd` file and press `Ctrl+Shift+K`.
2. Press `Ctrl+Shift+K` again. Confirm **one** panel, not two. Confirm with
   `ps -eo args | grep 'taliesin preview'` that there is **one** server.
3. Scroll the preview away from your cursor, then type a character in the editor. Confirm the
   preview does **not** scroll back, and that the block outline still tracks your cursor.
4. Press `Ctrl+Alt+J`. Confirm the preview scrolls to your block and that the **editor keeps
   focus**.
5. Hold `Ctrl` over the preview. Confirm blocks show a pointer cursor and the block under the mouse
   gets a dashed outline. Click. Confirm the editor cursor lands on the right line.
6. Confirm a plain click in the preview still browses normally and still selects text.

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/extension.ts editor/vscode/package.json
git commit -m "forward search: Ctrl+Alt+J puts the preview where you are"
```

---

## Task 6: Documentation sweep and the retired-name guard

Do this **last**: it is mechanical, and doing it earlier would leave the docs describing a gesture
the code has not got yet.

**Files:**
- Modify: the prose files listed below
- Modify: `crates/core/tests/retired_names.rs`

**Interfaces:**
- Consumes: the `tali-srcnav` name from Task 2.
- Produces: nothing.

- [ ] **Step 1: Write the failing guard test**

Append to `crates/core/tests/retired_names.rs`. It reuses the existing `walk` and `repo_root`
helpers in that file, which already skip build output, `node_modules`, `notes/` and
`docs/superpowers/`.

Two things about this test are load-bearing and are the reason it is written the awkward way:

1. **The needles are assembled at runtime**, exactly like `retired()` above. A guard that contains
   its own needle as a literal flags itself and can never go green.
2. **`altKey` is deliberately NOT a needle.** It is a legitimate DOM property, and
   `deck.js:1505,1696`, `code-enhance/07-keyboard.js:62,68` and `code-enhance/11-lightbox.js:158`
   all test `!e.altKey` as part of "no modifier is held" guards. Those must keep doing so. Only the
   retired *gesture names* are hunted. (`walk` already skips `.min.js`, so vendored `d3.min.js`
   needs no exemption.)

```rust
/// The Alt-click inverse-search gesture was retired for Ctrl/Cmd-click on 2026-07-28.
///
/// Same rationale as the brand guard above: every other assertion for this gesture is a
/// string literal sitting beside its emitter, so a half-finished rename fails nothing. The
/// stale spelling is worse than cosmetic here, because it teaches a gesture that no longer
/// works.
///
/// `altKey` is NOT hunted. It is a legitimate DOM property, and several "no modifier is
/// held" guards (`deck.js`, `code-enhance/07-keyboard.js`, `code-enhance/11-lightbox.js`)
/// must keep testing it. Only the names of the retired gesture are retired.
#[test]
fn the_alt_click_gesture_stays_retired() {
    // Assembled at runtime for the same reason `retired()` is: a guard holding its own
    // needle as a literal reports itself and can never be satisfied.
    let alt = format!("{}{}", "a", "lt");
    let needles = [
        format!("tali-{alt}"),
        format!("{alt}-click"),
        format!("{alt}+click"),
        format!("{}{}", "option", "-click"),
    ];
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    let mut offenders = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if needles.iter().any(|needle| lower.contains(needle.as_str())) {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the retired gesture is still named in {} place(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}
```

- [ ] **Step 2: Run it to see the full offender list**

```bash
cargo test -p taliesin-core --test retired_names the_alt_click_gesture_stays_retired -- --nocapture
```

Expected: FAIL, printing every remaining occurrence with its file and line. **That list is your
worklist for Step 3.** Do not work from the list below alone; the test is the authority.

- [ ] **Step 3: Rewrite every occurrence**

Replace "Alt-click (Option-click on Mac)" and its variants with "Ctrl-click (Cmd-click on Mac)".
Where prose explains the gesture pair, say *inverse search* (preview to editor) and *forward search*
(editor to preview), and mention `Ctrl+Alt+J` wherever the reverse direction is described as
automatic, because it no longer is.

Expected files, counted with `rg -c -i 'alt-click|alt\+click|option-click|tali-alt'` and excluding
`notes/` (the dated pre-change record, left as written) and `docs/superpowers/` (the plan and spec
archive, which both the guard and this list skip):

**Already handled by Task 2, so they should no longer appear:** `web-client/client.js`,
`tools/ui-audit/lib/probe.mjs`, `crates/core/assets/css/base.css`,
`crates/core/assets/css/deck.css`. If the guard still names one of them, Task 2 was left incomplete.

**User-facing prose.** `README.md` (3), `editor/vscode/README.md` (3),
`editor/vscode/walkthroughs/preview.md`, `editor/vscode/package.json` (the walkthrough
description), `web-client/README.md`, `site/features.tmd`, `site/demo.tmd`,
`docs/guide/using/preview.tmd` (5), `docs/guide/tour.tmd`, `docs/guide/demo.tmd`,
`docs/guide/index.tmd`, `docs/guide/using/formats.tmd`, `docs/guide/using/getting-started.tmd`,
`docs/internals/client.tmd` (4), `docs/internals/protocol.tmd` (3),
`docs/internals/block-model.tmd`, `docs/internals/validation.tmd`, `docs/internals/repository.tmd`,
`CLAUDE.md`.

**Code comments and test names.** `crates/core/tests/corpus.rs` (4), `crates/core/src/diff.rs` (2),
`crates/core/assets/js/code-enhance/12-link-preview.js` (2), `crates/core/src/render/emit.rs`,
`crates/core/src/render/model.rs`, `crates/core/src/render/tests.rs`,
`crates/server/src/serve_site/mod.rs`, `tools/live-edit-bench/tests/regression.rs`.

Two are easy to miss and both are user-facing: the **root `README.md`** and the **marketing site**
(`site/features.tmd`, `site/demo.tmd`).

Note that `deck.js`, `code-enhance/07-keyboard.js` and `code-enhance/11-lightbox.js` are **not** on
this list even though they contain `altKey`. Their `!e.altKey` tests are "no modifier is held"
guards and are correct as they stand.

`docs/guide/using/preview.tmd` needs more than a substitution: it currently documents the reverse
sync as automatic scrolling. Rewrite that passage to describe marking as automatic and scrolling as
`Ctrl+Alt+J`.

- [ ] **Step 4: Run the guard until it passes**

```bash
cargo test -p taliesin-core --test retired_names -- --nocapture
```

Expected: PASS, including the pre-existing brand guard.

- [ ] **Step 5: Run every gate**

```bash
./tools/gates.sh
```

Use this rather than running the pieces by hand. It arms all four `TALIESIN_REQUIRE_*` variables and
refuses to be green unless every gate actually ran, which matters because these gates skip silently
when their interpreter is missing. Quote its final verdict.

If `docs/` or `site/` prose changes moved any rendered output, `cargo test -p taliesin-core` will
say so. `body_html_snapshots.rs` reads from `corpus_dir()` only, so `docs/` and `site/` edits should
not drift it; if one does, investigate rather than re-blessing the snapshot.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "docs: teach the gesture the tool actually has"
```

---

## Self-review notes

Checked against the spec:

- **Spec Phase B** → Task 1, including the `::after` requirement, the `gap` collision warning, the
  comment rewrite, the deck case, and the BASE_CSS-not-page assertion trap.
- **Spec A1.1 (vocabulary)** → carried through the comments in Tasks 2, 3, 5 and the prose in Task 6.
- **Spec A1.2 (Ctrl/Cmd, rename, macOS contextmenu)** → Task 2, Steps 1, 2, 4, 5.
- **Spec A1.3 (reveal flag, deck gating, the command)** → Tasks 3 and 5.
- **Spec A1.4 (panel/server reuse)** → Task 4, including the in-flight latch the spec did not name
  but which the `await` in `openPreview` makes necessary.
- **Spec A1.5 (discoverability)** → Task 2 Step 3 (the hint), Task 5 Step 2 (palette entry),
  Task 6 Step 3 (the walkthrough).
- **Spec A1.6 (no behavioural conflicts)** → recorded in Task 2's preamble so the implementer does
  not re-derive it.
- **Spec A1.7 (documentation sweep)** → Task 6, driven by a guard test rather than by the list, so a
  missed file fails the build.
- **Spec verification section** → Task 6 Step 5 (`gates.sh`), Task 4 Step 6 (companion e2e),
  Task 2 Step 7 (`probe-run.mjs`), Task 1 Step 5 (browser).
- **Spec coverage gap** → Task 5 Step 4, marked not optional.

**Spec A2 is intentionally uncovered by this plan** and gets its own plan after A1 lands.
