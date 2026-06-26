# Reader polish bundle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three small reader/a11y/typography wins: CSS typography polish, a skip-to-content link, and a keyboard reader (`?` cheatsheet, `/` search, `←`/`→` chapter nav).

**Architecture:** All client-side + CSS, no Rust/render change. Two new `qmdEnhancers`-registered enhancers (`qmdInitSkipLink`, `qmdInitKeyboard`) in `code-enhance.js`, a one-line `window.qmdOpenSearch` export in `search.js`, and CSS in `base.css`. Reuses `qmdFocusTrap`, the book prev/next anchors, and the search palette.

**Tech Stack:** Vanilla JS (ES5-style, matching the surrounding enhancers), CSS, the shipped `qmdFocusTrap`. Tests: one Rust render assertion (CSS ships) + chrome-devtools browser verification on the guide book.

## Global Constraints

- **No Rust/core change** beyond a test: only `crates/core/assets/css/base.css`, `crates/core/assets/js/code-enhance.js`, `web-client/search.js` (+ a test in `render/tests.rs`).
- **Read-only:** navigation + focus only; never writes source.
- **Deck-skipped** (`.qmd-deck` guard), **idempotent** (`window.__qmdSkipLink` / `window.__qmdKeyboard`), **offline**, no new dependency.
- **JS style:** `var`/`function`/`[].slice.call`, no arrows/`const`/`let`, matching the enhancers.
- **Keyboard guards:** every shortcut is off while typing (INPUT/TEXTAREA/SELECT/contenteditable) or with Ctrl/Cmd/Alt held; `/` and arrows are also off while a modal (`[aria-modal="true"]`) other than the cheatsheet is open; arrows are off when a focused interactive control would use them.
- **Typography CSS is global** (not scoped to `main`/`#qmd-root`): `text-wrap`/orphans are no-ops on short chrome and global scope also covers no-TOC build pages.
- **Shortcuts:** `?` help · `/` search · `f` focus mode (existing) · `←`/`→` prev/next chapter · `Esc` close.

---

### Task 1: Typography polish (CSS) + CSS-ships test

**Files:**
- Modify: `crates/core/assets/css/base.css`
- Test: `crates/core/src/render/tests.rs`

**Interfaces:**
- Consumes: `qmd_fast_core::render::render_html_page(&str, &str) -> String` (full page incl. `<style>`).
- Produces: the bundled typography rules.

- [ ] **Step 1: Write the failing test**

In `crates/core/src/render/tests.rs`, append:

```rust
#[test]
fn typography_polish_css_ships() {
    let page = render_html_page("# Title\n\nSome prose.\n", "doc");
    assert!(page.contains("text-wrap: pretty"), "pretty wrap rule must ship");
    assert!(page.contains("text-wrap: balance"), "balance rule must ship");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::tests::typography_polish_css_ships 2>&1 | tail -8`
Expected: FAIL — the rules are not in base.css yet.

- [ ] **Step 3: Add the CSS**

In `crates/core/assets/css/base.css`, after the reader line-spacing block (search for the comment `Reader line-spacing (Display menu)`, insert after that rule group), add:

```css
  /* Reading-typography polish. Global: text-wrap/orphans are no-ops on short chrome
     (nav/TOC), and global scope also covers no-TOC build pages (no <main> wrapper). */
  p, li { text-wrap: pretty; orphans: 2; widows: 2; }
  h1, h2, h3, h4, h5, h6, figcaption, blockquote, .callout-title { text-wrap: balance; }
  figure, .qmd-figure { break-inside: avoid; }
  figcaption { break-before: avoid; }
  h1, h2, h3, h4, h5, h6 { break-after: avoid; }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib render::tests::typography_polish_css_ships 2>&1 | tail -8`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/css/base.css crates/core/src/render/tests.rs
git commit -m "feat(reader): typography polish CSS (text-wrap pretty/balance, widow/orphan, caption-keep)"
```

---

### Task 2: Skip-to-content link

**Files:**
- Modify: `crates/core/assets/js/code-enhance.js` (`qmdInitSkipLink` + register)
- Modify: `crates/core/assets/css/base.css` (`.qmd-skip`)

**Interfaces:**
- Consumes: `window.qmdEnhancers.register`.
- Produces: `qmdInitSkipLink()`; a `.qmd-skip` link targeting `#qmd-main`.

- [ ] **Step 1: Add the enhancer**

In `crates/core/assets/js/code-enhance.js`, above the built-in-enhancers registration block (near the other reader enhancers), add:

