# 3D & interactive visualizations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pinned corpus exhibit `corpus/graphics3d/` with four interactive demos (spin/fullscreen molecule picker, animated Lorenz attractor, CAD viewer + live parametric gear, sorting-algorithm visualizer) and surface them on the marketing site (`site/`) and docs (`docs/guide/`).

**Architecture:** Every demo is a client-side `{js}` cell. The 3D demos reuse an extended `makeScene3D` include (three.js from esm.sh, already gives spin/zoom/fullscreen); sorting is standalone Canvas 2D. The exhibit is a normal Taliesin website project auto-covered by `corpus.rs`; the marketing site mounts it at `/gallery/graphics3d` and lifts three headline demos onto `showcase.tmd`.

**Tech Stack:** Taliesin `.tmd`, `{js}` cells, three.js 0.163.0 + GLTFLoader (esm.sh), Canvas 2D, Web Audio. No Rust/crate changes. No Python/R kernel (everything is `{js}`, so it builds kernel-free).

## Global Constraints

- **No Rust/engine changes.** Content + include-JS only. If a demo seems to need an engine change, rethink the demo.
- **three.js is pinned at `0.163.0`**, imported from `https://esm.sh/three@0.163.0` (and `.../three@0.163.0/examples/jsm/...` for addons) — same version everywhere, matching the existing helper.
- **Kernel-free:** all cells are `{js}`. No `{python}`/`{r}` cell in the exhibit or the shipped site pages.
- **Valid cell options only:** `//| echo: false`, `//| name: <id>`, `//| input: <id>`. (Enforced by `cell_option_validation.rs`.)
- **Clean front-matter:** exhibit `_site.yml` + page front-matter use only known keys (enforced by `corpus.rs::every_corpus_doc_emits_no_unknown_key_warnings`). Copy an existing exhibit's shape.
- **Block-model invariants:** never emit raw HTML that would break `data-block-id`/`data-sourcepos`; author in Markdown + fenced divs + `{js}`, like the rest of the corpus.
- **Model licensing:** the vendored `.glb` must have an unambiguous license (CC0 preferred, or a confirmed-permissive license); record attribution in `cad.tmd` **and** `THIRD_PARTY.md`. Do **not** ship `2CylinderEngine`/`GearboxAssy` unless their license is confirmed permissive.
- **UI verification matrix:** browser-verify each page via the chrome-devtools MCP at three viewports — mobile ~390×844, laptop landscape ~1440×900, laptop portrait ~900×1440.
- **Commit discipline:** work stays on branch `feat/3d-graphics-exhibit`. Commit per task. Tree stays `cargo fmt`-clean (PostToolUse hook handles `.rs`; no `.rs` here).

**Preview/verify commands:**
```sh
cargo build -p taliesin-server                                  # rebuild binary if assets change (not needed here; content only)
cargo run -p taliesin-server -- preview corpus/graphics3d 4388  # live-preview the exhibit
cargo run -p taliesin-server -- preview site 4388               # live-preview the marketing site (mounts resolve)
cargo test -p taliesin-core --test corpus                       # exhibit invariants (front-matter, unknown keys, block model)
cargo test                                                      # full suite before final commit
```

---

## Task 1: Extend the `makeScene3D` helper (controls slot, `loadGLTF`, `rebuild`, `camera`)

**Files:**
- Create: `corpus/graphics3d/_includes/three-scene.tmd`
- Create: `site/_includes/three-scene.tmd` will be **overwritten** with the same extended content in Task 7 (keep them byte-identical). For this task, only create the exhibit copy.

**Interfaces:**
- Produces: `qmd.get("makeScene3D")` → `async makeScene3D(buildScene, invalidation, opts)`.
  - `buildScene(scene, THREE, ctx)` where `ctx = { O, spriteLabel, ctrl, camera, renderer, scene, rebuild, frameObject, loadGLTF }`. `buildScene` **may be async**.
  - `opts` adds `controls` (array) to the existing `width,height,cameraPos,fov,far,bgColor,alpha,target,minDistance,maxDistance,ambientIntensity,onFrame`.
  - `ctx.loadGLTF(url)` → `Promise<THREE.Object3D>` (adds to scene + auto-frames camera).
  - `ctx.rebuild(group, populate)` → clears+disposes `group`'s children, then calls `populate(group)`.
  - `ctx.frameObject(obj, fit=1.4)` → fits camera to `obj`'s bounding box.
  - `controls` entries: `{type:'select', label, options:[{value,label}], value, onChange(v)}` or `{type:'range', label, min, max, step, value, onInput(numberValue)}`.

- [ ] **Step 1: Write the extended helper.** Create `corpus/graphics3d/_includes/three-scene.tmd` with exactly this content (it is the existing helper plus the four additive features; existing callers that destructure only `{O, spriteLabel}` are unaffected):

