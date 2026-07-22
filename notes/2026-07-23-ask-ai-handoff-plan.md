# Ask AI Hand-off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a client-side, backendless "Ask AI" affordance to Taliesin books that hands a student's selected passage (+ book link) off to their own logged-in AI.

**Architecture:** One new `code-enhance` JS fragment (`19-ask-ai.js`) + CSS in `base.css`, wired into the existing `CODE_ENHANCE_JS` bundle. Two pure functions carry the logic (math-aware text extraction + prompt composition) and are asserted directly in the browser; the DOM layer (selection popover, per-heading button, focus-trapped composer dialog) reuses existing helpers (`taliCopyText`, `04-focus-trap.js`, `taliReaderMenu`). No server changes: the "read the book" link is the existing canonical URL; Tier A/B keys off the existing `url:`.

**Tech Stack:** Rust (edition 2024) for build + corpus tests; vanilla JS (`// @ts-check`, tsc-gated, no build step) for the client; CSS custom properties (`--tali-*`); chrome-devtools MCP for browser verification.

## Global Constraints

- **No backend, no API key, no author cost, no moderation.** Built book stays a static artifact. (spec §1)
- **Read-only:** never write back to source. (spec §1)
- **Zero new `_site.yml` config.** Enabled on books (`chapters:` present); Tier A/B keys off existing `url:`. (spec §1, §9-7)
- **One reader-local `localStorage` key** `tali-askai`, storing only `{v, provider, ack, picked_at}` — never any passage/question/answer/book content. (spec §5)
- **Owned naming:** `tali-` prefix for storage/classes/globals; do not touch frozen `qmd-theme` keys. (spec §5)
- **No reliance on auto-submit;** clipboard-write is the universal floor under every provider. (spec §4)
- **Encoded URL budget:** compact deep-link kept `< 1900` `encodeURIComponent` chars. (spec §3)
- **Provider set (fixed):** `chatgpt`, `perplexity`, `google` (AI Mode), `claude` (paste-primary), `copy`. Verbatim deep-links in spec §4. (spec §4, §12)
- **Skip decks** (`if (document.querySelector('.tali-deck')) return;`) and be **idempotent** (guard with a `data-` attr), like every reader enhancer. (spec §7)
- **rustfmt** runs on save (PostToolUse hook); CI enforces `cargo fmt`. Both `tsc` configs (web-client + assets) must stay green.

## File Structure

| File | Responsibility | Create/Modify |
|---|---|---|
| `crates/core/assets/js/code-enhance/19-ask-ai.js` | The whole feature: pure fns + provider table + storage + DOM | **Create** |
| `crates/core/assets/css/base.css` | `.tali-askai-*` rules (theme-aware, coarse-pointer, tap targets) | Modify (append) |
| `crates/core/src/render/mod.rs` | Add fragment to `CODE_ENHANCE_JS` `concat!` (~line 1437) | Modify |
| `crates/core/assets/js/code-enhance/09-register.js` | Register `taliInitAskAi` | Modify |
| `crates/core/src/render/tests.rs` | Extend `code_enhance_bundle_matches_fragments_in_order` (~line 3268) | Modify |
| `crates/core/assets/js/globals.d.ts` | Declare `window.taliAsk*` globals | Modify |
| `corpus/course/_site.yml` | Add `url:` so the pilot exercises Tier A | Modify |
| `crates/core/tests/ask_ai.rs` | Server-observable contract: canonical emit + asset-shipped marker | **Create** |

### Shared interfaces (locked; every task uses these exact names)

```js
// Pure, on window for testability:
window.taliAskExtractText(node)            // -> string  (math-aware text of node's contents)
window.taliAskComposePrompt(payload, tier) // -> { full, compact, deepLinkable }
//   payload = { bookTitle, sectionHeading, passage, sectionText, question, pageUrl, llmsUrl }
//   tier    = 'A' | 'B'
window.taliAskTier()                        // -> 'A' | 'B'  (reads <link rel=canonical> + ?taliAskTier override)
window.taliInitAskAi(root)                  // entry point (root defaults to document)

// Module-internal (not on window):
TALI_ASK_PROVIDERS      // { id: { label, deepLink(q)|null, paste } }
TALI_ASK_KEY = 'tali-askai'
TALI_ASK_BUDGET = 1900  // max encodeURIComponent length of the compact string
askProvider() / askSetProvider(id) / askForget()   // localStorage (defensive)
```

Marker string that proves the asset shipped (asserted in Rust): **`tali-askai`** appears literally in the bundle (class prefix + storage key), plus a build-stable sentinel comment `/*!tali-askai v1*/` at the top of the fragment.

---

### Task 1: Tier-A signal — canonical link on the course book (Rust, TDD)

Establishes the server-observable contract the client keys off: a book with `url:` emits `<link rel="canonical">`; without it, none. This is Task 1 because it needs zero JS and locks the Tier-A gate.

**Files:**
- Create: `crates/core/tests/ask_ai.rs`
- Modify: `corpus/course/_site.yml`

**Interfaces:**
- Consumes: existing `Site::discover` / build path; canonical emit at `crates/core/src/site/meta.rs:70`.
- Produces: the corpus fixture (`corpus/course` with `url:`) and the test module later tasks extend.

- [ ] **Step 1: Inspect the corpus fixture**

