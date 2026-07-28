# R9 — conformance oracle and a publishable VPAT/ACR

**Date:** 2026-07-28
**Round:** Wave 3 / R9 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** Does the output pass a real conformance tool, and can that be turned into a credential?

**Why novel here.** AP7 was a hand audit. **No conformance tool had ever run against built output.**
`a11y.rs` ships three static rules and defers contrast and document `lang` by its own admission, to a
live audit that had never been run.

**Method actually run.** Binary rebuilt from this branch (`9e80286`), then `taliesin build --out` for
`docs/guide`, `corpus/tarn` and `corpus/tech-blog` (54 pages), served over HTTP, audited with
**Lighthouse (axe-core engine)** across a book chapter, a deck and the blog, on desktop and mobile, in
**both themes**. New items are numbered from **124**.

---

## Headline

**The output is in genuinely good shape, and the one real WCAG 2.1 AA failure is invisible in the
score.**

| Target | Device | Theme | Accessibility | Best Practices | SEO | Failures |
|---|---|---|---|---|---|---|
| `tarn/concepts.html` (book chapter) | desktop | dark | **100** | 100 | 91 | 2 |
| `tarn/concepts.html` | desktop | **light** | **100** | 100 | 80 | 2 |
| `guide/demo.html` (deck) | desktop | dark | **100** | 100 | 100 | **0** |
| `tech-blog/index.html` (blog) | mobile | dark | **100** | 100 | 100 | **0** |

**The scores were verified non-vacuous rather than taken at face value.** `color-contrast` ran in
`binary` mode and *passed* with zero violation items on all three targets, in both themes — so the
attribute `a11y.rs` explicitly defers to a live audit is now measured, and it is clean. That closes a
gap the codebase has documented for weeks.

