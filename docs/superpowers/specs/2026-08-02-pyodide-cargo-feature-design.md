# Pyodide as an opt-in cargo feature (backlog item 205)

**Date:** 2026-08-02
**Status:** **IMPLEMENTED** 2026-08-02 as Wave 0.3 of the final-scope plan. Backlog item 205 is
deleted; item 148's `.crate` blocker is discharged. Four corrections the implementation
measured are recorded in "What this actually bought" at the end; read that before quoting any
number above it.
**Backlog item:** 205, P1

## Problem

`crates/core/src/render/pyodide.rs` `include_bytes!`s the vendored Pyodide WASM runtime into
every executable. Everyone who builds or installs Taliesin gets a Python runtime for a
capability one showcase page uses.

### The filed numbers were rot; these are measured 2026-08-02

| Claim | Filed | Measured |
| --- | ---: | ---: |
| `target/release/taliesin` | 101 MB | **72.1 MiB** (75,598,984 B) |
| Pyodide payload in the binary | 12.9 MB | **15.7 MiB** (16,455,447 B) |
| `crates/core` tracked bytes (item 148) | 7.3 MiB | **24.2 MiB** |

The 12.9 MB figure (written into three doc comments and two source comments) predates the
vendored numpy wheel: the other six payload files sum to 13,539,424 B, which is 12.91 MiB.
Adding `numpy-2.4.3-...whl` at 2,916,023 B took it to 15.7 MiB and no comment was updated.

### What the release actually costs a downloader

`.github/workflows/release.yml` packages with `tar czf`, so the download is compressed:

| | Bytes | MiB |
| --- | ---: | ---: |
| Binary, gzip -6 | 33,462,323 | **31.9** |
| Binary stripped, then gzip -6 | 32,531,934 | 31.0 |
| Pyodide payload alone, gzip -6 | 9,237,184 | **8.8** |

Two consequences, both of which shaped this design:

1. **Stripping is not the lever.** Debug symbols compress away, so `strip = true` buys 0.9 MiB
   off the download. Out of scope here.
2. **The download is 32 MiB, not 101 MB.** Pyodide is 27.6% of it. That is a real share but a
   different problem from the one the item describes, and it is not what this change fixes.

### The blocker nobody had measured

The eight payload files are **git-tracked**, and they are what puts `taliesin-core` at 24.2 MiB
against crates.io's 10 MiB `.crate` cap: 2.4x over. Item 148 attributes the `cargo publish`
failure solely to `Cargo.toml:14` declaring `taliesin-core = { path = "crates/core" }` with no
`version`. That is a second, independent blocker; the payload is the larger one, and a cargo
feature does **not** shrink a `.crate`. Only `exclude` does.

## Why not a smaller default download

`.github/workflows/release.yml:59-64` states a policy for `headless-js`, which is off by default
for source builds and **on** in the tarball:

> a DOWNLOADED binary should be the complete tool, the person who takes the tarball did not pay
> that cost and cannot re-enable it without rebuilding.

This design keeps that policy. The tarball stays complete and stays at roughly 32 MiB. Shrinking
the download would require a sidecar payload (a second release asset, a runtime lookup path, and
a new absent-payload failure mode) to buy 9 MiB off 32 MiB, at the cost of shipping an incomplete
default tool. That trade was considered and rejected. If a real signal ever shows that 32 MiB
deters evaluation, it is a separate item with its own ruling.

## Hard constraint

**The tool stays offline by default.** Nothing in this change makes a network call, at build
time or at render time. A build without the feature does not fetch the runtime, does not prompt
to fetch it, and does not fail: it renders `{pyodide}` cells as source.

## Design

### 1. Feature declaration

`crates/core/Cargo.toml`:

```toml
[package]
exclude = ["assets/pyodide/*"]

[features]
pyodide = []
```

`crates/server/Cargo.toml`, a passthrough so the workspace-root spelling matches `headless-js`:

```toml
[features]
default = []
headless-js = ["dep:chromiumoxide"]
pyodide = ["taliesin-core/pyodide"]
```

`exclude` affects `cargo package` / `cargo publish` only; it does not change the repo build.
A consumer of the published crate therefore cannot enable `pyodide`: the files are absent and
`include_bytes!` fails to compile. That is a loud error rather than a silent one, and it is
documented at the feature declaration.

