# Detection debt — a live register

**What this is.** One row per class of failure that would **ship silently**: not "is it broken"
but "if it broke, would anything tell us?" `D` is a detection score on R7's 1-10 scale, where
**10 = nothing anywhere would catch it** and low numbers mean a named test fails.

**Why it exists as a file.** R7 was the *third* round to assemble this list from scratch
(`LESSONS.md` names four classes in prose, `AUDITS.md` names the hand-run gates repeatedly, R14
built the deck exemption register). None was queryable, so each round paid to rebuild it. The
proof the "what would change this" column is real: the broken-SVG class went **D=10 → D=3** the
day `crates/core/tests/svg_assets_render.rs` landed, and that improvement was recorded nowhere a
later round would find it.

**This is a live file, not a dated findings doc.** Edit a row in place when its score changes,
and say what changed it. **Do not** add a dated header, do not append a new table, and do not
copy this into a findings doc — a second copy is the failure this file exists to end.

**Explicitly not proposed:** a coverage tool, a dashboard, or telemetry. Telemetry would violate
the offline invariant, which is a deliberate trade: no product signal, so detection has to come
from tests and gates or from nowhere. The cost of this register is one file.

**How to use it.** Before proposing a new audit round, read this instead — a round that would
only re-derive these rows is not worth running. When you ship a fix, update its row in the same
change, exactly as you delete a backlog item.

---

## The register

Sorted by current `D`, then severity. `S` is R7's severity, kept so the reading stays honest
rather than RPN-flattened.

