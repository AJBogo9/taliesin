# Scrollytelling `:::{.scrolly}` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `:::{.scrolly}` fenced div that pins a sticky visual stage beside a scrolling column of `.step` divs; the active step drives the stage via `data-scrolly-state` (CSS) and, with `name=`, a hidden `data-qmd-input` so a sticky `{js}` cell reacts through the shipped reactive graph.

**Architecture:** A new `build_container` arm in `divs.rs` (mirroring `.code-walkthrough`) partitions inner blocks into a sticky stage (non-`.step` blocks) + a `.step` column, emitting a hidden reactive input when `name=` is set. The `.step` arm gains `state=` → `data-state`. A new `scrolly.js` enhancer (the walkthrough IntersectionObserver band) flips the active step → sets `data-scrolly-state` + drives the hidden input. Reuses the shipped `{input}` registration; no `qmd-js.js`/reactive-runtime change.

**Tech Stack:** Rust (render module, edition 2024), vanilla JS (ES5-style, bundled), IntersectionObserver, the shipped `{js}` reactive graph, CSS grid + `position: sticky`. Tests: Rust unit + corpus + chrome-devtools.

## Global Constraints

- **HTML-only**, **offline**, **deck-skipped** (enhancer no-ops without a `.scrolly`).
- **Single editing surface:** scrolling is reader interaction; the preview never writes the `.qmd`.
- **Block model untouched:** the `.scrolly` div is one container block with `data-block-id` + `data-sourcepos`; inner blocks keep their ids via grouping.
- **No `qmd-js.js`/reactive-runtime change:** reuse the shipped `{input}` `[data-qmd-input]` scan → `registerInput` → `scheduleFrom` by emitting a hidden input the enhancer drives.
- **Rides supported seams** (`build_container` + `qmdEnhancers`); Do-NOT-touch machinery untouched. No refactor of `walkthrough.js`.
- **JS style:** `var`/`function`/`[].slice.call`, no arrows/`const`/`let`, mirroring `walkthrough.js`.
- **Naming:** active-step class is `scrolly-step-active` (NOT `cw-step-active`); root attr `data-scrolly-state`; hidden input `.qmd-scrolly-input`; stage `.scrolly-stage`; steps wrap `.scrolly-steps`.
- **Stage = all non-`.step` inner blocks concatenated; narration = `.step` blocks.**

---

### Task 1: `validate_scrolly` diagnostic helper

**Files:**
- Modify: `crates/core/src/render/validate.rs` (add `validate_scrolly` + unit tests)

**Interfaces:**
- Produces: `pub(crate) fn validate_scrolly(has_stage: bool, has_steps: bool, line: usize, file: Option<String>) -> Vec<Warning>`.

- [ ] **Step 1: Write the failing unit tests**

In `crates/core/src/render/validate.rs`, inside `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn scrolly_without_stage_is_flagged() {
        let w = validate_scrolly(false, true, 3, Some("s.qmd".into()));
        assert_eq!(w.len(), 1);
        assert!(w[0].message.contains("no sticky stage"), "got: {}", w[0].message);
        assert_eq!(w[0].line, Some(3));
    }

    #[test]
    fn scrolly_without_steps_is_flagged() {
        let w = validate_scrolly(true, false, 5, None);
        assert_eq!(w.len(), 1);
        assert!(w[0].message.contains("no `.step`"), "got: {}", w[0].message);
    }

    #[test]
    fn scrolly_complete_is_clean() {
        assert!(validate_scrolly(true, true, 1, None).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::validate 2>&1 | tail -10`
Expected: FAIL — `validate_scrolly` not found.

- [ ] **Step 3: Implement `validate_scrolly`**

In `crates/core/src/render/validate.rs`, after `validate_input` (before `#[cfg(test)]`), add:

```rust
/// Validate a `.scrolly` container (located, click-to-source). Warns when there is no
/// sticky stage block or no `.step` divs to scroll through. Purely diagnostic — it still
/// renders. Mirrors `validate_walkthrough`.
pub(crate) fn validate_scrolly(
    has_stage: bool,
    has_steps: bool,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if !has_stage {
        out.push(
            Warning::new(
                "`.scrolly` has no sticky stage (add a figure or `{js}` cell)".to_string(),
            )
            .at(file.clone(), line as u32),
        );
    }
    if !has_steps {
        out.push(
            Warning::new("`.scrolly` has no `.step` divs to scroll through".to_string())
                .at(file, line as u32),
        );
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib render::validate 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/render/validate.rs
git commit -m "feat(explorable): validate_scrolly diagnostics for .scrolly containers"
```

