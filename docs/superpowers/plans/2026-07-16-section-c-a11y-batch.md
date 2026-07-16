# Section C a11y batch — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the six verified a11y fixes in §C of `notes/backlog.md`, and delete the two that turned out to be rot.

**Architecture:** Six independent fixes across three surfaces: the concatenated client JS fragments in `crates/core/assets/js/code-enhance/`, the bundled CSS in `crates/core/assets/css/`, and one server-side listing emitter in `crates/core/src/site/mod.rs`. Only the listing chips have an automated test seam (Rust, over the built corpus blog); the rest are DOM/CSS behavior verified through the chrome-devtools browser loop.

**Tech Stack:** Rust (edition 2024), vanilla ES5-style browser JS (no build step, no framework), plain CSS. No new dependencies.

**Spec:** [`docs/superpowers/specs/2026-07-16-section-c-a11y-batch-design.md`](../specs/2026-07-16-section-c-a11y-batch-design.md)

**Branch:** `section-c-a11y` (already created, spec committed at `0fa0bc6`).

## Global Constraints

- **The fragments share one scope.** `assets/js/code-enhance/*.js` are concatenated in filename order into one script by `render/mod.rs:1109-1124`. A top-level `function`/`var` in any fragment is visible to every other fragment at runtime. There are no modules, no imports, no bundler.
- **`01-registry.js` is evaluated twice.** It is inlined standalone as `REGISTRY_JS` (`render/page.rs:231`) *and* concatenated as the first fragment of `code-enhance.js`. Anything added there must be idempotent. Plain function declarations are fine; do not add side effects.
- **ES5 style, matching the existing files.** `var`, `function`, no arrow functions in new code, no optional chaining. (`10-category-filter.js` uses one spread at `:18`; do not take that as licence to broaden.)
- **Assets are `include_str!`-compiled.** After editing any `assets/css/*` or `assets/js/*`, a `cargo build` is required before a *built* site reflects it. A live `preview` hot-swaps CSS. Never measure a built page without rebuilding the binary first.
- **Storage key is `tali-shortcuts`.** Not `qmd-shortcuts`. The rationale is load-bearing and must be recorded in a comment (see Task 3).
- **Never touch:** the exec/kernel zone; the single-editing-surface invariant (no preview write-back); `--tali-*` tokens only; no CDN; no new document config or `_site.yml` knob.
- **No em dashes in any prose you write** (comments, docs, commit messages). Use commas, colons, or parentheses.
- **Do not reformat untouched lines.** A `PostToolUse` hook runs `rustfmt` on edited `.rs` files; CI enforces `cargo fmt`.

---

### Task 1: Category-filter chips expose state to assistive tech

Backlog item: *"Category-filter chips expose state only visually"* (med). This is one logical change spanning server and client: the server must paint the correct initial state before JS runs, and the client must keep it in sync. Ship them together.

**Files:**
- Test: `crates/core/tests/tech_blog.rs` (add after the existing `tali-cat` assertion at `:299`)
- Modify: `crates/core/src/site/mod.rs:1145-1153` (the `listing_html` chip emitter)
- Modify: `crates/core/assets/js/code-enhance/10-category-filter.js:22-34` (the `apply` closure)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: nothing other tasks depend on.

- [ ] **Step 1: Write the failing test**

In `crates/core/tests/tech_blog.rs`, immediately after line 299 (`assert!(blog.contains("tali-cat"), "blog: category badges missing");`), add:

```rust
    // Filter chips expose their state to assistive tech, not only visually: the
    // server's initial paint has "All" pressed and every category chip unpressed, so
    // a screen reader reads the filter correctly before the client enhancer runs.
    // The client mirrors this on every toggle (10-category-filter.js). WCAG 4.1.2.
    assert!(
        blog.contains("aria-pressed=\"true\" data-cat=\"\">All</button>"),
        "blog: All chip has no aria-pressed=\"true\""
    );
    assert!(
        blog.contains("class=\"tali-cat-chip\" type=\"button\" aria-pressed=\"false\" data-cat=\""),
        "blog: category chips have no aria-pressed=\"false\""
    );
```

