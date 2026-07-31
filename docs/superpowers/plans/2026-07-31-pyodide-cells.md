# Opt-in Pyodide cells — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `{pyodide}` cell language that runs Python in the reader's browser via a
vendored, offline Pyodide, publishing values into the same reactive graph `{js}` cells use.

**Architecture:** A third row in `CLIENT_LANGS` (`render/client_lang.rs`), disjoint from the
kernel-backed `{python}`. Seven registry consumers are already driven off that table and cost
zero lines. The client half is a `registerLanguage` call in a new `assets/js/pyodide.js`,
modelled on `glsl.js`. The 12.9 MB runtime is vendored and delivered as a *directory* — a
served route in preview, an `_assets/` copy in site builds — never inlined.

**Tech Stack:** Rust (edition 2024), vanilla ES5-style JS (no build step), Pyodide 314.0.3
(MPL-2.0), NumPy 2.4.3 (BSD-3), chromiumoxide for the browser gate.

**Spec:** [2026-07-31-pyodide-cells-design.md](../specs/2026-07-31-pyodide-cells-design.md)

## Global Constraints

- **Never touch `exec.rs`, `freeze.rs`, `kernel.rs`, or `executes_to_kernel`.** If a task
  finds itself editing one, the design has been misread. `{pyodide}` and `{python}` are
  disjoint languages.
- **Never touch `MAX_WARM_PAGES` or the LRU order in `serve_site/exec_pool.rs`** (the one
  standing freeze).
- **The preview never writes to source.** Single editing surface.
- **No CDN, no network at render, build or read time.** Every byte is vendored.
- **Pinned versions:** Pyodide `314.0.3`, NumPy `2.4.3`. The vendored wheel filename is
  `numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl`.
- **Asset directory name is version-stamped:** `pyodide-314.0.3`. The build's content-hash
  convention (`app.ab.css`) **cannot** apply — `pyodide.mjs` resolves its siblings by fixed
  name — so the version in the path is the cache-buster.
- **Editing `assets/css/*` or `assets/js/*` needs `cargo build` before the change shows up**
  (they are `include_str!`-compiled). Rebuild the binary before rebuilding a site.
- **Verification:** `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python ./tools/gates.sh`.
  Never pipe it into `tail` — a pipeline returns `tail`'s exit code. Redirect to a file and
  check `$?`. Run the workspace suite with `-- --test-threads=1`.
- **Verify every fix by mutation** (restore the bug, watch the *named* test fail), never by a
  green suite.

---

## File Structure

### Created

| path | responsibility |
|---|---|
| `crates/core/assets/pyodide/` | the vendored runtime: `pyodide.mjs`, `pyodide.asm.mjs`, `pyodide.asm.wasm`, `python_stdlib.zip`, `pyodide-lock.json`, the numpy wheel, `LICENSE` |
| `crates/core/assets/js/pyodide.js` | the client language: registration, lazy boot, execution, display, publication |
| `crates/core/src/render/pyodide.rs` | payload accessors, `pyodide_url_for`, and the single-file degradation rewrite |
| `crates/core/tests/pyodide.rs` | server-side emission, asset gating, degradation |
| `corpus/reactive/pyodide.tmd` | the corpus pin |
| `crates/server/tests/pyodide_browser.rs` | the headless-Chrome gate |

### Modified

| path | change |
|---|---|
| `crates/core/src/render/client_lang.rs` | one `ClientLang` row |
| `crates/core/src/render/mod.rs` | `mod pyodide;` + re-exports, `PYODIDE_JS` const, one `gate(...)` line |
| `crates/core/src/render/page.rs` | the `<meta>` index-URL stamp + the External-mode enhancer arm |
| `crates/core/src/vocab.rs` | one offered-language entry |
| `crates/core/assets/js/tali-js.js` | `api.publish` (~6 lines) |
| `crates/core/tests/client_lang.rs` | extend the language loops to three |
| `crates/core/tests/third_party.rs` | `pyodide.js` into `OWN_JS`; a new attribution test for `assets/pyodide/` |
| `crates/server/src/serve/mod.rs`, `crates/server/src/serve_site/mod.rs` | the `/_taliesin/pyodide/{file}` route |
| `crates/server/src/build.rs` | copy the payload directory into `_assets/` |
| `crates/server/Cargo.toml` | `[[test]] name = "pyodide_browser"` with `required-features` |
| `tools/gates.sh` | `CANARY_PYODIDE` |
| `THIRD_PARTY.md` | three entries |
| `docs/guide/using/interactive.tmd`, `docs/internals/` | user + internals docs |
| `notes/backlog.md` | delete item 158 |

---

## Task 1: Vendor the payload with drift-locked provenance

**Files:**
- Create: `crates/core/assets/pyodide/{pyodide.mjs,pyodide.asm.mjs,pyodide.asm.wasm,python_stdlib.zip,pyodide-lock.json,numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl,LICENSE}`
- Modify: `THIRD_PARTY.md`, `crates/core/tests/third_party.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: the on-disk payload directory. Later tasks read it via `include_bytes!` /
  `include_str!` from `crates/core/src/render/pyodide.rs`.

**Why the payload is exactly these files.** Measured 2026-07-31: `pyodide-core-314.0.3`
unpacks to 15 MB, of which `python.exe` (1.43 MB), `pyodide.d.ts`, `ffi.d.ts`,
`python_cli_entry.mjs`, `python.bat`, `python` and `package.json` (1.50 MB together) are
node-CLI-only and are **not** vendored. The browser needs 12.90 MB across five files.

- [ ] **Step 1: Fetch and verify the runtime**

```bash
mkdir -p /tmp/pyo && cd /tmp/pyo
curl -sSL -o core.tar.bz2 \
  https://github.com/pyodide/pyodide/releases/download/314.0.3/pyodide-core-314.0.3.tar.bz2
tar xjf core.tar.bz2
# Expect exactly these five sizes, in bytes:
#   pyodide.asm.wasm   9596462
#   python_stdlib.zip  2545106
#   pyodide.asm.mjs    1249447
#   pyodide.mjs          17880
#   pyodide-lock.json   113804
for f in pyodide.asm.wasm python_stdlib.zip pyodide.asm.mjs pyodide.mjs pyodide-lock.json; do
  printf "%10s  %s\n" "$(stat -c%s pyodide/$f)" "$f"
done
```

- [ ] **Step 2: Fetch numpy and verify it against the lock's own sha256**

The lock records `sha256` for every wheel. Verifying against it (rather than a literal pasted
into this plan) means the check stays true if the wheel is re-fetched from a different mirror.

```bash
cd /tmp/pyo
WHL=numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl
curl -sSL -o "$WHL" "https://cdn.jsdelivr.net/pyodide/v314.0.3/full/$WHL"
python3 - <<'EOF'
import hashlib, json
lock = json.load(open('pyodide/pyodide-lock.json'))
want = [v for v in lock['packages'].values() if v['name'] == 'numpy'][0]
got = hashlib.sha256(open(want['file_name'], 'rb').read()).hexdigest()
assert got == want['sha256'], f"sha256 mismatch\n want {want['sha256']}\n got  {got}"
print("numpy wheel sha256 OK:", got)
EOF
```

Expected: `numpy wheel sha256 OK: 0cad9c1b91f0082e4f959bc0e0bf5835a2efbba6ab3b1e9d1fe6e7e564cca98e`

- [ ] **Step 3: Copy the seven files into the repo**

```bash
cd /home/bogo/Documents/personal/taliesin
mkdir -p crates/core/assets/pyodide
cd /tmp/pyo
for f in pyodide.mjs pyodide.asm.mjs pyodide.asm.wasm python_stdlib.zip pyodide-lock.json; do
  cp "pyodide/$f" /home/bogo/Documents/personal/taliesin/crates/core/assets/pyodide/