| D | S | Class | Why it is (un)detectable | What would change this score |
|---|---|---|---|---|
| **10** | 6 | **The warm-page LRU order is reordered** (`serve_site/exec_pool.rs`) | **Zero test files reference `MAX_WARM_PAGES` or `exec_pool`** (re-measured 2026-07-28). This is also the project's one standing freeze, so the guard against it is entirely social | A test that asserts the eviction *order* on a pool driven past `MAX_WARM_PAGES`. Note the freeze forbids *tuning* it, not *pinning* it |
| **10** | 5 | **A shape absent from the corpus breaks** | By construction: the corpus is the regression net, so it cannot catch what it does not contain. R11 inventoried real-document shapes with no corpus instance; R14 found 13 such deck constructs | Nothing general. Deliberately unfixed — the walker renders every corpus doc on every `cargo test`, so growing the corpus to chase this trades one cost for another. Pin only shapes that earn it (items 127, 128 did) |
| **9** | 6 | **Click-to-source stops landing the cursor** | The harness passes both directions but **stops at the relay** and cannot see whether the editor moves the cursor. Any change to the relay or the companion re-opens a manual check | An editor-side integration test. Until then this is permanently manual, and the companion rewrite (2026-07-28) is exactly the kind of change that re-opens it |
| **8** | 4 | **The `animate` tick becomes a free-running dataflow loop** | The named trap of item 155, and it has **no observable this harness can reach**. Two candidate browser assertions were written and both deleted after mutation-checking: a frame CEILING stayed green with both pacing mechanisms removed (headless `requestAnimationFrame` is throttled by the Plot re-render anyway), and a LAG measure stayed green too (each pass reads `tali.value` when it *starts*, so the last pass to run always reads the newest tick). The two mechanisms in `bindAnimate` are therefore enforced by construction and review only. Detail in `crates/server/tests/reactive_browser.rs` | A pass COUNTER on the runtime (`runSequentially` increments; the probe compares completed passes against published ticks). That is a shipped-code change made for a test, which is why it was not taken now |
| **8** | 5 | **A dogfood-only shape breaks** | `docs/guide` and `docs/internals` sit outside the corpus walker, so a construct used only there has no automatic renderer | Walking the two books in the corpus test, at the cost of build time |
| **5** | 3 | **A shared-`.bib` entry is cited only from a place the source scan miscounts** (`site/bibliography.rs`) | `validate_shared_bibliography` counts citations by scanning each page's **source** (post-include), not its render, so a `[@key]` inside a fenced code block counts as a citation. The bias is deliberate and one-directional: over-counting makes the unused-entry lint go *quiet*, never wrong-and-loud, so the failure mode is a dead entry not reported rather than a live one falsely flagged. Nothing detects the quiet case | Counting from `cite::process`'s resolved key set instead, which means rendering every page in the site-wide pass — a real cost for a SUGGESTION-severity lint, which is why it was not taken. The include-expansion half of the same trap IS pinned (`a_citation_that_only_an_include_makes_still_counts`) |
| **6** | 6 | **A contributor breaks a gate** | `./tools/gates.sh` now runs every gate in one process and **refuses to be green when one skipped**, which closed most of this. What remains: `core.hooksPath` is **unset in a fresh clone**, and `ci.yml` is guarded on `repository.private != true`, so **CI certifies nothing while this repo is private** | Publishing the repo arms CI. Until then the only real gate is the author running `gates.sh` |
| **5** | 9 | **A tag ships the wrong licence** | Was D=10 (nothing inspects a tag). The five pre-relicence tags were **deleted**, so the instance is gone — but the *class* is not: nothing still inspects a tag's contents | A release-time check that the tag's `LICENSE` matches the manifest. `release.yml` is the natural home |
| **5** | 8 | **Prose contradicts behaviour** | Was D=9 ("no gate of this kind exists"). Three tree-derived gates now exist (`stale_docs.rs`, `gate_script.rs`, `release_targets.rs`, plus `headless_js_feature.rs` added 2026-07-28), and they catch claims **derived from the tree**. They cannot catch a claim where both spellings resolve — which is most of them, and is why item 143 was a hand sweep | More gates of the derive-from-source shape. Each one only covers its own claim; there is no general mechanism, and pretending otherwise is how this class rots |
| **4** | 7 | **A deck ships broken** | Was D=10 (zero validators reached a deck in a site). A site `check` now walks `site.decks` and `build --strict` validates them, and `deck_browser.rs` is the **first browser test of `deck.js`** — it found two shipped layout defects on its first run | Broader deck coverage in the corpus; 13 deck constructs still have no instance anywhere |
| **4** | 6 | **The `{js}`/`{glsl}` client runtime regresses** | Was D=9: every test of the reactive client asserted what Rust *emitted*, so the whole browser half — the cell scheduler, the input registration, the shader language — could have been broken with the suite green. `reactive_browser.rs` now runs all four `corpus/reactive/` pages in headless Chrome and reads pixels, published values and DOM back. **Same coupling as the deck row**: it declares `required-features = ["headless-js"]`, so dropping that flag from `gates.sh`/`ci.yml` returns this to D=9 while the suite stays green | Keeping the feature wired (`gate_script.rs` pins the canary name; `headless_js_feature.rs` pins the flag) |
| **4** | 6 | **`deck.js` regresses** | Same fix as above: `deck_browser.rs` (11 test fns) steps a real deck in headless Chrome. **Watch the coupling**: that binary declares `required-features = ["headless-js"]`, so it is not built unless the feature is asked for. `gates.sh` and `ci.yml` pass it, and `headless_js_feature.rs` fails if either stops | Keeping that feature wired. Dropping the flag would return this row to D=9 while the suite stayed green |
| **3** | 9 | **`mounts:` escapes the project root** | Was D=9 (zero containment tests). Fixed and **pinned**, and the pin needed a target that does **not exist** on disk — a lexical check fully shadowed by the canonical-symlink check passes with the guard disabled. A guard can be dead and green | — |
| **3** | 9 | **`check` spawns a project-supplied interpreter** | Was D=8 (no gate on that path). Fixed: `check` reports the interpreter rather than spawning it, with a named test | — |
| **3** | 8 | **`--no-exec` does not stop browser-side code** | Was D=9, and the detection failure was the instructive part: the one test covered `read --run`, an adjacent property, so the test list *read* as covered. Fixed with a pin naming the preview path and the browser-side channels | — |
| **3** | 6 | **A kernel/interpreter test skips silently** | Was D=8 ("the gate is the author's memory"). `gates.sh` arms all four `TALIESIN_REQUIRE_*` variables, asserts each canary printed `... ok` **by name**, and treats one ignored test as a failure — so a renamed or deleted canary fails loudly | — |
| **3** | 3 | **A `draft:` page accumulates defects** | Was D=10 ("zero validators reach it"). **Ruled 2026-07-28 (item 110): not linting drafts is correct** — they do not ship, and the preview lints them where the author writes. The defect was the *silence*; `check` now names the drafts it held back | — |

---

## Reading the scores

- **Nothing here is below 3.** A fix plus a named test is a 3, not a 0: the test can rot, be
  renamed, or assert an adjacent property, which is exactly how `--no-exec` scored D=9 with a
  passing test in the suite.
- **A high `D` is not a criticism of the test suite.** It is a statement about *where* the net
  is. The suite is dense on pure functions over the block model and thin on anything that only
  exists at runtime, in a browser, in a published artefact, or in prose.
- **Three structural causes**, and every row is an instance of one: the regression net is the
  corpus, so it catches only what the corpus contains; nothing runs on anyone's machine but the
  author's; and the checks assert on emitted strings while the riskiest surfaces are behaviour.
  That third one is why Wave 1's three HIGH security findings survived ~30 correctness rounds —
  **correctness rounds read code, and that class only fails when something runs.**
