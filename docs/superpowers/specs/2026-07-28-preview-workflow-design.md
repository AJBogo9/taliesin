# The in-editor preview workflow, and the collapsed-callout affordance

**Date:** 2026-07-28
**Status:** design approved, implementation not started.

Two author-reported items, validated against the source and a real browser before any design work.
Both are about the same thing at different scales: **a reader or an author cannot see what the
interface can do.** A collapsed callout does not say it opens; the preview does not say it
navigates, and in one direction it does not navigate at all.

The scope split below is deliberate. **Phase B** is a confirmed rendering defect with a one-rule
fix and ships first. **Phase A1** is the gesture layer, self-contained. **Phase A2** is a
source-resolution redesign that A1 does not depend on, kept separate so a risky change cannot
delay a safe one.

---

## What was validated

The author's two reports, checked rather than assumed. One was confirmed and understated, one was
mistaken in its specifics and correct in its conclusion.

### Item 2, collapsed callouts: confirmed, and broader than reported

A page with collapsed note, warning and caution callouts was built and inspected in Chrome.
Computed styles:

| Selector | `display` | `list-style-type` | `::before` content |
|---|---|---|---|
| `summary.callout-title` | `flex` | `disclosure-closed` | **`none`** |
| `summary.tali-proof-head` | `flex` | `none` | `""`, 7.5px wide |

**Root cause.** A `<summary>` renders the browser's disclosure marker only at `display: list-item`.
`.callout-title` sets `display: flex` (`base.css:450`), so the marker box is never generated. The
computed `list-style-type` is still `disclosure-closed`, which is why the result reads as
suppressed rather than unsupported.

The exclusion was deliberate, not an oversight. `base.css:502-506` draws a themed caret for
`tali-code-fold` and `tali-proof-collapse` and its comment states the excluded case: *"(Callouts
keep their icon header, which is already an affordance.)"* That judgment is what this design
reverses, and the comment must be rewritten with it or the source asserts the opposite of what it
does.

**Broader than reported.** The affordance is missing in *both* states. An open `collapse="false"`
callout is visually identical to a plain non-collapsible callout, so a reader cannot tell that a
visible callout can be folded away either. Any fix must mark both states, which a state-driven CSS
rule does for free.

**Second silent case.** `deck.css:286` sets `list-style: none` on deck callout summaries, so a
collapsed callout on a slide has the same problem by a different route.

### Item 1, the preview workflow: the example is wrong, the conclusion is right

The report says preview to editor is Alt+click while editor to preview is Ctrl+click. **There is no
Ctrl+click editor-to-preview gesture.** The companion contributes six commands and two keybindings
(`Ctrl+Shift+K` open preview, `Ctrl+Alt+M` insert math symbol) and registers no click handler in the
editor. The only Ctrl+click in the editor is VS Code's standard follow-link on an
`{{< include >}}` documentLink (`lsp.rs:576`).

The felt problem is real and sits one level below the reported symptom. **The two directions are not
the same kind of thing:**

- Preview to editor is explicit, deliberate and discoverable: Alt+click, with a hover overlay that
  outlines exactly the block a click would resolve to (`client.js:1516-1577`).
- Editor to preview is implicit and involuntary: every selection change, 80 ms debounce, no
  gesture, no setting, no off switch (`extension.ts:96-106`).

There is no way to ask to be shown, and no way to stop being followed. That asymmetry is the shape
of memory that becomes "and the other way it's some other modifier."

**Two further defects found while validating, neither reported:**

1. **No panel or server reuse.** `openPreview` (`extension.ts:36`) unconditionally allocates a free
   port, spawns `taliesin preview`, and creates a new webview. Invoking it twice on one file yields
   two panels, two servers and two file watchers.
2. **Silent no-op on an unrelated buffer.** The reverse sync fires for any `.tmd` selection change.
   If the buffer is not the previewed document, `relativeKey` returns something like
   `../../other/foo.tmd`, no block matches, and nothing happens with no indication that the preview
   has stopped tracking the cursor.

**Nobody had looked before.** The 2026-07-28 author value-stream audit measured the warm render loop
at 90 to 152 ms and states explicitly that *"the editor half of the loop (companion, LSP,
click-to-source round trip) was not timed"*
(`notes/2026-07-28-author-value-stream-audit.md:173`).