This is pinned on `corpus/tech-blog/blog.tmd`, which sets `listing: { categories: true }` and is the page already rendered into `blog` at `tech_blog.rs:282`. It is the only corpus listing that emits a chip row.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p taliesin-core --test tech_blog 2>&1 | tail -20`

Expected: FAIL, with `blog: All chip has no aria-pressed="true"`.

This failure IS the mutation check the backlog's **gate the gate** rule requires (a drift test that cannot fail is worse than none). The assertion strings are the exact bytes the emitter is about to produce, so if this passes now, stop: something else already emits them and the item is rot.

- [ ] **Step 3: Emit `aria-pressed` from the server**

In `crates/core/src/site/mod.rs`, replace the chip-building block at `:1145-1153`:

```rust
        // `aria-pressed` mirrors the visual `tali-cat-active` state for assistive tech.
        // Emitted server-side so the initial paint is correct before the client enhancer
        // runs; 10-category-filter.js keeps it in sync on every toggle.
        let mut chips = String::from(
            "<button class=\"tali-cat-chip tali-cat-active\" type=\"button\" \
             aria-pressed=\"true\" data-cat=\"\">All</button>",
        );
        for (cat, n) in &counts {
            chips.push_str(&format!(
                "<button class=\"tali-cat-chip\" type=\"button\" aria-pressed=\"false\" \
                 data-cat=\"{c}\">{label}\
                 <span class=\"tali-cat-count\">{n}</span></button>",
                c = esc(cat),
                label = esc(cat),
            ));
        }
```

Note: Rust's `\` line continuation eats the newline *and* the following indentation, so each emitted attribute is separated by exactly one space.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p taliesin-core --test tech_blog 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Run the full core suite for drift**

Run: `cargo test -p taliesin-core 2>&1 | tail -25`

Expected: PASS. If `body_html_snapshots` fails, the chip HTML is snapshotted; review the diff, confirm it shows only the `aria-pressed` additions, and update the snapshot.

- [ ] **Step 6: Mirror `aria-pressed` + announce the count on the client**

In `crates/core/assets/js/code-enhance/10-category-filter.js`, after `if (!listing) return;` (`:12`), add the live region and its guard:

```js
    // A polite live region announces the result of a filter change; without it the
    // chips silently reorder the page for a screen-reader user. `tali-sr-only` is the
    // same visually-hidden class 03-focus-mode.js uses for its announcements.
    var live = document.createElement('span');
    live.className = 'tali-sr-only';
    live.setAttribute('aria-live', 'polite');
    wrap.appendChild(live);
    var announced = false;
```

Then replace the `apply` closure (`:22-34`) in full:

```js
    var apply = function () {
      var shown = 0, total = 0;
      listing.querySelectorAll('.tali-card').forEach(function (card) {
        var show = selected.size === 0 || catsOf(card).some(function (c) { return selected.has(c); });
        card.style.display = show ? '' : 'none';
        total++;
        if (show) shown++;
      });
      filter.querySelectorAll('.tali-cat-chip').forEach(function (chip) {
        var c = chip.getAttribute('data-cat');
        var on = c === '' ? selected.size === 0 : selected.has(c);
        chip.classList.toggle('tali-cat-active', on);
        chip.setAttribute('aria-pressed', on ? 'true' : 'false');
      });
      listing.querySelectorAll('.tali-cat[data-cat]').forEach(function (tag) {
        tag.classList.toggle('tali-cat-on', selected.has(tag.getAttribute('data-cat')));
      });
      // The first apply() is the initial paint, not a change: announcing there would
      // speak the unfiltered count at page load. Only real toggles announce.
      if (announced) live.textContent = 'Showing ' + shown + ' of ' + total + ' posts';
      announced = true;
    };
```

- [ ] **Step 7: Commit**

```bash
git add crates/core/tests/tech_blog.rs crates/core/src/site/mod.rs \
        crates/core/assets/js/code-enhance/10-category-filter.js
git commit -m "fix(a11y): category chips expose filter state (aria-pressed + live count)"
```

---

### Task 2: Settings popover moves focus to its first control on open

Backlog item: *"Settings popover never takes focus on open"* (med). Owner-decided this session: move focus (option A of three).

**Files:**
- Modify: `crates/core/assets/js/code-enhance/04-focus-trap.js:5-24` (extract `taliFocusables`)
- Modify: `crates/core/assets/js/code-enhance/13-reader-menu.js:38-39, 56-63` (comment + `openMenu`)

**Interfaces:**
- Produces: `taliFocusables(container) -> Element[]` — visible focusable descendants in DOM order. Task 2 is the only definer; Task 3 does not use it.

- [ ] **Step 1: Extract the focusable-element helper**

`04-focus-trap.js` currently defines its selector and visibility filter *inside* the trap closure (`SEL` at `:7`, `focusables()` at `:9-13`), so `13-reader-menu.js` cannot reuse them. Hoist both to file scope. Replace lines 1-13 with:

```js
// Visible focusable descendants of `container`, in DOM order. Shared by the modal trap below
// and the reader menu's focus-on-open (13-reader-menu.js): one definition so the two cannot
// drift. The `el === document.activeElement` clause keeps a zero-size element that currently
// holds focus. (The fragments are concatenated into one scope, so this is visible to 13.)
var TALI_FOCUS_SEL = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
function taliFocusables(container) {
  return [].slice.call(container.querySelectorAll(TALI_FOCUS_SEL)).filter(function (el) {
    return el.offsetWidth > 0 || el.offsetHeight > 0 || el === document.activeElement;
  });
}

