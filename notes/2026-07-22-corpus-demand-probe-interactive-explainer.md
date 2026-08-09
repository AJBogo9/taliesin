# Corpus demand-probe — interactive-explainer persona (findings)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Persona #3 of 4.** An author of *explorable explanations*: one long, scrollable page
that teaches a concept by letting the reader poke at it. Built `corpus/descent/` — a
gradient-descent explainer that stacks the interactive cluster the corpus had only ever
exercised one feature at a time: three `{{< input >}}` sliders driving a **draggable**
`{js}` loss-surface graphic, a `.scrolly` whose sticky `{js}` figure redraws per scene, a
reactive **Observable Plot** loss-curve over the same sliders, display **math**, and two
numbered **authored SVG figures** with resolved `@fig-` cross-refs. Pinned by
`crates/core/tests/descent.rs` (7 tests) and exhibited as the third `/gallery/descent`
card. Distinct from personas 1-2 (both multi-chapter *books*): this is a **single-page
site** whose stress is client-side interactivity, not structure.

**Headline result: the whole stack works, standalone and mounted, with zero console
errors.** Sliders + drag coexist without state loss; the scrolly graphic tracks all five
scenes; the Plot chart reacts to every slider; math + figures interleave cleanly; the
page has no horizontal overflow at 500 / 1440 / 900px; the interactive SVGs adapt to the
dark theme via `--tali-*` vars. Four findings, all P3, all on secondary surfaces or the
machine-facing projection — no interaction bug in the live HTML view.

## Findings

### F-01, the `read` text projection concatenates structured blocks  [friction · P3]

**Wanted:** `taliesin read corpus/descent/index.tmd` should project reactive controls and
scrolly steps as separable units, so a machine reader (or a diff) sees structure.
**Happened:** two run-ons. (a) Each `{{< input >}}` control projects as its label glued to
its value with no separator: `step size (η)0.12`, `momentum (β)0`, `steps25`. (b) The
`.scrolly` `.step` narrations concatenate *across step boundaries*: `…there to the dark
point in the middle.Which way is downhill. At the start point…`.
**Repro:** `taliesin read` any doc with `{{< input >}}` controls and a `.scrolly`.
**Disposition:** backlog. **This is the third straight persona to hit the machine-facing
`read` projection seam** (course F-02: book-chapter refs/numbering; docs F-03: list items;
now inputs + scrolly steps). The recurrence *is* the signal: book-aware,
structure-preserving `read` is the highest-yield cross-persona item (backlog items 16 F-02
+ 17 F-03). The HTML renders all of it correctly; this is projection-only.

### F-02, `<img>`-embedded SVG figures ignore the page theme toggle  [gap · P3]

**Wanted:** an authored figure added the documented way — `![cap](fig.svg){#fig-x}` — that
tracks the reader's theme, dark labels on light and light labels on dark.
**Happened:** a numbered figure is emitted as `<img src="fig.svg">`, and an `<img>`-embedded
SVG is style-isolated: it cannot see the page's CSS custom properties *or* the page theme
toggle. To adapt, the SVG must carry its own `@media (prefers-color-scheme: dark)` — but
that follows the **OS** color scheme, not Taliesin's `qmd-theme` toggle. So a reader who
forces the page theme *opposite* their OS (page dark, OS light) gets the figure in its
light palette on the dark page: the gray axis labels drop to weak contrast.
**Repro:** open `corpus/descent` with OS light, toggle the page to dark, scroll to Figure 1.
**Disposition:** backlog (P3, edge case — the common case is page-theme == OS-theme, so the
figure matches). Candidates: (a) offer an inline-SVG figure path (`![]()` with a local
`.svg` could be inlined into the DOM instead of `<img>`-referenced, so it inherits
`--tali-*`); (b) document that authored SVG figures should use a neutral palette that reads
on both themes. The inline (`{js}`/SVG) graphics on this same page *do* track the toggle,
because they use `--tali-*` directly — only the `<img>` figures are stranded.

### F-03, a `{js}` "once" cell runs before its returned node is mounted  [WAI · authoring nuance]

**Wanted (as an author):** initialize a `{js}` cell's DOM by guarding on attachment, e.g.
`if (!svg.isConnected) return;` inside a redraw, expecting the first paint to run.
**Happened:** a cell's body executes and *returns* its node, and qmd-js mounts that node
**after** the function returns — so during the cell body (and its first synchronous
`redraw()`), the returned node is not yet in the document. An attachment-gated init
silently no-ops the first paint; the graphic shows up blank until the first input event.
**Repro:** in a `{js}` cell, build a node, call a paint fn that early-returns on
`!node.isConnected`, return the node — the initial paint is skipped.
**Disposition:** WAI (this is inherent to the return-a-node contract), but a **sharp edge**
worth a doc line: "your returned node is not mounted during the cell body; gate teardown on
`invalidation`, not on DOM attachment." Cost me one debug cycle. Candidate: a note in the
`{js}`-cell reference, or an optional post-mount callback.

