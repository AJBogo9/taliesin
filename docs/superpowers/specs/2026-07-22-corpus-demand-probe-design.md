# Corpus demand-probe via realistic persona projects

**Date:** 2026-07-22
**Status:** design approved, spec under review
**Pilot:** Course author / lecturer

## 1. Context & motivation

The corpus (87 `.tmd` files) is Taliesin's regression net and its arbiter of "done".
But almost every corpus document is a **single-feature pin**: `highlight.tmd` pins
syntax highlighting, `deck.tmd` pins deck features, `panels.tmd` pins tabsets, each
with a test asserting exact behavior in isolation. The only genuinely realistic,
feature-*combining* documents are the author's own two projects (`tech-blog/`,
`bayesian-website/`).

That structure is blind to a whole class of bug: **feature-interaction bugs** — where
a `@fig-` ref resolves fine alone but breaks when the figure sits inside a tabset,
inside a callout, inside a book chapter with shared theorem counters. Real documents
stack features; the isolated pins never do. Taliesin is now mature (most of the backlog
has shipped), so *interaction coverage under realistic workloads* is the natural
frontier.

This project probes that frontier by building believable, in-scope projects for
realistic user personas, authoring them for real, and recording where the tool falls
short.

## 2. Goal & non-goals

**Primary goal (a demand probe):** build realistic persona projects, run them through
the tool, and surface where a real user would hit a wall — feeding `notes/backlog.md`
and the roadmap. This is product discovery, not just regression testing.

**Secondary outputs (same artifacts):** each project is also (b) a green corpus
regression pin covering feature interactions, and (c) a gallery exhibit on the
marketing site. **One artifact, three roles.**

**Scope stance — respect the identity.** Every persona is someone who would genuinely
choose an HTML-only, single-author, computational-document tool. A need that hits a
*settled non-goal* (LaTeX/Word/ePub export, collaboration, RTL/i18n, RSS categories,
etc. — see `notes/ROADMAP.md` non-goals) is logged `correctly-refused`, **never** filed
as a gap. The probe hunts for gaps *within* the declared identity, not against it.

**Non-goals of this work:**

- **No engine changes in this pass.** This work *authors documents* and *writes
  findings*. Any fix a finding motivates is its own later branch/spec. By construction
  the Do-NOT-touch machinery (MAX_WARM_PAGES + LRU eviction in
  `serve_site/exec_pool.rs`; the single-editing-surface invariant; the frozen
  subsystems) is never touched here.
- **Not new output formats or new user types.** The identity is fixed; we are stressing
  it, not widening it.

## 3. The probe recipe (the reusable method the pilot validates)

Applied once per persona:

1. **Persona brief** — one paragraph: who they are, what they are building, what they
   need from the tool.
2. **Author for real** — write the project in `.tmd`, deliberately pushing *into*
   feature combinations rather than around them, to **showcase quality** (see §5).
3. **Log every point of resistance** as a finding — what I wanted → what actually
   happened, with a category, severity, and a minimal repro (see §8).
4. **Package for the gallery** — build the project standalone (offline, self-contained),
   confirm it mounts into the marketing site under `/gallery/<name>`. This step is
   itself a probe of the build + `mounts:` path for a new project shape.
5. **Run the checks** — corpus invariants, `check`, `read`/`read --run`, plus a targeted
   pin test; browser-verify the live/visual parts via the `preview` skill.
6. **The clean parts become a green corpus doc** + a targeted pin test.

**Dual output per project:** a dated findings doc (feeds `notes/backlog.md`) **and** a
committed corpus project that is simultaneously a regression pin and a gallery exhibit.

## 4. Persona slate & sequencing

Four in-scope personas, chosen to stack feature combinations the current corpus does
not exercise together:

1. **Course author / lecturer** — the pilot (§5).
2. **OSS docs maintainer** — book + tabsets + Cmd-K search + mounts + walkthroughs +
   API-reference pages.
3. **Interactive-explainer author** — scrolly + reactive `{js}` graph + `{{< input >}}`
   + math + figures in one long page.
4. **Computational-report analyst** — heavy python+R exec + many figures/tables + freeze
   under realistic volume.

