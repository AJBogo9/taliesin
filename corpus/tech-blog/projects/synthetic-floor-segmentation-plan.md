# Synthetic-Only Indoor Free-Space Segmentation
### A sim-to-real computer vision project

**One-line pitch:** Train a segmentation model to distinguish *free floor* from *obstacles* in indoor scenes using **only synthetically rendered training data**, then measure how well it transfers to **real photographs**. The gap between synthetic-trained performance and real-world performance — and what closes it — is the heart of the project.

---

## 1. What this is (and isn't)

**It is** a *perception* project: the vision system a mobile robot would use, evaluated offline against a public benchmark.

**It is not** robot control, navigation, or a working robot. No VLA models, no reinforcement learning, no hardware. Accurate framing for the writeup: *"I trained an indoor free-space segmentation model using only synthetic data and measured its sim-to-real transfer."* Not *"I built a robot."*

**Framing chosen:** General indoor free-space segmentation (eye-level / robot-height cameras), which matches the real test data cleanly. The robot-vacuum's-eye-view variant was deliberately set aside to avoid stacking a viewpoint gap on top of the sim-to-real gap in a first project.

---

## 2. Why the tooling is justified

This is the case where 3D rendering genuinely earns its cost (it would *not* for plain 2D classification, where cut-and-paste compositing would be more efficient). Free-space segmentation needs **3D-derived labels** — per-pixel floor/obstacle masks that respect occlusion, plus optional depth — which 2D compositing cannot fake. The physically-based renderer (Blender Cycles) is also the evidence-backed choice over real-time game-engine renderers for sim-to-real transfer.

| Layer | Tool | Why |
|---|---|---|
| Synthetic scene + render + labels | **Blender + BlenderProc** | Free, scriptable (Python), physically-based rendering, automatic dense label export |
| Real test set | **ADE20K** (collapsed to binary) | Public, pre-labeled, comparable to others' results — no photography needed |
| Model + training + eval | **PyTorch** | Standard; entire training layer is Python end-to-end |
| (Deferred) deployment | C++ / OpenCV | **Project 2**, motivated by a real performance problem — not bolted on here |

Everything in this project lives in the **Python** layer. C++ is intentionally out of scope.

---

## 3. The real test set: ADE20K, collapsed to binary

ADE20K is a large indoor/outdoor scene-parsing dataset with 150 labeled classes and ground-truth segmentation masks. The standard robot-navigation trick is to **collapse the 150 classes into a small set** separating drivable floor from obstacles. For this project, collapse to **binary**:

- **Class 0 — FREE / FLOOR:** floor, rug/carpet, road, path, ground, runway
- **Class 1 — OBSTACLE / NOT-FLOOR:** everything else (walls, furniture, people, doors, stairs, etc.)

There is existing open code demonstrating the 150→27 class remapping (the `hellochick/Indoor-segmentation` repo) that you can adapt to your binary mapping.

**Why this is better than photographing your own floor:**
- Real test images come **pre-annotated** — no manual labeling, no photo sessions.
- It's an **established benchmark**, so your sim-to-real numbers are credible and comparable.
- The core experiment is unchanged: train on synthetic, test on real, measure the gap.

**Upgrade paths (later, if the project grows):**
- **NYU Depth V2** — indoor RGB-**D**; the reference if you want to validate synthetic *depth* labels.
- **SUN RGB-D / ScanNet** — larger / richer indoor scenes.
- **Roboflow Universe** — community floor-segmentation sets, variable quality, quick to browse.

---

## 4. Phased plan

Each phase is a natural stopping point and a shippable checkpoint. If an early phase is painful, you've found out cheaply.

### Phase 0 — Tooling warm-up *(low risk, ~a weekend)*
**Goal:** prove the rendering + label pipeline works before any model training.

1. Install Blender + BlenderProc (see §5).
2. Build **one** simple room (floor plane, a few walls, 3–5 furniture objects — primitives are fine).
3. Render a few hundred views with a camera at roughly human/robot height, exporting **all** label modalities: RGB, semantic segmentation, depth, surface normals.
4. Write a binary-collapse function that maps your synthetic semantic classes → {free, obstacle}.
5. **Ship a short blog post** that simply *shows* the pipeline and the free ground truth (RGB next to mask next to depth). This is a complete, satisfying first post on its own.

**Done when:** you have a folder of `(image, binary_mask)` pairs from synthetic renders and can show them side by side.

### Phase 1 — The core experiment *(the substance)*
**Goal:** an honest sim-to-real number.

