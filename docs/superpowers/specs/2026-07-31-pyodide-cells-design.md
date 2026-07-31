# Opt-in Pyodide cells — design

**Backlog item 158** (`FEATURE-IDEAS.md` #66). Branch `pyodide-cells-2026-07-31`, off
`origin/main` @ `d8f92085`.

Client-side Python backed by Pyodide, feeding the same reactive graph a `{js}` cell
feeds, so a published document stays interactive with numpy and no Jupyter kernel.

---

## 0. Two facts that reframe the item

**The backlog's framing is only half right.** It says this "should land as a
`registerLanguage` call, not as surgery — the seam is `window.taliJs.registerLanguage`
(client) + `render/client_lang.rs` (server), and `{glsl}` is the worked example."

That is achievable, but **not by adding `{python}` to the registry.** The registry's
standing contract is that client-side languages and kernel languages are *disjoint by
construction* (`client_lang.rs:19-21`), pinned by `client_langs_never_reach_a_kernel`
(`crates/core/tests/client_lang.rs:58`), which asserts `client_lang("python").is_none()`.
Making `{python}` mean either thing turns both `client_lang` and `executes_to_kernel`
into page-context-dependent predicates, which is surgery across `exec.rs`, `freeze.rs`,
the figure gate, `--no-exec`, `--bare` and `reactive.rs`.

**Decision: a new `{pyodide}` fence**, disjoint from `{python}` exactly as `{glsl}` is
from `{js}`. `executes_to_kernel` is untouched; the exec/kernel/freeze zone never learns
this exists. This also adds **no config knob**, which is what `CLAUDE.md`'s "perfect the
default before adding a knob" asks for: the fence language is the selector.

**The payload, measured on 2026-07-31, not recalled.** `pyodide-core-314.0.3` unpacked,
wheel sizes by HTTP range request against the jsDelivr mirror:

| file | raw | zlib (≈ git pack) | vendored? |
|---|---:|---:|---|
| `pyodide.asm.wasm` | 9.15 MB | 3.38 MB | yes |
| `python_stdlib.zip` | 2.43 MB | 2.39 MB | yes |
| `pyodide.asm.mjs` | 1.19 MB | 0.25 MB | yes |
| `pyodide.mjs` | 0.02 MB | 0.01 MB | yes |
| `pyodide-lock.json` | 0.11 MB | 0.02 MB | yes |
| `python.exe`, `*.d.ts`, CLI shims | 1.50 MB | — | **no** (node-only ballast) |
| numpy wheel | 2.75 MB | 2.75 MB | yes |
| scipy wheel | 13.22 MB | — | **no** |
| matplotlib wheel | 6.55 MB | — | **no** |

Browser minimum **12.9 MB raw / ~6.0 MB packed**; with numpy, **~8.8 MB packed**. The
repo pack is 22.76 MB today, so this is **+39%**, not the 2x the raw number suggests —
git zlib-compresses blobs and the wasm compresses 2.7x. The backlog's "10 MB+" was the
right order of magnitude.

---

## 1. Decisions

| # | decision | why |
|---|---|---|
| 1 | **A new `{pyodide}` fence**, a third row in `CLIENT_LANGS` | keeps `executes_to_kernel` and the whole exec/freeze zone untouched; adds no config knob |
| 2 | **Vendor pyodide-core + numpy** into `crates/core/assets/pyodide/` | +8.8 MB pack; offline out of the box, zero author setup |
| 3 | **Served route in preview, `_assets/` copy in builds; single-file `build out.html` degrades to source** | the 12.9 MB has nowhere to go in a self-contained file; the `PREVIEW_MERMAID_PATH` route is the precedent |
| 4 | **numpy only** — no scipy, no matplotlib | +16 MB more for plotting Taliesin already does better; the idiomatic shape is compute in Python, draw with Plot |
| 5 | **Lazy boot on scroll-into-view** | a reader who never reaches the cell pays nothing |

### Why numpy-only is a design choice, not a budget cut

The cross-language edge the registry already proved with `{glsl}`→`{js}` is the point:

```` markdown
```{pyodide}
#| name: samples
import numpy as np
rng = np.random.default_rng(0)
rng.normal(size=500).tolist()
```

```{js}
//| input: samples
const data = tali.get("samples");
if (!data) return document.createTextNode("waiting for Python…");
return Plot.rectY(data, Plot.binX({y: "count"})).plot();
```
````

**Corrected during implementation:** an earlier draft of this example read `samples` as a
bare binding. There is no such binding. A `{js}` cell body is compiled as
`new AsyncFunction("tali", "Plot", "d3", "num", "container", "invalidation", src)`, so a
published name is reached through `tali.get(name)` (or `tali.value(name)` for an input
control), exactly as `corpus/reactive/numerics.tmd` already does. The `null` guard is
required too: the value is `null` until the runtime finishes booting.

Compute in Python, draw with Plot. Vendoring matplotlib would duplicate plotting
Taliesin already does well and would need its own canvas backend wired into the mount
path.

---

## 2. Architecture

`{pyodide}` is a third `ClientLang` row:

```rust
ClientLang { lang: "pyodide", mime: "application/tali-pyodide", class: "tali-pyodide-cell" }
```

**Seven registry consumers cost zero lines**, because they were already driven off
`CLIENT_LANGS` rather than a `js` literal:

| seam | file |
|---|---|
| `#\|` cell-option parsing | `cell_extract.rs:219` — `option_directive` already accepts `#\|` |
| `--bare` script strip | `page.rs:789` |
| `--no-exec` source fallback | `mod.rs:1105` |
| figure / float materialization gate | `mod.rs:925` |
| reactive-graph dangling-input check | `diagnostics/reactive.rs:36,89` |
| repro-box exclusion | `repro.rs:94` |
| shared-runtime asset gate | `mod.rs:1782` |

### The one place this is surgery, not registration

No existing language produces its value **after** its `run()` resolves — `{js}` and
`{glsl}` are both synchronous to the scheduler. And `tali-js.js:987` runs **every**
freshly-mounted cell in one sequential `await` loop:

```js
runSequentially(fresh.filter(...))   // for (…) await cells[i].run()
```

So a `{pyodide}` cell whose `run()` waited on a scroll-triggered boot would **stall every
cell below it on the page**, dependency edge or not — a `{js}` chart further down would
stay blank until the reader scrolled to the Python cell.

Therefore `run()` returns a placeholder immediately and the language publishes the real
value later, which needs one new method on the cell api beside `api.set`:

```js
// tali-js.js — ~6 lines
publish: function (n, v) { r.scope[n] = v; return scheduleFrom(r, n); },
```

`scheduleFrom` already exists (`tali-js.js:745`). This is deliberately **not** general
mutable dataflow, and the limit is the design: it is reachable only from a language's
`setup`, never from author code, so no `{js}` cell can start a cascade with it. It is
the one honest exception to "registration, not surgery".

### What a cell shows and what it publishes

Two separate things, and the rule for each is explicit because nothing else in the
codebase implies it:

- **Shown** — captured `stdout`/`stderr`, followed by the last statement's value
  rendered via `_repr_html_` when the object defines one and `repr()` otherwise. This is
  Jupyter's behaviour, so a `{pyodide}` cell looks like the `{python}` cell beside it.
- **Published** (`#| name:`) — the **last expression statement's value**, converted with
  Pyodide's `.toJs()`. A cell whose last statement is not an expression (an assignment, a
  `def`) publishes `null`, and `check` warns when such a cell carries a `#| name:`.

