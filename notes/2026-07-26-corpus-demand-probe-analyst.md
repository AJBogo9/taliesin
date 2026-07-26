# Demand probe #4: the computational-report analyst (2026-07-26)

**Persona:** the fourth and last slate entry from
[the demand-probe design](../docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md)
§4 — *"heavy python+R exec + many figures/tables + freeze under realistic volume"*.
**Artifact:** `corpus/analyst/`, pinned by `crates/core/tests/analyst.rs`, exhibited at
`/gallery/analyst`. Run against `4006143`, release build, real Python 3.12 + R 4.3.3
kernels. This closes the program at 4 of 4 and closes backlog item 30.

## Headline

**The un-probed shape was never "more execution", it was "two languages in one
document"** — and that is where every finding came from. The corpus has Python docs
(`posts/`) and R docs (`bayesian-website/`) and had **no document with both**. Volume
turned out to be the well-covered axis; the language *seam* was the bare one.

The interaction stack itself is sound: the report built and executed correctly on the
first attempt, `check` was clean from the first run, and the numbering that spans the two
languages is right. **Two defects, both fixed in this batch** — AN-1 (a dangling
cross-reference, found in the HTML) and AN-2a (every R figure rasterised onto opaque
white on a dark-by-default page, found only in the *browser*). The remaining four are
friction on secondary surfaces, filed rather than built.

**Both fixes are the same shape, which is the finding under the findings: the R arm of a
two-arm facility was never built.** `figure_wrap` had a fallback and `table_wrap` did
not; `KernelSpec::python` carried two startup preambles and `KernelSpec::r` carried an
empty list. Neither gap is visible from inside a single-language document, and every
corpus document was single-language.

The probe also re-tested yesterday's `MAX_BYTES` freeze cap on a real mixed-language
document rather than a synthetic one, which is the closest thing to an independent
confirmation that fix will get.

---

## AN-1 (P2, interaction-bug) — a labelled table cell silently emits a dangling cross-reference. FIXED

**What I wanted.** `#| label: tbl-coefs` on the R cell that prints the coefficient table,
and `@tbl-coefs` in the prose.

**What happened.** The prose rendered `<a href="#tbl-coefs">Table&nbsp;3</a>` and
**nothing in the document carried `id="tbl-coefs"`.** A live link to nowhere, in a built
page, with `taliesin check` reporting *"no problems found"*.

**Root cause**, at `crates/server/src/exec.rs`'s `table_wrap`: it searched the output for
`<table` and, finding none, *returned the output unchanged*. But by then the number is
already spent and the reference already rewritten — `render::apply_table_captions`
numbers and registers `tbl-x` from the **label**, with no knowledge of what the cell will
print. So the anchor had nowhere to land and the failure was silent.

Two things make it worse than a one-off:

- **The figure path never had this bug.** `figure_wrap` wraps unconditionally, so a
  `fig-` cell that produces no image still emits its anchor. The two paths disagreed,
  and only the table one dropped the id.
- **`check` structurally cannot see it.** `check` never executes a cell, so no amount of
  `check` coverage would reach it. (Same class as the two build-only diagnostics DIAG-1
  could not see from the `check` side.)

**Measured on both paths, so it is not R-specific:**

| build | `id="tbl-…"` present | `id="fig-…"` present |
|---|---|---|
| real kernels | `tbl-slo` (authored) only | both `fig-p95`, `fig-effects` |
| no kernel at all | `tbl-slo` only | both |

The two executed tables were missing in *both* builds; the two executed figures were
present in both.

**Fix (shipped).** `table_wrap` now falls back to `table_figure_wrap`, carrying the
caption and anchor on a `<figure class="tali-figure tali-table-figure">` wrapper — the
same degradation `figure_wrap` has always had, with the caption leading because a table's
caption sits above it. One CSS rule flips the figure caption's margin. The in-place path
for a real `<table>` is untouched.

**Verified by mutation:** restoring `return inner.to_string()` fails
`a_labelled_table_cell_that_prints_text_still_carries_its_anchor` with *"the anchor must
survive a non-table output, else @tbl-coefs dangles"* and
`a_table_cell_that_never_ran_still_carries_its_anchor` with it.
`a_real_table_output_is_still_captioned_in_place` guards the unchanged path.

## AN-2a (P2) — every R figure rasterised onto opaque white, on a dark-by-default page. FIXED

**Found in the browser, not in the HTML.** The built markup looked right all along; the
rendered page showed the ggplot figure as a **glaring white slab** on the dark theme.

R's inline graphics device is opened with an opaque background — `repr.plot.bg` defaults
to `"white"` (measured through a live kernel, not read off a doc). So making ggplot's own
`plot.background` and `panel.background` transparent, which is the documented way to do
it, **still produced a white figure**: the transparency was real, and the device was
painting white underneath it.