---

### Task 2: `.step` `state=` extension + `.scrolly` `build_container` arm + render tests + CSS

**Files:**
- Modify: `crates/core/src/render/divs.rs` (extend `.step` arm; add `.scrolly` arm + `first_step_state` helper)
- Modify: `crates/core/src/render/tests.rs` (render unit tests)
- Modify: `crates/core/assets/css/base.css` (`.qmd-scrolly` layout)

**Interfaces:**
- Consumes: `validate_scrolly` (Task 1); `escape_attr`, `concat`, `data`, `open_line`, `file` in scope in `build_container`; `Block` (has `.html`).
- Produces: `<div class="qmd-scrolly">` with `scrolly-steps`/`scrolly-stage` + optional hidden `.qmd-scrolly-input[data-qmd-input]`; `.step` divs carry `data-state`.

- [ ] **Step 1: Write the failing render tests**

In `crates/core/src/render/tests.rs`, append:

```rust
#[test]
fn scrolly_arm_emits_stage_steps_and_reactive_input() {
    let doc = render_document(
        "::: {.scrolly name=\"scene\"}\nThe stage paragraph.\n\n::: {.step state=\"a\"}\nStep A.\n:::\n\n::: {.step state=\"b\"}\nStep B.\n:::\n:::\n",
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-scrolly\""), "wrapper: {h}");
    assert!(h.contains("class=\"scrolly-steps\"") && h.contains("class=\"scrolly-stage\""), "split: {h}");
    assert!(h.contains("data-scrolly-name=\"scene\""), "name attr: {h}");
    assert!(
        h.contains("<input type=\"hidden\" class=\"qmd-scrolly-input\" data-qmd-input=\"scene\" value=\"a\">"),
        "hidden reactive input with first step's state: {h}"
    );
    assert!(h.contains("data-state=\"a\"") && h.contains("data-state=\"b\""), "step states: {h}");
    assert!(h.contains("The stage paragraph."), "stage content present: {h}");
}

#[test]
fn scrolly_without_name_omits_hidden_input() {
    let doc = render_document(
        "::: {.scrolly}\nStage.\n\n::: {.step state=\"a\"}\nA.\n:::\n:::\n",
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-scrolly\""));
    assert!(!h.contains("data-qmd-input"), "no hidden input without name=: {h}");
    assert!(!h.contains("data-scrolly-name"), "no name attr: {h}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::tests::scrolly 2>&1 | tail -15`
Expected: FAIL — the bare `.scrolly` hits the generic arm (`<div class="scrolly">`), no split/hidden input.

- [ ] **Step 3: Extend the `.step` arm with `state=`**

In `crates/core/src/render/divs.rs`, replace the `.step` arm body:

```rust
    } else if attrs.classes.iter().any(|c| c == "step") {
        // A scroll step: carry its line-focus spec as `data-cw-lines` (walkthrough.js) and/or
        // its scrolly state as `data-state` (scrolly.js); keep the div's own id/sourcepos so
        // its prose stays locatable. Meaningful inside `.code-walkthrough`/`.scrolly`.
        let id_attr = id_attr(attrs.id.as_deref());
        let cw_lines = match attrs.get("lines") {
            Some(spec) if !spec.is_empty() => {
                format!(" data-cw-lines=\"{}\"", escape_attr(spec))
            }
            _ => String::new(),
        };
        let state = match attrs.get("state") {
            Some(s) if !s.is_empty() => format!(" data-state=\"{}\"", escape_attr(s)),
            _ => String::new(),
        };
        let body = concat(&inner);
        format!("<div class=\"step\"{id_attr}{data}{cw_lines}{state}>{body}</div>")
    } else if attrs.classes.iter().any(|c| c == "panel-tabset") {
```

- [ ] **Step 4: Add the `.scrolly` arm**

