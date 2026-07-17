# Reduction and modularity pass: design

> Status: approved (brainstorm 2026-07-17). Successor step to the reduction-audit
> request; the extension-system discussion that shaped it is summarized in Context.

## Context

The trigger was a "reduction audit" request: find features that are redundant or
not useful, and code that is never used, so the core stays lean and polished. The
longer-term motivation is an extension system (future), so non-core capability can
live outside a lean core.

Brainstorming reframed the request in four ways that this spec commits to:

1. **Three different audits, not one.** "Redundant", "never used", and "not core,
   belongs in an extension" have different stakes. Dead code and redundancy are safe
   to remove. "Move a working feature out of core" is a strategy decision that
   depends on an extension host that does not exist yet.
2. **The extension *system* is deferred.** Today "extensions" in Taliesin means
   shortcodes (`{{< … >}}`), `_extensions/` format bundles, and `--tali-*` themes,
   not a general feature-plugin host. Building a loader / manifest / packaging /
   sandboxing / marketplace is future work, gated on real users with real extension
   needs (phase iii below). We do not build it now.
3. **Phased ownership.** (i) internal modularity that benefits the developer and
   first user, now; (ii) user opt-in slimness, when the author actively uses the
   tool; (iii) third-party ecosystem, if/when external users appear. This spec is
   phase (i) only.
4. **Present benefit only.** After further discussion, the owner sharpened the rule:
   build nothing now that is not earning its keep for the developer/first-user
   today. So we do not carve speculative seams "for the future extension system";
   we decouple only where the current coupling is actively costing maintenance.

There is a relevant scar: an earlier extension system caused churn, where every core
change forced an extension-system change. Diagnosis: that system was built while the
core was still moving, its boundary leaked core internals, and it was abstracted
before it had two real consumers. The current core is settled (recent commits are
pre-release security hardening and bug fixes, not feature churn; the backlog's
Tier 1 is empty; schemas are closed), which is exactly the window where seams stop
churning. Doing this now, before publish, is also the cheapest time: the only
consumer of any seam is the author, so seams can still be reshaped freely.

## Goal

A leaner, more maintainable core, achieved by:

- deleting dead code and collapsing redundant paths (pure present benefit), and
- decoupling only the few features whose coupling actively costs maintenance,

leaving clean internal seams where decoupling already paid for itself. The extension
system, if it ever ships, grows out of those seams later; no extension machinery
ships in this pass.

## The ruler: what "core" means

Core is the moat architecture, not a feature list:

- the block model: every emitted block carries `data-block-id` (content hash) +
  `data-sourcepos`; included blocks carry `data-source-file`,
- per-keystroke block diff, warm server, warm Jupyter kernel, `_freeze` cache,
- click-to-source / single editing surface, closed schemas,

plus the Do-NOT-touch native subsystems (see Invariants). Everything else is
*breadth*, judged by one question: **does a real corpus doc exercise it, and does
keeping it in core cost anything to maintain?** The 71-doc corpus is the author's
real writing, so corpus coverage is a near-objective usefulness oracle.

## Non-goals (scope boundary)

Explicitly NOT in this pass, deferred until their phase's need is real:

- `_extensions/` loader for feature bundles, manifest schema, bundle discovery,
- a versioned public extension API, semver/compat guarantees,
- WASM or any sandboxing of untrusted extension code,
- packaging, install/uninstall flows, a marketplace,
- any change to the deck engine's decomposition (it is mid-redesign per
  `notes/2026-07-12-deck-audit.md`; the audit will classify it but Phase 3 will not
  touch it),
- new user-facing features, new output formats, or any invariant relaxation.

A short forward-note (Deferred extension future, below) records the intended growth
path so it is not lost, but zero machinery is built.

## Design

Three phases. Phase 1 is analysis only and gates the rest behind owner review.

### Phase 1: Audit (analysis only, zero code changes)

Sweep the Rust crates, client JS, and bundled assets, and classify every
feature/module into exactly one bucket:

- **dead**: unreachable, never called, or exercised by no corpus doc → delete
  candidate.
- **redundant**: two or more paths producing the same result → consolidate
  candidate.
- **tangled**: used, but coupled into core such that core edits ripple into it (the
  churn signature) → decouple candidate.
- **clean**: used and already a self-contained module → leave alone.
- **must-stay-native**: reaches the block model, numbering, includes, cite, or
  exec/freeze/kernel → never touch (this is the existing Do-NOT-touch set).

Method and evidence:

- **corpus coverage**: for each feature, name the corpus doc(s) that exercise it, or
  record "none". Trust the running product over notes; the backlog warns that stale
  notes misdescribe causes and costs.
- **reachability**: call-graph / usage sweep to catch never-called code and
  dead paths. Guard against known grep traps (a bare token matches substrings; check
  `crates/*/tests/` first, a pin named for a feature is near-proof it is live).
