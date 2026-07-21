# Deck Subtree Re-mount Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On a structural deck edit, re-mount only the changed `<section>` subtrees so every untouched slide keeps its live `{js}`/WebGL/video/input state, instead of the current whole-deck re-mount that destroys all of it.

**Architecture:** Purely client-side. The server keeps detecting structural deck edits and broadcasting a `full_render` unchanged. In `web-client/client.js`, the `full_render` handler gains a deck branch that reconciles incoming `<section>`s against the live ones — keyed by each slide's *content signature* (the in-order join of its descendants' content-hash `data-block-id`s, which is position-independent) — reusing unchanged slides in place (refreshing only their `data-sourcepos`), rebuilding changed/new/title slides, and tearing down removed ones with the existing per-element `teardownJs`.

**Tech Stack:** Vanilla JS (`web-client/client.js`, one big IIFE, `// @ts-check` under strict TypeScript via `web-client/jsconfig.json`). Verification via the CI-gated `tsc` type-check and the chrome-devtools MCP browser loop against `corpus/deck.tmd`.

## Global Constraints

- **No server change.** The fix lives entirely in `web-client/client.js`. Do not touch `crates/server/src/serve/mod.rs`'s `deck_structural` / `is_slide_structural` detection or `full_render_json`.
- **Block-model invariants:** every block keeps its `data-block-id`; preserved sections must have their descendants' `data-sourcepos` (and `data-source-file`) refreshed from the incoming HTML, or Alt-click / reverse cursor-sync go stale.
- **Single editing surface:** read-only preview; only DOM is touched, never the source.
- **Offline:** no CDN, no network; DOM-only APIs.
- **`tsc` strict-clean:** `cd web-client && npx -y -p typescript tsc -p jsconfig.json` must report zero errors (CI-gated).
- **Vertical stacks:** reconcile top-level `<section>`s only (owner ruling 2026-07-21). A stack wrapper is matched as one unit; if any sub-slide inside changed, the whole stack rebuilds.
- **No new headless-browser CI harness** (out of scope for v1). Verification is `tsc` + the chrome-devtools MCP scenario.
- **Do-NOT-touch:** no change to `MAX_WARM_PAGES` / `exec_pool.rs` or the kernel/exec zone.

Spec: `docs/superpowers/specs/2026-07-21-deck-subtree-remount-design.md`.

---

## File Structure

- **Modify** `web-client/client.js`:
  - Add a `reconcileDeckSections(container, bodyHtml)` helper (plus two nested helpers, `signature` and `patchSourcepos`) alongside the other DOM helpers (after `resetJs`, ~line 1019, before `afterChange`). It depends on the existing `fragment`, `keepScroll`, and `teardownJs` helpers (all defined by line 1019).
  - Rewire the `full_render` handler's `!skipMount` branch (~lines 1111-1119) to dispatch to `reconcileDeckSections` for a live deck, falling back to the existing `resetJs()` + `innerHTML` swap otherwise.

No new files. No Rust changes.

---

### Task 1: Client-side deck section reconciliation

**Files:**
- Modify: `web-client/client.js` (add helper ~after line 1019; rewire branch ~lines 1111-1119)
- Test: `cd web-client && npx -y -p typescript tsc -p jsconfig.json` (compile gate) + chrome-devtools MCP browser scenario on `corpus/deck.tmd`

**Interfaces:**
- Consumes (existing helpers in the same IIFE): `fragment(html: string): Element | null`, `keepScroll(fn: () => void): void`, `teardownJs(el: Element | null): void`, and the module-scope `const isDeck` (`web-client/client.js:817`), `root` (the `#tali-root` element).
- Produces: `reconcileDeckSections(container: Element, bodyHtml: string): boolean` — returns `true` when it reconciled the deck in place, `false` when it bailed (nothing recognizable to diff) so the caller falls back to a wholesale re-mount.

- [ ] **Step 1: Reproduce the state-loss bug in the browser (RED)**

Build the binary (the client is `include_str!`-compiled, so a source change needs a rebuild), then preview the corpus deck:

```bash
cd /home/bogo/Documents/personal/taliesin
cargo build -p taliesin-server
cargo run -p taliesin-server -- preview corpus/deck.tmd 4388
```

With the chrome-devtools MCP:
1. Navigate to `http://localhost:4388`.
2. Advance to the **"Ask what if?"** slide (the one with the `rate %` slider + `{js}` output text).
3. Drag the slider to a non-default value (e.g. 15). Confirm the `{js}` text below it recomputes ("at 15% growth, 100 becomes 115.0").
4. In the source `corpus/deck.tmd`, insert a new slide heading *above* that slide (e.g. add `\n## Inserted\n\nBody.\n` before `## Ask "what if?" live`) and save.