In `crates/core/src/render/divs.rs`, insert immediately before the final `} else {` of `build_container`:

```rust
    } else if attrs.classes.iter().any(|c| c == "scrolly") {
        // Scrollytelling: a sticky visual stage (the non-.step inner blocks) beside a
        // scrolling column of `.step` divs. The active step (scrolly.js, IntersectionObserver)
        // sets `data-scrolly-state` on the root for CSS, and — when `name=` is set — drives a
        // hidden `data-qmd-input` so a sticky `{js}` cell reacts via `//| input:` through the
        // shipped reactive graph. Read-only: scroll is reader interaction, never a source write.
        let is_step = |b: &Block| b.html.trim_start().starts_with("<div class=\"step\"");
        let steps: String = inner
            .iter()
            .filter(|b| is_step(b))
            .map(|b| b.html.as_str())
            .collect();
        let stage: String = inner
            .iter()
            .filter(|b| !is_step(b))
            .map(|b| b.html.as_str())
            .collect();
        let has_steps = inner.iter().any(is_step);
        let has_stage = inner.iter().any(|b| !is_step(b));
        for w in super::validate::validate_scrolly(has_stage, has_steps, open_line, file.clone()) {
            warnings.push(w);
        }
        // The reactive bridge: a hidden input named `name` whose value is the active step's
        // state (initial = the first .step's state, so consumer cells read a sane value).
        let (name_attr, hidden) = match attrs.get("name") {
            Some(n) if !n.is_empty() => {
                // `first_step_state` is read back out of the already-emitted step html, so it
                // is already attribute-escaped — do NOT re-escape it.
                let first_state = first_step_state(&inner).unwrap_or_default();
                (
                    format!(" data-scrolly-name=\"{}\"", escape_attr(n)),
                    format!(
                        "<input type=\"hidden\" class=\"qmd-scrolly-input\" data-qmd-input=\"{}\" value=\"{first_state}\">",
                        escape_attr(n)
                    ),
                )
            }
            _ => (String::new(), String::new()),
        };
        format!(
            "<div class=\"qmd-scrolly\"{data}{name_attr}>{hidden}<div class=\"scrolly-steps\">{steps}</div><div class=\"scrolly-stage\">{stage}</div></div>"
        )
    } else {
```

- [ ] **Step 5: Add the `first_step_state` helper**

In `crates/core/src/render/divs.rs`, after `build_container` (near the other free helpers, e.g. before `callout_icon`), add:

```rust
/// The `data-state="…"` of the first `.step` block in `inner` (already attribute-escaped,
/// since it is read back out of the step's emitted html). Used to seed the scrolly hidden
/// input's initial value so consumer cells read a sane value before any scroll.
fn first_step_state(inner: &[Block]) -> Option<String> {
    let step = inner
        .iter()
        .find(|b| b.html.trim_start().starts_with("<div class=\"step\""))?;
    let i = step.html.find("data-state=\"")?;
    let rest = &step.html[i + "data-state=\"".len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
```

- [ ] **Step 6: Run to verify the render tests pass**

Run: `cargo test -p qmd-fast-core --lib render::tests::scrolly 2>&1 | tail -15`
Expected: PASS (both).

- [ ] **Step 7: Add the CSS**

In `crates/core/assets/css/base.css`, after the `.code-walkthrough` block (the mobile `@media` rules ending ~line 442), add:

```css
  /* Scrollytelling (`::: {.scrolly}`): prose steps scroll past a sticky visual stage.
     Generalizes the code-walkthrough layout; the active step drives data-scrolly-state +
     (with name=) a hidden reactive input the sticky {js} cell reads. */
  .qmd-scrolly { display: grid; grid-template-columns: 1fr minmax(18rem, .9fr);
    gap: clamp(1rem, 4vw, 3rem); align-items: start; margin: 1.5rem 0; }
  .qmd-scrolly .scrolly-steps, .qmd-scrolly .scrolly-stage { min-width: 0; }
  .scrolly-steps .step { min-height: 60vh; display: flex; flex-direction: column;
    justify-content: center; opacity: .4; border-left: 3px solid transparent;
    padding: .4rem 0 .4rem 1rem; }
  .scrolly-steps .step.scrolly-step-active { opacity: 1; border-left-color: var(--qmd-accent); }
  .scrolly-steps .step > :first-child { margin-top: 0; }
  .scrolly-steps .step > :last-child { margin-bottom: 0; }
  .scrolly-stage { position: sticky; top: 0; align-self: start; height: 100vh;
    display: flex; flex-direction: column; justify-content: center; overflow: auto; }
  .qmd-scrolly-input { display: none; }
  @media (prefers-reduced-motion: no-preference) {
    .scrolly-steps .step { transition: opacity .25s ease, border-color .25s ease; }
  }
  @media (max-width: 73rem) {
    /* single column: lift the stage to a sticky strip on top, prose flows beneath */
    .qmd-scrolly { display: flex; flex-direction: column; gap: 0; }
    .scrolly-stage { order: -1; position: sticky; top: 0; height: auto; max-height: 48vh;
      z-index: 5; background: var(--qmd-bg); }
    .scrolly-steps .step { min-height: auto; padding: 1.4rem .9rem; }
  }
```

- [ ] **Step 8: Build + render lib tests + fmt**

Run: `cargo build -p qmd-fast-core && cargo test -p qmd-fast-core --lib render 2>&1 | tail -5 && cargo fmt --check && echo FMT_OK`
Expected: builds; render lib tests pass; `FMT_OK`.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/render/divs.rs crates/core/src/render/tests.rs crates/core/assets/css/base.css
git commit -m "feat(explorable): .scrolly arm (sticky stage + steps + reactive bridge) and .step state="
```

---

### Task 3: `scrolly.js` enhancer + bundle

**Files:**
- Create: `crates/core/assets/js/scrolly.js`
- Modify: `crates/core/src/render/mod.rs` (`SCROLLY_JS` const + `code_scripts()`)

**Interfaces:**
- Consumes: `window.qmdEnhancers.register`; the shipped `{input}` registration (reacts to the hidden input's `input` event).
- Produces: active-step tracking that sets `data-scrolly-state` + drives `.qmd-scrolly-input`.

- [ ] **Step 1: Write `scrolly.js`**

Create `crates/core/assets/js/scrolly.js`:

```js
// Scrollytelling: scroll-driven sticky-stage scenes.
//
// The server emits `::: {.scrolly}` as a `.scrolly-steps` column (one `.step[data-state]`
// per scene) beside a sticky `.scrolly-stage`. As the reader scrolls, the step nearest the
// viewport centre becomes active: its `data-state` is mirrored to `data-scrolly-state` on
// the root (for pure-CSS effects) and, when the `.scrolly` was given a `name=`, pushed into
// a hidden `.qmd-scrolly-input[data-qmd-input]` (value + an `input` event) so the shipped
// reactive graph re-runs the sticky `{js}` cell via `//| input:`. Read-only / scroll-only.
//
// Reuses the deck/walkthrough IntersectionObserver activation band, but does NOT depend on
// walkthrough.js. Registered through `qmdEnhancers`; idempotent (`data-scrolly-init`).
(function () {
  function initScrolly(root) {
    var steps = Array.prototype.slice.call(root.querySelectorAll('.scrolly-steps .step'));
    if (!steps.length) return;
    var input = root.querySelector('.qmd-scrolly-input');
    var active = -1;
    function apply(i, dispatch) {
      if (i === active) return;
      active = i;
      steps.forEach(function (s, j) { s.classList.toggle('scrolly-step-active', j === i); });
      var state = steps[i] ? (steps[i].getAttribute('data-state') || '') : '';
      root.setAttribute('data-scrolly-state', state);
      if (input && dispatch && input.value !== state) {
        input.value = state;
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }
    // Track which steps straddle the activation band; the LAST one wins. Before any step
    // crosses, the first is active so the stage never starts blank.
    var visible = new Set();
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target);
      });
      var last = -1;
      steps.forEach(function (s, j) { if (visible.has(s)) last = j; });
      apply(last === -1 ? 0 : last, true);
    }, { rootMargin: '-45% 0px -45% 0px', threshold: 0 });
    steps.forEach(function (s) { io.observe(s); });
    // Initial: set the state attribute but do NOT dispatch — the hidden input's
    // server-rendered value already matches step 0, and the cell ran once on mount.
    apply(0, false);
  }

  function enhance(root) {
    (root || document)
      .querySelectorAll('.qmd-scrolly:not([data-scrolly-init])')
      .forEach(function (el) {
        el.setAttribute('data-scrolly-init', '1');
        initScrolly(el);
      });
  }

  if (window.qmdEnhancers && window.qmdEnhancers.register) {
    window.qmdEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () { enhance(document); });
  }
})();
```

- [ ] **Step 2: Bundle it in `code_scripts()`**

In `crates/core/src/render/mod.rs`, add the const next to `WALKTHROUGH_JS`/`TABSET_JS` (~line 1052):

```rust
/// Scroll-driven sticky-stage scenes for `::: {.scrolly}`. Registers through `qmdEnhancers`,
/// no-ops without a `.scrolly`, rides in [`code_scripts`].
const SCROLLY_JS: &str = include_str!("../../assets/js/scrolly.js");
```

And add it to the `code_scripts()` format string (after `{TABSET_JS}`):

```rust
        "<script>{CODE_ENHANCE_JS}</script>\n<script>{mermaid}</script>\n<script>{QMD_JS}</script>\n<script>{WALKTHROUGH_JS}</script>\n<script>{TABSET_JS}</script>\n<script>{SCROLLY_JS}</script>"