````markdown
```{js}
//| name: makeScene3D
//| echo: false

return async function makeScene3D(buildScene, invalidation, opts = {}) {
  const THREE = await import("https://esm.sh/three@0.163.0");
  const { OrbitControls } = await import(
    "https://esm.sh/three@0.163.0/examples/jsm/controls/OrbitControls"
  );

  const {
    width            = 620,
    height           = 500,
    cameraPos        = [6, 4, 7],
    fov              = 40,
    far              = 100,
    bgColor          = 0x111827,
    alpha            = false,
    target           = [0, 0, 0],
    minDistance      = 3,
    maxDistance      = 20,
    ambientIntensity = 1.0,
    onFrame          = null,
    controls         = null,   // NEW: [{type:'select'|'range', ...}]
  } = opts;

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha });
  renderer.setSize(width, height);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setClearColor(bgColor, alpha ? 0 : 1);

  const scene  = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(fov, width / height, 0.1, far);
  camera.position.set(...cameraPos);
  scene.add(new THREE.AmbientLight(0xffffff, ambientIntensity));

  const O = new THREE.Vector3(0, 0, 0);

  function spriteLabel(text, cssColor, scale = 1.6) {
    const c = document.createElement("canvas");
    c.width = 320; c.height = 72;
    const ctx = c.getContext("2d");
    ctx.font = "bold 26px sans-serif";
    ctx.fillStyle = cssColor;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(text, 160, 36);
    const spr = new THREE.Sprite(new THREE.SpriteMaterial({
      map: new THREE.CanvasTexture(c), transparent: true, depthTest: false
    }));
    spr.scale.set(scale, 0.36, 1);
    return spr;
  }

  const ctrl = new OrbitControls(camera, renderer.domElement);
  ctrl.target.set(...target);
  ctrl.enableDamping = true;
  ctrl.dampingFactor = 0.06;
  ctrl.minDistance   = minDistance;
  ctrl.maxDistance   = maxDistance;

  // NEW: dispose + repopulate a group without tearing down the scene
  function rebuild(group, populate) {
    for (let i = group.children.length - 1; i >= 0; i--) {
      const c = group.children[i];
      c.geometry?.dispose?.();
      if (Array.isArray(c.material)) c.material.forEach((m) => m.dispose());
      else c.material?.dispose?.();
      group.remove(c);
    }
    populate(group);
  }

  // NEW: fit the camera to an object's bounding box
  function frameObject(obj, fit = 1.4) {
    const box    = new THREE.Box3().setFromObject(obj);
    const size   = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z) || 1;
    const dist   = (maxDim / 2) / Math.tan((fov * Math.PI) / 360) * fit;
    ctrl.target.copy(center);
    const dir = new THREE.Vector3(1, 0.6, 1).normalize();
    camera.position.copy(center).addScaledVector(dir, dist);
    ctrl.minDistance = dist * 0.25;
    ctrl.maxDistance = dist * 5;
    camera.near = Math.max(0.01, dist / 100);
    camera.far  = dist * 100;
    camera.updateProjectionMatrix();
    ctrl.update();
  }

  // NEW: load a glTF/glb, add to scene, auto-frame
  async function loadGLTF(url) {
    const { GLTFLoader } = await import(
      "https://esm.sh/three@0.163.0/examples/jsm/loaders/GLTFLoader"
    );
    const gltf = await new Promise((res, rej) =>
      new GLTFLoader().load(url, res, undefined, rej)
    );
    scene.add(gltf.scene);
    frameObject(gltf.scene);
    return gltf.scene;
  }

  const ctx = { O, spriteLabel, ctrl, camera, renderer, scene, rebuild, frameObject, loadGLTF };
  await buildScene(scene, THREE, ctx);   // NEW: await — buildScene may be async

  ctrl.update();

  let rafId;
  (function animate() {
    rafId = requestAnimationFrame(animate);
    if (onFrame) onFrame();
    ctrl.update();
    renderer.render(scene, camera);
  })();

  const container = document.createElement("div");
  container.style.cssText =
    `position:relative;display:inline-block;width:${width}px;max-width:100%;`;

  // NEW: optional controls toolbar (rendered above the canvas)
  if (controls && controls.length) {
    const bar = document.createElement("div");
    bar.style.cssText =
      "display:flex;flex-wrap:wrap;gap:14px;align-items:center;margin-bottom:8px;font-size:13px;";
    for (const c of controls) {
      const wrap = document.createElement("label");
      wrap.style.cssText = "display:inline-flex;gap:6px;align-items:center;";
      wrap.append(c.label + ":");
      if (c.type === "select") {
        const sel = document.createElement("select");
        for (const o of c.options) {
          const opt = document.createElement("option");
          opt.value = o.value; opt.textContent = o.label;
          if (o.value === c.value) opt.selected = true;
          sel.appendChild(opt);
        }
        sel.addEventListener("change", () => c.onChange(sel.value));
        wrap.appendChild(sel);
      } else if (c.type === "range") {
        const inp = document.createElement("input");
        inp.type = "range";
        inp.min = c.min; inp.max = c.max; inp.step = c.step ?? 1; inp.value = c.value;
        const out = document.createElement("output");
        out.textContent = c.value;
        inp.addEventListener("input", () => {
          out.textContent = inp.value;
          c.onInput(parseFloat(inp.value));
        });
        wrap.append(inp, out);
      }
      bar.appendChild(wrap);
    }
    container.appendChild(bar);
  }

  container.appendChild(renderer.domElement);

  const btnStyle = [
    "padding:4px 10px", "font-size:12px", "cursor:pointer",
    "background:rgba(30,30,30,0.75)", "color:#ddd",
    "border:1px solid #555", "border-radius:4px",
    "backdrop-filter:blur(4px)", "z-index:10",
  ].join(";");

  const fsBtn = document.createElement("button");
  fsBtn.textContent = "⛶ Fullscreen";
  fsBtn.style.cssText = "position:absolute;bottom:8px;right:8px;" + btnStyle;
  fsBtn.addEventListener("click", () => {
    if (!document.fullscreenElement) container.requestFullscreen();
    else document.exitFullscreen();
  });
  container.appendChild(fsBtn);

  function onFSChange() {
    if (document.fullscreenElement === container) {
      renderer.setSize(screen.width, screen.height);
      camera.aspect = screen.width / screen.height;
      camera.updateProjectionMatrix();
      fsBtn.textContent = "✕ Exit fullscreen";
    } else {
      renderer.setSize(width, height);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      fsBtn.textContent = "⛶ Fullscreen";
    }
  }
  document.addEventListener("fullscreenchange", onFSChange);

  invalidation.then(() => {
    cancelAnimationFrame(rafId);
    ctrl.dispose();
    renderer.dispose();
    document.removeEventListener("fullscreenchange", onFSChange);
  });

  return container;
};
```
````

- [ ] **Step 2: Verify it renders as a corpus doc.** Run: `cargo test -p taliesin-core --test corpus`. Expected: PASS (the include is a `{js}`-only doc; it renders and satisfies invariants like the existing `three-scene.tmd` copies). If it fails on unknown keys, check the `//|` options are only `name`/`echo`.

- [ ] **Step 3: Commit.**
```bash
git add corpus/graphics3d/_includes/three-scene.tmd
git commit -m "feat(graphics3d): extended makeScene3D helper (controls, loadGLTF, rebuild)"
```

---

## Task 2: Scaffold the exhibit (config + landing page)

**Files:**
- Create: `corpus/graphics3d/_site.yml`
- Create: `corpus/graphics3d/index.tmd`

**Interfaces:**
- Produces: a renderable Taliesin website project rooted at `corpus/graphics3d/` with a landing page linking the four demo pages (`molecules.html`, `lorenz.html`, `cad.html`, `sorting.html`).

- [ ] **Step 1: Write `_site.yml`** (flat native schema; only known keys — mirror `corpus/descent/_site.yml`, which omits `url:` so the standalone build stays offline-clean):
```yaml
# A small multi-page exhibit: interactive graphics that run client-side in the
# built page. `url:` is deliberately omitted so the standalone build stays
# offline-clean (no canonical/OG self-URL on a self-contained exhibit).
title: "Live 3D graphics"
description: "Interactive graphics authored in .tmd and run client-side: spin-and-zoom molecules, a Lorenz attractor, a CAD viewer with a live parametric gear, and instrumented sorting algorithms."
toc: false
```

- [ ] **Step 2: Write `index.tmd`** (the landing page):
```markdown
---
title: "Live 3D graphics"
description: "Four interactive graphics, each a real .tmd document you can open."
toc: false
---

Every graphic here runs **client-side in the built page** and is authored in plain
`.tmd` with `{js}` cells: no plugins, no server, no build step beyond Taliesin. Drag
to spin, scroll to zoom, and use the ⛶ button for fullscreen.

## [Molecules &rarr;](molecules.html)

A ball-and-stick viewer with a picker: water, benzene, caffeine, a DNA double helix,
and a C60 buckyball. CPK colors, real 3D geometry, spin and fullscreen.

## [The Lorenz attractor &rarr;](lorenz.html)

The butterfly, integrated live and drawn as it grows. Drag the σ / ρ / β sliders and
watch a deterministic system fold into chaos: a shape that only reads in three
dimensions.

## [CAD, loaded and computed &rarr;](cad.html)

A real engineering model rendered in the page, beside a spur gear the document
**computes** from three numbers. Change the tooth count and watch it regenerate.

## [Sorting algorithms &rarr;](sorting.html)

Seven real, instrumented sorts. The animation replays each algorithm's *actual*
comparisons and swaps, recorded as it runs. With optional sound.
```

- [ ] **Step 3: Preview + verify.** Run `cargo run -p taliesin-server -- preview corpus/graphics3d 4388`, then via chrome-devtools MCP open `http://localhost:4388/index.html` (or `/`), screenshot, confirm the landing renders with four section links and no console errors. Run `cargo test -p taliesin-core --test corpus` → PASS.

- [ ] **Step 4: Commit.**
```bash
git add corpus/graphics3d/_site.yml corpus/graphics3d/index.tmd
git commit -m "feat(graphics3d): exhibit scaffold + landing page"
```

---

## Task 3: Molecules page (`molecules.tmd`)

**Files:**
- Create: `corpus/graphics3d/molecules.tmd`