Expected (the bug): the whole deck re-mounts, the slider snaps back to **8**, and the `{js}` text resets. Screenshot this as the RED baseline. Revert the source edit before implementing.

- [ ] **Step 2: Add the `reconcileDeckSections` helper**

In `web-client/client.js`, immediately after the `resetJs` definition (the block ending at line 1019, before the `// Re-attach the deck…` comment for `afterChange`), insert:

```js
  // A deck's structural edit (add/remove/reorder/retitle a slide, or an inserted
  // `---`/`. . .`) arrives as a `full_render` carrying the whole slide body. Blowing the
  // deck away wholesale (`root.innerHTML = …`) would tear down every {js}/WebGL/video/
  // input state on EVERY slide, including the untouched ones — the one place a shipping
  // live view still breaks the DOM-state-preservation invariant. Instead, reconcile the
  // incoming <section>s against the live ones: keep an unchanged slide's live node in
  // place (preserving its state, refreshing only its click-to-source position), rebuild a
  // changed/new/title slide, and tear down a removed one. Slides are keyed by their
  // *content signature* — the in-order join of their descendants' content-hash
  // `data-block-id`s. That signature is position-independent, so a slide that only shifted
  // down the file keeps the same signature and is preserved; a within-slide content edit
  // changes a block id (hence the signature) and is rebuilt. Returns false (caller falls
  // back to a wholesale swap) when there is nothing recognizable to diff.
  /** @param {Element} container @param {string} bodyHtml @returns {boolean} */
  const reconcileDeckSections = (container, bodyHtml) => {
    const tpl = document.createElement("template");
    tpl.innerHTML = bodyHtml.trim();
    /** @type {Element[]} */
    const incoming = Array.from(tpl.content.children).filter((n) => n.tagName === "SECTION");
    /** @type {Element[]} */
    const oldSections = Array.from(container.children).filter((n) => n.tagName === "SECTION");
    if (!incoming.length || !oldSections.length) return false;

    // The content signature of a section: its descendants' block ids, in order. Empty for
    // the front-matter title slide (built outside the block model) — such a section is
    // never reused, so a title/subtitle edit always rebuilds it.
    /** @param {Element} sec @returns {string} */
    const signature = (sec) =>
      Array.from(sec.querySelectorAll("[data-block-id]"))
        .map((b) => b.getAttribute("data-block-id"))
        .join("");

    // Copy click-to-source position attrs from an incoming section onto the reused live
    // one, matched by block id (same semantics as the `set_meta` op, per block within the
    // section), so Alt-click / reverse cursor-sync stay exact after a line shift.
    /** @param {Element} live @param {Element} next */
    const patchSourcepos = (live, next) => {
      /** @type {Map<string, Element>} */
      const byId = new Map();
      next.querySelectorAll("[data-block-id]").forEach((b) => {
        const id = b.getAttribute("data-block-id");
        if (id) byId.set(id, b);
      });
      live.querySelectorAll("[data-block-id]").forEach((b) => {
        const id = b.getAttribute("data-block-id");
        const src = id ? byId.get(id) : undefined;
        if (!src) return;
        const sp = src.getAttribute("data-sourcepos");
        if (sp != null) b.setAttribute("data-sourcepos", sp);
        const sf = src.getAttribute("data-source-file");
        if (sf != null) b.setAttribute("data-source-file", sf);
        else b.removeAttribute("data-source-file");
      });
    };

    // Index reusable old sections by signature. A queue per signature consumes duplicate
    // (content-identical) slides positionally — e.g. a repeated auto-animate title.
    /** @type {Map<string, Element[]>} */
    const pool = new Map();
    for (const sec of oldSections) {
      const sig = signature(sec);
      if (!sig) continue; // never reuse an empty-signature (title) section
      const q = pool.get(sig);
      if (q) q.push(sec);
      else pool.set(sig, [sec]);
    }

    // Build the desired ordered child list, reusing live nodes where a slide is unchanged.
    /** @type {Element[]} */
    const next = [];
    /** @type {Set<Element>} */
    const reused = new Set();
    for (const sec of incoming) {
      const sig = signature(sec);
      const q = sig ? pool.get(sig) : undefined;
      const live = q && q.length ? q.shift() : null;
      if (live) {
        patchSourcepos(live, sec);
        reused.add(live);
        next.push(live);
      } else {
        const node = fragment(sec.outerHTML);
        if (node) next.push(node);
      }
    }

    // Tear down the {js}/WebGL cells of every old section we did NOT reuse (per-element,
    // NOT the global resetJs — that would kill preserved cells), releasing their WebGL
    // contexts / RAF loops and unregistering their inputs.
    for (const sec of oldSections) if (!reused.has(sec)) teardownJs(sec);

    // Apply the new order with minimal DOM churn: an unchanged slide never moves (so its
    // playing video / WebGL context is not detached), only inserted/moved/removed nodes do.
    keepScroll(() => {
      let i = 0;
      for (const node of next) {
        if (container.children[i] !== node) container.insertBefore(node, container.children[i] || null);
        i++;
      }
      while (container.children.length > next.length) container.lastElementChild?.remove();
    });
    return true;
  };
```