// Shared modal focus trap: while a modal is open, confine Tab/Shift+Tab to `container`, mark it
// aria-modal, and (on release) restore focus to the opener IF focus is still inside (a keyboard
// or programmatic close) — not when the user clicked elsewhere. Used by the lightbox + reader
// menu here and, via this global, by the Cmd-K palette in search.js. Returns release().
window.taliFocusTrap = window.taliFocusTrap || function (container, initial) {
  var prev = document.activeElement;
  container.setAttribute('aria-modal', 'true');
```

Then update the two former call sites inside the trap. At what is now the `onKey` body, change `var f = focusables();` to:

```js
    var f = taliFocusables(container);
```

and change the initial-focus line (was `:24`) to:

```js
  try { (initial || taliFocusables(container)[0] || container).focus(); } catch (e) {}
```

This is a mechanical extraction: same selector, same filter, same order. No behavior change is intended. The trap is live in the Cmd-K palette and the lightbox, so both get a regression check in Task 7.

- [ ] **Step 2: Replace the stale comment in `13-reader-menu.js`**

The comment at `:38-39` ("does NOT trap or move focus") and `:56-58` are about to become false in half. Replace the `:38-39` comment with:

```js
  // A DISCLOSURE, not a dialog: the launcher's aria-expanded + aria-controls point at a labelled
  // group, the correct ARIA shape for a light-dismiss popover. It does not TRAP focus (see below)
  // but it does MOVE focus in on open: the disclosure "leave focus where it is" rule assumes the
  // panel follows its trigger in DOM order so you can Tab straight into it. This panel is appended
  // to <body> while the gear lives in the navbar, so without the move, opening the menu from the
  // keyboard strands you a whole page away from what you just opened. Esc restores focus (below).
```

and replace the `:56-58` comment with:

```js
  // Light-dismiss POPOVER, not a modal (it doesn't cover/inert the page): no taliFocusTrap, so
  // trapping/focus-restore can't fight the outside-click dismissal — and aria-modal would suppress
  // the reader shortcuts, which treat [aria-modal="true"] as "a modal owns the keys". Moving focus
  // once on open is not trapping and does not fight dismissal. aria-expanded + Esc-to-close
  // (returning focus to the launcher) + click-away is the right shape.
```

- [ ] **Step 3: Move focus in `openMenu`**

Replace `openMenu` (`:60-63`):

```js
  function openMenu() {
    panel.hidden = false; setExpanded(true);
    sections.forEach(function (s) { if (s.onOpen) s.onOpen(); });
    // Focus AFTER unhiding and AFTER the onOpen hooks: taliFocusables filters on
    // offsetWidth/Height, so a still-hidden panel yields nothing, and a hook may have just
    // shown or hidden its own controls (07-keyboard's shortcut list does exactly that).
    var first = taliFocusables(panel)[0];
    if (first) { try { first.focus(); } catch (e) {} }
  }
```

- [ ] **Step 4: Verify it compiles into the bundle**

Run: `cargo build -p taliesin-core 2>&1 | tail -5`
Expected: builds clean. (The JS is `include_str!`-compiled; this only proves the file is still embedded, not that the JS is correct. Behavior is verified in Task 7.)

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance/04-focus-trap.js \
        crates/core/assets/js/code-enhance/13-reader-menu.js
git commit -m "fix(a11y): settings popover moves focus to its first control on open"
```

---

### Task 3: Reader toggle for single-key shortcuts (WCAG 2.1.4)

Backlog item: *"Bare `f` forces fullscreen with no opt-out"* (med). Owner-decided this session: `f` **keeps** its fullscreen coupling; this toggle is the opt-out. Scope correction from the spec: the toggle gates all three printable shortcuts (`f`, `?`, `/`), not just `f`, because WCAG 2.1.4 covers every character key. Esc and the arrow keys are non-printable and stay unconditional.

**Files:**
- Modify: `crates/core/assets/js/code-enhance/01-registry.js` (append the accessor pair)
- Modify: `crates/core/assets/js/code-enhance/07-keyboard.js:13-23` (toggle row) and `:31, :38` (gates)
- Modify: `crates/core/assets/js/code-enhance/03-focus-mode.js:80` (gate)

**Interfaces:**
- Consumes: `window.taliReaderMenu.addSection(title, node, onOpen) -> { setVisible(v) }` (existing, `13-reader-menu.js:90`).
- Produces: `taliShortcutsOn() -> boolean` and `taliSetShortcuts(on: boolean) -> void`, both consumed by `03-focus-mode.js` and `07-keyboard.js`.

- [ ] **Step 1: Add the shared accessor pair**

Append to `crates/core/assets/js/code-enhance/01-registry.js`:

```js
// Reader preference: are the single-key shortcuts (`f`, `?`, `/`) live? WCAG 2.1.4 (Character
// Key Shortcuts) requires a way to turn character-key shortcuts off; this is that mechanism, and
// it is why `f` can keep entering fullscreen directly. Default ON, so a reader who never opens
// Settings sees no change. Esc + the arrow keys are not character keys and are never gated.
// A blocked or throwing localStorage must not silently cost a reader their shortcuts, so every
// failure path returns true.
//
// Key: `tali-shortcuts`. This deliberately does NOT match its only two siblings, `qmd-theme`
// (render/theme.rs) and `qmd-deck-theme`, which still carry the retired `qmd-` prefix. Those are
// frozen: a storage key has no aliasing mechanism, so renaming one would silently reset every
// existing reader's saved choice. A brand-new key carries no such burden and uses the owned
// prefix. The mismatch is intentional; do not "fix" it.
function taliShortcutsOn() {
  try { return localStorage.getItem('tali-shortcuts') !== 'off'; } catch (e) { return true; }
}
// Absent === on (the default), mirroring how theme.rs stores its non-default choices only.
function taliSetShortcuts(on) {
  try {
    if (on) localStorage.removeItem('tali-shortcuts');
    else localStorage.setItem('tali-shortcuts', 'off');
  } catch (e) {}
}
```

- [ ] **Step 2: Build the toggle row in `07-keyboard.js`**

The row must live in the *Keyboard shortcuts* section (it governs that list) but **outside** the list itself, or switching shortcuts off would hide the control that switches them back on. Replace the `if (window.taliReaderMenu) { ... }` block at `:13-23` in full:

```js
  // Mount the shortcut list into the Settings menu (built by taliInitReaderMenu, which runs
  // first via the registry order). A static list of literal <kbd>s, no interpolation.
  if (window.taliReaderMenu) {
    var dl = document.createElement('dl');
    dl.className = 'tali-keys-list';
    dl.innerHTML =
      '<div><dt><kbd>?</kbd></dt><dd>Open settings</dd></div>' +
      '<div><dt><kbd>/</kbd></dt><dd>Search</dd></div>' +
      '<div><dt><kbd>f</kbd></dt><dd>Focus mode</dd></div>' +
      '<div><dt><kbd>&larr;</kbd> <kbd>&rarr;</kbd></dt><dd>Previous / next chapter</dd></div>' +
      '<div><dt><kbd>Esc</kbd></dt><dd>Close</dd></div>';

    // WCAG 2.1.4's turn-off mechanism, sitting directly above the list it governs. Same shape as
    // the Focus mode row (03-focus-mode.js): a one-button `.tali-reader-seg` reading On/Off with
    // aria-pressed. It is a sibling of the list, never inside it, so switching shortcuts OFF can
    // never hide the control that switches them back ON.
    var row = document.createElement('div');
    row.className = 'tali-reader-row';
    var label = document.createElement('span');
    label.textContent = 'Shortcuts';
    var seg = document.createElement('div');
    seg.className = 'tali-reader-seg';
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.title = 'Single-key shortcuts (f, ?, /)';
    var syncKeys = function () {
      var on = taliShortcutsOn();
      btn.textContent = on ? 'On' : 'Off';
      btn.setAttribute('aria-pressed', on ? 'true' : 'false');
      dl.hidden = !on; // don't advertise dead keys
    };
    btn.addEventListener('click', function () { taliSetShortcuts(!taliShortcutsOn()); syncKeys(); });
    seg.appendChild(btn);
    row.appendChild(label);
    row.appendChild(seg);

    var box = document.createElement('div');
    box.appendChild(row);
    box.appendChild(dl);
    window.taliReaderMenu.addSection('Keyboard shortcuts', box, syncKeys);
  }
```

- [ ] **Step 3: Gate `?` and `/` (but not the arrows)**

In the same file's keydown handler, add the gate to the `?` branch (`:31-35`), after the modal check:

```js
    if (e.key === '?' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (modal) return; // a modal owns the keys
      if (!taliShortcutsOn()) return;
      if (window.taliReaderMenu) { e.preventDefault(); window.taliReaderMenu.toggle(); }
      return;
    }
```

and to the `/` branch (`:38-41`):

```js
    if (e.key === '/') {
      if (!taliShortcutsOn()) return;
      if (window.taliOpenSearch) { e.preventDefault(); window.taliOpenSearch(); }
      return;
    }
```

Do **not** put the check in the shared early-return above them: the `ArrowLeft`/`ArrowRight` branch below must stay ungated (arrows are not character keys, so WCAG 2.1.4 does not reach them, and chapter nav should keep working).

- [ ] **Step 4: Gate `f`**

In `03-focus-mode.js`, in the keydown handler at `:80`:

```js
    if (e.key === 'f' && !e.metaKey && !e.ctrlKey && !e.altKey && !modal) {
      if (!taliShortcutsOn()) return;
      e.preventDefault();
      setFocus(!on());
    } else if (e.key === 'Escape' && on() && !modal) {
```

Esc stays ungated: it is not a character key, and it is the universal exit from focus mode. The menu toggle button (`:66`) also stays ungated, since it is an explicit click, not a shortcut.

- [ ] **Step 5: Build**

Run: `cargo build -p taliesin-core 2>&1 | tail -5`
Expected: builds clean.

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/js/code-enhance/01-registry.js \
        crates/core/assets/js/code-enhance/07-keyboard.js \
        crates/core/assets/js/code-enhance/03-focus-mode.js
git commit -m "feat(a11y): reader toggle to disable single-key shortcuts (WCAG 2.1.4)"
```

---

### Task 4: Link preview reachable by keyboard

Backlog item: *"Citation/xref link preview is hover-only"* (low).

**Files:**
- Modify: `crates/core/assets/js/code-enhance/12-link-preview.js:140-171` (`show`, `hide`, `forceHide`, listeners)

**Interfaces:**
- Consumes: nothing from other tasks.

- [ ] **Step 1: Add the `aria-describedby` bookkeeping**

The card element's id is `tali-link-preview` (used by the `.closest('#tali-link-preview')` guards at `:167`, `:178`, `:187`). Point the link at it while its card is open, so the card is announced rather than only painted.

In `12-link-preview.js`, add a helper immediately above `hide` (`:155`):

```js
  // The open card describes its link, so a screen reader announces the preview instead of
  // silently painting it. Cleared on every dismissal path and before a different link opens.
  function clearDescribed() {
    if (currentLink) currentLink.removeAttribute('aria-describedby');
  }
```

Replace `hide` and `forceHide` (`:155-156`):

```js
  function hide() { clearTimeout(showTimer); if (pinned) return; clearDescribed(); card.classList.remove('open'); currentLink = null; }
  function forceHide() { pinned = false; card.classList.remove('pinned'); clearTimeout(showTimer); clearDescribed(); card.classList.remove('open'); currentLink = null; }
```

In `show`, replace the `currentLink = link;` line (`:144`) with:

```js
      clearDescribed(); // a previously-previewed link may still point at the card
      currentLink = link;
      link.setAttribute('aria-describedby', 'tali-link-preview');
```

- [ ] **Step 2: Bind `focusin` / `focusout`**

Immediately after the `mouseout` listener (`:171`), add:

```js
  // Keyboard parity with hover: a focused citation/xref link surfaces the same card. Without
  // this the preview is mouse-only. Same eligibility + same delays as the mouse path.
  document.addEventListener('focusin', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = a; scheduleShow(a); }
  });
  document.addEventListener('focusout', function (e) {
    var a = e.target.closest && e.target.closest('a[href]');
    if (a && eligible(a)) { lastHovered = null; scheduleHide(); }
  });