**Interfaces:**
- Consumes: `qmd.get("makeScene3D")` from the include (Task 1), using `ctx.rebuild`, `ctx.frameObject`, and `opts.controls`.
- Produces: a self-contained page; no exported symbols.

Data model per molecule: `{ atoms: [{el, pos:[x,y,z]}], bonds: [[i,j], ...] }`. `el` is an element symbol keyed into `CPK` (color) and `RADII` (Å-ish display radius).

- [ ] **Step 1: Write the page.** Molecules water/ammonia/benzene are built by generator functions; **C60** and **DNA** are procedural; **caffeine** is parsed from an embedded XYZ constant (see Step 2 for how to obtain that constant). Content:

````markdown
---
title: "Molecules"
description: "A ball-and-stick molecule viewer with a picker, spin, and fullscreen."
toc: false
---

Ball-and-stick, CPK-colored, real 3D coordinates. Pick a molecule, drag to spin,
scroll to zoom, ⛶ for fullscreen. Every atom and bond is a three.js mesh built by
the `{js}` cell below.

{{< include _includes/three-scene.tmd >}}

```{js}
//| name: molecules
//| echo: false

const makeScene3D = qmd.get("makeScene3D");

// --- element display data -------------------------------------------------
const CPK = { H:0xffffff, C:0x222222, N:0x3050f8, O:0xff0d0d, P:0xff8000 };
const RADII = { H:0.25, C:0.40, N:0.40, O:0.40, P:0.50 };
const BOND_R = 0.09;

// --- molecule builders ----------------------------------------------------
function water() {
  const d = 0.96, a = 104.5 * Math.PI / 180;
  return {
    atoms: [
      { el:"O", pos:[0,0,0] },
      { el:"H", pos:[d*Math.sin(a/2),  d*Math.cos(a/2), 0] },
      { el:"H", pos:[-d*Math.sin(a/2), d*Math.cos(a/2), 0] },
    ],
    bonds: [[0,1],[0,2]],
  };
}

function ammonia() {
  const d = 1.01, ang = 107 * Math.PI / 180;
  const atoms = [{ el:"N", pos:[0,0,0] }];
  const bonds = [];
  for (let i = 0; i < 3; i++) {
    const t = i * 2 * Math.PI / 3;
    const el = Math.sin(ang);
    atoms.push({ el:"H", pos:[d*el*Math.cos(t), -d*Math.cos(ang), d*el*Math.sin(t)] });
    bonds.push([0, i+1]);
  }
  return { atoms, bonds };
}

function benzene() {
  const rC = 1.39, rH = rC + 1.09;
  const atoms = [], bonds = [];
  for (let i = 0; i < 6; i++) {
    const t = i * Math.PI / 3;
    atoms.push({ el:"C", pos:[rC*Math.cos(t), rC*Math.sin(t), 0] });
  }
  for (let i = 0; i < 6; i++) {
    const t = i * Math.PI / 3;
    atoms.push({ el:"H", pos:[rH*Math.cos(t), rH*Math.sin(t), 0] });
    bonds.push([i, (i+1)%6]);   // ring
    bonds.push([i, i+6]);        // C-H
  }
  return { atoms, bonds };
}

// C60: truncated icosahedron. Vertices are all even permutations of
// (0,±1,±3φ), (±1,±(2+φ),±2φ), (±φ,±2,±(2φ+1)); bonds = the 90 shortest edges.
function buckyball() {
  const P = (1 + Math.sqrt(5)) / 2;
  const base = [
    [0,1,3*P],[1,2+P,2*P],[P,2,2*P+1],
  ];
  const set = new Set(), verts = [];
  const evenPerms = (v) => [v, [v[1],v[2],v[0]], [v[2],v[0],v[1]]];
  const signs = (v) => {
    const out = [];
    for (const sx of [1,-1]) for (const sy of [1,-1]) for (const sz of [1,-1])
      out.push([sx*v[0], sy*v[1], sz*v[2]]);
    return out;
  };
  for (const b of base) for (const p of evenPerms(b)) for (const s of signs(p)) {
    const key = s.map((n) => n.toFixed(3)).join(",");
    if (!set.has(key)) { set.add(key); verts.push(s); }
  }
  // scale to a pleasant radius
  const R = 3.5, norm = Math.hypot(...verts[0]);
  const atoms = verts.map((p) => ({ el:"C", pos:p.map((n) => n*R/norm) }));
  // bonds: nearest-neighbor pairs (edge length is the global minimum distance)
  let min = Infinity;
  for (let i = 0; i < atoms.length; i++)
    for (let j = i+1; j < atoms.length; j++) {
      const dd = dist(atoms[i].pos, atoms[j].pos);
      if (dd < min) min = dd;
    }
  const bonds = [];
  for (let i = 0; i < atoms.length; i++)
    for (let j = i+1; j < atoms.length; j++)
      if (dist(atoms[i].pos, atoms[j].pos) < min * 1.05) bonds.push([i,j]);
  return { atoms, bonds };
}

// Idealized B-form DNA double helix (schematic): two phosphate backbones as P
// atoms on a helix, base pairs as N-N rungs. Not a real PDB structure.
function dna(turns = 2.5, perTurn = 10) {
  const n = Math.round(turns * perTurn);
  const R = 1.0, rise = 0.34 * 3, twist = 2 * Math.PI / perTurn, off = Math.PI;
  const atoms = [], bonds = [];
  const strandA = [], strandB = [];
  for (let i = 0; i < n; i++) {
    const y = i * rise - (n * rise) / 2, t = i * twist;
    strandA.push(atoms.length);
    atoms.push({ el:"P", pos:[R*Math.cos(t), y, R*Math.sin(t)] });
    strandB.push(atoms.length);
    atoms.push({ el:"P", pos:[R*Math.cos(t+off), y, R*Math.sin(t+off)] });
  }
  for (let i = 0; i < n; i++) {
    // base-pair rung (two N "bases" meeting in the middle)
    const t = i * twist, y = i * rise - (n * rise) / 2;
    const na = atoms.length;
    atoms.push({ el:"N", pos:[0.45*R*Math.cos(t), y, 0.45*R*Math.sin(t)] });
    const nb = atoms.length;
    atoms.push({ el:"N", pos:[0.45*R*Math.cos(t+off), y, 0.45*R*Math.sin(t+off)] });
    bonds.push([strandA[i], na], [na, nb], [nb, strandB[i]]);
    if (i > 0) bonds.push([strandA[i-1], strandA[i]], [strandB[i-1], strandB[i]]);
  }
  return { atoms, bonds };
}

// Parse a minimal XYZ block ("El x y z" per line) into the molecule model.
// Bonds are inferred by distance (< 1.7 Å covers typical single/double bonds).
function parseXYZ(text) {
  const atoms = text.trim().split("\n").map((line) => {
    const [el, x, y, z] = line.trim().split(/\s+/);
    return { el, pos:[+x, +y, +z] };
  });
  const bonds = [];
  for (let i = 0; i < atoms.length; i++)
    for (let j = i+1; j < atoms.length; j++)
      if (dist(atoms[i].pos, atoms[j].pos) < 1.75) bonds.push([i,j]);
  return { atoms, bonds };
}

function dist(a, b) {
  return Math.hypot(a[0]-b[0], a[1]-b[1], a[2]-b[2]);
}

// Caffeine coordinates (Ångström), sourced from PubChem CID 2519 (public domain).
// See Task 3 Step 2 for how this block was obtained.
const CAFFEINE_XYZ = qmd.get("caffeineXYZ");

const MOLECULES = {
  water:    { label:"Water (H₂O)",       build: water },
  benzene:  { label:"Benzene (C₆H₆)",    build: benzene },
  caffeine: { label:"Caffeine",          build: () => parseXYZ(CAFFEINE_XYZ) },
  dna:      { label:"DNA (schematic)",   build: () => dna() },
  c60:      { label:"Buckyball (C₆₀)",   build: buckyball },
};

// --- render ---------------------------------------------------------------
let group;   // holds the current molecule's meshes
let THREEref;

function populate(g, mol) {
  const T = THREEref;
  const sphereGeo = new T.SphereGeometry(1, 24, 16);
  for (const a of mol.atoms) {
    const r = RADII[a.el] ?? 0.4;
    const m = new T.Mesh(sphereGeo, new T.MeshStandardMaterial({
      color: CPK[a.el] ?? 0xdd77ff, roughness: 0.35, metalness: 0.1,
    }));
    m.scale.setScalar(r);
    m.position.set(...a.pos);
    g.add(m);
  }
  for (const [i, j] of mol.bonds) {
    const pi = new T.Vector3(...mol.atoms[i].pos);
    const pj = new T.Vector3(...mol.atoms[j].pos);
    const mid = pi.clone().add(pj).multiplyScalar(0.5);
    const len = pi.distanceTo(pj);
    const cyl = new T.Mesh(
      new T.CylinderGeometry(BOND_R, BOND_R, len, 12),
      new T.MeshStandardMaterial({ color: 0x999999, roughness: 0.5 }),
    );
    cyl.position.copy(mid);
    cyl.quaternion.setFromUnitVectors(
      new T.Vector3(0, 1, 0), pj.clone().sub(pi).normalize());
    g.add(cyl);
  }
}

return makeScene3D((scene, THREE, ctx) => {
  THREEref = THREE;
  // a second light so spheres read as 3D (helper only adds ambient)
  const key = new THREE.DirectionalLight(0xffffff, 0.9);
  key.position.set(5, 8, 6);
  scene.add(key);

  group = new THREE.Group();
  scene.add(group);

  function show(name) {
    ctx.rebuild(group, (g) => populate(g, MOLECULES[name].build()));
    ctx.frameObject(group, 1.6);
  }
  show("c60");
  ctx._show = show;   // used by the picker's onChange (closure below)
}, invalidation, {
  bgColor: 0x0e1420,
  controls: [{
    type: "select",
    label: "Molecule",
    value: "c60",
    options: Object.entries(MOLECULES).map(([value, m]) => ({ value, label: m.label })),
    onChange: (name) => scene3d._show(name),
  }],
});
```