### 2. The one byte-bearing site

`pyodide_payload()` at `crates/core/src/render/pyodide.rs:50` is the only `include_bytes!` in
the tree for this payload. Feature-off it returns `&[]`.

Every other consumer already handles the empty case through a contract that exists today, so
none of them needs a new branch:

| Consumer | Feature off | Change |
| --- | --- | --- |
| `pyodide_index_meta` (`pyodide.rs:69`) | returns `""`, which its own doc already names as the signal to degrade | none |
| `attach_pyodide_index` (`pyodide.rs:111`) | no-op, `has_pyodide_cell_markup` is false | none |
| `serve/mod.rs:878`, `serve_site/mod.rs:661` | `.find()` yields `None`, route 404s, and nothing links it | none |
| `build.rs:795`, `build.rs:1445` | `used.pyodide` false, `write_pyodide_payload` never called | none |
| `degrade_pyodide_cells` (`pyodide.rs:152`) | stays compiled unconditionally: it is the fallback for `build <file.tmd> out.html` and for `pdf.rs`/`query.rs`, none of which depends on the feature | none |

`PYODIDE_DIR_NAME` and `PREVIEW_PYODIDE_DIR` are `const` strings carrying no bytes. They stay
unconditional so the route registrations and the `build.rs` string matches keep compiling.

### 3. The one real hole, and the precedent that closes it

With only the payload gated, the render pipeline still emits a live
`<script type="application/tali-pyodide">` wrapper whose `indexURL` meta is absent: an empty
husk that loads nothing. That is exactly the failure mode `degrade_pyodide_cells`' doc comment
warns about.

**The renderer already has a mechanism for "this client cell cannot run, show its source",** and
it is not a post-pass. `crates/core/src/render/mod.rs:1233-1239`:

```rust
if no_exec_in_force() {
    // `--no-exec`: a client-side cell is a code cell whose kernel is the
    // browser, so it renders as source like a `{python}` cell with no kernel
    // does (item 79). `emit` keeps the highlighted source and the block's
    // id/sourcepos, so click-to-source and the incremental swap are unaffected.
    emit(node, &attrs, &mut html);
}
```

A feature-off `{pyodide}` cell takes that same arm. Introduce one predicate in
`render/client_lang.rs`:

```rust
/// False for a registered client language whose runtime was compiled out. Today only
/// `{pyodide}`, whose vendored payload is behind the `pyodide` cargo feature: the language
/// stays in `CLIENT_LANGS` (so the registry, the diagnostics and the mime contract are
/// feature-independent) and only its ability to RUN is gated.
#[cfg(feature = "pyodide")]
pub fn client_lang_runnable(_lang: &str) -> bool {
    true
}

#[cfg(not(feature = "pyodide"))]
pub fn client_lang_runnable(lang: &str) -> bool {
    lang != "pyodide"
}
```

Two `#[cfg]` bodies rather than one body containing `cfg!(...)`: the single-body form collapses
to `!(x && false)` when the feature is on, which is a const-foldable condition clippy flags, and
the workspace builds under `-D warnings`.

and AND it into the two gates that decide whether a client cell becomes live markup:

- `render/mod.rs:1053`, the figure-materialization gate
  (`client_lang(&lang).is_some() && !no_exec_in_force()`), which also governs the
  `emit_client_figure` arm at `mod.rs:1104`.
- `render/mod.rs:1229`, the plain client-cell arm, whose `no_exec_in_force()` branch is the
  emit-as-source path quoted above.

Both then fall to `emit(node, &attrs, &mut html)`.

**Why this rather than running `degrade_pyodide_cells` per block.** Four reasons, in order of
weight:

1. It is in-pipeline. `emit` is the ordinary fence emitter, so the block model is built correct
   the first time rather than rewritten afterwards.
2. It sidesteps the documented lossy round-trip entirely. `degrade_pyodide_cells` recovers the
   author's source by reversing `emit_client_cell`'s `</script` escape, and `pyodide.rs:143-151`
   records that an author who typed a literal `<\/script` is indistinguishable from one who did
   not. Never emitting the wrapper means never needing to reverse it.