---

## Prior art

Researched rather than guessed, because the author's instinct turned out to be calibrated to a
convention the tool does not follow.

| Tool | Preview to editor | Editor to preview | Panel / server |
|---|---|---|---|
| **LaTeX Workshop** | `Ctrl+click`, or double-click, via `view.pdf.internal.synctex.keybinding` | **`Ctrl+Alt+J`**, an explicit command. Auto exists: `synctex.afterBuild.enabled` defaults **off** | new viewer per `view` call |
| **VS Code Markdown** | double-click (`preview.doubleClickToSwitchToEditor`) plus scroll sync | automatic, with three off-switches (`scrollPreviewWithEditor`, `scrollEditorWithPreview`, `scrollPreviewWithEditorSelection`) | `Ctrl+K V`, reused |
| **Tinymist (Typst)** | plain click | `preview.scrollSync`: `never` / **`onSelectionChangeByMouse`** (default) / `onSelectionChange`; `preview.cursorIndicator` (experimental) | optional persistent background server on fixed port 23635 |
| **Quarto** | none | `Ctrl+Shift+K`; `render.renderOnSave` defaults **off** | reused |
| **Vue DevTools / vite-plugin-inspector** | hold `Ctrl+Shift`, overlay outlines the element, click opens the editor, `Esc` exits | n/a | n/a |
| **Taliesin today** | `Alt+click` with a hold-to-arm overlay | involuntary, every selection change, unconfigurable | new server and panel every invocation |

Five conclusions:

1. **Everyone else uses SyncTeX's vocabulary:** *forward search* (source to view) and *inverse
   search* (view to source). Taliesin has an unnamed inverse search and no forward search. Naming
   the pair is most of the conceptual fix.
2. **The author's "Ctrl+click" is LaTeX Workshop.** It is the most-used preview extension in VS
   Code and its inverse search is literally Ctrl+click. The instinct is calibrated; the tool
   diverges. Alt+click additionally collides with VS Code's own insert-cursor and, under GNOME,
   with window dragging.
3. **No comparable tool makes the editor-to-preview direction involuntary.** LaTeX Workshop and
   Quarto default their automatic behaviour to off. Tinymist's default deliberately excludes
   *keyboard* cursor movement so typing does not drag the view around. Taliesin fires on every
   arrow key with no off switch, which is strictly more aggressive than every tool surveyed. A
   Typst forum thread records a user asking for a modifier precisely because the unconditional
   version became annoying.
4. **The part Taliesin already has is the modern best practice.** Hold-to-arm plus an overlay that
   outlines the resolved target is the Vue DevTools inspector pattern and is better than LaTeX
   Workshop's bare Ctrl+click. The weakness is the modifier choice, the absence of a counterpart,
   and that nothing announces the gesture exists.
5. **Hidden affordances need a signifier and must never be the only path.** Alt+click currently has
   a signifier only after you already know to hold Alt.

