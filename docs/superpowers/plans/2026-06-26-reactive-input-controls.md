# `{input}` reactive controls — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A declarative `::: {.input name="k" type="slider" …}` fenced div that emits a static, keyboard-accessible labeled control feeding the already-shipped `{js}` reactive graph — "drag the slider, the chart updates" with no `//| viewof` boilerplate.

**Architecture:** A new `build_container` arm in `divs.rs` emits a static `.qmd-input` control tagged `data-qmd-input="name"`; an additive scan in `qmd-js.js` `enhance` (before the cell scan) registers it via the existing `registerInput`, so consumer cells read `qmd.value("name")` and the existing `scheduleFrom` re-runs the transitive-downstream closure. A `validate_input` helper emits located warnings. Five types: slider, number, checkbox, text, select.

**Tech Stack:** Rust (render module, edition 2024), vanilla JS (the bundled `qmd-js.js`, ES5-style), native form controls, the CSS Custom-property theme system. Tests: Rust unit + corpus + chrome-devtools.

## Global Constraints

- **HTML-only output**; **offline** (native controls + bundled `qmd-js.js`; no new dependency).
- **Single editing surface preserved:** the control is reader interaction with the read-only rendered view (exactly like existing `//| viewof` sliders); it drives client JS, never writes the `.qmd`.
- **Block model untouched:** the `.input` div is one container block carrying the usual `data-block-id` + `data-sourcepos` (like every `build_container` arm); no diff/numbering/sourcepos change.
- **Rides supported seams:** a `build_container` arm + an additive scan in the `qmdEnhancers`-registered `enhance`. Do-NOT-touch machinery (`cite.rs`, `includes.rs`, numbering, exec/freeze/kernel) untouched.
- **JS style:** match the surrounding `qmd-js.js` — `var`, `function`, `[].forEach.call`, no arrow funcs / `const`/`let`, "use strict" already set.
- **No new reactive machinery:** reuse `registerInput`, `scheduleFrom`, `readValue`, the graph.
- **Five types only** (`slider`/`range`, `number`, `checkbox`, `text`, `select`); `type` defaults to `slider`; `<output>` readout for slider only.

---

### Task 1: `validate_input` diagnostic helper

A pure validation function (decoupled from `DivAttrs`, like `validate_callout_kind`) the arm will call.

**Files:**
- Modify: `crates/core/src/render/validate.rs` (add `INPUT_TYPES` const + `validate_input` + unit tests)

**Interfaces:**
- Produces: `pub(crate) const INPUT_TYPES: &[&str]`; `pub(crate) fn validate_input(name: Option<&str>, kind: Option<&str>, options: Option<&str>, line: usize, file: Option<String>) -> Vec<Warning>`.
- Consumes: `crate::frontmatter::unknown_key_message`, `Warning::new(..).at(file, line)`.

- [ ] **Step 1: Write the failing unit tests**

In `crates/core/src/render/validate.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add:

```rust
    #[test]
    fn input_without_name_is_flagged() {
        let w = validate_input(None, Some("slider"), None, 4, Some("d.qmd".into()));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].message, "`.input` needs a `name=` to feed the reactive graph");
        assert_eq!(w[0].line, Some(4));
    }

    #[test]
    fn input_unknown_type_has_did_you_mean() {
        let w = validate_input(Some("k"), Some("slidr"), None, 2, None);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].message, "unknown input type `slidr` (did you mean `slider`?)");
    }

    #[test]
    fn input_select_without_options_is_flagged() {
        let w = validate_input(Some("c"), Some("select"), None, 9, None);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].message, "`.input type=select` needs `options=\"a,b,c\"`");
    }

    #[test]
    fn input_valid_slider_is_clean() {
        assert!(validate_input(Some("k"), Some("slider"), None, 1, None).is_empty());
        assert!(validate_input(Some("c"), Some("select"), Some("a,b"), 1, None).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::validate 2>&1 | tail -20`
Expected: FAIL — `validate_input` / `INPUT_TYPES` not found.

- [ ] **Step 3: Implement `validate_input`**

In `crates/core/src/render/validate.rs`, after the `CALLOUT_KINDS` const (line ~36), add:

```rust
/// Input control types `.input type=` recognizes.
pub(crate) const INPUT_TYPES: &[&str] =
    &["slider", "range", "number", "checkbox", "text", "select"];
```

After `validate_tabset` (before the `#[cfg(test)]` module), add:

```rust
/// Validate a `.input` reactive-control container (located, click-to-source). Warns when
/// `name` is missing (the control can't feed the reactive graph), when `type` is unknown
/// (with a did-you-mean), or when a `select` has no `options`. Purely diagnostic — the
/// div still renders.
pub(crate) fn validate_input(
    name: Option<&str>,
    kind: Option<&str>,
    options: Option<&str>,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if name.unwrap_or("").trim().is_empty() {
        out.push(
            Warning::new("`.input` needs a `name=` to feed the reactive graph".to_string())
                .at(file.clone(), line as u32),
        );
    }
    if let Some(t) = kind {
        if !INPUT_TYPES.contains(&t) {
            out.push(
                Warning::new(unknown_key_message("input type", t, INPUT_TYPES))
                    .at(file.clone(), line as u32),
            );
        }
    }
    if kind == Some("select") && options.unwrap_or("").trim().is_empty() {
        out.push(
            Warning::new("`.input type=select` needs `options=\"a,b,c\"`".to_string())
                .at(file, line as u32),
        );
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib render::validate 2>&1 | tail -20`
Expected: PASS (all four new tests + existing validate tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/render/validate.rs
git commit -m "feat(reactive): validate_input diagnostics for .input controls"
```

