# Gallery Self-Contained Demo Site Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `gallery/` as one flat, self-contained Taliesin project of five one-page demos, delete the publish-time composition and the `external-prefixes` key, and leave the corpus exhibits untouched as test fixtures.

**Architecture:** Five new demo pages plus a rewritten index live directly in `gallery/` under its single `_site.yml`. The 3D demos reuse the wave-6 `three-scene.tmd` helper recovered from git history (a `{js}` cell importing pinned three.js from esm.sh). Once the gallery stops linking into composed output, `tools/publish.sh` loses its composition machinery and core loses the now-userless `external-prefixes` key.

**Tech Stack:** Rust (taliesin-core/server), `.tmd` authoring, three.js 0.163 via esm.sh, Python (pandas/matplotlib via `.venv`), chrome-devtools MCP for page verification.

**Spec:** `docs/superpowers/specs/2026-08-19-gallery-self-contained-design.md`

## Global Constraints

- Never use em dashes or en dashes in any authored prose (`.tmd`, `.md`, code comments). Use commas, colons, parentheses.
- `corpus/tarn/`, `corpus/descent/`, `corpus/analyst/` and their tests (`tarn.rs`, `descent.rs`, `analyst.rs`, `book_has_no_rail_toc.rs`) are READ-ONLY. Copy from them; never edit them.
- No new core features, no new `_site.yml` keys, no rebuilt `{glsl}`, no three.js vendored into core assets, no committed model binaries (procedural geometry only).
- three.js is imported at view time from `https://esm.sh/three@0.163.0`, exactly as the recovered helper already does. Do not change the pin.
- After editing any `.rs` file, run `cargo fmt --all` as the LAST formatting action (a PostToolUse rustfmt hook fires per edit and fights `cargo fmt`).
- Kill stale `cargo test`/`cargo run` processes before starting any workspace suite (concurrent workspace suites deadlock).
- Gates and pushes need `TALIESIN_PYTHON="$PWD/.venv/bin/python"`.
- Work on branch `gallery-flat` (created in Task 1 from `main`). Commits are performed by the orchestrating session, not by subagents (subagents use read-only git).
- Browser verification per demo: serve with `cargo run -q -p taliesin-server -- preview gallery/<page>.tmd 4388` (run in background), then with chrome-devtools MCP: navigate to `http://127.0.0.1:4388/<page>.html`, check `list_console_messages` is free of errors, exercise the page's interaction, screenshot in light and dark (toggle via the preview dev menu), then stop the server. The two 3D demos need network access for the esm.sh import.
- `cargo run -q -p taliesin-server -- build gallery --check-only` must exit 0 at the end of every task from Task 2 onward.

---

### Task 1: Recover the 3D scene helper and the molecules demo

**Files:**
- Create: `gallery/_includes/three-scene.tmd` (recovered, 239 lines)
- Create: `gallery/molecules.tmd` (recovered, 227 lines, front matter adjusted)

**Interfaces:**
- Produces: a `{js}` cell named `makeScene3D` (via `//| name: makeScene3D`) that later pages obtain with `tali.get("makeScene3D")` after `{{< include _includes/three-scene.tmd >}}`. Its signature is `makeScene3D(buildScene, invalidation, opts)`; Task 3's gears page mirrors the call shape `gallery/molecules.tmd` uses.

- [ ] **Step 1: Create the branch**

```bash
git checkout -b gallery-flat
```

- [ ] **Step 2: Recover both files from the pre-cut tree**

```bash
mkdir -p gallery/_includes
git show 834bb89a^:corpus/graphics3d/_includes/three-scene.tmd > gallery/_includes/three-scene.tmd
git show 834bb89a^:corpus/graphics3d/molecules.tmd > gallery/molecules.tmd
```

- [ ] **Step 3: Adjust the molecules front matter**

Read `gallery/molecules.tmd`. Keep its `title:` and `toc: false`; ensure the front matter carries a `description:` (it has one; keep it). Confirm the include line is exactly `{{< include _includes/three-scene.tmd >}}` (the relative layout is unchanged, so it should be). Read both recovered files fully and note the exact `buildScene` callback signature molecules passes to `makeScene3D`; Task 3 reuses it.