Run: `sed -n '1,20p' corpus/course/_site.yml` and `ls corpus/course`
Expected: a book (`chapters:` present), currently **no** `url:` key. Note the first chapter's built page name (e.g. `mle.html`) for the assertion.

- [ ] **Step 2: Write the failing test**

Create `crates/core/tests/ask_ai.rs`. Copy the `corpus_dir()` + `course()` helpers verbatim from the top of `crates/core/tests/course.rs` (integration tests are separate crates, so helpers are duplicated, not shared). `course().render_page("mle.tmd")` returns `Option<String>` = the **full page** including `<head>` (meta `social_head` → canonical is pushed into `in_header` at `crates/core/src/site/mod.rs:596`; [corpus.rs:787](../crates/core/tests/corpus.rs) already asserts a canonical link this exact way).

```rust
// crates/core/tests/ask_ai.rs
// Server-observable contract for the Ask-AI feature (spec notes/2026-07-23-ask-ai-handoff-design.md §9).
use taliesin_core::site::Site; // match the exact import course.rs uses

fn corpus_dir() -> std::path::PathBuf { /* copy verbatim from course.rs */ unimplemented!() }
fn course() -> Site { Site::discover(&corpus_dir().join("course")) }

#[test]
fn course_book_emits_canonical_link_for_tier_a() {
    let html = course().render_page("mle.tmd").expect("mle renders");
    assert!(
        html.contains(r#"<link rel="canonical" href="https://course.example.edu/mle.html">"#),
        "book with url: must emit the canonical link the Ask-AI client keys off:\n{html}"
    );
}
```

(The `corpus_dir()` body above is a marker — replace it with the real one copied from `course.rs`, which resolves the workspace `corpus/` dir.)

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p taliesin-core --test ask_ai course_book_emits_canonical_link_for_tier_a -- --nocapture`
Expected: FAIL (no `url:` yet → no canonical link).

- [ ] **Step 4: Add `url:` to the corpus book**

Add to `corpus/course/_site.yml` (top level):

```yaml
url: https://course.example.edu
```

- [ ] **Step 5: Run to verify it passes; check nothing else broke**

Run: `cargo test -p taliesin-core --test ask_ai` then `cargo test -p taliesin-core course`
Expected: PASS. If a `course.rs` assertion breaks because sitemap/robots/llms/canonical now emit, update that assertion to expect the additive output (do not remove `url:`).

- [ ] **Step 6: Commit**

```bash
git add crates/core/tests/ask_ai.rs corpus/course/_site.yml
git commit -m "test(ask-ai): pin canonical Tier-A signal; deploy corpus/course with url:"
```

---

### Task 2: Ship an empty fragment through the bundle (scaffold + guard, TDD)

Create the fragment as a deck-skipping, idempotent no-op with the sentinel, wire it into the concat, register it, extend the order-guard, declare globals. Deliverable: the asset provably ships and both `tsc` configs pass.

**Files:**
- Create: `crates/core/assets/js/code-enhance/19-ask-ai.js`
- Modify: `crates/core/src/render/mod.rs` (~1437), `crates/core/assets/js/code-enhance/09-register.js`, `crates/core/src/render/tests.rs` (~3268), `crates/core/assets/js/globals.d.ts`, `crates/core/tests/ask_ai.rs`

**Interfaces:**
- Produces: `window.taliInitAskAi(root)` (no-op for now), the `/*!tali-askai v1*/` sentinel in the bundle.

- [ ] **Step 1: Add the asset-shipped failing test**

Append to `crates/core/tests/ask_ai.rs`:

```rust
#[test]
fn ask_ai_asset_ships_in_page_bundle() {
    let html = course().render_page("mle.tmd").expect("mle renders");
    assert!(
        html.contains("/*!tali-askai v1*/"),
        "the 19-ask-ai.js fragment must be compiled into the page bundle:\n(marker missing)"
    );
}
```

Run: `cargo test -p taliesin-core --test ask_ai ask_ai_asset_ships_in_page_bundle`
Expected: FAIL (fragment not created/wired yet).

- [ ] **Step 2: Create the fragment skeleton**

Create `crates/core/assets/js/code-enhance/19-ask-ai.js`:

```js
/*!tali-askai v1*/
// @ts-check
// Ask AI — client-side hand-off to the student's own logged-in AI.
// Spec: notes/2026-07-23-ask-ai-handoff-design.md. Read-only; no backend.
(function () {
  'use strict';

  /** Entry point; registered in 09-register.js. Idempotent; skips decks. */
  function taliInitAskAi(root) {
    if (typeof document === 'undefined') return;
    if (document.querySelector('.tali-deck')) return; // decks are not reading views
    var host = document.body;
    if (!host || host.getAttribute('data-tali-askai') === 'on') return;
    host.setAttribute('data-tali-askai', 'on');
    // Wiring added in later tasks.
  }

  window.taliInitAskAi = taliInitAskAi;
})();
```

- [ ] **Step 3: Wire into the bundle concat**

In `crates/core/src/render/mod.rs`, in the `CODE_ENHANCE_JS` `concat!` (the block ending with `include_str!("../../assets/js/code-enhance/18-media.js"),` near line 1437), add **after** the `18-media.js` line:

```rust
    include_str!("../../assets/js/code-enhance/19-ask-ai.js"),
```

- [ ] **Step 4: Register the initializer**

In `crates/core/assets/js/code-enhance/09-register.js`, add a line alongside the other `reg.register(...)` calls:

```js
  reg.register(function () { taliInitAskAi(); });