### Display and publication are decoupled

`tali-js.js:543` already reads:

```js
if (name) r.scope[name] = (node instanceof Node && na.value !== undefined) ? na.value : node;
```

That escape hatch exists for `viewof` controls and fits exactly: the pyodide cell returns
an **output node carrying the converted Python value on `.value`**, so the wrapper mounts
the display *and* publishes the value with no further change. Before the boot completes
the placeholder carries `.value = null`, so a downstream `{js}` cell sees `null` first and
the real value after — the same contract a `viewof` control's initial state already has.

---

## 3. Components

### New

| path | what |
|---|---|
| `crates/core/assets/pyodide/` | the five vendored runtime files + the numpy wheel + `LICENSE` (MPL-2.0 text; the `pyodide-core` tarball ships none) |
| `crates/core/assets/js/pyodide.js` | the client registration, modelled on `glsl.js` |
| `crates/core/src/render/pyodide.rs` | payload accessors + `pyodide_url_for(mode)`, mirroring the `PREVIEW_MERMAID_PATH` / `mermaid_url_for` pair at `mod.rs:1710-1723` |
| `corpus/reactive/pyodide.tmd` | the corpus pin, beside `corpus/reactive/glsl.tmd` |
| `crates/server/tests/pyodide_browser.rs` | the headless-Chrome gate |