> Note: the `controls.onChange` needs a handle to `show`. Wire it by capturing the
> returned promise's scene handle. Concretely, replace the trailing
> `return makeScene3D(...)` with:
>
> ```js
> const el = document.createElement("div");
> let showFn;
> makeScene3D((scene, THREE, ctx) => {
>   THREEref = THREE;
>   const key = new THREE.DirectionalLight(0xffffff, 0.9);
>   key.position.set(5, 8, 6); scene.add(key);
>   group = new THREE.Group(); scene.add(group);
>   showFn = (name) => { ctx.rebuild(group, (g)=>populate(g, MOLECULES[name].build())); ctx.frameObject(group, 1.6); };
>   showFn("c60");
> }, invalidation, {
>   bgColor: 0x0e1420,
>   controls: [{ type:"select", label:"Molecule", value:"c60",
>     options: Object.entries(MOLECULES).map(([value,m])=>({value,label:m.label})),
>     onChange: (name)=>showFn(name) }],
> }).then((node)=>el.appendChild(node));
> return el;
> ```
>
> This avoids the `scene3d._show` forward-reference. Use this form.
````

- [ ] **Step 2: Provide caffeine coordinates.** Add a hidden cell **above** the `molecules` cell that returns the caffeine XYZ as a named value. Obtain the coordinates from **PubChem CID 2519** (Caffeine, public domain): download its 3D SDF (`https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/cid/2519/record/SDF?record_type=3d`), take the 24 atom lines (element + x/y/z), and paste them as `"El x y z"` lines. Cell:

````markdown
```{js}
//| name: caffeineXYZ
//| echo: false
// 24 atoms, Ångström, from PubChem CID 2519 (public domain 3D conformer).
return `N 1.35 -1.29 0.00
C 0.32 -0.40 0.00
... (paste all 24 atom lines: C, N, O, and the methyl/ring H atoms) ...`;
```
````
Verify it parses: 24 non-empty lines, each `El x y z`. (If a line count mismatch occurs, `parseXYZ` still renders whatever atoms are present — but aim for the full 24.)

- [ ] **Step 3: Preview + browser-verify.** `preview corpus/graphics3d 4388`, open `/molecules.html`. Via chrome-devtools MCP confirm: buckyball renders on load; drag rotates; scroll zooms; the picker switches through all five molecules (water, benzene, caffeine, DNA, C60) with the camera re-framing each time; ⛶ enters fullscreen; **no console errors**. Repeat at the three viewports.

- [ ] **Step 4: Corpus test + commit.**
```bash
cargo test -p taliesin-core --test corpus     # PASS
git add corpus/graphics3d/molecules.tmd
git commit -m "feat(graphics3d): molecule picker (CPK ball-and-stick, 5 molecules)"
```

---

## Task 4: Lorenz attractor page (`lorenz.tmd`)

**Files:**
- Create: `corpus/graphics3d/lorenz.tmd`

**Interfaces:**
- Consumes: `makeScene3D` with `opts.controls` (three range sliders) + `opts.onFrame` (progressive reveal).

- [ ] **Step 1: Write the page.**

````markdown
---
title: "The Lorenz attractor"
description: "A Lorenz attractor integrated live and drawn as it grows, with σ/ρ/β sliders."
toc: false
---

The Lorenz system is three coupled ODEs. Deterministic, yet its trajectory never
repeats and never escapes: it folds onto a two-winged *strange attractor*. The shape
only makes sense in three dimensions. Drag the sliders to change σ, ρ, β and watch
the wings appear and collapse.

{{< include _includes/three-scene.tmd >}}

```{js}
//| name: lorenz
//| echo: false

const makeScene3D = qmd.get("makeScene3D");

const P = { sigma: 10, rho: 28, beta: 8/3 };
const N = 8000, dt = 0.006, SCALE = 0.14;

// integrate the Lorenz system (RK4) into a flat Float32Array of xyz points
function integrate({ sigma, rho, beta }) {
  const f = (s) => [
    sigma * (s[1] - s[0]),
    s[0] * (rho - s[2]) - s[1],
    s[0] * s[1] - beta * s[2],
  ];
  let s = [0.1, 0, 0];
  const pts = new Float32Array(N * 3);
  for (let i = 0; i < N; i++) {
    const k1 = f(s);
    const k2 = f(s.map((v, j) => v + dt/2 * k1[j]));
    const k3 = f(s.map((v, j) => v + dt/2 * k2[j]));
    const k4 = f(s.map((v, j) => v + dt   * k3[j]));
    s = s.map((v, j) => v + dt/6 * (k1[j] + 2*k2[j] + 2*k3[j] + k4[j]));
    // center the classic attractor (~z=25) and scale to view units
    pts[i*3+0] = s[0] * SCALE;
    pts[i*3+1] = (s[2] - 25) * SCALE;
    pts[i*3+2] = s[1] * SCALE;
  }
  return pts;
}

const el = document.createElement("div");
let line, geom, drawn, THREEref;

function rebuildLine(scene) {
  const pts = integrate(P);
  geom.setAttribute("position", new THREEref.BufferAttribute(pts, 3));
  geom.setDrawRange(0, 0);
  drawn = 0;
}

makeScene3D((scene, THREE, ctx) => {
  THREEref = THREE;
  geom = new THREE.BufferGeometry();
  // color along the path for depth
  const mat = new THREE.LineBasicMaterial({ color: 0x8bd3ff });
  line = new THREE.Line(geom, mat);
  scene.add(line);
  rebuildLine(scene);
}, invalidation, {
  cameraPos: [0, 0, 9], fov: 45, target: [0, 0, 0],
  minDistance: 3, maxDistance: 25, bgColor: 0x0b0f1a,
  onFrame: () => {                       // reveal ~40 new points/frame
    if (!geom) return;
    drawn = Math.min(drawn + 40, N);
    geom.setDrawRange(0, drawn);
  },
  controls: [
    { type:"range", label:"σ", min:1,  max:20, step:0.5, value:P.sigma,
      onInput:(v)=>{ P.sigma=v; rebuildLine(); } },
    { type:"range", label:"ρ", min:1,  max:50, step:1,   value:P.rho,
      onInput:(v)=>{ P.rho=v;   rebuildLine(); } },
    { type:"range", label:"β", min:0.5,max:5,  step:0.1, value:P.beta,
      onInput:(v)=>{ P.beta=v;  rebuildLine(); } },
  ],
}).then((node) => el.appendChild(node));
return el;
```

