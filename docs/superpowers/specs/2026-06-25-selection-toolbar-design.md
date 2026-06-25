# Selection toolbar (copy / quote / text-fragment share) — design

> Status: building (2026-06-25, branch `feat/selection-toolbar`). Synthesized from a 5-lens
> judge-panel design workflow (web-standards / UX / security / YAGNI / a11y). The "share"
> complement to highlights ("keep"): turns the read-only HTML into something a reader can
> quote and deep-link at passage granularity, using the native W3C Text Fragment URL feature
> (no author markup, no server, offline). A web-native capability Quarto's output lacks.

## Goal

On a valid content selection a small toolbar appears above it with four actions, plus the
existing highlight flow:

- **Copy** — the selection's plain text to the clipboard.
- **Quote** — a Markdown blockquote of the selection + an attribution link back to the exact
  passage, to the clipboard.
- **Share link** — a same-origin URL carrying a W3C **Text Fragment** (`#:~:text=…`) that
  deep-links to the exact words; modern browsers scroll-to + flash-highlight on open.
- **Highlight** — unchanged (persist the passage); **Remove highlight** unchanged (on a mark
  click, shown alone).

## Architecture (extend `qmdInitHighlights`, do not add a second enhancer)

A second enhancer would mean a competing selection listener. Instead, **wrap** today's lone
`.qmd-hl-action` button in a toolbar container and add three clipboard-only children:

- `var bar = div.qmd-seltools` (`role="toolbar"`, `aria-label="Selection actions"`, hidden),
  built **once** under the existing `if (!window.__qmdHL)` guard, appended to `<body>`. The
  current `var btn` (Highlight) becomes one child; Copy / Quote / Share link are added before
  it. Children order: Copy, Quote, Share link, Highlight.
- The `mousedown → preventDefault` (keep the selection alive on click) moves to the **bar**
  (delegation), so clicking any child preserves the selection.
- Reuse the existing state machine verbatim: `onSelect`'s success branch (single block,
  not in `.katex`/`pre`/`code`, `e>s`) sets `mode='add'`, `pending={id,s,e,text}` where
  **`text` = the block's highlightable text sliced `[s,e]`** (`textNodes(b1)` joined — the
  same math/code carve-out the highlighter uses), shows all four children. The mark-click
  branch sets `mode='remove'`, hides Copy/Quote/Share, shows only Highlight relabeled
  "Remove highlight". `hideBtn` hides the bar + resets state. Generalize `btn.contains` →
  `bar.contains` and the click-away guard likewise. So **all four actions are single-block
  in v1** (same gate as Highlight) — no relaxed early-returns, no multi-block `sel.toString`.
- `placeBtn(rect)`: un-hide first (so `offsetWidth` is measurable), center on the selection,
  clamp to `[8, innerWidth - width - 8]`; place above, or flip below when `rect.top-38 < 8`.

## Share link — `buildTextFragmentUrl(rawText)` (pure)

1. `text = rawText.replace(/\s+/g,' ').trim()`; if empty → `return null`.
2. Form: `text.length <= 300` → `textStart = text`, no end. Else if `≥12` words → range form
   `textStart = first 6 words`, `textEnd = last 6 words`; else (long, few words) →
   `textStart = first 300 chars cut at a word boundary`, no end.
3. `encTF(s) = encodeURIComponent(s).replace(/-/g,'%2D').replace(/,/g,'%2C').replace(/&/g,'%26')`
   — the three chars are structurally significant in a text directive (`-` marks
   prefix/suffix, `,` separates parts, `&` separates directives); `encodeURIComponent`
   already handles space/`#`/`~`/non-ASCII so the directive can't break out of itself.
4. `directive = 'text=' + encTF(textStart) + (textEnd ? ',' + encTF(textEnd) : '')`.
5. Compose preserving any element id, replacing any prior fragment, exactly one `:~:`:
   `u = new URL(location.href); id = u.hash.replace(/^#/,'').split(':~:')[0]; u.hash='';`
   then **string-concat** `href = u.href + '#' + id + ':~:' + directive` (assigning `u.hash`
   would re-encode `%` → `%25`).
