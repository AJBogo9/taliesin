# Reduction Audit (Phase 1) Implementation Plan

> **For agentic workers:** This is the analysis phase of the reduction-and-modularity
> pass ([spec](../specs/2026-07-17-reduction-and-modularity-pass-design.md)). It
> produces a classified map, not code. The TDD task template does **not** apply here;
> it applies to the follow-up Phase 2/3 code plan, written against this map's findings.

**Goal:** Produce one classified map of every feature/module in the codebase, sorting each into dead / redundant / tangled / clean / must-stay-native, with per-item evidence, so the owner can decide what to delete, consolidate, or decouple.

**Architecture:** Read-only parallel fan-out. Five explorers each own a disjoint code area and classify its features against the whole 71-doc corpus and whole-repo call sites. Their partial maps merge into one document. No code changes in this phase; the owner reviews the map before Phase 2/3 is planned.

**Tech Stack:** Rust (taliesin-core, taliesin-server), vanilla client JS, bundled CSS/JS assets. Evidence tools: ripgrep/grep, git log/grep (read-only), corpus docs under `corpus/`.

## Global Constraints

- **Read-only.** This phase creates/modifies/deletes no source file. The only file written is the map itself, by the orchestrator (not the explorers).
- **The 5 buckets** (exactly one per item): **dead** (unreachable / never called / no corpus doc); **redundant** (two+ paths to one result); **tangled** (used, but coupled into core so core edits ripple); **clean** (used, self-contained module); **must-stay-native** (block model, numbering, includes, cite, exec/freeze/kernel, warm-page eviction).
- **Must-stay-native is fixed by fiat** (the existing Do-NOT-touch set): block model (`data-block-id`/`data-sourcepos`/`data-source-file`), the numbering scanners, `includes.rs`, `cite.rs`, exec/freeze/kernel, and warm-page eviction (`MAX_WARM_PAGES` + `serve_site/exec_pool.rs` LRU). Explorers do not recommend rewriting these; they may still flag a genuinely dead sub-part.
- **Corpus is the usefulness oracle.** The 71 `corpus/**/*.tmd` docs are the author's real writing. A feature no corpus doc exercises is a strong dead/extension-candidate signal; a corpus doc named for a feature is near-proof it is live.
- **Grep-trap guardrails** (project scars): a bare token matches substrings (`pen` matched "happens"); check `crates/*/tests/` first (a pin named for the feature ≈ proof it is live); `grep -c` counts lines not matches; `grep | head || echo` never fires (pipeline exit is head's); assets live under `crates/core/assets/`, not repo root; `git log -S` misses multi-line exprs. Trust the running product and tests over any note.
- **Output schema per explorer** (so partials merge): a markdown table with columns `Feature/Module | Files (path:line) | Bucket | Corpus coverage | Reachability | Coupling | Recommended action | Confidence (H/M/L)`, plus a short prose summary and a "surprises / cross-area flags" note.

---

## Area assignments (disjoint, one explorer each)

- **A1 core/render** — `crates/core/src/render/**` (mod, model, emit, text, fm_extract, validate, divs, figure, deck, theme, page, extension/) + `math.rs`, `highlight.rs`.
- **A2 core/site** — `crates/core/src/site/**` (mod, card, feed, llms, meta, chrome, links, book, search, xref, backlinks, config/, frontmatter) — every multi-page/site-chrome feature.
- **A3 core/cite + diagnostics + text infra** — `crates/core/src/cite/**`, `crates/core/src/diagnostics/**`, `includes.rs`, `frontmatter.rs`, `prose.rs`, `vocab.rs`, `schema.rs`, `agents.rs`, `ext.rs`, `diff.rs`, `hash.rs`.
- **A4 server** — `crates/server/src/**` (main, cli, serve/, serve_site/, exec, freeze, kernel, build, publish, check, query, mcp, minify, interpreter, warm_pool, protocol, build_budget, log).
- **A5 client + assets** — `web-client/**` (client.js, search.js, toc-spy.js, toc-sheet.js) + `crates/core/assets/js/**` + `crates/core/assets/css/**`.

## Tasks

- [ ] **Task 1: Fan out five read-only explorers**, one per area above, each returning a classified table in the shared schema. Explorers run concurrently.
- [ ] **Task 2: Merge partials into one map** at `notes/2026-07-17-reduction-audit-map.md`: dedup cross-area items, reconcile conflicting buckets, sort by bucket then confidence, and add a top summary (counts per bucket, the highest-confidence deletions, the tangled-and-painful shortlist for Phase 3).
- [ ] **Task 3: Orchestrator verification pass** — spot-check every High-confidence "dead" and "redundant" call directly (re-grep call sites, confirm no corpus doc, check `crates/*/tests/`) before it can reach Phase 2. Downgrade anything that does not survive. Record what was checked.
- [ ] **Task 4: Owner review gate** — present the map; the owner rules on what proceeds to Phase 2 (safe deletions/consolidations) and whether any feature is worth Phase 3 decoupling. No code changes before this gate.

## Next (out of this plan's scope)

After the owner reviews the map, write `docs/superpowers/plans/2026-07-17-reduction-phase2-3.md` with concrete TDD tasks (exact files, real deletions/extractions) grounded in the confirmed findings.