> With σ=10, ρ=28, β≈2.67 you get the classic butterfly. Push ρ down toward ~14 and
> the chaos collapses to a fixed point; that transition is the whole story, and it is
> unreadable in a 2-D projection.
````

- [ ] **Step 2: Browser-verify.** Open `/lorenz.html`. Confirm: the line draws progressively into the two-winged butterfly; dragging spins it; each slider re-integrates and the shape visibly changes (e.g. ρ→14 collapses the wings); fullscreen works; no console errors. Three viewports.

- [ ] **Step 3: Corpus test + commit.**
```bash
cargo test -p taliesin-core --test corpus
git add corpus/graphics3d/lorenz.tmd
git commit -m "feat(graphics3d): live Lorenz attractor with σ/ρ/β sliders"
```

---

## Task 5: Sorting visualizer page (`sorting.tmd`)

**Files:**
- Create: `corpus/graphics3d/sorting.tmd`

**Interfaces:**
- Standalone (does **not** use `makeScene3D`). Canvas 2D + Web Audio.
- Internal op-trace model: each instrumented sort takes `arr` (mutable copy) + a `rec` object `{ cmp(i,j), swap(i,j), set(i,v) }` and returns after recording every operation into an op array `ops` where each op is `["cmp"|"swap"|"set", a, b]` (`b` = second index for cmp/swap, or the written value for set).

- [ ] **Step 1: Write the page.** This is the largest cell; all seven algorithms are real and complete.

````markdown
---
title: "Sorting algorithms"
description: "Seven instrumented sorting algorithms, replayed from their real comparison/swap traces, with optional sound."
toc: false
---

Each algorithm below is a **real function**. As it runs, it records every comparison
and every move it makes; the animation just replays that trace. So you are watching
the actual algorithm, not a hand-drawn cartoon of it. Pick one, set the size and
speed, and turn on the sound if you like.