```

Esc dismissal already works through the existing `forceHide` handler at `:191`.

- [ ] **Step 3: Build**

Run: `cargo build -p taliesin-core 2>&1 | tail -5`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/code-enhance/12-link-preview.js
git commit -m "fix(a11y): link preview opens on keyboard focus, describes its link"
```

---

### Task 5: `forced-color-adjust` no longer hides the current nav item

Backlog item: *"`forced-color-adjust: none` hides the current nav item"* (low). The audit cited `site.css:293` + `base.css:780`; both drifted, corrected below.

**Files:**
- Modify: `crates/core/assets/css/base.css:867-868`
- Modify: `crates/core/assets/css/site.css:311-313`

- [ ] **Step 1: Narrow the opt-out in `base.css`**

The rule pins a foreground with no background on the nav item, so an opposite-polarity High Contrast theme can paint it invisible. Only the reader-seg pressed button needs the opt-out, because it pins a **bg+fg pair**. Replace `:867-868`:

```css
    /* Only the pressed seg button opts out: it pins a bg+fg PAIR, so forced colors would
       otherwise drop the pressed state entirely. The nav markers must NOT opt out — they pin a
       fg with no bg, which under an opposite-polarity theme paints "you are here" invisible.
       Their marker survives without it (site.css underlines the active item). */
    .tali-reader-seg button[aria-pressed="true"] { forced-color-adjust: none; }
```