Python has never had this problem because Taliesin configures it away at kernel start —
`MPL_THEME_PREAMBLE` sets `InlineBackend.print_figure_kwargs = {'facecolor': 'none'}`.
`KernelSpec::r`'s `preambles` list was **empty**.

**Fix (shipped).** An R preamble, the exact counterpart of the Python one:

```r
options(repr.plot.bg = "transparent")
```

**The safety question is the whole design here, so it was measured rather than argued.**
Making every existing R figure transparent would be a regression, not a fix: dark text on
a dark page. It isn't one, because a figure that never asked for transparency paints its
own background — a default `ggplot` (`theme_grey` fills `plot.background` white) and
base-R graphics both still rasterise as **8-bit RGB with no alpha channel at all**. All
the preamble removes is the white the *device* painted under a figure that had already
asked to be transparent.

**Verified by mutation:** emptying `preambles` fails
`a_transparent_r_figure_keeps_its_alpha_and_a_default_one_stays_opaque` with `left: 2,
right: 6` (opaque RGB where RGBA was required). That test asserts **both** halves — the
transparent figure keeps its alpha *and* the default one stays opaque — because a test
that only checked the first would pass on the regression above. Confirmed in the browser:
the figure's corner pixels read `[0,0,0,0]`.

## AN-2b (P3, gap) — matplotlib figures follow the page theme; R figures cannot

