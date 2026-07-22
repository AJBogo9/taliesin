# Corpus demand-probe — Course pilot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a realistic in-scope "course author" project (`corpus/course/`), authored to showcase quality, that stacks feature interactions the current corpus never combines — and while authoring it, mine and log every point where the tool falls short, then pin what works and exhibit it in the marketing-site gallery.

**Architecture:** `corpus/course/` is a book project (`_site.yml` with `chapters:`) of a preface + three numbered chapters + a draft appendix, plus an embedded lecture deck. Authoring is the probe: each resistance point becomes a categorized finding in a dated notes doc. The parts that render cleanly are locked by a new `crates/core/tests/course.rs` pin test (modeled on the existing demo-book book tests) and are mounted into `site/` under `/gallery/course` with a `site/gallery.tmd` exhibit card. **No engine/crate source changes** — this pass authors documents, writes a test, edits site config, and records findings.

**Tech Stack:** Taliesin `.tmd` (comrak-flavored Markdown + `:::` divs + `#|`/`{{< >}}` shortcodes + `@ref` crossrefs + KaTeX math), the `taliesin` CLI (`build`/`check`/`preview`/`read`), Rust integration tests (`cargo test -p taliesin-core`), chrome-devtools MCP via the `preview` skill.

## Global Constraints