---

### Task 2: The `.input` `build_container` arm + render tests + CSS

Emit the static control HTML for the five types, calling `validate_input`. Style it.

**Files:**
- Modify: `crates/core/src/render/divs.rs` (add the `.input` arm in `build_container`, before the final `else`)
- Modify: `crates/core/src/render/tests.rs` (render unit tests)
- Modify: `crates/core/assets/css/base.css` (`.qmd-input` styling)

**Interfaces:**
- Consumes: `validate_input` (Task 1); `DivAttrs::get`, `escape_attr`, `html_escape`, the arm-local `id`, `data`, `open_line`, `file` already in scope in `build_container`.
- Produces: a `<div class="qmd-input">` carrying a `data-qmd-input="<name>"` control (+ slider `<output data-qmd-out>`), consumed by Task 3's runtime + Task 4's pin doc.

- [ ] **Step 1: Write the failing render tests**

In `crates/core/src/render/tests.rs`, add (the module already `use super::*;` and calls `render_document`):

```rust
#[test]
fn input_slider_emits_reactive_control() {
    let doc = render_document(
        "::: {.input name=\"k\" type=\"slider\" min=\"1\" max=\"10\" value=\"3\" label=\"k\"}\n:::\n",
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-input\""), "wrapper: {h}");
    assert!(h.contains("data-qmd-input=\"k\""), "named input: {h}");
    assert!(h.contains("type=\"range\""), "range control: {h}");
    assert!(h.contains("min=\"1\"") && h.contains("max=\"10\"") && h.contains("value=\"3\""));
    assert!(
        h.contains("<output class=\"qmd-input-out\" data-qmd-out>3</output>"),
        "slider readout: {h}"
    );
    assert!(h.contains(">k</label>"), "label: {h}");
}

#[test]
fn input_other_types_emit_their_native_control() {
    let num = render_document("::: {.input name=\"n\" type=\"number\" step=\"0.1\"}\n:::\n").body_html();
    assert!(num.contains("type=\"number\"") && num.contains("step=\"0.1\""));
    assert!(!num.contains("data-qmd-out"), "no readout on number: {num}");

    let cb = render_document("::: {.input name=\"on\" type=\"checkbox\" value=\"true\"}\n:::\n").body_html();
    assert!(cb.contains("type=\"checkbox\"") && cb.contains(" checked"), "checked: {cb}");

    let tx = render_document("::: {.input name=\"q\" type=\"text\" value=\"hi\"}\n:::\n").body_html();
    assert!(tx.contains("type=\"text\"") && tx.contains("value=\"hi\""));

    let sel = render_document("::: {.input name=\"c\" type=\"select\" options=\"a,b,c\" value=\"b\"}\n:::\n").body_html();
    assert!(sel.contains("<select"), "select: {sel}");
    assert!(sel.contains("<option>a</option>") && sel.contains("<option selected>b</option>"), "options: {sel}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p qmd-fast-core --lib render::tests::input_ 2>&1 | tail -20`
Expected: FAIL — the bare `.input` div currently hits the fallback arm (`<div class="input">`), so `data-qmd-input`/`type="range"` are absent.

- [ ] **Step 3: Implement the `.input` arm**

In `crates/core/src/render/divs.rs`, in `build_container`, insert this arm **immediately before the final `} else {`** (after the `panel-tabset` arm, ~line 471):