```{js}
//| name: sorting
//| echo: false

// ---- instrumented algorithms --------------------------------------------
// Each takes (a, rec); rec = { cmp(i,j), swap(i,j), set(i,v) }.
function bubble(a, rec) {
  for (let i = 0; i < a.length; i++)
    for (let j = 0; j < a.length - i - 1; j++)
      if (rec.cmp(j, j+1) && a[j] > a[j+1]) rec.swap(j, j+1);
}
function insertion(a, rec) {
  for (let i = 1; i < a.length; i++) {
    let j = i;
    while (j > 0 && (rec.cmp(j-1, j), a[j-1] > a[j])) { rec.swap(j-1, j); j--; }
  }
}
function selection(a, rec) {
  for (let i = 0; i < a.length; i++) {
    let m = i;
    for (let j = i+1; j < a.length; j++) if (rec.cmp(j, m) && a[j] < a[m]) m = j;
    if (m !== i) rec.swap(i, m);
  }
}
function quick(a, rec) {
  (function qs(lo, hi) {
    if (lo >= hi) return;
    const p = a[hi]; let i = lo;
    for (let j = lo; j < hi; j++) if (rec.cmp(j, hi) && a[j] < p) { rec.swap(i, j); i++; }
    rec.swap(i, hi);
    qs(lo, i-1); qs(i+1, hi);
  })(0, a.length - 1);
}
function merge(a, rec) {
  const tmp = a.slice();
  (function ms(lo, hi) {
    if (hi - lo < 2) return;
    const mid = (lo + hi) >> 1;
    ms(lo, mid); ms(mid, hi);
    for (let k = lo; k < hi; k++) tmp[k] = a[k];
    let i = lo, j = mid;
    for (let k = lo; k < hi; k++) {
      if (i < mid && (j >= hi || (rec.cmpVals(tmp[i], tmp[j]), tmp[i] <= tmp[j])))
        rec.set(k, tmp[i++]);
      else rec.set(k, tmp[j++]);
    }
  })(0, a.length);
}
function heap(a, rec) {
  const n = a.length;
  const down = (i, n) => {
    for (;;) {
      let l = 2*i+1, r = 2*i+2, big = i;
      if (l < n && (rec.cmp(l, big), a[l] > a[big])) big = l;
      if (r < n && (rec.cmp(r, big), a[r] > a[big])) big = r;
      if (big === i) break;
      rec.swap(i, big); i = big;
    }
  };
  for (let i = (n>>1)-1; i >= 0; i--) down(i, n);
  for (let i = n-1; i > 0; i--) { rec.swap(0, i); down(0, i); }
}
function radix(a, rec) {
  const max = Math.max(...a);
  for (let exp = 1; Math.floor(max/exp) > 0; exp *= 10) {
    const out = new Array(a.length), cnt = new Array(10).fill(0);
    for (let i = 0; i < a.length; i++) { rec.touch(i); cnt[Math.floor(a[i]/exp)%10]++; }
    for (let d = 1; d < 10; d++) cnt[d] += cnt[d-1];
    for (let i = a.length-1; i >= 0; i--) {
      const d = Math.floor(a[i]/exp)%10;
      out[--cnt[d]] = a[i];
    }
    for (let i = 0; i < a.length; i++) rec.set(i, out[i]);
  }
}
const ALGOS = { bubble, insertion, selection, quick, merge, heap, radix };

// ---- recorder: run an algorithm on a copy, capture the op trace ----------
function trace(algo, values) {
  const a = values.slice(), ops = [];
  const rec = {
    cmp:(i,j)=>{ ops.push(["cmp",i,j]); return true; },
    cmpVals:()=>{ ops.push(["cmp",-1,-1]); },   // merge compares values, not slots
    swap:(i,j)=>{ ops.push(["swap",i,j]); const t=a[i]; a[i]=a[j]; a[j]=t; },
    set:(i,v)=>{ ops.push(["set",i,v]); a[i]=v; },
    touch:(i)=>{ ops.push(["cmp",i,i]); },
  };
  algo(a, rec);
  return ops;
}

// ---- animation ------------------------------------------------------------
const WIDTH = 620, HEIGHT = 360;
const state = { algo:"bubble", size:60, speed:6, sound:false };
let values, ops, work, opi, comps, accesses, audio, raf;

function shuffle(n) {
  const v = Array.from({length:n}, (_,i)=>i+1);
  for (let i = n-1; i > 0; i--) { const j = (Math.random()*(i+1))|0; [v[i],v[j]]=[v[j],v[i]]; }
  return v;
}
function restart() {
  values = shuffle(state.size);
  ops = trace(ALGOS[state.algo], values);
  work = values.slice();
  opi = 0; comps = 0; accesses = 0;
  hi = { a:-1, b:-1 };
}
let hi = { a:-1, b:-1 };

function tone(freq) {
  if (!state.sound) return;
  if (!audio) audio = new (window.AudioContext || window.webkitAudioContext)();
  const o = audio.createOscillator(), g = audio.createGain();
  o.frequency.value = 120 + freq * 12;
  o.connect(g); g.connect(audio.destination);
  g.gain.setValueAtTime(0.06, audio.currentTime);
  g.gain.exponentialRampToValueAtTime(0.0001, audio.currentTime + 0.05);
  o.start(); o.stop(audio.currentTime + 0.05);
}

function stepOnce() {
  if (opi >= ops.length) { hi = { a:-1, b:-1 }; return false; }
  const [t, x, y] = ops[opi++];
  if (t === "cmp") { comps++; hi = { a:x, b:y }; if (x>=0) tone(work[x]); }
  else if (t === "swap") { const s=work[x]; work[x]=work[y]; work[y]=s; accesses+=2; hi={a:x,b:y}; tone(work[x]); }
  else if (t === "set") { work[x]=y; accesses++; hi={a:x,b:-1}; tone(y); }
  return true;
}

function draw(ctx) {
  ctx.clearRect(0, 0, WIDTH, HEIGHT);
  const n = work.length, bw = WIDTH / n, maxv = n;
  for (let i = 0; i < n; i++) {
    const h = (work[i] / maxv) * (HEIGHT - 24);
    let hue = (work[i] / maxv) * 300;      // rainbow by value
    if (i === hi.a || i === hi.b) ctx.fillStyle = "#fff";
    else ctx.fillStyle = `hsl(${hue} 80% 55%)`;
    ctx.fillRect(i*bw, HEIGHT - h, Math.max(1, bw-1), h);
  }
  ctx.fillStyle = "#9aa4b2";
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillText(`${state.algo} — comparisons: ${comps}  array writes: ${accesses}`, 8, 16);
}

// ---- DOM + loop -----------------------------------------------------------
const wrap = document.createElement("div");
wrap.style.cssText = "max-width:100%;";
const bar = document.createElement("div");
bar.style.cssText = "display:flex;flex-wrap:wrap;gap:14px;align-items:center;margin-bottom:8px;font-size:13px;";
const canvas = document.createElement("canvas");
canvas.width = WIDTH; canvas.height = HEIGHT;
canvas.style.cssText = "width:100%;max-width:"+WIDTH+"px;background:#0b0f1a;border-radius:6px;";
const cx = canvas.getContext("2d");

function control(label, node) {
  const l = document.createElement("label");
  l.style.cssText = "display:inline-flex;gap:6px;align-items:center;";
  l.append(label + ":", node);
  return l;
}
const sel = document.createElement("select");
for (const k of Object.keys(ALGOS)) { const o=document.createElement("option"); o.value=k; o.textContent=k; sel.appendChild(o); }
sel.addEventListener("change", () => { state.algo = sel.value; restart(); });

function rangeCtl(min, max, val, on) {
  const r = document.createElement("input");
  r.type="range"; r.min=min; r.max=max; r.value=val;
  r.addEventListener("input", () => on(+r.value));
  return r;
}
const sizeR  = rangeCtl(10, 200, state.size, (v)=>{ state.size=v; restart(); });
const speedR = rangeCtl(1, 40, state.speed, (v)=>{ state.speed=v; });
const restartBtn = document.createElement("button");
restartBtn.textContent = "↻ Shuffle";
restartBtn.addEventListener("click", restart);
const soundBox = document.createElement("input");
soundBox.type = "checkbox";
soundBox.addEventListener("change", () => { state.sound = soundBox.checked; });

bar.append(
  control("Algorithm", sel),
  control("Size", sizeR),
  control("Speed", speedR),
  control("Sound", soundBox),
  restartBtn,
);
wrap.append(bar, canvas);

restart();
(function loop() {
  raf = requestAnimationFrame(loop);
  for (let s = 0; s < state.speed; s++) if (!stepOnce()) break;
  draw(cx);
})();
invalidation.then(() => { cancelAnimationFrame(raf); if (audio) audio.close(); });

return wrap;
```
````

- [ ] **Step 2: Browser-verify.** Open `/sorting.html`. For **each** of the seven algorithms confirm via chrome-devtools MCP: bars shuffle, animate, and end fully sorted (monotonic rainbow); the comparison/write counters increase and differ sensibly between algorithms (e.g. selection ≈ n²/2 comparisons, radix shows pass-based sweeps); Size and Speed sliders work; Shuffle restarts; the Sound checkbox enables tones on interaction (and there is **no** autoplay before it is checked); no console errors. Three viewports.

- [ ] **Step 3: Corpus test + commit.**
```bash
cargo test -p taliesin-core --test corpus
git add corpus/graphics3d/sorting.tmd
git commit -m "feat(graphics3d): sorting visualizer (7 instrumented algorithms + sound)"
```

---

## Task 6: CAD page + vendored engineering model (`cad.tmd`)