Taliesin renders every inline matplotlib figure **twice**, once with the light theme's
foreground and once with the dark theme's, and swaps them on the theme toggle
(`kernel.rs`'s `MPL_THEME_PREAMBLE`). Measured on the readout: the Python figure emits
two PNGs (70,184 and 70,164 bytes, genuinely different renders); the ggplot figure emits
one.

There is no R equivalent, so **in a mixed-language report half the figures track the
reader's theme and half are baked**. Nothing is broken — but the page has two figures
that behave differently, which is exactly the seam a single-language document cannot
show. The R figure is also emitted as `<img alt="output">` where the Python pair is
`alt=""`; both sit inside a `<figcaption>`-bearing `<figure>`, so `alt=""` is the correct
one and `"output"` is noise a screen reader reads out.

**Worked around in the document, not in the engine:** the R cells use a neutral mid-grey
for every axis, label and gridline, so the baked figure is legible on both themes. That
is the "document a neutral-palette convention" option named in backlog item 18's F-02,
now with a second instance behind it. A real fix means re-rendering an R figure twice
against two foregrounds, which is a feature, not a drive-by — and note it is a *separate*
question from AN-2a: transparency lets the page show through, but the ink in the figure
is still baked at one colour.

## AN-3 (P3, friction) — the natural R table idiom silently prints its own markup

`knitr::kable(format = "html")` returns a `knitr_kable` **string**. Printed from a bare
Jupyter R kernel it goes to stdout, so Taliesin receives a text stream and renders it in
a `<pre>` — the reader sees `&lt;table&gt;`, escaped, as source. Under knitr/rmarkdown
the same call works, because knitr splices the string in itself; that is what makes this
a trap rather than an obvious mistake.

The fix is one wrapper, `IRdisplay::display_html(as.character(kable(...)))`, and the
readout now uses it (rendering a clean `<table>` with no vendor classes). It is
undocumented: `docs/guide/using/code.tmd` documents `#| tbl-cap:` without saying that an
R cell must *publish* HTML rather than print it. **Candidate: a line in the guide's code
page.** Note this finding is what exposed AN-1 — the dangling anchor only became visible
because the output was not a table.

## AN-4 (P3, friction) — a bare pandas DataFrame carries vendor markup into the page

`display(df)` (or a trailing `df`) emits pandas' `_repr_html_`: a `<table border="1"
class="dataframe">`, a row-index column, and a `<style scoped>` block. **`scoped` was
removed from the HTML standard and no current browser implements it**, so that style
element is injected into the document body and applies page-wide (its selectors are
`.dataframe`-prefixed, so the blast radius is small, but it is emitted once per table).

`to_html(index=False, border=0)` produces clean markup that the page's own table styling
reaches, and the readout uses it with a comment saying why. Same disposition as AN-3: an
authoring nuance worth one documented line, not engine work.

## AN-5 (P3, friction) — a cross-page `@sec-` renders as the bare word "Section"

`@sec-model` from `methods.tmd` to `index.tmd` builds
`<a href="index.html#sec-model">Section</a>` — correct target, **no number and no
title**, so the sentence reads "…as set out in Section." The same reference *on its own
page* renders "Section 3".

**The obvious fix is already refuted in-source, which is the point of filing this
carefully.** `site/mod.rs`'s `harvest_xref_numbers` excludes `sec-` deliberately, and its
comment says why: harvesting the render's flat per-page section counter would fill a
website target with a bare "1", which `rewrite_one_xref` then mislabels **"Chapter 1"**.
So "just harvest the number" is wrong and was already considered.

What is left is the *label*: with no number available, the bare kind word carries no
information. **Candidate:** carry the heading's title in `XrefTarget` (it is right there
in `scan_page_anchors`, which already reads the heading line) and use it when the number
is empty — "Section “Is the canary still slower?”". Note `XrefTarget: PartialEq` drives
the dev server's "did a target move" check, so adding a field makes a heading edit
re-render referring pages, which is correct but should be intended rather than
discovered. Cross-page `@fig-` and `@tbl-` are **not** affected: both resolved to the
right numbers (Figure 1, Table 3), because floats *are* harvested from the render.

## AN-6 (P3, friction) — the editor reports valid cross-page references as errors

Every cross-page `@tbl-`/`@fig-`/`@sec-` in `methods.tmd` draws a red
`TAL-XREF-UNDEF: broken cross-reference` in the editor, while `taliesin check
corpus/analyst` reports *"no problems found"* and the built page resolves all of them
correctly. `taliesin check corpus/analyst/methods.tmd` (single-document mode) agrees with
the editor and exits 1.

So this is not an LSP bug so much as a **scope mismatch**: the language server has no
`Site::discover` and is per-document by construction, but the *project* is a site, so the
author is shown errors on correct content in the file they are editing. An author who
trusts the squiggle deletes a working reference; one who learns to ignore it stops
reading the diagnostics that matter. **Candidate:** have the LSP resolve the enclosing
`_site.yml` project (it already knows the file's path) or, much cheaper, downgrade an
unresolved `sec-`/`fig-`/`tbl-` to a hint when the document sits inside a site project.

---

## Measured and healthy (do not re-scope)

**Cross-language freeze isolation holds.** Editing one language's cell never re-executes
the other's. Measured both directions: a Python cell-body edit re-ran 3 Python cells and
0 R cells; an R cell edit re-ran 3 R cells and 0 Python. Across a 140-edit warm preview
run the steady state was **"restored 5 cached cells · 1 re-ran"** in 123 of 140 samples —
only the edited cell.

**The `MAX_BYTES` cap binds correctly on a real document.** 140 cell-body edits against a
warm preview, sampling the on-disk freeze after each render:

| edit | `_freeze/index.json` bytes | entries |
|---|---|---|
| 1 | 168,712 | 6 |
| 20 | 3,017,692 | 26 |
| 60 | 8,732,296 | 66 |
| 100 | 14,446,416 | 106 |
| 120 | 16,737,864 | 122 |
| 140 | 16,748,936 | **120** |

Linear at ~143 KB/edit (one dual-theme figure pair per edit) until the 16 MB budget binds
near edit 117, then a plateau with the **entry count falling**. The 1024-entry count cap
never came close — exactly the failure mode AP1-R1 was about, now confirmed on a real
mixed-language page. The working set kept being restored throughout, so eviction took the
abandoned edit history, not the live cells.

**Also correct, first try:** one table counter spanning the authored `: caption {#tbl-}`
path and the executed `#| label: tbl-` path in document order (Table 1 authored, 2
Python, 3 R); one figure counter spanning both languages (Figure 1 Python, 2 R);
cross-page `@tbl-`/`@fig-` to *cell-produced* floats carrying the right page **and**
number; `check` clean on the project from the first run; both kernels warming and
executing in one build.

## Not measured, stated so it is not mistaken for a clean bill

- **Per-edit render latency.** The probe's loop carries a fixed 0.35 s settle before it
  fetches the page, which floors the measurement well above the effect. The flat
  `elapsed_s` column is **not** evidence that the plateau is free.
- **Which keys the cap evicted, in what order.** The plateau and the restore behaviour
  are directly measured; the per-key eviction sequence is not, and the reconstruction I
  attempted from entry sizes did not survive contact with the log. Not needed for any
  claim above, so it is left unmeasured rather than guessed.
- **R under the warm pool.** Only Python is pre-warmed; the R kernel is started per
  build. Whether R belongs in the warm pool is a separate question this did not ask.
- **The gallery exhibit's executed output in CI.** The exhibit is the only one whose
  pages execute, so a machine without both kernels builds it with placeholders. That is
  recorded in `site/README.md`, not solved.

## Programme note: the slate is finished, and the yield curve is real

Four personas, four artifacts, and the disposition is consistent with what items 16-18
already recorded: **zero interaction bugs between the stacked features themselves**, and
every finding on a secondary surface. Personas 1-3 found 0; this one found 1 real defect
(AN-1) and it was at a *language seam*, not a feature seam.

The lesson worth keeping is about **slate design, not probe count**: the three earlier
personas each stacked features the corpus had not combined, and found nothing, because
the features compose. This one stacked a *dimension* the corpus had not crossed (two
kernels in one document) and immediately found a path where one arm had a fallback and
the other did not. A fifth persona is not indicated; **a fifth un-crossed dimension might
be.** None is currently known.
