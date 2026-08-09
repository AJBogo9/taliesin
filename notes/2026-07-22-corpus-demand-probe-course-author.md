# Demand-probe findings, Course author pilot

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

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

### F-04, a mounted sub-project's code cells do not execute in the host site preview  [gap · P3]

**Wanted:** Previewing the marketing site (`preview site`) and opening the mounted course
(`/gallery/course/em.html`) should show the `{python}` cell's executed output, matching
the standalone build/preview of the course, so the live gallery exhibit is faithful.
**Happened:** Under `preview site` (kernel available, warm-pool booted), the mounted page
renders the `{python}` cell as **bare source**: no output, no output element, and **no
"kernel unavailable" notice** (polled ~6s after reload). Mounts are served for navigation
("mount is preview-only") without a live exec channel, so their cells never run. The
static `build` of the mount (`build corpus/course --out .../gallery/course`) DOES execute
the cell (`[1.7 1.3] -> total 3.0`), so the **deployed** gallery is correct; only the live
**preview** of the mount is degraded.
**Repro:** `preview <site-with-a-mount-containing-a {python} cell>`; open the mounted page;
the cell shows source only, silently.
**Disposition:** backlog (P3). In-scope. The shipped/static gallery is unaffected. Candidate:
give mounted sub-projects a live exec channel in preview, or at minimum surface an exec/kernel
state so a mounted computational cell does not look like dead source. Low priority because the
build artifact is correct.

### Observation (tangential, for the OFF-1 workstream, not a course-persona finding)

Building the marketing site emits OFF-1 offline warnings on each page's own **canonical/OG
self-URL** (`https://taliesin.dev/<page>.html`), including the new `gallery.tmd`. The URL is
**not present in the built page** (grep of `gallery.html` finds nothing), so the warning
appears to fire on canonical/OG metadata rather than a view-time fetch. Every site page
already gets this; the gallery page merely inherits it. The `esm.sh` three.js warnings on
`index.tmd` are legitimate (real CDN dep). Flagged for whoever owns OFF-1; not diagnosed
further here (out of this pilot's scope).

## Progress log (which surfaces produced findings)

- **Task 1 scaffold** clean: book builds (4 pages), draft `problems.tmd` correctly excluded from `build`.
- **Task 2, ch1 (probability):** authored cleanly, no findings. `definition` numbers chapter-scoped (Definition 1.1) with no config; `@fig-`/`@def-` in-page refs resolve; `check` clean. A "covered" case.
- **Task 3, ch2 (MLE):** the headline interaction WORKS: shared counter × chapter scope gives Theorem 2.1 / Lemma 2.2; auto-QED proof; cross-page refs resolve (`probability.html#def-expectation` → Definition 1.1, `@sec-probability` → "Chapter 1", `@fig-distributions` → Figure 1.1). One finding: **F-01** (book-level `theorems:` config unsupported).
- **Task 4, lecture deck:** builds clean as a standalone deck (title slide + 3 content slides, ARIA roles, `. . .` fragment, `.incremental` list, footer). No findings.
- **Task 5, ch3 (EM):** the heavy HTML stack all WORKS: Theorem 3.1; collapsible `<details>` proof (`.tali-proof-collapse`); `{{< embed lecture.tmd >}}` → iframe + deck built beside the page; `.code-walkthrough` with `data-cw-lines` 2/3/4; `{python}` cell executes with the venv kernel (`[1.7 1.3] -> total 3.0`); all cross-page refs resolve (Theorem 2.1 / Chapter 2 / Definition 1.1). Kernel-unavailable degradation is documented behavior, not a finding. The `read` probe surfaced **F-02** (book-chapter read loses cross-refs/numbering) and **F-03** (read projection of embed + walkthrough is lossy).
- **Task 6, draft appendix (problems):** draft handling correct: `build` drops `problems.tmd`, no `problems.html`, 0 dangling nav links, chapter numbers intact (1.1/2.1/2.2/3.1), `check` clean. Cross-refs from the appendix target valid anchors. No findings (preview-shows-badged-draft deferred to the Task 8 browser pass).
- **Task 7, pin test:** `crates/core/tests/course.rs` (4 tests) passes; full core suite green; clippy `-D warnings` clean on the test.
- **Task 8, gallery:** mounted the course at `/gallery/course` (additive `mounts:` entry) + a `site/gallery.tmd` exhibit card + nav item; static build wires the mount with its own `build ... --out` step (mirrors the docs books). **Browser-verified (dark, desktop):** the gallery page renders + links resolve; the mounted course renders with chapter numbering, cross-page refs + "Referenced by" back-links, the Definition/Theorem boxes, the authored SVG figure, the collapsible proof, the **embedded lecture deck** (live, 1/4 with nav), the code-walkthrough (sticky panel + line-2 highlight + scroll scenes), and prev/next showing the draft appendix ("4 Problem set") in preview. **Zero console errors.** Findings: **F-04** (mount cells don't execute in preview) + an OFF-1 canonical-URL observation.

## Roll-up

**5 chapters/artifacts authored, 4 findings + 1 tangential observation.**

- **gaps (2):** F-02 (`read` on a book chapter loses cross-refs/numbering, P2) · F-04 (mount cells don't execute in preview, P3).
- **friction (2):** F-01 (book-level `theorems:` config unsupported, P3) · F-03 (`read` projection of embed/walkthrough lossy, P3).
- **interaction-bugs (0).** Every stacked HTML interaction worked: book × shared-theorem-counter × chapter scope × cross-page refs × deck-embed-in-chapter × code-walkthrough × executable cell × draft-appendix-renumber. This is the headline result: the *combinations* the corpus never tested together are solid.
- **correctly-refused (0).** The persona never reached for a settled non-goal, so nothing had to be refused. The slate stayed cleanly in scope.

All four findings are P2/P3, in-scope, with working workarounds; none block the pilot. Notably, all four sit on **secondary surfaces** (machine-facing `read`, book-wide config ergonomics, mount live-preview), not on the core render path.

## Pilot retro

**Did the recipe (spec §3) hold?** Yes. Author-for-real → log resistance → pin what works → package for the gallery worked cleanly, and the gallery step doubled as a probe (it surfaced F-04 and the OFF-1 observation, which pure authoring would have missed). Confirmation: the demand probe's value here was less "the tool is broken" (it is not) and more "here are the exact seams where a real book-length computational project meets friction," which is precisely the roadmap-feeding signal we wanted.

**Refinements before scaling:**
1. Run the machine-facing `read` probe on **every** persona, not just when convenient: it produced the highest-value finding (F-02) and is cheap.
2. Build + browser-verify the **mounted** gallery exhibit for each persona (not just the standalone build): the mount is its own surface (F-04).
3. Keep authoring to showcase quality: F-01/F-03/F-04 only surfaced because the doc was pushed to real depth + real packaging.

**Go/no-go for the next persona (OSS docs maintainer):** GO. The recipe is validated and cheap per finding. The docs-maintainer persona stresses a different cluster (tabsets × search × mounts × API-reference), so expect a fresh finding set with little overlap. No slate change: the four-persona plan stands; sequence docs-maintainer → interactive-explainer → analyst.

**One slate note:** F-04 (mount preview exec) will recur for every gallery exhibit, so it is worth fixing (or at least surfacing a state) before the gallery has 4 exhibits, else live-previewing the gallery misleads.