```js
// Skip-to-content link: a visually-hidden-until-focused link that jumps keyboard /
// screen-reader users past the chrome to the content. The content container varies by
// mode (build <main>, preview #qmd-root, no-TOC build = first block), so resolve it at
// runtime. Read-only, deck-skipped, idempotent.
function qmdInitSkipLink() {
  if (window.__qmdSkipLink) return;
  if (document.querySelector('.qmd-deck')) return;
  var main =
    document.querySelector('main') ||
    document.getElementById('qmd-root') ||
    document.querySelector('[data-block-id]');
  if (!main) return;
  window.__qmdSkipLink = true;
  if (!main.id) main.id = 'qmd-main';
  main.setAttribute('tabindex', '-1');
  if (document.querySelector('.qmd-skip')) return;
  var a = document.createElement('a');
  a.className = 'qmd-skip';
  a.href = '#' + main.id;
  a.textContent = 'Skip to content';
  a.addEventListener('click', function () {
    // move focus (not just scroll) so the keyboard reader continues from the content
    setTimeout(function () { main.focus(); }, 0);
  });
  document.body.insertBefore(a, document.body.firstChild);
}
```

- [ ] **Step 2: Register it**

In the registration block (after `qmdInitReadAloud`, before `qmdInitCategoryFilter`), add:

```js
window.qmdEnhancers.register(function () { qmdInitSkipLink(); });
```

- [ ] **Step 3: Add the CSS**

In `crates/core/assets/css/base.css`, near the other reader-chrome rules, add:

```css
  /* Skip-to-content link (qmdInitSkipLink): hidden until focused, then pinned top-left. */
  .qmd-skip { position: fixed; top: -3rem; left: .5rem; z-index: 2147482000;
    background: var(--qmd-fg); color: var(--qmd-bg); padding: .5rem .8rem;
    border-radius: 6px; font: 600 .9rem var(--qmd-font-head); text-decoration: none; }
  .qmd-skip:focus { top: .5rem; }
  @media (prefers-reduced-motion: no-preference) { .qmd-skip { transition: top .15s ease; } }
```

- [ ] **Step 4: Syntax-check + build**

Run: `node --check crates/core/assets/js/code-enhance.js && cargo build -p qmd-fast-core 2>&1 | tail -2`
Expected: no error; build succeeds. (Browser verification is in Task 4.)

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance.js crates/core/assets/css/base.css
git commit -m "feat(reader): skip-to-content link (a11y)"
```

---

### Task 3: Keyboard reader (`?` cheatsheet, `/` search, `←`/`→` chapter nav)

**Files:**
- Modify: `web-client/search.js` (export `window.qmdOpenSearch`)
- Modify: `crates/core/assets/js/code-enhance.js` (`qmdInitKeyboard` + cheatsheet + register)
- Modify: `crates/core/assets/css/base.css` (`.qmd-keys` cheatsheet)

**Interfaces:**
- Consumes: `window.qmdFocusTrap(container, initial) -> release`; `.qmd-book-prev`/`.qmd-book-next` anchors; `window.qmdOpenSearch` (this task).
- Produces: `qmdInitKeyboard()`; `window.qmdOpenSearch`.

- [ ] **Step 1: Export the search opener**

In `web-client/search.js`, inside the IIFE (after `function open()` is defined; just before the final `})();`), add:

```js
  window.qmdOpenSearch = open;