**Sequencing: deep pilot, then scale.** Build the pilot end-to-end at real depth, prove
the recipe (what a finding looks like, how it lands in corpus + notes + gallery, whether
the yield justifies the cost), then scale to the other three with a validated recipe —
adjusting the slate if the pilot suggests it. This honors all four personas while
de-risking the largest cost, and matches the project's own "pin one doc, verify, then
next" discipline.

## 5. The pilot artifact — Course author / lecturer

**Persona brief.** A university lecturer teaching an applied statistics / ML course
wants interactive lecture notes their students read in the browser, with the slides they
present in class embedded alongside. They write everything in `.tmd`, want theorems and
proofs numbered and cross-referenced across chapters, want an algorithm walked through
line by line, and want exercises inline.

**Domain.** Reuse the author's own domain (probability / MLE / EM / KL) so content is
credible and no shaky math is invented. The course is *structurally* new (a numbered,
theorem-bearing, cross-referenced multi-chapter course with an embedded lecture deck),
even though it shares a subject with existing blog posts. It is **not** a copy of those
posts.

**Location & shape.** `corpus/course/` — a book project:

- `index.tmd` — syllabus / preface (`{.unnumbered}`).
- ~3 chapters, e.g. *Probability refresher* → *Maximum likelihood* → *The EM algorithm*.
- An appendix problem set.
- `lecture.tmd` — one companion slide deck, embedded into a chapter via
  `{{< embed lecture.tmd >}}`.
- `_site.yml` — flat native book config (`chapters:`), mirroring `corpus/demo-book`
  conventions but denser.

**Feature stack (deliberately combined — this is where interaction bugs live):**

- Theorems / lemmas / definitions / proofs **across chapters**, with per-kind continuous
  numbering **and** a `theorems: shared: [...]` group; auto-QED proofs; a collapsible
  `::: {.proof collapse="true"}`.
- Cross-**page** references: `@thm-`, `@def-`, `@lem-`, `@sec-`, `@fig-` resolving to the
  right numbers across chapter files.
- The embedded lecture **deck** inside a book chapter (deck built standalone beside the
  page, kept out of nav).
- A **code walkthrough** (`::: {.code-walkthrough}`) for the EM update step.
- **Exercises as callouts** (e.g. `::: {.callout-note}` / a custom-styled exercise).
- Figures + display math + at least one executable code cell.
- A draft appendix chapter (dropped + renumbered on `build`) to keep the numbering
  interaction honest.

**Why it stresses interactions no current doc does:** book numbering × theorem numbering
× cross-*page* refs × deck-embedded-in-a-book × walkthrough-in-a-chapter × draft-chapter
renumbering — none of these combinations are exercised together today (`demo-book` is a
minimal book pin; this is a dense one).

