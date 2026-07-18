# DX8 plan — Cmd-K command palette (search + actions)

Spec: `docs/superpowers/specs/2026-07-19-dx8-command-palette.md`. Branch `dx8-command-palette`.
The load-bearing behavior is browser JS; the primary check is the chrome-devtools loop, backed
by Rust drift pins on the `include_str!`'d assets. Order: globals → palette → pins → browser.

## Step 1 — `window.taliToggleTheme` (`crates/core/src/render/theme.rs`)

Extract the button's inline toggle into a global and reuse it.
- Add `window.taliToggleTheme = function(){ window.taliSetTheme(pref() === "dark" ? "light" : "dark"); };`
  next to `taliWireThemeToggles`.
- Change the button handler (line ~170) from the inline toggle to
  `window.taliToggleTheme(); sync();`.
- Ships on every page via `theme_head` (build + preview), so the palette's theme action is
  always available.

## Step 2 — preview-only globals (`web-client/client.js`)

After `gotoSource` + the `ws` are defined:
- `window.taliRestartKernel = () => { if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "restart_kernel" })); };`
- `window.taliOpenPageSource = () => gotoSource(null, 1);` (gotoSource already no-ops without
  `window.TALIESIN_DOC`).
Both live in `client.js`, which is `include_str!`'d only in `serve/mod.rs` → absent from static
builds, so the palette hides these actions there for free.

## Step 3 — action registry + unified palette (`web-client/search.js`)

- **Registry** (module const): each `{ id, title, keywords, run, available }` where `available`
  is a `() => boolean` capability gate:
  - theme → `available: () => typeof window.taliToggleTheme === "function"`, `run: () => window.taliToggleTheme()`, keywords "theme dark light mode appearance".
  - restart kernel → gate `taliRestartKernel`, keywords "restart kernel jupyter python r rerun".
  - open source → gate `taliOpenPageSource`, keywords "open source editor vscode file".
- `availableActions()` → the registry filtered by `available()`, mapped to action SearchItems
  `{ action: true, run, id, title, tLow: title.toLowerCase(), bLow: keywords, level: 0, body: "" }`
  so `score()` (title+body matcher) works unchanged.
- `render(query)`:
  - `acts = query ? availableActions().filter(a => score(a, terms) > 0) : availableActions();`
    (empty query shows all available actions).
  - keep the existing content `matches` computation, then `matches = acts.concat(content)`.
  - so arrow-nav / `sel` / `markSel` / `aria-activedescendant` need no change.
- `itemEl(item, …)`: if `item.action`, set `sec.textContent = "action"`, add class
  `tali-s-action`, skip the body snippet, and on click call `go(item)` (same path).
- `go(item)`: at the top, `if (item.action) { close(); try { item.run(); } catch (e) {} return; }`.
- Placeholder: "Search or run a command…" (both the innerHTML default and the `open()`
  reassignments for site/single-doc).
- CSS: a `.tali-s-item .tali-s-sec` already styles the tag; add a subtle
  `#tali-search .tali-s-action .tali-s-sec{...}` accent so actions read as distinct (small,
  optional — keep within the existing CSS string).

**Guard already present:** `open()` early-returns for a single doc with no headings
(`!buildIndex().length`). With actions, a single doc with actions but no headings should still
open — change that guard to also open when `availableActions().length` (so a heading-less doc
can still run the theme command). Verify this doesn't regress the "nothing to search" case
(actions are always ≥1 because theme is always available, so the palette now always opens —
acceptable and desirable: Cmd-K is now a command palette, not only search).

## Step 4 — Rust drift pins (`crates/core/src/render/tests.rs`)

JS is `include_str!`'d, so a Rust test guards the wiring ships:
- `SEARCH_JS` contains the action registry marker (e.g. `taliToggleTheme` reference +
  `item.action` branch + `"Search or run a command"`).
- `THEME_HEAD`/theme asset ships `window.taliToggleTheme` (find the right const;
  theme JS is emitted by `theme.rs` — assert against the function that returns it, or a
  `render`-level snapshot that includes it).
- `CLIENT_JS` (server crate) ships `taliRestartKernel` + `taliOpenPageSource` — this pin lives
  in the **server** crate (where `CLIENT_JS` is `include_str!`'d), so add it to a server test
  (e.g. `crates/server/src/serve/mod.rs` tests or an existing client-asset test).

## Step 5 — verify

- `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`, `cargo clippy
  --workspace`, and both JS jsconfig type-checks (search.js + client.js).
- chrome-devtools MCP (three viewports): live preview — empty Cmd-K shows 3 actions; "theme" →
  Enter flips theme; "kernel"/"editor" present; content search intact. Static build — only the
  theme action; it toggles. Console clean; screenshots.

## Step 6 — close

ff-merge to `main`, delete DX8 from `notes/backlog.md`, AUDITS.md note, update dx-audit memory.
Push when asked.
