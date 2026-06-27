# Marketing Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. This is browser-verified CONTENT work (authoring `.qmd`), not unit-tested code: each task's "test" is `qmd-fast check site` + a build + a chrome-devtools MCP browser pass. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add live, self-referential, genuinely-flashy capability demos to the qmd-fast marketing site so the page itself is the proof, replacing passive screencasts with interactive WebGL + reactive graphics the visitor can touch.

**Architecture:** A new `site/showcase.qmd` page plus a live 3D hero on `site/index.qmd`, authored entirely with existing qmd-fast features (`{js}` cells importing Three.js from esm.sh, `{{< input >}}` reactive controls, `.scrolly`, `.code-walkthrough`, `.panel-tabset`). 3D reuses the corpus `makeScene3D` helper, copied to `site/_includes/`. No engine changes.

**Tech Stack:** qmd-fast `{js}` cells, Three.js + OrbitControls (esm.sh), vendored Observable Plot + d3, server-side KaTeX, the native deck engine for the embedded deck.

## Global Constraints

- **Content-only.** No changes under `crates/` or `web-client/`. If a demo seems to need an engine change, rethink the demo. Verbatim from spec.
- **Self-referential reveal.** Every demo is a `::: {.panel-tabset}` with a `## Result` tab (the live demo) and a `## Source` tab (the SAME code in a non-executing fence). The Source tab is a hand-kept copy of the Result cell body; keep them identical.
- **No em dashes or en dashes** anywhere (use commas/colons/parentheses).
- **prefers-reduced-motion:** auto-rotation and scroll/RAF animation must pause to a static, legible state when `window.matchMedia("(prefers-reduced-motion: reduce)").matches`.
- **Mobile:** legible and usable at 390px wide; canvases scale to container width (`max-width:100%`), sliders reachable, tabsets stack.
- **Performance:** `renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))`; `cancelAnimationFrame` + `renderer.dispose()` + `ctrl.dispose()` on the cell's `invalidation`; lazy-init heavy 3D via `IntersectionObserver` (scene starts only when scrolled near).
- **Theme-aware:** 3D renderers use `alpha:true` (transparent) so they follow the page light/dark; no hard-coded backgrounds that fight the theme.
- **esm.sh** for Three.js (same as the corpus). Each 3D cell has an error/fallback path (a short message) if the import fails.
- **Green tree:** `qmd-fast check site` clean; `qmd-fast build site` succeeds; browser console error-free.

---

### Task 1: Scaffold the showcase page + nav + shared 3D helper

**Files:**
- Create: `site/_includes/three-scene.qmd` (copy of `corpus/_includes/three-scene.qmd` verbatim: the `//| name: makeScene3D` helper)
- Create: `site/showcase.qmd`
- Modify: `site/_site.yml` (add nav entry)

**Interfaces:**
- Produces: a `makeScene3D(buildScene, invalidation, opts)` reactive helper available to later 3D demos via `{{< include _includes/three-scene.qmd >}}`; a published `{js}` name `makeScene3D` consumed by Task 2.

- [ ] **Step 1: Copy the 3D helper into the site project**

```sh
cp corpus/_includes/three-scene.qmd site/_includes/three-scene.qmd
```

(It is the `makeScene3D` cell: imports Three + OrbitControls from esm.sh, caps DPR, builds renderer/camera/controls, runs the RAF loop, adds a fullscreen button, and disposes on `invalidation`.)

- [ ] **Step 2: Create `site/showcase.qmd` scaffold**

```markdown
---
title: "See it live"
description: "Interactive demos of qmd-fast, every one a real capability with its source beside it."
page-layout: full
---

# See it live {.unnumbered}

Everything below runs in your browser, right here on this page, which is itself a
qmd-fast document. Each demo shows its **Result** and the real **Source** that
produced it: open the Source tab and you are reading the exact `.qmd` cell. No
screenshots, no video.

{{< include _includes/three-scene.qmd >}}
```

- [ ] **Step 3: Add the page to the site nav**

In `site/_site.yml`, add to `nav.left` after the "Document types" entry:

```yaml
    - { text: "See it live", href: showcase.qmd }
```

- [ ] **Step 4: Build + check**

Run:
```sh
QMD_FAST_NO_EXEC=1 ./target/release/qmd-fast build site --out /tmp/showcase-out && ./target/release/qmd-fast check site
```
Expected: build succeeds, `showcase.html` emitted, nav shows "See it live"; `check` prints "no problems found".

- [ ] **Step 5: Commit**

```sh
git add site/_includes/three-scene.qmd site/showcase.qmd site/_site.yml
git commit -m "feat(site): scaffold the live showcase page + nav + 3D helper"
```

---

### Task 2: Demo 1, the reactive 3D hero (showcase + index)

