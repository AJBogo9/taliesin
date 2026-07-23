# CAD-as-code feature: feasibility, licensing, and market research

**Date:** 2026-07-23
**Status:** PARKED. Feasible and legally clean, but no market demand. Not started.
**Decision rule (owner):** "If there is no market demand for this feature I'll not implement it."
By that rule the answer is **no, do not build it now** (see Revisit triggers to know when to reopen).

## The idea

Let an author write "CAD as code" (OpenSCAD, or a Python CadQuery/build123d cell) in a `.tmd`
code block and have Taliesin render an interactive 3D model in the browser preview, so the model
is version-controlled as text and editable anywhere without CAD software. Origin motivation was
"so people doing CAD could use the tool," later refined by the owner to "I want to visualize and
version-control CAD models" (personal use) and "can this be commercialized."

## Verdict in three lines

1. **Feasible: yes**, and a genuinely clean architectural fit (it reuses existing machinery, is
   not a new output format, and respects the read-only-preview invariant).
2. **Licensing / commercialization: green.** The tool and the models are legally sellable via an
   arm's-length subprocess design (get counsel before charging money).
3. **Market demand: no.** CAD workers are the wrong audience; code-CAD is a niche within the
   hobbyist 3D-printing niche; nobody in Taliesin's peer group asks for CAD-in-a-document. The
   only person in the target set is essentially the author.

## Feasibility + recommended architecture

Not a new output format (still HTML plus an interactive canvas), so it does not touch the HTML-only
invariant; the rendered mesh is display-only, so the read-only-preview invariant holds (edits still
flow through the `.tmd`). Existing machinery it reuses:

- `{js}` cells already dynamically `import()` Three.js (see `crates/core/assets/js/qmd-js.js`).
- Heavy assets are already bundled and gated per-page (mermaid at 2.5 MB), so "too heavy to bundle"
  is a weak objection: a CAD viewer would be gated the same way.

Two clean paths (avoid a third):

- **Preferred: `{openscad}` cell via subprocess.** The server invokes a *user-installed* `openscad`
  (`openscad -o model.stl model.scad`), then a bundled MIT Three.js viewer renders the STL. Best
  LLM fluency (OpenSCAD has the largest training corpus), license-clean (see below), on-model for
  Taliesin (server re-renders on edit, same loop as a `{python}` cell).