```

- [ ] **Step 2: Add the keyboard enhancer + cheatsheet**

In `crates/core/assets/js/code-enhance.js`, near `qmdInitSkipLink`, add:

```js
// Keyboard reader: `?` opens a shortcuts cheatsheet, `/` opens search, `←`/`→` move to
// the previous/next chapter (the book prev/next anchors). All guarded so they never fire
// while typing or under another modal. Read-only, deck-skipped, idempotent.
function qmdInitKeyboard() {
  if (window.__qmdKeyboard) return;
  if (document.querySelector('.qmd-deck')) return;
  window.__qmdKeyboard = true;

  var sheet = null;
  var sheetRelease = null;
  function buildSheet() {
    var wrap = document.createElement('div');
    wrap.className = 'qmd-keys';
    wrap.setAttribute('role', 'dialog');
    wrap.setAttribute('aria-modal', 'true');
    wrap.setAttribute('aria-label', 'Keyboard shortcuts');
    wrap.hidden = true;
    var card = document.createElement('div');
    card.className = 'qmd-keys-card';
    card.innerHTML =
      '<h2>Keyboard shortcuts</h2>' +
      '<dl class="qmd-keys-list">' +
      '<div><dt><kbd>?</kbd></dt><dd>Show this help</dd></div>' +
      '<div><dt><kbd>/</kbd></dt><dd>Search</dd></div>' +
      '<div><dt><kbd>f</kbd></dt><dd>Focus mode</dd></div>' +
      '<div><dt><kbd>&larr;</kbd> <kbd>&rarr;</kbd></dt><dd>Previous / next chapter</dd></div>' +
      '<div><dt><kbd>Esc</kbd></dt><dd>Close</dd></div>' +
      '</dl>';
    var close = document.createElement('button');
    close.className = 'qmd-keys-close';
    close.type = 'button';
    close.setAttribute('aria-label', 'Close');
    close.textContent = '×';
    card.appendChild(close);
    wrap.appendChild(card);
    document.body.appendChild(wrap);
    close.addEventListener('click', closeSheet);
    wrap.addEventListener('click', function (e) { if (e.target === wrap) closeSheet(); });
    sheet = wrap;
  }
  function sheetOpen() { return !!sheet && !sheet.hidden; }
  function openSheet() {
    if (!sheet) buildSheet();
    sheet.hidden = false;
    if (window.qmdFocusTrap) {
      sheetRelease = window.qmdFocusTrap(sheet, sheet.querySelector('.qmd-keys-close'));
    }
  }
  function closeSheet() {
    if (!sheetOpen()) return;
    sheet.hidden = true;
    if (sheetRelease) { sheetRelease(); sheetRelease = null; }
  }

  document.addEventListener('keydown', function (e) {
    var t = e.target;
    var typing =
      t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    var modal = document.querySelector('[aria-modal="true"]');
    // `?` (Shift+/) toggles help — allowed even when the cheatsheet itself is open.
    if (e.key === '?' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (modal && !sheetOpen()) return; // a different modal owns the keys
      e.preventDefault();
      if (sheetOpen()) closeSheet(); else openSheet();
      return;
    }
    if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'Escape' && sheetOpen()) { e.preventDefault(); closeSheet(); return; }
    if (modal) return;
    if (e.key === '/') {
      if (window.qmdOpenSearch) { e.preventDefault(); window.qmdOpenSearch(); }
      return;
    }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      // leave arrows to a focused interactive control (slider, tablist, link, button)
      if (t && t.closest && t.closest('a,button,input,select,textarea,[role="tab"]')) return;
      var nav = document.querySelector(e.key === 'ArrowRight' ? '.qmd-book-next' : '.qmd-book-prev');
      if (nav && nav.href) { e.preventDefault(); window.location.assign(nav.href); }
    }
  });
}
```

(The `innerHTML` is a constant string with no untrusted data, so it is XSS-safe.)

- [ ] **Step 3: Register it**

In the registration block, after the skip-link registration, add:

```js
window.qmdEnhancers.register(function () { qmdInitKeyboard(); });
```

- [ ] **Step 4: Add the cheatsheet CSS**

In `crates/core/assets/css/base.css`, after the `.qmd-skip` rules, add:

```css
  /* Keyboard shortcuts cheatsheet (qmdInitKeyboard). */
  .qmd-keys { position: fixed; inset: 0; z-index: 2147482000; display: flex;
    align-items: center; justify-content: center; background: rgba(0, 0, 0, .45); }
  .qmd-keys[hidden] { display: none; }
  .qmd-keys-card { position: relative; background: var(--qmd-bg); color: var(--qmd-fg);
    border: 1px solid var(--qmd-border); border-radius: 12px; padding: 1.4rem 1.6rem;
    max-width: 22rem; box-shadow: 0 10px 40px rgba(0, 0, 0, .3); }
  .qmd-keys-card h2 { margin: 0 0 .8rem; font-size: 1.1rem; }
  .qmd-keys-list { margin: 0; }
  .qmd-keys-list > div { display: flex; gap: .8rem; align-items: baseline; margin: .35rem 0; }
  .qmd-keys-list dt { flex: 0 0 5rem; margin: 0; }
  .qmd-keys-list dd { margin: 0; color: var(--qmd-muted); }
  .qmd-keys kbd { font: inherit; font-size: .8rem; background: var(--qmd-code-bg);
    border: 1px solid var(--qmd-border); border-radius: 4px; padding: .05rem .35rem; }
  .qmd-keys-close { position: absolute; top: .5rem; right: .6rem; border: 0;
    background: transparent; color: var(--qmd-muted); cursor: pointer; font-size: 1.2rem; line-height: 1; }