But **`label-content-name-mismatch` scored 0 on both book runs while the category still read 100**,
because Lighthouse assigns that audit **weight 0** in the accessibility category (measured from the
report's `auditRefs`). A real WCAG 2.1 AA failure exists and the number does not move. **This is
exactly why the round's deliverable is an ACR and not a score.**

---

## Items

### 124. WCAG 2.1 AA 2.5.3 "Label in Name" fails on the search button, on 50 of 54 built pages, from one source line. (MEDIUM)

**Measured.** On `tarn/concepts.html`, axe reports:

> `<button class="tali-search-btn" type="button" data-tali-search aria-label="Search" aria-keyshortcuts="Control+K Meta+K">`
> Fix any of the following: Text inside the element is not included in the accessible name

Read from the live DOM:

```json
{"visibleText":"Ctrl K", "textContent":"Ctrl K", "ariaLabel":"Search"}
```

The visible text and the accessible name **share no words**. SC 2.5.3 requires the accessible name to
contain the visible label text; the criterion exists for speech-input users, who say what they see. A
user saying "click Ctrl K" reaches nothing.

**Blast radius, enumerated rather than assumed:** `data-tali-search` appears on **50 of 54** built
pages, and all of them come from a single emitter, `crates/core/src/site/chrome.rs:50`.

**Desktop only, and the reason is measured, not guessed.** The mobile blog run scored this audit
`1` (passed). Cause: `site.css:210` and `:239` set `.tali-search-kbd { display: none; }` on narrow
viewports, so there is no visible text to mismatch. The failure therefore appears exactly where the
shortcut hint is shown.

**Fix, and the better of the two options.** Either extend the accessible name (`aria-label="Search
(Ctrl K)"`) or mark the shortcut hint `aria-hidden="true"` so it is decoration rather than a label.
**Prefer `aria-hidden` on the `kbd` span**: `aria-keyshortcuts` already carries the shortcut
semantically, so the visible hint is redundant to assistive tech, and this also stops a screen reader
announcing "Search Ctrl K" on every page. One attribute, one line.

**Refuted if** the visible text is not inside the button element (measured: `textContent` is `"Ctrl K"`).

### 125. A conformance tool cannot audit a deck, and the deck's 100 is evidence about one slide. (MEDIUM, methodological)

**Measured** on `guide/demo.html`:

```json
{"totalSlides":14, "inert":12, "hiddenByCss":0, "totalParas":15, "reachableParas+headings":10}
```

**Twelve of fourteen slides carry `inert`**, so they are removed from the accessibility tree and axe
never sees them. The deck's clean sweep — 43 audits passed, 0 failed — covers roughly **14% of its
content**, and the same is true of any single-snapshot conformance run against any deck this tool
produces.

`inert` is the **correct** implementation: hiding off-screen slides from assistive tech is what a deck
should do, and `deck.js`'s `syncInert()` exists for that reason. The finding is not about the deck's
behaviour. It is that **the project cannot currently make a conformance claim about deck content**,
and the slate specifically named decks as R9's richest target and the surface where peer tools are
weakest.

**This compounds R14 and R7.** R14 measured that a deck in a site reaches **zero of thirteen** static
validators. R7 scored `deck.js` regressions at Detection 9. This round adds that the *external*
oracle cannot see decks either. **Three independent quality mechanisms all stop at the same boundary**,
which is a stronger statement than any of them alone.

**Deliverable.** A per-slide audit harness: step the deck with `TaliesinDeck`'s API and run a snapshot
audit at each slide. The machinery exists — R14's item 112 proposes the same harness for behaviour,
and this is the same loop with a different assertion. **Do these together or not at all**; two
harnesses for one loop would be waste.

### 126. The ACR is a deliverable the project can publish today, and it is currently worth more than any fix on this list. (MEDIUM, one artefact)

Draft below. The evidence for "Supports" on the hard criteria now exists and is fresh.

**Why it matters, restated from the slate because it is easy to lose:** the ADA Title II rule requires
WCAG 2.1 AA for public institutions, roughly 70% of edtech tools publish a VPAT, and accessibility
documentation is used to screen vendors during procurement. For a tool authored at a university, this
converts accessibility work **already done** into an adoption credential. Wave 1's adoption-friction
round found the project has no answer to "who else uses this"; an honest ACR is one of the few
artefacts that speaks to an institutional evaluator without needing users.

---

## Draft ACR — Taliesin generated HTML output

**Product:** Taliesin 0.2.0, HTML output of `taliesin build`.
**Standard:** WCAG 2.1 Level AA.
**Evaluation methods:** automated conformance testing (Lighthouse / axe-core) against built output on
54 pages across three projects, desktop and mobile, in both light and dark themes; plus the project's
own static rule set (`crates/core/src/diagnostics/a11y.rs`) and the AP7 manual audit of 2026-07-25.
**Date:** 2026-07-28.

**Scope note, stated up front because an inflated report is worse than none:** this evaluates the
**tool's generated chrome and layout**. Author-supplied content (alt text, heading order, link text,
media captions) is the author's responsibility; the tool lints for it (`TAL-A11Y-ALT`,
`TAL-A11Y-NAME`, heading-skip) but cannot guarantee it.

| Criterion | Conformance | Remarks |
|---|---|---|
| 1.1.1 Non-text Content | **Partially supports** | Tool chrome supplies names for every control. Author images are linted (`TAL-A11Y-ALT`) but not enforced |
| 1.3.1 Info and Relationships | Supports | Semantic landmarks, `<section>` extents, real heading hierarchy; verified by axe on 54 pages |
| 1.4.3 Contrast (Minimum) | **Supports** | axe `color-contrast` passed with zero violations in **both** themes, desktop and mobile |
| 1.4.10 Reflow | Supports | Mobile audit passed at 100; the 2026-07-26 mobile round measured no horizontal overflow |
| 1.4.11 Non-text Contrast | Supports | No axe violations |
| 2.1.1 Keyboard | **Partially supports** | Not evaluated in this round. Deck navigation is keyboard-first and pinned by `deck_key_sheet.rs`; a full keyboard walkthrough has not been run |
| 2.4.2 Page Titled | Supports | `page.rs:95` composes `"{page} · {site}"`, page name first |
| 2.4.4 Link Purpose | Supports | No axe violations; `validate_link_text_collisions` covers the static case |
| 2.4.6 Headings and Labels | Supports | axe clean; heading-skip linted, **except on decks** (`a11y.rs:228`) |
| 2.5.3 Label in Name | **Does not support** | Search button: visible "Ctrl K", accessible name "Search". Desktop, 50 of 54 pages. Item 124 |
| 3.1.1 Language of Page | Supports | Page builders default `lang`; honours front-matter `lang:` |
| 4.1.2 Name, Role, Value | **Partially supports** | axe clean on audited pages; deck slide content is `inert` and was not reachable by the oracle (item 125) |