- [ ] **Step 2: Narrow the opt-out in `site.css`**

Replace `:311-313`:

```css
  .tali-nav-active, .tali-book-active, a[aria-current="page"] {
    text-decoration: underline; text-underline-offset: 3px;
  }
```

The `forced-color-adjust: none` is dropped; the underline is what marks the active item, and forced colors preserves it.

- [ ] **Step 3: Build**

Run: `cargo build -p taliesin-core 2>&1 | tail -5`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/css/base.css crates/core/assets/css/site.css
git commit -m "fix(a11y): keep the active nav marker visible under forced colors"
```

---

### Task 6: Settings panel reflows at 200% text

Backlog item: *"Settings panel doesn't reflow at 200% text"*. The content-loss half is already fixed (see the `box-sizing` note at `base.css:85-88`); what remains is horizontal scrolling inside the panel.

**Files:**
- Modify: `crates/core/assets/css/base.css:102-106` and `:254`

- [ ] **Step 1: Let the rows and the seg wrap**

`flex-wrap` rather than an em-based breakpoint: wrapping responds to actual available space, so there is no threshold to tune and it covers full-page zoom and text-only zoom alike. `justify-content: space-between` survives it, because each flex line distributes independently, so an unwrapped row keeps today's exact look and a wrapped one stacks left-aligned.

Replace `base.css:102-106`:

```css
  /* flex-wrap, not a breakpoint: at 200% text a doubled label + a 4-button Theme seg cannot
     share one line, and the panel's `overflow: auto` would turn that into a horizontal scroll
     (WCAG 1.4.4 / 1.4.10). Wrapping keys off real available space, so it needs no tuned
     threshold and handles page zoom and text-only zoom the same way. `space-between` is
     unaffected when everything fits: flex lines distribute independently. */
  .tali-reader-row { display: flex; flex-wrap: wrap; align-items: center;
    justify-content: space-between; gap: .5rem; margin: 0 0 .7rem; }
  .tali-reader-row > span { font-size: .82rem; color: var(--tali-muted); }
  .tali-reader-seg { display: inline-flex; flex-wrap: wrap; border: 1px solid var(--tali-border-strong);
    border-radius: 7px; overflow: hidden; }