```

- [ ] **Step 5: Syntax-check + build**

Run: `node --check crates/core/assets/js/code-enhance.js && node --check web-client/search.js && cargo build -p qmd-fast-core 2>&1 | tail -2`
Expected: no error; build succeeds.

- [ ] **Step 6: Commit**

```bash
git add web-client/search.js crates/core/assets/js/code-enhance.js crates/core/assets/css/base.css
git commit -m "feat(reader): keyboard reader — ? cheatsheet, / search, arrow chapter nav"
```

---

### Task 4: Browser verification + docs + full verification

**Files:**
- Modify: `docs/guide/using/reading.qmd` (document the skip link + keyboard shortcuts)
- Modify: `notes/backlog.md`, `notes/FEATURE-IDEAS.md` (mark #16/#19/#23/#55 shipped)

- [ ] **Step 1: Browser-verify on the guide book (chrome-devtools MCP)**

Build the server and serve the guide (it has a TOC, search, and chapters with prev/next):
```bash
cargo build -p qmd-fast-server
./target/debug/qmd-fast preview docs/guide 4392   # run in background
```
Navigate to `http://127.0.0.1:4392/using/reading.html` and verify with `evaluate_script` / key dispatch:
- The skip link exists and targets the content: `document.querySelector('.qmd-skip')` is present; its `href` is `#` + an id that exists; that element has `tabindex="-1"`.
- `?` opens the cheatsheet: dispatch `new KeyboardEvent('keydown',{key:'?',bubbles:true})` on `document` → `.qmd-keys:not([hidden])` exists with `role=dialog`/`aria-modal`; dispatch again → hidden.
- `/` opens search: dispatch `{key:'/'}` → the search overlay is open (`window.qmdOpenSearch` ran).
- `→` navigates: on a chapter with a `.qmd-book-next`, confirm `.qmd-book-next` exists and its href is the next page (assert the element + href rather than actually navigating, to keep the page stable; optionally navigate and assert `location`).
- Guard: focus the search input, dispatch `{key:'/'}` and `{key:'?'}` → no extra cheatsheet/search toggling (the guard holds).
- `list_console_messages` → 0 errors. Screenshot the cheatsheet.

- [ ] **Step 2: Document in the reading guide page**

In `docs/guide/using/reading.qmd`, add a short "Keyboard and accessibility" section: the `Skip to content` link (Tab from the top), and the shortcuts (`?` help, `/` search, `f` focus mode, `←`/`→` previous/next chapter, `Esc` close). No em dashes.

- [ ] **Step 3: Notes**

In `notes/backlog.md`, add a one-line shipped note (reader polish bundle: typography polish [text-wrap pretty/balance + widow/orphan + caption-keep], skip-to-content link, keyboard reader [? cheatsheet, / search, arrow chapter nav]; all client-side + CSS, no core change). In `notes/FEATURE-IDEAS.md`, mark #16, #19, #23, and #55 ✅ SHIPPED 2026-06-26.

- [ ] **Step 4: Full verification**

Run:
```bash
cargo test -p qmd-fast-core
cargo fmt --check
node --check crates/core/assets/js/code-enhance.js && node --check web-client/search.js
```
Expected: all pass; fmt clean; JS OK.

- [ ] **Step 5: Commit**

```bash
git add docs/guide/using/reading.qmd notes/backlog.md notes/FEATURE-IDEAS.md
git commit -m "docs(reader): document the keyboard/skip-link polish; mark ideas #16/#19/#23/#55 shipped"
```

---

## Self-Review

**Spec coverage:**
- Typography polish (pretty/balance/orphans/caption-keep), global → Task 1. ✓
- Skip-to-content (enhancer + CSS, content-container fallback chain, focus on activate) → Task 2. ✓
- Keyboard reader (`?` cheatsheet via qmdFocusTrap, `/` search, `←`/`→` chapter nav, guards) + `qmdOpenSearch` export → Task 3. ✓
- Deck-skip, idempotent, no core change, offline → Global Constraints + each enhancer. ✓
- Tests: CSS-ships (Rust) + browser verification on the guide book → Tasks 1, 4. ✓
- Docs → Task 4. ✓

**Placeholder scan:** no TBD/TODO; all code complete; the one constant-`innerHTML` is flagged XSS-safe. ✓

**Type/name consistency:** `qmdInitSkipLink()`, `qmdInitKeyboard()`, `window.qmdOpenSearch`, `window.qmdFocusTrap`, the `.qmd-skip`/`.qmd-keys`/`.qmd-keys-card`/`.qmd-keys-close`/`.qmd-keys-list` classes, and the `.qmd-book-prev`/`.qmd-book-next` anchors are consistent across the JS (Tasks 2-3) and the CSS (Tasks 2-3) and the browser checks (Task 4). The CSS-ships test uses `render_html_page` (full page incl. `<style>`), not `body_html`. ✓
```