**Criteria not evaluated:** 1.2.x (time-based media), 2.4.3 focus order, 2.4.7 focus visible, 3.2.x,
3.3.x, and all deck content beyond slide 1. **Listing them is the point of the format.**

---

## Which axe rules could become static `check` rules

The slate asked for this boundary explicitly, and it is the interesting engineering question.

**Could move into the kernel-free `check` channel** (they are facts about emitted HTML):

- `label-content-name-mismatch` — item 124's rule. Comparing an element's text content against its
  `aria-label` is pure string work on block HTML, and `a11y.rs` already parses interactive elements
  for rule 2. **This is the cheapest genuine win on the list**, and it would have caught 124 the day
  the search button shipped.
- `aria-allowed-attr`, `aria-required-attr`, `duplicate-id-aria` — static attribute facts.
- `html-has-lang` — already known statically; `a11y.rs` defers it only because the page builders
  always set it, which is a defensible reason to skip rather than to add.

**Genuinely require a browser** (they need computed style or layout):

- `color-contrast` — needs computed CSS, exactly as `a11y.rs`'s docstring says. The docstring is
  correct and this round confirms it rather than contradicting it.
- `target-size` — needs layout boxes.
- Anything about focus order or visible focus.

**Recommendation:** add `label-content-name-mismatch` as a fourth static rule when item 124 is fixed,
so the fix ships with the rule that keeps it fixed. That is this project's own standing method
(verify by mutation, pin the mechanism), applied here.

---

## Measured healthy

- **Contrast passes in both themes, on every audited surface, with zero violations.** This is the
  attribute `a11y.rs` explicitly deferred, and the deferral has now been discharged.
- **Best Practices 100 on every target**, including the deck.
- **The deck's audited surface is clean**: 43 passed, 0 failed, and `label-content-name-mismatch` was
  `notApplicable` (no search chrome on a deck).
- **The blog on mobile is 100 across all four categories with 0 failures.**
- **`inert` is used correctly** to hide off-screen slides, which is why item 125 is a measurement
  limitation rather than a defect.

**One SEO failure was found and deliberately not filed:** `meta-description` missing on
`tarn/concepts.html`. That is the known L5 `description:` residual (**item 56**, already open), not a
new finding.

---

## Not measured

- **No standalone axe-core run.** Lighthouse embeds axe but runs a subset with its own weighting; that
  weighting is precisely what hid item 124. A direct axe-core run would report more, and the offline
  invariant means it would need vendoring rather than a CDN.
- **No screen reader.** No NVDA, VoiceOver or TalkBack pass. Every "Supports" above rests on automated
  testing plus AP7's manual pass, and automated tools detect roughly a third of real barriers.
- **No keyboard walkthrough.** 2.1.1, 2.4.3 and 2.4.7 are recorded as unevaluated in the ACR for that
  reason.
- **13 of 14 deck slides**, per item 125.
- **The preview path was not audited**, only built output, as the slate specified.
- **`corpus/tech-blog` post pages and the marketing site were not individually audited**, only the
  blog index.

## Round bookkeeping

This round wrote only this file. Items 124-126 follow R2's 120-123. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**Remaining slate after this round:** R8 (author value stream), R11 (real external document), R10
(demand and positioning), R13 (green software, optional), and R12 (real-device mobile), which needs
the author's phone.