1. **Randomize** the synthetic scenes: furniture layout, flooring textures, wall materials, lighting (position/intensity/color), camera pose, and clutter/distractor objects. Generate a real training set (start ~2,000–5,000 images).
2. Prepare the **real test set**: download ADE20K, apply the binary collapse, hold it out — **train on synthetic only, test on real only.**
3. Train a **small segmentation model** (good starting points: U-Net, or SegFormer-B0 / DeepLabV3 with a light backbone). Binary segmentation, so the head is simple.
4. Evaluate on ADE20K-binary. Report **mIoU / IoU** for the free-floor class. This number — synthetic-trained, real-tested — is your sim-to-real baseline.

**Done when:** you have a single defensible IoU on real images from a model that never saw a real image in training.

### Phase 2 — The ablation that makes it a real post *(the contribution)*
**Goal:** study *what closes the gap* — this is the blog post.

Run controlled comparisons, changing one thing at a time:
- **Randomization level:** low vs. high (does aggressive randomization help transfer?).
- **Lighting isolation:** specifically test whether lighting randomization dominates transfer (a robust claim in the literature worth confirming or refuting on your data).
- **Synthetic + a little real:** add ~50–200 real ADE20K images to the synthetic training mix — how much does the gap close per real image added?

Plot **real-world IoU vs. randomization level** (and vs. number of real images mixed in). That curve, on your setup, is the contribution.

**Blog post:** embed a few synthetic renders as **interactive Three.js** (rotate the scene, toggle the label layers) so the post demonstrates both your data-pipeline *and* 3D-viz skills at once. Title direction: *"How far can synthetic data alone get you? A sim-to-real study on indoor free-space segmentation."*

---

## 5. Getting started this week (Phase 0 setup)

> Recommended: run Blender + BlenderProc inside an **isolated VM or container** (you discussed this) so any rendering/scripting stays sandboxed. BlenderProc runs **headless**, which makes a container clean and reproducible — a good fit for your Docker/home-server habits.

```bash
# BlenderProc manages its own Blender download — you don't install Blender separately
pip install blenderproc

# Sanity check: render the built-in example scene
blenderproc quickstart

# Run a script (BlenderProc launches Blender's Python under the hood)
blenderproc run my_render_script.py
```

**Phase 0 render script — what it should do:**
1. Load or build a simple room (a floor plane + walls + a few furniture meshes).
2. Assign a **category id** to each object (floor = 0, everything else = 1) so semantic masks export correctly.
3. Place a camera at ~1.0–1.5 m, sample several poses around the room.
4. Enable output for: `colors` (RGB), `segmentation` (semantic), `depth`, `normals`.
5. Write to an output dir; verify the RGB ↔ mask alignment by eye.

**Free assets to populate rooms** (so you're not modeling furniture by hand): BlenderProc has loaders/examples for indoor scene datasets and CC0 asset sources — start with a handful of free furniture models and a library of floor/wall textures for randomization.

---

## 6. What you'll learn

- The **full data-centric loop**: scene generation → label export → training → real-world eval.
- **Semantic segmentation** (a real step up from classification).
- **Domain randomization & sim-to-real** as the central object of study — the genuinely unsolved, interesting part.
- A defensible **"I worked on robotics-adjacent perception ML"** portfolio piece, without overclaiming.
- Direct relevance to your **construction / real-estate vision** thread — floor/site perception is the same shape of problem.

---

## 7. Scope discipline (the things deliberately cut)

Keeping these out is what makes the project finishable:
- **No robot control / VLA / RL / hardware** — that's a separate, much larger track (LeRobot / SmolVLA if the itch persists).
- **No C++** — deferred to a motivated Project 2 (deploying the trained model efficiently).
- **No photographing your own floor** — replaced by ADE20K.
- **No vacuum's-eye-view viewpoint gap** — set aside to keep the first sim-to-real study clean.

---

## 8. Open questions / decisions still to make

- **Model choice:** U-Net (simplest, most tutorials) vs. SegFormer-B0 (modern, strong, slightly more setup). Either works for binary segmentation.
- **Synthetic asset source:** which free furniture/texture libraries to standardize on for randomization.
- **Compute:** confirm your GPU situation for Phase 1 training (segmentation on a small backbone is modest, but worth checking).

---

### Suggested sequence
1. **This week:** Phase 0 setup → one room → render a few hundred labeled images → ship the "look what synthetic data gives you for free" post.
2. **Next:** Phase 1 → randomize, train, get the first real-world IoU.
3. **Then:** Phase 2 → ablation, plots, the main blog post with interactive Three.js renders.