```rust
    } else if attrs.classes.iter().any(|c| c == "input") {
        // Reactive input control: a static, keyboard-accessible labeled control whose value
        // feeds the {js} reactive graph (registered by qmd-js.js as a named input via
        // `data-qmd-input`). Five types; the slider gets a live <output> readout. The div
        // body is ignored (empty by convention). Read-only: reader interaction with the
        // rendered view, never a source write.
        for w in super::validate::validate_input(
            attrs.get("name"),
            attrs.get("type"),
            attrs.get("options"),
            open_line,
            file.clone(),
        ) {
            warnings.push(w);
        }
        let name = attrs.get("name").unwrap_or("");
        let kind = attrs.get("type").unwrap_or("slider");
        let label = attrs.get("label").unwrap_or(name);
        let ctrl_id = format!("qin-{id}");
        let name_a = escape_attr(name);
        let num_attr = |k: &str| {
            attrs
                .get(k)
                .map(|v| format!(" {k}=\"{}\"", escape_attr(v)))
                .unwrap_or_default()
        };
        let control = match kind {
            "select" => {
                let opts: String = attrs
                    .get("options")
                    .unwrap_or("")
                    .split(',')
                    .map(|o| o.trim())
                    .filter(|o| !o.is_empty())
                    .map(|o| {
                        let sel = if attrs.get("value") == Some(o) { " selected" } else { "" };
                        format!("<option{sel}>{}</option>", html_escape(o))
                    })
                    .collect();
                format!(
                    "<select id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\">{opts}</select>"
                )
            }
            "checkbox" => {
                let checked = if attrs.get("value") == Some("true") { " checked" } else { "" };
                format!(
                    "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"checkbox\"{checked}>"
                )
            }
            "text" => {
                format!(
                    "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"text\"{}>",
                    num_attr("value")
                )
            }
            other => {
                // slider/range/number: numeric, sharing min/max/step/value
                let html_type = if other == "number" { "number" } else { "range" };
                format!(
                    "<input id=\"{ctrl_id}\" class=\"qmd-input-control\" data-qmd-input=\"{name_a}\" type=\"{html_type}\"{}{}{}{}>",
                    num_attr("min"),
                    num_attr("max"),
                    num_attr("step"),
                    num_attr("value")
                )
            }
        };
        let readout = if kind == "slider" || kind == "range" {
            format!(
                "<output class=\"qmd-input-out\" data-qmd-out>{}</output>",
                html_escape(attrs.get("value").unwrap_or(""))
            )
        } else {
            String::new()
        };
        format!(
            "<div class=\"qmd-input\"{data}><label class=\"qmd-input-label\" for=\"{ctrl_id}\">{}</label>{control}{readout}</div>",
            html_escape(label)
        )
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p qmd-fast-core --lib render::tests::input_ 2>&1 | tail -20`
Expected: PASS (both render tests).

- [ ] **Step 5: Add the CSS**

In `crates/core/assets/css/base.css`, after the read-aloud block (the `@keyframes qmd-ra-in` rule added earlier), add:

```css
  /* Reactive input controls (::: {.input ...}). A labeled native control feeding the
     {js} reactive graph; native controls inherit color-scheme so dark/sepia just work. */
  .qmd-input { display: flex; align-items: center; gap: .6rem; flex-wrap: wrap;
    margin: 1rem 0; font: inherit; }
  .qmd-input-label { font-weight: 600; }
  .qmd-input-control { font: inherit; accent-color: var(--qmd-accent); }
  .qmd-input-control[type="range"] { flex: 1 1 12rem; max-width: 22rem; }
  .qmd-input-out { font-variant-numeric: tabular-nums; min-width: 2.5em;
    color: var(--qmd-muted); }
```

- [ ] **Step 6: Build + full render tests**

