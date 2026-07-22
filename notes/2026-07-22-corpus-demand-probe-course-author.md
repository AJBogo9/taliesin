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

## Progress log (which surfaces produced findings)

- **Task 1 scaffold** clean: book builds (4 pages), draft `problems.tmd` correctly excluded from `build`.
- **Task 2, ch1 (probability):** authored cleanly, no findings. `definition` numbers chapter-scoped (Definition 1.1) with no config; `@fig-`/`@def-` in-page refs resolve; `check` clean. A "covered" case.
- **Task 3, ch2 (MLE):** the headline interaction WORKS: shared counter × chapter scope gives Theorem 2.1 / Lemma 2.2; auto-QED proof; cross-page refs resolve (`probability.html#def-expectation` → Definition 1.1, `@sec-probability` → "Chapter 1", `@fig-distributions` → Figure 1.1). One finding: **F-01** (book-level `theorems:` config unsupported).

## Roll-up (filled at Task 9)

- gaps: … · friction: … · interaction-bugs: … · correctly-refused: …