- **Alternative: `{python}` build123d cell.** Runs in the existing warm Jupyter kernel, exports a
  `.glb`, a bundled Three.js / `<model-viewer>` renders it. Reuses the kernel + freeze cache with
  almost no new execution infrastructure; true B-rep; but the OpenCascade dep is heavy (~88 MB
  `cadquery-ocp` wheel in the user's env) and build123d's export is cleaner than CadQuery's
  (CadQuery exposes glTF only on `Assembly`, not a bare `Workplane`).
- **Avoid: bundling openscad-wasm.** It is GPL-2.0; `include_str!`-ing it would force the whole
  Taliesin binary to GPL.

Viewer facts (all permissive, offline-bundleable): Three.js MIT (~687 KB min, ~160 KB gzip) plus a
loader + OrbitControls; or Google `<model-viewer>` Apache-2.0 (~979 KB min, glTF-only, graceful
`poster` fallback). Rendering is the cheap, solved part; geometry is the expensive part, so
tessellate server-side (subprocess/kernel) and ship a mesh, never ship a multi-MB OCCT wasm into
the page. Needs WebGL2; no-GPU falls back to slow SwiftShader, so add a poster/fallback.

## Licensing + commercialization (the owner's direct question)

**Can LLM-authored CAD code be commercialized? Yes, on both counts:**

- **The tool.** A proprietary/commercial Taliesin can shell out to a *user-installed* GPL OpenSCAD
  via an arm's-length CLI subprocess without becoming GPL. This is the FSF's own "mere aggregation"
  position (pipes / CLI args / files between separate programs is not a combined work). Two hard
  constraints: (1) never bundle or redistribute the OpenSCAD binary (the user installs it, like the
  Jupyter kernel); (2) keep the interface a plain "write `.scad`, run `openscad -o out.stl`" file
  call, not intimate shared-memory IPC. No judicial precedent exists, so it is defensible, not
  bulletproof: **get counsel before charging money.** Consequence: you do NOT need a separate
  edition that strips the feature. The subprocess design keeps both open-source and commercial
  editions clean, because no GPL code is distributed in either.
- **The output.** OpenSCAD's GPL does not reach the `.scad` files or STL a user creates: output of
  a GPL tool is the user's own (FSF; GPL §0). The one snag is a GPL-licensed OpenSCAD *library* a
  user chooses to `include`, whose license can attach to the model (a per-library disclosure
  concern, not a blanket one; BOSL2 is BSD, MCAD is LGPL/varied).
- **AI-authored code copyright.** The US Copyright Office (Jan 2025) holds that pure prompt-to-code
  output is not copyrightable, so a user may lack an exclusive monopoly on the purely-AI parts. But
  they can still freely use, sell, and manufacture the model, and substantial human editing (which
  Taliesin's edit-the-source workflow naturally involves) restores protectable authorship.

Net: "commercializable" is true in the sense that the tool and the models are legally sellable, NOT
that a market is waiting to pay for this specific feature.

## Market research (why demand is the blocker)

Consistently undercuts the case for building on market grounds:

- **CAD workers are the wrong audience, structurally.** ~18M+ professional mechanical engineers
  work in GUI parametric modelers (SolidWorks ~3M, Fusion ~2M), assemblies, 2D drawings, and
  PDM/PLM vaults. None of that is text you author in a document. Category mismatch, not a marketing
  gap. (User-count figures are unaudited market-firm/vendor estimates; directionally reliable.)
- **Code-CAD is a niche within the hobbyist 3D-printing niche.** GitHub stars: OpenSCAD ~9.8k,
  CadQuery ~5.5k, build123d ~2.7k. Realistically tens of thousands of active humans. The flagship
  OpenSCAD has shipped no stable release since 2021.01 (issue #6664, Feb 2026, flags the 5-year
  gap). Momentum is shifting to the Python side (build123d rising).
- **Even code-CAD users do not author in documents.** They live in the OpenSCAD GUI, VS Code +
  ocp-vscode, or cq-editor. The one true CAD-in-notebook analog, jupyter-cadquery, has ~397 stars
  and fewer than 10 contributors. The ecosystem tilts ~3:1 toward standalone editors over docs.
- **Taliesin's actual peer group is silent.** Quarto, Jupyter Book, and mdBook have no
  CAD-embedding feature request with traction. (sphinxcadquery proves the pattern is feasible and
  non-zero, so the tech works; the demand just is not there.)
- **Text-to-CAD demand is real but modest, hype-inflated, and mis-routed.** It lives in dedicated
  apps (Zoo ~$5M raised, Adam $4.1M seed, and Adam is pivoting away from consumer text-to-3D).
  Incumbents (Onshape, PTC, Siemens, Dassault) ship AI copilots/advisors/drawing-automation, not
  prompt-to-geometry. Practitioners are skeptical about real parts (tolerances, assemblies,
  moldability). LLM CAD is reliable for simple single parametric parts and unreliable on precise
  dimensions, orientation, and assemblies; "executable" is not "correct."
- **Anti-signal specific to a document tool.** Text-to-CAD's core need is a geometry→code
  debugging round-trip ("point at the wrong face"), which a read-only preview deliberately does not
  provide.
- **The only supporting sliver** is the intersection {programmer who writes computational
  documents} ∩ {enjoys code-CAD}, which is precisely Taliesin's single-author archetype. That is
  the overlap of two niches, not a market.

## Revisit triggers (reopen this if any becomes true)

1. **Author-pull (the legitimate one for this tool).** You actually want to write a `.tmd` doc that
   is better with a live parametric model in it: a 3D-printing build log, a parametric-design
   explainer, a hardware/mechanism tutorial, a geometry/physics teaching piece. Under
   corpus-plus-roadmap this is a valid reason on its own (a doc you want to write pulls the feature
   in). Name the pin doc and it graduates.
2. **Peer-group demand appears.** Quarto / Jupyter Book / mdBook ship native embedded-CAD, or a
   feature request for it gets real traction (upvotes, maintainer interest). That would show the
   document-tool audience actually wants this.
3. **Notebook-CAD demand grows materially.** jupyter-cadquery / build123d-in-notebook usage
   multiplies (say jupyter-cadquery clears low-thousands of stars, or a mainstream notebook
   platform adds native CAD), signaling the crossover audience is expanding.
4. **Text-to-CAD reliability crosses a threshold AND moves in-document.** LLM CAD becomes
   trustworthy on dimensions/assemblies and the workflow shifts toward "iterate in a live doc
   preview" rather than dedicated apps.
5. **A concrete external ask.** A course, client, collaborator, or grant scope specifically wants
   Taliesin to render parametric CAD.

## If revived: the pre-decided path (so no re-research)

- Build the **`{openscad}` cell via user-installed subprocess → STL → bundled MIT Three.js viewer**
  (or `{python}` build123d → glTF if B-rep fidelity is wanted). Gate the viewer per-page like the
  other heavy assets. **Do not bundle openscad-wasm (GPL).**
- Ship it pinned by a real corpus doc (a 3D-printing build log or parametric explainer) per
  corpus-plus-roadmap.
- Commercialization stays clean via the arm's-length subprocess boundary; confirm with counsel
  before charging.

## Provenance

Two background research workflows (5 parallel web-research agents each) on 2026-07-23: pass 1 =
technical feasibility (browser-native engines, kernel-side Python, web viewers, LLM-authoring
quality, licensing); pass 2 = market demand (community size, professional-CAD audience,
text-to-CAD demand, embedded-CAD-in-docs precedent, commercialization + ownership). Some specific
LLM-benchmark numbers surfaced looked synthetic/future-dated and are not relied upon; the
directional findings (small niche, wrong audience, no doc-tool demand, LLM ceiling) are robust and
rest on high-confidence signals (GitHub stars, peer-group silence, audience structure, the FSF and
US Copyright Office positions). Memory topic file: `cad-as-code-feature-evaluation` (private recall).