- Output: **one classified map**, one row per feature/module, with the bucket, the
  evidence, and a one-line recommended action.

The owner reviews the map before any code changes. This is the original
"reduction audit" deliverable, and it doubles as the decision input for Phases 2-3.

### Phase 2: Safe wins (pure present benefit)

Act on the unambiguous buckets:

- delete verified **dead** code,
- collapse **redundant** paths to a single implementation.

Guarded by the 71 corpus pins and the full `cargo test -p taliesin-core` suite;
client JS changes type-checked (`tsc -p jsconfig.json`) and browser-verified where
they affect rendered output.

### Phase 3: Targeted decoupling (only where Phase 1 shows present pain)

For features that are **both** tangled **and** costing real maintenance churn,
extract into a clean module with a minimal internal boundary. Add a tiny registry
only where core currently hardcodes a feature list. Each extraction obeys the three
anti-churn rules:

1. **Narrow contract.** The seam exposes blocks + context, never render-pipeline
   internals. A core refactor that changes *how* a block is produced, but leaves the
   block's shape (`data-block-id` / `data-sourcepos` contract) intact, must be
   invisible across the seam.
2. **Called, not woven.** The registry is a passive lookup core consults at a few
   defined points (e.g. div-kind dispatch, shortcode expansion, client-enhancer
   collection). It is not control flow threaded through core.
3. **Real-instance-driven.** Each seam is shaped by an existing feature, not
   imagined. A seam is generalized to a second shape only when a second real feature
   needs it.

Acceptance test per extraction: the feature reduces to **one module + one
registration line**, and core still compiles if both are deleted.

**If a feature is already clean, do nothing.** No speculative seams. Whether any
feature is extracted at all is decided by the Phase 1 map, not pre-committed.
Likely-but-not-guaranteed candidates to examine: `:::` div kinds / callouts (a
Tier-1 declarative shape riding on top of the frozen div machine) and OG-card
generation (`site/card.rs`, a build-time seam with no block-model or client risk).

## Deferred extension future (forward-note, not built)

Recorded so the growth path is not lost. If/when phases (ii)/(iii) arrive, the
intended model is tiers of power, cheapest-first, each riding a seam that already
exists informally:

- Tier 0 Themes (`--tali-*`, exists today),
- Tier 1 Declarative bundles (shortcodes, `:::` div kinds, assets, theme vars),
- Tier 2 Client behavior (`qmdEnhancers` registration + assets; the most on-thesis
  tier, since "wider" means richer browser behavior in a live HTML view),
- Tier 3 Execution/language (register a `{lang}` kernel / output transformer),
- Tier 4 Native compiled (Rust in the binary; needs WASM to distribute safely,
  which is deferred until demanded).

The discipline that governs ever building this: **the extension mechanism must be
smaller than the code it lets you externalize, or it is a net loss.** The
Do-NOT-touch set is already the "must stay native" set; the extension system's real
future job is to keep *new* breadth out of core, not to extract the frozen
subsystems.

## Invariants honored

- Block model preserved: `data-block-id`, `data-sourcepos`, `data-source-file` on
  included blocks. Corpus tests (`crates/core/tests/corpus.rs`) enforce this.
- Single editing surface: the preview never writes back; click-to-source only
  navigates.
- HTML-only output; no new compiler target.
- Do-NOT-touch respected: the standing warm-page eviction freeze
  (`MAX_WARM_PAGES` + `exec_pool.rs` LRU order) and the div machine (`divs.rs`),
  cite (`cite.rs`), includes (`includes.rs`), the numbering scanners, and
  exec/freeze/kernel are not rewritten. Phase 3 may register *on top of* these seams
  (e.g. a div kind), never rewrite them.

## Verification

- `cargo test -p taliesin-core` (corpus invariants + unit tests) green before and
  after each deletion/extraction; the 71 corpus docs are the regression net.
- `cargo fmt` clean (enforced by the PostToolUse hook + CI).
- Client JS: `cd web-client && npx -y -p typescript tsc -p jsconfig.json` clean;
  browser-verify via chrome-devtools MCP any change that alters rendered output.
- Per-extraction acceptance test: feature = one module + one registration line, and
  core compiles with both removed.

## Success criteria

- The Phase 1 map exists and the owner has reviewed it (the reduction audit
  deliverable).
- Verified dead code and redundant paths are gone, suite green.
- Any feature that was tangled *and* painful is now a clean module; nothing was
  decoupled speculatively.
- No invariant weakened, no Do-NOT-touch subsystem rewritten, no extension machinery
  shipped.

## Open decisions (resolved by the Phase 1 map, not now)

- Which features (if any) reach Phase 3. Decided by the map, gated on "tangled AND
  painful", not pre-committed.
- Exact consolidation target when two redundant paths exist. Decided per finding.
