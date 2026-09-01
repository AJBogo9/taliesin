# Product-audit backlog, 2026-09-01

Durable state for the whole-product end-user audit run at `e23e42d4` (1.1.0, clean tree).
Produced by an 84-agent audit (5-lens code review plus an 8-dimension product workflow,
one adversarial refuter per finding, plus gap probes); 44 raw findings, 13 confirmed
critical/moderate after dedup. Items below the cutoff were deliberately not filed.

## Landed: all 13 items (T1-T13), 2026-09-01

Implemented and verified the same day the queue was filed, in one pass: nine parallel
work packages, a failing witness test first wherever a test could hold the defect, then
a 21-agent adversarial review over the merged diff, whose 14 confirmed findings were all
folded in. The largest of those was a HIGH sibling of T2: an Update-before-Update of the
same block id still corrupted 7 of 326 edit shapes ([a,b,c,d] to [d,b,c,a] never
rendered the swap). `emit_gap` now demotes a gap pairing whose new side is a moved old
block to Remove + Insert, the diff sweep replays every shape through a client model, and
a source-pin test ties that model to web-client/client.js so the two cannot drift
silently. Per the standing rule ("delete an item when it lands") the item bodies are
gone; `git log` on this file holds them. Landed alongside the queue items:

- the `--check-only` spelling of T11's refusal (same message, json-aware);
- exec-phase warnings reach both writing builds' `--format json`, not just the preview
  panel (T5's channel, all three drains);
- the third asset-resolution surface, `deploy_referenced_sources`, percent-decodes like
  the validator and the copier (T3);
- the partial rule matches the walker: underscore DIRECTORIES suppress the standalone
  bibliography advice too (T13b);
- a committed instrument for the bare-newline-split ban (`lsp_source_hygiene.rs`),
  proven against the pre-fix file bytes at HEAD (4+1 needle hits there, 0 now);
- four stale "fonts inlined" docs lines invalidated by T6, and the `stale_docs`
  README-pin test rewritten to pin the newest EXISTING release tag: its old
  crate-version-parity invariant was itself the T1 rot mechanism.

Verification: `./tools/gates.sh` verdict on the final tree, quoted: "PASSED — every gate
ran and passed (13 gates)". Browser-verified live: T2 move-to-front AND the first-last
swap (DOM matches the file, same tab, no reload, no console errors), T5's warning
located in the preview diagnostics panel, T12's search-button name and h2 card headings
on the built blog in Chrome on Linux.

## Residual, recorded not hidden

- Two open parents including the same PARTIAL can still clobber each other's foreign
  diagnostics (transient, self-healing on the next edit; named in `lsp.rs::publish`'s
  doc comment; the dirty-buffer half IS fixed by the open-URI guard).
- `serve_site::build_page`'s three-line exec-warnings wiring has no automated witness at
  its own seam: pinned at the Executor seam and at build's json seam, and verified live
  in the preview panel on 2026-09-01 only.
- Safari remains code-read only (the pre-existing R12 real-device gap).

---

# Decisions recorded, not tasks (the author's, not code)

- **D1: the `_quarto.yml` probe vs the 2026-08-17 ruling.** `load_config`
  (`site/config/mod.rs:162-169`) still probes another tool's config filename and answers
  with migration advice: the one compatibility note that survived the FD2 register sweep
  (it lives in load_config, not a register const). Item 53 shipped it deliberately; FD2
  later ruled "no compatibility notes". The keep argument is real (a `_quarto.yml`-only
  directory otherwise builds with every setting silently defaulted). Cutting costs ~10
  lines plus two tests. Only the author can rule; record the ruling in DO-NOT-REBUILD.
- **D2: three.js from esm.sh on published pages.** Already the recorded open owner decision
  (`notes/2026-08-03-scope-inventory.md` (b)): vendor it beside the include or keep
  declaring the trade. Restated here only because T9 shows the strict gate would not catch
  a future undeclared instance.

# Refuted this round: do not re-file

Killed by adversarial verifiers on 2026-09-01; candidates for DO-NOT-REBUILD entries when
this file is worked.

1. "HTML-level block invariants are pinned on one document only" (the walker test covers
   the entire corpus plus docs).
2. "Preview front-matter digest refresh shipped with zero tests" (a dedicated unit test
   exists in the file the finder grepped).
3. "base64_encode's claimed twin is missing" (a byte-identical twin exists in
   `crates/core/build.rs:11-33`).

# Affirmations worth not re-litigating (measured 2026-09-01)

- **Click-to-source through includes is fully intact** (preview half): 67 attributed
  elements, zero cross-file spans, zero orphaned half-pairs; live edits shift only the
  partial's blocks; both open paths resolve the partial itself. T8 is LSP-only.
- **Live-state preservation is intact**: one-block update on save; scroll, mermaid SVG
  identity, details state, {js} slider and derived values, window sentinel all survived.
- **Feeds, sitemap, robots are conformant and deterministic**: feedparser bozo-false,
  byte-identical rebuilds, draft fixture absent from the whole output tree.
- **Published pages degrade cleanly off Chromium**: Firefox 154 verified empirically in
  both themes; worst realistic loss anywhere is cosmetic. Safari remains code-read only
  (the R12 real-device gap stands).
- **The first-run journey is sound** except T7: atomic scaffold refusals naming the
  colliding file, honest red/warn doctor paths with exact fix commands, JSON/human
  agreement everywhere else.