**Files:**
- Modify: `site/showcase.qmd` (append the demo)
- Modify: `site/index.qmd` (place the same hero near the top, after the lead paragraph)

**Interfaces:**
- Consumes: `makeScene3D` from Task 1.
- Produces: reactive names `detail`, `twist`, `amplitude` (from `{{< input >}}`).

- [ ] **Step 1: Author the reactive 3D demo** (append to `site/showcase.qmd`)

A parametric wave-interference surface the visitor orbits; three sliders reshape it. Lazy-inits on scroll, honors reduced-motion, disposes on invalidation. The `Result` tab holds the live cell; the `Source` tab is the identical code in a `js` fence.

````markdown
## A parametric surface you can shape and spin

::: {.panel-tabset}

## Result

{{< input name="detail" type="slider" min="16" max="80" step="4" value="48" label="detail" >}}
{{< input name="twist" type="slider" min="0" max="6" step="0.2" value="2" label="twist" >}}
{{< input name="amplitude" type="slider" min="0" max="1.4" step="0.05" value="0.7" label="amplitude" >}}

```{js}
//| input: detail, twist, amplitude
//| echo: false
const detail = qmd.value("detail"), twist = qmd.value("twist"), amp = qmd.value("amplitude");
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

// lazy mount: build the scene only when scrolled into view
const host = document.createElement("div");
host.style.cssText = "min-height:420px";
const build = async () => {
  let canvas;
  try {
    canvas = await makeScene3D((scene, THREE) => {
      const n = Math.round(detail);
      const geo = new THREE.PlaneGeometry(6, 6, n, n);
      const p = geo.attributes.position;
      for (let i = 0; i < p.count; i++) {
        const x = p.getX(i), y = p.getY(i), r = Math.hypot(x, y);
        p.setZ(i, amp * Math.sin(r * twist - 0) * Math.cos(x * 0.9));
      }
      geo.computeVertexNormals();
      const mat = new THREE.MeshStandardMaterial({
        color: 0x4c8dff, metalness: 0.1, roughness: 0.55,
        wireframe: false, flatShading: false, side: THREE.DoubleSide,
      });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.rotation.x = -Math.PI / 2.4;
      scene.add(mesh);
      const key = new THREE.DirectionalLight(0xffffff, 1.1); key.position.set(4, 6, 5);
      scene.add(key);
    }, invalidation, {
      width: 620, height: 420, alpha: true, cameraPos: [0, 5, 7],
      ambientIntensity: 0.9, autoRotate: !reduce,
    });
  } catch (e) {
    canvas = document.createElement("p");
    canvas.className = "qmd-muted";
    canvas.textContent = "3D preview needs network access to load Three.js.";
  }
  host.replaceChildren(canvas);
};
if (reduce) build();
else { const io = new IntersectionObserver((es, o) => { if (es[0].isIntersecting) { o.disconnect(); build(); } }); io.observe(host); invalidation.then(() => io.disconnect()); }
return host;
```

Drag to orbit, scroll to zoom, press the button for fullscreen. Move a slider and
the mesh rebuilds live. The whole thing is the cell in the **Source** tab.

## Source

```js
//| input: detail, twist, amplitude
//| echo: false
const detail = qmd.value("detail"), twist = qmd.value("twist"), amp = qmd.value("amplitude");
// ... (identical to the Result cell above; the verifier confirms the two match) ...
```

:::
````

NOTE during execution: paste the FULL Result cell body verbatim into the Source fence (no elision); the elision above is only to keep the plan readable. The verification step diffs the two.

NOTE on `makeScene3D`: it does not currently accept `autoRotate`. In Task 1's copied helper, add `autoRotate = false` to the `opts` destructure and `ctrl.autoRotate = autoRotate; ctrl.autoRotateSpeed = 0.6;` after the controls are built (this is editing the SITE's copy of the helper, not the corpus or engine). Fold this edit into this task.

- [ ] **Step 2: Place the hero on the index**

