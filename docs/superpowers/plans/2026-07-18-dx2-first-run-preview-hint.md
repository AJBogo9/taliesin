# DX2 — First-run in-preview hint: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a one-time, dismissible, `localStorage`-gated callout above the `◇</>` dev button in the live preview that surfaces the flagship Alt-click-to-source gesture (and, where live, the `?` shortcuts menu).

**Architecture:** Pure preview chrome. All behavior in the single `web-client/client.js` IIFE (built inside `buildDevMenu()`, mounted into the existing `#tali-controls` host); all styling in the shared server-side `STATUS_CSS` const in `crates/server/src/serve/mod.rs`, which both serve paths already inject. No protocol change, no server routes, no build-path emission.

**Tech Stack:** Vanilla JS (`// @ts-check` via `web-client/jsconfig.json` + `globals.d.ts`), Rust (bin crate `#[cfg(test)]` unit tests), chrome-devtools MCP for the UI loop.

## Global Constraints

- **Preview-only.** `client.js` is never in `build`; the nudge must never appear in built output.
- **No preview write-back.** The nudge only *navigates* (via the existing Alt-click); it must not mutate source (single-editing-surface invariant).
- **`localStorage` key `tali-hint-seen`**, owned `tali-` prefix. Storage failures **fail closed** (treat as seen → never show).
- **`client.js` stays one whole IIFE** — do NOT split the file (JS-modularization decision).
- **No new output format, no CDN, `--tali-*` vars + existing DOM/CSS only.** Block model + `MAX_WARM_PAGES`/`exec_pool.rs` freeze untouched.
- **`cargo fmt`-clean** (a PostToolUse hook runs rustfmt on edited `.rs`; CI enforces it).
- **Reduced motion:** the ease-in animation is gated `@media (prefers-reduced-motion: no-preference)`, matching the existing `STATUS_CSS` pulse rules.

---

### Task 1: Nudge CSS in `STATUS_CSS` + Rust regression pin

**Files:**
- Modify: `crates/server/src/serve/mod.rs` (append to `STATUS_CSS`, ends line ~620; add a `#[cfg(test)]` test near the existing `blog_index_ships_toc_scrollspy_when_toc_enabled` at ~L1653).

**Interfaces:**
- Consumes: `STATUS_CSS` const; `blog_index_html(ctx: &PageCtx) -> String`; `PageCtx` (fields shown in the existing tests at L1660-1674).
- Produces: the CSS class `.tali-hint-nudge` present in `STATUS_CSS` and therefore in every assembled preview page head. Task 2's JS depends on these class names: `.tali-hint-nudge`, `.tali-hint-line`, `.tali-hint-dismiss`.

- [ ] **Step 1: Write the failing test**

Add this test in the `#[cfg(test)] mod tests` block (right after `blog_index_ships_toc_scrollspy_when_toc_enabled`, ~L1682):

```rust
#[test]
fn preview_page_ships_first_run_hint_css_and_mount() {
    // DX2: the first-run nudge is built by client.js into #tali-controls and styled by
    // STATUS_CSS. Pin both on the assembled single-doc preview page so a future edit can't
    // silently drop the style (the nudge would then render unstyled) or the mount host.
    let includes = taliesin_core::render::PageIncludes::default();
    let ctx = PageCtx {
        format: DocFormat::Html,
        toc: false,
        theme_css: "",
        theme_default: "auto",
        theme_is_custom: false,
        doc_path: "/tmp/doc.tmd",
        base_dir: "/tmp",
        includes: &includes,
        body: "<h2 data-block-id=\"b\">S</h2>",
        generation: 0,
    };
    let html = blog_index_html(&ctx);
    assert!(
        html.contains(".tali-hint-nudge"),
        "preview page head must ship the first-run nudge CSS (STATUS_CSS)"
    );
    assert!(
        html.contains("id=\"tali-controls\""),
        "preview page must ship the #tali-controls host the nudge mounts into"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-server preview_page_ships_first_run_hint_css_and_mount`
Expected: FAIL on the first assertion (`.tali-hint-nudge` not yet in `STATUS_CSS`). The `#tali-controls` assertion already passes.

- [ ] **Step 3: Add the CSS block to `STATUS_CSS`**