```

- [ ] **Step 5: Extend the fragment-order guard**

In `crates/core/src/render/tests.rs`, find `code_enhance_bundle_matches_fragments_in_order` (~line 3268). It builds the expected bundle by `concat!`/joining the fragment list in order. Add `19-ask-ai.js` to that expected list in the same position (after `18-media.js`) so the guard matches the real concat.

Run: `cargo test -p taliesin-core code_enhance_bundle_matches_fragments_in_order`
Expected: PASS (guard now includes the new fragment).

- [ ] **Step 6: Declare the global**

In `crates/core/assets/js/globals.d.ts`, add to the `Window` interface:

```ts
    taliInitAskAi: (root?: Document | Element) => void;
```

- [ ] **Step 7: Verify asset-shipped test + both tsc**

Run: `cargo test -p taliesin-core --test ask_ai`
Expected: PASS (both canonical + marker tests).
Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json` and `cd web-client && npx -y -p typescript tsc -p jsconfig.json`
Expected: no type errors.

- [ ] **Step 8: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/src/render/mod.rs crates/core/assets/js/code-enhance/09-register.js crates/core/src/render/tests.rs crates/core/assets/js/globals.d.ts crates/core/tests/ask_ai.rs
git commit -m "feat(ask-ai): scaffold 19-ask-ai.js fragment, wire into bundle + register"
```

---

### Task 3: `taliAskExtractText` — math-aware text extraction (pure fn, browser TDD)

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`, `crates/core/assets/js/globals.d.ts`

**Interfaces:**
- Produces: `window.taliAskExtractText(node) -> string`. Consumed by Task 4's payload builder.

- [ ] **Step 1: Implement the function**

Inside the IIFE in `19-ask-ai.js`, above `taliInitAskAi`:

```js
  /**
   * Visible text of a node's contents, math-aware: KaTeX renders its source twice
   * (MathML <annotation> + glyph spans), so raw textContent doubles it. We emit the
   * LaTeX from the annotation ($…$, or $$…$$ inside .katex-display) and skip glyphs.
   * Code (<pre>/<code>) is kept verbatim. Everything else is visible text with a space
   * at each element boundary so adjacent blocks stay word-separated.
   * @param {Node} node
   * @returns {string}
   */
  function taliAskExtractText(node) {
    var out = [];
    walk(node, out);
    return out.join('').replace(/[ \t]+/g, ' ').replace(/ *\n */g, '\n').trim();
  }

  function walk(node, out) {
    if (node.nodeType === 3 /* text */) { out.push(node.nodeValue); return; }
    if (node.nodeType !== 1 /* element */) return;
    var el = /** @type {Element} */ (node);
    if (el.classList && el.classList.contains('katex')) {
      var ann = el.querySelector('annotation[encoding="application/x-tex"]');
      var tex = ann ? (ann.textContent || '').trim() : '';
      if (tex) {
        var display = !!el.closest('.katex-display');
        out.push(display ? ('\n$$' + tex + '$$\n') : ('$' + tex + '$'));
      }
      return; // never descend into the doubled render
    }
    var tag = el.tagName;
    if (tag === 'PRE' || tag === 'CODE') { out.push(el.textContent || ''); return; }
    if (tag === 'SCRIPT' || tag === 'STYLE') return;
    out.push(' ');
    for (var c = el.firstChild; c; c = c.nextSibling) walk(c, out);
    out.push(' ');
  }

  window.taliAskExtractText = taliAskExtractText;
```

Add to `globals.d.ts`: `taliAskExtractText: (node: Node) => string;`

- [ ] **Step 2: Type-check**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors.

- [ ] **Step 3: Build + preview for browser assertion**

Run: `cargo build` then start preview: `cargo run -p taliesin-server -- preview corpus/course 4388` (background). Open `http://localhost:4388/mle.html` (or the first chapter) in chrome-devtools MCP.

- [ ] **Step 4: Assert the pure fn in the browser**

Via chrome-devtools `evaluate_script`, run and check each returns the expected value:

```js
// inline math
(() => { const d=document.createElement('div');
  d.innerHTML = 'Energy <span class="katex"><span class="katex-mathml"><math><semantics><annotation encoding="application/x-tex">E=mc^2</annotation></semantics></math></span><span class="katex-html">E=mc2</span></span> is conserved.';
  return window.taliAskExtractText(d); })()
// Expected: "Energy $E=mc^2$ is conserved." (no doubled "E=mc2")
```

```js
// display math
(() => { const d=document.createElement('div');
  d.innerHTML = '<span class="katex-display"><span class="katex"><span class="katex-mathml"><math><semantics><annotation encoding="application/x-tex">\\hat\\theta=\\arg\\max_\\theta \\mathcal{L}(\\theta)</annotation></semantics></math></span></span></span>';
  return window.taliAskExtractText(d).includes('$$\\hat\\theta'); })()
// Expected: true
```

Also select a real equation on the live page and confirm `taliAskExtractText(getSelection().getRangeAt(0).cloneContents())` returns LaTeX, not doubled glyphs. Record the outputs in the commit message.

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/assets/js/globals.d.ts
git commit -m "feat(ask-ai): math-aware taliAskExtractText (LaTeX from KaTeX annotation)"
```

---

### Task 4: Tier detection + `taliAskComposePrompt` (pure fns, browser TDD)

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`, `crates/core/assets/js/globals.d.ts`