- [ ] **Step 3: Rewire the `full_render` `!skipMount` branch**

In the `full_render` case, replace the existing branch body (currently `web-client/client.js:1111-1119`):

```js
        if (!skipMount) {
          // Wholesale re-mount (stale SSR / reconnect / structural change): tear down
          // ALL prior `{js}` cells first (resolving every outstanding `invalidation`)
          // so their WebGL contexts + RAF loops are released and the qmd-js runtime is
          // rebuilt fresh, rather than re-pushing duplicate cells onto a never-reset
          // registry.
          resetJs();
          keepScroll(() => { root.innerHTML = msg.body_html; });
        }
```

with:

```js
        if (!skipMount) {
          // For a live deck, reconcile the incoming <section>s against the mounted ones so
          // only the edited slides re-mount — every untouched slide keeps its {js}/WebGL/
          // video/input state (the DOM-state-preservation invariant, extended to decks).
          // Any other case (non-deck, first mount, unrecognizable body) falls back to the
          // wholesale swap: tear down ALL prior `{js}` cells first (resolving every
          // outstanding `invalidation`) so their WebGL contexts + RAF loops are released
          // and the qmd-js runtime is rebuilt fresh, rather than re-pushing duplicate cells
          // onto a never-reset registry.
          const reconciled =
            isDeck &&
            root.querySelector(":scope > section") &&
            reconcileDeckSections(root, msg.body_html);
          if (!reconciled) {
            resetJs();
            keepScroll(() => { root.innerHTML = msg.body_html; });
          }
        }
```

- [ ] **Step 4: Type-check (compile gate)**

Run:

```bash
cd /home/bogo/Documents/personal/taliesin/web-client && npx -y -p typescript tsc -p jsconfig.json
```

Expected: no output, exit 0. Fix any type error before proceeding (common ones: cast a `NodeListOf` via `Array.from`, or guard a possibly-`undefined` map lookup — the code above is written to pass strict, so an error means a transcription slip).

- [ ] **Step 5: Verify the fix in the browser (GREEN)**

Rebuild and preview (the client is compiled into the binary):

```bash
cd /home/bogo/Documents/personal/taliesin
cargo build -p taliesin-server
cargo run -p taliesin-server -- preview corpus/deck.tmd 4388
```

Repeat Step 1's scenario with the chrome-devtools MCP:
1. Advance to "Ask what if?"; drag the slider to 15 (confirm the `{js}` text updates).
2. Insert `## Inserted\n\nBody.\n` above `## Ask "what if?" live` in the source and save.

Expected (fixed): the deck does **not** fully re-mount — the slider stays at **15**, the `{js}` text stays "…becomes 115.0", and the current slide stays current. Screenshot as the GREEN result.

