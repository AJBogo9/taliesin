# Ask AI — client-side, backendless AI hand-off for a Taliesin course textbook

**Status:** design, awaiting author review (brainstorming → writing-plans)
**Date:** 2026-07-23
**Scope:** a reading-time affordance that lets a student ask *their own* AI to clarify
part of a Taliesin book, with the author hosting **no** AI infrastructure.

---

## 1. Goal & non-goals

**Goal.** A student reading a Taliesin course textbook selects a passage (or presses a
per-section button), optionally types their own question, and is handed off to **their
own logged-in AI** (ChatGPT / Claude / Perplexity / Google AI Mode) with the passage +
enclosing section inlined and — because the book is publicly hosted — a link to the page
so a browsing-capable AI can read the fuller book context. The composed prompt lets the
AI "fill in the gaps" the student didn't understand.

**Hard constraints (load-bearing, do not relax).**
- The author hosts **no AI backend**, holds **no API key**, pays **nothing per student**,
  and runs **no moderation**. The built book stays a static, self-contained artifact.
- Every AI call runs on the **student's** subscription in the **student's** browser session.
- Read-only: the feature never writes back to source (single-editing-surface invariant).
- **Zero new `_site.yml` config** (perfect-the-default). One reader-local `localStorage`
  key (provider choice + consent), exempt like the theme toggle.
- Corpus-pinned with a test.

**Non-goals (v1).** No hosted RAG / chat widget, no answers rendered in-page, no browser
extension/bookmarklet, no conversation history, no analytics on student questions, no
tutor-prompt injection or content filtering (out of scope by design *and* by the
no-backend constraint), no per-page `.md` artifact (see §11), no reliance on auto-submit.

## 2. Locked decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Architecture | Client-side hand-off to the student's own logged-in AI. Zero author infra. |
| Interaction | Text-selection popover (desktop) **+ per-heading "Ask AI about this section" button** (touch/keyboard baseline). |
| Framing | **Raw passthrough** — no authorial tutor prompt; the student types their own question. |
| Book link (refinement 1) | Prompt references the **page's own canonical URL** so a browsing AI can read it (see §3, §11). |
| Provider memory (refinement 2) | Remember the student's provider; default to it; **reversible** (change / forget). |
| Hosting | **Public website** → Tier A (book-linked) is the normal case. |
| Enablement | **Books only** (projects with `chapters:`), reusing the existing book/website gate. No new config. |

## 3. Prompt template + book link

Two strings are built from one payload. The **full prompt** is written to the clipboard and
shown in the composer's text area (unlimited length; the always-works floor). The **compact
prompt** is URL-encoded into a provider deep-link, budgeted on **encoded** length.

### Full prompt (clipboard + composer — the tier that always works)

Passage-first and link-*conditional*, so a browsing-off model is explicitly told to answer
from the inlined text (avoids the "confabulate a page reading" failure mode):

```
I'm reading "{BOOK_TITLE}", section "{SECTION_HEADING}".

Passage I highlighted:
"""
{SELECTED_PASSAGE}
"""

Surrounding section (for context):
"""
{ENCLOSING_SECTION_TEXT}
"""

My question: {STUDENT_QUESTION || "Explain this passage in simpler terms."}

If you can browse the web, you may also open this page for fuller context and
answer using it; otherwise answer from the passage and section above:
{PAGE_CANONICAL_URL}
(Whole-book map, if you need it: {LLMS_TXT_URL})
```

- `{PAGE_CANONICAL_URL}` ← the page's `<link rel="canonical">` (emitted at
  [meta.rs:70](../crates/core/src/site/meta.rs) when `url:` is set). `{LLMS_TXT_URL}` ←
  `{canonical-base}/llms.txt` (already emitted, same `url:` gate). Both lines present only
  in **Tier A**; both omitted in Tier B.
- `{SELECTED_PASSAGE}` / `{ENCLOSING_SECTION_TEXT}` come from the **math-aware extractor**
  (§ below), never raw `getSelection().toString()`.

### Compact prompt (deep-link URL — best-effort, keeps a floor)