```

- [ ] **Step 3: Syntax-check + build**

Run: `node --check crates/core/assets/js/scrolly.js && cargo build -p qmd-fast-core`
Expected: no error; build succeeds.

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/scrolly.js crates/core/src/render/mod.rs
git commit -m "feat(explorable): scrolly.js enhancer — active step drives state + reactive input"
```

---

### Task 4: Corpus pin doc + browser verification

**Files:**
- Create: `corpus/explorable/scrolly.qmd`
- (verify) chrome-devtools

- [ ] **Step 1: Write the pin doc**

Create `corpus/explorable/scrolly.qmd`:

````markdown
---
title: "Scrollytelling"
---

Scroll the narration on the left; the chart on the right reacts to the step in view. The
active step drives a reactive value (`scene`), so the sticky `{js}` cell re-runs as you scroll.

::: {.scrolly name="scene"}
```{js}
//| input: scene
const scene = tali.value("scene");
const xs = [1, 2, 3, 4, 5, 6];
const data = xs.map((x) => ({ x, y: scene === "spike" ? x * x : x * 2 }));
return Plot.plot({
  height: 240,
  marks: [Plot.line(data, { x: "x", y: "y" }), Plot.dot(data, { x: "x", y: "y" })],
});
```
::: {.step state="trend"}
First, the steady **linear** trend rises gently across the range.
:::
::: {.step state="spike"}
Now the same series read as a **quadratic** spike — the curve bends sharply upward.
:::
::: {.step state="trend"}
And back to the linear baseline for comparison.
:::
:::
````