**Files:**
- Create: `corpus/graphics3d/assets/<model>.glb` (vendored, see Step 1)
- Create: `corpus/graphics3d/cad.tmd`
- Modify: `THIRD_PARTY.md` (add the model's attribution)

**Interfaces:**
- Consumes: `makeScene3D` with `ctx.loadGLTF` (viewer) and `opts.controls` + `ctx.rebuild`/`ctx.frameObject` (parametric gear).

- [ ] **Step 1: Pick + vendor a clean-license model.** Choose one, in priority order, and save it as `corpus/graphics3d/assets/<model>.glb`:
  1. Confirm a Khronos engineering model's license from its folder README/`LICENSE.md`; if permissive (CC0 / CC-BY), use it (e.g. `2CylinderEngine.glb`), recording exact attribution.
  2. Otherwise a **CC0** engineering/mechanical model from Smithsonian Open Access or NASA 3D Resources (convert to `.glb` with e.g. Blender export if the source is OBJ/STL). Keep it under ~4 MB (decimate if larger).
  Record the exact model name, source URL, author, and license — you will paste them into `cad.tmd` and `THIRD_PARTY.md`. Set `MODEL_FILE`/`MODEL_CREDIT` in the page accordingly.

- [ ] **Step 2: Write the page.**

````markdown
---
title: "CAD: loaded and computed"
description: "A real engineering model rendered in the page, beside a spur gear the document computes from three numbers."
toc: false
---

Two ways to put a 3-D part on a page. On the left, a **real engineering model**,
loaded and rendered live. On the right, a spur gear the document **computes** from
its module, tooth count, and pressure angle: drag the sliders and the mesh
regenerates. One is data you bring; the other is geometry the page derives.

{{< include _includes/three-scene.tmd >}}

## A real model, rendered live

```{js}
//| name: cad-viewer
//| echo: false
const makeScene3D = qmd.get("makeScene3D");
const MODEL_FILE = "assets/2CylinderEngine.glb";  // set to the vendored file (Step 1)
const el = document.createElement("div");
makeScene3D(async (scene, THREE, ctx) => {
  const key = new THREE.DirectionalLight(0xffffff, 1.1);
  key.position.set(6, 10, 8); scene.add(key);
  const fill = new THREE.DirectionalLight(0xffffff, 0.4);
  fill.position.set(-6, -3, -8); scene.add(fill);
  await ctx.loadGLTF(MODEL_FILE);   // auto-frames the camera
}, invalidation, { bgColor: 0x11151f }).then((n) => el.appendChild(n));
return el;
```

*Model: <!-- MODEL_CREDIT: paste "Name — Author, License (source URL)" from Step 1 -->.*

## A gear, computed from three numbers

```{js}
//| name: cad-gear
//| echo: false
const makeScene3D = qmd.get("makeScene3D");
const P = { module: 0.5, teeth: 18, pressure: 20 };

// involute spur-gear cross-section → extruded 3-D gear
function gearShape(THREE, { module:m, teeth:z, pressure }) {
  const alpha = pressure * Math.PI / 180;
  const r  = m * z / 2;             // pitch radius
  const rb = r * Math.cos(alpha);   // base radius
  const ra = r + m;                 // addendum (outer)
  const rf = r - 1.25 * m;          // dedendum (root)
  const inv = (t) => [rb*(Math.cos(t)+t*Math.sin(t)), rb*(Math.sin(t)-t*Math.cos(t))];
  const tMax = Math.sqrt((ra/rb)**2 - 1);
  const ang = (t) => { const [x,y]=inv(t); return Math.atan2(y,x); };
  // half-tooth angular thickness at the pitch circle
  const tPitch = Math.sqrt((r/rb)**2 - 1);
  const half = Math.PI/(2*z) + (Math.tan(alpha) - alpha) - (ang(tPitch));
  const shape = new THREE.Shape();
  const STEPS = 6;
  let first = true;
  for (let k = 0; k < z; k++) {
    const base = k * 2*Math.PI/z;
    // rising involute flank
    for (let s = 0; s <= STEPS; s++) {
      const t = tMax * s/STEPS; const [x,y] = inv(t);
      const a = base - half + (ang(t) - ang(0));
      const px = Math.hypot(x,y)*Math.cos(a + Math.atan2(y,x)-Math.atan2(y,x));
      const rr = Math.hypot(x,y);
      const pt = [rr*Math.cos(base - half + Math.atan2(y,x)), rr*Math.sin(base - half + Math.atan2(y,x))];
      if (first) { shape.moveTo(pt[0], pt[1]); first = false; } else shape.lineTo(pt[0], pt[1]);
    }
    // tip arc + falling (mirrored) flank
    for (let s = STEPS; s >= 0; s--) {
      const t = tMax * s/STEPS; const [x,y] = inv(t);
      const rr = Math.hypot(x,y);
      const a = base + half - Math.atan2(y,x);
      shape.lineTo(rr*Math.cos(a), rr*Math.sin(a));
    }
    // root circle to next tooth
    const aNext = (k+1) * 2*Math.PI/z - half;
    shape.lineTo(rf*Math.cos(base + half), rf*Math.sin(base + half));
    shape.lineTo(rf*Math.cos(aNext), rf*Math.sin(aNext));
  }
  shape.closePath();
  const hole = new THREE.Path();
  hole.absarc(0, 0, Math.max(0.6, rf*0.35), 0, Math.PI*2, true);
  shape.holes.push(hole);
  return shape;
}

const el = document.createElement("div");
let gearGroup, THREEref;
function makeGear(g) {
  const T = THREEref;
  const geo = new T.ExtrudeGeometry(gearShape(T, P), { depth: P.module*3, bevelEnabled:false });
  geo.center();
  g.add(new T.Mesh(geo, new T.MeshStandardMaterial({ color:0xb8c2d0, metalness:0.7, roughness:0.35 })));
}
makeScene3D((scene, THREE, ctx) => {
  THREEref = THREE;
  const key = new THREE.DirectionalLight(0xffffff, 1.1); key.position.set(4,8,6); scene.add(key);
  gearGroup = new THREE.Group(); scene.add(gearGroup);
  const regen = () => { ctx.rebuild(gearGroup, makeGear); ctx.frameObject(gearGroup, 1.5); };
  regen();
  ctx._regen = regen; // not used; see closure form below
}, invalidation, {
  cameraPos: [0,4,10], bgColor: 0x11151f,
  controls: [
    { type:"range", label:"Teeth",    min:6,  max:40, step:1,   value:P.teeth,
      onInput:(v)=>{ P.teeth=v; regenRef(); } },
    { type:"range", label:"Module",   min:0.2,max:1,  step:0.05,value:P.module,
      onInput:(v)=>{ P.module=v; regenRef(); } },
    { type:"range", label:"Pressure", min:14.5,max:25,step:0.5, value:P.pressure,
      onInput:(v)=>{ P.pressure=v; regenRef(); } },
  ],
}).then((n) => el.appendChild(n));
let regenRef;   // wired inside buildScene:
// NOTE: replace `ctx._regen = regen;` with `regenRef = regen;` so the sliders call it.
return el;
```

The gear is not a stored asset. It is recomputed from the involute equations every
time you move a slider: change the tooth count and the page redraws a valid gear.
````

> **Wiring fix (apply in both cells):** the slider callbacks reference `regenRef`
> (gear) / the picker references `showFn` (molecules). Set that variable **inside**
> `buildScene` (`regenRef = regen;`) so the closure is populated before any slider
> fires. Remove the unused `ctx._regen`/`ctx._show` lines.

- [ ] **Step 3: Attribution.** Add the model to `THIRD_PARTY.md` under a suitable section (e.g. a new "Sample 3-D models" subsection): name, author, exact license, source URL. Fill the `MODEL_CREDIT` line in `cad.tmd` to match.

- [ ] **Step 4: Browser-verify.** Open `/cad.html`. Confirm: the engineering model loads and is centered/auto-framed; drag spins it; the gear renders as a valid toothed gear with a center bore; the Teeth/Module/Pressure sliders regenerate it live (tooth count visibly changes); fullscreen works on both; no console errors (especially no GLTFLoader 404 — check the asset path). Three viewports.

- [ ] **Step 5: Corpus test + commit.**
```bash
cargo test -p taliesin-core --test corpus
git add corpus/graphics3d/cad.tmd corpus/graphics3d/assets THIRD_PARTY.md
git commit -m "feat(graphics3d): CAD viewer (vendored model) + live parametric gear"
```

> **Gear caveat for the implementer:** the involute `gearShape` above is a working
> approximation; if the flanks look wrong in the browser, simplify to a robust
> profile (pitch-circle tooth trapezoids with rounded tips) rather than block on
> exact involute geometry — the marketing point is "computed live from parameters,"
> which either profile satisfies. Keep the extrude + center-bore + slider-regen
> structure. Verify visually before committing.

---

## Task 7: Surface on the marketing site (mount + gallery card + showcase headliners)

**Files:**
- Modify: `site/_site.yml` (add a mount)
- Modify: `site/gallery.tmd` (add a card)
- Modify: `site/showcase.tmd` (add three headline demos)
- Overwrite: `site/_includes/three-scene.tmd` (byte-identical to the exhibit's extended helper)
- Modify: `site/README.md` (add the exhibit's build step)

**Interfaces:**
- Consumes: the exhibit at `../corpus/graphics3d` (mounted); the extended `makeScene3D`.

- [ ] **Step 1: Sync the site helper.** Overwrite `site/_includes/three-scene.tmd` with the **exact** content created in Task 1 (so showcase demos get `controls`/`loadGLTF`). Verify identical:
```bash
cp corpus/graphics3d/_includes/three-scene.tmd site/_includes/three-scene.tmd
diff corpus/graphics3d/_includes/three-scene.tmd site/_includes/three-scene.tmd   # no output
```

- [ ] **Step 2: Add the mount.** In `site/_site.yml`, under `mounts:`, add (after the existing `gallery/descent` line):
```yaml
  gallery/graphics3d: ../corpus/graphics3d
```

- [ ] **Step 3: Add the gallery card.** In `site/gallery.tmd`, append a section matching the existing card style:
```markdown
## Live 3-D graphics

Interactive graphics that run **client-side in the built page**: a ball-and-stick
**molecule** viewer you can spin and fullscreen, a **Lorenz attractor** drawn as it
integrates, a **CAD** model loaded beside a spur gear the document *computes* from
three numbers, and seven **sorting algorithms** replayed from their real traces.
Every moving part is authored in `.tmd` with `{js}` cells.

[Open the graphics &rarr;](gallery/graphics3d/)
```

- [ ] **Step 4: Add three showcase headliners.** In `site/showcase.tmd`, add three `::: {.panel-tabset}` demos (molecules, sorting, Lorenz) following the file's **existing** Result/Source pattern (`### Result` with the live `{js}` cell; `### Source` with the same code in a non-executed ```` ```js ```` block). Reuse the cells from Tasks 3, 5, 4 verbatim in the Result tab. Keep CAD out of the showcase (heavier `.glb` load). Confirm `{{< include _includes/three-scene.tmd >}}` is present once near the top (it already is for the existing 3-D demo).

- [ ] **Step 5: Update the build docs.** In `site/README.md`, under "Build", add the exhibit's step alongside the docs books:
```sh
taliesin build corpus/graphics3d --out _site/gallery/graphics3d
```

- [ ] **Step 6: Verify preview + build.** `preview site 4388`; via chrome-devtools MCP: `/gallery.html` shows the new card and its link resolves to the mounted exhibit; `/showcase.html` shows the three new live demos (spin, picker, sort, sliders) with working Result/Source tabs; no console errors. Then confirm the static build wiring:
```bash
cargo run -p taliesin-server -- build site --out /tmp/gx_site
cargo run -p taliesin-server -- build corpus/graphics3d --out /tmp/gx_site/gallery/graphics3d
# open /tmp/gx_site/showcase.html and /tmp/gx_site/gallery/graphics3d/index.html in the browser; verify no 404s
```
Three viewports on `/showcase.html`.

- [ ] **Step 7: Commit.**
```bash
git add site/_site.yml site/gallery.tmd site/showcase.tmd site/_includes/three-scene.tmd site/README.md
git commit -m "feat(site): mount graphics3d gallery + molecules/sorting/Lorenz showcase demos"
```

---

## Task 8: Docs how-to (`docs/guide/using/interactive.tmd`)

**Files:**
- Modify: `docs/guide/using/interactive.tmd`

**Interfaces:**
- A short instructional section; not a copy of the showcase. Points readers at the pattern and notes the `{python}`-compute variant.

- [ ] **Step 1: Add a section.** Append after the tabset section (`## Tabbed panels`), a compact how-to:
```markdown
## Interactive 3-D with `{js}`

A `{js}` cell can mount **any** DOM node, including a WebGL canvas. The gallery's
[Live 3-D graphics](../../gallery/graphics3d/) exhibit builds molecules, a Lorenz
attractor, and a live parametric gear this way, each from a small `{js}` cell that
imports [three.js](https://esm.sh/three@0.163.0) and returns a `<canvas>`.

The shape is always the same:

```{.js}
const THREE = await import("https://esm.sh/three@0.163.0");
// ...build a scene, add a renderer.domElement to a container...
return container;   // the cell mounts whatever node you return
```

Data can come from a `{python}` cell: compute coordinates or a mesh in Python,
`define()` them, and read them in the `{js}` cell with `qmd.get(...)`, exactly like
the [PCA post](https://andreasbogossian.com) pairs a NumPy computation with a
three.js scatter. The rendering is browser-native; the data can be whatever your
kernel produces.
```
(Use a **non-executed** ```` ```{.js} ```` fence — the leading dot marks it as a
highlighted, non-running example so this doc stays kernel-free and hermetic.)

- [ ] **Step 2: Verify.** `preview docs/guide 4388`; open `/using/interactive.html`; confirm the section renders, the example code block is highlighted but not executed, and the internal links are correct. `cargo test -p taliesin-core --test corpus` (docs/guide is not in corpus, but run the guide build to be safe: `cargo run -p taliesin-server -- build docs/guide --out /tmp/gx_guide` and confirm success).

- [ ] **Step 3: Commit.**
```bash
git add docs/guide/using/interactive.tmd
git commit -m "docs(guide): how to build interactive 3-D with a {js} cell"
```

---

## Task 9: Full-suite verification + finish

**Files:** none (verification only)

- [ ] **Step 1: Full test suite.** Run: `cargo test`. Expected: PASS (core + server). If a flake appears at full parallelism, re-run the failing binary with `--test-threads=1` (known project behavior). Confirm no snapshot drift (there should be none — `body_html_snapshots` pins only `reactive/*` + `explorable/scrolly`, untouched here).

- [ ] **Step 2: fmt/clean check.** `git status` clean; no stray files. (No `.rs` changed, so `cargo fmt` is a no-op, but run `cargo fmt --check` to be sure.)

- [ ] **Step 3: Final browser sweep.** With `preview site 4388`, walk `/gallery/graphics3d/` → each of the four exhibit pages + the three showcase demos, at all three viewports, capturing a screenshot each and confirming zero console errors. Spot-check fullscreen on one 3-D page and the sound toggle on sorting.

- [ ] **Step 4: Review the whole diff.** `git diff main...feat/3d-graphics-exhibit --stat` then read the content diff. Confirm: no Rust changes, no raw HTML that breaks the block model, model license recorded in `THIRD_PARTY.md`, caffeine/model attribution present.

- [ ] **Step 5: Hand off.** Report status to the user (tests green, screenshots, license note) and use `superpowers:finishing-a-development-branch` to decide integration (merge to `main` / open PR / keep the branch). Do **not** push or merge without the user's go-ahead.

---

## Self-review (completed by plan author)

- **Spec coverage:** exhibit (Task 2) ✓; molecules picker (3) ✓; Lorenz + sliders (4) ✓; CAD viewer + parametric gear + license rule (6) ✓; sorting 7 algos + sound (5) ✓; helper extensions (1) ✓; site mount + gallery card + 3 showcase headliners (7) ✓; docs how-to (8) ✓; kernel-free/no-Rust/viewport/verification constraints ✓ (Global Constraints + per-task browser steps). Correction vs. spec: **no `body_html_snapshots` regeneration is needed** (that test pins only `reactive/*`+`explorable/scrolly`; the new `{js}` docs are covered structurally by `corpus.rs`) — Task 9 Step 1 asserts no drift instead.
- **Placeholder scan:** the two data-acquisition steps (vendored `.glb` in Task 6 Step 1; caffeine XYZ in Task 3 Step 2) are concrete artifacts with named public sources and exact formats/parsers, not TODOs. The gear profile has an explicit fallback (Task 6 caveat). No "add error handling"-style gaps.
- **Type/name consistency:** `makeScene3D(buildScene, invalidation, opts)` and `ctx = {O, spriteLabel, ctrl, camera, renderer, scene, rebuild, frameObject, loadGLTF}` used identically across Tasks 1/3/4/6; op-trace `rec` shape (`cmp/cmpVals/swap/set/touch`) consistent between the recorder and all seven algorithms in Task 5; `controls` descriptor shape identical in helper (Task 1) and all callers. The forward-reference footguns (`showFn`/`regenRef`) are called out with the exact fix in Tasks 3 and 6.