The const ends (line ~617-620):

```rust
    @media (prefers-reduced-motion: no-preference) { \
      [data-qmd-cell-state=\"running\"] .tali-cell-badge { animation: tali-pulse 1s ease-in-out infinite; } \
      @keyframes tali-pulse { 50% { opacity: .35; } } \
    }";
```

Change the closing `    }";` line so the block continues, inserting the nudge rules before the final `";`:

```rust
    @media (prefers-reduced-motion: no-preference) { \
      [data-qmd-cell-state=\"running\"] .tali-cell-badge { animation: tali-pulse 1s ease-in-out infinite; } \
      @keyframes tali-pulse { 50% { opacity: .35; } } \
    } \
    .tali-hint-nudge { position: absolute; bottom: calc(100% + .45rem); left: 0; max-width: 15rem; \
      display: flex; flex-direction: column; gap: .4rem; padding: .6rem .7rem; \
      font: 12px/1.4 ui-sans-serif, system-ui, sans-serif; \
      background: var(--tali-bg, #fff); color: var(--tali-fg, #111); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 9px; \
      box-shadow: 0 8px 28px rgba(0,0,0,.2); } \
    .tali-hint-nudge[hidden] { display: none; } \
    .tali-hint-nudge::after { content: \"\"; position: absolute; top: 100%; left: 1.1rem; \
      border: 6px solid transparent; border-top-color: var(--tali-bg, #fff); \
      filter: drop-shadow(0 1px 0 var(--tali-border, #e0e0e0)); } \
    .tali-hint-line { display: flex; align-items: baseline; gap: .35rem; color: var(--tali-fg, #111); } \
    .tali-hint-nudge kbd { font: 11px/1 ui-monospace, SFMono-Regular, Menlo, monospace; \
      padding: .1rem .3rem; border: 1px solid var(--tali-border, #d0d0d0); border-radius: 4px; \
      background: var(--tali-code-bg, #f5f5f5); color: var(--tali-fg, #111); } \
    .tali-hint-dismiss { align-self: flex-end; margin-top: .1rem; cursor: pointer; background: none; \
      border: none; padding: .15rem .2rem; color: var(--tali-accent, #4c8dff); \
      font: 600 12px ui-sans-serif, system-ui, sans-serif; } \
    .tali-hint-dismiss:hover { text-decoration: underline; } \
    @media (prefers-reduced-motion: no-preference) { \
      .tali-hint-nudge { animation: tali-hint-in .18s ease-out; } \
      @keyframes tali-hint-in { from { opacity: 0; transform: translateY(4px); } } \
    }";
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p taliesin-server preview_page_ships_first_run_hint_css_and_mount`
Expected: PASS.

- [ ] **Step 5: Mutation-check the test is not vacuous**

Temporarily delete the `.tali-hint-nudge { position: absolute; ...` line from `STATUS_CSS`, re-run the test, confirm it FAILS on the `.tali-hint-nudge` assertion, then restore the line.
Run: `cargo test -p taliesin-server preview_page_ships_first_run_hint_css_and_mount`

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/serve/mod.rs
git commit -m "feat(serve): first-run preview-hint CSS in STATUS_CSS + regression pin (DX2)"
```

---

### Task 2: Build + reveal the nudge, with the "Got it" dismissal

**Files:**
- Modify: `web-client/globals.d.ts` (declare `taliShortcutsOn`).
- Modify: `web-client/client.js` (storage helpers before `buildDevMenu`; build block inside `buildDevMenu` after `host.append(toggle, panel);` at ~L473).

**Interfaces:**
- Consumes: the `#tali-controls` `host`, `toggle`, `panel` locals inside `buildDevMenu` (L363-473); the `.tali-hint-nudge`/`.tali-hint-line`/`.tali-hint-dismiss` CSS from Task 1; `window.taliShortcutsOn?()` (from code-enhance/01-registry.js).
- Produces: IIFE-scoped `hintSeen()`, `markHintSeen()`, and a mutable `dismissHint` (`?() => void`) that Task 3's hooks call. The nudge DOM node with class `tali-hint-nudge`.

- [ ] **Step 1: Declare `taliShortcutsOn` in `globals.d.ts`**