Sources: [LaTeX Workshop View wiki](https://github.com/James-Yu/LaTeX-Workshop/wiki/View),
[VS Code Markdown docs](https://code.visualstudio.com/docs/languages/markdown),
[Tinymist preview](https://myriad-dreamin.github.io/tinymist/feature/preview.html),
[Tinymist VS Code config](https://myriad-dreamin.github.io/tinymist/config/vscode.html),
[Quarto VS Code](https://quarto.org/docs/tools/vscode/index.html),
[Vue DevTools Vite plugin](https://devtools.vuejs.org/guide/vite-plugin),
[Typst forum on the jump key](https://forum.typst.app/t/how-to-set-the-synctex-jump-key-to-ctrl-left-click/3733),
[Tanimoto on liveness](https://liveprogramming.github.io/2013/papers/liveness.pdf).

---

## Decisions

Four, all made by the author against the prior art above.

| # | Decision | Rationale |
|---|---|---|
| D1 | **Inverse search becomes Ctrl+click (Cmd+click on macOS). Alt is retired, not aliased.** | Matches the dominant convention and the author's own instinct. A clean break keeps one gesture to document, test and teach. |
| D2 | **Highlight always, scroll only when asked.** The preview marks the cursor's block continuously and never moves the page on its own; a forward-search keybinding scrolls it. | Separates "where am I" from "take me there", makes the two directions symmetric, and needs **no setting**, satisfying the minimal-config convention where Tinymist's three-way knob would not. |
| D3 | **One preview, site-aware when the document belongs to a project.** | Authoring a book in VS Code currently shows an orphan page with no nav and dead cross-page links. One command and one mental model. |
| D4 | **The chevron is trailing and right-aligned**, pointing right when closed and rotating down when open. | The left slot is owned by the kind icon, so a leading caret would cluster two glyphs. Mirrors `.tali-book-expand`, which is the arrow the author named. |

### Non-goals

- **No write-back.** Every gesture here navigates. Nothing edits the source. The `.tmd` remains the
  single editing surface and the preview remains a read-only view.
- **No new output format.**
- **No new settings.** D2 was chosen specifically so the default is right rather than configurable.
  If a knob later proves necessary, that is a separate decision with its own evidence.
- **No change to warm-page eviction** (`MAX_WARM_PAGES` and the LRU order in
  `serve_site/exec_pool.rs`), the project's one standing freeze.

---

## Phase B: the disclosure chevron

Ships first. Independent of everything else.

### Changes

**The callout caret must be `::after`, not `::before`.** The existing carets for `tali-code-fold`
and `tali-proof-collapse` lead their text, so `::before` is correct there. A trailing chevron cannot
reuse it: in a flex container `::before` is the *first* item, and `margin-left: auto` on the first
item pushes every following item right too, which would right-align the icon and the title as well.
`::after` is the last flex item, so `margin-left: auto` on it consumes the free space to its left
and pins the caret to the right edge with the title still left-aligned. The callout therefore gets
its own small rule mirroring the `::before` geometry rather than being appended to the existing
selector lists.

1. **`crates/core/assets/css/base.css`.** For
   `.callout-collapse > details > summary.callout-title`:
   - `list-style: none` only. Do **not** append the selector to the existing rule at 507-509: that
     rule also sets `gap: .45em`, which would silently override the callout title's own
     `gap: .45rem` (`base.css:450`) and shift the icon-to-text spacing on every collapsible callout.
   - `::-webkit-details-marker { display: none }` for Safari.
   - `::after` carrying the same geometry as the `::before` caret at 512-516 (`.42em` box,
     `2px currentColor` top and right borders, `rotate(45deg)`, `transition: transform var(--tali-dur) ease`,
     `opacity: .75`) plus `margin-left: auto`.
   - `details[open] > summary.callout-title::after { transform: rotate(135deg) }`.
2. **Rewrite the comment at `base.css:502-506`.** It currently states the excluded case as settled
   design. It must record the reversal and why: the icon header identifies the *kind*, it never
   signalled *disclosure*, and with `display: flex` suppressing the native marker there was no
   indicator at all in either state. It should also record why this caret trails while the other two
   lead.
3. **`crates/core/assets/css/deck.css:286`.** Same caret for deck callout summaries.

No change to emission: `divs.rs:573` already produces the `<details>`/`<summary>` structure the rule
needs. The only Rust touched is the new assertion described below.

### Verification, and the trap it must avoid

**The assertion must target `BASE_CSS` directly, never a rendered page.** Every Taliesin page
inlines the whole stylesheet, so a page-level `contains(".callout-collapse")` passes on a page
containing no callouts whatsoever. That is a known vacuous-test class in this repo and it is
exactly the shape this change invites.

Real verification is the browser: rebuild, re-render the collapsed/open/plain comparison page, and
confirm by screenshot and computed style that the caret has non-`none` content in both states and
that the rotation differs between closed and open.

**Already prototyped.** The exact rule above was injected into a built comparison page in Chrome on
2026-07-28 and behaves as specified: the three collapsed callouts gain a right-aligned caret, the
open `collapse="false"` callout gains a down caret that distinguishes it from the plain
non-collapsible callout below it, and the icon stays 14px from the summary's left edge, confirming
the title is not dragged right. Computed `::after` transforms were `rotate(45deg)` closed and
`rotate(135deg)` open. Implementation is transcription, not discovery.

---

## Phase A1: the gesture layer

### A1.1 Vocabulary

Documentation and code comments adopt **inverse search** (preview to editor) and **forward search**
(editor to preview). This is not cosmetic: the pair is currently hard to name, which is why it is
hard to remember.

### A1.2 Inverse search: Ctrl/Cmd+click

- `client.js:1517`: `if (!e.altKey) return;` becomes a check for `e.ctrlKey || e.metaKey`.
- The hold-to-arm overlay (`client.js:1537-1577`) arms on `Control` or `Meta` keydown and disarms on
  keyup, `blur` and `visibilitychange`, which the existing code already handles correctly.
- **macOS.** Ctrl+click is the secondary click there, so accepting both modifiers means a Mac user
  pressing Ctrl gets a context menu on top of the jump. A `contextmenu` listener calls
  `preventDefault()` while the overlay is armed. Cmd is the documented Mac gesture; Ctrl continues
  to work without producing a menu.
- **Rename `tali-alt` to `tali-srcnav`.** A class named after a retired modifier is precisely the
  drift `crates/core/tests/retired_names.rs` exists to prevent. Sites: `client.js` (3),
  `base.css` (2 plus its comment), `deck.css:1078`, `tools/ui-audit/lib/probe.mjs:177`. Add
  `tali-alt` to the retired-names test so it cannot return.

### A1.3 Forward search: highlight always, scroll on command

- **Protocol.** `tali-cursor` gains an intent flag: `{type, file, line, reveal: boolean}`.
- **`client.js`.** `highlightAtLine` always applies `.tali-hl`; it calls `scrollIntoView` only when
  `reveal` is true. The deck branch is gated identically, because `TaliesinDeck.slide()` is the
  deck's equivalent of scrolling and is just as disruptive.
- **`extension.ts`.** The debounced selection listener keeps firing and sends `reveal: false`.
- **New command `taliesin.revealInPreview`**, keybinding `ctrl+alt+j` / `cmd+alt+j` (LaTeX
  Workshop's forward-search key), `when: editorLangId == taliesin && editorTextFocus`, plus a
  command-palette entry.

This works because the marking half is already solved: `.tali-hl` is a persistent solid accent
outline (`base.css:718`) whose comment already reserves it for cursor sync. Only the scrolling was
conflated with it.

### A1.4 Panel and server lifecycle

`openPreview` keeps a `Map<docPath, {panel, server}>`. An existing entry means `panel.reveal()`;
disposal clears the entry. Both existing disposal paths (`panel.onDidDispose` and the extension
subscription that survives a window close) must clear it, since `dispose()` is idempotent and both
may fire.

### A1.5 Discoverability

A hidden gesture needs a signifier and must not be the only path.

- The dev-panel hint (`client.js:429`, currently the literal `"Alt-click a block"`) becomes
  platform-aware: "Ctrl-click a block" or "Cmd-click a block".
- The walkthrough teaches forward and inverse search as a pair rather than teaching one gesture.
- The Cmd-K palette carries both actions by name, so neither direction is reachable only by gesture.

### A1.6 No behavioural conflicts (checked, not assumed)

Before committing to Ctrl/Cmd, every modifier-sensitive handler in the bundled JS was checked for a
collision. **There are none**, because each "plain click only" guard already treats ctrl and meta as
a modified click alongside alt:

| Site | Guard | Effect of the change |
|---|---|---|
| `code-enhance/11-lightbox.js:158` | `!altKey && !ctrlKey && !metaKey && !shiftKey` | already excludes ctrl/meta, so an inverse search will not also open the lightbox |
| `code-enhance/07-keyboard.js:62,68` | bails when any of meta/ctrl/alt is held | shortcuts stay inert under the new modifier |
| `deck.js:1696` | bails on `defaultPrevented \|\| metaKey \|\| ctrlKey \|\| altKey` | deck click navigation stays inert |
| `deck.js:1084,1094` | `ctrlKey` on **wheel** events, for trackpad pinch detection | different event type, unaffected |
| `search.js:1057` | Cmd/Ctrl+**K** keydown | different event type, unaffected |

`code-enhance/12-link-preview.js` and `crates/core/src/diff.rs` mention Alt-click in comments only.

### A1.7 Documentation sweep

The clean break's real cost, stated so it is not discovered mid-implementation. Counted with
`rg -c -i 'alt-click|alt\+click|option-click|altKey'`, excluding `notes/` (historical records, left
as written) and vendored `d3.min.js`.

**User-facing prose:** `README.md` (3), `editor/vscode/README.md` (3),
`editor/vscode/walkthroughs/preview.md`, `editor/vscode/package.json` (walkthrough description),
`web-client/README.md`, `site/features.tmd`, `site/demo.tmd`, `docs/guide/using/preview.tmd` (5),
`docs/guide/tour.tmd`, `docs/guide/demo.tmd`, `docs/guide/index.tmd`,
`docs/guide/using/formats.tmd`, `docs/guide/using/getting-started.tmd`,
`docs/internals/protocol.tmd` (3), `docs/internals/client.tmd` (3),
`docs/internals/block-model.tmd`, `docs/internals/validation.tmd`,
`docs/internals/repository.tmd`, `CLAUDE.md`.

**Code comments and test names:** `crates/core/tests/corpus.rs` (4), `crates/core/src/diff.rs` (2),
`crates/core/src/render/emit.rs`, `crates/core/src/render/model.rs`,
`crates/core/src/render/tests.rs`, `crates/server/src/serve_site/mod.rs`,
`crates/core/assets/js/code-enhance/12-link-preview.js` (2),
`tools/live-edit-bench/tests/regression.rs`.

Two of these are easy to miss and both are user-facing: the **root `README.md`** and the
**marketing site** (`site/features.tmd`, `site/demo.tmd`). Mechanical work, but it is the bulk of
the diff and it is the price of retiring the alias rather than keeping it.

---

## Phase A2: site-aware preview

Separate from A1 because it is a source-resolution redesign, not a wiring change. A1 must not wait
on it.

### Resolution rule

Walk up from the `.tmd` to the nearest `_site.yml`. This mirrors the single-document include-root
rule already in the codebase, which resolves to the nearest `_site.yml` and explicitly **not** to
`.git`. Found means the project directory; spawn `taliesin preview <projectDir>`. Not found means
today's single-document path, unchanged.

### File to page URL

Already solved in Rust. `taliesin map <dir> --format json` emits per-page
`{"rel": "index.tmd", "url": "index.html", ...}`, verified against `docs/guide`. The companion reads
that JSON and never reimplements the mapping in TypeScript, matching the companion's standing rule
that editor knowledge lives in Rust. The spawn must use inline literal args so the existing manifest
gate can statically check the subcommand against `main.rs`'s `COMMANDS`.

### The risk, named

Once the webview can navigate between pages, **`docPath` goes stale.** `resolveSourceFile`
(`paths.ts:39`) resolves a `tali-goto` against the previewed document's directory, so an inverse
search on page B would resolve against page A's directory and open the wrong file. `relativeKey`
has the mirror-image problem on the cursor side.

Resolution must key off the project root rather than a remembered file. `serve_site` already emits
`root` in `TALIESIN_DOC` (`serve_site/mod.rs:788`) and already handles `click_block`
(`serve_site/mod.rs:1034`), so the data exists and the wire protocol does not change shape. What
changes is which anchor the companion resolves against.

---

## Verification

- **`./tools/gates.sh`.** The only harness that refuses to be green when a gate skipped silently.
  Both `tsc` checks and `cargo test -p taliesin-core` are inside it.
- **Companion e2e** (`npm run test:e2e`), extended to cover panel reuse. It runs headless in this
  environment despite the README's claim to the contrary.
- **`tools/ui-audit/lib/probe.mjs:177`** asserts the armed class by name and must be updated with
  the rename, or it will pass vacuously against a class that no longer exists.
- **Browser check** for Phase B, per the trap above.

### Coverage gap, stated rather than implied

**Click-to-source has no automated end-to-end coverage.** The harness stops at the relay. Every
automated gate above can pass while the changed modifier fails in real VS Code, so D1 requires a
manual check in a real editor: hold the modifier, confirm the overlay arms, click, confirm the
cursor lands on the right line, and confirm no context menu appears on macOS. A green suite must not
be reported as covering this.