```

Replace `base.css:254`:

```css
  .tali-keys-list > div { display: flex; flex-wrap: wrap; gap: .8rem; align-items: baseline; margin: .35rem 0; }
```

- [ ] **Step 2: Build, then check the known cosmetic risk**

Run: `cargo build -p taliesin-core 2>&1 | tail -5`

`.tali-reader-seg` sets `overflow: hidden` + `border-radius: 7px` while its buttons use `border-left` as dividers (`:109-113`). A seg wrapped onto two lines may show a stray leading divider at the start of line 2. Verify this in Task 7 at 200%; if it reads badly, the fix is to suppress the divider on the first button of each line. Do not pre-emptively add CSS for a defect you have not seen.

- [ ] **Step 3: Commit**

```bash
git add crates/core/assets/css/base.css
git commit -m "fix(a11y): settings panel rows reflow instead of h-scrolling at 200%"
```

---

### Task 7: Browser verification sweep

The six fixes share one served site, so they verify in one session rather than six. **Nothing in Tasks 1-6 is proven until this task passes**: five of the six have no automated test, per the spec.

**Files:** none (verification only; fixes go back into the owning task's file + an amended commit).

- [ ] **Step 1: Rebuild the binary, then serve**

The assets are `include_str!`-compiled, so a stale binary silently serves the OLD css/js and you will measure the wrong page:

```bash
cargo build -p taliesin-server 2>&1 | tail -3
./target/debug/taliesin preview corpus/tech-blog 4388
```

- [ ] **Step 2: Run each check via the chrome-devtools MCP**

Drive the browser directly (`mcp__plugin_chrome-devtools-mcp_chrome-devtools__*`). Record the actual observed result for each row, not an expectation:

| # | Check | Pass condition |
|---|-------|----------------|
| 1 | On `/blog.html`, click a category chip | `aria-pressed` flips on the clicked chip and on All; the live region text becomes "Showing N of M posts"; no announcement fired on initial load |
| 2 | Open Settings with the gear, then with `?` | `document.activeElement` is inside `#tali-rmenu-panel` both ways; Esc returns focus to the gear; a click outside still dismisses |
| 3 | Settings → Shortcuts → Off | `f`, `?`, `/` all inert; the `<dl>` shortcut list hides; the Shortcuts row stays visible; the gear still opens the menu; `←`/`→` still navigate chapters |
| 4 | Reload the page | Shortcuts still Off (persisted); toggle back On restores all three keys |
| 5 | Tab to a citation link (a `[@key]` in a post) | The preview card opens on focus; the link gains `aria-describedby="tali-link-preview"`; blur hides it; Esc dismisses |
| 6 | Emulate forced-colors + both polarities | The active nav item stays visible; the pressed reader-seg button stays legible |
| 7 | 200% text | No horizontal scroll inside the settings panel; rows stack; check for the stray seg divider from Task 6 |
| 8 | **Trap regression** (Task 2 touched shared code): open Cmd-K, then open the lightbox | Focus still moves in, Tab still cycles inside, Esc still restores focus to the opener |