3. The block-model invariants are already pinned on this arm. The `--no-exec` comment states
   that id/sourcepos, click-to-source and the incremental swap survive it, and `corpus.rs`
   enforces that for every block.
4. One predicate covers both the cell arm and the figure arm. A post-pass would have to be
   threaded through `emit_client_cell` and `emit_client_figure` separately.

**Behaviour consequence, stated rather than hidden.** `emit` highlights by fence language, and
syntect has no `pyodide` syntax, so a feature-off `{pyodide}` cell renders as an unhighlighted
listing, whereas `degrade_pyodide_cells` emits `class="language-python"`. This is not a
regression introduced here: it is exactly what `TALIESIN_NO_EXEC=1` produces for a `{pyodide}`
cell today. Consistency with the shipped `--no-exec` path is the deliberate choice. If the
unhighlighted listing is later judged wrong, the fix belongs on the `--no-exec` path, where it
fixes both.

### 4. Gates

The project's own warning applies directly here: a gate that skips silently is worse than no
gate. `tools/gates.sh` already arms `--features headless-js` and asserts by name that its tests
printed `... ok`. This change extends that pattern rather than inventing one.

**Tests that need the runtime compiled in** are gated at one of two altitudes, and which one is
not a free choice: `required-features` gates a whole `[[test]]` target, so it is only correct for
a file whose every test needs the payload.

Whole-file, via `[[test]] required-features = ["pyodide"]`:

- `crates/core/tests/pyodide.rs`
- `crates/server/tests/pyodide_browser.rs`

Per-test, via `#[cfg(feature = "pyodide")]` on the test function, because the file's other tests
must keep running by default:

- `crates/core/tests/third_party.rs::the_vendored_pyodide_payload_is_complete_and_carries_its_licence`
  (the file's other third-party assertions are feature-independent)
- `crates/server/tests/asset_bundle.rs`, the test containing the `pyodide_payload()` loop at
  line 712

The per-test form does mean those two vanish silently from a default `cargo test`. That is
precisely what `tools/gates.sh` asserting them **by name** exists to catch, which is why the
gates change below is part of this design and not a follow-up.

**`tools/gates.sh`** arms `--features taliesin-server/pyodide` and asserts the pyodide tests
printed `... ok` by name, so dropping the feature turns the gate red instead of green-and-empty.

**`.github/workflows/release.yml`** adds the feature to its build line, keeping the tarball the
complete tool:

```
--features taliesin-server/headless-js,taliesin-server/pyodide
```

### 5. The corpus pin

`corpus/reactive/pyodide.tmd` exists and pins the feature-on path. This change makes it render
two ways, and the second way is what is currently unpinned. The new coverage this change owes is
a feature-off assertion that the document renders its `{pyodide}` cells as **source blocks
carrying their `data-block-id` and `data-sourcepos`**, and emits neither a
`<script type="application/tali-pyodide">` wrapper nor a `tali-pyodide-index` meta.

Per the standing rule against growing `corpus/` past the pin a feature needs, this adds **no new
corpus document**. It adds an assertion over the existing one.

Note the inlined-asset needle trap: every page inlines its whole JS payload, and
`assets/js/pyodide.js` calls `registerLanguage("application/tali-pyodide", ...)`, so a bare
whole-page `contains("application/tali-pyodide")` is true whenever the enhancer shipped. The
negative assertion must needle `<script type="application/tali-pyodide"`, the prefix
`has_pyodide_cell_markup` already uses for exactly this reason.

### 6. Stale comments fixed in the same change

Five sites say 12.9 MB and are wrong by 2.8 MiB:

- `crates/core/src/render/pyodide.rs:35` and `:95`
- `crates/core/src/render/page.rs:272`
- `crates/server/src/query.rs:72`
- `crates/server/src/serve/mod.rs:281`

Correct them to 15.7 MiB. A test tying the number to the payload is deliberately **not** added:
it would go red on every upstream re-vendor for a figure that is prose, and the version literal
is already drift-locked to upstream's `package.json` by
`the_vendored_pyodide_version_is_locked_to_the_payload`.

## What this buys, measured

| | Before | After |
| --- | ---: | ---: |
| `cargo install` / source build | 72.1 MiB | ~56 MiB |
| `taliesin-core` `.crate` | 24.2 MiB | ~8.5 MiB (under the 10 MiB cap) |
| Release tarball (gz) | 31.9 MiB | 31.9 MiB (unchanged, deliberate) |

## Out of scope

- `strip = true`. Measured at 0.9 MiB off the compressed download; not worth coupling to this
  change.
- Shrinking the tarball. See "Why not a smaller default download" above.
- Item 148's missing `version` field. This change removes the larger of the two `cargo publish`
  blockers; the manifest metadata gap stays filed there.

## Backlog effects

- **Item 205** is deleted from `notes/backlog.md` when this lands.
- **Item 148** is amended: its `crates/core = 7.3 MiB` figure is rot (24.2 MiB), and the
  `.crate` cap blocker is discharged by this change, leaving only the manifest metadata.

---

## What this actually bought, measured at implementation (2026-08-02)

Four things the design got wrong, all found by running it. The design's *reasoning* held
throughout; these are its numbers and its lists.

**1. The saving is 31.3 MiB, not ~16 MB, because the payload was embedded TWICE.**

| | measured |
| --- | ---: |
| `target/release/taliesin`, feature on | 75,599,072 B (**72.0 MiB**) |
| `target/release/taliesin`, feature off | 42,682,064 B (**40.7 MiB**) |
| difference | 32,917,008 B (**31.3 MiB**, 43.5% of the binary) |

The feature-on figure reproduces the design's baseline to within 88 bytes, so the comparison is
sound. The difference is 2x the 16,455,447 B payload to within 6 KB, and a literal taken from
`pyodide.mjs` occurs **twice** in the feature-on binary against once in the source file. The
payload was duplicated in the binary (most likely the const-promoted array literal materializing
a second copy of the `include_bytes!` statics). Both copies go. The final-scope spec's "75.6 MB
→ ~59 MB" is therefore understated: it is **72.0 MiB → 40.7 MiB**.

**2. The `.crate` lever works, verified by running it.** `cargo package -p taliesin-core` now
reports **248 files, 8.6 MiB (2.8 MiB compressed)** against the 10 MiB cap.

**3. The test-gating list was incomplete, and one entry was wrong.**

- Missed: `crates/core/tests/client_lang.rs` (three sites, not zero) and
  `crates/server/tests/render_blocks_cli.rs`.
- Wrong: `third_party.rs::the_vendored_pyodide_payload_is_complete_and_carries_its_licence`
  must **not** be gated. It reads `assets/pyodide/` off disk, and those files are git-tracked
  either way (`exclude` affects only a published `.crate`). Gating it would drop a
  licence-compliance assertion from every default build for no reason.
- The whole-file `required-features` altitude was right for `tests/pyodide.rs`, but for a
  reason the design did not give: of the 4 tests that still *passed* feature-off, 3 passed
  **vacuously** (with no wrapper emitted, a test that `degrade_pyodide_cells` strips one
  asserts nothing). The 1 genuinely feature-independent test, the version drift lock, was
  moved to `tests/third_party.rs` so it keeps running by default.

**4. Two drift gates fired that the design did not anticipate**, both correctly:
`gate_script.rs` pins the canary count (9 → 11) and `headless_js_feature.rs` requires the
feature flag to sit on the `cargo test --workspace` line itself, not on a continuation.

**Added beyond the design:** `crates/core/tests/pyodide_feature.rs`, a twin of
`headless_js_feature.rs` pinning all six files that must agree, and
`crates/core/tests/pyodide_feature_off.rs`, the feature-off corpus pin the design asked for
(4 tests over `corpus/reactive/pyodide.tmd`, no new corpus document).

**Verified:** `cargo clippy --workspace --all-targets -D warnings` clean both ways;
`cargo test --workspace` **2319 passed / 0 failed / 0 ignored** feature-off and **2328 / 0 / 0**
feature-on (the +9 is 13 gated tests gained minus the 4 feature-off-only ones); and a real
`build corpus/reactive/pyodide.tmd --out` emits 0 wrappers, 0 index meta and 0 copied assets
feature-off, against 3 wrappers and a 16M `_assets/pyodide-314.0.3/` feature-on.