6. `file://`: Text Fragments are spec-ignored there; still build + copy the valid URL, and
   the confirmation note says the highlight activates over http(s). No UA sniffing — an
   unsupported browser lands at the element id / page top (graceful degrade).

No prefix/suffix disambiguation in v1 (needs a duplicate-scan; omission only risks a failed
scroll, never a wrong highlight).

## Quote format (plain strings only; never `innerHTML`)

```
body  = pending.text split on newlines, each line prefixed "> "
url   = buildTextFragmentUrl(pending.text) || location.href
label = (document.title || location.href) with []()\ escaped
md    = body + "\n>\n> -- [" + label + "](" + url + ")"
```
The only injectable spot is the link label (escaped); the URL scheme/host come from
`location` + `encTF` (no open-redirect, no `javascript:`); the body is line-prefixed plain
text. ASCII `--` attribution (house no-em-dash rule; it is in shipped source).

## Clipboard — shared `qmdCopyText(text, onOk, onFail)`

Lift the proven dual-path from `qmdCopyButtons` (lines 64-78) into one module-scope helper:
`navigator.clipboard.writeText` in secure contexts, hidden-`<textarea>` + `execCommand('copy')`
fallback for `file://` / `--host` LAN http, never throws, `onFail` on total failure. Repoint
`qmdCopyButtons` (and the highlight-index export's best-effort copy) at it in the same change
— the existing code-copy button is the regression net. Confirmation = swap the **clicked**
button's `textContent` to a per-action message (stored original label, per-button timer,
~1200ms), bar stays open; Copy/Quote/Share do **not** clear the selection (Highlight/Remove
do, as today). One `aria-live="polite"` announce span.

## Invariants honored

Read-only (Copy/Quote/Share only read DOM text + write the clipboard; never the author
`.qmd`); block model untouched (no element/`data-block-id`/`data-sourcepos` change — only
Highlight wraps a `<mark>` inside a block as today); **no `innerHTML` with selection text**
anywhere (the XSS boundary + the PostToolUse hook); offline (a native URL feature, no
library/CDN); HTML-only (JS + CSS only); idempotent (built once under `__qmdHL`); decks
skipped; the highlight gate + `id:s:e` storage + `qmd:hlchange` contract unchanged.

## Verification

- **Corpus pin:** `corpus/reader/share.qmd` (prose passages), corpus block-invariant test.
- **Rust test** (`render/tests.rs`): the page ships `qmd-seltools` + `buildTextFragmentUrl`.
- **Browser (chrome-devtools MCP, served over http for the fragment):** select a passage →
  the toolbar shows Copy/Quote/Share/Highlight, positioned on-screen; **navigate to the
  generated Share URL and assert scroll-to + a `::target-text` match** (end-to-end, no
  clipboard-read needed); on secure localhost also read the clipboard back and assert exactly
  one `:~:text=`, `encTF` escaping, no `%25`, preserved `#sec-x`; Copy/Quote produce the right
  strings; the `execCommand` fallback path works; a mark click shows only Remove; <380px stays
  single-line; a deck creates no `.qmd-seltools`; the existing code-copy button still works.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`. A post-build adversarial
  review workflow (correctness / security / a11y / invariants / edge cases) before merge.

## Deferred

prefix/suffix disambiguation; multi-block selections; ARIA roving-tabindex keyboard nav;
copy-as-rich-HTML / social targets / citation export / multi-fragment; Esc-to-dismiss; icons.

## Files

`crates/core/assets/js/code-enhance.js` (the `qmdCopyText` helper + the `qmdInitHighlights`
toolbar + `buildTextFragmentUrl`), `crates/core/assets/css/base.css` (`.qmd-seltools`),
`corpus/reader/share.qmd` (pin), a test in `crates/core/src/render/tests.rs`.