**Quality bar (the gallery constraint).** Narrow but deep: few chapters, but each
polished to showcase standard — real prose, correct math, clean figures, visually
complete light + dark. "Minimal" governs *breadth* (don't write a 200-page textbook),
never *polish*. This bar is what makes the doc gallery-worthy and, usefully, what
surfaces the polish/DX findings that minimal authoring would miss.

## 6. Gallery integration

The marketing site (`site/`, taliesin.dev) already has the exact mechanism:

- `mounts:` serves whole sub-projects under a URL prefix (that is how `docs/guide` +
  `docs/internals` are wired: mounted in `preview`, and the static `build` wires each
  with its own `build … --out` step).
- `showcase.tmd` ("See it live") shows inline **feature** demos (Result + Source).

The gallery is a clean extension, complementary to showcase:

- **Mount** each persona project under `/gallery/<name>` via an additive `mounts:` entry
  (e.g. `gallery/course: ../corpus/course`), and add the matching `build … --out` step
  to the static site build.
- **A new `site/gallery.tmd` page** ("Gallery" / "Built with Taliesin") with one card per
  project: title, one-line description, the capabilities it demonstrates, and a link to
  the mounted live project. Add a nav entry.
- Showcase = "here's a feature"; Gallery = "here's a whole real project."

**Sequencing of the gallery page.** A gallery of one is thin, so the *page* grows with
the slate. The **pilot** must nonetheless: (a) author the course to gallery quality, (b)
prove the standalone-build + mount packaging works end-to-end (a real probe of that
path), and (c) stub `site/gallery.tmd` with the first exhibit so the mechanism is
validated and the finding surface (build + mount) is exercised now. Later personas each
add a card.

**Scope note.** Marketing-site work is on-request (feature-first is relaxed for it); the
gallery angle is explicitly user-requested, so it is in bounds. It stays *secondary*:
the primary deliverable is the probe + corpus pin; the gallery is a quality overlay + a
page that reuses the existing `mounts:` mechanism (no new machinery).

## 7. Automated vs. browser-verified coverage

**Automated** (a pin test beside `crates/core/tests/corpus.rs`, e.g. `course.rs`):

- Block-model invariants (free via the existing corpus harness): unique block ids, valid
  sourcepos, document order, includes resolved.
- Cross-page refs resolve to the **right numbers** (assert the rendered ref text/target).
- Theorem numbering correct across chapters (per-kind + shared group).
- Book TOC / chapter nav present; draft appendix dropped + contiguous renumber on build.
- `check` is clean (or expected diagnostics asserted exactly).
- A plain `read` projection snapshot for the machine-facing view (ungated — no
  execution). Any `read --run` executed-output snapshot is a *separate*
  kernel/Chrome-gated test, mirroring the existing `read_run.rs` / `read_run_js.rs`
  pins; it must not make the core invariant pin depend on a kernel.
- Must not trip the corpus-wide clean-vocabulary guard.

**Browser-verified** (documented checklist, not committed automation — via the `preview`
skill + chrome-devtools): deck steps + embed, walkthrough scroll-focus, theorem
hover-preview / collapsible proofs, gallery card + mounted-project navigation, visual
layout light + dark, mobile + laptop-portrait + laptop-landscape (per the viewport
matrix).

## 8. Findings capture & triage

**Findings doc:** `notes/2026-07-22-corpus-demand-probe-course-author.md` (matches the
dated audit-doc convention in `notes/`).

**Each finding records:** title; category ∈ {`gap`, `friction`, `interaction-bug`,
`correctly-refused`}; severity (P1–P3); *what I wanted* → *what happened*; minimal repro;
**disposition** (roadmap item / backlog entry / no-op / correctly-refused-with-rationale).

**After the pilot:** fold actionable findings into `notes/backlog.md`; decide go/no-go
and any slate adjustments before scaling to the remaining three personas.

## 9. Guardrails

- **Respect the identity** (§2): no out-of-scope authoring; a settled-non-goal wall is
  `correctly-refused`, not a gap.
- **Authoring + findings only — no engine changes** in this pass; Do-NOT-touch machinery
  untouched by construction.
- **Corpus discipline:** the new project passes existing invariants (or asserts its
  expected diagnostics) and respects the clean-vocabulary guard.
- **Offline / no-CDN / `--tali-*` tokens only** for any gallery/site touch (the standing
  website constraints).
- **The gallery is additive** to `site/` (`mounts:` + one page + nav), reusing existing
  machinery; no new build path.

## 10. Deliverables & success criteria (pilot)

Pilot is done when:

1. `corpus/course/` is committed and renders **green** under the corpus invariants + a
   targeted `course.rs` pin test.
2. The course **builds standalone** (offline, self-contained) and **mounts** into `site/`
   under `/gallery/course`; `site/gallery.tmd` exists with the first exhibit card + nav
   entry; browser-verified across the viewport matrix, light + dark.
3. A findings doc exists with categorized findings, each with a repro + disposition;
   actionable ones folded into `notes/backlog.md`.
4. A **validated, written recipe** (this doc's §3, refined by pilot reality) ready to
   apply to the remaining three personas.
5. A go/no-go + any slate adjustments recorded.

## 11. Scaling to the remaining personas (after pilot go)

Apply the validated recipe to OSS-docs maintainer → interactive-explainer → analyst,
each: authored to gallery quality, packaged + mounted at `/gallery/<name>` with a card
added, pinned by a corpus test, mined for a findings doc. The gallery page fills out as
the slate grows. Each persona is its own plan/branch.
