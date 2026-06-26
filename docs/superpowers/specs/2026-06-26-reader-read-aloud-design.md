# Read-aloud study mode (design)

Date: 2026-06-26
Status: approved (brainstorm), pre-implementation
Feature branch: `feat/reader-read-aloud`
Pillar / cluster: reader-experience cluster (FEATURE-IDEAS.md #9 + moonshot 3,
"listen-and-follow study mode"). BEYOND-QUARTO.md "big bet, opens a new loved-feature
category".

## Summary

A reader-side, read-only "Listen" feature. The reader presses **Listen** in the reader
menu; the built page is read aloud with the Web Speech API, **block by block from the
block currently in view**. Prose is spoken **sentence by sentence** with the current
sentence highlighted and auto-scrolled into view; code blocks are **announced and then
visually line-stepped** (no code text spoken); figures, display equations, and tables are
**announced by label/number**. A floating mini-player (play/pause, prev/next block, speed,
stop) controls playback without reopening the menu.

It is a major accessibility win (a math/code-heavy document becomes listenable) and is
**block-model-native**: the playlist, the exact sentence ranges, the code line-step, and
the figure/equation/table numbering all derive from structure qmd-fast already emits. It
runs **entirely client-side and offline** (Web Speech is a browser API, zero bundle, zero
network) and changes **no Rust / no render output**.

## Goals

- Read the built page aloud, in document order, starting from the topmost block in view.
- Prose: speak one sentence per utterance, highlight that exact sentence, auto-scroll it
  to centre.
- Code: speak a short label, then walk the highlighted lines (`.qhl-ln`) at the listening
  pace without speaking the code text.
- Figures / display math / tables: speak a short labelled announcement (with the existing
  number where one exists), skip the body.
- A floating mini-player: play/pause, previous/next **block**, speed, stop. Plus a
  reader-menu "Listen" section with Play, a speed control, and a voice picker.
- Fully accessible controls (the feature is itself an a11y feature): `aria-live` status,
  labelled buttons, keyboard-operable.
- Respect `prefers-reduced-motion` (no smooth scroll / no slide-in animation).

## Non-goals (v1, YAGNI)

- **No persisted listening position.** The existing block-anchored *reading* resume pill
  already answers "where was I". A listen cursor would duplicate it; ephemeral for v1.
- **No word-level karaoke highlight.** Sentence granularity only. (Web Speech `boundary`
  events are patchy and not needed for the study-mode experience.)