- [ ] **Step 3: Run every UI check at three viewports**

390x844 (mobile), 1440x900 (laptop landscape), 900x1440 (the narrow-tall band that is easy to forget).

- [ ] **Step 4: Fix anything found, amending the owning task's commit**

If a check fails, diagnose to root cause before editing (do not paper over a symptom). Amend the fix into the commit that owns that file rather than adding a "fix the fix" commit.

- [ ] **Step 5: Stop the server**

Kill it by PID from the `preview` output. **Do not** run `pkill -f 'taliesin preview'`: it matches and kills the Bash tool's own shell.

---

### Task 8: Docs + backlog

The manual is dogfooded, and Task 2 makes one of its statements factually wrong. Both must land with the code.

**Files:**
- Modify: `docs/guide/using/reading.tmd:87-99` (shortcuts), `:110-113` (the now-false focus claim)
- Modify: `notes/backlog.md` (delete section C entirely)

- [ ] **Step 1: Document the Shortcuts toggle**

In `docs/guide/using/reading.tmd`, replace the shortcut list block at `:91-99`:

```markdown
emitted server-side, so they work even with JavaScript disabled. A handful of keyboard
shortcuts speed up reading (each is ignored while you are typing in a field or a dialog
is open):

- `?` opens (and closes) the Settings menu, whose Keyboard shortcuts section lists these.
- `/` opens the search palette.
- `f` toggles focus mode.
- `←` / `→` move to the previous / next chapter (in a book).
- `Esc` closes the open menu, dialog, or palette.

The three single-character shortcuts (`?`, `/`, `f`) can be switched off: the **Shortcuts**
control sits directly above that list in the Settings menu. Speech-input users in particular
can fire a bare letter key by accident, so WCAG 2.1.4 requires a way to turn character-key
shortcuts off, and this is it. Switching them off hides the list (the keys are dead, so
advertising them would be a lie) and leaves the Settings gear as your way back. `Esc` and the
arrow keys are not character keys and always stay live. The choice persists per reader.

This is not a contradiction of the no-comfort-panel stance above: a text-size slider is a
preference, whereas a shortcut that cannot be turned off is a barrier.
```

- [ ] **Step 2: Correct the focus claim Task 2 invalidated**

Replace `:110-113`:

```markdown
Modal overlays (the search palette and the image viewer) move focus into themselves on
open, trap Tab while open, and restore focus to whatever opened them on close. The
**Settings** menu is a light-dismiss popover: it moves focus to its first control on open and
returns it to the gear when you press Esc, but it never traps Tab, so you can always leave it
by tabbing or by clicking away. (It moves focus because the panel is not adjacent to its gear
in the page's source order, so without the move a keyboard reader would be stranded far from
the menu they just opened.)
```

- [ ] **Step 3: Verify the docs book still builds clean**

```bash
cargo run -p taliesin-server -- build docs/guide --out /tmp/guide-check 2>&1 | tail -5
cargo test -p taliesin-core --test stale_docs 2>&1 | tail -5
```

Expected: build reports no warnings for `using/reading.tmd`; `stale_docs` passes.

- [ ] **Step 4: Close section C in the backlog**