In `site/index.qmd`, after the lead paragraph (around line 17, before the ` ```sh ` block), insert an `{{< include _includes/three-scene.qmd >}}` and a compact version of the 3D cell (no sliders on the index, just an auto-rotating orbitable surface, reduced-motion aware) under a short heading like "Markdown in. This out." Keep it self-contained.

- [ ] **Step 3: Browser-verify** (start a site preview, then drive chrome-devtools)

```sh
QMD_FAST_NO_EXEC=1 ./target/release/qmd-fast preview site 4390
```
(Three.js still loads under `--no-exec`: it is a `{js}` cell, browser-side, not a kernel cell.)
Verify on `http://127.0.0.1:4390/showcase.html` and `/`:
- 1440x900 and 390x844 and 900x1440; light and dark.
- The mesh renders, orbits on drag, zooms on scroll; each slider reshapes it.
- Console has zero errors.
- Emulate reduced-motion: auto-rotate is off, mesh static but still orbitable.
- The Source tab shows the full real cell, identical to Result.

- [ ] **Step 4: Commit**

```sh
git add site/showcase.qmd site/index.qmd site/_includes/three-scene.qmd
git commit -m "feat(site): reactive 3D hero demo (showcase + index)"
```

---

### Task 3: Demo 2, scrollytelling the pitch

**Files:** Modify: `site/showcase.qmd` (append)

**Interfaces:** Produces reactive name `stage` (from the `.scrolly`).

- [ ] **Step 1: Author the scrolly demo**, modeled exactly on `corpus/explorable/scrolly.qmd`. Stage is a `{js}` Plot (or light canvas) cell reading `qmd.value("stage")`; four `.step`s carry `state="one-source" | "blocks" | "diff" | "many-outputs"` and narrate the value prop. Wrap in the `Result`/`Source` panel-tabset.

````markdown
## One source, watched as you scroll

::: {.panel-tabset}

## Result

::: {.scrolly name="stage"}
```{js}
//| input: stage
//| echo: false
const stage = qmd.value("stage") || "one-source";
// render a simple, legible diagram per stage with Plot/d3 or DOM;
// e.g. blocks stacking, one block highlighting (the diff), three output icons.
// ... full cell body ...
```
::: {.step state="one-source"}
**One `.qmd` file.** Plain text: prose, math, and code cells.
:::
::: {.step state="blocks"}
**Parsed into top-level blocks**, each with a content-hash id and a source position.
:::
::: {.step state="diff"}
**Save, and only the changed block re-renders** in place. Scroll, canvas, and kernel survive.
:::
::: {.step state="many-outputs"}
**The same blocks become a post, a deck, or a book.** One source, many outputs.
:::
:::

## Source

```markdown
::: {.scrolly name="stage"}
... identical skeleton + cell ...
:::
```

:::
````

- [ ] **Step 2: Browser-verify** on `/showcase.html`: scrolling the narration advances the stage; sticky behavior works at 1440 and 390 wide; reduced-motion still shows each state legibly; console clean; Source matches Result.

- [ ] **Step 3: Commit**

```sh
git add site/showcase.qmd
git commit -m "feat(site): scrollytelling pitch demo"
```

---

### Task 4: Demo 3, reactive explorer (Plot + KaTeX)

**Files:** Modify: `site/showcase.qmd` (append)

**Interfaces:** Produces reactive name `harmonics` (a `{{< input >}}` slider).

- [ ] **Step 1: Author the explorer**, modeled on the Fourier reactive in `docs/guide/using/code.qmd` and `corpus/reactive/inputs.qmd`. A `harmonics` slider drives a live `Plot.lineY` square-wave synthesis; a KaTeX display equation `$$ \sum ... $$` states the partial sum. Wrap in the `Result`/`Source` tabset.

````markdown
## A live equation and its plot

The square wave is the odd-harmonic sum
$$ f(t) = \sum_{k=1}^{N} \frac{\sin\big((2k-1)\,2\pi t\big)}{2k-1}. $$
Drag $N$ and watch the sum sharpen toward the square.

::: {.panel-tabset}

## Result

{{< input name="harmonics" type="slider" min="1" max="24" step="1" value="5" label="N (harmonics)" >}}

```{js}
//| input: harmonics
//| echo: false
const N = qmd.value("harmonics");
const ys = Array.from({length: 500}, (_, t) =>
  d3.range(1, N + 1).reduce((s, k) => s + Math.sin(2*Math.PI*(2*k-1)*t/500)/(2*k-1), 0));
return Plot.lineY(ys, {curve: "basis", stroke: "var(--qmd-accent)"})
  .plot({height: 220, marginLeft: 36, y: {label: "amplitude"}});
```

## Source

```js
//| input: harmonics
//| echo: false
... identical ...
```

:::
````

- [ ] **Step 2: Browser-verify**: KaTeX equation renders (server-side, present even under `--no-exec`); slider sharpens the wave live; stroke follows theme accent in light/dark; mobile legible; console clean; Source matches Result.

- [ ] **Step 3: Commit**

```sh
git add site/showcase.qmd
git commit -m "feat(site): reactive Plot + KaTeX explorer demo"
```

---

### Task 5: Demo 4, code that moves (code-walkthrough)

**Files:** Modify: `site/showcase.qmd` (append)

- [ ] **Step 1: Author the walkthrough**, modeled exactly on `corpus/narrate/walkthrough.qmd`. A lead code block (a short, real qmd-fast `{js}` reactive cell or a Python cell) plus `.step` divs with `lines="..."` that move the highlight as the reader scrolls. Wrap in the `Result`/`Source` tabset (the Source tab shows the `.code-walkthrough` markdown).

- [ ] **Step 2: Browser-verify**: the highlighted lines track scroll; sticky code panel works at 1440 and 390 wide; reduced-motion shows the code statically without trapping scroll; console clean; Source matches Result.

- [ ] **Step 3: Commit**

```sh
git add site/showcase.qmd
git commit -m "feat(site): code-walkthrough demo"
```

---

### Task 6: Demo 5, magic-move + code-stepping in the embedded deck

**Files:** Modify: `site/demo.qmd`

**Rationale:** `.magic-move` and code line-stepping animate ONLY in the deck engine (`deck.js`), not on a standalone page, so they belong in the embedded deck where they genuinely work.

- [ ] **Step 1: Add a `.magic-move` slide** to `site/demo.qmd` (two consecutive code blocks inside `::: {.magic-move}` so the deck morphs between them on step) and a **code-stepping** slide (a code cell with `#| code-line-numbers: "1|3-5|all"` or the fenced `{.code-line-numbers}` deck form) so arrowing through reveals line ranges. Model the exact syntax on the existing decks in the corpus / `docs/guide/demo.qmd`.

- [ ] **Step 2: Browser-verify** the deck embedded on `/` (or `/showcase.html` if also embedded there): arrow through the deck, confirm the magic-move slide morphs and the code-step slide reveals line ranges; console clean.

- [ ] **Step 3: Commit**

```sh
git add site/demo.qmd
git commit -m "feat(site): show magic-move + code-stepping in the embedded deck"
```

---

### Task 7: Final integration, full verification, copy pass

**Files:** Modify: `site/index.qmd` (decide on the two existing screencasts), `site/showcase.css` (only if the browser pass shows a real layout need)

- [ ] **Step 1: Flow + copy pass.** Read the full showcase top to bottom: tighten transitions between demos, ensure each has a one-line "this is real, here is the source" framing, and confirm no em dashes. On the index, decide whether the two `{{< video >}}` screencasts stay (as a quieter "in an editor" beat) or are removed now that the live 3D leads; default: keep one, remove the redundant one.

- [ ] **Step 2: Full `check` + build.**

```sh
./target/release/qmd-fast check site
QMD_FAST_NO_EXEC=1 ./target/release/qmd-fast build site --out /tmp/showcase-out
```
Expected: "no problems found"; build succeeds with the showcase + index; the bundled enhancers (`qmd-js.js`, `scrolly.js`, `walkthrough.js`, `tabset.js`) ship on `showcase.html`.

- [ ] **Step 3: Full browser sweep** (chrome-devtools), `/` and `/showcase.html`, at 1440x900, 900x1440, 390x844, in light AND dark, AND with reduced-motion emulated:
  - every demo renders and is interactive; no console errors on any page/size.
  - scrolling the whole showcase stays smooth; confirm (via the Network/Performance or a temporary log) that a 3D scene does NOT initialize until scrolled near.
  - each Source tab is byte-identical to its Result cell body.

- [ ] **Step 4: Performance + leak check.** Navigate away from the showcase and confirm (heap snapshot or console instrumentation) that `invalidation` ran (RAF cancelled, renderer disposed). Add `showcase.css` only if a real spacing/layout issue appeared; otherwise skip it.

- [ ] **Step 5: Final commit.**

```sh
git add site/
git commit -m "feat(site): finalize live showcase (flow, verification, index)"
```

---

## Self-Review

- **Spec coverage:** Demo 1 (reactive 3D hero + index) = Task 2; Demo 2 (scrolly) = Task 3; Demo 3 (explorer) = Task 4; Demo 4 (code-walkthrough) = Task 5; Demo 5 (deck magic-move/stepping) = Task 6; self-referential tabset = every task; nav + page = Task 1; constraints (reduced-motion, mobile, DPR, dispose, lazy-init, theme, esm.sh) = Global Constraints + verified in Tasks 2-3 and the Task 7 sweep; verification plan = Tasks' browser steps + Task 7. All spec sections map to a task.
- **Placeholder scan:** the only elisions are the Source-tab copies and two demo cell bodies, each with an explicit "paste the full body verbatim during execution / model on <corpus file>" note, not silent TODOs. The 3D hero cell (the riskiest) is given in full.
- **Type consistency:** `makeScene3D(buildScene, invalidation, opts)` signature matches the copied helper; the added `autoRotate` opt is introduced in Task 2 Step 1's helper-edit note before it is used. Reactive names (`detail`/`twist`/`amplitude`/`stage`/`harmonics`) are each produced by an `{{< input >}}`/`.scrolly` in the same task that consumes them.