- [ ] **Step 4: Lint the project**

Run: `cargo run -q -p taliesin-server -- build gallery --check-only`
Expected: exit 0. If a validator added since wave 6 complains (for example a missing alt text or a heading skip), fix it inside the two recovered files in the smallest way that satisfies the diagnostic.

- [ ] **Step 5: Verify in the browser**

Per the Global Constraints verification loop, on `http://127.0.0.1:4388/molecules.html`: the canvas renders a ball-and-stick molecule, dragging spins it, the picker switches molecules, no console errors, and the canvas background follows the theme in light and dark.

- [ ] **Step 6: Commit**

```bash
git add gallery/_includes/three-scene.tmd gallery/molecules.tmd
git commit -m "feat(gallery): resurrect the three-scene helper and the molecules demo"
```

---

### Task 2: The gradient-descent demo

**Files:**
- Create: `gallery/descent.tmd` (near-verbatim copy of `corpus/descent/index.tmd`, 432 lines)
- Create: `gallery/landscape.svg`, `gallery/momentum.svg` (copies)

**Interfaces:**
- Produces: the reference example of `{{< input >}}` slider declarations and reactive `{js}` cells that Task 3 mirrors. Anchors `#fig-landscape`, `#fig-momentum` (already verified disjoint from every other demo's labels).

- [ ] **Step 1: Copy the page and its two figures**

```bash
cp corpus/descent/index.tmd gallery/descent.tmd
cp corpus/descent/landscape.svg corpus/descent/momentum.svg gallery/
```

- [ ] **Step 2: Add a description to the front matter**

In `gallery/descent.tmd`, extend the front matter (it currently has only `title:`) with the description from `corpus/descent/_site.yml`:

```yaml
---
title: "Gradient descent, by hand"
description: "An interactive, scrollable explainer: watch gradient descent pick its way down a loss surface as you change the learning rate, the momentum, and where it starts."
---
```

Change nothing else in the page body.

- [ ] **Step 3: Lint**

Run: `cargo run -q -p taliesin-server -- build gallery --check-only`
Expected: exit 0.

- [ ] **Step 4: Verify in the browser**

On `http://127.0.0.1:4388/descent.html`: the three sliders drive the headline graphic, the ball is draggable, the scene select steps the five scenes, the Observable Plot chart updates with the sliders, KaTeX math renders, both SVG figures display and are numbered, no console errors, light and dark both correct.

- [ ] **Step 5: Commit**

```bash
git add gallery/descent.tmd gallery/landscape.svg gallery/momentum.svg
git commit -m "feat(gallery): the gradient-descent explainer as a one-page demo"
```

---

### Task 3: The parametric gears demo

**Files:**
- Create: `gallery/gears.tmd`

**Interfaces:**
- Consumes: `makeScene3D` from `gallery/_includes/three-scene.tmd` (Task 1) via `tali.get`; the `{{< input >}}` slider grammar as used in `gallery/descent.tmd` (Task 2).

- [ ] **Step 1: Write the page**

Create `gallery/gears.tmd`. Front matter and structure:

```markdown
---
title: "A mechanical part, parametric"
description: "Two meshed gears built from primitives inside one {js} cell: sliders drive the tooth count and the speed, and the geometry rebuilds live."
toc: false
---

Two gears, meshed and spinning, every vertex computed in the `{js}` cell below. Change
the tooth count and the geometry rebuilds; change the speed and the mesh keeps its
ratio. Drag to orbit, scroll to zoom.

{{< include _includes/three-scene.tmd >}}

{{< input name="teeth" type="slider" min="8" max="24" step="1" value="12" label="teeth (driver)" >}}
{{< input name="speed" type="slider" min="0" max="2" step="0.1" value="0.6" label="speed" >}}
```

Then one `{js}` cell that mirrors the reactive wiring of the `molecules` cell in `gallery/molecules.tmd` (same `//|` options for a named, input-reading cell; copy the option lines from the molecules cell and rename). The cell body, adapted to the exact `buildScene` signature noted in Task 1 Step 3:

```js
const makeScene3D = tali.get("makeScene3D");
const teeth = Math.round(tali.get("teeth"));
const speed = tali.get("speed");

const MODULE = 0.25; // tooth size; pitch radius = teeth * MODULE / 2

function gearGeometry(THREE, n) {
  const r = (n * MODULE) / 2;
  const outer = r + MODULE;
  const root = r - 1.25 * MODULE;
  const shape = new THREE.Shape();
  const step = (Math.PI * 2) / n;
  shape.moveTo(root, 0);
  for (let i = 0; i < n; i++) {
    const a = i * step;
    shape.lineTo(root * Math.cos(a), root * Math.sin(a));
    shape.lineTo(outer * Math.cos(a + step * 0.25), outer * Math.sin(a + step * 0.25));
    shape.lineTo(outer * Math.cos(a + step * 0.5), outer * Math.sin(a + step * 0.5));
    shape.lineTo(root * Math.cos(a + step * 0.75), root * Math.sin(a + step * 0.75));
  }
  shape.closePath();
  const hole = new THREE.Path();
  hole.absarc(0, 0, Math.max(root * 0.3, MODULE), 0, Math.PI * 2, true);
  shape.holes.push(hole);
  return new THREE.ExtrudeGeometry(shape, { depth: MODULE * 2, bevelEnabled: false });
}

// Driver gear with `teeth` teeth, driven gear with twice as many; centers sit one
// pitch-radius sum apart so the teeth mesh, and the driven gear counter-rotates at
// half the rate (the tooth ratio).
```

Inside the `buildScene` callback: create two meshes from `gearGeometry(THREE, teeth)` and `gearGeometry(THREE, teeth * 2)` with a `MeshStandardMaterial` per gear (two distinct colors that read on both themes, for example `0x4a7dbd` and `0xc0863a`), position them along the x axis at distance `(teeth * MODULE) / 2 + (teeth * 2 * MODULE) / 2`, offset the driven gear's initial rotation by half a tooth (`Math.PI / (teeth * 2)`) so the teeth interleave, and in the helper's `onFrame` hook advance `driver.rotation.z += speed * dt` and `driven.rotation.z -= (speed * dt) / 2` (match the actual `onFrame` parameter shape found in the helper). Pass `cameraPos` and `controls` options consistent with what `molecules.tmd` passes.

- [ ] **Step 2: Lint**

Run: `cargo run -q -p taliesin-server -- build gallery --check-only`
Expected: exit 0.

- [ ] **Step 3: Verify in the browser**

On `http://127.0.0.1:4388/gears.html`: two meshed gears spin without their teeth passing through each other, the teeth slider rebuilds both gears, the speed slider changes the rate (0 stops them), orbit and zoom work, no console errors, both themes correct.

- [ ] **Step 4: Commit**

```bash
git add gallery/gears.tmd
git commit -m "feat(gallery): a parametric meshed-gears demo in one {js} cell"
```

---

### Task 4: The computational report demo

**Files:**
- Create: `gallery/report.tmd`
- Create: `gallery/data/latency.csv` (copy, 3 KB)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: labels `fig-p95`, `tbl-coefs`, plus one authored table label `tbl-regions` (all disjoint from other demos).

- [ ] **Step 1: Copy the dataset**

```bash
mkdir -p gallery/data
cp corpus/analyst/data/latency.csv gallery/data/latency.csv
```

- [ ] **Step 2: Write the page**

Create `gallery/report.tmd` by distilling `corpus/analyst/index.tmd` (319 lines) and the model summary from `corpus/analyst/methods.tmd` into one page of roughly 150 lines. Front matter:

```markdown
---
title: "A computational report, in one page"
description: "Python cleans a committed dataset, draws the charts and fits the model as the page builds; every figure and table is numbered and cross-referenced."
---
```

Required structure, reusing the corpus analyst's own cells as the source of truth:

1. A short intro paragraph saying the page ran its own analysis at build time.
2. The load-and-clean `{python}` cell copied from `corpus/analyst/index.tmd`, with the read path changed to `data/latency.csv` and imports kept identical (the same `.venv` executes both).
3. One chart cell producing the weekly p95 figure, keeping `#| label: fig-p95` and its `#| fig-cap:` from the corpus page.
4. One authored markdown table (a small per-region summary, values copied from the corpus page's rendered numbers) with a caption line ending in `{#tbl-regions}`, to show the authored and executed paths share one counter.
5. The model cell producing the coefficient table, keeping `#| label: tbl-coefs`.
6. Closing prose that cross-references `@fig-p95`, `@tbl-regions` and `@tbl-coefs` in document order.

Do not copy the corpus page's `@sec-` cross-page references (this page has no second page); reword those sentences.

- [ ] **Step 3: Lint and execute**

Run: `cargo run -q -p taliesin-server -- build gallery --check-only`
Expected: exit 0.
Then verify execution in the preview (next step); the preview runs the kernel.

- [ ] **Step 4: Verify in the browser**

With `TALIESIN_PYTHON="$PWD/.venv/bin/python"` exported before starting the preview, on `http://127.0.0.1:4388/report.html`: the chart figure renders (no "kernel unavailable" diagnostic), the figure is numbered, both tables carry sequential table numbers in document order, the three cross-references link to them, no console errors.

- [ ] **Step 5: Commit**

```bash
git add gallery/report.tmd gallery/data/latency.csv
git commit -m "feat(gallery): a one-page executed report demo"
```

---

### Task 5: The API-page craft demo

**Files:**
- Create: `gallery/api-craft.tmd`

**Interfaces:**
- Consumes: nothing from other tasks. All anchors are page-local; use a `craft-` prefix on any new `{#...}` ids to stay disjoint.

- [ ] **Step 1: Locate the source material**

The distillation sources in `corpus/tarn/` (read them, copy the best material, adapt in place):

- Line-by-line code reading: `corpus/tarn/quickstart.tmd` (the numbered `**Line N.**` walkthrough of the core query).
- Install callouts (per package manager, per OS): `corpus/tarn/install.tmd`.
- Version and deprecation callouts: find them with `grep -n ':::' corpus/tarn/*.tmd` and pick one version callout and one deprecation callout.
- Definition list: an excerpt of `corpus/tarn/glossary.tmd` (4 to 6 terms).

- [ ] **Step 2: Write the page**

Create `gallery/api-craft.tmd`, roughly 120 lines:

```markdown
---
title: "The craft of an API page"
description: "What documentation looks like when the page format helps: a line-by-line code reading, versioned callouts, definitions in place, and checked cross-references."
---
```

Sections, in order: an intro sentence; the line-by-line reading (adapted from quickstart, with its highlighted code block); an install section with two or three of tarn's callouts; a short "reference" section holding one version callout and one deprecation callout; a glossary excerpt as a definition list; a closing paragraph whose `@sec-` cross-references point at this page's own sections (give two of the section headings explicit `{#sec-craft-reading}` / `{#sec-craft-glossary}` ids and reference those).

The copied prose contains cross-page links like `[`Frame`](api-frame.tmd#sec-api-frame)`. Those targets do not exist in the gallery: rewrite each one either as a plain code span (`` `Frame` ``) or as a local link to a section this page actually has. `--check-only` reports any missed one as a broken link, so let the linter find stragglers.

- [ ] **Step 3: Lint**

Run: `cargo run -q -p taliesin-server -- build gallery --check-only`
Expected: exit 0, in particular no broken-link diagnostics.

- [ ] **Step 4: Verify in the browser**

On `http://127.0.0.1:4388/api-craft.html`: highlighted code renders with the line-by-line commentary, the callouts show their kinds, the definition list renders, the cross-references resolve to numbered sections, both themes correct.

- [ ] **Step 5: Commit**

```bash
git add gallery/api-craft.tmd
git commit -m "feat(gallery): a one-page API-documentation craft demo"
```

---

### Task 6: The flip: new index, de-composed publish, retargeted corpus gate

Everything in this task lands in ONE commit: the index rewrite, the `_site.yml` change, the publish.sh de-composition, the corpus.rs test rewrite, the corpus/README rewording and the pre-push comment are mutually load-bearing (the ordering rule: a gate dies in the same commit as its subject).

**Files:**
- Modify: `gallery/index.tmd` (full replacement)
- Modify: `gallery/_site.yml` (full replacement)
- Modify: `tools/publish.sh`
- Modify: `.githooks/pre-push:98-107` (comment block only)
- Modify: `corpus/README.md` (the "What you verify by eye" section and the three exhibit rows)
- Modify: `crates/core/tests/corpus.rs` (the `the_readme_marks_the_same_visual_set_the_deploy_ships` test)

**Interfaces:**
- Consumes: the five demo pages from Tasks 1 to 5 (`descent.tmd`, `report.tmd`, `molecules.tmd`, `gears.tmd`, `api-craft.tmd`).
- Produces: a gallery project with no `external-prefixes:` key in use, which Task 7 requires before cutting the key from core.

- [ ] **Step 1: Replace `gallery/index.tmd`**

```markdown
---
title: "Gallery"
description: "Short one-page demos of what Taliesin can do: reactive graphics, executed reports, 3D scenes, and the craft of a good page."
toc: false
---

Every page here is one `.tmd` file, rendered by Taliesin. No screenshots, no mockups:
open a demo and poke at the real thing.

## Gradient descent, by hand

An explorable explanation on a single page. Drag the starting point, work the step-size
and momentum sliders, and watch a reactive graphic, a live Observable Plot chart and the
math respond as you move.

[Open the explainer →](descent.tmd)

## A computational report

The one page here that runs code as it builds. Python cleans a committed dataset, draws
the charts and fits the model; the figures and tables are numbered in document order and
cross-referenced from the prose.

[Open the report →](report.tmd)

## Molecules

A ball-and-stick molecule viewer built entirely inside one `{js}` cell: real 3D
coordinates, CPK colors, drag to spin, scroll to zoom.

[Open the viewer →](molecules.tmd)

## A mechanical part, parametric

Two gears built from primitives, meshed and spinning. Sliders drive the tooth count and
the speed, and the geometry rebuilds live.

[Open the part →](gears.tmd)

## The craft of an API page

What documentation looks like when the page format helps: a line-by-line reading of real
code, version and deprecation callouts, a glossary that defines terms in place, and
cross-references the build has already checked.

[Open the page →](api-craft.tmd)
```

- [ ] **Step 2: Replace `gallery/_site.yml`**

```yaml
# The Taliesin gallery: short one-page demos, each showing something impressive
# Taliesin can do. Its own project, its own Cloudflare Pages deploy, its own domain
# (tools/publish.sh). Self-contained: every page this site publishes lives in this
# directory, and nothing is composed into its output.
title: "Taliesin Gallery"
description: "Short one-page demos of what Taliesin can do: reactive graphics, executed reports, 3D scenes, and the craft of a good page."
url: "https://gallery.taliesin.sh"

nav:
  left:
    - { text: "Taliesin", href: "https://taliesin.sh/" }
    - { text: "Guide", href: "https://guide.taliesin.sh/" }
    - { text: "Internals", href: "https://internals.taliesin.sh/" }
  right:
    - { icon: github, href: "https://github.com/AJBogo9/taliesin" }

footer:
  left:
    - { text: "Built with Taliesin" }
  right:
    - { icon: github, href: "https://github.com/AJBogo9/taliesin" }
```

- [ ] **Step 3: De-compose `tools/publish.sh`**

Apply these deletions and replacements (line numbers from the current file):

1. Header comment: on line 8, delete `gallery exhibit links asserted, ` so the line reads `#   tools/publish.sh --check          # THE GATE: lint + build --no-exec into temp dirs, nothing deployed`. Delete the whole paragraph on lines 20 to 25 ("THE GALLERY IS THE ONE EXCEPTION ...").
2. Delete lines 60 to 65 (`GALLERY_EXHIBITS=( ... )` and its comment).
3. Delete the `build_target` function (lines 81 to 103) entirely.
4. Delete `assert_gallery_links` (lines 105 to 146) entirely.
5. In the `--check` loop, replace the body between `$TALIESIN build "$src" --check-only --no-exec` and `rm -rf "$out"` so it reads:

```bash
        out=$(mktemp -d -t "tali-publish-$target-XXXXXX")
        $TALIESIN build "$src" --out "$out" --no-exec
```

6. In the deploy loop, replace `build_target "$target" "$out"` and the gallery `assert_gallery_links` conditional with the single line `$TALIESIN build "$src" --out "$out"`.

- [ ] **Step 4: Reword the pre-push comment**

In `.githooks/pre-push`, replace the comment block on lines 98 to 107 with:

```bash
# The publish gate. FOUR separate projects, four Cloudflare Pages deploys, four domains:
# the marketing site, the two docs books and the gallery each build alone, and they reach
# each other by absolute URL rather than by composition (Cloudflare Pages has no subpath
# deploy, so a composed tree would have to be re-uploaded whole on every change).
#
# `--check` lints and builds all four with --no-exec into temp dirs. build.rs recorded
# that an unrun script had already shipped this project's call-to-action with a 404 once
# (item 149), which is why this runs here rather than by hand.
```

Do not touch the commands the hook runs (a corpus test cross-checks them against gates.sh).

- [ ] **Step 5: Rewrite the corpus.rs gate**

In `crates/core/tests/corpus.rs`, replace the entire `the_readme_marks_the_same_visual_set_the_deploy_ships` test (its doc comment starts at "**Which of these 82 documents ...", roughly lines 1428 to 1534) with:

```rust
/// **Which of these documents a person actually looks at.** Since 2026-08-19 the deploy
/// ships no corpus project at all: the gallery is a self-contained demo site
/// (`gallery/`), and the three former exhibits stayed behind as corpus goldens. What is
/// left to pin is the README's honesty about that: `tech-blog/` is the single
/// human-facing member (deliberately not deployed, so no script can derive it), and
/// every other entry is machine-checked. A row still saying `eye` sends the author to
/// read a fixture; a NEW corpus project with no row at all fails here too, which is the
/// drift this would otherwise grow.
#[test]
fn the_readme_marks_only_tech_blog_as_looked_at() {
    let readme = fs::read_to_string(corpus_dir().join("README.md")).unwrap();
    let rows: Vec<(String, String)> = readme
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| {
            let mut cells = l.trim_matches('|').split('|').map(str::trim);
            Some((cells.next()?.to_string(), cells.next()?.to_string()))
        })
        .collect();
    assert!(
        rows.len() > 15,
        "only {} document rows parsed out of corpus/README.md",
        rows.len()
    );

    // Every project directory and every loose document, from the tree rather than a list.
    let mut entries: Vec<String> = Vec::new();
    for e in fs::read_dir(corpus_dir()).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        if p.is_dir() || name.ends_with(".tmd") {
            entries.push(name);
        }
    }
    entries.sort();

    for entry in &entries {
        let want = if entry == "tech-blog" { "eye" } else { "machine" };
        let token = if entry.ends_with(".tmd") {
            format!("`{entry}`")
        } else {
            format!("`{entry}/")
        };
        let matched: Vec<&(String, String)> =
            rows.iter().filter(|(path, _)| path.contains(&token)).collect();
        assert!(
            !matched.is_empty(),
            "corpus/{entry} has no row in corpus/README.md's document table, so nothing \
             says whether a person is meant to look at it"
        );
        for (path, pass) in matched {
            assert_eq!(
                pass, want,
                "corpus/{entry} is marked `{pass}` in the row {path}, but only tech-blog/ \
                 is human-facing now that no corpus project is deployed"
            );
        }
    }
}
```

If `HashSet` was imported solely for the old test, remove the now-unused import (the compiler will say).

- [ ] **Step 6: Reword `corpus/README.md`**

Replace the "What you verify by eye" paragraph (the one beginning "**Three of these nineteen projects ...", around lines 27 to 33) with:

```markdown
**One project here is looked at by a person**: one post out of `tech-blog/`, sampled
rather than swept, because 19 near-identical posts do not each earn an eyeball. It is
human-facing but deliberately not deployed, so no script can derive it; its row says so.