### Touched

| path | change |
|---|---|
| `render/client_lang.rs` | one `ClientLang` row |
| `vocab.rs:409` | one offered-language entry |
| `render/mod.rs` | `PYODIDE_JS` const + one `gate(...)` line |
| `render/page.rs:322` | External-mode inline enhancer, mirroring the `glsl` arm |
| `assets/js/tali-js.js` | `api.publish` (~6 lines) |
| `serve/mod.rs:239`, `serve_site/mod.rs:370` | a `/_taliesin/pyodide/*` route on both dev servers |
| `build.rs` | copy the payload directory into `_assets/pyodide/` |
| `tools/gates.sh` | `CANARY_PYODIDE` |
| `THIRD_PARTY.md` | three entries (see §6) |

**The payload is a directory**, which no existing asset is — `pyodide.mjs` resolves its
siblings relative to `indexURL`. That is the one genuinely new piece of build machinery;
every other vendored asset is a single file with a hashed name.

---

## 4. Data flow

```
author     ```{pyodide}
             #| name: samples
             import numpy as np
             rng = np.random.default_rng(0)
             rng.normal(size=500).tolist()
           ```

render     <div class="cell tali-pyodide-cell">
             <div class="tali-js-out" id=…></div>
             <script type="application/tali-pyodide" data-name="samples">…</script>
           </div>
           — byte-identical wrapper shape to {js} and {glsl}

mount      run() -> placeholder node, .value = null        [resolves at once;
                                                            the page never stalls]
             └ IntersectionObserver on api.container
                 └ near viewport: import(indexURL + "pyodide.mjs")
                     └ loadPyodide({indexURL}) + loadPackage("numpy")
                         └ run source; capture stdout + last expression
                             └ replace placeholder with the output node
                             └ api.publish("samples", value.toJs())
                                 └ scheduleFrom -> downstream {js} re-runs
```

---

## 5. Failure modes

| case | behaviour |
|---|---|
| `build file.tmd out.html` | source + a render warning naming the fix (`--out <dir>`); `--strict` promotes it to an error |
| standalone deck | decks take `AssetMode::Inline` at `page.rs:13`, so a standalone deck degrades like the single file; a deck inside a site gets the route |
| payload missing / route 404 | existing `showCellError`, message names `_assets/pyodide/` |
| Python exception | Pyodide traceback into the existing error box |
| `import scipy` (or any non-vendored module) | a targeted message — "only `numpy` is vendored beside the core" — not a bare `ModuleNotFoundError`. This is *the* predictable failure of decision 4, so it gets a real message. |
| `while True:` in a cell | hangs the reader's tab. **Accepted and not a new class**: a `{js}` cell already can. Pyodide runs on the main thread; a Web Worker would lose DOM access and is out of scope. Documented, not defended against. |
| `--bare`, `--no-exec` | free, registry-driven |

---

## 6. Licensing

Checked against the licence texts on 2026-07-31 (documentary reading, not legal advice;
the evidence is recorded so it can be re-judged):

- **MPL-2.0 §1.12** names *"the GNU Affero General Public License, Version 3.0"*
  explicitly as a **Secondary License**, and **§3.3** permits distributing Covered
  Software under a Secondary License as part of a Larger Work — provided the Covered
  Software is not marked *Incompatible With Secondary Licenses*.
- Authenticated GitHub code search over `repo:pyodide/pyodide` finds that marker string
  in **exactly one file: `LICENSE`**, the boilerplate MPL template itself. No source
  file and neither vendored artifact carries an Exhibit B notice.