Keeps a trimmed section floor so the undetectable SSO/LMS-401 case (a public-*looking* host
the AI still can't fetch) never degrades to "truncated passage + dead link":

```
From "{BOOK_TITLE}" § "{SECTION_HEADING}". Passage: "{PASSAGE}". Context: "{SECTION_TRIMMED}". {QUESTION}. If you can browse, more at {PAGE_CANONICAL_URL}.
```

**Budget (encoded, whole-string).**
- Compute `encodeURIComponent(candidate).length` of the *entire* compact string plus the
  provider base; keep it **under ~1900** (headroom below the ~2000-char cross-infrastructure
  URL ceiling; some browsers/providers truncate silently above it).
- Truncate on **encoded** length, not raw: math passages full of `"`, `\`, `{`, `^`, `=`,
  and non-ASCII (`§`→`%C2%A7`) inflate 3–6×. Trim `{SECTION_TRIMMED}` first, then `{PASSAGE}`,
  each at a **word boundary** with an ellipsis.
- If passage **+ question alone** still overflow after trimming the section to zero,
  **suppress the deep-link and force copy-paste** (open the provider's bare new-chat URL; the
  full prompt is on the clipboard + in the composer). Never emit a mid-token, truncated URL.

### Math-aware text extraction (fidelity fix)

Pure, testable `taliAskExtractText(range | element)`:
- For each `.katex` element, emit the LaTeX from
  `el.querySelector('annotation[encoding="application/x-tex"]').textContent`, wrapped
  `$$…$$` inside `.katex-display`, else `$…$`. **Never** emit the visual glyph spans
  (KaTeX renders the source twice → raw `getSelection()` doubles/garbles it; documented by
  the codebase's own test in `llms.rs`).
- Code cells / `<pre>`: keep text verbatim (optionally fenced); not double-rendered, safe.
- Everything else: visible text with tag→space boundaries (the same rule `text_content`
  uses so adjacent blocks stay word-separated).

This is the client-side analogue of `strip_katex` + `text_content`, made math-*preserving*
instead of math-*deleting*.

## 4. Provider table (the one place a break is a one-line fix)

`{q}` = `encodeURIComponent(compact)`. Verified against 2026 sources (see §12).

| Provider | Deep-link | Prefill | Auto-runs | Fetches a pasted public URL | Hand-off shape |
|---|---|---|---|---|---|
| **ChatGPT** | `https://chatgpt.com/?q={q}` (optional `&hints=search`) | Yes | No (cross-site prefill-only since Jul-2025 fix) | only if browsing on | open `?q`, clipboard floor |
| **Perplexity** | `https://www.perplexity.ai/search/?q={q}` | Yes | Yes | Yes (strong fetcher) | open `?q`, clipboard floor |
| **Google AI Mode** | `https://www.google.com/search?udm=50&q={q}` | Yes | Yes | Yes (live web) | open `?q`, clipboard floor |
| **Claude** | `https://claude.ai/new` (bare; `?q=` removed ~Oct-2025) | Unreliable → not relied on | No | only if per-chat web search on | **paste-primary**: open bare, clipboard floor, composer says "paste it" |
| **Copy prompt** | — | — | — | — | clipboard only; universal, always present |

Every path writes the full prompt to the clipboard **and** shows it in the composer, so
Claude (and any future no-param provider) degrade identically and safely. **Not shipped as
first-class** (offer nothing over "Copy", documented for the author only): Gemini *app*
(no URL prefill — route Google users to AI Mode), Copilot (prefill regressed in 2026),
Mistral Le Chat, Duck.ai.

## 5. Provider memory + reversibility

**Flow.**
1. **First run (no stored provider):** composer shows a **provider picker** (tiles + a
   universal "Copy prompt" tile), preceded once by the consent line (§7). **No pre-selected
   provider** — the student picks their own AI (consistent with raw passthrough, and avoids
   promoting an auto-submitting provider while framing "pause to think").
2. **Remembered:** the trigger's primary action becomes **`Ask {Provider} ▾`**.
3. **Reversible:** the caret menu (a) switches provider — picking one uses it *and* promotes
   it to the new default — and (b) **"Forget my choice"** → back to the picker. An inline
   **"Using {Provider} · change"** label keeps it discoverable but quiet. The same control is
   also mounted as an **"AI hand-off"** row in the existing Settings menu (next to the theme
   picker) via `window.taliReaderMenu.addSection` ([13-reader-menu.js:104](../crates/core/assets/js/code-enhance/13-reader-menu.js)),
   so a wrong default is resettable from one canonical place.

Button copy is always **"Ask {Provider}"** ("opens a ready-to-send prompt"), never "Get the
answer." Auto-submit is a *property* stated in the table, never a promise.

**`localStorage` schema.** One namespaced, versioned, defensively-read key (owned `tali-`
prefix; frozen `qmd-theme` keys untouched):

```jsonc
// key: "tali-askai"
{ "v": 1, "provider": "chatgpt", "ack": true, "picked_at": 1737600000000 }
```

- `provider ∈ {chatgpt, claude, perplexity, google, copy}`. `ack` = first-run consent.
- Read in `try/catch`; parse failure / missing → first-run picker. **Forget** =
  `removeItem('tali-askai')`.
- **Stores only the choice + ack — never any passage, question, answer, or book content.**
  Upholds "the book stores/sends nothing but a reader preference"; sits in the reader-local
  prefs exemption (like theme).

## 6. Grounding tiers + degradation

Reachability derives from the **canonical URL**, not `location.host` — so Tier A actually
composes in the `localhost` preview loop (canonical is the real public URL even when
previewing), closing the verification blind spot, while still degrading on a genuinely
private deploy.

```
canonical := document.querySelector('link[rel="canonical"]')?.href
Tier A (book-linked): canonical present AND http(s) AND host NOT
                      localhost/127.*/0.0.0.0/*.local/RFC-1918 (10.*,192.168.*,172.16–31.*)
Tier B (paste-only):  otherwise (no canonical → no url:; private/localhost canonical; file://)
```

| Tier | Condition | Payload |
|---|---|---|
| **A — public book** (the confirmed normal case) | canonical public | inline passage + section (math-safe) **and** link the page's canonical URL + optional `llms.txt` map. Deep-link where supported; clipboard always holds the full prompt. |
| **B — self-contained** | no/private canonical, `file://` | **omit both link lines**; inline passage + section as the only context. Messaging: *"Sends the selected passage to your AI."* |

**Honesty guards in every tier.**
- A manual escape hatch is always visible in the composer: *"AI didn't read the page? The
  full prompt is right here — copy and paste it."* (An SSO/LMS origin can look public yet
  401 the AI's fetch; we never auto-detect auth, and the compact prompt keeps the section
  floor precisely for that undetectable case.)
- **No AI account / never picked a provider:** strictly opt-in, additive. No auto-popups, no
  nag, no content gating; a non-account student just lands on the provider's login wall —
  their choice, not a book dependency.

**Dev/testability override:** `?taliAskTier=B` (or a `localStorage` flag the browser loop
sets) forces Tier B on a public build so the paste-only path is exercisable. The prompt
builder is a pure function, asserted directly.

## 7. Interaction + a11y / mobile

**One composer, two entry points.** The question input lives in a **composer dialog**, not
in the transient selection popover — keeps the popover a11y contract trivial and gives the
"student types their own question" pillar a clean, focus-managed home.

- **Composer dialog** (`role="dialog"`, `aria-modal="true"`, reuses
  [04-focus-trap.js](../crates/core/assets/js/code-enhance/04-focus-trap.js)): provider
  control (picker first run, `Ask {Provider} ▾` after), an **optional question input**
  (autofocused once the provider is known; placeholder = the default question), a read-only
  passage preview, the **full composed prompt with an explicit "Copy" button** (the ultimate
  floor), and the one-line disclosure. Esc closes, returns focus to the trigger.
- **Selection popover (desktop mouse, progressive enhancement):** a single **"Ask AI"**
  button that opens the composer. Shown on selection *settle* (debounced `selectionchange` +
  `mouseup`/`keyup`), positioned from `getRangeAt(0).getBoundingClientRect()`, edge-flipped.
  `role="toolbar"` + `aria-label`; WCAG 1.4.13: Dismissible (Esc), Hoverable, Persistent.
- **Per-section button (accessible baseline; keyboard/SR/touch-primary):** an "Ask AI about
  this section" affordance on each heading's `#` permalink row, scoped to the enclosing
  section (DOM-walked from that heading). **Does NOT inherit the anchor-link hover-reveal
  CSS** (`.tali-anchor { opacity: 0 }` is invisible on touch). On coarse pointers it is
  **persistent** (`@media (pointer: coarse) { opacity: 1 }`) and **≥ 24×24 CSS px**
  (WCAG 2.5.8; target 44×44), discoverable exactly where the popover is suppressed.

**Popup-blocker-safe hand-off (critical sequencing).** In the `Ask {Provider}` click
handler, with **no `await` between the gesture and `window.open`**:
1. Build `full` + `compact`.
2. `navigator.clipboard.writeText(full)` — **initiate, do not await** (bound during the
   focused gesture; falls back to `taliCopyText`'s `execCommand` path on insecure/`file://`).
3. `window.open(providerUrl, '_blank', 'noopener')` — synchronous, in-gesture.

Because `window.open` moves focus to the new tab, the clipboard write is initiated *first*
while the book tab still has focus. The composer's disclosure line is read **before** the
switch (*"Opens {provider} in a new tab. The full prompt is also on your clipboard — paste
with Cmd/Ctrl+V if the box is empty."*), so the paste-recovery cue is never stranded on the
tab the student just left. The visible prompt + Copy button in the still-open composer is the
belt-and-suspenders if a strict browser drops the focus-lost write.

**Mobile:** suppress the custom selection popover on touch (`pointer: coarse` / `touchstart`
guard) — it collides with native selection handles and hits real Blink/WebKit light-dismiss
bugs — and defer to the persistent heading button, which opens the same composer.

**Flat / heading-less pages** (blog-style, not the book case but defined for safety):
"enclosing section" is undefined, so the fallback scope is the page's main content prose
(capped to the encoded budget for compact; full text for clipboard). The page-level button
uses this scope.

**Reuse, don't reinvent:** clipboard via `taliCopyText`
([01-registry.js:46](../crates/core/assets/js/code-enhance/01-registry.js)); focus trap via
`04-focus-trap.js`; Settings row via `taliReaderMenu.addSection`; skip on decks
(`if (document.querySelector('.tali-deck')) return;`) like every reader enhancer; idempotent
(guard with a `data-` attribute).

## 8. Privacy

**What leaves the browser — only on the student's click, only to the student's own logged-in
AI tab:** (1) the selected passage / enclosing section (the author's own prose), (2) the
student's typed question, (3) in Tier A, the book link(s). Nothing goes to any author
endpoint (no backend). The provider tab learns the book URL only from the prompt text we
compose.

**Disclosure — two layers, zero config.**
1. **One-time first-run notice** before the first hand-off: *"This opens {provider} in a new
   tab and sends the passage you selected, your question, and (if this book is online) a link
   to it. It goes to **your own {provider} account** under their privacy policy, where **it
   may be used to train their AI**. This book has no server and stores nothing but your
   provider choice."* [Continue] [Cancel]; recorded as `ack:true`.
2. **Persistent one-liner** at the point of action, linking **out** to each provider's live
   policy (so it can't go stale).

**Accepted, disclosed tradeoff:** every hand-off writes the full prompt to the clipboard (the
guaranteed floor + paste-recovery path), overwriting whatever the student had copied —
including on the prefill-happy path where it is technically redundant. We do **not** branch
this per-provider (one safe path beats a fragile "was prefill reliable this week?"
heuristic); the composer states the clipboard write plainly, so it is never *silent*.

No moderation / tutor-prompt / content filtering — out of scope by design and constraint.
Over-reliance is mitigated only by framing (question-first, "Ask" not "Solve",
prefill-not-auto-submit, not placing the affordance on assessment blocks by default) —
nudges, not controls; assessment integrity stays a course-design responsibility.

## 9. Implementation plan by file

**New assets**
1. `crates/core/assets/js/code-enhance/19-ask-ai.js` — enhancer `taliInitAskAi(root)`:
   provider registry, defensive `localStorage`, first-run picker + consent, selection
   popover, persistent heading button, focus-trapped composer dialog, split-button + caret,
   Settings "AI hand-off" section, tier detection off `<link rel="canonical">`, and two
   **pure exported functions** for testability: `taliAskExtractText(node)` (math-aware) and
   `taliAskComposePrompt(payload, tier)` → `{full, compact, deepLinkable}`. `// @ts-check`;
   reuses `taliCopyText`, `04-focus-trap.js`, `taliReaderMenu`; idempotent; skips decks.
2. CSS: append `.tali-askai-*` rules (popover, split button, composer dialog, coarse-pointer
   heading button, toast) to `crates/core/assets/css/base.css` using `--tali-*` tokens
   (theme-aware; reader chrome already lives here — no new file).

**Wiring (render)**
3. [render/mod.rs](../crates/core/src/render/mod.rs): add
   `include_str!(".../19-ask-ai.js")` to the `CODE_ENHANCE_JS` `concat!` (after
   `18-media.js`, line ~1437).
4. [09-register.js](../crates/core/assets/js/code-enhance/09-register.js):
   `reg.register(function () { taliInitAskAi(); });`.
5. [render/tests.rs](../crates/core/src/render/tests.rs): extend
   `code_enhance_bundle_matches_fragments_in_order` with `19-ask-ai.js` (guard fails loudly
   otherwise).
6. `crates/core/assets/js/globals.d.ts` (+ the web-client merge): declare new
   `window.taliAsk*` globals so both `tsc` configs stay green.

**No server changes.** The book link is the **existing** canonical URL (meta.rs:70); the
optional map is the **existing** `llms.txt`. Net server surface added: **zero**.

**Config**
7. **Zero new `_site.yml` knobs.** Enabled on **books** (`chapters:` present); Tier A/B keys
   off the existing `url:`; provider set / templates / tiering / math extraction are fixed
   near-perfect defaults; provider choice + consent are reader-local. (Extending to websites
   later = a one-line gate change.)

**Corpus pin + test**
8. `corpus/course/_site.yml`: add `url: https://course.example.edu` (a course textbook is
   meant to be deployed) so the existing course-pilot exercises Tier A (canonical + `llms.txt`
   now emit — all additive). Check `course.rs` has no assertion that breaks when
   sitemap/robots/llms/canonical start emitting; adjust if so.
9. `crates/core/tests/ask_ai.rs` (new), pinning the **server-observable** contract: (a) built
   `mle.html` carries `<link rel="canonical" href="https://course.example.edu/mle.html">`
   when `url:` is set, **no** canonical when unset (this link *is* the Tier-A signal); (b) the
   page contains a literal marker unique to `19-ask-ai.js` (proves the asset shipped).
   **Client behavior** verified two ways: the pure `taliAskComposePrompt` / `taliAskExtractText`
   called via chrome-devtools `evaluate_script` with a fixed math-containing payload and their
   output strings asserted (deterministic, no network — exercises Tier-A composition +
   `$…$`-from-annotation floor + encoded-budget truncation); and the full interaction loop
   (popover, heading button, first-run picker, remembered default, change/forget, consent,
   popup-safe open, clipboard contents, Tier-B via `?taliAskTier=B`) driven in
   `/preview corpus/course` at ~390×844 / ~1440×900 / ~900×1440, with `19-ask-ai.js`
   type-checked by both `tsc` configs.

**Sequence.** (1) step 8 + 9(a)(b) → `cargo test -p taliesin-core` green. (2) steps 1–6 →
`cargo build`; both `tsc` green; marker test green. (3) `cargo build` (assets are
`include_str!`-compiled), then `/preview corpus/course`: browser-verify interactions,
popup-safe open + real clipboard contents, Tier-A compose (canonical public even on
localhost) **and** forced Tier-B, at the three viewports. (4) full `cargo test` + both `tsc`
+ `cargo fmt`.

**Browser-test checklist:** clipboard actually contains the full prompt after hand-off;
`window.open` is not preceded by an `await`; math in a selected equation appears as LaTeX
(not doubled glyphs); the compact URL stays < ~1900 encoded chars on a math-heavy passage;
the heading button is visible + ≥24×24 tappable at 390×844; Esc returns focus from composer
to trigger.

## 10. Corpus pin

`corpus/course` gains `url:` and a new `crates/core/tests/ask_ai.rs`. The corpus doc is the
regression net: it pins that (1) a book with `url:` emits the canonical link the client keys
off, (2) the Ask-AI asset ships in the bundle, and (3) the composer's pure functions produce
a math-faithful, budget-bounded prompt. The document leads the capability (a real course
textbook a student would actually hand to their AI), matching the corpus-plus-roadmap
discipline.

## 11. Out of scope (v1) / YAGNI

- **Per-page `.md` projection** — *dropped after the red-team.* `Site::page_prose` /
  `strip_katex` deletes all math ("v1 omits math"), so a `.md` of a probability course ships
  the AI a document with every equation removed. The canonical `.html` carries clean LaTeX in
  its KaTeX `<annotation>` and needs **zero** new build infra, so it is both more faithful
  *and* less surface. Revisit only if `page_md` is made math-faithful **and** a non-math
  corpus needs the token reduction.
- Any reliance on **auto-submit** (cross-site is prefill-only and under active tightening).
- `model=`; anything beyond an optional best-effort `&hints=search` on ChatGPT.
- Hosted chat widget / backend / API keys / server-side reachability probing / moderation /
  answer capture / analytics.
- Writing back to source (forbidden — read-only view).
- Custom selection popover on touch / over native selection handles.
- Gemini *app* / Copilot / Mistral / Duck.ai as first-class providers (copy-only at most).
- `llms-full.txt` as the per-question link (wrong granularity; the per-page canonical URL +
  the `llms.txt` map cover it).

## 12. Verified facts (2026 provider mechanics)

Source-checked and adversarially re-verified in the design workflow (16 load-bearing claims,
2 refuted, corrected below):

- **ChatGPT `chatgpt.com/?q=`** prefills a logged-in session; since a **Jul-2025** security
  fix a **cross-site** click **prefills only, does not auto-submit** (student presses Enter).
  URL transport binds first → keep the whole encoded URL **< ~2000** chars. Optional
  `&hints=search` nudges browsing (observed, working). Do **not** rely on ChatGPT fetching a
  pasted URL (free tier does search, not reliable URL-fetch; no tier can fetch
  localhost/private/login-gated).
- **Claude `claude.ai/new?q=`** — **REFUTED as reliable.** The web `?q=` prefill for general
  chat was removed ~**Oct-2025** (Anthropic closed it "not planned"); there is no reliable
  plain-URL prefill for Claude chat in 2026. → **Claude is paste-primary.** Claude *can*
  fetch a **public** URL if the per-chat web-search toggle is on (server-side; can't reach
  localhost/private).
- **Perplexity `perplexity.ai/search/?q=`** prefills **and auto-runs**, and reads pasted
  **public** URLs (best-effort; its own crawler, not a Bing index — Bing Search API shut off
  2025-08-11). Strong fit for "read the linked book," public only.
- **Google AI Mode `google.com/search?udm=50&q=`** works, auto-runs, has live web access.
  Do **not** rely on `aep=11` (unverified). The Gemini *app* does **not** prefill from a URL
  → route Google users to AI Mode.
- **Copilot** `?q=` prefill **regressed** in early 2026 (query shows as heading, box empty);
  treat prefill *and* auto-submit as unreliable → copy-only.
- **Duck.ai** — no prefill param, no reliable URL fetch → copy-only, deprioritized.
- **The "Open in ChatGPT/Claude" docs-site button pattern is exactly this** (`?q=` with a
  prompt embedding the page URL), shipping in Mintlify (which runs Anthropic's own docs) —
  i.e. the mechanism is proven, current, and in production.

## 13. Residual risks (honest)

- A strict browser can drop the clipboard write if focus is lost to the new tab — mitigated
  by the visible prompt + explicit Copy button in the still-open composer, not eliminated.
- An SSO/LMS origin that looks public but 401s the AI's fetch is undetectable — mitigated by
  keeping the section floor inline in **both** prompt variants (student still gets a useful
  answer, just no whole-book fetch).
- A free-tier, browsing-off model may still confabulate a "reading" of the linked page
  despite the conditional phrasing — mitigated by passage-first + explicitly-optional link;
  not controllable from a static page.
- `?q=` params are undocumented third-party contracts that have broken before (ChatGPT
  Jul-2025, Claude Oct-2025) — mitigated by the clipboard floor under every provider and the
  single-table break-glass.

## 14. Resolved minor defaults (author may override)

- **First-run pick, no pre-selection** — chosen (honors raw passthrough + pause-to-think).
- **`&hints=search`** appended to the ChatGPT deep-link — **included** (best-effort browsing
  nudge, since the whole point is to let the AI read the book link; removable in the table).
- **Clipboard clobber** — accepted, disclosed in the composer (one safe path, not per-provider
  branching).
- **Consent for possible minors** — the one-liner + first-run notice ship by default; a hook
  is left to append an institutional-policy link if the author wants a fuller notice later.

## 15. Open questions for the author

1. Confirm §14 defaults (esp. `&hints=search` and clipboard clobber).
2. Consent copy for minors: is the default notice enough, or add an institutional-policy link?
3. Tier-B investment: for any non-public deploy, is the inlined passage + section enough, or
   should we inline a larger section? (Moot if the deploy is purely public.)