Nothing else is deployed anywhere. Since 2026-08-19 the gallery is a self-contained demo
site (`gallery/`), and `tarn/`, `descent/` and `analyst/` remain here purely as goldens:
their pins (`tarn.rs`, `descent.rs`, `analyst.rs`) are what a defect in them breaks.
```

In the document table, change the second cell of the `tarn/`, `analyst/` and `descent/` rows from `eye` to `machine`, and in the same rows delete the deploy claims: "; the marketing site's `/gallery/tarn` exhibit" from the tarn row, "`/gallery/analyst`" from the analyst row, "`/gallery/descent`" from the descent row (adjust surrounding punctuation so each row still reads as a sentence).

- [ ] **Step 7: Run the affected suites**

```bash
cargo test -p taliesin-core --test corpus
cargo run -q -p taliesin-server -- build gallery --check-only
tools/publish.sh --check
```

Expected: all exit 0. Note `--check` still exercises `external-prefixes` parsing for zero projects; the key still exists in core until Task 7, which is fine.

- [ ] **Step 8: Format and commit**

```bash
cargo fmt --all
git add gallery/index.tmd gallery/_site.yml tools/publish.sh .githooks/pre-push corpus/README.md crates/core/tests/corpus.rs
git commit -m "feat(gallery)!: a flat self-contained demo site; the publish composition is gone"
```

---

### Task 7: Cut `external-prefixes` from core (test first)

**Files:**
- Modify: `crates/core/src/site/config/mod.rs` (field ~65-68, KNOWN entry + comment ~133-150, parse arm ~319-323, new pin test)
- Modify: `crates/core/src/site/chrome.rs` (use ~273-281, test `a_working_nav_or_footer_href_is_not_reported` ~1116-1143)
- Modify: `crates/core/src/site/mod.rs` (use ~789-799)
- Modify: `crates/core/assets/schema/tali-site.schema.json` (line 74)
- Modify: `editor/vscode/schema/tali-site.schema.json` (line 74)
- Modify: `docs/guide/reference/frontmatter.tmd` (delete the row at line 390)
- Modify: `docs/guide/reference/cli.tmd` (~294-302)

**Interfaces:**
- Consumes: Task 6 (no project in the tree sets the key any more).

- [ ] **Step 1: Write the failing pin test**

In the tests module of `crates/core/src/site/config/mod.rs`, next to `head_is_no_longer_read_and_is_diagnosed_as_unknown` (line ~540), add:

```rust
/// `external-prefixes:` was the gallery composition's one config key (cut 2026-08-19
/// when the gallery became self-contained). The read is gone, not just the docs: a
/// `_site.yml` still carrying it draws the unknown-key diagnostic, and links into a
/// formerly external prefix are reported broken like any other.
#[test]
fn external_prefixes_is_no_longer_read_and_is_diagnosed_as_unknown() {
    let mut w = Vec::new();
    let v: serde_yaml::Value =
        serde_yaml::from_str("title: X\nexternal-prefixes:\n  - tarn\n").unwrap();
    let cfg = parse_native(&v, &mut w, ConfigSource(None));
    assert_eq!(
        cfg.title.as_deref(),
        Some("X"),
        "the rest of the config still parses"
    );
    assert!(
        w.iter().any(|d| d.contains("external-prefixes")),
        "`external-prefixes:` must draw the unknown-key diagnostic: {w:?}"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-core external_prefixes_is_no_longer_read -- --nocapture`
Expected: FAIL (the key is currently in `KNOWN` and draws no warning).

- [ ] **Step 3: Delete the read**

In `crates/core/src/site/config/mod.rs`: delete the `external_prefixes` field and its doc comment, the `"external-prefixes",` entry in the known-keys list together with its whole comment block ("`external-prefixes:` — URL prefixes this project LINKS INTO ..."), and the `external_prefixes:` arm in the parse (the four lines building it).

In `crates/core/src/site/chrome.rs` (~273-281): remove the `self.config.external_prefixes.iter().any(...)` disjunct so the condition is just `if self.root.join(&target).is_file() { continue; }`, and trim the comment above it to only the raw-file judgement.

In `crates/core/src/site/mod.rs` (~789-799): delete the whole `if self.config.external_prefixes...` block and its comment.

- [ ] **Step 4: Fix the working-href test**

In `chrome.rs`'s `a_working_nav_or_footer_href_is_not_reported`: remove `external-prefixes:\n  - docs/guide\n` from the `_site.yml` literal, remove the `- { text: Guide, href: "docs/guide/" }` nav line, drop "an external-prefix project, " from the doc comment and the assertion message, and change the doc comment's "four ways" count accordingly.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p taliesin-core`
Expected: PASS, including the new pin test (the key now falls through to the unknown-key path, which already carries did-you-mean and line-number behavior pinned by neighboring tests).

- [ ] **Step 6: Remove the schema entries**

Delete the `"external-prefixes": {},` line from BOTH `crates/core/assets/schema/tali-site.schema.json` and `editor/vscode/schema/tali-site.schema.json`. Then run the companion's own gate:

```bash
cd editor/vscode && npm test
```

Expected: PASS (this is the only gate that compares the two copies).

- [ ] **Step 7: Update the docs**

In `docs/guide/reference/frontmatter.tmd`, delete the whole `| external-prefixes | ... |` table row (line 390).

In `docs/guide/reference/cli.tmd`, replace the sentence "Add the nested prefix to the parent's `external-prefixes:` so its link checker stops at links it cannot see, and resolve those links yourself against the built output." with "The parent cannot see the nested project's pages, so any link it writes into the nested prefix is reported as broken; resolve such links yourself against the built output." In the following paragraph, replace "Nesting is worth it when the pages genuinely belong to one site (a gallery and its exhibits), and a cost otherwise:" with "Nesting keeps every page under one domain, and costs you the checks:". Leave the rest of both paragraphs unchanged.

Run: `cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec`
Expected: exit 0.

- [ ] **Step 8: Format and commit**

```bash
cargo fmt --all
git add -A crates/core editor/vscode/schema docs/guide/reference
git commit -m "cut(site): external-prefixes, the composition's one config key"
```

---

### Task 8: Prose sweep over the living docs

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/guide/index.tmd:31`
- Check: `site/README.md`, root `README.md`

**Interfaces:** none; pure documentation truth-keeping.

- [ ] **Step 1: CLAUDE.md**

Three edits:

1. Tree-map entry: replace the `gallery/` line ("the exhibit index: its own project + domain, the ONE project that builds others (corpus/{tarn,descent,analyst}) under its output") with: `gallery/         short one-page demos (its own project + domain): a flat, self-contained site; nothing is composed into its output`.
2. In the `src/site/` bullet, delete the sentence "Only gallery/ nests others under its output, parent first (the parent's sweep deletes output it did not itself write)" so the deploy note ends at "(tools/publish.sh)".
3. Grep for any remaining composition claim: `grep -n 'exhibit\|nests\|composes' CLAUDE.md` and fix stragglers (the `serve_site` section's publish.sh sentence stays; it makes no composition claim).

- [ ] **Step 2: docs/guide/index.tmd**

Replace "and the [Gallery](https://gallery.taliesin.sh/) has whole projects you can open and read." with "and the [Gallery](https://gallery.taliesin.sh/) shows one-page demos you can open and poke at."

- [ ] **Step 3: Sweep for stragglers**

```bash
grep -rn 'exhibit' README.md site/README.md docs/guide docs/internals site/*.tmd
```

Fix any hit that claims the gallery composes or contains other projects (historical `notes/` files are records; do NOT edit them). `site/README.md`'s deploy table needs no change unless a hit says otherwise.

- [ ] **Step 4: Lint the touched books and commit**

```bash
cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec
git add CLAUDE.md docs/guide site/README.md README.md
git commit -m "docs: the gallery is a flat demo site; composition claims corrected"
```

(Drop unchanged files from the `git add` list as appropriate.)

---

### Task 9: Full verification

**Files:** none new; fixes only if something fails.

- [ ] **Step 1: Kill stale processes**

```bash
pkill -f 'cargo (test|run)' || true
```

- [ ] **Step 2: Run every gate in one process**

```bash
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Expected: the script's own verdict line reports every gate ran and passed, zero ignored. Quote that verdict line in the completion report; never summarize it from memory. If anything fails, fix it (returning to the responsible task's files) and re-run.

- [ ] **Step 3: Final browser pass**

Serve `cargo run -q -p taliesin-server -- preview gallery 4388` and click through from the index: all five demo links resolve, each demo still behaves per its task's verification list, Cmd-K search finds content from at least two different demos, no console errors anywhere, light and dark both correct on the index.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A && git commit -m "fix(gallery): verification fallout"
```

Only if Step 2 or 3 required changes; otherwise nothing to commit.
