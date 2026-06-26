# Read-aloud study mode — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reader-side, read-only "Listen" feature that reads the built page aloud (Web Speech API) block-by-block from the block in view — prose spoken sentence-by-sentence with the sentence highlighted + auto-scrolled, code announced then visually line-stepped, figures/equations/tables announced — controlled by a floating mini-player.

**Architecture:** One new client-side enhancer `qmdInitReadAloud()` in `crates/core/assets/js/code-enhance.js`, registered through the existing `qmdEnhancers` registry, plus CSS in `base.css` and a corpus pin doc. Pure-function playlist compiler at module scope; a stateful driver + highlight + UI inside the init closure. **No Rust render change** — the enhancer rides the bundled `code_scripts()` JS unchanged.

**Tech Stack:** Vanilla JS (ES5-style, matching the surrounding enhancers — `var`, function declarations, no transpile step), the Web Speech API (`speechSynthesis` / `SpeechSynthesisUtterance`), the CSS Custom Highlight API (`CSS.highlights` / `Highlight` / `::highlight()`), `Intl.Segmenter`. Rust corpus test via `qmd_fast_core::render_document_with_includes`.

## Global Constraints

- **HTML-only output.** No new output target. (`crates/core/src/render` untouched.)
- **Single editing surface.** Read-only; never write the author `.qmd`; no preview write-back. Navigation/scroll only.
- **Block model untouched.** Never read/mutate `data-block-id` / `data-sourcepos`; never change the diff. Highlight via CSS Custom Highlight API (no DOM mutation); transient-`<mark>` fallback fully unwound on advance/stop.
- **Do-NOT-touch machinery** (`divs.rs`, `cite.rs`, `includes.rs`, numbering scanners, exec/freeze/kernel): untouched.
- **Offline / self-contained.** Web Speech is a browser API. No vendored bundle, no CDN, no new dependency.
- **Deck-skipped.** No-op on a `.qmd-deck` page.
- **Idempotent enhancer** guarded by `window.__qmdReadAloud`; graceful no-op (no UI) when `window.speechSynthesis`/`SpeechSynthesisUtterance` is absent.
- **JS style:** match the existing enhancers in `code-enhance.js` — `var`, `function` declarations, `[].slice.call`/`[].forEach.call`, no arrow functions / `const` / `let` (the file is shipped unminified and type-checked loosely).
- **Reader-owned state in `localStorage`:** `qmd-ra-rate` and `qmd-ra-voice` (GLOBAL keys — reader prefs, not per-page). No persisted listening position.
- **Distinct highlight token:** the spoken-sentence cursor uses `--qmd-ra-highlight` (NOT the reader-highlight `--qmd-userhl-bg`).

---

### Task 1: Corpus pin doc + structural render test

The enhancer is client-side JS verified in the browser; this task pins the *server output* it walks (code lines, a figure, a display equation, a captioned table) with an automated Rust test, and adds the corpus document.

**Files:**
- Create: `corpus/reader/read-aloud.qmd`
- Create: `crates/core/tests/read_aloud.rs`
- (uses) `crates/core/tests/common/mod.rs` (`corpus_dir()`)

**Interfaces:**
- Consumes: `qmd_fast_core::render_document_with_includes(&str, &Path) -> RenderedDoc`, `RenderedDoc::body_html() -> String`.
- Produces: the pin document `corpus/reader/read-aloud.qmd` (auto-covered by `corpus.rs` invariants) and `read_aloud.rs::pin_doc_renders_structures_read_aloud_walks`.

- [ ] **Step 1: Write the pin document**

Create `corpus/reader/read-aloud.qmd`:

```markdown
---
title: "Listening to a document"
toc: true
---

This page exists to be **listened to**. Open the reader menu (the *Aa* button) and press
*Listen*: the page is read aloud from the block in view, one sentence at a time, with the
current sentence highlighted and scrolled to the centre. It is read-only, like every reader
feature; nothing here is written back to the source.

Read-aloud is a study aid, e.g. for revising on a commute, and an accessibility feature: a
math-heavy or code-heavy page becomes something you can follow without looking. The voice and
speed are yours to choose, and they persist across pages.

## How non-prose is handled

Some blocks are not sentences. The reader announces them instead of reading their contents:

- A code block is announced ("Code block, N lines") and then its lines light up one by one.
- A figure is announced by its caption and number.
- A display equation is announced as "Equation"; inline math like $e^{i\pi} + 1 = 0$ inside a
  sentence is skipped in the spoken stream, but the words around it still read.
- A table is announced by its caption.

## A code block

```python
import numpy as np

def em_step(x, theta):
    resp = e_step(x, theta)
    return m_step(x, resp)
```

## A figure

![The reading cursor advances through the document.](https://upload.wikimedia.org/wikipedia/commons/thumb/a/a7/Camponotus_flavomarginatus_ant.jpg/320px-Camponotus_flavomarginatus_ant.jpg){#fig-cursor}

## An equation

$$
\mathcal{L}(\theta) = \sum_{i} \log p(x_i \mid \theta)
$$

## A table

| Symbol | Meaning      |
|--------|--------------|
| x      | observation  |
| theta  | parameters   |

: Notation used above {#tbl-notation}
```

- [ ] **Step 2: Write the failing structural test**

Create `crates/core/tests/read_aloud.rs`:

```rust
//! Structural contract for the read-aloud reader enhancer. The enhancer (client-side
//! JS, browser-verified) walks the rendered DOM, so the *server output* of its pin doc
//! must contain the structures it keys off: per-line code spans (`.qhl-ln`), a numbered
//! figure (`<figcaption>`), a display equation (`.katex-display`), and a captioned table
//! (`<caption>`). If a future render change drops one of these, read-aloud silently stops
//! announcing/stepping it — this test makes that a hard failure.

mod common;
use common::corpus_dir;
use std::fs;

fn body() -> String {
    let path = corpus_dir().join("reader/read-aloud.qmd");
    let src = fs::read_to_string(&path).unwrap();
    qmd_fast_core::render_document_with_includes(&src, path.parent().unwrap()).body_html()
}

#[test]
fn pin_doc_renders_structures_read_aloud_walks() {
    let html = body();
    assert!(
        html.contains("class=\"qhl-ln\""),
        "code block must emit per-line .qhl-ln spans for the line-step"
    );
    assert!(
        html.contains("<figcaption>Figure&nbsp;1:"),
        "figure must render a numbered figcaption for the announce step"
    );
    assert!(
        html.contains("katex-display"),
        "display equation must render as .katex-display for the announce step"
    );
    assert!(
        html.contains("<caption>Table&nbsp;1:"),
        "captioned table must render a numbered <caption> for the announce step"
    );
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p qmd-fast-core --test read_aloud -- --nocapture`
Expected: FAIL — initially the file/doc may be missing or, if present, confirm all four asserts pass once the doc is in place. (If the doc is already written from Step 1, this test should PASS; the "fail-first" signal is that the test crate did not exist before this task.)

- [ ] **Step 4: Run the full corpus + new test to verify they pass**

Run: `cargo test -p qmd-fast-core --test read_aloud && cargo test -p qmd-fast-core --test corpus`
Expected: PASS — the new doc renders, satisfies the block-model invariants (unique ids, valid sourcepos, document order), and has clean front-matter (`title`, `toc` are known keys).

- [ ] **Step 5: Commit**

```bash
git add corpus/reader/read-aloud.qmd crates/core/tests/read_aloud.rs
git commit -m "test(reader): pin read-aloud corpus doc + structural render contract"
```

---

### Task 2: CSS — highlight token, mini-player, mark fallback, reduced-motion

Standalone styling. No behavior yet; harmless if added before the JS.

**Files:**
- Modify: `crates/core/assets/css/base.css` (near the `--qmd-userhl-bg` block at ~line 97-100)
- Modify: `crates/core/assets/css/dark.css` (the `html[data-theme="dark"]` token block at line 2)

**Interfaces:**
- Produces: the `--qmd-ra-highlight` token (per theme), `::highlight(qmd-readaloud)`, `mark.qmd-ra-mark`, and `.qmd-ra-bar` / `.qmd-ra-btn` / `.qmd-ra-speed` / `.qmd-ra-live` classes the JS in Task 3 attaches.

- [ ] **Step 1: Add the highlight token + highlight styles to `base.css`**

After the existing reader-highlight block (`mark.qmd-userhl { ... }`, ~line 100), insert:

```css
  /* Read-aloud: the moving spoken-sentence cursor. A DISTINCT token from the
     reader-highlight (--qmd-userhl-bg) so the transient reading cursor never looks
     like a saved highlight. */
  :root { --qmd-ra-highlight: rgba(96, 165, 250, .38); }
  html[data-theme="sepia"] { --qmd-ra-highlight: rgba(56, 132, 200, .32); }
  ::highlight(qmd-readaloud) { background-color: var(--qmd-ra-highlight); color: inherit; }
  mark.qmd-ra-mark { background-color: var(--qmd-ra-highlight); color: inherit; border-radius: 2px; }

  /* Read-aloud mini-player: a small floating transport, shown only while listening. */
  .qmd-ra-bar { position: fixed; left: 50%; bottom: 1rem; transform: translateX(-50%);
    z-index: 2147481600; display: flex; align-items: center; gap: .35rem;
    background: var(--qmd-bg); border: 1px solid var(--qmd-border); border-radius: 999px;
    padding: .3rem .5rem; box-shadow: 0 4px 18px rgba(0,0,0,.18); font-size: .85rem; }
  .qmd-ra-bar[hidden] { display: none; }
  .qmd-ra-btn { border: 0; background: transparent; color: var(--qmd-fg); cursor: pointer;
    font-size: 1rem; line-height: 1; padding: .25rem .4rem; border-radius: 999px; }
  .qmd-ra-btn:hover { background: var(--qmd-code-bg); }
  .qmd-ra-speed { font-variant-numeric: tabular-nums; opacity: .8; min-width: 2.6em;
    text-align: center; }
  .qmd-ra-live { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0);
    clip-path: inset(50%); white-space: nowrap; }
  .qmd-ra-voice-sel { max-width: 11rem; }
  @media print { .qmd-ra-bar { display: none !important; } }
  @media (prefers-reduced-motion: no-preference) {
    .qmd-ra-bar { animation: qmd-ra-in .18s ease-out; }
  }
  @keyframes qmd-ra-in { from { opacity: 0; transform: translate(-50%, 6px); }
    to { opacity: 1; transform: translateX(-50%); } }
```

- [ ] **Step 2: Add the dark-theme highlight token to `dark.css`**

Inside the `html[data-theme="dark"] { ... }` block (after line 2), add:

```css
    --qmd-ra-highlight: rgba(96, 165, 250, .30);
```