done
cp numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl \
   /home/bogo/Documents/personal/taliesin/crates/core/assets/pyodide/
curl -sSL -o /home/bogo/Documents/personal/taliesin/crates/core/assets/pyodide/LICENSE \
   https://raw.githubusercontent.com/pyodide/pyodide/main/LICENSE
```

The `LICENSE` copy is **required**, not optional: MPL-2.0 §3.4 forbids removing notices and
the `pyodide-core` tarball ships none.

- [ ] **Step 4: Write the failing provenance + attribution tests**

Append to `crates/core/tests/third_party.rs`:

```rust
/// Pyodide is vendored for `{pyodide}` cells (backlog 158): a CPython + NumPy stack compiled
/// to WebAssembly, so client-side Python runs with no kernel and no network.
///
/// The version is read from the bundle's OWN source, not asserted as a literal, so
/// re-vendoring without updating THIRD_PARTY.md goes red. Same shape as the paged.js gate
/// above, and for the same reason: a literal on both sides is one edit away from agreeing
/// with itself and nothing else.
#[test]
fn the_pyodide_version_claim_matches_the_vendored_runtime() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let loader = std::fs::read_to_string(core.join("assets/pyodide/pyodide.mjs"))
        .expect("the vendored pyodide loader should exist");
    let version = loader
        .split("314.0.3")
        .nth(1)
        .map(|_| "314.0.3")
        .expect("expected the vendored pyodide.mjs to carry its own version string");
    assert!(
        third_party_md().contains(version),
        "THIRD_PARTY.md claims a different Pyodide version than the vendored runtime \
         (runtime says `{version}`)"
    );
}