- [ ] **Step 2: Corpus invariants**

Run: `cargo test -p qmd-fast-core --test corpus 2>&1 | tail -6`
Expected: PASS — the new doc renders; unique block ids; valid sourcepos; clean front-matter.

- [ ] **Step 3: Serve + browser-verify (chrome-devtools MCP)**

Build + serve on a real port (live `{js}` needs a server):
```bash
cargo build -p qmd-fast-server
./target/debug/qmd-fast preview corpus/explorable/scrolly.qmd 4390   # run in background
```
Navigate chrome-devtools to `http://127.0.0.1:4390`, then verify:

```js
// initial: first step active, state="trend", chart present
var root = document.querySelector('.qmd-scrolly');
var before = { state: root.getAttribute('data-scrolly-state'),
               svg: !!root.querySelector('.scrolly-stage svg') };
// drive the 2nd step into view (scrollIntoView is reliable in the harness)
var steps = root.querySelectorAll('.scrolly-steps .step');
steps[1].scrollIntoView({ block: 'center' });
await new Promise(r => setTimeout(r, 200));
var after = { state: root.getAttribute('data-scrolly-state'),
              active: root.querySelector('.step.scrolly-step-active').getAttribute('data-state') };
return { before, after };
```

Expected: `before.state` is `"trend"`, `before.svg` true; `after.state` is `"spike"` and `after.active` is `"spike"` (the chart re-ran for the new scene).

