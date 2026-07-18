# DX8 — Cmd-K command palette (search + actions)

**Status:** spec (autonomous; proceeding on documented decisions per the standing "work
autonomously on sensible defaults" instruction, mirroring DX4/DX6).
**Persona:** ✍️ (weekly blogger) + 🎤 (deck speaker), who both "reached for Cmd-K to *do*
things, got search only" (2026-07-18 DX audit row 8).
**Size:** M · [surface] (wires existing capability into the palette; no net-new engine).
**Backlog:** §6 DX audit batch, DX8; detail `notes/2026-07-18-dx-audit.md` row 8.

## Problem

Cmd-K opens a polished palette, but it only *searches content*. Every modern palette
(VS Code, Linear, Notion, Slack) also *runs commands*, so users reach for Cmd-K to act —
toggle theme, restart the kernel, jump to their editor — and hit a dead end. The audit:
"✍️🎤 reached for Cmd-K to do things, got search only."

The capabilities already exist, just not from the palette: theme toggle
(`window.taliSetTheme`, ships on every page via `theme_head`), kernel restart (the dev
menu's `#tali-kernel-ctl` → `ws.send({type:"restart_kernel"})`), and open-in-editor
(`gotoSource(null, 1)`). DX8 surfaces them as palette **actions**.

## Current state (measured, not assumed)

- `web-client/search.js` is a self-contained IIFE (the only Cmd-K client), `include_str!`'d
  as `SEARCH_JS` in `crates/core/src/render/mod.rs` and shipped in **both** static builds and
  live previews (Cmd-K works under `file://`). It builds an `index` of `SearchItem`s, matches
  with `score()`, renders a `<ul role=listbox>`, and `go(item)` navigates.
- Empty query already shows a menu: a book's chapter list (level-0 entries) or a single
  doc's heading outline. **This is the natural home for an always-visible action menu.**
- Theme: `theme.rs` defines `window.taliSetTheme(mode)` + `pref()` (local) and wires
  `[data-qmd-theme-toggle]` buttons with the inline toggle
  `taliSetTheme(pref() === "dark" ? "light" : "dark")`. Ships on every page (build + preview).
  **No `window.taliToggleTheme` global yet.**
- `web-client/client.js` is the **preview** client — `include_str!`'d as `CLIENT_JS` only in
  `crates/server/src/serve/mod.rs`, so it is **absent from static builds**. It owns the `ws`
  (local, line 37), the dev-menu "Restart kernel" button (`ws.send({type:"restart_kernel"})`),
  and `gotoSource(file, line)` (`file=null` ⇒ the previewed doc itself, guarded by
  `window.TALIESIN_DOC`). **No `window.taliRestartKernel` / `window.taliOpenPageSource`
  globals yet.**
- Because `client.js` ships only in preview, a `typeof window.taliX === "function"` gate on an
  action naturally hides the preview-only actions in a published build.

## Resolved decisions (autonomous, documented)

### D1 — Unified list, not a `>`-prefix mode

Actions live in the same result list as content. When the query is empty, the available
actions are shown first (a discoverable menu — the direct answer to "reached for Cmd-K to do
things"). When there is a query, actions whose label/keywords match are shown above content
matches. **Rejected:** a `>` command-mode prefix (VS Code) — it teaches a hidden syntax and
loses the immediacy; a separate keybinding — the whole point is that users already press
Cmd-K.

### D2 — Action set (v1), each self-gating on capability

| Action | Availability gate | Runs |
|---|---|---|
| Toggle light / dark theme | always (`typeof window.taliToggleTheme === "function"`) | `window.taliToggleTheme()` |
| Restart kernel | preview only (`typeof window.taliRestartKernel === "function"`) | `window.taliRestartKernel()` |
| Open source in editor | preview only (`typeof window.taliOpenPageSource === "function"`) | `window.taliOpenPageSource()` |

The gate is capability presence, not environment sniffing: `taliToggleTheme` ships in
`theme.rs` (everywhere), the other two in `client.js` (preview only). A published static site
therefore shows exactly one action (theme); a live preview shows all three. Each action
carries `keywords` (synonyms) so "dark"/"light"/"mode" find the theme action, "restart"
finds the kernel one, "editor"/"vscode" find the source one.

### D3 — Excluded actions (on principle)

- **New post / new draft** — scaffolding a file from the browser is a browser→server *write*
  path. It fights the **single-editing-surface / read-only-preview** invariant (the browser
  is a read-only view; edits flow one way). The in-scope way to scaffold is `taliesin new`
  (CLI) or an editor command. It is also meaningless in a static build (no server). Excluded.
- **Jump to slide** — decks own their chrome and `search.js` deliberately no-ops on
  `.tali-deck`; the deck engine has its own overview/navigation. Cross-page **jump to page**
  already works (a search result on another page navigates). Excluded.

### D4 — Actions reuse existing behavior via three thin globals (no reimplementation)

- `theme.rs`: extract the button's inline toggle into `window.taliToggleTheme()`, and have the
  `[data-qmd-theme-toggle]` handler call it (DRY; the dev-menu button and the palette share one
  path).
- `client.js` (preview): `window.taliRestartKernel = () => ws?.readyState === OPEN &&
  ws.send(...)`; `window.taliOpenPageSource = () => gotoSource(null, 1)`.

Keeping behavior in its owning module means the palette never duplicates theme/kernel/editor
logic; it only *invokes* it.

### D5 — Selection + rendering

An action item is `{ action: true, id, title, keywords, run }` (no `url`/`page`). `go(item)`
branches: an action calls `item.run()` then `close()`; a content item navigates as today.
Actions render with an "action" tag in the right-hand `.tali-s-sec` slot (where content shows
"H2"/chapter) and never show a body snippet. Arrow-key nav, `aria-selected`, and
`aria-activedescendant` already operate over the combined `matches` array unchanged. The input
placeholder becomes "Search or run a command…" to signal the palette does more than search.
Running an action closes the palette (theme flips are visible once the overlay is gone).

## Scope / non-goals

- **In:** the three actions above; the action registry + unified render + run-on-select in
  `search.js`; the three globals; the "Search or run a command…" placeholder; the "action"
  tag styling; Rust drift pins; browser verification (preview + static, three viewports).
- **Out:** new-post/draft + slide-jump (D3); a `>` command mode (D1); fuzzy-matching redesign
  (actions reuse the shipped `score()` over title+keywords); any change to content search
  ranking; deck palette.

## Files

- `crates/core/src/render/theme.rs` — `window.taliToggleTheme`; button handler reuses it.
- `web-client/client.js` — `window.taliRestartKernel`, `window.taliOpenPageSource` (preview).
- `web-client/search.js` — action registry, `availableActions()`, unified `render()`,
  action branch in `go()`/`itemEl()`, placeholder, action CSS class.
- `crates/core/src/render/tests.rs` — drift pins that the three JS assets ship the wiring
  (JS is `include_str!`'d, so a Rust test is the guard).

## Verification

- `cargo test -p taliesin-core` (drift pins) + `cargo test -p taliesin-server`; `cargo fmt
  --check`; `cargo clippy`; `cd web-client && npx -y -p typescript tsc -p jsconfig.json`
  (search.js is type-checked) and the client.js jsconfig.
- chrome-devtools MCP (per the browser-testing rule), at the three viewport sizes
  (~390×844, ~1440×900, ~900×1440):
  - **Live preview** (a doc with a `{python}` cell so the kernel action is meaningful): open
    Cmd-K empty → the three actions show as a menu; type "theme" → the toggle action → Enter
    flips light/dark (verify `documentElement` class / `qmd-theme`); "kernel" and "editor"
    surface their actions; content search still works below.
  - **Static build** (`file://` or `build` output): Cmd-K shows **only** the theme action (the
    preview globals are absent); it toggles theme.
  - Console clean; screenshots at each size.

## Test-first note

The load-bearing behavior is JS in the browser, so the primary regression guard is the
chrome-devtools screenshot/console loop (per the global testing rule), backed by Rust drift
pins on the `include_str!`'d assets. There is no Rust unit surface for palette *behavior*;
the pins assert the wiring ships, and the browser loop asserts it works.
