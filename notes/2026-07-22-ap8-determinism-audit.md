# AP8 — Determinism / reproducibility audit

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Date:** 2026-07-22
**Perspective:** AP8 from the "Audit perspectives" menu in `backlog.md`. *Read half fan-out-safe;
the rebuild-twice check is (lightly) stateful.* Chosen after reading the three concurrent sessions:
AP2/AP5/AP9/AP12 were already claimed, the feature session (DX17b) owned the exec/serve/build +
browser/kernel surface, so AP8 was the highest-value **disjoint** pick — it strikes a load-bearing
invariant (deterministic content-hash block-ids underpin the incremental diff + `_freeze` cache)
and its core check runs in-process, colliding with nobody.
**Method:** the definitive test for determinism is **cross-process** byte comparison — a `HashMap`
has a fixed `RandomState` seed *within* one process, so rendering twice in one process would hide
map-iteration-order bugs; two separate process invocations get different seeds. Harness
(`notes/ap8-determinism-harness/`): (1) render every corpus + docs `.tmd` to a full page in **3
separate processes** and compare sha256; (2) `taliesin build <site>` **twice into separate dirs**
(separate processes, `TALIESIN_NO_CACHE=1`) and `diff -rq` the trees, for all 9 site projects;
(3) the **executed-cell** path with a live Python kernel (matplotlib); (4) an absolute-path **leak**
grep the same-path cross-process diff structurally cannot catch. Plus a source read of every
non-determinism source (HashMap/HashSet iteration, `read_dir` order, wall-clock, `uuid`/`rand`).

---

## Headline