Fallback if headless scroll is unreliable: prove the reactive wiring directly —
```js
var inp = document.querySelector('.qmd-scrolly-input');
inp.value = "spike"; inp.dispatchEvent(new Event('input', {bubbles:true}));
await new Promise(r=>setTimeout(r,120));
return document.querySelector('.scrolly-stage svg') ? 're-ran' : 'no svg';
```
Expected: `"re-ran"` (the cell re-executed reading `tali.value("scene")` = "spike").

Then `list_console_messages` → 0 errors. Take a screenshot.

- [ ] **Step 4: Commit**

```bash
git add corpus/explorable/scrolly.qmd
git commit -m "test(explorable): pin scrollytelling corpus doc (reactive sticky-stage scenes)"
```

---

### Task 5: Docs + full verification

**Files:**
- Modify: `corpus/README.md` (add an `explorable/` row)
- Modify: `notes/backlog.md` + `notes/FEATURE-IDEAS.md` (mark #46 shipped)

- [ ] **Step 1: Corpus README**

In `corpus/README.md`'s Documents table, add:

```markdown
| `explorable/scrolly.qmd` | Scrollytelling | `::: {.scrolly}` sticky stage + `.step` scenes; the active step drives a reactive value a `{js}` cell reads (`//| input:`) | (purpose-built) |
```

- [ ] **Step 2: Notes**

In `notes/backlog.md`, add a one-line shipped note (Pillar III `:::{.scrolly}` scrollytelling: sticky stage + `.step` scenes; active step → `data-scrolly-state` + hidden `data-qmd-input` reusing the `{input}` graph; generalizes the walkthrough machine; pinned `corpus/explorable/scrolly.qmd`). In `notes/FEATURE-IDEAS.md`, mark idea #46 ✅ SHIPPED 2026-06-26 with a one-line note (incl. that it reuses the `{input}` hidden-input bridge rather than a bespoke event).

- [ ] **Step 3: Full verification**

Run:
```bash
cargo test -p qmd-fast-core
cargo fmt --check
node --check crates/core/assets/js/scrolly.js
```
Expected: all tests pass (0 failed across binaries), fmt clean, JS OK.

- [ ] **Step 4: Commit**

```bash
git add corpus/README.md notes/backlog.md notes/FEATURE-IDEAS.md
git commit -m "docs(explorable): record scrollytelling; mark idea #46 shipped"
```

---

## Self-Review

**Spec coverage:**
- `.scrolly` arm: stage (all non-step) + `.step` column + hidden reactive input when `name=` → Task 2. ✓
- `.step` `state=` → `data-state` → Task 2 Step 3. ✓
- First-step-state seeds hidden input value → Task 2 `first_step_state` + test. ✓
- `scrolly.js`: IO band, `data-scrolly-state`, drive hidden input, no-dispatch-on-init → Task 3. ✓
- Reuse shipped `{input}` registration, no `qmd-js.js` change → Task 3 (drives the input; registration already exists). ✓
- CSS grid + sticky + mobile + reduced-motion → Task 2 Step 7. ✓
- `validate_scrolly` (no stage / no steps), located → Task 1, wired Task 2. ✓
- Corpus pin + tests (render, corpus, browser incl. scrollIntoView + fallback) → Tasks 2/4. ✓
- Invariants → Global Constraints. ✓
- Docs → Task 5. ✓

**Placeholder scan:** no TBD/TODO; all code complete; the browser step has exact expected values + a fallback. ✓

**Type/name consistency:** `validate_scrolly(has_stage, has_steps, line, file) -> Vec<Warning>` (Task 1) called in Task 2. HTML contract — `.qmd-scrolly` / `scrolly-steps` / `scrolly-stage` / `scrolly-step-active` / `data-scrolly-state` / `data-scrolly-name` / `.qmd-scrolly-input[data-qmd-input]` / `.step[data-state]` — identical across Task 2 (emits), Task 2 CSS (styles), Task 3 (reads/drives). `first_step_state` defined + used in Task 2. `SCROLLY_JS` const + `code_scripts()` in Task 3. ✓