- **§3.4** requires notices be preserved, and `pyodide-core` ships **no** LICENSE file →
  we add `crates/core/assets/pyodide/LICENSE`, the pattern `assets/js/LICENSES.md`
  already establishes.
- NumPy's wheel carries BSD-3 plus every bundled component's licence *inside the file*,
  so redistributing it verbatim satisfies its notices automatically.

`THIRD_PARTY.md` gains: **Pyodide** (MPL-2.0), **CPython** (PSF-2.0, compiled into
`pyodide.asm.wasm` + `python_stdlib.zip`), **NumPy** (BSD-3, notices ride inside the wheel).

---

## 7. Testing

**The corpus pin is the right instrument here, and that needs saying** — a superficial
reading of item 175's lesson ("a corpus doc is the wrong instrument for anything
execution-dependent") would forbid it. That lesson is about *kernel* execution, where the
walker renders but never runs cells. For `{pyodide}` the walker's render **is** the entire
server-side contract — wrapper emission, option parsing, asset gating — so a corpus doc
pins exactly the right half. Only the browser half needs Chrome.

1. **`crates/core/tests/client_lang.rs`** — its tests already loop over `["js", "glsl"]`;
   adding `"pyodide"` re-proves the mime round-trip, never-reaches-kernel, `--bare` strip
   and the cross-language graph edge in one edit. Each verified by **mutation** (restore
   the `js`-only spelling, watch the named test fail), never by a green suite.
2. **Asset-gate independence** — a `{js}`-only page ships no pyodide enhancer; a
   `{pyodide}`-only page ships the shared runtime but *not* d3/Plot.
3. **Needle the full tag, not the word.** Every page inlines the whole JS payload, so
   `page.contains("pyodide")` is a claim about the bundle rather than the document — the
   trap that has now fired in both directions on this repo. Assertions use
   `<script type="application/tali-pyodide"`, with the reason in the doc comment.
4. **Payload completeness** — all five files plus the numpy wheel present, and the emitted
   `indexURL` resolves correctly in each of the three asset modes.
5. **`crates/server/tests/pyodide_browser.rs`**, `required-features = ["headless-js"]` —
   build a page whose `{pyodide}` cell publishes a numpy result to a downstream `{js}`
   cell, load it, scroll to trip the observer, wait on `data-tali-done`, assert the chart
   received real numbers and not `null`. The only test that proves the whole chain.
   **`CANARY_PYODIDE` must be added to `tools/gates.sh:280`** or it skips silently, which
   is precisely what that canary list exists to prevent.

Verification command:

```sh
TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python ./tools/gates.sh
```

Never piped into `tail` — a pipeline returns `tail`'s exit code. Redirect to a file and
check `$?`.

---

## 8. Out of scope

- **scipy, matplotlib, micropip.** micropip installs from PyPI at runtime, which is a
  network call the offline invariant forbids. The vendored package set is fixed.
- **Web Worker isolation.** Would fix the runaway-loop hazard but loses DOM access.
- **Converting existing `{python}` corpus docs.** `{pyodide}` is a new language, not a
  migration; every existing `{python}` cell keeps running against the warm kernel.
- **Any change to `exec.rs`, `freeze.rs`, `kernel.rs`.** If the implementation finds
  itself editing one of these, the design has been misread.

---

## 9. Open risks

1. **The directory-shaped asset** is new machinery in `build.rs` and both dev servers.
   Most likely place for a path bug that only shows up in a nested book page, where
   `asset_href` computes a relative prefix.
2. **Boot latency in the headless test.** A Pyodide boot is seconds; the Chrome gate must
   wait on `data-tali-done` rather than a fixed sleep, or it will be flaky in exactly the
   way `CHROME_PATH` contention already makes browser tests flaky.
3. **`.value` on a `<pre>`** is an own-property assignment, not a native property. Fine in
   every browser, but worth a comment so a later reader does not "fix" it.
4. **Vendoring bumps the repo pack ~39%.** A future Pyodide bump adds another ~6 MB with
   no delta compression on wasm. Bump deliberately, not routinely.