Add inside the `interface Window { … }` block (e.g. after the `taliOpenSearch` line, ~L83):

```ts
  /** Reader preference: are single-key shortcuts (`f`, `?`, `/`) live?
   *  (code-enhance/01-registry.js). The first-run nudge omits the `?` line when this
   *  is present and returns false, matching the "don't advertise dead keys" rule. */
  taliShortcutsOn?: () => boolean;
```

- [ ] **Step 2: Add the storage helpers before `buildDevMenu`**

Immediately before the `// --- preview control bar:` comment / `(function buildDevMenu() {` (~L351-362), inside the IIFE, add:

```js
  // First-run discoverability nudge (preview-only, one-time): a small callout tethered above
  // the ◇</> button naming the flagship Alt-click-to-source gesture (and, where live, the `?`
  // shortcuts menu). Gated by localStorage so it shows once per browser. Storage failures FAIL
  // CLOSED (treat as seen → never show): an un-dismissable nag is worse than a missed hint —
  // the opposite trade-off from taliShortcutsOn's fail-open.
  const HINT_KEY = "tali-hint-seen";
  const hintSeen = () => {
    try { return localStorage.getItem(HINT_KEY) !== null; } catch (e) { return true; }
  };
  const markHintSeen = () => {
    try { localStorage.setItem(HINT_KEY, "1"); } catch (e) {}
  };
  // Set when a nudge is actually built (only when !hintSeen()); Task 3's dismissal hooks
  // (dev-menu open, first Alt-click, Esc) call it. Null when no nudge exists.
  /** @type {?() => void} */
  let dismissHint = null;
```

- [ ] **Step 3: Build + reveal the nudge inside `buildDevMenu`**

Directly after `host.append(toggle, panel);` (L473) and before `setStatus("connecting…");` (L474), add:

```js
    // First-run nudge: only when never dismissed on this browser.
    if (!hintSeen()) {
      const nudge = document.createElement("div");
      nudge.className = "tali-hint-nudge";
      nudge.setAttribute("role", "status");
      nudge.setAttribute("aria-live", "polite");
      nudge.hidden = true;

      const line1 = document.createElement("div");
      line1.className = "tali-hint-line";
      line1.title = "Hold Alt (Option on Mac) and click any block";
      const kbdAlt = document.createElement("kbd");
      kbdAlt.textContent = "Alt";
      const alt1 = document.createElement("span");
      alt1.textContent = "-click any block to open its source";
      line1.append(kbdAlt, alt1);
      nudge.appendChild(line1);

      // `?` opens the reader Settings menu (shortcuts list). It is dead on a deck (the reader
      // menu is `.tali-deck`-skipped) and when a reader has turned shortcuts off. Omit the line
      // there, matching the "don't advertise dead keys" discipline in 07-keyboard.js.
      const shortcutsOn = window.taliShortcutsOn; // local so `strict` narrows the typeof cleanly
      const askLive =
        !document.querySelector(".tali-deck") &&
        (typeof shortcutsOn !== "function" || shortcutsOn());
      if (askLive) {
        const line2 = document.createElement("div");
        line2.className = "tali-hint-line";
        const pre = document.createElement("span");
        pre.textContent = "Press";
        const kbdQ = document.createElement("kbd");
        kbdQ.textContent = "?";
        const post = document.createElement("span");
        post.textContent = "for keyboard shortcuts";
        line2.append(pre, kbdQ, post);
        nudge.appendChild(line2);
      }

      const gotIt = document.createElement("button");
      gotIt.type = "button";
      gotIt.className = "tali-hint-dismiss";
      gotIt.textContent = "Got it";
      nudge.appendChild(gotIt);

      host.appendChild(nudge);

      let dismissed = false;
      dismissHint = () => {
        if (dismissed) return;
        dismissed = true;
        nudge.remove();
        markHintSeen();
      };
      gotIt.addEventListener("click", (e) => { e.stopPropagation(); dismissHint(); });

      // Reveal after first paint (the body is already server-rendered, so there is nothing to
      // wait for); the CSS eases it in unless the reader prefers reduced motion.
      setTimeout(() => { if (!dismissed) nudge.hidden = false; }, 400);
    }
```