### Confirmation (not a new finding): F-04 is kernel-only; it does **not** affect this persona

The pilot's F-04 ("a mounted sub-project's cells don't execute in the host `preview`") was
recorded as `{js}`/`{python}`. Re-verified this session, split by cell type: **client-side
`{js}` cells run correctly in a mounted preview** — the static mount render emits the
qmd-js runtime, the vendored Plot/d3 load under the mount prefix, `{{< input >}}` wires to
`qmd.value`, and live reactivity + the scrolly all work (browser-verified on
`/gallery/descent`, 0 console errors). Only **kernel** (`{python}`/`{r}`) cells fail in a
mount. So this persona's gallery exhibit is faithful in `preview` as-is, and **F-04 is a
persona-4 (analyst) prerequisite, not a persona-3 one** — the fix is deferred to that
session (owner decision, 2026-07-22).

## Progress log (which surfaces produced findings)

- **Scaffold + intro:** single-page website (`_site.yml`, no `chapters:`) builds clean (1 page). No findings.
- **Headline interactive (sliders + draggable start):** the load-bearing combination works — a "once" `{js}` cell reads sliders via `qmd.value`, subscribes with `qmd.onInput` to redraw in place (no teardown), and owns a pointer-capture drag, so sliders and drag coexist without losing the dragged start. Surfaced **F-03** (once-cell-not-yet-mounted, during authoring). Browser-verified: 26-point path, live divergence message, drag moves the start.
- **Math + Figure 1:** display + inline math renders (31 KaTeX); the authored theme-adaptive SVG numbers as Figure 1 and `@fig-landscape` resolves. Surfaced **F-02** (img-SVG theme isolation) on the dark-mode pass.
- **Scrolly (5 scenes):** the sticky `{js}` graphic keys off `qmd.value("scene")` and tracks landscape→gradient→step→iterate→diverge exactly as each `.step` scrolls in; URL syncs `scene=`. No finding (works).
- **LR story + reactive Plot + momentum + callouts:** the Plot loss-curve is a reactive sink over `lr,beta,steps` (rebuilds on change); Figure 2 (momentum) numbers + `@fig-momentum` resolves; callouts render. No finding.
- **`read` probe:** surfaced **F-01** (input + scrolly-step concatenation). Math projects well (display kept as LaTeX, inline → unicode); figures give `Figure N: <cap>` + `[image: <alt>]`; callouts give `[warning] <title>`.
- **Pin test `descent.rs` (7):** green; full `taliesin-core` suite green (622-test corpus invariant renderer included); clippy `-D warnings` clean.
- **Gallery:** additive `mounts:` entry + third card; `check site` clean; `build site` + `build corpus/descent --out …/gallery/descent` both succeed, mount artifact offline-complete (`_assets` jslibs/katex + both SVGs + search-index). Browser-verified the **mounted** exhibit end to end, 0 console errors.

## Roll-up

**One long interactive page authored, 3 findings + 1 program-level confirmation.** All
findings P3; zero interaction bugs in the live HTML view. Finding overlap with personas
1-2 is, again, a *confirmation*: the `read` projection seam (F-01) recurs a third time,
which is the strongest signal yet that structure-preserving `read` is the item to pull
forward. The two genuinely new seams are both on secondary surfaces: img-SVG figures don't
follow the theme toggle (F-02), and the once-cell mount timing (F-03). The headline
interaction budget — sliders × drag × onInput coexistence — worked on the first honest
try, which is the load-bearing thing for an explorable-explanation authoring story.

## Retro

**Did the recipe (spec §3) hold?** Yes, a third time. Author-for-real → log resistance →
`read` probe → pin → gallery worked cleanly, and this persona confirmed the recipe scales
past *books* to a single-page interactive doc with no method change. The demand-probe value
here was, as before, less "the tool is broken" (it is not) and more "here are the exact
seams a real explorable-explanation author meets": the projection run-ons, the img-SVG
theme gap, and the once-cell timing edge — none blocking, all now on the backlog.

**Verdict: GO for persona 4 (computational-report analyst).** Slate note: persona 4 is the
one that *does* hit F-04 (heavy `{python}`+`{r}` exec, mounted), so the F-04 fix (deferred
here on evidence) should open that session. This persona also leaves a reusable interactive
kit — the anisotropic-bowl loss, the `qmd.onInput`-redraw drag pattern, the scene-keyed
scrolly graphic — that the analyst persona can lean on where it wants a live figure over
executed data.