/// Every file under `assets/pyodide/` is vendored third-party code, and the directory MUST
/// carry the upstream licence text beside it — MPL-2.0 §3.4 forbids removing notices, and the
/// `pyodide-core` tarball ships no LICENSE of its own, so this is the only copy there is.
///
/// The completeness half matters as much as the attribution half: `pyodide.mjs` resolves its
/// siblings by fixed name at runtime, so a payload missing one file fails in the reader's
/// browser with a 404 and no server-side symptom at all.
#[test]
fn the_vendored_pyodide_payload_is_complete_and_carries_its_licence() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = core.join("assets/pyodide");
    for required in [
        "pyodide.mjs",
        "pyodide.asm.mjs",
        "pyodide.asm.wasm",
        "python_stdlib.zip",
        "pyodide-lock.json",
        "numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl",
        "LICENSE",
    ] {
        assert!(
            dir.join(required).is_file(),
            "the vendored Pyodide payload is missing `{required}` — the runtime resolves its \
             siblings by fixed name, so this fails only in the reader's browser"
        );
    }
    let licence = std::fs::read_to_string(dir.join("LICENSE")).expect("LICENSE readable");
    assert!(
        licence.contains("Mozilla Public License Version 2.0"),
        "assets/pyodide/LICENSE should be the MPL-2.0 text"
    );
    let doc = third_party_md();
    for claim in ["Pyodide", "NumPy", "CPython"] {
        assert!(
            doc.contains(claim),
            "THIRD_PARTY.md must attribute `{claim}` — it is redistributed inside \
             assets/pyodide/"
        );
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --test third_party`
Expected: FAIL — `the_pyodide_version_claim_matches_the_vendored_runtime` and
`the_vendored_pyodide_payload_is_complete_and_carries_its_licence` fail on the missing
THIRD_PARTY.md claims.

- [ ] **Step 6: Add the THIRD_PARTY.md entries**

Insert into the "Vendored (redistributed with this project)" list in `THIRD_PARTY.md`, after
the Observable Plot entry:

```markdown
- **Pyodide** (`crates/core/assets/pyodide/`, MPL-2.0, v314.0.3, Copyright (c) 2018-2024
  Michael Droettboom, Dexter Chua and contributors). CPython compiled to WebAssembly; the
  runtime behind `{pyodide}` cells. MPL-2.0 §1.12 names the GNU Affero General Public
  License v3.0 as a *Secondary License*, and §3.3 permits distributing Covered Software
  under one as part of a Larger Work provided it is not marked "Incompatible With Secondary
  Licenses" — Pyodide does not apply that notice to any source file (checked 2026-07-31).
  The licence text ships beside the payload at `crates/core/assets/pyodide/LICENSE`, as
  §3.4 requires. License: <https://github.com/pyodide/pyodide/blob/main/LICENSE>.
- **CPython** (PSF-2.0, v3.14.0), compiled into `pyodide.asm.wasm` and
  `python_stdlib.zip` above. License: <https://docs.python.org/3/license.html>.
- **NumPy** (`crates/core/assets/pyodide/numpy-2.4.3-*.whl`, BSD-3-Clause, v2.4.3,
  Copyright (c) 2005-2025 NumPy Developers). The one package vendored beside the Pyodide
  core; the wheel carries its own licence and every bundled component's licence inside it.
  License: <https://github.com/numpy/numpy/blob/main/LICENSE.txt>.
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core --test third_party`
Expected: PASS, all tests in the binary.

- [ ] **Step 8: Verify by mutation**

Temporarily change `v314.0.3` to `v314.0.2` in the THIRD_PARTY.md Pyodide entry.
Run: `cargo test -p taliesin-core --test third_party the_pyodide_version_claim`
Expected: FAIL. Then restore the text **by inverse edit** (never `git checkout --`, which
restores from HEAD and would delete uncommitted work).

- [ ] **Step 9: Commit**

```bash
git add crates/core/assets/pyodide THIRD_PARTY.md crates/core/tests/third_party.rs
git commit -m "feat(pyodide): vendor Pyodide 314.0.3 + NumPy with drift-locked provenance (158)"
```

---

## Task 2: Register `{pyodide}` server-side

**Files:**
- Modify: `crates/core/src/render/client_lang.rs:39-50`, `crates/core/src/vocab.rs:409`,
  `crates/core/tests/client_lang.rs`, `crates/core/tests/third_party.rs`
- Create: `crates/core/assets/js/pyodide.js` (skeleton only; Task 5 fills it in)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `client_lang("pyodide") -> Some(ClientLang { lang: "pyodide", mime:
  "application/tali-pyodide", class: "tali-pyodide-cell" })`, and
  `has_client_cells_of(body, "pyodide") -> bool`. Task 4 gates the asset on the latter.

**Why a skeleton `pyodide.js` in this task.** `every_registered_mime_is_looked_up_by_the_client_runtime`
asserts that some client file registers each registered mime. Registering server-side without
a client file would leave that test red across a commit boundary, so the registration lands
with a minimal client counterpart and Task 5 replaces its body.

- [ ] **Step 1: Write the failing tests**

In `crates/core/tests/client_lang.rs`, extend the three language loops from two entries to
three — `["js", "glsl"]` becomes `["js", "glsl", "pyodide"]` at lines 43, 59 and in
`bare_output_strips_every_client_language_not_just_js`'s table. Then append:

```rust
const PY: &str = "```{pyodide}\n#| name: xs\nimport numpy as np\nnp.arange(3).tolist()\n```\n";

/// The wrapper contract is language-agnostic, and `{pyodide}` is the first client language
/// whose comment marker is `#` rather than `//`. `option_directive` already accepts all three
/// markers (`#`, `//`, `%%`), so the reactive options parse with no parser change — this is
/// the test that says so, because a silent failure here is a cell that mounts and publishes
/// nothing.
#[test]
fn a_pyodide_cell_emits_the_shared_wrapper_with_hash_bar_options() {
    let h = render(PY).body_html();
    assert!(
        h.contains("<script type=\"application/tali-pyodide\""),
        "its own mime: {h}"
    );
    assert!(
        h.contains("class=\"cell tali-pyodide-cell\"") && h.contains("class=\"tali-js-out\""),
        "the SAME wrapper shape a `{{js}}` cell uses: {h}"
    );
    assert!(
        h.contains("data-name=\"xs\""),
        "`#| name:` parsed through the shared directive parser: {h}"
    );
    assert!(h.contains("np.arange(3)"), "author source rides verbatim: {h}");
}

/// The whole reason `{pyodide}` is a separate fence rather than a mode on `{python}`: the
/// kernel-backed language must be completely unaffected. A `{python}` cell on the same page
/// still goes to the executor, and a `{pyodide}` cell never does.
#[test]
fn pyodide_and_python_stay_disjoint_on_one_page() {
    assert!(client_lang("pyodide").is_some());
    assert!(!executes_to_kernel("pyodide"));
    assert!(client_lang("python").is_none());
    assert!(executes_to_kernel("python"));

    let both = render(&format!("{PY}\n```{{python}}\nx = 1\n```\n")).body_html();
    assert!(
        both.contains("application/tali-pyodide"),
        "the browser cell emits its wrapper: {both}"
    );
    assert!(
        !both.contains("application/tali-python"),
        "the kernel cell must NOT be wrapped as a client cell: {both}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --test client_lang`
Expected: FAIL — `client_lang("pyodide")` returns `None`, so
`a_pyodide_cell_emits_the_shared_wrapper_with_hash_bar_options` fails on the missing mime.

- [ ] **Step 3: Add the registry row**

In `crates/core/src/render/client_lang.rs`, append to `CLIENT_LANGS`:

```rust
    ClientLang {
        lang: "pyodide",
        mime: "application/tali-pyodide",
        class: "tali-pyodide-cell",
    },
```

And extend that table's doc comment, replacing the sentence about `{sql}`/`{ts}`:

```rust
/// **Deliberately short.** `{sql}`/DuckDB and `{ts}`/esbuild stay cut until a corpus
/// document needs one (each is a multi-MB vendored payload and its own licence question);
/// `{glsl}` earned its place by needing neither — WebGL is a browser API, so the whole
/// language costs one small enhancer and no vendored bytes. `{pyodide}` is the one entry
/// that DID pay the multi-MB price, which is why it is delivered as a served directory
/// rather than inlined, and why it is a separate fence from the kernel-backed `{python}`
/// rather than a mode on it: the two sets below must stay disjoint.
```

- [ ] **Step 4: Add the vocab entry**

In `crates/core/src/vocab.rs`, add `"pyodide",` immediately after `"glsl",` at line 409.

- [ ] **Step 5: Create the skeleton client file**

Create `crates/core/assets/js/pyodide.js`:

```js
// `{pyodide}` cells — Python in the reader's browser, no kernel.
//
// Filled in by Task 5. This skeleton exists so the server-side registration and its client
// counterpart land in the same commit: `every_registered_mime_is_looked_up_by_the_client_runtime`
// asserts that some client file registers each registered mime.
(function () {
  "use strict";

  /**
   * @param {string} src @param {any} api
   * @param {{name: string|null, viewof: string|null, inputs: string[], kind: string}} opts
   */
  function setupPyodide(src, api, opts) {
    var out = document.createElement("div");
    out.className = "tali-pyodide-out";
    // The shared wrapper publishes `node.value` when the node has one (tali-js.js:543).
    // `null` until a real value arrives, so a downstream `{js}` cell always has a defined
    // thing to guard on rather than an undefined name.
    out.value = null;
    return {
      run: function () {
        return out;
      },
    };
  }

  if (window.taliJs && window.taliJs.registerLanguage) {
    window.taliJs.registerLanguage("application/tali-pyodide", setupPyodide);
  }
})();
```

- [ ] **Step 6: Declare it as first-party**

In `crates/core/tests/third_party.rs`, add `"pyodide.js",` to `OWN_JS` after `"numerics.js",`
with the reason:

```rust
    // `pyodide.js` is taliesin's own enhancer — a registration against tali-js.js. The
    // VENDORED Pyodide runtime it loads lives in `assets/pyodide/` and is attributed by
    // `the_vendored_pyodide_payload_is_complete_and_carries_its_licence`.
    "pyodide.js",
```

- [ ] **Step 7: Type-check the new asset and run the tests**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors. (`jsconfig.json` sets `"strict": true` but not `noUnusedParameters`, so
the skeleton's unused `src`/`opts` params are fine — they are in the signature because Task 5
uses both.)

Run: `cargo test -p taliesin-core --test client_lang --test third_party`
Expected: PASS.

- [ ] **Step 8: Run the whole core suite for drift**

Run: `cargo test -p taliesin-core -- --test-threads=1`
Expected: PASS. A new offered language touches `vocab.rs`'s drift gates and the body-HTML
snapshots; if `body_html_snapshots` fails, read the diff before regenerating — an unexpected
change means the registration reached a page it should not have.

- [ ] **Step 9: Verify by mutation**

Remove the `ClientLang` row added in Step 3.
Run: `cargo test -p taliesin-core --test client_lang a_pyodide_cell_emits_the_shared_wrapper`
Expected: FAIL. Restore by inverse edit.

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/render/client_lang.rs crates/core/src/vocab.rs \
        crates/core/assets/js/pyodide.js crates/core/tests/client_lang.rs \
        crates/core/tests/third_party.rs
git commit -m "feat(pyodide): register {pyodide} as a client-side cell language (158)"
```

---

## Task 3: `api.publish` on the shared wrapper

> **AS BUILT (two review rounds): this landed as `hooks.publish`, NOT `api.publish`.**
> A review found that anything on `api` is author-reachable, because the `{js}`
> language passes `api` verbatim into the author `AsyncFunction` as `tali` — so a
> cell could publish to a name it also declares as an `//| input:` and create a
> feedback edge `buildGraph` never cycle-checked. A masking shield was tried and
> leaked via `Object.getPrototypeOf(tali).publish`. The capability is now a FOURTH
> argument to `setup(src, api, opts, hooks)` and is never on `api` at all.
> Read the section below with that substitution applied.

**Files:**
- Modify: `crates/core/assets/js/tali-js.js:385` (add beside `set`)
- Test: `crates/server/tests/reactive_browser.rs` (a new case on the existing harness)

**Interfaces:**
- Consumes: nothing.
- Produces: `api.publish(name: string, value: any) => Promise<void>` on the cell scope —
  writes `r.scope[name]` **and** re-runs the downstream cells of `name`. Task 5's enhancer
  calls exactly this.

**Why this exists.** `tali-js.js:987` runs every freshly-mounted cell in one sequential
`await` loop. A cell whose `run()` waits on a scroll-triggered boot would stall every cell
below it on the page — a `{js}` chart further down would stay blank until the reader scrolled
to the Python cell. So the Pyodide cell's `run()` must return at once and publish later, and
`api.set` alone schedules nothing.

- [ ] **Step 1: Add the method**

In `crates/core/assets/js/tali-js.js`, immediately after the `set` property at line 385:

```js
      /**
       * Publish a value that arrived AFTER this cell's `run()` resolved, and re-run the
       * cells that consume it. `set` alone writes the scope and schedules nothing, which is
       * correct for a synchronous cell — the wrapper publishes its return value and the
       * scheduler orders everything. A language whose value is genuinely asynchronous
       * (`{pyodide}`: boot the runtime, then execute) has no such moment.
       *
       * Deliberately NOT general mutable dataflow, and the limit is the design: this is
       * reachable only from a LANGUAGE's `setup`, never from author cell source, so no
       * `{js}` cell can start a cascade with it. The reactive-VM trap this project has
       * refused three times stays refused.
       * @param {string} n @param {any} v @returns {Promise<void>}
       */
      publish: function (n, v) { r.scope[n] = v; return scheduleFrom(r, n); },
```

- [ ] **Step 2: Type-check the assets bundle**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors. (`scheduleFrom` is declared later in the same IIFE; function
declarations hoist, so the reference is valid.)

- [ ] **Step 3: Write the failing browser assertion**

In `crates/server/tests/reactive_browser.rs`, inside the existing observation page used by the
`{glsl}` cases, this is covered end-to-end by Task 7 instead — `api.publish` has no observable
effect until a language calls it. Record that here rather than writing a vacuous test:

Add to the module doc comment of `crates/server/tests/reactive_browser.rs`:

```rust
//! `api.publish` (item 158) is exercised by `pyodide_browser.rs`, not here: it is the
//! asynchronous-value hook, and no language in this file produces one. A test asserting it
//! exists without a language driving it would pass with `scheduleFrom` deleted.
```

- [ ] **Step 4: Commit**

```bash
git add crates/core/assets/js/tali-js.js crates/server/tests/reactive_browser.rs
git commit -m "feat(pyodide): api.publish for values that arrive after run() resolves (158)"
```

---

## Task 4: Asset delivery — route, build copy, single-file degradation

**Files:**
- Create: `crates/core/src/render/pyodide.rs`, `crates/core/tests/pyodide.rs`
- Modify: `crates/core/src/render/mod.rs`, `crates/core/src/render/page.rs`,
  `crates/server/src/serve/mod.rs`, `crates/server/src/serve_site/mod.rs`,
  `crates/server/src/build.rs`

**Interfaces:**
- Consumes: `has_client_cells_of(body, "pyodide")` from Task 2; the payload from Task 1.
- Produces:
  - `pub const PREVIEW_PYODIDE_DIR: &str = "/_taliesin/pyodide-314.0.3/"`
  - `pub const PYODIDE_DIR_NAME: &str = "pyodide-314.0.3"`
  - `pub fn pyodide_payload() -> &'static [(&'static str, &'static [u8])]` — (filename, bytes)
  - `pub fn pyodide_index_meta(body: &str, mode: OutputMode, base: Option<&str>) -> String` —
    the `<meta>` tag, or `""` when the page has no `{pyodide}` cells
  - `pub fn degrade_pyodide_cells(body: &str) -> String` — rewrite each wrapper into visible
    highlighted source, for the single-file build

**The delivery rule.** The index URL depends on both mode and asset mode:

| asset mode | output mode | index URL | path |
|---|---|---|---|
| Inline | Preview | `/_taliesin/pyodide-314.0.3/` | single-doc preview, served route |
| Inline | Build | `""` | single self-contained file → **degrade** |
| External | any | `<rel>/_assets/pyodide-314.0.3/` | site build + portable folder |

- [ ] **Step 1: Write the failing tests**

Create `crates/core/tests/pyodide.rs`:

```rust
//! Delivery of the vendored Pyodide payload (backlog item 158).
//!
//! **Read this before adding an assertion here.** Every Taliesin page inlines the whole CSS
//! and JS payload, so a whole-page `contains("pyodide")` is a claim about the BUNDLE as much
//! as about the document — it passes on a page that renders no Python at all, and it fails on
//! a page whose bundled CSS merely mentions the word. Every needle below is therefore a full
//! emitted tag, never a bare word. That trap has now fired in both directions on this repo.

use taliesin_core::OutputMode;
use taliesin_core::render::{code_scripts_for, degrade_pyodide_cells, has_client_cells_of};

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, std::path::Path::new("."))
}

const PY: &str = "```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n";
const JS: &str = "```{js}\nreturn document.createElement(\"p\");\n```\n";

/// Two gates, not one: a Python page must not drag in d3 + Plot, and a chart page must not
/// ship the Pyodide enhancer. This is the assertion that would have shipped 490 KB of
/// plotting library to a page that only computes.
#[test]
fn the_pyodide_and_js_asset_gates_are_independent() {
    let py = render(PY).body_html();
    let js = render(JS).body_html();

    assert!(has_client_cells_of(&py, "pyodide"));
    assert!(!has_client_cells_of(&py, "js"), "no d3/Plot for a compute-only page");
    assert!(has_client_cells_of(&js, "js"));
    assert!(!has_client_cells_of(&js, "pyodide"), "no Pyodide enhancer for a chart page");
}

/// A `{pyodide}`-only page in a static Build ships the shared runtime AND the language
/// enhancer. `code_scripts_for` opens every gate in Preview, so only the Build arm can catch
/// a dead cell.
#[test]
fn a_build_of_a_python_page_ships_the_runtime_and_the_enhancer() {
    let scripts = code_scripts_for(&render(PY).body_html(), OutputMode::Build);
    assert!(
        scripts.contains("application/tali-pyodide"),
        "pyodide.js (which registers that mime) must ship"
    );
    assert!(
        scripts.contains("tali-js cell error:"),
        "the shared runtime must ship for a python-only page"
    );

    let prose = code_scripts_for(&render("Just prose.\n").body_html(), OutputMode::Build);
    assert!(
        !prose.contains("application/tali-pyodide"),
        "a prose page must ship neither"
    );
}

/// The single-file build is the one output path that cannot carry a 12.9 MB directory. The
/// cell degrades to VISIBLE SOURCE rather than to an empty div, which is what stripping the
/// script alone would leave: the author's code is in the `<script>`, so removing it without
/// re-emitting it silently deletes the content.
#[test]
fn a_single_file_build_degrades_a_pyodide_cell_to_visible_source() {
    let body = render(PY).body_html();
    let out = degrade_pyodide_cells(&body);
    assert!(
        !out.contains("<script type=\"application/tali-pyodide\""),
        "the runnable wrapper must be gone: {out}"
    );
    // `arange`, NOT `np.arange(3)`. Server-side highlighting splits the source into
    // `<span>`-wrapped tokens, so the multi-token literal never appears contiguously in
    // correct output — asserting it would fail on a correctly-degraded page rather than on
    // the regression this row exists to catch. Measured: `contains("np.arange(3)")` is
    // false and `contains("arange")` is true for this exact source.
    assert!(
        out.contains("arange"),
        "the author's source must remain VISIBLE, not just deleted: {out}"
    );
    assert!(
        out.contains("<pre><code class=\"language-python\">"),
        "and it must be marked up as a python listing, the same shape emit.rs uses: {out}"
    );
}

/// The degradation must leave every OTHER client language alone — it is keyed on one mime,
/// and a `{js}` cell in the same document still runs in a single-file build.
#[test]
fn the_degradation_leaves_js_cells_running() {
    let body = render(&format!("{PY}\n{JS}")).body_html();
    let out = degrade_pyodide_cells(&body);
    assert!(
        out.contains("<script type=\"application/tali-js\""),
        "a `{{js}}` cell must survive a single-file build untouched: {out}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --test pyodide`
Expected: FAIL to compile — `degrade_pyodide_cells` is not defined.

- [ ] **Step 3: Create `crates/core/src/render/pyodide.rs`**

```rust
//! Delivery of the vendored Pyodide runtime for `{pyodide}` cells.
//!
//! **Why this is a directory and not a hashed blob.** Every other vendored asset is a single
//! file the build renames to `app.<hash>.css`. Pyodide cannot be: `pyodide.mjs` resolves
//! `pyodide.asm.mjs`, `pyodide.asm.wasm` and `python_stdlib.zip` by FIXED name relative to
//! its `indexURL`, and `pyodide-lock.json` names the wheel. Renaming any of them breaks the
//! runtime at load. The version therefore lives in the DIRECTORY name, which is what makes
//! it cache-safe across a Pyodide bump.

use crate::OutputMode;

/// The version-stamped directory name, shared by the preview route and the build's
/// `_assets/`. Bumping Pyodide means bumping this, which busts every reader's cache.
pub const PYODIDE_DIR_NAME: &str = "pyodide-314.0.3";

/// Same-origin path both dev servers serve the vendored runtime from. A route rather than an
/// inline blob for the same reason `PREVIEW_MERMAID_PATH` is one, only more so: the page
/// shell is re-served on every navigation, and this payload is 12.9 MB.
pub const PREVIEW_PYODIDE_DIR: &str = "/_taliesin/pyodide-314.0.3/";

macro_rules! payload_file {
    ($name:literal) => {
        ($name, include_bytes!(concat!("../../assets/pyodide/", $name)) as &[u8])
    };
}

/// The vendored payload as (filename, bytes), for the dev servers to route and the build to
/// copy. `LICENSE` rides along: MPL-2.0 §3.4 forbids removing notices, so the licence travels
/// with the bytes into every built site, not just the source tree.
pub fn pyodide_payload() -> &'static [(&'static str, &'static [u8])] {
    &[
        payload_file!("pyodide.mjs"),
        payload_file!("pyodide.asm.mjs"),
        payload_file!("pyodide.asm.wasm"),
        payload_file!("python_stdlib.zip"),
        payload_file!("pyodide-lock.json"),
        payload_file!("numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl"),
        payload_file!("LICENSE"),
    ]
}

/// The `<meta>` the enhancer reads its `indexURL` from, or `""` when the page has no
/// `{pyodide}` cells. One tag serves all three asset modes, so `pyodide.js` needs no
/// knowledge of how the page was built.
///
/// `base` is `Some(rel)` only in `AssetMode::External`, where it is the page-relative prefix
/// the build already computes for every other asset (`asset_href`). An empty return in Build
/// + Inline is the single-file path, and is the signal to [`degrade_pyodide_cells`].
pub fn pyodide_index_meta(body: &str, mode: OutputMode, base: Option<&str>) -> String {
    if !crate::render::has_client_cells_of(body, "pyodide") {
        return String::new();
    }
    let url = match base {
        Some(rel) => format!("{rel}_assets/{PYODIDE_DIR_NAME}/"),
        None if mode == OutputMode::Preview => PREVIEW_PYODIDE_DIR.to_string(),
        None => return String::new(),
    };
    format!("<meta name=\"tali-pyodide-index\" content=\"{url}\">")
}

/// Rewrite every `{pyodide}` wrapper into visible highlighted source, for the one output path
/// that cannot carry the runtime: `build <file.tmd> out.html`.
///
/// **Re-emitting the source is the whole job.** The author's code lives inside the
/// `<script type="application/tali-pyodide">`, so stripping the script (what `--bare` does)
/// would leave an empty `<div>` and silently delete the content the reader came for.
pub fn degrade_pyodide_cells(body: &str) -> String {
    let spec = match crate::render::client_lang("pyodide") {
        Some(s) => s,
        None => return body.to_string(),
    };
    let open = format!("<script type=\"{}\"", spec.mime);
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find(&open) {
        let (before, tail) = rest.split_at(i);
        out.push_str(before);
        let Some(gt) = tail.find('>') else {
            out.push_str(tail);
            return out;
        };
        let after_open = &tail[gt + 1..];
        let Some(end) = after_open.find("</script>") else {
            out.push_str(tail);
            return out;
        };
        let src = after_open[..end].replace("<\\/script", "</script");
        // The same shape `emit.rs` produces for a listing, so the degraded cell is
        // indistinguishable from an ordinary ```python block: `highlight` returns the
        // token spans only, and the caller supplies the `<pre><code class=…>` frame.
        out.push_str(&format!(
            "<pre><code class=\"language-python\">{}</code></pre>",
            crate::highlight::highlight(&src, Some("python"))
        ));
        rest = &after_open[end + "</script>".len()..];
    }
    out.push_str(rest);
    out
}
```

**Signature verified 2026-07-31:** `crates/core/src/highlight.rs:129` is
`pub fn highlight(code: &str, lang: Option<&str>) -> String` and returns the token spans
only; `emit.rs:59-60,103` supplies the `<pre><code class="language-{lang}">` frame around
it. The code above matches that split exactly. Do not invent a `highlight_block` helper.

- [ ] **Step 4: Wire the module and the asset gate**

In `crates/core/src/render/mod.rs`:

```rust
mod pyodide;
pub use pyodide::{
    PREVIEW_PYODIDE_DIR, PYODIDE_DIR_NAME, degrade_pyodide_cells, pyodide_index_meta,
    pyodide_payload,
};
```

**And re-export at the crate root**, or the dev-server handlers in Step 7 will not compile.
`crates/core/src/lib.rs:52-57` carries one `pub use render::{…}` block; `PREVIEW_MERMAID_PATH`
and `mermaid_min_js` are already in it, which is the precedent. Add to that same block, in its
existing alphabetical order:

```rust
    PREVIEW_PYODIDE_DIR, PYODIDE_DIR_NAME, degrade_pyodide_cells, pyodide_payload,
```

`pyodide_index_meta` is used only inside `page.rs` and does **not** need a crate-root export.

Add the enhancer const beside `GLSL_JS` (line 1835):

```rust
/// `{pyodide}` cells: boots the vendored Pyodide runtime lazily and runs Python in the
/// reader's browser. Registers into `tali-js.js`'s language registry, so it is gated on
/// `{pyodide}` cells being present rather than shipping with every `{js}` page.
const PYODIDE_JS: &str = include_str!("../../assets/js/pyodide.js");
```

And one gate line in `code_scripts_for` (line 1783), after `glsl_s`:

```rust
        pyodide_s = gate(has_client_cells_of(body, "pyodide"), PYODIDE_JS),
```

adding `{pyodide_s}` to the format string after `{glsl_s}`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core --test pyodide`
Expected: PASS.

- [ ] **Step 6: Stamp the meta tag in both asset modes**

In `crates/core/src/render/page.rs`, in the `AssetMode::Inline` arm (~line 265) and the
`AssetMode::External` arm (~line 296), compute the meta and add it to the head block. Inline
passes `None` for `base`; External passes the same relative prefix `a.app_js` is built from.
Add the External-mode enhancer beside the `glsl` arm at line 323:

```rust
                let pyodide = if has_client_cells_of(p.body, "pyodide") {
                    format!("\n<script>{PYODIDE_JS}</script>")
                } else {
                    String::new()
                };
```

appending `{pyodide}` to that arm's `format!`.

- [ ] **Step 7: Add the dev-server routes**

In `crates/server/src/serve/mod.rs` beside line 239, and identically in
`crates/server/src/serve_site/mod.rs` beside line 370:

```rust
        .route(
            &format!("{}{{file}}", taliesin_core::PREVIEW_PYODIDE_DIR),
            get(pyodide_asset),
        )
```

and the handler, modelled on `mermaid_lib_js` (`serve/mod.rs:664`):

```rust
/// Serve one file of the vendored Pyodide payload. Immutable caching is safe because the
/// version is in the PATH: a Pyodide bump changes the directory name, so a reader never sees
/// a stale mix of old wasm and new loader.
async fn pyodide_asset(
    axum::extract::Path(file): axum::extract::Path<String>,
) -> impl IntoResponse {
    let Some((_, bytes)) = taliesin_core::pyodide_payload()
        .iter()
        .find(|(name, _)| *name == file)
    else {
        return (axum::http::StatusCode::NOT_FOUND, [], Vec::new()).into_response();
    };
    let ct = match () {
        _ if file.ends_with(".wasm") => "application/wasm",
        _ if file.ends_with(".mjs") => "text/javascript; charset=utf-8",
        _ if file.ends_with(".json") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    };
    (
        [
            (axum::http::header::CONTENT_TYPE, ct),
            (
                axum::http::header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            ),
        ],
        bytes.to_vec(),
    )
        .into_response()
}
```

**`application/wasm` is load-bearing** — `WebAssembly.instantiateStreaming` rejects any other
content type, and the failure surfaces in the browser as an opaque instantiate error.

- [ ] **Step 8: Copy the payload in the site build**

In `crates/server/src/build.rs`, where `_assets/` is written (~line 1729), add: when any built
page's body has `{pyodide}` cells, create `_assets/<PYODIDE_DIR_NAME>/` and write each
`pyodide_payload()` entry into it verbatim (no hashing — see the module doc).

- [ ] **Step 9: Run the full suite**

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS. `asset_bundle.rs` and `build_reproducibility.rs` are the likely complainers;
read any diff rather than regenerating.

- [ ] **Step 10: Verify by mutation**

Change the handler's `application/wasm` to `application/octet-stream`, rebuild, and confirm
Task 7's browser test fails once it exists. Until then, delete the `pyodide_s` gate line from
Step 4 and confirm `a_build_of_a_python_page_ships_the_runtime_and_the_enhancer` fails.
Restore by inverse edit.

- [ ] **Step 11: Commit**

```bash
git add crates/core/src/render/pyodide.rs crates/core/src/render/mod.rs \
        crates/core/src/render/page.rs crates/core/tests/pyodide.rs \
        crates/server/src/serve/mod.rs crates/server/src/serve_site/mod.rs \
        crates/server/src/build.rs
git commit -m "feat(pyodide): serve and copy the runtime; degrade the single-file build (158)"
```

---

## Task 5: The client enhancer

**Files:**
- Modify: `crates/core/assets/js/pyodide.js` (replace the Task 2 skeleton)

**Interfaces:**
- Consumes: `hooks.publish` (Task 3 — the FOURTH argument to `setup`, not a method on
  `api`; the review loop moved it off the api object so author cell source cannot reach it),
  `<meta name="tali-pyodide-index">` (Task 4),
  `window.taliJs.registerLanguage` (existing).
- Produces: a working `{pyodide}` cell. Task 7 asserts against it in a real browser.

- [ ] **Step 1: Replace the skeleton body**

Replace the whole of `crates/core/assets/js/pyodide.js`:

```js
// `{pyodide}` cells — Python in the reader's browser, no kernel.
//
// This file is the whole of the `{pyodide}` language: a REGISTRATION against the seam
// `tali-js.js` exposes (`window.taliJs.registerLanguage`), not a second runtime. Mounting,
// `#| name:` publication, the dependency graph, the error box, teardown and click-to-source
// are the shared wrapper's job and appear nowhere below.
//
// TWO THINGS HERE ARE LOAD-BEARING AND NON-OBVIOUS.
//
// 1. `run()` MUST NOT await the boot. `tali-js.js` runs every freshly-mounted cell in ONE
//    sequential `await` loop, so a cell that blocked on a scroll-triggered download would
//    stall every cell below it on the page — a `{js}` chart further down would stay blank
//    until the reader scrolled here. `run()` therefore returns a placeholder at once and the
//    real value is published later through `hooks.publish`, which re-runs the consumers.
//    `hooks` is `setup`'s FOURTH argument and is language-only: it is deliberately NOT on
//    `api`, because `api` is handed verbatim to `{js}` author source as `tali`. Do not copy
//    it onto `api`, `out`, or anything else author source can reach.
//
// 2. The output node carries the value on `.value`. `tali-js.js` publishes
//    `node.value` when a returned Node has one, so ONE returned node both mounts the display
//    and publishes the value — no wrapper change needed. It is `null` until the first real
//    result, so a downstream `{js}` cell always has a defined thing to guard on.
(function () {
  "use strict";

  /** The build/serve-time index URL, stamped into the head by render/pyodide.rs. */
  function indexUrl() {
    var m = document.querySelector('meta[name="tali-pyodide-index"]');
    return (m && m.getAttribute("content")) || "";
  }

  /** One boot per page, shared by every cell. @type {Promise<any> | null} */
  var booting = null;

  function boot() {
    if (booting) return booting;
    var base = indexUrl();
    if (!base) {
      booting = Promise.reject(
        new Error(
          "pyodide: this page was built as a single self-contained file, which cannot " +
            "carry the 12.9 MB runtime. Rebuild with `--out <dir>` for a working page."
        )
      );
      return booting;
    }
    // A dynamic `import()` in an INLINE script resolves relative to the page, which is what
    // makes a page-relative `_assets/...` index work in a nested book chapter. See the note
    // in render/page.rs about why this runtime stays inline even in External asset mode.
    booting = import(base + "pyodide.mjs")
      .then(function (mod) {
        return mod.loadPyodide({ indexURL: base });
      })
      .then(function (py) {
        return py.loadPackage("numpy").then(function () {
          return py;
        });
      });
    return booting;
  }

  /** Run `cb` once `el` is near the viewport. Returns a disposer. */
  function whenNear(el, cb) {
    if (typeof IntersectionObserver !== "function") {
      cb();
      return function () {};
    }
    // 600px of lead time: the runtime starts fetching while the reader is still reading the
    // paragraph above, so it is usually running by the time the cell is actually on screen.
    var io = new IntersectionObserver(
      function (entries) {
        for (var i = 0; i < entries.length; i++) {
          if (entries[i].isIntersecting) {
            io.disconnect();
            cb();
            return;
          }
        }
      },
      { rootMargin: "600px" }
    );
    io.observe(el);
    return function () { io.disconnect(); };
  }

  /** @param {string} text @param {string} cls */
  function note(text, cls) {
    var p = document.createElement("p");
    p.className = cls;
    p.textContent = text;
    return p;
  }

  /**
   * Turn a failure into the message the reader can act on. A bare ModuleNotFoundError is the
   * ONE predictable failure of vendoring numpy and nothing else, so it does not get to
   * surface as a raw traceback.
   * @param {any} e
   */
  function explain(e) {
    var msg = (e && e.message) || String(e);
    if (msg.indexOf("ModuleNotFoundError") >= 0) {
      return (
        msg +
        "\n\nOnly the Python standard library and numpy are vendored with this page. " +
        "Installing another package would need a network fetch, which Taliesin does not do."
      );
    }
    return msg;
  }

  /**
   * @param {string} src @param {any} api
   * @param {{name: string|null, viewof: string|null, inputs: string[], kind: string}} opts
   * @param {{publish: (n: string, v: any) => Promise<void>}} hooks
   */
  function setupPyodide(src, api, opts, hooks) {
    var out = document.createElement("div");
    out.className = "tali-pyodide-out";
    // `.value` is an own-property assignment on a plain <div>, not a native property; the
    // codebase's idiom for that under `tsc --strict` is an inline `/** @type {any} */` cast
    // (precedent: tali-js.js and deck.js). See note 2 in the header.
    /** @type {any} */ (out).value = null;
    var stop = null;
    var dead = false;
    var started = false;

    function show(node) {
      if (dead) return;
      out.replaceChildren(node);
    }

    async function execute() {
      if (dead) return;
      show(note("Starting Python…", "tali-pyodide-status"));
      var chunks = [];
      var result = null;
      try {
        var py = await boot();
        if (dead) return;
        py.setStdout({ batched: function (s) { chunks.push(s); } });
        py.setStderr({ batched: function (s) { chunks.push(s); } });
        result = await py.runPythonAsync(src);
        if (dead) return;

        var frag = document.createDocumentFragment();
        if (chunks.length) {
          var pre = document.createElement("pre");
          pre.className = "tali-pyodide-stdout";
          pre.textContent = chunks.join("");
          frag.appendChild(pre);
        }
        // Rich display first, `repr` second — Jupyter's order, so a `{pyodide}` cell looks
        // like the `{python}` cell beside it.
        if (result && typeof result._repr_html_ === "function") {
          var host = document.createElement("div");
          host.innerHTML = result._repr_html_();
          frag.appendChild(host);
        } else if (result !== undefined && result !== null) {
          var v = document.createElement("pre");
          v.className = "tali-pyodide-value";
          v.textContent = String(result.toString ? result.toString() : result);
          frag.appendChild(v);
        }
        show(frag);

        if (opts.name) {
          var js = result && typeof result.toJs === "function"
            ? result.toJs({ dict_converter: Object.fromEntries })
            : result;
          /** @type {any} */ (out).value = js;
          await hooks.publish(opts.name, js);
        }
      } catch (e) {
        if (dead) return;
        var box = document.createElement("pre");
        box.className = "tali-js-error";
        box.textContent = explain(e);
        show(box);
      } finally {
        // PyProxies are not garbage collected: a chapter re-run twenty times would otherwise
        // leak the WASM heap until the tab dies.
        if (result && typeof result.destroy === "function") {
          try { result.destroy(); } catch (_) { /* already destroyed */ }
        }
      }
    }

    return {
      run: function () {
        if (out.parentNode !== api.container) {
          show(note("Python runs when this scrolls into view.", "tali-pyodide-status"));
        }
        if (!started) {
          started = true;
          stop = whenNear(api.container, function () { execute(); });
        } else {
          // A re-run: an input this cell consumes changed. Fire and FORGET — awaiting here
          // would reintroduce the stall note 1 exists to prevent. Downstream consumers see
          // the previous value on this tick and the fresh one when `hooks.publish` lands.
          execute();
        }
        return out;
      },
      dispose: function () {
        dead = true;
        if (stop) stop();
      },
    };
  }

  if (window.taliJs && window.taliJs.registerLanguage) {
    window.taliJs.registerLanguage("application/tali-pyodide", setupPyodide);
  }
})();
```

- [ ] **Step 2: Type-check**

Run: `cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json`
Expected: no errors. If `import()` in a classic script trips the config, add the minimal
`globals.d.ts` declaration rather than loosening the project's strictness.

- [ ] **Step 3: Add the four style tokens**

In `crates/core/assets/css/base.css`, add rules for `.tali-pyodide-out`,
`.tali-pyodide-status`, `.tali-pyodide-stdout` and `.tali-pyodide-value`. Use **only**
existing `--tali-*` tokens — an invented token renders nothing and there is a test that says
so. Mirror the existing `.tali-js-error` rule for spacing and family.

**Do not add a comment naming any string that another test forbids page-wide.** A comment in
`base.css` ships its literal into every page; that has already broken a negative test once.

- [ ] **Step 4: Rebuild the binary**

Run: `cargo build`
Expected: success. The JS and CSS are `include_str!`-compiled, so nothing downstream sees the
change until this runs.

- [ ] **Step 5: Commit**

```bash
git add crates/core/assets/js/pyodide.js crates/core/assets/css/base.css
git commit -m "feat(pyodide): lazy-booting client enhancer with stdout + value publication (158)"
```

---

## Task 6: The corpus pin and the docs

**Files:**
- Create: `corpus/reactive/pyodide.tmd`
- Modify: `corpus/README.md`, `docs/guide/using/interactive.tmd`, `notes/backlog.md`

**Interfaces:**
- Consumes: everything from Tasks 1-5.
- Produces: the page Task 7's browser test builds and loads.

**Why a corpus doc is right here**, when item 175 says corpus docs are the wrong instrument
for execution-dependent work: that lesson is about *kernel* execution, where the walker
renders but never runs cells. For `{pyodide}` the walker's render **is** the entire
server-side contract — wrapper emission, option parsing, asset gating. Only the browser half
needs Chrome.

- [ ] **Step 1: Write the pin**

Create `corpus/reactive/pyodide.tmd`:

````markdown
---
title: "Python in the browser"
---

A `{pyodide}` cell runs Python in the reader's browser. It is a *client-side cell language*,
the same kind of thing a `{js}` or `{glsl}` cell is: the browser is its kernel, so nothing is
executed at build time and no Jupyter kernel is involved. It is a different language from
`{python}`, which runs against a warm kernel at build time and is unaffected by anything here.

The runtime is vendored and loads on first scroll into view, so a reader who never reaches
this section downloads none of it.

```{pyodide}
#| name: samples
import numpy as np
rng = np.random.default_rng(0)
rng.normal(size=500).tolist()
```

The cell's last expression is its published value. A `{js}` cell consumes it through the same
reactive graph a slider or a `{glsl}` shader publishes into — one graph, not one per language:

```{js}
//| input: samples
if (!samples) return document.createTextNode("waiting for Python…");
return Plot.rectY(samples, Plot.binX({ y: "count" }, { x: (d) => d })).plot({ height: 200 });
```

`print` output is shown above the value, exactly as it is in a `{python}` cell:

```{pyodide}
import numpy as np
a = np.arange(6).reshape(2, 3)
print("shape:", a.shape)
a.sum()
```

Only the standard library and numpy are available: installing anything else would need a
network fetch, which Taliesin does not do.
````

- [ ] **Step 2: Run the corpus walker**

Run: `cargo test -p taliesin-core -- --test-threads=1`
Expected: PASS. The walker renders every corpus doc, so this proves the emission path on a
real document. If `corpus.rs`'s block-model invariants fail, the wrapper is missing a
`data-block-id` or `data-sourcepos` — read the failure, do not regenerate.

- [ ] **Step 3: Document it in corpus/README.md**

Add a line to the `reactive/` section naming `pyodide.tmd` and what it exercises: the
`{pyodide}` client language, the cross-language reactive edge into `{js}`, and stdout display.

- [ ] **Step 4: Document it for users**

In `docs/guide/using/interactive.tmd`, add a `{pyodide}` section beside the `{glsl}` one:
what the fence is, that it is not `{python}`, the lazy-load behaviour, `#| name:` publishing
the last expression, that only stdlib + numpy are available, and that a single-file
`build out.html` shows the source instead of running it.

- [ ] **Step 5: Preview it in a real browser**

```bash
cargo run -p taliesin-server -- preview corpus/reactive/pyodide.tmd 4388
```

Then drive the chrome-devtools MCP: screenshot the page, scroll to the cell, confirm the
histogram renders and the console is clean. **This is the first moment anything proves the
feature works** — every test before now asserted what Rust emitted.

- [ ] **Step 6: Commit**

```bash
git add corpus/reactive/pyodide.tmd corpus/README.md docs/guide/using/interactive.tmd
git commit -m "docs(pyodide): corpus pin and user guide (158)"
```

**The backlog item is NOT closed here.** Task 7's browser gate is the only end-to-end proof
the feature works; closing 158 before it is green would record "done" on the strength of
tests that only assert what Rust emitted. The deletion is Task 7 Step 9.

---

## Task 7: The headless-Chrome gate

**Files:**
- Create: `crates/server/tests/pyodide_browser.rs`
- Modify: `crates/server/Cargo.toml`, `tools/gates.sh`

**Interfaces:**
- Consumes: everything. This is the only test that proves the whole chain.
- Produces: `CANARY_PYODIDE = "a_pyodide_cell_boots_and_publishes_to_a_js_consumer"`.

**The `file://` constraint, which is the trap in this task.** `reactive_browser.rs` loads its
page as a `file://` document. Pyodide **cannot** work there: Chrome blocks `fetch()` and ES
module imports for `file://` origins, and Pyodide must fetch its wasm and stdlib. This test
must therefore drive a real HTTP origin. Spawn `taliesin preview <dir> <port>` — the harness
in `crates/server/tests/preview_single_instance.rs` already does exactly this, including port
slotting so concurrent tests do not collide, and it costs no new dependency. Using the preview
server also exercises the served route from Task 4, so one test covers route, boot and publish.

- [ ] **Step 1: Declare the test binary**

In `crates/server/Cargo.toml`, after the `reactive_browser` entry:

```toml
[[test]]
name = "pyodide_browser"
required-features = ["headless-js"]
```

- [ ] **Step 2: Write the failing test**

Create `crates/server/tests/pyodide_browser.rs`. Reuse `which_chrome` / `have_chrome` from
`reactive_browser.rs:38-60` verbatim (same gate: no Chrome → skip, unless
`TALIESIN_REQUIRE_CHROME=1` makes the skip a hard failure), and the preview-spawning helper
shape from `preview_single_instance.rs:110-135`.

```rust
//! The `{pyodide}` browser gate (backlog item 158).
//!
//! **Why HTTP and not `file://` like reactive_browser.rs.** Chrome blocks `fetch()` and ES
//! module imports for `file://` origins, and Pyodide must fetch `pyodide.asm.wasm` and
//! `python_stdlib.zip` to start at all. A `file://` version of this test would fail with an
//! opaque module error that reads exactly like a code defect. So this drives a real
//! `taliesin preview`, which also exercises the served `/_taliesin/pyodide-*/` route.
//!
//! **Why one test and not five.** Everything Rust emits is already asserted in
//! `crates/core/tests/pyodide.rs`. What no Rust test can see is whether the runtime actually
//! booted, whether the last expression became the published value, and whether the downstream
//! `{js}` cell re-ran when `hooks.publish` landed. That chain is one claim, so it is one test.

/// The canary. Pinned by name in `tools/gates.sh`; renaming it without updating that file
/// silently removes the only proof this feature is exercised at all.
#[test]
fn a_pyodide_cell_boots_and_publishes_to_a_js_consumer() {
    let Some(chrome) = which_chrome() else {
        skip_or_fail("no system Chrome");
        return;
    };
    let server = Preview::spawn(Path::new("corpus/reactive"), pick_port());

    // Poll for the downstream `{js}` cell's chart, NOT for a fixed sleep. A Pyodide boot is
    // seconds and varies with disk cache, so a sleep is either flaky or wastefully long.
    // Budget generously: this asserts "it eventually works", never "it is fast".
    let observed = drive(&chrome, &server.url("pyodide.html"), |page| async move {
        page.wait_for_selector_with_timeout(
            "[data-tali-done] ~ * svg, .tali-pyodide-stdout",
            Duration::from_secs(120),
        )
        .await
    });

    // Known-positive first: without it every assertion below could pass on a probe that had
    // simply stopped reading the page.
    assert!(
        observed.stdout_text.contains("shape: (2, 3)"),
        "the second cell's `print` output must reach the page — if this is empty the runtime \
         never booted, and the publish assertions below mean nothing (console: {:?})",
        observed.console
    );
    assert!(
        observed.published_len == 500,
        "the first cell's last expression must become the PUBLISHED value: the downstream \
         `{{js}}` cell saw {} samples, not 500. A 0 here means `hooks.publish` never re-ran the \
         consumer; a null means the wrapper published the placeholder instead of the value.",
        observed.published_len
    );
    assert!(
        observed.chart_paths > 0,
        "the downstream `{{js}}` cell must have RE-RUN after the publish and drawn the \
         histogram ({} <rect>/<path> nodes found)",
        observed.chart_paths
    );
    assert!(
        observed.console.iter().all(|m| !m.contains("tali-js cell error")),
        "no cell may have errored: {:?}",
        observed.console
    );
}
```

The `drive` helper reads, in one `evaluate_script` after the wait:

```js
({
  stdout_text: [...document.querySelectorAll(".tali-pyodide-stdout")]
    .map((e) => e.textContent).join("\n"),
  published_len: (() => {
    const r = window.__talijs;
    return r && Array.isArray(r.scope.samples) ? r.scope.samples.length : -1;
  })(),
  chart_paths: document.querySelectorAll("svg rect, svg path").length,
})
```

- [ ] **Step 3: Run it to verify it fails**

First confirm it fails for the *right* reason by pointing it at a page with no `{pyodide}`
cell (temporarily `corpus/reactive/glsl.tmd`).

Run: `cargo test -p taliesin-server --features headless-js --test pyodide_browser -- --nocapture`
Expected: FAIL on the known-positive stdout assertion, not on a panic in the harness.
Then restore the page name.

- [ ] **Step 4: Run it against the real page**

Run: `cargo test -p taliesin-server --features headless-js --test pyodide_browser -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Add the gates.sh canary**

In `tools/gates.sh`, after `CANARY_PRINT`:

```sh
# A fifth browser-backed capability, independent of the other four: `{pyodide}`. It is the
# only thing that boots the vendored Pyodide runtime at all — every other test of item 158
# asserts what Rust EMITTED, and would stay green with the runtime payload deleted.
CANARY_PYODIDE="a_pyodide_cell_boots_and_publishes_to_a_js_consumer"
```

and add `"chrome (pyodide):$CANARY_PYODIDE" \` to the canary `for` loop at line 280.

- [ ] **Step 6: Verify the canary actually gates**

Rename the test function temporarily.
Run: `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python ./tools/gates.sh > /tmp/g.log 2>&1; echo "exit=$?"`
Expected: non-zero exit, with the log naming the missing `chrome (pyodide)` canary.
**Do not pipe into `tail`** — a pipeline returns `tail`'s status. Restore the name by inverse
edit.

- [ ] **Step 7: Run the full gate**

Run: `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python ./tools/gates.sh > /tmp/gates.log 2>&1; echo "exit=$?"`
Expected: `exit=0`, and all **eight** canaries reporting ok by name in `/tmp/gates.log`.

- [ ] **Step 8: Commit**

```bash
git add crates/server/tests/pyodide_browser.rs crates/server/Cargo.toml tools/gates.sh
git commit -m "test(pyodide): headless-Chrome gate over boot, publish and re-run (158)"
```

- [ ] **Step 9: Close the backlog item**

Only now, with the end-to-end gate green, delete item 158 from `notes/backlog.md`'s P1 list
(never leave a `[x]`) and update the "Now" section's pointer to the new top of P1.

```bash
git add notes/backlog.md
git commit -m "docs(notes): close item 158 — {pyodide} cells shipped and gated (158)"
```

---

## Self-review notes

**Spec coverage.** Every spec section maps to a task: §1 decisions 1/4 → Task 2; decision 2 →
Task 1; decisions 3/5 → Tasks 4/5; §2 architecture → Tasks 2-4; §2 `api.publish` → Task 3;
§2 display/publication → Task 5; §3 components → all; §4 data flow → Task 5; §5 failure modes
→ Tasks 4 (single-file, deck, missing payload) and 5 (traceback, ModuleNotFoundError,
runaway loop documented); §6 licensing → Task 1; §7 testing → Tasks 2, 4, 6, 7; §8 out of
scope → Global Constraints; §9 risks → called out in the tasks that carry them (directory
asset in Task 4 Step 8, boot latency in Task 7 Step 2, `.value` in Task 5's header comment,
repo growth in Task 1).

**Two places the plan deliberately does not pretend to certainty**, flagged inline rather
than hidden:
- `highlight_block` in Task 4 Step 3 is the *intent*, not a verified signature; the step says
  to check `highlight.rs` first and gives the fallback.
- Task 4 Step 6 and Step 8 describe where to stamp the meta tag and copy the directory but do
  not quote exact surrounding lines, because `page.rs`'s head assembly and `build.rs`'s asset
  writer are both long `format!` blocks whose current shape should be read at edit time.