**The render and build pipelines are deterministic and reproducible *by design* — comprehensively,
and clearly on purpose.** 121 single documents rendered byte-identically across 3 separate
processes; all 9 site builds (tech-blog, the two docs books, scaffolds, embed, cite-this, bayesian,
demo-book) produced **byte-identical output trees** across separate processes with no timestamps,
no absolute paths, and no map-order drift. The author has already engineered the obvious hazards
shut: pages are **sorted** (`discovery.rs:17,68`, "path-ordered"), the book offline `.zip` **sorts
its entries and stamps a fixed 1980 DOS date** (`build.rs:1866` + `zip.rs:25`, comment: "a rebuild
of an unchanged book produces a byte-identical archive"), block-ids are **content hashes**, and the
`body_html_snapshots` tests **pin exact bytes** hermetically.

**Exactly one real gap, and it is cross-cutting (reproducibility + a local-path leak + reader
noise): executed-cell stderr embeds the kernel's PID-bearing temp path.** It is a P3, but it is
genuinely new — the AP12 offline-guarantee round (which hunted absolute-path leaks) did not catch
it, because it lives only in *kernel-executed* output, not the static build the leak check covered.

---

## Verified finding

### AP8-1 (P3) — Cell **stderr stream** embeds the non-deterministic `/tmp/ipykernel_<PID>/…py` path → cold/cross-machine builds are non-reproducible (and it leaks a local path + adds reader noise)

Any `{python}`/`{r}` cell that writes a **warning** to stderr (the Python `warnings` module prints
the emitting file's path) splices `/tmp/ipykernel_<PID>/<cell-hash>.py:LINE: …Warning: …` verbatim
into the built HTML. The `<PID>` changes on every kernel start, so:

- **Two cold builds of the same doc differ.** Verified: a 3-line matplotlib doc
  (`plt.show()` under Agg → the ubiquitous `FigureCanvasAgg is non-interactive` `UserWarning`) built
  twice under `TALIESIN_NO_CACHE=1` differs in exactly one line —
  `…tali-stderr">/tmp/ipykernel_3505167/4130013440.py:6: UserWarning…` vs `…ipykernel_3505239/…`.
  Everything else (including the matplotlib **SVG** plot itself) is byte-identical.
- **Cross-machine / CI is non-reproducible** (different PID, different `/tmp` path), and it **leaks a
  local absolute path** into published HTML (the static build leaks nothing — verified 0 files
  contain the source path / `$HOME` / username; this is the one leak vector AP12 missed).
- The `_freeze` cache **masks it on a warm same-machine rebuild** (it replays the first execution's
  bytes) but that just bakes in a *stale, meaningless* PID path; the cache key is a content hash
  that (correctly) does not include the PID, and `_freeze` is gitignored, so every machine bakes in
  its own PID.

**Root cause (pinned):** `render_outputs` at `crates/server/src/kernel.rs:994`, the
`Output::Stream { stderr, text }` arm, splices `esc(&strip_ansi(text))` — it strips ANSI and
HTML-escapes but does **not** normalize the ipykernel temp path. The sibling `Output::Error`
(traceback) arm is already clean, because modern ipykernel formats traceback frames as `Cell In[N]`
(verified: a `1/0` cell builds byte-identically twice) — so **only the stream arm needs the scrub**,
and warnings are the common trigger (matplotlib / pandas `SettingWithCopy` / numpy `RuntimeWarning`
/ sklearn convergence, etc.).

**Fix direction:** normalize stream text before splicing — replace `/tmp/ipykernel_\d+/\d+\.py`
(and the legacy `<ipython-input-\d+-\w+>`) with a stable placeholder such as `<cell>`, mirroring
`nbconvert`/Quarto. One small function at the `kernel.rs:994` splice point (apply to the stream arm,
and defensively to traceback lines for older kernels). It fixes reproducibility, closes the leak,
and improves the reader-facing warning (`<cell>:6: UserWarning: …` instead of a meaningless
`/tmp/ipykernel_3505167/4130013440.py`). Pin with a test: the matplotlib-warning doc builds
byte-identically twice under `TALIESIN_NO_CACHE=1`.

---

## Surfaces confirmed deterministic (so a future round need not re-audit them)

- **Static single-doc render** — 121 corpus + docs `.tmd`, 3 separate processes each, 0 drift
  (render / cite / math-KaTeX-in-quick-js / syntect-highlight / deck / divs all map-order-stable).
- **Full site build** — 9 projects, separate processes, `NO_CACHE`, byte-identical output trees
  (search index, RSS feed, `llms.txt`, backlinks, cross-page xref, sitemap: **no** map-order drift;
  **no** build-time timestamps; parallel per-page tasks collected in stable order).
- **Executed-cell rich output** — matplotlib **SVG** and **PNG** byte-reproducible across executions
  (matplotlib 3.11 emits stable ids here); error **tracebacks** clean (`Cell In[N]`).
- **The book offline `.zip`** — sorted entries + fixed 1980 timestamp = reproducible even
  cross-machine.
- **No absolute-path leak** in static output; `data-source-file` is relative.

## False leads (grep-flagged, code-verified clean — per "trust the symptom, re-derive the cause")

- **`read_dir` page order** (`discovery.rs:111`) — *sorted* immediately after (`inputs.sort()` +
  `pages.sort_by(rel)`); output is path-ordered, not filesystem-ordered.
- **`body_html_snapshots` "drift"** (cited in the backlog as a determinism symptom) — it is
  **edit-driven** (you changed the `{js}` source → rewrite the pinned bytes), not run-to-run
  variation. The snapshots are byte-exact and pass hermetically in CI, so they are *evidence of*
  determinism, not against it.
- **The book `.zip` walk** (`build.rs:1854`, `read_dir`) — already sorted + fixed-timestamp.
- **`uuid`/`SystemTime::now`** hits — all in server *runtime* (session tokens, `/tmp` runtime dirs,
  log timing, ZMQ message ids) or `#[cfg(test)]`; none reach built output.

## Out of AP8 scope (other perspectives)

`_freeze` cache-correctness beyond determinism → AP4. `{js}` cells render client-side, so their
output determinism is a browser-runtime matter → AP1/AP6. Whether *user cell code* is deterministic
(unseeded `np.random`, wall-clock in a cell) is the author's responsibility, not the tool's;
Taliesin's contribution to cell reproducibility is the `_freeze` replay (AP4) plus AP8-1 above.

---

## Build-ready item to file into `backlog.md`  (prefix `AP8-`)

- **AP8-1 (P3):** Normalize the ipykernel temp path in executed-cell **stream** output. In
  `render_outputs` (`crates/server/src/kernel.rs:994`, `Output::Stream` arm), scrub
  `/tmp/ipykernel_<PID>/<hash>.py` (and legacy `<ipython-input-…>`) to a stable `<cell>` placeholder
  before escaping. Makes cold/CI/cross-machine builds byte-reproducible for any warning-emitting
  cell, closes a local-path leak the AP12 round missed, and cleans reader-facing warnings. Small,
  contained; pin with a matplotlib-`UserWarning` doc that must build byte-identically twice under
  `TALIESIN_NO_CACHE=1`. *(The R kernel's stderr warnings are the same class — apply the same scrub.)*

## AUDITS.md ledger line

> **AP8 determinism / reproducibility audit — 2026-07-22** →
> [2026-07-22-ap8-determinism-audit.md](2026-07-22-ap8-determinism-audit.md). Cross-process byte
> comparison (the only test that catches per-process HashMap-seed drift): 121 single docs ×3
> processes + 9 site builds ×2 (separate processes, `NO_CACHE`) + the executed matplotlib path +
> an absolute-path leak grep. **Headline: render + build are deterministic/reproducible by design**
> (sorted pages, sorted + fixed-1980-timestamp `.zip`, content-hash block-ids, byte-exact hermetic
> snapshots; 0 drift across 130 targets, no timestamps, no path leaks). **One real P3, new
> (AP12 missed it): AP8-1** — cell **stderr** streams embed the PID-bearing `/tmp/ipykernel_<PID>/…py`
> path (Python warnings), so cold/CI/cross-machine builds are non-reproducible + it leaks a local
> path + adds reader noise; fix = scrub the path at `kernel.rs:994`. Tracebacks are already clean.