- [ ] **Step 4: Type-check the client**

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json`
Expected: clean (no errors). If `taliShortcutsOn` errors, confirm Step 1's declaration was saved.

- [ ] **Step 5: Rebuild the binary (client.js is `include_str!`-compiled)**

Run: `cargo build -p taliesin-server`
Expected: builds clean.

- [ ] **Step 6: Browser verify (single-doc) via chrome-devtools MCP**

Use the `/preview` skill on `corpus/native-tmd.tmd`. In the browser:
1. In the page's devtools console (via MCP `evaluate_script`), run `localStorage.removeItem('tali-hint-seen')` then reload.
2. Screenshot: confirm a callout appears above the `◇</>` button (bottom-left) with **both** lines ("Alt-click any block…" + "Press ? for keyboard shortcuts") and a "Got it" button.
3. Click "Got it" → screenshot: nudge gone.
4. `evaluate_script` returns `localStorage.getItem('tali-hint-seen')` → expect `"1"`.
5. Reload → screenshot: nudge does NOT reappear.
6. Confirm no console errors (MCP `list_console_messages`).

- [ ] **Step 7: Commit**

```bash
git add web-client/client.js web-client/globals.d.ts
git commit -m "feat(client): build + reveal first-run preview hint, Got-it dismiss (DX2)"
```

---

### Task 3: Wire the remaining dismissals (dev-menu open, first Alt-click, Esc)

**Files:**
- Modify: `web-client/client.js` — the `toggle` click handler (L392-396); the Alt-click handler (L1225-1237); add one `keydown` listener near the Alt-click handler.

**Interfaces:**
- Consumes: `dismissHint` (from Task 2); the existing `toggle` click handler; the Alt-click `document.addEventListener("click", …)` at L1225.
- Produces: no new exports; three call sites that invoke `dismissHint?.()`.

- [ ] **Step 1: Dismiss on opening the dev menu**

In the `toggle.addEventListener("click", …)` handler (L392-396), after it flips `panel.hidden`, add a `dismissHint` call. The handler becomes:

```js
    toggle.addEventListener("click", (e) => {
      e.stopPropagation();
      panel.hidden = !panel.hidden;
      toggle.setAttribute("aria-expanded", panel.hidden ? "false" : "true");
      if (!panel.hidden && dismissHint) dismissHint(); // opening the menu = the tools were found
    });
```

Note: `dismissHint` is declared before `buildDevMenu` (Task 2 Step 2), so it is in scope here.

- [ ] **Step 2: Dismiss on the first resolving Alt-click**

In the Alt-click handler (L1225-1237), after the successful resolve — right after `openSource(el);` (L1236) — add:

```js
    openSource(el);
    if (dismissHint) dismissHint(); // they discovered click-to-source; retire the hint
```

(Placed after the early `if (!el) return;`, so only a click that actually resolves to a block dismisses it — a stray Alt-click on empty space does not.)

- [ ] **Step 3: Dismiss on Esc**

Immediately after the Alt-click `document.addEventListener("click", …)` block (after L1237), add:

```js
  // Esc also retires the first-run hint (consistent with the error overlay's Esc-to-dismiss).
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && dismissHint) dismissHint();
  });
```

- [ ] **Step 4: Type-check the client**

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json`
Expected: clean.

- [ ] **Step 5: Rebuild the binary**

Run: `cargo build -p taliesin-server`
Expected: builds clean.

- [ ] **Step 6: Browser verify the three dismissals (single-doc)**