Run: `cargo build -p qmd-fast-core && cargo test -p qmd-fast-core --lib render 2>&1 | tail -8`
Expected: builds; all render lib tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/divs.rs crates/core/src/render/tests.rs crates/core/assets/css/base.css
git commit -m "feat(reactive): .input build_container arm — static control feeding the {js} graph"
```

---

### Task 3: Runtime registration in `qmd-js.js`

Register each static `[data-qmd-input]` control into the reactive runtime, before cells run.

**Files:**
- Modify: `crates/core/assets/js/qmd-js.js` (the `enhance` function, ~line 239)

**Interfaces:**
- Consumes: existing `rt()`, `registerInput(r, name, el)`, `readValue(el)`.
- Produces: registered inputs so `qmd.value(name)` works + the control's `input` event fires `scheduleFrom`.

- [ ] **Step 1: Add the static-input scan**

In `crates/core/assets/js/qmd-js.js`, in `enhance`, **after** `var r = rt();` and **before** the `script[type="application/qmd-js"]` cell scan, insert:

```js
    // Register declarative `::: {.input}` controls (static HTML tagged data-qmd-input) as
    // named reactive inputs, BEFORE cells run so their value is available on first run.
    // Reuses the same registerInput path as `//| viewof` cells; the change event fires the
    // existing scheduleFrom (transitive-downstream re-run). Live-swap re-registers via the
    // :not(...) guard. A sibling [data-qmd-out] (the slider readout) tracks the value.
    (root || document).querySelectorAll('[data-qmd-input]:not([data-qmd-input-bound])')
      .forEach(function (el) {
        el.setAttribute("data-qmd-input-bound", "1");
        var name = el.getAttribute("data-qmd-input");
        if (!name) return;
        registerInput(r, name, el);
        var out = el.parentNode && el.parentNode.querySelector("[data-qmd-out]");
        if (out) {
          var upd = function () { out.textContent = readValue(el); };
          el.addEventListener("input", upd);
          upd();
        }
      });
```

- [ ] **Step 2: Syntax-check + build**

Run: `node --check crates/core/assets/js/qmd-js.js && cargo build -p qmd-fast-core`
Expected: "JS OK" implicit (no error) + build succeeds.

- [ ] **Step 3: Commit**

```bash
git add crates/core/assets/js/qmd-js.js
git commit -m "feat(reactive): register .input controls into the {js} reactive runtime"
```

---

### Task 4: Corpus pin doc + browser verification

Pin the feature with a corpus doc exercising all five types + the graph, and verify the live reactive behavior in the browser.

**Files:**
- Create: `corpus/reactive/inputs.qmd`
- (verify) the whole feature via chrome-devtools

**Interfaces:** none (corpus doc + verification).

- [ ] **Step 1: Write the pin document**

Create `corpus/reactive/inputs.qmd`:

````markdown
---
title: "Reactive inputs"
---

`::: {.input}` emits a labeled control whose value is a reactive node. A `{js}` cell that
lists the name in `//| input:` re-runs when the control changes — including transitively.

::: {.input name="k" type="slider" min="1" max="10" step="1" value="3" label="k"}
:::

```{js}
//| name: doubled
//| input: k
return qmd.value("k") * 2;
```

```{js}
//| input: doubled
const p = document.createElement("p");
p.textContent = "k doubled (transitively) = " + qmd.get("doubled");
return p;
```

A number, a checkbox, a text box, and a dropdown, each driving their own cell:

::: {.input name="n" type="number" min="0" max="100" step="5" value="20"}
:::

```{js}
//| input: n
const p = document.createElement("p");
p.textContent = "n = " + qmd.value("n");
return p;
```

::: {.input name="on" type="checkbox" value="true" label="enabled"}
:::

```{js}
//| input: on
const p = document.createElement("p");
p.textContent = qmd.value("on") ? "enabled" : "disabled";
return p;
```

::: {.input name="q" type="text" value="hello" label="query"}
:::

```{js}
//| input: q
const p = document.createElement("p");
p.textContent = "query: " + qmd.value("q");
return p;
```

::: {.input name="c" type="select" options="alpha,beta,gamma" value="beta" label="channel"}
:::

```{js}
//| input: c
const p = document.createElement("p");
p.textContent = "channel: " + qmd.value("c");
return p;
```

One cell reading several inputs at once:

```{js}
//| input: k, n, on
const p = document.createElement("p");
p.textContent = `k=${qmd.value("k")} n=${qmd.value("n")} on=${qmd.value("on")}`;
return p;
```
````

- [ ] **Step 2: Corpus invariants**

Run: `cargo test -p qmd-fast-core --test corpus 2>&1 | tail -6`
Expected: PASS — the new doc renders, unique block ids, valid sourcepos, clean front-matter.

- [ ] **Step 3: Serve + browser-verify the reactive behavior (chrome-devtools MCP)**

Build the server and serve the pin doc on a real port (live `{js}` needs a server):
```bash
cargo build -p qmd-fast-server
./target/debug/qmd-fast preview corpus/reactive/inputs.qmd 4389   # run in background
```
Navigate chrome-devtools to `http://127.0.0.1:4389` and `evaluate_script` to drive + assert:

```js
// 1) slider drives its cell + the transitive downstream
var s = document.querySelector('[data-qmd-input="k"]');
s.value = "7"; s.dispatchEvent(new Event('input', {bubbles:true}));
await new Promise(r=>setTimeout(r,50));
var doubled = [...document.querySelectorAll('p')].find(p=>/doubled/.test(p.textContent));
var readout = document.querySelector('.qmd-input-out').textContent;
// 2) checkbox
var cb = document.querySelector('[data-qmd-input="on"]'); cb.checked = false; cb.dispatchEvent(new Event('input',{bubbles:true}));
await new Promise(r=>setTimeout(r,30));
var onP = [...document.querySelectorAll('p')].find(p=>/^enabled$|^disabled$/.test(p.textContent));
// 3) select
var sel = document.querySelector('[data-qmd-input="c"]'); sel.value = "gamma"; sel.dispatchEvent(new Event('input',{bubbles:true}));
await new Promise(r=>setTimeout(r,30));
var chP = [...document.querySelectorAll('p')].find(p=>/channel:/.test(p.textContent));
return { doubled: doubled && doubled.textContent, readout, on: onP && onP.textContent, channel: chP && chP.textContent };
```

Expected: `doubled` contains "14" (7×2, transitive), `readout` is "7", `on` is "disabled", `channel` is "channel: gamma". Then `list_console_messages` → 0 errors. Take a screenshot for the record.

- [ ] **Step 4: Commit**

```bash
git add corpus/reactive/inputs.qmd
git commit -m "test(reactive): pin {input} controls corpus doc (5 types + transitive graph)"
```

---

### Task 5: Docs + full verification

**Files:**
- Modify: `corpus/README.md` (note the new doc on the `reactive/` row or add one)
- Modify: `notes/backlog.md` (record shipped under Pillar III / reactive) and `notes/FEATURE-IDEAS.md` (mark #47 shipped)

**Interfaces:** none.

- [ ] **Step 1: Corpus README**

In `corpus/README.md`, update the reactive row so it covers both docs, e.g. change the `reactive/graph.qmd` row to a `reactive/` row mentioning `graph.qmd` (the `//|` graph) **and** `inputs.qmd` (`::: {.input}` controls: slider/number/checkbox/text/select feeding `{js}` cells). Keep it one line.

- [ ] **Step 2: Notes**

In `notes/backlog.md`, add a one-line shipped note (Pillar III `{input}` reactive controls: 5 types, static control + `registerInput`, pinned `corpus/reactive/inputs.qmd`). In `notes/FEATURE-IDEAS.md`, mark idea #47 (`{input}` bound to the reactive graph) as ✅ SHIPPED 2026-06-26 with a one-line note.

- [ ] **Step 3: Full verification**

Run:
```bash
cargo test -p qmd-fast-core
cargo fmt --check
node --check crates/core/assets/js/qmd-js.js
```
Expected: all tests pass (0 failed across binaries), fmt clean, JS OK.

- [ ] **Step 4: Commit**

```bash
git add corpus/README.md notes/backlog.md notes/FEATURE-IDEAS.md
git commit -m "docs(reactive): record {input} controls; mark idea #47 shipped"
```

---

## Self-Review

**Spec coverage:**
- Authoring syntax + 5 types + defaults (slider, label=name, readout slider-only) → Task 2 arm. ✓
- Static control + `data-qmd-input` + block-model attrs → Task 2. ✓
- Runtime registration before cells, reuse `registerInput`/`scheduleFrom`, slider readout, live-swap guard → Task 3. ✓
- Validation (missing name / unknown type did-you-mean / select no options), located → Task 1, wired in Task 2. ✓
- Styling, dark/sepia via color-scheme → Task 2 CSS. ✓
- Corpus pin (5 types + transitive graph) + tests (render unit, corpus, browser) → Tasks 2/4. ✓
- Invariants (HTML-only, single-surface, block model, offline, rides seams) → Global Constraints + arm/runtime design. ✓
- Docs → Task 5. ✓

**Placeholder scan:** no TBD/TODO; all code complete; the one runtime-dependent assertion (browser) has exact expected values. ✓

**Type/name consistency:** `validate_input(name, kind, options, line, file) -> Vec<Warning>` defined in Task 1, called with `attrs.get("name"/"type"/"options")` in Task 2. `INPUT_TYPES` shared. HTML contract — `.qmd-input` / `.qmd-input-label` / `.qmd-input-control` / `data-qmd-input` / `.qmd-input-out`+`data-qmd-out` — consistent across Task 2 (emits), Task 2 CSS (styles), Task 3 (`[data-qmd-input]`, `[data-qmd-out]` registers). `data-qmd-input-bound` guard only in Task 3. ✓