Then verify click-to-source stayed exact on the shifted slide: Alt-click a paragraph on a slide *below* the insertion and confirm the editor jumps to its **new** (shifted-down) source line, not the old one. (If you cannot drive the editor from MCP, instead read the slide's `data-sourcepos` in the DOM and confirm it advanced by the inserted line count.)

- [ ] **Step 6: Commit**

```bash
cd /home/bogo/Documents/personal/taliesin
git add web-client/client.js
git commit -m "fix(deck): re-mount only edited slides on a structural edit (B3-18)

A structural deck edit broadcast a full_render that the client applied as
a wholesale root.innerHTML swap, tearing down every {js}/WebGL/video/input
state on every slide. Reconcile incoming <section>s against the live ones,
keyed by each slide's content-hash block-id signature: reuse unchanged
slides in place (refreshing their data-sourcepos), rebuild changed/new/
title slides, tear down removed ones. Client-only; server unchanged."
```

---

### Task 2: Broaden verification across every structural edit type + viewports

**Files:**
- Modify (only if an edge case fails): `web-client/client.js`
- Test: chrome-devtools MCP on `corpus/deck.tmd` + `cargo test`

**Interfaces:**
- Consumes: `reconcileDeckSections` from Task 1.
- Produces: nothing new; this task confirms correctness across the full matrix and fixes any defect it surfaces.

- [ ] **Step 1: Verify each structural edit type preserves untouched slides**

With the deck previewed (as in Task 1 Step 5) and the slider on "Ask what if?" set to 15, exercise each edit and confirm the slider value survives every time (the "Ask what if?" section is never the one edited):

1. **Add above** — insert a slide heading before it (done in Task 1; re-confirm).
2. **Add below** — insert a slide heading after it.
3. **Remove elsewhere** — delete an unrelated slide heading (e.g. `## Fragments`). Confirm no console error and the slider survives.
4. **Reorder** — move an unrelated slide's block above another. Confirm the slider survives and slides render in the new order.
5. **Retitle elsewhere** — change an unrelated slide's `##` heading text. Confirm the slider survives and the retitled slide's `<section id>` anchor updates (its signature changed → it rebuilt).

Expected: in every case the slider stays at 15 and the browser console is clean (`mcp__…__list_console_messages`).

- [ ] **Step 2: Verify vertical stack + duplicate-title (auto-animate) behavior**

1. **Vertical stack** — `corpus/deck.tmd` has the `# A deeper topic` stack (`## First sub-point`, `## Second sub-point`). Edit one sub-point's body and confirm the deck reconciles without error (per the ruling, the whole stack may rebuild — that is expected; assert only that unrelated top-level slides, e.g. "Ask what if?", keep state).
2. **Duplicate auto-animate title** — the deck has the `## One idea, refined {auto-animate=true}` pair. Make a structural edit elsewhere and confirm both auto-animate slides still render and the auto-animate transition between them still works (the per-signature queue consumed them positionally).

Expected: no console errors; unrelated slide state preserved; auto-animate pair intact.

- [ ] **Step 3: Three-viewport spot check**

Repeat the Task 1 add-above scenario at each viewport (per project convention), confirming the slider survives and layout is intact:
- mobile ~390×844
- laptop landscape ~1440×900
- laptop portrait ~900×1440

Use `mcp__…__resize_page` between runs. Screenshot each.

- [ ] **Step 4: Full regression sweep (no server change, confirm nothing regressed)**

```bash
cd /home/bogo/Documents/personal/taliesin
cargo test -p taliesin-core -- --test-threads=1
cargo test -p taliesin-server -- --test-threads=1
cd web-client && npx -y -p typescript tsc -p jsconfig.json
```

Expected: all green (the server is untouched, so this confirms the corpus renders and the client type-checks). If an `exec` probe test flakes, re-run with `--test-threads=1` before blaming the change (documented flake family).

- [ ] **Step 5: Commit (only if Step 1-3 surfaced a fix)**

If any edge case required a code change:

```bash
cd /home/bogo/Documents/personal/taliesin
git add web-client/client.js
git commit -m "fix(deck): <precise description of the edge-case fix>"
```

If no fix was needed, record that verification passed (no commit) and stop.

---

## Self-Review

**Spec coverage:**
- Client-side reconciliation in the `full_render` handler → Task 1 Steps 2-3. ✓
- Content-signature keying (block-id sequence) → Task 1 Step 2 `signature`. ✓
- Reuse unchanged sections; rebuild changed/new/title; tear down removed → Task 1 Step 2 loop + `teardownJs`. ✓
- Sourcepos patching on preserved sections → Task 1 Step 2 `patchSourcepos` + Task 1 Step 5 verification. ✓
- Fallback for non-deck / first-mount / unrecognizable → Task 1 Step 3 guard + `return false`. ✓
- Vertical stack top-level-only ruling → Task 2 Step 2. ✓
- Duplicate-signature (auto-animate) queue → Task 1 Step 2 `pool` queue + Task 2 Step 2. ✓
- Verification via tsc + chrome-devtools MCP (no new CI harness) → Task 1 Steps 4-5, Task 2. ✓
- No server change → Global Constraints + Task 2 Step 4 regression sweep. ✓

**Placeholder scan:** No TBD/TODO; all code is complete and literal. The only conditional is Task 2 Step 5's commit-if-fix, which is a real branch, not a placeholder. ✓

**Type consistency:** `reconcileDeckSections(container, bodyHtml): boolean` used identically in Task 1 Step 2 (definition) and Step 3 (call site). `signature`/`patchSourcepos` names consistent. `fragment`/`keepScroll`/`teardownJs` match their existing definitions (`web-client/client.js:990/998/1012`). ✓