Per the backlog's own rule ("Only open tasks live here — delete items once landed; don't leave `[x]`"), the eight `### C.` bullets go. The two rotted ones must not be silently dropped: record that they were verified already-fixed, in the same parenthetical style the closed sections A and B use.

Replace the eight bullets of the `### C.` block (from "Real a11y bugs (WCAG/APCA/OKLCH/CVD harness evidence)…" through the "Settings panel doesn't reflow at 200% text." bullet) with:

```markdown
*(Section C is closed, 2026-07-16. Six items built: the single-key-shortcut reader toggle
(WCAG 2.1.4, gating `f`/`?`/`/`), settings-popover focus-on-open, category chips'
`aria-pressed` + live count, keyboard-reachable link previews, the forced-colors nav
marker, and settings-panel reflow at 200%. Spec:
[2026-07-16-section-c-a11y-batch-design.md](../docs/superpowers/specs/2026-07-16-section-c-a11y-batch-design.md).
Two items were NOT built: both had rotted, closed by §F's deck theming/a11y step, verified
against source before deletion. "Embedded deck ignores a sepia host" was already fixed at
its own named anchor (`render/deck.rs:164` reads `(t==='sepia' ? 'light' : null)`, the
recommended fix verbatim). "Deck slide-number chip not restyled per-slide" was fixed by
removing the premise: the chip is now one dark-glass surface in both themes
(`deck.css:352-361`), so the `html.tali-deck-dark`-scoped restyle the bug described no
longer exists. See [[backlog-entries-rot]].)*
```

**Keep** the "Owner-calls kept as-is" paragraph that follows the bullets (hairline table cells, `tip`/`important` under protanopia, no deck sepia palette). Those are open owner decisions, not built work.

Then update the State block. Replace the "Sections A, B and F are now closed" paragraph with:

```markdown
**Sections A, B, C and F are now closed.** A (blog identity) finished with #7 draft-aware preview
(2026-07-16). B (publish hardening) was **backlog rot** — all three items were already shipped by
the author; entries deleted with evidence (see the note in section A). C (theme/a11y follow-ups)
finished 2026-07-16: six items built, two more were rot (see the note in section C). F (the deck
audit) is fully landed except the deliberately-deferred B3-18. **→ The next open work is D (needs a
direction ruling), E (own session), and G (priority vs D/E is the owner's call).**
```

Finally, the State block's sync line is about to go stale. Replace:

```markdown
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse
cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). `origin/main == local main ==
a4a96bc` (draft-aware preview merged + pushed; nothing unpushed). **Tier 1 is empty.**
```

with:

```markdown
with DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse
cursor sync, located diagnostics, CSS hot-swap, Cmd-K search). `origin/main == a4a96bc`; local
main is ahead by the section-C a11y batch, pending the author's push. **Tier 1 is empty.**
```

- [ ] **Step 5: Commit**

```bash
git add docs/guide/using/reading.tmd notes/backlog.md
git commit -m "docs: shortcuts toggle + corrected settings-menu focus behavior; close backlog §C"
```

---

### Task 9: Land the branch

- [ ] **Step 1: Full gate**

```bash
cargo test -p taliesin-core 2>&1 | tail -15
cargo fmt --check && cargo clippy -p taliesin-core -p taliesin-server 2>&1 | tail -5
```

Expected: all pass. If a kernel test flakes, re-run it alone before believing it: `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state` and `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` assert on **timing** and fail under CPU load. That is a known open Tier-2 item, not a regression from this branch.

- [ ] **Step 2: Review the whole diff**

```bash
git diff main...HEAD
```

Read it in full. Confirm: no reformatted untouched lines, no em dashes in new prose, no new document config, no `qmd-`-prefixed new storage key.

- [ ] **Step 3: Fast-forward merge to LOCAL main and report**

```bash
git checkout main && git merge --ff-only section-c-a11y && git log --oneline -3
```

**Do not push.** The author pushes. Report the merged SHA and how far ahead of `origin/main` local main now sits.

## Notes for the implementer

- **Trust the symptom, never the cause or the line number.** Every line number here was read from source on 2026-07-16, but the author pushes mid-session. If an anchor does not match, re-find it; if a fix appears to be already present, stop and report it as rot rather than forcing the edit. Two of the original eight items in this section were exactly that.
- **`taliFocusables` is defined in Task 2 and used only there.** Task 3 does not need it.
- **Order matters in `openMenu`** (Task 2, Step 3): unhide, run hooks, *then* focus. Focusing first yields nothing, because the visibility filter rejects a hidden panel's children.