(If that block is a single `{ ... }` of `--qmd-*` declarations, add the line among them. Otherwise add a sibling rule `html[data-theme="dark"] { --qmd-ra-highlight: rgba(96,165,250,.30); }` next to the other dark `--qmd-*` overrides.)

- [ ] **Step 3: Verify the CSS bundles + nothing breaks**

Run: `cargo build -p qmd-fast-core`
Expected: builds (CSS is `include_str!`-bundled; a syntax slip won't fail the build, so eyeball the diff for balanced braces).

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/css/base.css crates/core/assets/css/dark.css
git commit -m "feat(reader): read-aloud CSS — distinct cursor token + mini-player"
```

---

### Task 3: The read-aloud enhancer (compiler + driver + UI)

The cohesive unit. Pure playlist helpers at module scope; stateful driver + highlight + UI inside the init closure; registration through `qmdEnhancers`. A `window.__qmdReadAloud` test hook exposes the compiled playlist and the driver.

**Files:**
- Modify: `crates/core/assets/js/code-enhance.js` (add the helpers + `qmdInitReadAloud`; register it next to the other reader enhancers near line 307)

**Interfaces:**
- Consumes: `window.qmdEnhancers.register`, `window.qmdReaderMenu.addSection(title, node, onOpen?)` and `window.qmdReaderMenu.close()`, the `.qhl-ln`/`.qhl-ln-hl`/`.qhl-lines-active` class contract, the `.qmd-reader-row`/`.qmd-reader-seg` prefs CSS, `window.speechSynthesis`, `SpeechSynthesisUtterance`, `CSS.highlights`/`Highlight`, `Intl.Segmenter`.
- Produces: `qmdInitReadAloud()` enhancer; `window.__qmdSpeakImpl(utterance)` (injectable speak seam, default = real `speechSynthesis.speak`); `window.__qmdReadAloud = { compile(): Array<{kind,text,block}>, driver }` test hook; the reader-menu "Listen" section + the `.qmd-ra-bar` mini-player.

- [ ] **Step 1: Confirm the hook is absent (fail-first)**

With the current build served, in the browser console `window.__qmdReadAloud` is `undefined` and there is no "Listen" section in the reader menu. (Baseline before the code exists.)

- [ ] **Step 2: Add the module-scope playlist helpers + the speak seam**

In `crates/core/assets/js/code-enhance.js`, ABOVE the `// --- Built-in enhancers` block (~line 259), add:

```js
// ===== Read-aloud study mode =====================================================
// Reader-side, read-only "Listen": speaks the built page (Web Speech API) block by
// block from the block in view. Prose -> one utterance per sentence, the sentence
// highlighted (CSS Custom Highlight API) + auto-scrolled; code -> announced then the
// .qhl-ln lines stepped (no code text spoken); figure/equation/table -> announced. A
// floating mini-player controls playback. No source write, no block-model change,
// offline (Web Speech is a browser API), deck-skipped, idempotent.
//
// The speak primitive is injectable so headless tests (no TTS voices) drive the
// playlist deterministically: override window.__qmdSpeakImpl to invoke u.onend().
window.__qmdSpeakImpl = window.__qmdSpeakImpl || function (u) { window.speechSynthesis.speak(u); };

function qmdRaGet(k, d) { try { return localStorage.getItem(k) || d; } catch (e) { return d; } }
function qmdRaSet(k, v) { try { if (v == null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (e) {} }

// Top-level content blocks: a [data-block-id] not nested inside another block.
function qmdRaContentBlocks() {
  return [].slice.call(document.querySelectorAll('[data-block-id]')).filter(function (el) {
    return !el.parentElement || !el.parentElement.closest('[data-block-id]');
  });
}

// A text node is non-spoken if it sits inside math/code within the block.
function qmdRaSkip(node, block) {
  var p = node.parentNode;
  while (p && p !== block) {
    if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
        (p.classList && p.classList.contains('katex')))) return true;
    p = p.parentNode;
  }
  return false;
}

// The block's highlightable (prose) text as one string + a map back to text nodes.
function qmdRaBlockSpoken(block) {
  var walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, null);
  var full = '', spans = [], n;
  while ((n = walker.nextNode())) {
    if (qmdRaSkip(n, block)) continue;
    spans.push([full.length, n]);
    full += n.nodeValue;
  }
  return { full: full, spans: spans };
}

// Map a global offset in `full` back to [textNode, localOffset].
function qmdRaLocate(spans, off) {
  for (var i = spans.length - 1; i >= 0; i--) {
    if (off >= spans[i][0]) {
      var nd = spans[i][1];
      return [nd, Math.min(off - spans[i][0], nd.nodeValue.length)];
    }
  }
  return spans.length ? [spans[0][1], 0] : null;
}

// Trim whitespace off a [s,e) offset window.
function qmdRaTrim(full, s, e) {
  while (s < e && /\s/.test(full.charAt(s))) s++;
  while (e > s && /\s/.test(full.charAt(e - 1))) e--;
  return [s, e];
}

// Sentence boundaries in `full` as [start,end) offsets (Intl.Segmenter, regex fallback).
function qmdRaSentences(full, lang) {
  var ranges = [];
  if (window.Intl && Intl.Segmenter) {
    try {
      var seg = new Intl.Segmenter(lang || undefined, { granularity: 'sentence' });
      Array.from(seg.segment(full)).forEach(function (s) {
        if (/\S/.test(s.segment)) ranges.push([s.index, s.index + s.segment.length]);
      });
      if (ranges.length) return ranges;
    } catch (e) {}
  }
  var re = /[^.!?]*[.!?]+["')\]]*\s*|[^.!?]+$/g, m;
  while ((m = re.exec(full))) {
    if (!m[0]) { re.lastIndex++; continue; }
    if (/\S/.test(m[0])) ranges.push([m.index, m.index + m[0].length]);
  }
  return ranges.length ? ranges : (full.trim() ? [[0, full.length]] : []);
}

// Compile a prose element into per-sentence `say` steps (with a DOM Range each).
function qmdRaCompileProse(el, steps) {
  var sp = qmdRaBlockSpoken(el);
  if (!sp.full.trim()) return;
  var lang = document.documentElement.lang || undefined;
  qmdRaSentences(sp.full, lang).forEach(function (r) {
    var t = qmdRaTrim(sp.full, r[0], r[1]);
    var text = sp.full.slice(t[0], t[1]);
    if (!text.trim()) return;
    var a = qmdRaLocate(sp.spans, t[0]), b = qmdRaLocate(sp.spans, t[1]);
    if (!a || !b) return;
    var range = document.createRange();
    try { range.setStart(a[0], a[1]); range.setEnd(b[0], b[1]); } catch (e) { return; }
    steps.push({ kind: 'say', text: text, range: range, el: el });
  });
}

// Compile one top-level block into ordered steps (code/figure/equation/table/prose).
function qmdRaCompileBlock(block, steps) {
  var pre = block.matches('pre') ? block : block.querySelector('pre');
  var codeLines = pre ? pre.querySelectorAll('.qhl-ln') : [];
  if (codeLines.length) {
    var codeEl = pre.querySelector('code'), lang = '';
    if (codeEl) { var cls = (codeEl.className || '').match(/language-([\w+-]+)/); if (cls) lang = cls[1]; }
    var label = 'Code block. ' + codeLines.length + (codeLines.length === 1 ? ' line.' : ' lines.') +
      (lang ? ' ' + lang + '.' : '');
    steps.push({ kind: 'say', text: label, el: pre });
    steps.push({ kind: 'code', pre: pre, lines: codeLines, el: pre });
    return;
  }
  var fig = block.matches('figure') ? block : block.querySelector('figure');
  if (fig) {
    var cap = fig.querySelector('figcaption');
    var ftext = ((cap ? cap.textContent : '') || 'Figure').replace(/ /g, ' ').trim() || 'Figure';
    steps.push({ kind: 'say', text: ftext, el: fig });
    return;
  }
  if (block.querySelector('.katex-display') && !qmdRaBlockSpoken(block).full.trim()) {
    steps.push({ kind: 'say', text: 'Equation.', el: block });
    return;
  }
  var table = block.matches('table') ? block : block.querySelector('table');
  if (table) {
    var tcap = table.querySelector('caption');
    var ttext = (tcap ? tcap.textContent.replace(/ /g, ' ').trim() : 'Table') || 'Table';
    steps.push({ kind: 'say', text: ttext.replace(/\.?$/, '.'), el: table });
    return;
  }
  if (block.matches('ul, ol, dl')) {
    [].slice.call(block.children).forEach(function (li) {
      if (li.matches && li.matches('li, dd, dt')) qmdRaCompileProse(li, steps);
    });
    return;
  }
  qmdRaCompileProse(block, steps);
}

// The first content block at/below the viewport top (where Listen starts).
function qmdRaStartBlock() {
  var blocks = qmdRaContentBlocks();
  for (var i = 0; i < blocks.length; i++) {
    if (blocks[i].getBoundingClientRect().top >= -4) return blocks[i];
  }
  return blocks[0] || null;
}

// Compile the whole playlist from `startEl` to the end; tag each step with its block index.
function qmdRaCompile(startEl) {
  var blocks = qmdRaContentBlocks(), startIdx = 0;
  if (startEl) { var i = blocks.indexOf(startEl); if (i >= 0) startIdx = i; }
  var steps = [];
  for (var k = startIdx; k < blocks.length; k++) {
    var before = steps.length;
    qmdRaCompileBlock(blocks[k], steps);
    for (var j = before; j < steps.length; j++) steps[j].block = k;
  }
  return { steps: steps, blocks: blocks };
}

// A segmented control reusing the prefs CSS (.qmd-reader-row/.qmd-reader-seg).
function qmdRaSeg(title, options, getCur, onPick) {
  var row = document.createElement('div'); row.className = 'qmd-reader-row';
  var label = document.createElement('span'); label.textContent = title;
  var group = document.createElement('div'); group.className = 'qmd-reader-seg';
  group.setAttribute('role', 'group'); group.setAttribute('aria-label', title);
  var buttons = [];
  function sync() { var cur = getCur(); buttons.forEach(function (b, i) { b.setAttribute('aria-pressed', options[i][0] === cur ? 'true' : 'false'); }); }
  options.forEach(function (opt) {
    var b = document.createElement('button'); b.type = 'button'; b.textContent = opt[1];
    b.addEventListener('click', function () { onPick(opt[0]); sync(); });
    group.appendChild(b); buttons.push(b);
  });
  row.appendChild(label); row.appendChild(group); sync();
  return row;
}

// The voice picker row (OS voices); refresh() re-reads getVoices() (async on some browsers).
function qmdRaVoiceRow(onPick) {
  var row = document.createElement('div'); row.className = 'qmd-reader-row';
  var label = document.createElement('span'); label.textContent = 'Voice';
  var sel = document.createElement('select'); sel.className = 'qmd-ra-voice-sel';
  sel.setAttribute('aria-label', 'Reading voice');
  sel.addEventListener('change', function () { onPick(sel.value); });
  function refresh() {
    var cur = qmdRaGet('qmd-ra-voice', '');
    var vs = (window.speechSynthesis && window.speechSynthesis.getVoices()) || [];
    sel.innerHTML = '';
    var def = document.createElement('option'); def.value = ''; def.textContent = 'Default'; sel.appendChild(def);
    vs.forEach(function (v) { var o = document.createElement('option'); o.value = v.name; o.textContent = v.name + (v.lang ? ' (' + v.lang + ')' : ''); sel.appendChild(o); });
    sel.value = cur;
  }
  row.appendChild(label); row.appendChild(sel);
  return { row: row, refresh: refresh };
}
```

- [ ] **Step 3: Add the init closure (highlight + driver + UI + registration)**

Immediately after the helpers (still above the built-in enhancers block), add:

```js
function qmdInitReadAloud() {
  if (window.__qmdReadAloud && window.__qmdReadAloud.__inited) return;
  if (document.querySelector('.qmd-deck')) return;        // decks have their own chrome
  if (!window.speechSynthesis || typeof SpeechSynthesisUtterance === 'undefined') return; // no API -> no UI
  if (!window.qmdReaderMenu) return;                       // need the menu host

  // --- highlight (CSS Custom Highlight API, with a <mark> fallback) ---------------
  var hl = (window.CSS && CSS.highlights && window.Highlight) ? new Highlight() : null;
  if (hl) CSS.highlights.set('qmd-readaloud', hl);
  var marks = [];
  function clearMark() {
    marks.forEach(function (m) {
      var parent = m.parentNode; if (!parent) return;
      while (m.firstChild) parent.insertBefore(m.firstChild, m);
      parent.removeChild(m); parent.normalize();
    });
    marks = [];
  }
  function setHighlight(range) {
    if (hl) { hl.clear(); if (range) hl.add(range); return; }
    clearMark();
    if (range) { try { var m = document.createElement('mark'); m.className = 'qmd-ra-mark'; range.surroundContents(m); marks.push(m); } catch (e) {} }
  }
  function clearHighlight() { if (hl) hl.clear(); else clearMark(); }

  // --- code line-step --------------------------------------------------------------
  var activePre = null;
  function clearCode() {
    if (!activePre) return;
    activePre.classList.remove('qhl-lines-active');
    [].slice.call(activePre.querySelectorAll('.qhl-ln-hl')).forEach(function (l) { l.classList.remove('qhl-ln-hl'); });
    activePre = null;
  }

  function reducedMotion() { return window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches; }
  function rate() { var r = parseFloat(qmdRaGet('qmd-ra-rate', '1')); return r > 0 ? r : 1; }
  function currentVoice() {
    var name = qmdRaGet('qmd-ra-voice', '');
    if (!name) return null;
    var vs = (window.speechSynthesis.getVoices && window.speechSynthesis.getVoices()) || [];
    for (var i = 0; i < vs.length; i++) if (vs[i].name === name) return vs[i];
    return null;
  }

  // --- driver (state machine) ------------------------------------------------------
  var state = { steps: [], idx: 0, playing: false, codeTimer: null, token: 0 };

  function stopTimers() { if (state.codeTimer) { clearTimeout(state.codeTimer); state.codeTimer = null; } }
  function scrollTo(el) { if (el) el.scrollIntoView({ block: 'center', behavior: reducedMotion() ? 'auto' : 'smooth' }); }

  function focusStep(step) {
    clearCode();
    if (step.kind === 'say') setHighlight(step.range || null); else clearHighlight();
    scrollTo(step.el);
  }

  function speakSay(step, done) {
    if (!step.text || !step.text.trim()) { done(); return; }
    var u = new SpeechSynthesisUtterance(step.text);
    u.rate = rate();
    var v = currentVoice(); if (v) u.voice = v;
    u.onend = done; u.onerror = done;
    window.__qmdSpeakImpl(u);
  }

  function runCode(step, done) {
    var lines = step.lines, pre = step.pre, i = 0;
    activePre = pre; pre.classList.add('qhl-lines-active');
    function tick() {
      [].forEach.call(lines, function (l, k) { l.classList.toggle('qhl-ln-hl', k === i); });
      if (lines[i]) lines[i].scrollIntoView({ block: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
      i++;
      state.codeTimer = setTimeout(i >= lines.length ? function () { clearCode(); done(); } : tick, 650 / rate());
    }
    tick();
  }

  function play() {
    state.playing = true; ui.setPlaying(true);
    var step = state.steps[state.idx];
    if (!step) { stop(); return; }
    if (step.el && !document.body.contains(step.el)) { stop(); return; } // live-swap safety
    var myToken = ++state.token;
    function done() { if (myToken !== state.token) return; advance(); }
    focusStep(step);
    if (step.kind === 'code') runCode(step, done); else speakSay(step, done);
  }
  function advance() {
    state.idx++;
    if (state.idx >= state.steps.length) { stop(); ui.announce('Finished'); return; }
    play();
  }
  function start(steps) {
    state.token++; stopTimers(); window.speechSynthesis.cancel();
    state.steps = steps; state.idx = 0;
    if (!steps.length) return;
    ui.show(); ui.announce('Playing'); play();
  }
  function pause() { state.token++; state.playing = false; ui.setPlaying(false); stopTimers(); window.speechSynthesis.cancel(); }
  function resume() { if (state.steps.length) play(); }
  function stop() { state.token++; state.playing = false; stopTimers(); window.speechSynthesis.cancel(); clearHighlight(); clearCode(); ui.setPlaying(false); ui.hide(); }
  function jumpBlock(dir) {
    if (!state.steps.length) return;
    var cur = state.steps[state.idx].block, ni = -1, j;
    if (dir > 0) { for (j = 0; j < state.steps.length; j++) if (state.steps[j].block > cur) { ni = j; break; } }
    else { for (j = state.steps.length - 1; j >= 0; j--) if (state.steps[j].block < cur) { ni = j; break; } }
    if (ni < 0) return;
    state.token++; stopTimers(); window.speechSynthesis.cancel();
    state.idx = ni;
    if (state.playing) play(); else focusStep(state.steps[ni]);
  }
  function applyRate() { if (state.playing) { stopTimers(); window.speechSynthesis.cancel(); play(); } }

  var driver = { start: start, pause: pause, resume: resume, stop: stop, jumpBlock: jumpBlock, applyRate: applyRate, isPlaying: function () { return state.playing; } };

  // --- mini-player UI --------------------------------------------------------------
  function btn(cls, label, txt) { var b = document.createElement('button'); b.type = 'button'; b.className = 'qmd-ra-btn ' + cls; b.setAttribute('aria-label', label); b.textContent = txt; return b; }
  var bar = document.createElement('div');
  bar.className = 'qmd-ra-bar'; bar.setAttribute('role', 'group'); bar.setAttribute('aria-label', 'Read aloud'); bar.hidden = true;
  var prev = btn('qmd-ra-prev', 'Previous block', '⏮');
  var toggle = btn('qmd-ra-toggle', 'Pause', '⏸');
  var next = btn('qmd-ra-next', 'Next block', '⏭');
  var speed = document.createElement('span'); speed.className = 'qmd-ra-speed';
  var stopb = btn('qmd-ra-stop', 'Stop', '✕');
  var live = document.createElement('span'); live.className = 'qmd-ra-live'; live.setAttribute('aria-live', 'polite');
  bar.appendChild(prev); bar.appendChild(toggle); bar.appendChild(next); bar.appendChild(speed); bar.appendChild(stopb); bar.appendChild(live);
  document.body.appendChild(bar);

  var ui = {
    show: function () { bar.hidden = false; },
    hide: function () { bar.hidden = true; },
    setPlaying: function (p) { toggle.textContent = p ? '⏸' : '▶'; toggle.setAttribute('aria-label', p ? 'Pause' : 'Play'); },
    announce: function (m) { live.textContent = m; },
    setSpeed: function (r) { speed.textContent = r + '×'; }
  };
  ui.setSpeed(qmdRaGet('qmd-ra-rate', '1'));

  toggle.addEventListener('click', function () {
    if (driver.isPlaying()) { driver.pause(); ui.announce('Paused'); }
    else { driver.resume(); ui.setPlaying(true); ui.announce('Playing'); }
  });
  prev.addEventListener('click', function () { driver.jumpBlock(-1); });
  next.addEventListener('click', function () { driver.jumpBlock(1); });
  stopb.addEventListener('click', function () { driver.stop(); });

  // --- reader-menu "Listen" section ------------------------------------------------
  var bodyEl = document.createElement('div');
  var listen = document.createElement('button');
  listen.type = 'button'; listen.className = 'qmd-reader-reset'; listen.textContent = 'Listen';
  listen.addEventListener('click', function () {
    driver.start(qmdRaCompile(qmdRaStartBlock()).steps);
    window.qmdReaderMenu.close();
  });
  bodyEl.appendChild(listen);

  var SPEEDS = [['0.8', '0.8×'], ['1', '1×'], ['1.25', '1.25×'], ['1.5', '1.5×'], ['2', '2×']];
  bodyEl.appendChild(qmdRaSeg('Speed', SPEEDS, function () { return qmdRaGet('qmd-ra-rate', '1'); }, function (v) {
    qmdRaSet('qmd-ra-rate', v === '1' ? null : v); ui.setSpeed(v); driver.applyRate();
  }));

  var voiceRow = qmdRaVoiceRow(function (name) { qmdRaSet('qmd-ra-voice', name || null); });
  bodyEl.appendChild(voiceRow.row);
  voiceRow.refresh();
  if (typeof window.speechSynthesis.onvoiceschanged !== 'undefined') {
    window.speechSynthesis.addEventListener('voiceschanged', voiceRow.refresh);
  }

  window.qmdReaderMenu.addSection('Listen', bodyEl);

  // --- test hook -------------------------------------------------------------------
  window.__qmdReadAloud = {
    __inited: true,
    driver: driver,
    compile: function () {
      return qmdRaCompile(qmdRaStartBlock()).steps.map(function (s) { return { kind: s.kind, text: s.text || null, block: s.block }; });
    }
  };
}
```

- [ ] **Step 4: Register the enhancer**

In the built-in enhancers block (near line 307, after `qmdInitFocusMode`), add:

```js
window.qmdEnhancers.register(function () { qmdInitReadAloud(); });
```

- [ ] **Step 5: Type-check the client JS**

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json` (if `code-enhance.js` is in scope), and eyeball `qmdInitReadAloud` for balanced braces. Then `cd .. && cargo build -p qmd-fast-core`.
Expected: no new type errors attributable to the read-aloud code; build succeeds.

- [ ] **Step 6: Browser-verify the playlist compiler (chrome-devtools MCP)**

Serve the pin doc: use the `/preview` skill or `cargo run -p qmd-fast-server -- preview corpus/reader/read-aloud.qmd 4388`. Then with chrome-devtools MCP navigate to it and run `evaluate_script`:

```js
// scroll to top so Listen starts at the first block
window.scrollTo(0, 0);
var pl = window.__qmdReadAloud.compile();
return {
  total: pl.length,
  kinds: pl.map(function (s) { return s.kind; }).filter(function (v, i, a) { return a.indexOf(v) === i; }),
  hasCodeAnnounce: pl.some(function (s) { return /^Code block\. 5 lines\. python\./.test(s.text || ''); }),
  hasCodeStep: pl.some(function (s) { return s.kind === 'code'; }),
  hasFigure: pl.some(function (s) { return /Figure 1:/.test(s.text || ''); }),
  hasEquation: pl.some(function (s) { return s.text === 'Equation.'; }),
  hasTable: pl.some(function (s) { return /Table 1:/.test(s.text || ''); }),
  // prove code/math text never enters the spoken stream:
  noCodeText: !pl.some(function (s) { return /import numpy|e_step/.test(s.text || ''); }),
  noMathML: !pl.some(function (s) { return /\bmathrm\b|sum_/.test(s.text || ''); })
};
```

Expected: `kinds` contains `"say"` and `"code"`; `hasCodeAnnounce/hasCodeStep/hasFigure/hasEquation/hasTable` all `true`; `noCodeText` and `noMathML` both `true`. Also `list_console_messages` shows 0 errors.

- [ ] **Step 7: Commit**

```bash
git add crates/core/assets/js/code-enhance.js
git commit -m "feat(reader): read-aloud enhancer — playlist compiler, driver, mini-player"
```

---

### Task 4: Browser behavior verification (playback, with the speak stub)

Drive the full feature deterministically by stubbing the speak seam, and assert the highlight walk, autoscroll, code line-step, announcements, controls, and reduced-motion. Fix any defect found, then re-verify.

**Files:**
- (verify only) `crates/core/assets/js/code-enhance.js` — fix in place if a check fails.

**Interfaces:**
- Consumes: `window.__qmdSpeakImpl`, `window.__qmdReadAloud.driver`, `CSS.highlights.get('qmd-readaloud')`.

- [ ] **Step 1: Install the deterministic speak stub**

With the preview open (chrome-devtools), `evaluate_script`:

```js
window.__qmdSpoken = [];
window.__qmdSpeakImpl = function (u) {
  window.__qmdSpoken.push(u.text);
  // fire end on a microtask so the driver advances without real TTS
  Promise.resolve().then(function () { if (u.onend) u.onend(); });
};
return 'stub installed';
```

- [ ] **Step 2: Verify the highlight walks + spoken stream is prose-only**

`evaluate_script`:

```js
window.scrollTo(0, 0);
window.__qmdSpoken = [];
var d = window.__qmdReadAloud.driver;
d.start(window.__qmdReadAloud.compile === undefined ? [] : undefined); // no-op guard
return 'started';
```

(The Listen button is the real entry; for the test, click it instead.) Use chrome-devtools `click` on the reader menu *Aa* button, then on *Listen*. Wait ~1s, then `evaluate_script`:

```js
return {
  spokenCount: window.__qmdSpoken.length,
  firstFew: window.__qmdSpoken.slice(0, 3),
  // after the run completes, a highlight range existed at some point:
  spokenHasProse: window.__qmdSpoken.some(function (t) { return /listened to/i.test(t); }),
  spokenNoCode: !window.__qmdSpoken.some(function (t) { return /import numpy/.test(t); })
};
```

Expected: `spokenCount` > 5; `spokenHasProse` true; `spokenNoCode` true.

- [ ] **Step 3: Verify the code line-step**

Re-run from the code section: scroll to the "A code block" heading, click *Aa* → *Listen*, and within the code phase `evaluate_script` (poll a few times):

```js
var pre = document.querySelector('pre');
return { activeClass: pre ? pre.classList.contains('qhl-lines-active') : null,
         litLines: document.querySelectorAll('.qhl-ln-hl').length };
```

Expected: at some tick `activeClass` is `true` and `litLines` is `1` (one line lit at a time); after the block, both clear.

- [ ] **Step 4: Verify mini-player controls**

With playback running: chrome-devtools `click` the pause button (`.qmd-ra-toggle`) → `evaluate_script` `return window.__qmdReadAloud.driver.isPlaying();` expected `false`; click again → expected `true`. Click next (`.qmd-ra-next`) and confirm `window.__qmdSpoken` grows from a later block. Click stop (`.qmd-ra-stop`) → `evaluate_script`:

```js
return { barHidden: document.querySelector('.qmd-ra-bar').hidden,
         lit: document.querySelectorAll('.qhl-ln-hl').length,
         hlEmpty: !CSS.highlights.has || !CSS.highlights.has('qmd-readaloud') || CSS.highlights.get('qmd-readaloud').size === 0 };
```

Expected: `barHidden` true, `lit` 0, `hlEmpty` true.

- [ ] **Step 5: Verify speed + reduced-motion**

Set speed: click the `1.5×` button in the Listen section; `evaluate_script` `return localStorage.getItem('qmd-ra-rate');` expected `"1.5"`. Then emulate reduced motion (chrome-devtools `emulate` CPU/none won't do this — use `evaluate_script` to confirm the code path): with `matchMedia('(prefers-reduced-motion: reduce)').matches` forced via the DevTools rendering emulation if available, confirm no console errors during a Listen run. (If emulation is unavailable, assert the guard exists by code review: `behavior: reducedMotion() ? 'auto' : 'smooth'`.)

- [ ] **Step 6: Confirm zero console errors + speech-absent no-op**

`list_console_messages` → 0 errors across the session. Then in a fresh tab `evaluate_script` BEFORE load is impractical; instead confirm by code review that `if (!window.speechSynthesis ...) return;` precedes any DOM creation, so a browser without the API renders no Listen UI and throws nothing.

- [ ] **Step 7: Commit any fixes**

```bash
git add crates/core/assets/js/code-enhance.js
git commit -m "fix(reader): read-aloud playback corrections from browser verification"
```

(Skip the commit if no fix was needed.)

---

### Task 5: Docs, corpus README, backlog, and full verification

**Files:**
- Modify: `corpus/README.md` (note the new reader doc)
- Modify: `docs/guide/reference/` — the reader-features reference page (add a "Listen / read-aloud" entry); find it with `grep -rl "reader menu\|Reader menu\|highlights" docs/guide`
- Modify: `notes/backlog.md` (record under the reader-experience cluster) and `notes/FEATURE-IDEAS.md` (mark #9 shipped)
- (verify) `THIRD_PARTY.md` — confirm NO change needed (Web Speech is a browser API, no new dep)

**Interfaces:** none (docs + notes).

- [ ] **Step 1: Add the corpus README entry**

In `corpus/README.md`, in the `reader/` section, add a line:

```markdown
- `read-aloud.qmd` — exercises read-aloud study mode: prose, a code block (line-step),
  a numbered figure, a display equation, and a captioned table (the announce steps).
```

- [ ] **Step 2: Document the feature in the user guide**

In the reader-features reference page found above, add a subsection describing: press *Aa* → *Listen*; reads from the block in view; sentence highlight + auto-scroll; code announced + line-stepped; figures/equations/tables announced; speed + voice persist; stop/skip via the floating bar; respects reduced-motion; entirely offline (browser speech). Match the page's existing prose voice; no em dashes.

- [ ] **Step 3: Update the notes**

In `notes/backlog.md` under the reader-experience cluster, add a one-line "shipped" note (read-aloud study mode: sentence-highlight + autoscroll, code line-step, announce figure/eq/table, mini-player, speed+voice prefs; pinned `corpus/reader/read-aloud.qmd`). In `notes/FEATURE-IDEAS.md`, mark idea #9 (read-aloud) + moonshot 3 as shipped.

- [ ] **Step 4: Full test + type-check pass**

Run:
```bash
cargo test -p qmd-fast-core
cd web-client && npx -y -p typescript tsc -p jsconfig.json ; cd ..
cargo fmt --check
```
Expected: all green (the `PostToolUse` rustfmt hook keeps `.rs` clean; only `read_aloud.rs` is new Rust). No client type errors.

- [ ] **Step 5: Final browser smoke (full document, real or stubbed speak)**

With the preview open, do one end-to-end Listen from the top through to "Finished" (using the stub if headless has no voices), confirming the cursor moves through prose → code line-step → figure → equation → table, the bar hides on finish, and `list_console_messages` is clean.

- [ ] **Step 6: Commit**

```bash
git add corpus/README.md docs/guide notes/backlog.md notes/FEATURE-IDEAS.md
git commit -m "docs(reader): document read-aloud study mode; mark idea #9 shipped"
```

---

## Self-Review

**Spec coverage:**
- Architecture / no-Rust / graceful no-op → Task 3 (init guards). ✓
- Playlist (prose sentences, code announce+line-step, figure/eq/table announce, inline-math omitted) → Task 3 `qmdRaCompile*`; verified Task 3 Step 6 + Task 4. ✓
- Driver (speak seam, advance, pause/resume/stop, prev/next block, speed, reduced-motion, live-swap safety, token guard) → Task 3 driver; verified Task 4. ✓
- Highlight (Custom Highlight API + `<mark>` fallback, distinct token) → Task 2 CSS + Task 3 `setHighlight`. ✓
- UI (reader-menu Listen section: Play + speed seg + voice select; floating mini-player; aria-live) → Task 3. ✓
- Persistence (rate + voice global; position ephemeral) → Task 3 `qmdRaGet/Set`; no position key. ✓
- Corpus pin + tests → Task 1 (doc + structural test) + corpus auto-walk; browser verification Tasks 3-4. ✓
- Invariants → Global Constraints; honored throughout. ✓
- Docs → Task 5. ✓

**Placeholder scan:** no TBD/TODO; all code blocks complete; the one judgement call (reduced-motion emulation may be unavailable headless) has an explicit code-review fallback. ✓

**Type/name consistency:** `qmdRaCompile` returns `{steps, blocks}`; steps carry `{kind, text?, range?, el, pre?, lines?, block}`; driver methods `start/pause/resume/stop/jumpBlock/applyRate/isPlaying` are defined in Task 3 and called by the UI in the same task + Task 4. `window.__qmdReadAloud.{driver, compile, __inited}` and `window.__qmdSpeakImpl` consistent across Tasks 3-4. CSS classes `.qmd-ra-bar/.qmd-ra-btn/.qmd-ra-speed/.qmd-ra-live/.qmd-ra-voice-sel/mark.qmd-ra-mark` + token `--qmd-ra-highlight` consistent between Task 2 (defines) and Task 3 (attaches). ✓