**Interfaces:**
- Consumes: `taliAskExtractText` (Task 3).
- Produces: `window.taliAskTier() -> 'A'|'B'`, `window.taliAskComposePrompt(payload, tier) -> {full, compact, deepLinkable}`, and the `payload` shape. Consumed by the DOM tasks (6-8).

- [ ] **Step 1: Implement tier detection**

```js
  var TALI_ASK_BUDGET = 1900;

  /** Tier A only when the canonical URL is a real public http(s) host. */
  function taliAskTier() {
    try {
      var params = new URLSearchParams(location.search);
      if (params.get('taliAskTier') === 'B') return 'B';
    } catch (e) {}
    var link = document.querySelector('link[rel="canonical"]');
    var href = link && link.getAttribute('href');
    if (!href) return 'B';
    var u;
    try { u = new URL(href, location.href); } catch (e) { return 'B'; }
    if (u.protocol !== 'http:' && u.protocol !== 'https:') return 'B';
    var h = u.hostname;
    if (h === 'localhost' || h === '0.0.0.0' || /\.local$/.test(h)) return 'B';
    if (/^127\./.test(h) || /^10\./.test(h) || /^192\.168\./.test(h) ||
        /^172\.(1[6-9]|2\d|3[01])\./.test(h)) return 'B';
    return 'A';
  }
  window.taliAskTier = taliAskTier;
```

- [ ] **Step 2: Implement prompt composition**

```js
  function trimEncodedToBudget(build, target) {
    // build(len) -> string using the first `len` chars of the trimmable part; binary/greedy
    // shrink at word boundaries until encodeURIComponent(result).length <= target.
    var s = build(Infinity);
    if (encodeURIComponent(s).length <= target) return s;
    var lo = 0, hi = build.maxLen, best = build(0);
    while (lo <= hi) {
      var mid = (lo + hi) >> 1;
      var cand = build(mid);
      if (encodeURIComponent(cand).length <= target) { best = cand; lo = mid + 1; }
      else hi = mid - 1;
    }
    return best;
  }

  function atWordBoundary(text, n) {
    if (n >= text.length) return text;
    var cut = text.slice(0, n);
    var sp = cut.lastIndexOf(' ');
    return (sp > 0 ? cut.slice(0, sp) : cut).replace(/\s+$/, '') + '…';
  }

  /**
   * @param {{bookTitle:string,sectionHeading:string,passage:string,sectionText:string,question:string,pageUrl:string,llmsUrl:string}} p
   * @param {'A'|'B'} tier
   * @returns {{full:string, compact:string, deepLinkable:boolean}}
   */
  function taliAskComposePrompt(p, tier) {
    var q = p.question && p.question.trim() ? p.question.trim() : 'Explain this passage in simpler terms.';
    var linkBlock = (tier === 'A' && p.pageUrl)
      ? '\n\nIf you can browse the web, you may also open this page for fuller context and answer using it; otherwise answer from the passage and section above:\n' + p.pageUrl +
        (p.llmsUrl ? '\n(Whole-book map, if you need it: ' + p.llmsUrl + ')' : '')
      : '';
    var full =
      'I\'m reading "' + p.bookTitle + '", section "' + p.sectionHeading + '".\n\n' +
      'Passage I highlighted:\n"""\n' + p.passage + '\n"""\n\n' +
      'Surrounding section (for context):\n"""\n' + p.sectionText + '\n"""\n\n' +
      'My question: ' + q + linkBlock;

    var linkTail = (tier === 'A' && p.pageUrl) ? ' If you can browse, more at ' + p.pageUrl + '.' : '';
    var head = 'From "' + p.bookTitle + '" § "' + p.sectionHeading + '". Passage: "';
    var fixedTail = function (passage, section) {
      return passage + '". Context: "' + section + '". ' + q + '.' + linkTail;
    };
    // Trim section first, then passage; suppress deep-link if passage+question alone overflow.
    var passageBuild = { maxLen: p.passage.length, fn: function (n) { return atWordBoundary(p.passage, n); } };
    function compactWith(passageLen, sectionLen) {
      return head + atWordBoundary(p.passage, passageLen) + '". Context: "' +
             atWordBoundary(p.sectionText, sectionLen) + '". ' + q + '.' + linkTail;
    }
    // 1) full section trim
    var sBuild = function (n) { return compactWith(p.passage.length, n); };
    sBuild.maxLen = p.sectionText.length;
    var compact = trimEncodedToBudget(sBuild, TALI_ASK_BUDGET);
    var deepLinkable = true;
    if (encodeURIComponent(compact).length > TALI_ASK_BUDGET) {
      // section trimmed to zero still overflows -> trim passage
      var pBuild = function (n) { return compactWith(n, 0); };
      pBuild.maxLen = p.passage.length;
      compact = trimEncodedToBudget(pBuild, TALI_ASK_BUDGET);
      if (encodeURIComponent(compact).length > TALI_ASK_BUDGET) deepLinkable = false;
    }
    return { full: full, compact: compact, deepLinkable: deepLinkable };
  }
  window.taliAskComposePrompt = taliAskComposePrompt;
```

Add to `globals.d.ts`:
```ts
    taliAskTier: () => 'A' | 'B';
    taliAskComposePrompt: (p: {bookTitle:string;sectionHeading:string;passage:string;sectionText:string;question:string;pageUrl:string;llmsUrl:string}, tier: 'A'|'B') => {full:string;compact:string;deepLinkable:boolean};
```