- **No speaking of code text**, table cells, or raw inline-math MathML (it is not English).
- **No Rust / render / block-model / diff / sourcepos change.** Pure client enhancer.
- No new keyboard global shortcuts beyond what the mini-player buttons provide (a
  dedicated keyboard-reader is a separate FEATURE-IDEAS.md item, #55).

## Invariants honoured

- **HTML-only output** — no new output target; a browser API used at read time.
- **Single editing surface** — read-only; never writes the author `.qmd`; no preview
  write-back. Navigation/scroll only.
- **Block model** — never reads or mutates `data-block-id` / `data-sourcepos`; never
  changes the diff. Highlight uses the CSS Custom Highlight API (no DOM mutation) with a
  transient-`<mark>` fallback that is fully unwound on advance/stop.
- **Do-NOT-touch machinery** — `divs.rs`, `cite.rs`, `includes.rs`, numbering scanners,
  exec/freeze/kernel: all untouched.
- **Offline / self-contained** — Web Speech API only; no vendored bundle, no CDN.
- **Deck-skipped** — a `.qmd-deck` page has its own chrome; the enhancer no-ops there.

## Where it lives

- `crates/core/assets/js/code-enhance.js`: new `qmdInitReadAloud()` enhancer, registered
  through `window.qmdEnhancers.register(...)` alongside the other reader enhancers.
  Idempotent (`if (window.__qmdReadAloud) return;`), deck-skip guard, and a graceful
  no-op (hide the UI) when `window.speechSynthesis` is unavailable.
- `crates/core/assets/css/base.css`: `::highlight(qmd-readaloud)` style, the `.qmd-ra-*`
  mini-player styling (dark/sepia aware via `--qmd-*` tokens), the transient-`<mark>`
  fallback style, and reduced-motion handling.
- `corpus/reader/read-aloud.qmd`: the pin document.
- No server-side / Rust changes.

## Architecture

### 1. The playlist (block walk → steps)

Reuse the top-level block filter already used by reading-progress:

```js
// a [data-block-id] not nested inside another block
function contentBlocks() { ... }   // same predicate as qmdInitReadingProgress
```

The playlist is the ordered list of **steps** produced by compiling each block from the
**start block** (the first block whose top is at/below the viewport top) through the last
block. Step kinds:

| Block / element                         | Step(s) produced                                                                 |
|-----------------------------------------|----------------------------------------------------------------------------------|
| Prose (`p`, `li`, `blockquote`, `dd`)   | one step **per sentence**: `{kind:'say', text, range}` (Range over the sentence)  |
| Heading (`h1`–`h6`)                     | one `say` step, Range = whole heading                                             |
| Code (`pre` containing `.qhl-ln`)       | one `say` announce step ("Code block, N lines[, <lang>]") + silent line-step ticks |
| Figure / image (`figure`, bare `img`)   | one `say` step: "Figure N. <caption or alt>" (N from the rendered figure number)  |
| Display math (`.katex-display`)         | one `say` step: "Equation[ N]." (N if the block carries an equation number)       |
| Table (`table` / `.qmd-listing` w/ cap) | one `say` step: "Table N. <caption>." (cells skipped)                             |
| Anything else (e.g. `hr`, empty)        | skipped                                                                           |

Spoken prose text is built by walking the block's text nodes and **skipping `.katex`,
`pre`, `code`** (the exact `skip(node, block)` predicate the highlights feature uses), so
KaTeX's duplicated MathML text and code syntax spans never enter the spoken stream. Inline
math is therefore silently omitted from the sentence; the surrounding prose still reads.

Sentence segmentation: `Intl.Segmenter(lang, {granularity:'sentence'})` when available
(gives correct ranges incl. abbreviations), else a fallback that splits on `.!?` followed
by whitespace + capital/end. Each sentence yields a DOM `Range` (start container/offset →
end container/offset) computed against the block's text nodes, so the highlight is exact.

A block whose compiled spoken text is empty (e.g. a figure with no caption/alt) still
produces its labelled announce step; a prose block that is whitespace-only is skipped.

### 2. The driver (state machine)

```
state: { steps, idx, playing, paused, rate, voice }
play()        -> speakStep(steps[idx])
onStepDone()  -> idx++; if more: focus + play; else stop()
pause()       -> speechSynthesis.cancel(); paused = true   (idx kept)
resume()      -> speakStep(steps[idx])                     (re-speaks current)
stop()        -> cancel; clear highlight + code focus; hide mini-player; idx = 0
prevBlock()/nextBlock() -> idx = first step of adjacent block; if playing, play()
setRate(r)    -> rate = r; if playing, re-speak current step at new rate
```

`speakStep(step, onDone)`:
- `say` step → build a `SpeechSynthesisUtterance(step.text)`, set `rate`, `voice`,
  `onend = onDone`, `onerror = onDone` (so a failed utterance advances rather than wedges),
  then `window.__qmdSpeakImpl(utterance)`.
- silent code line-step → no utterance; a `setTimeout` of `BASE_TICK_MS / rate` advances
  `.qhl-ln-hl` to the next line; when lines are exhausted, clear `.qhl-lines-active` and
  call `onDone`.

**Headless testability seam:** `window.__qmdSpeakImpl` defaults to
`function (u) { speechSynthesis.speak(u); }`. A test (or headless Chrome, which has no
voices) overrides it to invoke `u.onend()` on a microtask/short timer, driving the whole
playlist deterministically so the highlight-walk, autoscroll, code line-step, and controls
can be asserted without real TTS audio.

Per-step focus: clear the previous highlight, set the new one, and
`el.scrollIntoView({ block: 'center', behavior: reducedMotion ? 'auto' : 'smooth' })`
where `reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches`.

Incremental-swap safety: the enhancer re-runs idempotently after a live block swap; if the
block currently being read is replaced by the diff, the driver detects the missing node on
the next step and stops gracefully (it never holds a stale node across a swap because it
re-resolves the element per step).

### 3. Highlight mechanism

Primary: the **CSS Custom Highlight API**. A module-level `Highlight` is registered once
(`CSS.highlights.set('qmd-readaloud', hl)`); each `say` step does `hl.clear();
hl.add(step.range)`. Styled in base.css:

```css
::highlight(qmd-readaloud) { background: var(--qmd-mark, #fde68a); color: inherit; }
```

No DOM mutation, no offset corruption, no interaction with the reader's own highlights,
and it auto-clears on stop (`CSS.highlights.delete('qmd-readaloud')`).

Fallback (when `CSS.highlights` is unsupported): wrap the step's Range in a transient
`<mark class="qmd-ra-mark">` via `range.surroundContents` (or a split-safe equivalent) and
**unwrap it on advance/stop** so the DOM is always restored. The fallback is best-effort;
the primary path is the supported one for the chrome-devtools-verified target.

Code line-step always uses the existing `.qhl-ln-hl` / `.qhl-lines-active` class contract
(class toggling on existing spans, exactly as walkthrough.js does), never the highlight
API.

### 4. UI

**Reader-menu "Listen" section** (`window.qmdReaderMenu.addSection('Listen', node, …)`):
- a **Play** button (starts from the block in view, closes the menu, shows the mini-player);
- a **speed** segmented control (reuse `seg()`): 0.8× / 1× / 1.25× / 1.5× / 2×;
- a **voice** `<select>` populated from `speechSynthesis.getVoices()` (re-populated on the
  `voiceschanged` event); empty/default if none.

The whole section is hidden when `speechSynthesis` is unavailable.

**Floating mini-player** (`<div class="qmd-ra-bar" role="group" aria-label="Read aloud">`,
fixed bottom-centre): prev-block · play/pause (toggles) · next-block · speed readout · stop
(×). Appears on play, hides on stop. Dark/sepia aware via `--qmd-*`. Slide-in only when
reduced-motion is off. A visually-hidden `aria-live="polite"` node announces
"Playing" / "Paused" / "Finished".

### 5. Persistence

`localStorage`, reader-owned. **Global** keys (voice/speed are reader preferences, not
per-page):
- `qmd-ra-rate` → the chosen rate (default "1").
- `qmd-ra-voice` → the chosen voice `name` (default unset → browser default).

No per-page listening-position key (ephemeral, by design).

## Corpus pin

`corpus/reader/read-aloud.qmd`, exercising every step kind:
- multi-sentence prose paragraphs (incl. an abbreviation like "e.g." to stress the
  segmenter), and a list;
- nested headings;
- a fenced **code** block with several lines (so the line-step has something to walk);
- a captioned **figure** (`@fig-` numbered);
- a **display equation** (`$$…$$`);
- a **table** with a caption;
- some **inline math** inside a sentence (to prove it is omitted from the spoken text but
  the prose still reads).

Auto-covered by the existing corpus-wide invariants (`crates/core/tests/corpus.rs`):
renders, every block has a unique id + valid sourcepos, document order. Front-matter stays
clean (no diagnostics keys).

## Testing strategy

1. **Corpus invariants (Rust):** `cargo test -p qmd-fast-core` renders the new pin doc and
   enforces the block-model guarantees automatically.
2. **Client type-check:** the reader enhancers are bundled JS; keep `qmdInitReadAloud`
   warning-clean under the client `tsc` check used for the web client (and lint-consistent
   with the surrounding enhancers).
3. **Browser (chrome-devtools MCP)** against the live preview of the pin doc, with
   `window.__qmdSpeakImpl` overridden to auto-advance:
   - Listen starts from the block in view; the spoken text excludes code/math.
   - The highlight walks sentence → sentence within a paragraph, then block → block.
   - The current step scrolls toward centre (autoscroll fires).
   - The code block announces, then `.qhl-ln-hl` advances line by line, then clears.
   - Figure / equation / table announce by label.
   - Mini-player: play/pause resumes the current step; next/prev jump by block; speed
     changes the rate of subsequent utterances; stop clears highlight + code focus and
     hides the bar.
   - With `prefers-reduced-motion: reduce` emulated, scrolling uses `auto` (no smooth) and
     the bar does not animate in.
   - 0 console errors throughout; on a page where `speechSynthesis` is stubbed away the
     Listen UI is hidden and nothing throws.

## Risks & mitigations

- **No voices in headless Chrome** → utterances may never fire `end`. Mitigated by the
  `__qmdSpeakImpl` seam (tests/headless drive advancement) and `onerror = onDone`.
- **Sentence segmentation across inline markup** (a sentence spanning `<em>`/`<code>`):
  ranges are computed over the block's text-node sequence, so a sentence can span child
  elements; the Range simply starts/ends in the right text nodes.
- **`speechSynthesis` paused-tab / Chrome's ~15s auto-pause quirk:** the per-sentence
  utterance design keeps utterances short, which sidesteps the long-utterance cutoff;
  `pause()`/`resume()` go through `cancel()` + re-speak rather than the flaky
  `speechSynthesis.pause()/resume()`.
- **Live block swap mid-listen:** driver re-resolves the element per step and stops if it
  vanished; never caches a node across a swap.
- **Reduced-motion + autoscroll:** explicitly gated.

## Out of scope follow-ups (recorded, not built)

- Word-level karaoke highlight via `boundary` events.
- Persisted "resume listening" cursor.
- A `?`-style keyboard reader binding `space`/arrows to playback (FEATURE-IDEAS.md #55).
- Reading code text aloud (intentionally rejected as grating).
