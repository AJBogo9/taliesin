# Demand-probe findings, Course author pilot

**Date:** 2026-07-22 · **Spec:** docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md
**Plan:** docs/superpowers/plans/2026-07-22-corpus-demand-probe-course-pilot.md
**Persona:** university lecturer authoring interactive lecture notes (a book) plus a companion deck.

Categories: `gap` (in-scope capability missing) · `friction` (works but awkward) ·
`interaction-bug` (breaks only in combination) · `correctly-refused` (a settled non-goal).

## Findings

<!-- One entry per finding:
### F-NN, <title>  [category · Pn]
**Wanted:** … **Happened:** … **Repro:** … **Disposition:** …
-->

### F-01, book-wide theorem policy cannot be set at book level  [friction · P3]

**Wanted:** As a lecturer, set the theorem-numbering policy once for the whole book in
`_site.yml` (`theorems:\n  shared: [theorem, lemma, corollary]`), not repeated in every
chapter's front matter.
**Happened:** `theorems:` is not a recognized book-config key. `build` warns
`_site.yml: unknown config key 'theorems'` and `check` reports it as an **error**
(`TAL-CHECK`). The policy is silently not applied: a shared lemma reverts to its own
per-kind counter (Lemma **2.1** instead of **2.2**). It only takes effect in per-chapter
front matter.
**Repro:** Add `theorems:\n  shared: [theorem, lemma, corollary]` to a book `_site.yml`;
`build`/`check`; the key warns/errors and shared numbering is ignored.
**Disposition:** backlog. In-scope (book config, HTML-only). Candidate: recognize
`theorems:` as a book-config key inherited by chapters (chapter front matter overrides).
Fits "perfect the default / minimal config": a book-wide policy set once beats per-chapter
duplication. Workaround (per-chapter front matter) works today, so P3.

### F-02, `read` on a book chapter loses cross-ref resolution and chapter numbering  [gap · P2]

**Wanted:** A machine-facing text projection of a book chapter (for an agent/tool
consumer) that resolves cross-references the way the HTML does, e.g. "Recall Theorem 2.1
from Chapter 2".
**Happened:** `taliesin read corpus/course/em.tmd` projects cross-page refs as **bare
kind words** with the referent lost: "Recall **Theorem** from **Section**", "in the sense
of **Definition**". In-page theorem numbers are single-doc, not chapter-scoped: `@thm-elbo`
(HTML: Theorem 3.1) projects as "**Theorem 1**". `read --run` executes the cell fine
(`[output: effective counts: [1.7 1.3] -> total 3.0]`) but does **not** fix the refs.
And `read corpus/course` (the book dir) **errors** ("read projects a single .tmd file...
use build or preview"), so there is no book-aware read at all: single-file read has no
site/chapter context.
**Repro:** `taliesin read <one-chapter-of-a-book>.tmd`; cross-page refs collapse to bare
kind words and in-page floats number single-doc-style.
**Disposition:** backlog. In-scope and on-direction (the machine-facing `read`/`map`/`mcp`
surface). The build path already resolves this via `render_document_with_includes_scoped`
+ `Site::chapter_for`; `read` could take the same book context (or `read <book-dir>` could
project chapters in order with refs resolved) instead of erroring.

### F-03, `read` projection of rich blocks (embed, code-walkthrough) is lossy/noisy  [friction · P3]

**Wanted:** The embed and the code-walkthrough to project as something meaningful for a
machine reader.
**Happened:** The `{{< embed lecture.tmd >}}` projects as its iframe **UI chrome text**
("⤢ FullscreenOpen ↗ (opens in a new tab)") rather than naming the embedded document
(e.g. "[embedded deck: lecture.tmd]"). The `.code-walkthrough` step prose and the code
panel **concatenate with no separation** ("...it currently owns.Re-estimate...for the
next E-step.def m_step(x, resp): ..."), losing the step/code structure.
**Repro:** `taliesin read` a doc containing an `{{< embed >}}` and a `.code-walkthrough`.
**Disposition:** backlog (P3). In-scope polish of the `read` projection for structured
blocks; HTML renders both correctly, so this is projection-only.

## Progress log (which surfaces produced findings)

- **Task 1 scaffold** clean: book builds (4 pages), draft `problems.tmd` correctly excluded from `build`.
- **Task 2, ch1 (probability):** authored cleanly, no findings. `definition` numbers chapter-scoped (Definition 1.1) with no config; `@fig-`/`@def-` in-page refs resolve; `check` clean. A "covered" case.
- **Task 3, ch2 (MLE):** the headline interaction WORKS: shared counter × chapter scope gives Theorem 2.1 / Lemma 2.2; auto-QED proof; cross-page refs resolve (`probability.html#def-expectation` → Definition 1.1, `@sec-probability` → "Chapter 1", `@fig-distributions` → Figure 1.1). One finding: **F-01** (book-level `theorems:` config unsupported).
- **Task 4, lecture deck:** builds clean as a standalone deck (title slide + 3 content slides, ARIA roles, `. . .` fragment, `.incremental` list, footer). No findings.
- **Task 5, ch3 (EM):** the heavy HTML stack all WORKS: Theorem 3.1; collapsible `<details>` proof (`.tali-proof-collapse`); `{{< embed lecture.tmd >}}` → iframe + deck built beside the page; `.code-walkthrough` with `data-cw-lines` 2/3/4; `{python}` cell executes with the venv kernel (`[1.7 1.3] -> total 3.0`); all cross-page refs resolve (Theorem 2.1 / Chapter 2 / Definition 1.1). Kernel-unavailable degradation is documented behavior, not a finding. The `read` probe surfaced **F-02** (book-chapter read loses cross-refs/numbering) and **F-03** (read projection of embed + walkthrough is lossy).
- **Task 6, draft appendix (problems):** draft handling correct: `build` drops `problems.tmd`, no `problems.html`, 0 dangling nav links, chapter numbers intact (1.1/2.1/2.2/3.1), `check` clean. Cross-refs from the appendix target valid anchors. No findings (preview-shows-badged-draft deferred to the Task 8 browser pass).

## Roll-up (filled at Task 9)

- gaps: … · friction: … · interaction-bugs: … · correctly-refused: …