- **Authoring + findings + test + site-config ONLY. No changes to `crates/` source.** If a finding tempts an engine fix, STOP, log the finding, do NOT fix it here (fixes are separate later work).
- **Do-NOT-touch (untouched by construction):** `MAX_WARM_PAGES` + LRU eviction in `serve_site/exec_pool.rs`; the single-editing-surface invariant (preview never writes source).
- **Respect the identity:** every element is in-scope for an HTML-only, single-author, computational-document tool. A wall that is a settled non-goal (LaTeX/Word/ePub export, collaboration, RTL/i18n, RSS categories) is logged `correctly-refused`, never filed as a gap.
- **Offline / no-CDN / `--tali-*` tokens only** for any `site/` touch.
- **Corpus guards stay green:** the new docs must pass `every_corpus_doc_has_clean_front_matter` and the corpus-wide clean-vocabulary guard (use only Taliesin's own vocabulary; no legacy keys).
- **Branch:** all work lands on `feat/corpus-demand-probe` (already created; the spec is its first commit). Do NOT move `main`. Do NOT push (the author pushes).
- **Findings doc:** `notes/2026-07-22-corpus-demand-probe-course-author.md`. Every finding: title · category ∈ {`gap`,`friction`,`interaction-bug`,`correctly-refused`} · severity P1–P3 · *wanted* → *happened* · minimal repro · disposition.
- **Fixed chapter structure (so pin-test numbers are deterministic):** `index.tmd` = unnumbered Preface; `probability.tmd` = **Ch 1**; `mle.tmd` = **Ch 2**; `em.tmd` = **Ch 3**; `problems.tmd` = draft appendix (dropped on `build`). Chapter-scoped numbering ⇒ `def-expectation`=Definition 1.1, `fig-distributions`=Figure 1.1 (ch1); `thm-consistency`=Theorem 2.1, `lem-score`=Lemma 2.2 (shared counter), `fig-likelihood`=Figure 2.1 (ch2); `thm-elbo`=Theorem 3.1 (ch3).
- **Verification commands** (used throughout — the binary is `target/debug/taliesin` or the `taliesin` launcher):
  - Invariants + pin: `cargo test -p taliesin-core`
  - Build standalone: `taliesin build corpus/course` (and `taliesin build corpus/course --out /tmp/course-out`)
  - Static lint: `taliesin check corpus/course`
  - Machine view: `taliesin read corpus/course/em.tmd`
  - Live/visual: the `preview` skill (chrome-devtools) at three viewports (≈390×844, ≈1440×900, ≈900×1440), light **and** dark.

## File structure

- Create `corpus/course/_site.yml` — book manifest (`chapters:`, `toc: true`).
- Create `corpus/course/index.tmd` — unnumbered Preface / syllabus.
- Create `corpus/course/probability.tmd` — Ch 1 (definition + figure + in-page refs).
- Create `corpus/course/mle.tmd` — Ch 2 (shared-counter theorem+lemma, proof, figure, **cross-page** refs to ch1).
- Create `corpus/course/lecture.tmd` — the companion deck (`format: deck`).
- Create `corpus/course/em.tmd` — Ch 3 (display-math theorem + collapsible proof, code-walkthrough, `{python}` cell, `{{< embed lecture.tmd >}}`, cross-page refs to ch2/ch1).
- Create `corpus/course/problems.tmd` — draft appendix (exercise callouts + refs).
- Create `corpus/course/*.svg` — 2 tiny hand-written figures (`distributions.svg`, `likelihood.svg`).
- Create `crates/core/tests/course.rs` — the interaction pin test.
- Modify `site/_site.yml` — add a `mounts:` entry + a nav entry.
- Create `site/gallery.tmd` — the exhibit-cards page (first card = the course).
- Create `notes/2026-07-22-corpus-demand-probe-course-author.md` — findings doc.
- Modify `corpus/README.md` — add the `course/` row.
- Modify `notes/backlog.md` — fold actionable findings (Task 9).

---

### Task 1: Scaffold the book skeleton + findings doc (green baseline)

**Files:**
- Create: `corpus/course/_site.yml`, `corpus/course/index.tmd`, `corpus/course/probability.tmd`, `corpus/course/mle.tmd`, `corpus/course/em.tmd`, `corpus/course/problems.tmd` (stubs)
- Create: `notes/2026-07-22-corpus-demand-probe-course-author.md`

**Interfaces:**
- Produces: the `corpus/course/` book directory that `Site::discover(&corpus_dir().join("course"))` (Task 7) and the `mounts:` entry (Task 8) depend on.

- [ ] **Step 1: Create the book manifest.** `corpus/course/_site.yml`:

```yaml
title: "Probabilistic Modeling — A Short Course"
author: "Andreas Bogossian"
toc: true

chapters:                 # presence makes this a book
  - index.tmd
  - part: "Foundations"
    chapters:
      - probability.tmd
      - mle.tmd
  - em.tmd
  - problems.tmd
```

- [ ] **Step 2: Create stub chapters** so the book renders end-to-end before the real content lands. Each stub is one heading + one sentence.

`corpus/course/index.tmd`:
```markdown
# Preface {.unnumbered}

A short, worked course in probabilistic modeling: from expectations to maximum
likelihood to the EM algorithm, with the lecture slides beside the notes.
```

`corpus/course/probability.tmd`:
```markdown
# Probability refresher {#sec-probability}

Stub — expectations and distributions.
```

`corpus/course/mle.tmd`:
```markdown
# Maximum likelihood {#sec-mle}

Stub — the likelihood and its estimator.
```

`corpus/course/em.tmd`:
```markdown
# The EM algorithm {#sec-em}

Stub — latent variables and the ELBO.
```

`corpus/course/problems.tmd`:
```markdown
---
draft: true
---

# Problem set

Stub — exercises.
```

- [ ] **Step 3: Create the findings-doc skeleton.** `notes/2026-07-22-corpus-demand-probe-course-author.md`:

```markdown
# Demand-probe findings — Course author pilot

**Date:** 2026-07-22 · **Spec:** docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md
**Persona:** university lecturer authoring interactive lecture notes (book) + a companion deck.

Categories: `gap` (in-scope capability missing) · `friction` (works but awkward) ·
`interaction-bug` (breaks only in combination) · `correctly-refused` (a settled non-goal).

## Findings

<!-- One entry per finding:
### F-NN — <title>  [category · Pn]
**Wanted:** … **Happened:** … **Repro:** … **Disposition:** …
-->

## Roll-up (filled at Task 9)
- gaps: … · friction: … · interaction-bugs: … · correctly-refused: …
```

- [ ] **Step 4: Verify the skeleton builds and stays green.**

Run: `taliesin build corpus/course --out /tmp/course-out && cargo test -p taliesin-core`
Expected: build succeeds (a book `_site/`-style output under `/tmp/course-out`); the corpus suite PASSES — `every_corpus_doc_has_clean_front_matter` and the block-id/sourcepos/order invariants now include `corpus/course/` and stay green. If front-matter warns, a key is wrong — fix the manifest, do not add a legacy key.

- [ ] **Step 5: Commit.**

```bash
git add corpus/course notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "test(corpus): scaffold course pilot book skeleton + findings doc"
```

---

### Task 2: Author Chapter 1 — Probability refresher

**Files:**
- Modify: `corpus/course/probability.tmd`
- Create: `corpus/course/distributions.svg`

**Interfaces:**
- Produces: anchors `#def-expectation` (Definition 1.1) and `#fig-distributions` (Figure 1.1), referenced cross-page by Tasks 3 & 5.

- [ ] **Step 1: Author the chapter to showcase quality.** Replace the stub with real prose + math + a numbered definition + a labelled figure + in-page cross-refs. Skeleton (flesh out the prose; keep the ids and blocks exactly):

```markdown
# Probability refresher {#sec-probability}

<!-- 2–4 short paragraphs motivating expectation as the first tool of the course. -->

![Three densities that recur throughout the course.](distributions.svg){#fig-distributions}

As @fig-distributions shows, <!-- one sentence tying the figure to the text. -->

::: {.definition #def-expectation}
The **expectation** of a random variable $X$ with density $p$ is
$\mathbb{E}[X] = \int x\,p(x)\,\mathrm{d}x$.
:::

By @def-expectation, <!-- one sentence using the definition in-page. -->
```

- [ ] **Step 2: Create `corpus/course/distributions.svg`** — a tiny self-contained SVG (three simple curves), no external refs, `<svg>` with a `viewBox`. Keep it under ~30 lines.

- [ ] **Step 3: Build + lint + browser-verify, logging findings as you go.**

Run: `taliesin build corpus/course --out /tmp/course-out && taliesin check corpus/course`
Then preview via the `preview` skill; confirm Definition 1.1 renders with its number, `@fig-distributions` → "Figure 1.1", `@def-expectation` → "Definition 1.1", math renders, light + dark, three viewports.
For **every** point of resistance (a ref that won't resolve, a definition that numbers oddly, an SVG that clips, a diagnostic that misfires) add an `F-NN` entry to the findings doc.

- [ ] **Step 4: Commit.**

```bash
git add corpus/course/probability.tmd corpus/course/distributions.svg notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(corpus): course ch1 (probability) + probe findings"
```

---

### Task 3: Author Chapter 2 — Maximum likelihood (shared theorem counter + cross-page refs)

**Files:**
- Modify: `corpus/course/mle.tmd`
- Create: `corpus/course/likelihood.svg`

**Interfaces:**
- Consumes: `#def-expectation`, `#fig-distributions` from Task 2 (cross-page refs).
- Produces: `#thm-consistency` (Theorem 2.1), `#lem-score` (Lemma 2.2), `#fig-likelihood` (Figure 2.1), referenced cross-page by Tasks 5 & 6.

- [ ] **Step 1: Author the chapter.** Put the shared-counter config in this chapter's front matter (the proven per-doc location; whether it also works at `_site.yml` level is probed in Step 3). Skeleton:

```markdown
---
theorems:
  shared: [theorem, lemma, corollary]
---

# Maximum likelihood {#sec-mle}

Building on @def-expectation from @sec-probability, <!-- motivate the likelihood. -->

![The log-likelihood of a Gaussian mean.](likelihood.svg){#fig-likelihood}

::: {.theorem #thm-consistency}
Under regularity conditions the maximum-likelihood estimator is consistent.
:::

::: {.lemma #lem-score}
The score has zero expectation at the true parameter: $\mathbb{E}[\nabla \log p] = 0$.
:::

::: {.proof}
<!-- a two-to-four line proof; auto-QED appends the tombstone. -->
:::

By @thm-consistency and @lem-score, <!-- in-page use. --> Compare @fig-distributions
(@sec-probability) with @fig-likelihood here.
```

- [ ] **Step 2: Build + browser-verify.**

Run: `taliesin build corpus/course --out /tmp/course-out && taliesin check corpus/course`
Confirm: `#thm-consistency` numbers **2.1** and `#lem-score` numbers **2.2** (shared counter, chapter-scoped); the proof shows an auto-QED tombstone; `@def-expectation` and `@fig-distributions` resolve **across pages** to "Definition 1.1" / "Figure 1.1"; `@fig-likelihood` → "Figure 2.1". Browser-verify (three viewports, light+dark).

- [ ] **Step 3: Probe the config location (finding).** Temporarily move `theorems: shared: [...]` from `mle.tmd` front matter into `corpus/course/_site.yml` (book-level), rebuild, and check whether the shared counter still applies. Whichever way it behaves, record an `F-NN` finding ("can `theorems:` policy live at book level?" — `gap`/`friction`/works). Then **restore** the working per-chapter location so the pin test is deterministic.

- [ ] **Step 4: Create `corpus/course/likelihood.svg`** (tiny self-contained curve).

- [ ] **Step 5: Commit.**

```bash
git add corpus/course/mle.tmd corpus/course/likelihood.svg notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(corpus): course ch2 (MLE) shared counters + cross-page refs + probe findings"
```

---

### Task 4: Author the companion lecture deck

**Files:**
- Create: `corpus/course/lecture.tmd`

**Interfaces:**
- Produces: a `format: deck` document embedded by Task 5 via `{{< embed lecture.tmd >}}`.

- [ ] **Step 1: Author the deck** (one slide per `##`; include a `. . .` pause and an `.incremental` list so the embed exercises deck features). Skeleton:

```markdown
---
title: "Lecture — the EM intuition"
subtitle: "Companion slides"
format: deck
footer: "Probabilistic Modeling"
---

## Why EM

The likelihood is hard; a lower bound is easy.

. . .

So we maximize the bound and iterate.

## The two steps

::: {.incremental}
- **E-step:** fill in the latent responsibilities.
- **M-step:** re-fit the parameters.
- Repeat until the bound stops rising.
:::

## Where it lands

We prove the bound (the ELBO) is what EM ascends — next, in the notes.
```

- [ ] **Step 2: Verify the deck builds standalone.**

Run: `taliesin build corpus/course/lecture.tmd /tmp/lecture.html`
Then preview `corpus/course/lecture.tmd` via the `preview` skill; step through slides (arrow keys), confirm the `. . .` pause fragment and the `.incremental` list reveal, footer shows. Log any findings.

- [ ] **Step 3: Commit.**

```bash
git add corpus/course/lecture.tmd notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(corpus): course companion lecture deck"
```

---

### Task 5: Author Chapter 3 — EM (deck embed + code walkthrough + {python} cell + cross-page refs)

**Files:**
- Modify: `corpus/course/em.tmd`

**Interfaces:**
- Consumes: `#thm-consistency` (Task 3), `#def-expectation` (Task 2), `lecture.tmd` (Task 4).
- Produces: `#thm-elbo` (Theorem 3.1), referenced by Task 6.

- [ ] **Step 1: Author the chapter**, stacking the embed + walkthrough + cell + a collapsible proof + cross-page refs. Skeleton:

```markdown
# The EM algorithm {#sec-em}

Recall @thm-consistency (@sec-mle) and @def-expectation (@sec-probability).

::: {.theorem #thm-elbo}
For any distribution $q$ over the latents,
$$ \log p(x) \ge \mathbb{E}_{q}\!\left[\log \frac{p(x,z)}{q(z)}\right] =: \mathcal{L}(q,\theta). $$
:::

::: {.proof collapse="true"}
<!-- Jensen's inequality in three lines; auto-QED. -->
:::

The lecture slides give the intuition:

{{< embed lecture.tmd >}}

The M-step update, walked through line by line:

::: {.code-walkthrough}
```python
def m_step(x, resp):            # responsibilities from the E-step
    w = resp.sum(axis=0)        # effective counts per component
    mu = (resp.T @ x) / w[:, None]
    return mu, w / w.sum()
```

::: {.step lines="2"}
Sum the responsibilities to get each component's effective count.
:::

::: {.step lines="3"}
Re-estimate each mean as a responsibility-weighted average of the data.
:::

::: {.step lines="4"}
The mixing weights are the normalized counts.
:::
:::

A quick numerical sanity check:

```{python}
#| echo: true
import numpy as np
resp = np.array([[0.9, 0.1], [0.2, 0.8]])
print("counts:", resp.sum(axis=0))
```
```

- [ ] **Step 2: Build + lint + browser-verify.**

Run: `taliesin build corpus/course --out /tmp/course-out && taliesin check corpus/course`
Confirm: `#thm-elbo` numbers **3.1**; the collapsible proof is a working `<details>`; the embed renders `lecture.tmd` as an iframe (deck built beside the page, kept out of nav); the code-walkthrough panel sticks and the steps focus their line ranges on scroll; the `{python}` cell renders (as source if no kernel; executed if a kernel is present); cross-page refs → "Theorem 2.1" / "Definition 1.1". Three viewports, light+dark. Log findings — especially any interaction breakage (e.g. embed-inside-a-book-chapter, walkthrough numbering, cell inside a numbered chapter).

- [ ] **Step 3: Machine-view probe.**

Run: `taliesin read corpus/course/em.tmd` (and, if a kernel is available, `taliesin read --run corpus/course/em.tmd`)
Confirm the projection names the figure/theorem/cell blocks sensibly; log any `gap`/`friction`.

- [ ] **Step 4: Commit.**

```bash
git add corpus/course/em.tmd notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(corpus): course ch3 (EM) embed + walkthrough + cell + cross-page refs + findings"
```

---

### Task 6: Author the draft appendix (exercises + draft renumber probe)

**Files:**
- Modify: `corpus/course/problems.tmd`

**Interfaces:**
- Consumes: `#thm-elbo` (Task 5), `#thm-consistency` (Task 3).

- [ ] **Step 1: Author the problem set** as callouts, referencing earlier results. Keep `draft: true`. Skeleton:

```markdown
---
draft: true
---

# Problem set

::: {.callout-note title="Exercise 1"}
Using @def-expectation, show the score in @lem-score has zero mean.
:::

::: {.callout-note title="Exercise 2"}
Verify the bound in @thm-elbo is tight when $q(z) = p(z \mid x)$.
:::

::: {.callout-tip title="Hint"}
Relate the gap to a KL divergence and invoke @thm-consistency.
:::
```

- [ ] **Step 2: Verify draft behavior (interaction probe).**

Preview `corpus/course` (drafts shown + badged): confirm the appendix appears last, badged draft, and its cross-refs resolve.
Then `taliesin build corpus/course --out /tmp/course-out`: confirm the appendix is **dropped** and the remaining chapters keep contiguous numbers (ch1/2/3 unchanged; no dangling nav link). If a built page still references a dropped-chapter anchor, that is an `interaction-bug` finding — log it.

- [ ] **Step 3: Commit.**

```bash
git add corpus/course/problems.tmd notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(corpus): course draft appendix (exercises) + draft-renumber probe"
```

---

### Task 7: Pin test — `crates/core/tests/course.rs`

**Files:**
- Create: `crates/core/tests/course.rs`

**Interfaces:**
- Consumes: `taliesin_core::Site` (`Site::discover`, `site.render_page(name) -> Option<String>`, `site.hover_index_json`), exactly as `crates/core/tests/corpus.rs` uses them.

- [ ] **Step 1: Write the pin test** (assertions modeled verbatim on the demo-book book tests). Content of `crates/core/tests/course.rs`:

```rust
//! Interaction pin for the "course author" demand-probe pilot (corpus/course/).
//! Locks the feature *combinations* the single-feature corpus docs never exercise
//! together: shared theorem counters × chapter scoping, cross-PAGE crossrefs, a deck
//! embedded in a book chapter, and draft-appendix renumbering. See
//! notes/2026-07-22-corpus-demand-probe-course-author.md for the findings this produced.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn course() -> Site {
    Site::discover(&corpus_dir().join("course"))
}

#[test]
fn ch2_shares_theorem_counter_and_scopes_to_chapter() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // Shared counter (theorem+lemma one sequence) AND chapter scoping (2.x): a
    // combination pinned nowhere else — theorems-shared.tmd is flat, demo-book scopes
    // an un-shared counter.
    assert!(
        mle.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "consistency theorem is 2.1: {mle}"
    );
    assert!(
        mle.contains(
            "<span class=\"tali-theorem-label\">Lemma<span class=\"tali-theorem-number\">&nbsp;2.2</span></span>"
        ),
        "score lemma shares the counter as 2.2: {mle}"
    );
}

#[test]
fn cross_page_refs_resolve_to_scoped_numbers() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // ch2 references ch1's definition and figure across pages.
    assert!(
        mle.contains("#def-expectation") && mle.contains("Definition&nbsp;1.1"),
        "cross-page ref to the ch1 definition resolves to 1.1: {mle}"
    );
    assert!(
        mle.contains("#fig-distributions") && mle.contains("Figure&nbsp;1.1"),
        "cross-page ref to the ch1 figure resolves to 1.1: {mle}"
    );

    let em = course().render_page("em.tmd").expect("em renders");
    // ch3 references ch2's theorem across pages, and its own theorem is 3.1.
    assert!(
        em.contains("#thm-consistency") && em.contains("Theorem&nbsp;2.1"),
        "cross-page ref to the ch2 theorem resolves to 2.1: {em}"
    );
    assert!(
        em.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;3.1</span></span>"
        ),
        "the ELBO theorem is 3.1 in chapter 3: {em}"
    );
}

#[test]
fn em_chapter_embeds_the_lecture_deck() {
    let em = course().render_page("em.tmd").expect("em renders");
    // The {{< embed lecture.tmd >}} lowers to an iframe pointing at the built deck html.
    assert!(
        em.contains("<iframe") && em.contains("lecture"),
        "the EM chapter embeds the lecture deck as an iframe: {em}"
    );
}

#[test]
fn defined_blocks_enter_the_hover_index_sections_do_not() {
    let idx = course().hover_index_json;
    assert!(idx.contains("\"thm-elbo\":\""), "ELBO theorem is hover-indexed: {idx}");
    assert!(idx.contains("\"def-expectation\":\""), "definition is hover-indexed: {idx}");
    assert!(!idx.contains("\"sec-em\":\""), "section headings are not hover-indexed: {idx}");
    assert!(!idx.contains("</script"), "raw </script must be neutralized: {idx}");
}
```

- [ ] **Step 2: Run the pin test.**

Run: `cargo test -p taliesin-core --test course`
Expected: PASS. If a cross-page ref renders a different exact string (e.g. a `.html` prefix on the href, or a non-breaking-space variant), adjust the assertion to the **actual** rendered output (view it by printing the page in the failing assertion), keeping the number/anchor checks. If a number is genuinely wrong, that is an `interaction-bug` finding — log it and, if it blocks a green pin, weaken that one assertion to a documented TODO-free `// known gap: F-NN` comment referencing the finding rather than asserting broken behavior.

- [ ] **Step 3: Run the whole core suite** to confirm nothing else moved.

Run: `cargo test -p taliesin-core`
Expected: PASS (the generic corpus invariants already cover `corpus/course/`; `course.rs` adds the interaction assertions).

- [ ] **Step 4: Commit.**

```bash
git add crates/core/tests/course.rs
git commit -m "test(corpus): course.rs pins the stacked interactions (shared counters × chapter scope, cross-page refs, deck embed, hover index)"
```

---

### Task 8: Gallery integration (mount + exhibit page)

**Files:**
- Modify: `site/_site.yml`
- Create: `site/gallery.tmd`

**Interfaces:**
- Consumes: the built `corpus/course/` book (Tasks 1–6).

- [ ] **Step 1: Mount the course + add nav.** Edit `site/_site.yml`: add a `gallery/course` entry to the existing `mounts:` block, and a "Gallery" nav item.

```yaml
mounts:
  docs/guide: ../docs/guide
  docs/internals: ../docs/internals
  gallery/course: ../corpus/course      # <-- add

nav:
  left:
    - { text: "Features", href: features.tmd }
    - { text: "Document types", href: formats.tmd }
    - { text: "See it live", href: showcase.tmd }
    - { text: "Gallery", href: gallery.tmd }   # <-- add
    - { text: "Guide", href: "docs/guide/" }
    - { text: "Internals", href: "docs/internals/" }
```

- [ ] **Step 2: Create the exhibit page** `site/gallery.tmd` (one card now; grows as later personas land). Use only in-repo vocabulary and relative links; no CDN.

```markdown
---
title: "Gallery"
description: "Whole projects built with Taliesin — each one a real document you can open, not a screenshot."
toc: false
---

Where [See it live](showcase.tmd) shows one capability at a time, the gallery shows
**entire projects**, each a real `.tmd` source tree rendered by Taliesin.

## Probabilistic Modeling — a short course

A lecturer's interactive notes: a numbered **book** with theorems and proofs that
number and cross-reference **across chapters**, a **lecture deck embedded** right in
the chapter, a line-by-line **code walkthrough** of the EM update, and an executable
cell — the whole authoring surface a course needs, in one project.

[Open the course →](gallery/course/)
```

- [ ] **Step 3: Verify in preview** (mounts are native in `preview`).

Preview `site` via the `preview` skill. Confirm: the "Gallery" nav item appears; `gallery.tmd` renders the card; the "Open the course" link resolves to the mounted book at `/gallery/course/`; navigating into it shows the chapters + embedded deck. Three viewports, light+dark. Log any mount/build findings (this step is itself a probe of the `mounts:` path for a book-with-embed).

- [ ] **Step 4: Confirm the static-build story.** The static site build mirrors the docs books: after `taliesin build site --out <out>`, the course is wired with its own step `taliesin build corpus/course --out <out>/gallery/course`. Verify both build without error:

Run: `taliesin build site --out /tmp/site-out && taliesin build corpus/course --out /tmp/site-out/gallery/course`
Expected: both succeed; `/tmp/site-out/gallery/course/index.html` exists and is self-contained (offline). If the top-level `taliesin build site` does NOT automatically build mounts, note whether that is expected (it mirrors docs) or a `friction` finding.

- [ ] **Step 5: Commit.**

```bash
git add site/_site.yml site/gallery.tmd notes/2026-07-22-corpus-demand-probe-course-author.md
git commit -m "feat(site): gallery page + mount the course pilot at /gallery/course"
```

---

### Task 9: Findings roll-up, backlog fold, corpus README, pilot retro

**Files:**
- Modify: `notes/2026-07-22-corpus-demand-probe-course-author.md`
- Modify: `notes/backlog.md`
- Modify: `corpus/README.md`

**Interfaces:**
- Consumes: all findings logged in Tasks 2–8.

- [ ] **Step 1: Fill the findings roll-up.** In the findings doc, complete the "Roll-up" section: count findings per category, and for each actionable one give a disposition (roadmap item / backlog entry / no-op / correctly-refused-with-rationale).

- [ ] **Step 2: Fold actionable findings into `notes/backlog.md`.** Add each `gap`/`friction`/`interaction-bug` with a disposition of "backlog/roadmap" as a backlog item (follow the file's existing item style; do NOT re-add already-shipped work). Leave `correctly-refused` items in the findings doc only.

- [ ] **Step 3: Add the corpus README row.** In `corpus/README.md`, add a row to the Documents table:

```markdown
| `course/` | Realistic course (demand-probe pilot) | a lecturer's interactive lecture-notes **book** + an embedded companion **deck**: theorems/proofs numbered + cross-referenced **across chapters** (shared counter × chapter scope), a `{{< embed >}}` deck inside a chapter, a `.code-walkthrough`, a `{python}` cell, and a draft appendix; the first corpus doc that stacks these interactions. Pinned by `course.rs`; also the first marketing-site **gallery** exhibit | (purpose-built, demand-probe pilot) |
```

- [ ] **Step 4: Write the pilot retro** at the end of the findings doc: (a) did the recipe (§3 of the spec) hold? (b) any refinements before scaling; (c) a go/no-go recommendation for the next persona (OSS docs maintainer); (d) any slate adjustment.

- [ ] **Step 5: Final green gate.**

Run: `cargo test -p taliesin-core && taliesin build corpus/course --out /tmp/course-out && taliesin check corpus/course`
Expected: all PASS/clean. Re-read the full diff (`git diff main...feat/corpus-demand-probe --stat`) to confirm **no `crates/` source** changed (only `crates/core/tests/course.rs` is allowed under `crates/`).

- [ ] **Step 6: Commit.**

```bash
git add notes/2026-07-22-corpus-demand-probe-course-author.md notes/backlog.md corpus/README.md
git commit -m "docs(corpus): course pilot findings roll-up, backlog fold, README row, retro"
```

---

## Self-review notes

- **Spec coverage:** recipe §3 → Tasks 2–8 (author→build→check→browser→log) + Task 9 (roll-up/retro); pilot artifact §5 → Tasks 1–6; automated pins §7 → Task 7 (interaction assertions) + free invariants from Task 1; the §7 "draft dropped + contiguous renumber on build" line is verified **functionally** in Task 6 Step 2 (via the `build` command), not in the unit pin, because `render_page` is preview-mode (drafts shown) and asserting the build-drop would mean guessing the build API; gallery §6 → Task 8; findings capture §8 → findings doc from Task 1 on; guardrails §9 → Global Constraints + Task 9 Step 5 no-`crates`-source gate; success criteria §10 → green gate (Task 9), gallery mount (Task 8), recipe retro (Task 9).
- **No engine changes:** the only file under `crates/` is the new integration test `crates/core/tests/course.rs`; Task 9 Step 5 asserts this.
- **Determinism:** chapter order + ids are fixed in Global Constraints, so Task 7's expected numbers (1.1/2.1/2.2/3.1) are derivable; Task 7 Step 2 covers exact-string drift by matching actual rendered output for hrefs while keeping number/anchor checks.
- **Probe honesty:** Tasks 3/5/6/8 include explicit probe steps (config-location, machine-view, draft-renumber, mount-build) that are expected to *generate* findings, not just pass.