- [ ] **Step 3: Type-check**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors. (If `trimEncodedToBudget`'s `build.maxLen` trips `// @ts-check`, type `build` as `{(n:number):string; maxLen:number}` via a JSDoc `@typedef`.)

- [ ] **Step 4: Build + assert in browser**

Run: `cargo build`, ensure preview is running, reload the page, then via `evaluate_script`:

```js
// Tier A includes the link block; Tier B omits it
(() => { const p={bookTitle:'Prob',sectionHeading:'MLE',passage:'The likelihood.',sectionText:'A section.',question:'why?',pageUrl:'https://course.example.edu/mle.html',llmsUrl:'https://course.example.edu/llms.txt'};
  const a=window.taliAskComposePrompt(p,'A'), b=window.taliAskComposePrompt(p,'B');
  return {aHasLink:a.full.includes('course.example.edu/mle.html'), bHasLink:b.full.includes('course.example.edu'), aTail:a.compact.includes('more at')}; })()
// Expected: {aHasLink:true, bHasLink:false, aTail:true}
```

```js
// Encoded budget respected on a math/punctuation-heavy passage; deep-link suppressed when it can't fit
(() => { const big='x'.repeat(4000); const p={bookTitle:'B',sectionHeading:'S',passage:big,sectionText:big,question:'q',pageUrl:'https://c.edu/p.html',llmsUrl:''};
  const r=window.taliAskComposePrompt(p,'A');
  return {enc:encodeURIComponent(r.compact).length, deepLinkable:r.deepLinkable}; })()
// Expected: enc <= 1900; deepLinkable === false (passage alone overflows)
```

```js
// live Tier detection: on localhost with a public canonical, Tier A; ?taliAskTier=B forces B
window.taliAskTier() // Expected: 'A'  (canonical host is course.example.edu, not localhost)
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/assets/js/globals.d.ts
git commit -m "feat(ask-ai): tier detection + encoded-budget prompt composition"
```

---

### Task 5: Provider table + defensive `localStorage` (browser TDD)

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`

**Interfaces:**
- Produces (module-internal): `TALI_ASK_PROVIDERS`, `askProvider()`, `askSetProvider(id)`, `askForget()`, `askAck()`, `askSetAck()`. Consumed by DOM tasks. Exposed on `window.__taliAskTest` **only** for assertion (documented as test-only).

- [ ] **Step 1: Implement the provider table + storage**

```js
  var TALI_ASK_KEY = 'tali-askai';

  // Deep-link builders take the already-encoded compact string. `paste:true` => open bare + rely on clipboard.
  var TALI_ASK_PROVIDERS = {
    chatgpt:    { label: 'ChatGPT',        deepLink: function (q) { return 'https://chatgpt.com/?q=' + q + '&hints=search'; }, paste: false },
    perplexity: { label: 'Perplexity',     deepLink: function (q) { return 'https://www.perplexity.ai/search/?q=' + q; },     paste: false },
    google:     { label: 'Google AI Mode', deepLink: function (q) { return 'https://www.google.com/search?udm=50&q=' + q; },  paste: false },
    claude:     { label: 'Claude',         deepLink: function () { return 'https://claude.ai/new'; },                          paste: true  },
    copy:       { label: 'Copy prompt',    deepLink: null,                                                                     paste: true  }
  };

  function askRead() {
    try {
      var raw = localStorage.getItem(TALI_ASK_KEY);
      if (!raw) return null;
      var o = JSON.parse(raw);
      if (!o || o.v !== 1 || !TALI_ASK_PROVIDERS[o.provider]) return null;
      return o;
    } catch (e) { return null; }
  }
  function askWrite(o) { try { localStorage.setItem(TALI_ASK_KEY, JSON.stringify(o)); } catch (e) {} }
  function askProvider() { var o = askRead(); return o ? o.provider : null; }
  function askSetProvider(id) {
    if (!TALI_ASK_PROVIDERS[id]) return;
    var o = askRead() || { v: 1, ack: false };
    o.v = 1; o.provider = id; o.picked_at = Date.now();
    askWrite(o);
  }
  function askAck() { var o = askRead(); return !!(o && o.ack); }
  function askSetAck() { var o = askRead() || { v: 1 }; o.v = 1; o.ack = true; askWrite(o); }
  function askForget() { try { localStorage.removeItem(TALI_ASK_KEY); } catch (e) {} }

  // Test-only surface (documented): lets the browser loop assert storage without UI.
  window.__taliAskTest = { providers: TALI_ASK_PROVIDERS, provider: askProvider, set: askSetProvider,
    ack: askAck, setAck: askSetAck, forget: askForget };
```

Add to `globals.d.ts` a loose declaration: `__taliAskTest?: any;`

- [ ] **Step 2: Type-check**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors.

- [ ] **Step 3: Build + assert storage round-trip in browser**

Run: `cargo build`, reload preview, then `evaluate_script`:

```js
(() => { const t=window.__taliAskTest; t.forget();
  const empty=t.provider(); t.set('perplexity'); const set=t.provider();
  t.setAck(); const ack=t.ack(); t.forget(); const gone=t.provider();
  // defensive: corrupt value -> null, no throw
  localStorage.setItem('tali-askai','{not json'); const bad=t.provider(); t.forget();
  return {empty, set, ack, gone, bad}; })()
// Expected: {empty:null, set:'perplexity', ack:true, gone:null, bad:null}
```

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/assets/js/globals.d.ts
git commit -m "feat(ask-ai): provider table + defensive localStorage (provider/ack)"
```

---

### Task 6: Composer dialog — picker, consent, question input, prompt preview + Copy (browser-verified)

The focus-trapped dialog is the single home for the question input and every provider path. Builds on Tasks 3-5. CSS for the dialog is added here.

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`, `crates/core/assets/css/base.css`

**Interfaces:**
- Consumes: `taliAskComposePrompt`, `taliAskTier`, provider/storage fns, `taliCopyText` (`01-registry.js:46`), `04-focus-trap.js` (open one and note its exact API before use).
- Produces (module-internal): `openAskComposer(payload)` — builds/open the dialog for a given payload.

- [ ] **Step 1: Note the focus-trap API**

Run: `sed -n '1,60p' crates/core/assets/js/code-enhance/04-focus-trap.js`
Record the exact function name/signature it exposes (e.g. `taliFocusTrap(el)` returning a release fn). Use it verbatim below; do **not** invent an API.

- [ ] **Step 2: Build the composer**

Add `openAskComposer(payload)` to `19-ask-ai.js`. It creates (once, reused) a `role="dialog" aria-modal="true"` element `.tali-askai-dialog` containing, in order:
1. `.tali-askai-consent` (shown only when `!askAck()`): the exact disclosure copy from spec §8.1 with **[Continue]** (calls `askSetAck()`, hides consent) and **[Cancel]** (closes dialog).
2. `.tali-askai-providers` (shown when `!askProvider()`): one `<button>` per `TALI_ASK_PROVIDERS` id (label = `.label`), each calling `askSetProvider(id)` then re-rendering to the "ready" state. Includes the `copy` tile.
3. Ready state (when a provider is stored): a primary `<button class="tali-askai-go">Ask {label}</button>` + a caret `<button class="tali-askai-caret" aria-haspopup="menu">▾</button>` whose menu offers each other provider (switch = `askSetProvider` + promote) and **"Forget my choice"** (`askForget()` → back to picker). An inline `.tali-askai-using` label: "Using {label} · change".
4. `<label>` + `<textarea class="tali-askai-q">` (optional question; `placeholder="Ask about this… (e.g. explain this passage in simpler terms)"`), autofocused once a provider is known.
5. `.tali-askai-preview` read-only: the highlighted passage.
6. `<details class="tali-askai-full">` "Full prompt" containing the composed `full` text + an explicit **Copy** button (`taliCopyText(full, ok, fail)`).
7. `.tali-askai-note`: the persistent one-liner (spec §8.2) + "Opens {provider} in a new tab. The full prompt is also on your clipboard — paste with Cmd/Ctrl+V if the box is empty."

Behavior: recompute `full`/`compact` from `taliAskComposePrompt(payload, taliAskTier())` whenever the question `input` fires. Apply the focus trap on open; **Esc** closes and returns focus to `payload.trigger`. The "Ask {label}" click handler is **stubbed** here (real hand-off in Task 8) — for now it may just `console.log` — so this task's deliverable is the dialog itself.

- [ ] **Step 3: Add dialog CSS**

Append to `crates/core/assets/css/base.css` (use `--tali-*` tokens; must work in light + dark — the file already has both):

```css
/* Ask AI ------------------------------------------------------------------ */
.tali-askai-dialog { position: fixed; z-index: 60; max-width: 32rem; width: min(92vw, 32rem);
  inset: auto; background: var(--tali-bg); color: var(--tali-fg);
  border: 1px solid var(--tali-border); border-radius: var(--tali-radius-md);
  box-shadow: 0 8px 40px rgb(0 0 0 / 0.28); padding: 1rem; }
.tali-askai-dialog[hidden] { display: none; }
.tali-askai-providers { display: flex; flex-wrap: wrap; gap: 0.5rem; }
.tali-askai-providers button, .tali-askai-go, .tali-askai-caret {
  min-height: 44px; min-width: 44px; padding: 0.4rem 0.8rem;
  border: 1px solid var(--tali-border); border-radius: var(--tali-radius-md); background: transparent;
  color: inherit; cursor: pointer; }
.tali-askai-q { width: 100%; min-height: 3.5rem; margin: 0.6rem 0; }
.tali-askai-preview { font-size: 0.9em; color: var(--tali-muted); border-left: 3px solid var(--tali-accent); padding-left: 0.6rem; }
.tali-askai-note { font-size: 0.8em; color: var(--tali-muted); margin-top: 0.6rem; }
.tali-askai-backdrop { position: fixed; inset: 0; z-index: 59; background: var(--tali-scrim); }
```

Tokens used are all confirmed present in `crates/core/assets/css/tokens.css` (`--tali-bg`, `--tali-fg`, `--tali-border`, `--tali-accent`, `--tali-muted`, `--tali-radius-md`, `--tali-scrim`, `--tali-dur`). Do not invent new token names; if a new shade is genuinely needed, add it to `tokens.css` in both light + dark first.

- [ ] **Step 4: Build + browser-verify the dialog at 3 viewports**

Run: `cargo build`, reload preview. Via chrome-devtools MCP, open the composer (temporarily call `openAskComposer` from the console with a fixed payload), and verify at 390×844, 1440×900, 900×1440:
- First run shows consent → Continue hides it and reveals the picker.
- Picking a provider reveals "Ask {label} ▾" + question textarea (autofocused).
- Typing in the textarea updates the "Full prompt" contents.
- Copy button copies; Esc closes and returns focus to the trigger.
Screenshot each viewport; confirm no console errors and dark-mode legibility (toggle theme).

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/assets/css/base.css
git commit -m "feat(ask-ai): focus-trapped composer dialog (picker, consent, question, preview)"
```

---

### Task 7: Triggers — selection popover (desktop) + per-heading section button (browser-verified)

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`, `crates/core/assets/css/base.css`

**Interfaces:**
- Consumes: `openAskComposer` (Task 6), `taliAskExtractText` (Task 3). Uses `04-anchor` heading rows (`02-anchor-links.js` pattern) for placement, but **does not** inherit its hover-reveal CSS.
- Produces: the two entry points, both building a `payload` and calling `openAskComposer`.

- [ ] **Step 1: Payload builder**

Add a helper that assembles the `payload`: `bookTitle` from `document.title` or the book topbar; `sectionHeading` from the nearest preceding heading; `passage` = `taliAskExtractText(range.cloneContents())` (selection) or the section text (button); `sectionText` = `taliAskExtractText(sectionEl)`; `pageUrl` = canonical href or `location.href`; `llmsUrl` = canonical base + `/llms.txt` (or ''); `trigger` = the activating element (for focus return).

- [ ] **Step 2: Selection popover (desktop mouse only)**

In `taliInitAskAi`, if `matchMedia('(pointer: coarse)').matches` is **false**, attach a debounced `selectionchange`+`mouseup`/`keyup` listener. On a non-empty, settled selection inside the reading column, show a single `.tali-askai-pop` button `role="toolbar" aria-label="Ask AI"` positioned from `range.getBoundingClientRect()` (edge-flipped, offset below the selection). Clicking it builds the selection payload and calls `openAskComposer`. Popover is **Dismissible** (Esc), **Persistent** (until Esc / outside-click / selection collapse / scroll / resize) per WCAG 1.4.13.

- [ ] **Step 3: Per-heading section button (always; touch-primary)**

For each section heading in the reading column (mirror the selector `02-anchor-links.js` uses for the `#` permalink row), append a button `.tali-askai-heading` "Ask AI about this section" (accessible name via `aria-label` including the heading text), scoped to the enclosing section (siblings until the next same-or-higher heading). It opens the composer with the section payload.

- [ ] **Step 4: CSS — persistent on touch, hover-reveal on fine pointers**

Append to `base.css`:

```css
.tali-askai-pop { position: absolute; z-index: 61; min-height: 32px; padding: 0.25rem 0.6rem;
  border: 1px solid var(--tali-border); border-radius: var(--tali-radius-md); background: var(--tali-bg); color: inherit;
  box-shadow: 0 2px 10px rgb(0 0 0 / 0.2); cursor: pointer; }
.tali-askai-heading { margin-left: 0.4rem; font-size: 0.8em; opacity: 0; transition: opacity var(--tali-dur);
  min-width: 24px; min-height: 24px; border: 0; background: transparent; color: var(--tali-accent); cursor: pointer; }
:is(h1,h2,h3,h4,h5,h6):hover .tali-askai-heading,
.tali-askai-heading:focus-visible { opacity: 1; }
@media (pointer: coarse) { .tali-askai-heading { opacity: 1; min-width: 44px; min-height: 44px; } }
```

- [ ] **Step 5: Build + browser-verify triggers at 3 viewports**

Run: `cargo build`, reload preview.
- **1440×900 (mouse):** select a sentence → popover appears → click → composer opens with that passage. Select an equation → passage shows LaTeX, not doubled glyphs.
- **390×844 (touch emulation):** custom popover is **suppressed**; the per-heading button is **visible** (opacity 1) and ≥44×44; tap → composer opens scoped to the section.
- **900×1440:** heading button visible/tappable; no layout overflow.
Screenshot each; no console errors.

- [ ] **Step 6: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js crates/core/assets/css/base.css
git commit -m "feat(ask-ai): selection popover + persistent per-heading section trigger"
```

---

### Task 8: Popup-safe hand-off + Settings row (browser-verified)

**Files:**
- Modify: `crates/core/assets/js/code-enhance/19-ask-ai.js`

**Interfaces:**
- Consumes: `taliAskComposePrompt`, provider table, `taliCopyText`, `taliReaderMenu.addSection` (`13-reader-menu.js:104`).
- Produces: the real "Ask {label}" action + a Settings "AI hand-off" section.

- [ ] **Step 1: Implement the hand-off (critical ordering)**

Replace the Task-6 stub. In the "Ask {label}" click handler, **with no `await` before `window.open`**:

```js
  function askHandOff(provider, composed) {
    // 1) initiate the clipboard write (do NOT await) — bound while the book tab is focused
    try { navigator.clipboard && navigator.clipboard.writeText(composed.full); }
    catch (e) {}
    // taliCopyText fallback also runs for insecure/file:// contexts:
    taliCopyText(composed.full, function () {}, function () {});
    // 2) open the provider synchronously in the same gesture
    var prov = TALI_ASK_PROVIDERS[provider];
    var url = (!prov.paste && composed.deepLinkable && prov.deepLink)
      ? prov.deepLink(encodeURIComponent(composed.compact))
      : (prov.deepLink ? prov.deepLink('') : null);
    if (url) window.open(url, '_blank', 'noopener');
    // copy-only provider: nothing to open; the composer already shows "copied".
  }
```

Note: `taliCopyText` uses the `execCommand` fallback on `file://`; calling both is intentional belt-and-suspenders. Do not wrap the open in a promise `.then`.

- [ ] **Step 2: Settings "AI hand-off" row**

After init, call `window.taliReaderMenu.addSection('AI hand-off', node, onOpen)` (use the exact signature confirmed in Task 6 Step 1 / `globals.d.ts:24`) with a small control: the current provider label + a **Change** button (reopens the picker) + **Forget my choice** (`askForget()`). Guard for `window.taliReaderMenu` existing.

- [ ] **Step 3: Build + browser-verify hand-off**

Run: `cargo build`, reload preview.
- Pick **Copy prompt** → click → assert clipboard: `await navigator.clipboard.readText()` equals the full prompt.
- Pick **ChatGPT** → click → a new tab opens to `chatgpt.com/?q=…&hints=search`; **and** the clipboard still holds the full prompt (read it back on the book tab). Confirm via network/pages list that `window.open` fired (no popup-block console warning).
- Force a > budget passage → `deepLinkable` false → ChatGPT opens **bare** (`chatgpt.com/?q=` empty or new-chat) and the composer note tells the student to paste.
- Settings gear → "AI hand-off" row shows current provider; **Forget** returns to first-run picker on next open.
Screenshot; no console errors.

- [ ] **Step 4: Guard against an `await` regression**

Grep the fragment to prove the ordering constraint holds:

Run: `grep -n "await" crates/core/assets/js/code-enhance/19-ask-ai.js`
Expected: **no** `await` inside `askHandOff` (the function must be synchronous). If any exists, refactor it out.

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/code-enhance/19-ask-ai.js
git commit -m "feat(ask-ai): popup-safe hand-off (clipboard-then-open) + Settings AI hand-off row"
```

---

### Task 9: Full verification pass + fmt + Tier-B degradation (browser-verified)

**Files:** none new (verification + any fixes uncovered).

- [ ] **Step 1: Whole test suite + type-checks + fmt**

Run, in order:
- `cargo test -p taliesin-core` → all green (corpus invariants + `ask_ai.rs` + fragment guard).
- `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json` → clean.
- `cd web-client && npx -y -p typescript tsc -p jsconfig.json` → clean.
- `cargo fmt --check` → clean (the save-hook should already keep it clean).

- [ ] **Step 2: Tier-B degradation in the browser**

Run: preview `corpus/course`, open `http://localhost:4388/mle.html?taliAskTier=B`.
Verify: `window.taliAskTier()` returns `'B'`; a composed prompt **omits** both link lines; the composer messaging is the Tier-B copy ("Sends the selected passage to your AI."). Screenshot.

- [ ] **Step 3: Run the explicit browser-test checklist (spec §9)**

Confirm each, at 390×844 / 1440×900 / 900×1440: clipboard contains the full prompt after hand-off; no `await` before `window.open`; selected equation → LaTeX not doubled glyphs; compact URL `< 1900` encoded on a math-heavy passage; heading button visible + ≥24×24 (≥44 on coarse) tappable at 390×844; Esc returns focus from composer to trigger; dark + light both legible; decks unaffected (open a deck in the corpus, confirm no Ask-AI chrome).

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "test(ask-ai): full verification pass (Tier-B degradation, a11y, viewports)"
```

---

## Self-Review

**Spec coverage** (spec §→task): §3 prompt/math → T3,T4; §4 provider table → T5,T8; §5 memory/reversibility → T5,T6,T8; §6 tiers/degradation → T4,T9; §7 interaction/a11y/mobile → T6,T7; §8 privacy/consent/clipboard → T6,T8; §9 impl-by-file → T1-T8; §10 corpus pin → T1,T2; §12 verified provider facts → T5 (table) . No spec section is unimplemented.

**Placeholders:** none — every code step shows real code; DOM-construction tasks (T6,T7) specify exact elements, classes, ARIA, ordering, and behavior rather than a placeholder, and hand the implementer the composer/popover structure with acceptance in the browser step.

**Type consistency:** `taliAskComposePrompt(payload, tier)` returns `{full, compact, deepLinkable}` used identically in T6/T8; `payload` shape defined in T7 Step 1 matches the JSDoc in T4; storage fns `askProvider/askSetProvider/askForget/askAck/askSetAck` consistent across T5/T6/T8; provider ids `chatgpt|perplexity|google|claude|copy` consistent T5→T8.

**Verified against source (not guessed):** helper is `course().render_page("mle.tmd")` returning the full page with `<head>` (canonical pushed at `site/mod.rs:596`); canonical string is exactly `https://course.example.edu/mle.html`; `corpus/course` is a book (`chapters:` present) with no `url:` yet; CSS tokens `--tali-bg/-fg/-border/-accent/-muted/-radius-md/-scrim/-dur` all exist in `tokens.css`; bundle concat + order-guard at `render/mod.rs:1437`/`tests.rs:3268`; `taliCopyText` (`01-registry.js:46`), `taliReaderMenu.addSection` (`13-reader-menu.js:104`, declared `globals.d.ts:24`), `04-focus-trap.js` all present. The one thing the implementer still confirms live: `04-focus-trap.js`'s exact exported API (Task 6 Step 1).