Use `/preview` on `corpus/native-tmd.tmd`. For EACH dismissal, first reset via console `localStorage.removeItem('tali-hint-seen')` + reload so the nudge is showing, then:
1. **Alt-click:** Alt-click a paragraph block → screenshot: nudge gone; `localStorage.getItem('tali-hint-seen')` === `"1"`; the editor-jump still fires (expected — it's the existing gesture).
2. **Dev-menu open:** click `◇</>` → screenshot: nudge gone, panel open; key persisted.
3. **Esc:** press Esc (MCP `press_key`) → screenshot: nudge gone; key persisted.
4. No console errors.

- [ ] **Step 7: Commit**

```bash
git add web-client/client.js
git commit -m "feat(client): retire first-run hint on menu-open / Alt-click / Esc (DX2)"
```

---

### Task 4: Cross-surface verification (site + deck) and build-absence

**Files:** none (verification + fixes only, if the matrix surfaces a defect).

**Interfaces:** consumes the finished feature from Tasks 1-3.

- [ ] **Step 1: Site preview (both lines)**

`/preview` on `corpus/tech-blog`. Reset the key + reload. Confirm the nudge shows above `◇` with **both** lines on a content page, "Got it" dismisses + persists, reload stays quiet, no console errors. Screenshot each state.

- [ ] **Step 2: Deck preview (Alt-click line only)**

`/preview` on `corpus/deck.tmd`. Reset the key + reload. Confirm: the nudge shows with **only** the "Alt-click any block…" line (NO `?` line, since the reader menu is deck-skipped), "Got it" dismisses + persists, no console errors. Screenshot.

- [ ] **Step 3: Viewport matrix**

Repeat Step 1 (site) at the three viewport sizes (MCP `resize_page`): mobile 390×844, laptop 1440×900, portrait 900×1440. Confirm the callout stays anchored above `◇` (bottom-left), does not overflow the viewport, and the tail points at the button. Screenshot each.

- [ ] **Step 4: Build output must NOT contain the nudge**

```bash
cargo run -p taliesin-server -- build corpus/native-tmd.tmd /tmp/dx2-build.html
grep -c "tali-hint-nudge" /tmp/dx2-build.html   # expect 0
grep -c "tali-hint-seen" /tmp/dx2-build.html    # expect 0
rm -f /tmp/dx2-build.html
```
Expected: both `0` (client.js + STATUS_CSS are preview-only; `build` ships neither).

- [ ] **Step 5: Full test + format + lint gate**

```bash
cargo test -p taliesin-core -p taliesin-server
cargo fmt --check
cargo clippy -p taliesin-server --all-targets -- -D warnings
```
Expected: all green.

- [ ] **Step 6: Commit any matrix fixes**

If Steps 1-5 surfaced a defect and you fixed it, commit with a `fix(client|serve): … (DX2)` message. Otherwise nothing to commit here.

---

## Self-Review

**Spec coverage:**
- Callout tethered above `◇` with a tail → Task 1 CSS (`::after` tail, `bottom: calc(100% + .45rem)`).
- Preview-only / never in build → Task 4 Step 4 (grep-absence) + Global Constraints.
- localStorage `tali-hint-seen`, fail-closed → Task 2 Step 2 (`hintSeen`/`markHintSeen`).
- Per-line liveness (`?` omitted on decks / shortcuts-off) → Task 2 Step 3 (`askLive`) + Task 4 Step 2 (deck verify).
- Four dismissals (Got it / menu-open / Alt-click / Esc), all persist → Task 2 Step 3 (Got it) + Task 3 (other three).
- Reveal after first paint, reduced-motion-safe → Task 2 Step 3 (`setTimeout`) + Task 1 CSS (`prefers-reduced-motion: no-preference`).
- A11y (`role=status`, `aria-live=polite`, real `<button>`, `<kbd>`) → Task 2 Step 3.
- Rust regression pin, mutation-checked → Task 1 Steps 1-5.
- Viewport matrix + three surfaces → Task 4.

**Placeholder scan:** none — every step has concrete code/commands.

**Type consistency:** `hintSeen`/`markHintSeen`/`dismissHint`/`HINT_KEY` used consistently across Tasks 2-3; CSS classes `.tali-hint-nudge`/`.tali-hint-line`/`.tali-hint-dismiss` consistent between Task 1 (CSS) and Task 2 (JS); `taliShortcutsOn` declared (Task 2 Step 1) before use (Task 2 Step 3).

## Notes for the implementer

- `client.js` and `STATUS_CSS` are `include_str!`-compiled into the binary, so **rebuild the binary** (`cargo build -p taliesin-server`) after editing either before a build-and-inspect check. A live `/preview` hot-swaps CSS but not the client JS logic, so restart the preview after a `client.js` change.
- Do NOT split `client.js` — it is one deliberate IIFE.
- The dev menu already reddens/ambers its badge; the nudge is independent of that and must not interfere with diagnostics.
