# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git, [AUDITS.md](AUDITS.md) and
> [ROADMAP.md](ROADMAP.md); **delete an item when it lands**, never leave a `[x]`. Method lessons
> that outlive their item go to [LESSONS.md](LESSONS.md). "Do not re-add / re-scope" is a compact
> anti-rot guard — **one line per entry**, not a changelog. This file was 1,767 lines on
> 2026-07-29 because that rule was not enforced; if it is growing again, the fix is to move detail
> out, not to add a summary at the top.

## Now

**Fresh session with no context: read this section, then "Standing constraints", then P1. That is
enough to start.**

- **Ask git, never this file, for git state.** No SHA, branch name or commit count is recorded
  here on purpose: the author and parallel sessions both push, and a recorded SHA is the line that
  rots first.

  ```sh
  git log --oneline origin/main..HEAD   # what is unpushed
  git branch -vv                        # what branches still exist
  ```

- **Items 178 + 177 (LSP editor ergonomics) shipped on 2026-07-30** against
  [2026-07-29-lsp-editor-ergonomics.md](../docs/superpowers/plans/2026-07-29-lsp-editor-ergonomics.md),
  all 8 tasks, `./tools/gates.sh` green. The plan and its spec stay in the repo as the record of
  *why*; nothing on the board depends on them now. Four new LSP capabilities (inlay hints,
  folding, document highlight, selection ranges) plus visible math delimiters, and item 178's open
  question is **answered with a number**: one `publish` on the largest page of the 25-page guide is
  33 ms in a debug build against a 120 ms debounce window, so debouncing alone was enough and the
  anchor scan needs no memo. Ideas 67 and 72 remain parked in
  [FEATURE-IDEAS.md](FEATURE-IDEAS.md) Session 3.

- **The board was refilled on 2026-07-29 by an owner ruling.** Every feature parked in the old
  "Tier 3, demand-driven" tail was reviewed with the author and **promoted**. That includes the
  **print/PDF track**, which the author had been cool on and is now warm to, so its Wave 5
  deferral no longer holds. P1 is therefore a **ranked build queue**, not a drained board;
  take from the top. Five of the promoted items (153-157, the explorable cluster) shipped on
  2026-07-29.
- **Exactly one thing was declined:** the FL-weather Quarto migration, which is now the sole
  line in the demand-driven tail.
- **Everything below P2 is still blocked** on an owner ruling, a device, or a real user. The
  audit slate is complete except **R12** (real-device mobile, Android, needs the author's
  phone), and **no new round should be opened**: an audit's value decays to zero if its findings
  never ship, three waves of them have now shipped, and the P1 queue is now the work.
- **Nothing is owed by the author** except R12 and the rulings in P3.
- **Two measurement hazards, both of which have cost time.** (1) `target/release/taliesin` is
  shared across sessions and may be built from another branch — check `taliesin --version` against
  your own HEAD before trusting any CLI number. (2) A table-shaped probe whose every cell is
  negative is a **broken probe** until proven otherwise; carry a known-positive row. The full trap
  catalogue is [LESSONS.md](LESSONS.md), and it is worth reading before writing any probe or pin.
- **Entries rot; the rule for reading one is in "Standing constraints" below.** It has now been
  vindicated on three consecutive batches — most recently item 151, whose filed cause was flatly
  false while its symptom was real.

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen.
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit KEEPs
  live in that file. Every change stays invariant-safe: no CDN, no preview write-back, no new output
  format, offline bundling, `--tali-*` tokens only.
- **Author policy:** feature-first (finish framework features before marketing-site work).
- **Working method:** branch per feature; brainstorm if there's a fork; spec under
  `docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; **delete the item here when it lands.** Push to
  `origin/main` only when the author asks. **Review subagents get a git worktree or you commit
  first** (a "read-only" reviewer with `Bash` still writes scratch files to your CWD; one ran
  `cat > Cargo.toml` in the repo root and destroyed the workspace manifest).
- **Tests: three gates, or the suite silently under-tests itself:** `TALIESIN_REQUIRE_NODE=1`,
  `TALIESIN_R=R TALIESIN_REQUIRE_R=1`, `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1` (a missing
  interpreter must be a hard fail, not a skip). `cargo test` aborts the remaining binaries at the
  first failure, so re-run before trusting a total. A **fourth** gate nothing else runs:
  `TALIESIN_REQUIRE_CHROME=1 --test read_run_js`.
- **A red `exec`/`kernel` probe is now real signal, not a coin flip.** The flake was fixed 2026-07-25
  (a port race in `prepare_connection`; the re-roll now lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive). Verified 0 failures in 45 post-fix runs under the same load.
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks. **Use it instead of minting
  a fixture.** It is a *documentation* book, not a scale fixture: do NOT grow it toward 200 pages and
  do NOT mint `corpus/longbook` (the walker renders every corpus doc on every `cargo test`).
- **Git:** `git reflog show origin/main` before believing any "not pushed" claim in any notes file.
- **How this file lies to you:** entries rot. Before picking an item, **grep its named symbol/flag in
  source** and prefer measuring the running product over reading this file. Trust an item's
  *symptom*, never its cause, line number or stated cost. Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. **What would ship silently is tracked
  per class in [DETECTION-DEBT.md](DETECTION-DEBT.md)** — a live register, updated in the same
  change as the fix, not a dated findings doc. **The full trap catalogue — probes,
  instruments, cargo-mutants scoping, the coverage illusions — is in [LESSONS.md](LESSONS.md); read
  it before writing a probe or a pin.**

## Open items

**Ranked by what a session should pick up, not by theme.** P1 is buildable today; P2 is filed so it
is not rediscovered as a defect; P3, P4 and P5 are blocked and are listed so they are not
re-scoped. **Item numbers are stable** and are referenced from the findings docs and
[AUDITS.md](AUDITS.md): they are never renumbered and a closed item's number is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation* (restore the bug,
watch the named test fail), browser-verify anything client-side, and **delete the item from this
file when it lands**.

### P1 — build now

**A ranked build queue, not a menu: the order below IS the priority order.** Take from the top.
The ranking encodes three things: **dependencies**, **size** (cheap
substrate and small wins first, then the two large swings), and the author's
standing **feature-first policy** (170, the marketing site, is deliberately last).

**Items 153-174 were promoted on 2026-07-29 by owner ruling** from the demand-driven tail, where
several had sat since 2026-06-24; **153-157 shipped the same day** and are gone from this
list. Each keeps a **pointer** to its design detail in
[ROADMAP.md](ROADMAP.md) or [FEATURE-IDEAS.md](FEATURE-IDEAS.md) instead of re-expanding it here;
that is the anti-bloat rule this file exists under. Two standing conditions apply to all of them:

- **Each still owes a corpus pin doc** (corpus-plus-roadmap: a capability ships pinned by a target
  corpus document added in the same change). Where the pin is already named upstream it is
  repeated below. **Do not grow `corpus/` past the pin a feature needs**, the walker renders every
  corpus doc on every `cargo test`.
- **Promotion is not a design.** Several of these were parked with an open design question, not
  just for lack of demand (166's line-shift problem, 160's source-map gate, 155/156's reactive-VM
  trap). Those say so; brainstorm before coding.

150. **Phase A2: site-aware in-editor preview — the WIRING half only; the risk half shipped
     2026-07-29.** (MEDIUM.) **The spec is written and its facts are verified:**
     [2026-07-29-site-aware-in-editor-preview.md](../docs/superpowers/specs/2026-07-29-site-aware-in-editor-preview.md).
     Read it before writing code; do not re-derive.
     - **Done:** the staleness bug this item called "the risk". `data-source-file` is relative to
       the *currently loaded* page, and the host resolved it against the document the preview was
       *opened for* — so after a cross-page navigation a click on chapter B opened chapter A's
       same-named file (a real file, no error). The page now sends `base_dir`/`doc_path` with each
       `tali-goto` and `resolveSourceFile` prefers it, back-compatible both directions.
       `projectRootFor()` (nearest `_site.yml`, never `.git`) also landed with tests.
     - **Left:** spawn `taliesin preview <root>` instead of the file and open the page's URL from
       `map --format json`; key `PreviewRegistry` by project root so one server serves a whole
       book; and `relativeKey`'s mirror problem — reverse sync must *select the page* (a new
       `tali-navigate` host→iframe message) before marking the block. §1-4 of the spec.

163. **Site-level shared bibliography + hygiene.** (M.) `bibliography:` is per-document only, so
     a growing blog retypes keys per post and nothing reports an unused or duplicate entry. Allow
     `bibliography:` in `_site.yml`, merged **under** each page's own, plus two **read-only**
     diagnostics ("entry never cited", "duplicate key"). *Explicitly does not touch the BibTeX
     parser or the CSL formatter, which are Do-NOT-touch-for-rewrite.*

162. **Session revision digest.** (M. [FEATURE-IDEAS.md](FEATURE-IDEAS.md) #36.) Surface the
     `BlockOp` stream the client already receives: a session word delta (`+340 / -180`) plus a
     feed of the last N ops, each click-to-source, so an edit answers "what did that actually
     do?" instead of being console-only. Makes the moat **visible**, which is worth as much to
     150's marketing half as to authoring. Honest caveat carried from the parked entry: the pin
     is behavioural (a `tools/live-edit-bench` assertion), not a corpus doc.

161. **Author structure panel.** (M/L, and **smaller than when it was parked**.
     [FEATURE-IDEAS.md](FEATURE-IDEAS.md) #26.) A read-only preview sidebar: heading tree with
     per-section word count and a badge per node for unresolved xref / TODO / over-goal length,
     click to scroll. This is the *revision* view, not the reader TOC. **Re-scope before
     building:** the heading-tree half is now largely free from the shipped LSP
     (`textDocument/documentSymbol`, `lsp_outline.rs`), so the unique value left is the
     **annotation layer**. Scope it as an annotation layer on the dev panel or it grows to L.

160. **Block-level transclusion** `{{< include file.tmd#sec-id >}}`. (M, needs-care.
     [FEATURE-IDEAS.md](FEATURE-IDEAS.md) #28.) Pull one anchored section instead of a whole
     file, so a shared derivation lives in one place across a post series without copy-paste
     drift. **Must ride on top of the `includes.rs` source-map pass** (resolve the fragment to a
     block range, hand the existing machinery a sub-slice), never rewrite it: `includes.rs` is on
     the do-not-rewrite list. **Hard merge gate: the source map must not perturb.**

165. **Companion Phase 2: editor commands.** (M. [FEATURE-IDEAS.md](FEATURE-IDEAS.md) #31/#33.)
     Insert block, reorder slide, move/promote/demote a heading section, strictly as `.tmd`-buffer
     text transforms in the **editor**, never preview gestures. This is the *legal* replacement
     for the drag-to-reorder that was removed for breaking single-editing-surface. **Cap the
     command set**: this is the named route by which the companion metastasizes into WYSIWYG.
     Note #32 (rename label) already shipped in the LSP, so it is out of scope here.

166. **`.tmd` format-on-save for PROSE.** (Open design question, and narrower than it was.) The
     table-only formatter shipped 2026-07-28 (`crates/server/src/lsp_format.rs`,
     `textDocument/formatting`), and it **sidesteps** the recorded objection rather than
     answering it: a table's rows map one-to-one onto its lines, so the replacement has exactly
     the line count of the range it replaces and no `data-sourcepos` below it moves
     (`formatting_never_changes_the_line_count` pins that). **A prose pretty-printer still has
     the original problem**, reflowing a paragraph moves every line after it. **Brainstorm the
     line-shift answer before any reflow code.**

168. **`build-seo-completeness`.** (LOW value tag upstream, small.
     [ROADMAP.md](ROADMAP.md) Pillar V.) At publish time, when `url:` is set, emit `sitemap.xml`
     + `robots.txt` + `Article`/`WebSite` JSON-LD, reusing the existing nav `_`/`.`/`draft:`
     exclusion. Build-time metadata files, HTML-only intact. **Author flagged this as
     publish-critical on 2026-07-29**, so it outranks its upstream "low" tag. Pairs naturally
     with the auto social-card idea ([FEATURE-IDEAS.md](FEATURE-IDEAS.md) #58), which is NOT in
     scope here.

169. **Image optimization.** (Large.) WebP/AVIF transcode + responsive `srcset` + lazy-load
     behind a content-hashed asset cache. Parked as "deferred until posts get image-heavy";
     **author flagged it publish-critical on 2026-07-29.** Split from `image-lightbox`, which
     shipped. Watch the payload and the build-time cost.

159. **Print/PDF track.** (Large. **The deferral is lifted: the author warmed to it 2026-07-29.**
     [ROADMAP.md](ROADMAP.md) Pillar IV / Wave 5 is the frame, and
     [FEATURE-IDEAS.md](FEATURE-IDEAS.md) #57 is the substance.) A paged-media rendering
     **derived from the built HTML**, HTML staying the single source of truth: `@page` running
     chapter and section heads (`string-set` + `running()`), real folios, `@fig-`/`@sec-` refs
     that become "Figure 3 (p. 12)" via `target-counter()`, auto list-of-figures and index with
     true page numbers, widow/orphan control and optical hyphenation; paged.js (vendored) where
     native paged media is absent; `{js}` and video degrade to a poster frame. Pin:
     `corpus/print/paged.tmd`. **The line, restated because this is the item most likely to
     cross it:** the moment it forks into a separate Pandoc/Typst/LaTeX path it has violated
     HTML-only. It is a *rendering of* the build artifact, not a second compiler target.

158. **Opt-in Pyodide `{python}` cells.** (L, needs-care. [FEATURE-IDEAS.md](FEATURE-IDEAS.md)
     #66. **Its dependency on 153 is discharged: the registry shipped 2026-07-29**, so this
     is now a `registerLanguage` call plus the bundle question.) Client-side `{python}`
     backed by Pyodide, feeding the reactive
     graph like any cell, so a published document stays interactive with numpy/scipy and no
     kernel. This is what JupyterLite is. **Bundle guard is the whole risk:** Pyodide is 10 MB+,
     so opt-in per page and vendored offline. Known caveats: **no torch**, and a real cold-start
     cost. Registry graduate, so it must land as a *registration*, not as surgery — the seam is
     `window.taliJs.registerLanguage` (client) + `render/client_lang.rs` (server), and `{glsl}`
     is the worked example of using it.

164. **`docs-as-spec`.** (L. [ROADMAP.md](ROADMAP.md) Pillar V.) Promote the two dogfooded books
     to a versioned normative spec: an RFC-2119 `.tmd`-dialect reference plus a WebSocket
     protocol reference. Upstream says start only once the validation epic has settled, which it
     has. Value is credibility and adoption, not capability.

167. **`check --online`.** (Opt-in.) Dead-link checking as the **single sanctioned network
     call**; the default stays offline, deterministic, kernel-free and network-free. **Scope
     note:** the *citation/DOI-existence* half of this was separately **declined 2026-07-16** and
     is not revived by the 2026-07-29 promotion; if it is ever wanted it needs its own ruling.

172. **`taliesin publish` follow-up: an optional `--init` wrapper** for the one-time `wrangler`
     setup. (S.)

171. **An end-to-end live-HTTP test for `mounts:` serving.** (S, test debt.) The F-04 work
     unit-pins the pure `match_mount`/`resolve_project`/`classify_change` helpers and live mount
     serving is browser-verified; what is missing is only the bin-crate gap of a real
     `reqwest`/`TcpListener` harness. Mounts are preview-only.

173. **PMF reader tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C): a
     document-level reader show/hide-code toggle, a reader code+data download affordance, and
     instant client-side navigation polish. Three small reader affordances, promoted together.

56. **L5-1 residual: the manual's cross-page references.** (The `description:` half shipped
    2026-07-26: 0 of 36 tracked pages → 36 of 36.) What is left is not the authoring pass the item
    assumed, and splits two ways:
    - **Glossary, term index and float digest have no surface to feed.** `glossary`, `term-index` and
      `float-digest` grep to **zero** across `crates/core/src` + `crates/server/src`, so "they render
      empty until an authoring pass happens" describes a *feature proposal*, not authoring work.
      Writing `{.definition}` blocks today feeds only `skim.rs`, which reads them as statement heads.
    - **Backlinks ship and render nothing, and authoring genuinely could fix that.**
      `site/backlinks.rs` builds its reverse index from **cross-page** xref markers; the books' 33
      xrefs (17 guide + 16 internals) are all intra-page, so **0** "Referenced by" lines are emitted
      in either book. Real cross-chapter references would light it up, but they have to be references
      someone means — a writing judgment, not a sweep.

174. **`serde_yaml` fallback swap.** (Conditional: **this one has no trigger yet**, and is ranked
     last-but-one for that reason.) The `Cargo.toml` workspace comment names `serde_yml`, which
     carries RUSTSEC-2025-0068 (unsound + unmaintained); `serde_norway` is over a year stale. The
     maintained continuation is **`serde_yaml_ng`** (v0.10). No urgency: config is trusted and
     local, and 0.9 still builds. **Act when** 0.9 breaks against a future serde or edition, and
     gate the swap on a test that `Error::location().line()` still works (the front-matter
     linter's located diagnostics depend on it). Fix the stale comment whenever this file is
     touched for any other reason.

175. **The long-running-cell workflow (computationally heavy / ML notebooks).** (LARGE, four
     separable parts. **Ranking is provisional and probably too low**: it sits here only because
     the author has not re-ranked the queue, and the case for moving it near the top is that
     part (a) is a wall on *first contact* for an entire user population Taliesin otherwise
     serves. Filed 2026-07-29 from an author-raised direction, evidence re-derived from source,
     not from a prior note.) Jupyter's daily-driver property for expensive work is "watch it
     run, then re-run only what you choose". Taliesin currently has neither half. **Verified:**
     - **(a) The default cell timeout is 120 s** (`kernel.rs:29-34`) and expiry produces a SIGINT
       with "no result", not a traceback (`kernel.rs:531`). A 40-minute training cell dies at two
       minutes until the author discovers `TALIESIN_CELL_TIMEOUT`. **The fix is a better default,
       not a knob** (minimal-config convention): cap on *silence*, not wall-clock. A cell printing
       an epoch line every 30 s is alive; a cell silent for 10 minutes is the real runaway. The
       machinery already exists, `kernel.rs` (~line 915) tracks last-iopub-arrival for an uncapped
       "no output for 60 s" signal and only warns with it. **Verify that tracker's exact current
       role before speccing**, this entry asserts it from one comment.
     - **(b) There is no streaming output.** `exec_cell` awaits `kernel.execute(code)` and gets the
       *complete* output vector; `kernel.rs` collects iopub until the kernel returns to idle. So a
       long cell shows a `⏳` elapsed badge and nothing else: no tqdm bar, no epoch log, no partial
       figure. This is the biggest gap of the four, and it fits the existing protocol shape (a
       `cell-output-append` message beside `cell-state`).
     - **(c) No escape hatch for an expensive cell.** Freeze keys on a cumulative hash (this cell +
       all upstream same-language code + interpreter id), so editing *any* upstream cell busts the
       expensive one, and the full cell-option set (`validate.rs:18`) has `cache: false` to opt
       *out* but nothing to opt *into* stickiness. Proposal: a `#| checkpoint:` cell whose freeze
       entry survives an upstream edit **but renders a visible stale-relative-to-inputs marker**,
       with `build` warning or refusing. This is the differentiator, not a copy: **Jupyter lets the
       notebook lie silently; this would let you defer the re-run while showing the debt.**
     - **(d) No per-cell run and no interrupt.** The preview can only `restart_kernel`
       (`serve/mod.rs:43`), which nukes all state. Per-cell run/interrupt belongs in the **editor**
       (CodeLens), not the preview: it keeps single-editing-surface clean and avoids a second
       control surface in the browser. **Depends on (b)** and is specced with the editor-ergonomics
       work, do not build it twice.
     - **Pin problem, name it before coding:** the corpus walker renders every corpus doc on every
       `cargo test`, so the pin must *exercise* streaming and the liveness cap without being slow.
       A cell that emits N lines with sub-100 ms sleeps, not a genuinely long job.

176. **Dataset provenance (`datasets:` / `{{< dataset >}}`).** (M. Filed 2026-07-29 alongside 175,
     same author-raised direction.) **Verified:** `copy_local_assets` only follows `src=`/`href=`
     in the emitted HTML (`build.rs:761`) and warns-and-skips anything outside the doc tree
     (`build.rs:769`), so a `data/train.csv` referenced only inside a `{python}` string is
     **invisible to the build**: not copied, not validated, not mentioned. **Bundling is the wrong
     default** (a multi-GB parquet in a book folder), and so is the status quo. The real failure is
     that a reader cannot re-run the book, so the web-native answer is *provenance, not a blob*: a
     front-matter `datasets:` block or a `{{< dataset data/train.csv >}}` shortcode rendering a
     card with name, size, sha256, licence, and either a download link (small, in-tree) or a fetch
     snippet plus checksum (large, remote). Gives the build something to validate against, which
     turns "my figure changed and I do not know why" into a diagnostic. **Explicitly out of scope:**
     shipping large data inside the built book. Pin: one corpus doc with a small in-tree CSV and one
     declared remote entry. *Touches the drop-a-`.csv` gesture in the editor-ergonomics spec; that
     gesture should insert the card plus a loader cell, so land this first.*

170. **Marketing site.** (Last by the author's own standing feature-first policy, which is why it
     sits below everything above it even though parts are buildable today.) The
     `live-edit-hero-demo` clip (= `ROADMAP.md` Wave 2's unshipped deliverable), swapping the
     `site/_site.yml` placeholders, a demo-led hero rebuild, mobile embed refinement, and deploy.
     **The deploy half is additionally flip-gated** and overlaps item 149's launch-presentation
     group in P3; do not build the same thing twice from both entries.

### P2 — filed so it is not rediscovered as a defect

Not worth a session on its own. Each is a record or a known cost, not a task.

131. **The cold-build cliff: 3,981 ms vs 789 ms warm.** (LOW, and probably correct as-is.) Filed so
     it is not rediscovered as a defect. Kernel *variable* state is never cached — the property that
     makes the cache trustworthy — so a cold start genuinely cannot skip work unless the whole
     document is unchanged. **The waste is inherent to a correctness guarantee worth keeping.**

129. **Shape inventory from two real external documents — the durable half of R11.** (MEDIUM, mostly
     a record.) What real documents contain that `corpus/` has nowhere: `lang,attr` fences (734
     occurrences → item 127), ` ```console ` (209), links with a non-`.tmd` extension (128 → item
     128), a `SUMMARY.md`-driven chapter spine, **112 pages in one flat directory** (the largest
     corpus project is 14), and chapter files with **no front matter at all**. **Do NOT grow
     `corpus/` toward these** — the walker renders every corpus doc on every `cargo test`. **Pin
     only the two that earned it** (127 and 128) — **both pinned and shipped 2026-07-28**: the
     `lang,attr` fence is now a fixture in `corpus/highlight.tmd` with its own test, and the
     link-extension shape has `crates/core/tests/migrated_link_extensions.rs`. The rest are
     recorded so a later round does not re-derive them.

152. **RESOLVED 2026-07-28: the companion e2e suite runs again.** It had been failing with
     `EMFILE: too many open files` inside VS Code startup because `fs.inotify.max_user_instances`
     was still the kernel default of 128 while the desktop session already held ~154 (dconf ~40,
     code ~32, plus Electron apps). Raised to 512 via `/etc/sysctl.d/99-inotify.conf`. Kept here
     as the diagnosis, because the same limit throttles `taliesin preview`'s own file watchers:
     if previews ever stop hot-reloading, or VS Code refuses to start, check
     `find /proc/*/fd -lname 'anon_inode:inotify' | wc -l` against
     `/proc/sys/fs/inotify/max_user_instances` before suspecting the code.

### P3 — blocked on an owner ruling (not a task until then)

100. **RULED 2026-07-28 — the answer is "archive plus fresh public", and it is specced.** See
     [2026-07-28-public-flip-audit-design.md](../docs/superpowers/specs/2026-07-28-public-flip-audit-design.md).
     The ruling threads the needle both routes below missed: **the history IS published** (1,608
     single-author commits are the evidence a grant applicant wants), and the private planning docs
     leave *every* commit that ever held them. Mechanism: relocate the purged docs to
     `~/Documents/personal/taliesin-private/`, rewrite history, rename this remote to
     `taliesin-private-archive` (stays private, complete backup), create a **new public**
     `AJBogo9/taliesin` and push the rewritten history there. No force-push, no destructive remote
     op, and the private blobs never reach the public repo at all. Zero forks and never having been
     public is what makes it cheap. **Kept, not purged:** security audits, `.claude/`,
     `docs/superpowers/`, `AGENTS.md`, `LESSONS.md` — for the stated goal those are the exhibit.
     **Purged:** money and strategy documents only, plus ~11 commit subjects that name them.
     **Execution status: NOT STARTED and not to be started without a separate instruction.**
     Phase 1 is a read-only audit and is safe whenever wanted; **Phase 2 is irreversible** and is
     additionally gated on Phase 1's findings being signed off.
     **What still lands on this item:** the spec's own D-checks (incl. the provenance check on
     corpus documents), and its rule that any still-open finding reading as an **exploit recipe** is
     reported for individual judgement, default keep — which is exactly items **79, 80, 81**, so
     **fix those before Phase 2** rather than deciding whether to redact them. **All three are
     FIXED (2026-07-28, `launch-blockers-2026-07-28`, unpushed)**, so this clause is discharged once
     that branch is merged — verify, do not trust this line. **New input for Phase 2 from item
     83:** five local tags ship an MIT `LICENSE` and none has been pushed, so whether tags travel
     to the new public repo belongs to this spec.
     *Original framing, kept because it records why the ruling was hard:*
     `notes/STARTUP-PLAN.md:126` records a plan to publish as a **fresh repo with no history**
     ("Keep this repo private forever; the public one is a separate repo"), *not* to flip this
     repo's visibility. Those two routes resolve different findings, so the prune work cannot be
     scoped until this is ruled. Two hard facts either way:
     - **`notes/FUNDING-RESEARCH.md` and `notes/STARTUP-PLAN.md` are git-TRACKED while their own
       text says they must not be** (`FUNDING-RESEARCH.md:4` "keep this file out of";
       `STARTUP-PLAN.md:119` "remove anything private: `STARTUP-PLAN.md`"). They carry the ***REMOVED***
       analysis, a table of named funders being skipped and why, a funder's contact address, and
       "***REMOVED***". The 2026-07-17 round already filed this and
       recorded the prune as **not done**.
     - **A fresh `git init` fixes none of the tree-level findings** and discards the 1,573-commit
       process record, which is the strongest evidence an individual grant applicant has. The
       due-diligence doc's §6 proposes a third route (targeted `filter-repo`) with its honest cost.
     Supersedes the "flip-day artefact checklist" framing; **extends item 25, does not replace it.**

102. **Decide what to do about constructs that render elsewhere and silently do not here.**
     (Ruling.) Detail in [adoption friction](2026-07-27-adoption-friction-audit.md).

103. **Clear the name in software classes before the flip.** (Ruling, legal not code.) Trademark
     search in the relevant classes; the name is the retained optionality per the product stance.

148. **Distribution: the binary channel now has a MECHANISM but still has no artifact; the package
    managers are untouched.** **Amended 2026-07-28 by item 92** — read this before re-filing any of
    it. What shipped: `.github/workflows/release.yml` builds Linux x86-64, macOS arm64 and macOS
    x86-64 on a `v*` tag, attaches a tarball + `.sha256` with `LICENSE` + `THIRD_PARTY.md` inside,
    and the README states the matrix (Windows explicitly unsupported). **What is still true:** no
    tag has been cut and the workflow is guarded inert until the repo is public, so `gh release
    list` is still empty and there is still nothing to download **today**; crates.io `taliesin` /
    `taliesin-core` / `taliesin-server` are all still 404 (all three names free); no Homebrew, Nix,
    or install script. The remaining work is therefore **cut a tag after the flip**, then decide
    about crates.io / brew / nix separately.
    - **Cold-build cost re-measured 2026-07-28: 2m11s, 268 crates, 2.6 GB peak RSS at `-j4`**, for
      one ~38 MB binary; the README now states this. The filed **2m59s** was a different machine or
      job count, not a regression. Either way the argument stands: the audience for a documentation
      tool is not the population that will install a Rust toolchain and wait it out, which is
      exactly why the release workflow exists.
    - **Prerequisite the critic missed and the defender found:** `cargo publish` will *reject*
      this workspace as-is. `Cargo.toml:14` declares `taliesin-core = { path = "crates/core" }`
      with **no `version`**; add `version = "0.2.0"` first.
    - Also blank on crates.io without it: no `keywords`, `categories`, `readme`, `homepage` or
      `documentation` in any manifest, so the crate pages would carry one description line and
      nothing else. Watch `crates/core` = 7.3 MiB tracked against the 10 MiB `.crate` cap.

149. **Launch presentation, all gated on the flip.** Grouped because none is actionable until the
    repo is public, and each is small once it is.
    - **The README does not lead with the speed moat**, contradicting this file's own ruling at
      `:577-579`. "Quarto" appears **zero** times in the README and in `site/*.tmd`, and
      `tools/live-edit-bench/RESULTS.md` (cold 123,994.9 µs vs warm 28,425.1 µs, diff 685.6 µs,
      83× smaller payload) is cited from nowhere. Note the ruling says *lead with the moat*; it
      does not say *name Quarto* — that inference is the critic's.
    - **The GitHub repo is a dead first impression**: description defines Taliesin in terms of
      Taliesin, `homepageUrl` empty, one topic ("rust"), zero releases, and the README's only
      image is the licence badge — while four screencasts demonstrating the moat sit committed
      in `site/assets/` and appear on no page a visitor sees. (They are MP4; a GIF conversion or
      an uploaded asset URL is needed, not a one-line embed.)
    - ~~**No platform statement anywhere**~~ **DONE 2026-07-28 (item 92).** The README carries a
      platform matrix naming the three built targets and stating Windows unsupported, and
      `release_targets.rs` pins it against the release workflow in both directions. The underlying
      fact is unchanged and still worth knowing: `/proc` is read directly in five places with
      `#[cfg(not(unix))]` fallbacks that `LESSONS.md:88` records as never executed by any test.
    - **CoC and issue templates only** — ~~CONTRIBUTING / CLA or DCO~~ **DONE 2026-07-28 (item
      89).** `CONTRIBUTING.md` exists and its clause 3 is the inbound grant, explicitly including
      relicensing, so `README.md:156-158` is no longer ended by the first merged outside PR;
      `gate_script.rs` fails the suite if that grant disappears. Still absent: a code of conduct and
      GitHub issue templates, both of which are only worth doing once the repo is public.
    - **`taliesin.dev` resolves to nothing** (registered, NS + SPF + a google-site-verification
      TXT, zero web records) and is baked into every canonical URL, `og:url`, sitemap and feed.
      `site/README.md:11-12` already flags it as a placeholder.
    - ~~**`taliesin build site` 404s its own primary CTA**~~ **DONE 2026-07-29.** `build --strict`
      counts each mount warning as a problem (site/: exit 0 → 1) and `check` reports
      `TAL-MOUNT-PREVIEW` per mount at severity `suggestion`, so neither gate blesses the deploy
      any more. `site/build.sh` builds all 8 projects into one tree (verified: `/`,
      `/docs/guide/`, `/docs/internals/`, `/gallery/*` all serve 200 where a plain build 404s
      three of them), pinned against `_site.yml` in both directions by `site_build_script.rs`.
      **The ordering is load-bearing and the test pins it:** the parent build's `sweep_stale`
      deletes anything under the output dir it did not write, so a mount built first is silently
      swept away — and re-running `taliesin build site` alone afterwards puts you back to the
      broken tree.
    - **The name** (surfaced, not a task): TALIESIN is a live registered mark of the Frank Lloyd
      Wright Foundation (Reg. 4150375). Software is outside the recited goods so legal risk is
      low; the cost is permanent SEO invisibility, and `github.com/taliesin` + `/taliesins` are
      both taken. Renaming twice is worse than a bad search name — if keeping it, always publish
      as "Taliesin — the `.tmd` dev server" so the disambiguator travels.

25. **Pre-public release: the flip procedure, and a contradiction to resolve first** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md) and
    [2026-07-28-launch-critique.md](2026-07-28-launch-critique.md)). All five code items shipped
    2026-07-25. **oss-4 was ruled 2026-07-25: deferred** ("I'll do it at the end of summer").

    **Author leaning, 2026-07-28 (a leaning, NOT a ruling — re-confirm before acting):** do a
    **visibility flip** with the sensitive documents removed, deliberately keeping the commit
    history public so readers can see how the work was done.

    **The one fact that decides whether that plan works.** A visibility flip exposes **every past
    commit**, and `git rm` in a new commit does not remove a file from history. Two documents are
    tracked and both instruct otherwise in their own headers —
    `notes/STARTUP-PLAN.md:3-5` ("keeping it out of any public release") and
    `notes/FUNDING-RESEARCH.md:4` ("keep this file out of git") — and they contain the ***REMOVED***
    ***REMOVED***, and "MIT
    would let a competitor or a cloud provider close it against you. ***REMOVED***."
    So **"flip + delete the files" leaves them fully readable in history**, which is the opposite
    of the intent. Only three options actually work:
    - **(a) Flip, and rewrite history first** (`git filter-repo` over those paths). Keeps the
      visible history the author wants, at the cost of rewriting every SHA — and any SHA recorded
      in `notes/` or in a findings doc stops resolving.
    - **(b) Fresh public repo** per `notes/STARTUP-PLAN.md:111-127`, which is a *dated ruling*
      ("decided 2026-06-18") prescribing exactly this: `rsync -a --exclude='.git'`, remove the
      private docs, "Keep this repo private forever; the public one is a separate repo." Clean,
      but **discards the commit history**, which is the thing the author said they wanted to keep.
    - **(c) Flip as-is and accept the exposure.** Cheapest, and the least consistent with having
      written "keep this out of git" twice.

    **Note the procedure collision, because two committed documents currently disagree:**
    `***REMOVED*** (fresh repo), while this file and
    `2026-07-17-security-release-audit.md:217-218` both sequence the `oss-*` items to "whenever
    the repo actually flips public". Whichever option is chosen, **fix the losing document in the
    same change** or the next session will follow the wrong one.

    Still open under whichever route: whether to prune `notes/` + `docs/superpowers/`. The
    deferral's stated reason — "no secret is exposed … but it is a curated bug roadmap" —
    describes the audit notes and **does not describe the two files above**, which is why they
    were never named in it. Scale, measured: `git ls-files notes/` = 63, `docs/superpowers` = 69,
    and the largest is `2026-07-03-quarto-design-decisions-catalog.md` at **1,129,387 bytes** of
    adversarial self-critique sitting under `docs/`, which a visitor reads as "the manual".

    **Correction to this item's own former text.** It claimed "**Verified NOT open, do not
    re-scope:** … the tracked `/home/bogo` paths are scrubbed." Measured 2026-07-28:
    `git grep -Il "/home/bogo"` → **11 files**. The 2026-07-17 scrub was scoped to the four paths
    under `docs/superpowers/*` and did do that; the summary generalised it to "the tracked paths",
    and one new occurrence has since accreted (`2026-07-18-shell-completion-dynamic-design.md:189`,
    dated the day after). Eight of the remaining ten are `notes/*` prose covered by the prune
    above, and two are self-references *documenting the scrub*. Low impact — the username is
    already public via git author metadata — but **a "verified NOT open" line in this file was
    measurably false**, which is the failure mode `LESSONS.md` warns about. Still correctly
    closed: `SECURITY.md` exists, PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped
    2026-07-17, and `dos-yaml` + NET-3 were refuted.

### P4 — blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the
   phone-feed deck mode); drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor
   first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down opens the overview
   map, with a 250 ms hysteresis.)* **The device blocker is gone.** **Partly measured 2026-07-27**
   (deck × touch round): with synthetic touch events, swipe navigation works (h 0→1→0), a two-finger
   pinch-in opens the overview, and an overview one-finger pan neither navigates nor exits (B6-31
   holds). **What is still unmeasured is the part emulation cannot reach**: a real finger, and
   overview pan while zoomed *past* fit — at fit scale `clampOv` has nothing to pan, so the probe
   proved only that pan does not misfire, not that panning works. Chromium touch emulation is still
   not evidence for a pinch on glass.

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    **The `alt="output"` half SHIPPED 2026-07-29** (every executed figure now emits `alt=""`,
    pinned in `r_kernel.rs`). What is left is the theming, and an attempt on 2026-07-29 was
    **built and then reverted**, so start from what it measured rather than from the framing above:
    - **The interception point is NOT the value's repr, and NOT a global `print` override.**
      ipykernel asks the returned figure for a representation; **IRkernel does not**. Printing a
      ggplot DRAWS it, and IRkernel captures the graphics DEVICE and publishes that as `image/png`.
      Measured: `repr::repr_html(p)` returned a correct themed light/dark pair while the built page
      still carried a single un-themed `<img>`; a `print.ggplot` assigned into `globalenv()` never
      fired for an auto-printed plot (file-marker probe), though it fired for an explicit `print(p)`
      and produced two correct PNGs. Registering `repr_html`/`repr_png` for **both** S7 class names
      (`ggplot2::ggplot` and `ggplot`, ggplot2 4.0.2) did not reach it either, and pushing the
      method into base's S3 table **broke a passing test and hung the suite**.
    - **So the remaining question is a narrow one:** which IRkernel seam publishes an
      auto-displayed plot, and can it be given a `text/html` twin-PNG pair. `render_media` already
      prefers `text/html` over `image/png`, so nothing needs suppressing once that seam is found.
    - **The twin render itself is solved** and is not the hard part: a `ggplot2::theme()` override
      of the colour slots plus `ggsave(bg="transparent")` at the reader's `repr.plot.*` size
      produces two genuinely different PNGs (verified standalone). Emit the same
      `tali-fig-light`/`tali-fig-dark` pair the Python side does, so `base.css` needs no new rule.
    - **Trap, cost one debugging round:** every Taliesin page inlines `base.css`, which *contains*
      the strings `tali-fig-light`/`tali-fig-dark`, so a whole-page `contains()` for them passes on
      a page with no figure at all. Needle the full emitted `<img class="tali-fig tali-fig-light"
      alt="" src="data:image/png;base64,` tag. **Do NOT confuse this with AN-2a, which is fixed.**

18. **Demand-probe residual: the inline-SVG figure path.** (P3; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md).)
    **F-03 and F-02's documented-convention half both SHIPPED 2026-07-29** — the `{js}` once-cell
    attachment trap is in `using/interactive.tmd`, and the `<img>`-is-style-isolated rule plus the
    palette-that-works-on-both convention are in `using/theming.tmd` (with `corpus/course/likelihood.svg`
    as the worked example and `corpus/descent/landscape.svg` as the counter-example).
    **Left: F-02's other candidate, an inline-SVG figure path** so `![](x.svg)` inherits `--tali-*`.
    Deliberately not built, and the reason is the whole design problem: inlining an authored SVG
    puts its `<style>` selectors and element ids into the page, where `.label` / `.ink` / `.axis`
    from two figures collide with each other and with page CSS. So it needs a **selector-scoping
    strategy**, not a change of emitter, and that wants its own spec before any code. Edits
    `crates/core/src/render/figure.rs`.

70. **A project with no `_site.yml` declares no boundary** (P3, filed 2026-07-27 from the path-parity
    batch's "surfaced, not fixed"). `build <dir>` accepts a bare directory, so a single-document render
    of one of its pages roots at that page, and the site path's own inference can still widen to
    `.git`. Nothing can infer an undeclared boundary; the fix is for the author to declare one. Live
    instance: `corpus/posts/pca-geometry/` (the loose twin of the tech-blog page, byte-identical to it
    and pinned so by `twinned_corpus_sources_stay_byte_identical`) sits under no project marker, so
    `build` of it warns `include not resolved` — true since PT-2 shipped and **now uncovered by any
    test**, since the corpus pin moved to the tech-blog copy. Decide whether that warning is correct
    behaviour or wants a better message before writing code.

104. **Three Wave 1 items whose own round could not verify them, filed with the measurement each
     needs.** (Do not build until measured — each says so in its findings doc.)
     - **The `.gitattributes` line that makes `.tmd` behave like `.md`** on GitHub. Needs GitHub
       linguist-override behaviour confirmed; the round could not.
     - **The Jupyter on-ramp that already exists outside the project.** Needs `nbconvert` output
       confirmed to survive the rename.
     - **The scale ceiling**, measured with a **runtime-generated fixture that never enters the
       corpus walker** — deliberately shaped to respect the standing ban on growing `corpus/tarn`
       and on minting `corpus/longbook`, whose stated reason is that the walker renders every
       corpus doc on every `cargo test`.

105. **The headless `--no-sandbox` rationale rests on an assumption this round retired.** (LOW.)
     The justification assumed only author-written documents reach the headless path; item 79's
     family says otherwise. Re-derive the rationale before changing the flag.

10. **Two kernel limitations with no clean fix** (P3, dev-facing):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no `ParentPollerUnix`
      equivalent, so there is nothing to arm; PDEATHSIG is the only other lever and is hazardous. R is
      rarely the cold single-doc path, and the warm-pool, cold-Python and `/tmp`-sweep halves all
      landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`. (Not
      forbidden — the old "do-not-touch" note was the completed rewrite-scoping list, not a freeze.)

12. **i18n / Unicode: done bar a demand-driven residual.** The LSP UTF-16 fix shipped 2026-07-22
    (detail: [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)).
    *Residual, do not spin up without a real ask: RTL layout, CJK line-breaking, non-ASCII heading-slug
    collisions.*

### P5 — frozen, do not spin up

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused
  2026-07-17. Eviction drops the executor and kills its kernel child processes, so this is kernel
  lifecycle, not a constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not missing)
  interpreter costs ~161s recovery, downstream of the (bounded) `interp_id` probe in the warm-pool
  forkserver READY wait + kernel-start retries.
  `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the *missing*
  case is handled and the *hanging* one is not. `kernel.rs`/`warm_pool.rs`. *(Aside, pre-existing +
  load-bearing: `crates/server/Cargo.toml` doesn't list tokio's `process` feature though
  `kernel.rs`/`warm_pool.rs`/`exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite
  runs, only when the build is cold. Measured, unexplained, argued test-only (a real kernel has three
  reclaim nets where the stand-in has one). Worth an hour only if a real kernel is ever seen outliving
  its pool.
- **D72 bare `@key`:** declined for now (the diagnostic already ships, so nothing renders wrong
  silently, which makes it a feature question not a defect). Edits `crates/core/src/cite/`, needs
  sign-off if revived.

## Tier 3 — demand-driven (build only when a real user asks)

**This tail held 17 lines until 2026-07-29, when the author reviewed all of them and promoted
every one but the first below into the P1 queue as items 153-174.** Do not re-file a promoted
item here: it has a number now, and the number is where its detail lives.

- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test (exercises `book.rs`, includes, the freeze cache, file-mode portability). If
  it renders clean, consider pinning a reduced version under `corpus/`. **Explicitly declined
  2026-07-29** as unnecessary, and kept only because the *class* of defect it would surface is
  real (it is the same class the external-document audit found, see item 129). Revive on a
  concrete portability doubt, not on capacity.

## Audit lenses — closed, do not open a new round

[AUDITS.md](AUDITS.md) is the round index and a *record*, not a menu. The 14-round slate
([spec](../docs/superpowers/specs/2026-07-27-audit-slate-design.md)) is **complete except R12**,
real-device mobile on Android, which needs the author's phone. Its priority order is in the spec:
the book drawer scroll lock first (item 76 made the drawer a book's only nav surface), then the
`--host` QR flow, momentum scrolling and the dynamic viewport toolbar, tablet widths, TalkBack.
**Record explicitly that an Android round does not cover WebKit/iOS**, or it will later read as
full mobile coverage.

The slate's own thesis is the part worth carrying: every earlier lens asked *is this correct?*,
and asking instead whether the tool is **detectable**, **holds under stress**, would be **adopted**
and can be **handed over** produced three HIGH security findings in one pass, none of them a
correctness bug. Wave findings docs are linked from AUDITS.md. Durable artefacts, so a later round
does not rebuild them: the deck exemption register (R14), the sensitivity/tradeoff register (R6),
the D>=8 detection cluster (R7, now living in [DETECTION-DEBT.md](DETECTION-DEBT.md)), the draft
ACR (R9, now published in the guide) and the external-document shape inventory (R11, item 129).

**Two lenses remain un-run and both are blocked, not declined.** L3: `lsp.rs`, `complete.rs`,
`skim.rs` and `manifest.rs` post-date every lens that would have owned them, though the mutation
campaign has since pinned much of what one would look at. L6: a real external document, blocked on
a repository that is not on this machine.

**Never scope a round from the exemptions that are written down.** R14's premise was too generous
by an order of magnitude: the two documented `DocFormat::Reveal` exemptions turned out to be
*correct*, while the real hole was that a deck in a site never reaches the code those exemptions
live in. A dense do-not-touch cluster is not evidence of coverage; it is a reason to measure.

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** Detail lives in git, in [AUDITS.md](AUDITS.md), in the dated findings docs
and in [LESSONS.md](LESSONS.md) — look there rather than re-expanding this list. A batch's date and
branch are enough to find its commits.

### Shipped

- **2026-07-30 LSP editor ergonomics** (178, 177): all 8 tasks of
  [2026-07-29-lsp-editor-ergonomics.md](../docs/superpowers/plans/2026-07-29-lsp-editor-ergonomics.md),
  one commit each, `./tools/gates.sh` green with all five interpreter canaries named and passing.
  Zero new TypeScript: the four new capabilities reach Zed/Neovim/Helix too.
  - **178, `didChange` is coalesced**, and the spec's open question is answered by measurement
    rather than argument: `buffer_diagnostics` on the largest page of the 25-page guide is **33 ms
    in a debug build** against a 120 ms window, so debouncing alone was enough and the anchor scan
    got **no** memo. Two departures from a naive debounce, each with its own test: `pending` is a
    **list**, because a single slot lets an edit to document B silently discard the diagnostics
    owed to A; and the window's deadline is set by the **edit**, not refreshed by every message,
    or a client that polls (hover as the pointer moves, inlay hints on scroll) starves the
    publish. `render_buffer` is memoized on `(uri, text)`, and text as the key means there is no
    invalidation logic and no staleness class.
  - **177, inlay hints, folding, document highlight, selection ranges**, plus visible math
    delimiters. **Three of the plan's steps were wrong in ways only the code showed.** (a) Folding
    had to track code fences, which the plan omitted: a `# comment` on a `{python}` cell's first
    line parses as an h1 and closes the enclosing section's fold at the cell. (b) Document
    highlight needed **no new scanner**: `lsp_nav::anchor_occurrences` already had the exact
    signature and semantics, and a second scanner could disagree with rename about what an anchor
    is. (c) The plan's `bib_field` was a substring search, which finds "year" inside a title and
    then reads the *next* field's value.
  - **Three tests were vacuous as first written and were caught by mutation, not by review**: a
    scope-name check that passed because `str::find` is case-sensitive; a heading-fold fixture that
    only ever deepened, so nothing distinguished "close this level or shallower" from "close this
    level"; and a containment check whose levels already grew, so deleting the filter changed
    nothing. Each was rewritten until the mutation failed. **This is the third batch in which the
    mutation step, not the test-writing step, found the real gap.**
  - Math delimiters are pinned at three levels because the manifest alone proves nothing: the
    manifest's shape, the **grammar** actually emitting those scope strings (a typo would be
    silently inert), and a real Extension Host confirming VS Code *accepts* an
    extension-contributed `editor.tokenColorCustomizations` default.

- **2026-07-29 explorable cluster** (153, 154, 155, 156, 157): the whole `{js}`-side batch, on
  branch `explorable-cluster-2026-07-29`.
  - **153 — the cell-language registry, with `{glsl}` as the proof.** `render/client_lang.rs`
    (server) + `window.taliJs.registerLanguage` (client) replace six spellings of
    `lang == "js"`. **The refactor found a live bug**: `reactive.rs` read "any cell that is not
    `js`" as "a cell that could publish names at runtime", so a second client language would
    have silently switched the whole dangling-input check off for its page. `{glsl}` is one
    file and zero vendored bytes (WebGL is a browser API). **Two gaps closed on the way**: the
    External/site path gated the runtime on `{js}` alone (a `{glsl}`-only page shipped a dead
    canvas), and a static shader blanked on any resize (`canvas.width =` clears the buffer).
  - **154 — the `num` global**, first-party, on the existing `{js}` asset gate. Keep it
    curated: it is what a *drawing* cell needs, not a numeric library.
  - **155 — `animate` + `point`.** Both publish through the same hidden `[data-tali-input]`
    element and the same `input` event as a slider, so the fragment sync, the readout and the
    single scheduled pass are all reused. The one widening was `readValue` parsing
    `data-tali-json`. **The tick is a `type="number"` field, not `type="hidden"`** — the latter
    hands every downstream cell the *string* `"3"`.
  - **156 — `tali.state`**, per cell, scheduling nothing, keyed by the container id so an edit
    clears it for free.
  - **157 — `tali.tex` / `tali.table`.** **Not KaTeX-the-parser** (only its CSS + fonts are
    bundled, and a 280 KB parser on the `{js}` gate is not worth it): the grammar is closed, so
    the glyphs come from KaTeX's faces and the bracket/grid layout is ours, with a serif
    fallback for a page that ships no KaTeX sheet.
  - **Coverage**: `crates/core/tests/client_lang.rs` + `crates/server/tests/reactive_browser.rs`
    (the first browser test of the reactive client — 9 tests over all four new corpus pages,
    reading pixels, published values and DOM back). 13 mutants killed.
  - **Two illusions deleted rather than shipped**: both candidate assertions for 155's
    "one pass per frame" trap stayed green with the pacing removed, so the property is recorded
    in [DETECTION-DEBT.md](DETECTION-DEBT.md) instead of faked. See the note in
    `reactive_browser.rs`.
  - **A new gate came out of it**: `every_tali_custom_property_read_is_defined_somewhere`
    (`token_contract.rs`). Three invented `--tali-*` names made the `point` pad render as a
    floating dot with no box, and **only a browser screenshot caught it** — a `var()` naming a
    property nothing defines makes the browser drop the whole declaration, silently.
- **2026-07-29 ruled-and-built batch** (101, 122, 71, 78, 149's buildable half, 18's doc halves,
  41's `alt` half, 150's risk half). Four owner rulings taken in one pass, then built:
  - **101 — the licence position on what Taliesin *emits* is stated.**
    `LICENSE-OUTPUT-EXCEPTION.md` is an **additional permission under AGPL §7** (the GCC
    runtime-library pattern): output may be conveyed under any terms, no notice required in it,
    publishing it does not by itself engage §13. The carve-out stays carved — conveying Taliesin
    itself, or running a modified one as a service, is untouched. **Deliberately NO per-asset
    licence headers**: those files are copied verbatim into every page, so a header would add
    ~1 KB per page to assert a licence the exception exists to disclaim. `output_exception.rs`
    pins the grant, its limits, the three places a user looks, and the premise (first-party
    assets are still `include_str!`'d into pages).
  - **122 — `check` names the interpreter it would use, and still spawns nothing.** PL14 was
    right that it must not SPAWN and wrong to conclude it must therefore say NOTHING.
    `ProbePolicy` splits the two decisions. **The filed +50% cost objection is GONE, not
    accepted:** `CheckScope` carries the language list off the render the diagnostics walk
    already did, so `collect_environment` renders nothing — measured identical to the
    pre-change binary on all four projects. A site's decks now count toward the report too.
  - **71 — both deck-on-touch behaviours.** The stepped-mode swipe has **no time bound** (only
    the 50 px floor); the share panel leads with Copy and demotes the QR on `(hover: none) and
    (pointer: coarse)`, changing DOM order so tab order follows reading order.
  - **78 — figure text on a data fill keeps its own colour.** The discriminator is `ax.texts`
    (the author's own `text()`/`annotate()` artists; the title and axis labels are never in it).
  - **150's risk half** — the click-to-source anchor travels with the message.
  **Two probe traps re-confirmed the hard way, both already in LESSONS.md:** a whole-page
  `contains()` for a new class matches the *inlined CSS* (cost one wrong "it works"), and a
  deck probe that compares only the `h` index reads every fragment step as "ignored".
- **2026-07-29: the demand-driven tail's "LSP for the language intelligence" line was DELETED as
  stale, not promoted.** It had survived in that tail describing work that already shipped:
  `taliesin lsp` exists (`crates/server/src/lsp.rs` plus seven sibling modules), the VS Code
  companion was rewritten as a thin client over it on 2026-07-28, and the specific gap the entry
  cited as its own justification, the `#| label:` completion drift, is closed at
  `lsp_complete.rs:281-284` and pinned by a test at `:1250`. **Do not re-file it.** It is also
  why item 161 shrank: `textDocument/documentSymbol` now supplies the heading tree that the
  structure panel was going to build.
- **2026-07-29 first-hour + positioning** (144, 151, 87, 88, 94, 95, 96, 135, 136): eight CLI /
  diagnostic / LSP residuals, the two lying ui-audit probes (suite now 7/7), the first-run
  execution notice, and `docs/guide/using/choosing.tmd`. **Three filed causes were false** — 151's
  "`id="TOC"` is in no emitter" (it is; the probe targeted a *book*, which by ruling has no rail),
  94's stale 8.59% (7.3% today), and 144c's scope (also an unfiled per-page repeat).
- **2026-07-28 block model + docs gate** (138, 146, 143's path half): every block has exactly one
  root element; prose is gated against the tree (dead source paths, retired front-matter keys,
  undocumented CLI flags) rather than against a needle list. **`notes/` and `docs/superpowers/` are
  excluded from that gate and must stay excluded** — they are dated records.
- **2026-07-28 deck harness** (112, 125, 113, 111): `deck.js` has a browser test; deck content is
  auditable at 0 violations across 100% of slides. It found **two shipped layout defects on its
  first run** (code blocks clipped on 5 of 21 slides; a focus ring around every vertical-stack
  slide), neither filed and neither visible to any emission test. The eleven deck shapes 113 listed
  stay deliberately unbuilt — the walker renders every corpus doc on every `cargo test`.
- **2026-07-28 honesty + build cost** (91, 110, 115, 119, 126, 134, 143): `chromiumoxide` is an
  opt-in `headless-js` feature, off by default; not linting `draft:` pages is **ruled correct** and
  the defect was the silence; `Block::sourcepos`'s empty-string contract is documented; the ACR is
  published; [DETECTION-DEBT.md](DETECTION-DEBT.md) is the live register.
- **2026-07-28 verified sweep** (85, 86, 97, 98, 99, 114, 123, 130): a `theme:` extension bundle is
  contained (**item 80's absolute-`Path::join` footgun in a second place**); no built page fetches
  off-origin; a shortcode source is a path, not a URL; both `jsconfig.json` include lists are
  globbed. 130 was already fixed and 99 was a clean measurement — both closed with no code.
- **2026-07-28 critique-round client/LSP/manifest** (139, 140, 141, 142): rename validates the new
  name and leaves an external URL's fragment alone; `toc_html` stopped double-escaping an explicit
  heading id (a dead link in the published build); the Cmd-K palette locks the background scroller;
  the web manifest stops shipping Taliesin's brand and stops pointing at a 404. **The splash colour
  is deliberately still one light value** — a manifest cannot express an OS-conditional colour.
- **2026-07-28 reader cost** (150's Phase B half, 137, 124): the body typeface ships as
  content-hashed files, not base64 in the render-blocking sheet (**125 KB gzipped off the critical
  path of every page**); the three conditional blobs are written only when something links them
  (94% cut on prose-only `corpus/tarn`); the Label-in-Name static rule.
- **2026-07-28 publication readiness** (84, 89, 90, 92, 93): `tools/gates.sh` (the one script that
  runs every gate and **refuses to be green when one skipped**), `CONTRIBUTING.md` with the inbound
  relicensing grant, `ci.yml` + `release.yml` **guarded inert until the repo is public**, the
  measured install expectation and platform matrix, and "Coming from Quarto".
- **2026-07-28 launch blockers** (79-82, 109, 117, 118, 120, 121, 127, 128): `mounts:` is contained;
  `check` does not spawn a project-supplied interpreter; `--no-exec` covers `{js}`; a deck in a site
  is validated; comma fences highlight; a migrated link gets a did-you-mean. **`--no-exec` is
  deliberately NOT a sanitizer** (2026-07-03 CSP ruling) — do not re-scope as "strip the HTML too".
- **2026-07-28 item 83 — the five pre-relicence MIT tags are deleted** (owner-approved; none had
  ever been pushed). All five commits stay reachable from `main`, so only the labels went. **The
  durable rule: never tag before the licence is settled**, and cut a release tag only from a tree
  whose `LICENSE` matches `Cargo.toml`.
- **2026-07-27 item 76 — a book has no right-rail TOC** (owner ruling, reversing 2026-07-06). The
  gate is `Site::page_toc`, ahead of the page's own `toc:`. **Do not re-scope as "give books their
  TOC back" or as "delete the rail everywhere"** — websites and single documents keep it. The
  drawer marks which section of the open chapter you are in, computed on each open (the drawer
  locks the root scroller, so a scrollspy would watch a dead event).
- **2026-07-27 item 77** (the 72-75 residuals): shortcode arguments linted against a closed
  vocabulary; `TAL-SHORTCODE` is its own WARNING family; `favicon:` resolves like `logo:`. **A book
  with neither title nor logo still emits no brand link, deliberately.** The fourth residual was
  refuted by measurement.
- **2026-07-27 mutation campaign** (58-69): every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned. **Do not
  re-run it against the same scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66** (`404.html` links the shared `_assets/` bundle; its hrefs are root-absolute
  on purpose) and **item 67** (the `~/.local/bin/taliesin` launcher exits early for `__complete`
  only, 24.3 s -> 0.024 s per tab press; **`completions` is deliberately NOT exempt**).
- **2026-07-26 deck weight + headless bounding** (52, 55): a site deck went 4.6 MB -> 7 KB via a
  separate `deck.<hash>.{css,js}` pair. **A deck cannot link the page's `app.js`** — `search.js`
  would steal Cmd-K. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path parity** (50, 51, 57): `render_single_doc` decides the single-document
  containment root once (nearest `_site.yml`, **never `.git`**); `TOC_SHEET_MARKUP` is the one copy
  all four assemblers emit. **Do not re-scope as "give the single-file build the inferred root"** —
  that is a revert of `9359a2c`.
- **2026-07-26, earlier**: migration UX (53, 54); the mobile batch (42-49 — the tree asks what
  device it is on via `hover`/`pointer`, it had none); owner rulings 24 (`data-section-end` shipped
  as option (b)), 17 (book breadcrumb ruled **no**) and 2 (deck presenter tools declined); reporting
  surfaces 39, 40; demand probe #4 (`corpus/analyst`); AP1-R1 (the freeze cache is byte-bounded) and
  DOCS-2/3/4/5.
- **2026-07-25 and earlier, closed:** AP7-1..5 (a11y), AP3-1, AP11-1 (`TAL-KERNEL`), DIAG-1, DOCS-1,
  AP3-3, PA-M3, PA-M13, PA-H1's residuals, the backlink-context + resume batch, book wayfinding, the
  hardening batch, book-level `theorems:`, live-executor mounts (F-04), book-aware `read`, AP8-1's
  output scrub, DET-1, the DX audit batch, `taliesin lsp`, DX17(a)+(b), the deck audit, the polish
  audit batch, the PMF builds, corpus coverage, the machine-facing audit, AI-native packaging, the
  R/Python ANSI leak, ungraceful-death reaping, and the `assets/js` `tsc` gate.

**Numbers retained, never reused** (each closed by a ruling or folded into another item, and kept
here so a later round does not re-derive them): **116** — the positional cascade vs a Python DAG,
CLOSED, do not build; reactivity is marimo's well-made claim while reproducibility is unclaimed by
anyone, so tell the cascade story instead. **132** and **133** — R8's value-stream pricing of 109
and of 127/128; a deck's defects are found by an *audience*, the latest and most expensive point in
the stream, and 447 of the 457 diagnostics a real external book produces are the tool's vocabulary
gap rather than the author's mistakes. **145** — retired into 137. **147** — retired into 101.
**151** and **152** — see P2 and the 2026-07-29 batch.

### Decided against

- **"Adjacent slides bleed into the deck's letterbox" (DT-5, filed and RETRACTED 2026-07-27, same
  day):** **false — the letterbox is empty.** `.tali-deck` is sized to the 16:9 stage
  (`min(100%, 100vh*16/9)`) with `overflow: hidden`, and its comment already says "adjacent cells
  fall outside and are clipped (no peek)". The probe intersected each neighbour with the
  **viewport** instead of with its **clipping ancestor**, and `getBoundingClientRect` knows nothing
  about `overflow: hidden` — re-measured, the neighbour contributes **0 px** inside the clip box and
  `elementFromPoint` returns `BODY` there. **Do not re-file it from a rect measurement**; if it ever
  looks true again, the only valid evidence is a rendered pixel, not a rectangle.
- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22 and
  **re-declined 2026-07-26** on the same grounds — no real speaker ask has appeared. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst case
  is 55 ops in one frame, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so batching saves
  ~220 bytes on a 32,303-byte payload (0.7%), none on the critical path. Reopen only if render cost drops
  far enough that framing is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`; T2's
  "three modules pre-scan" is partly rotted — the real duplication is a six-line idiom in two places, and
  the divergence that looked like a latent bug is unreachable.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP) declined —
  do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse* (15 of 25
  slides vs 23 of 25); do not re-refine without measuring.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building it):
  the index it would duplicate is already lazy-loaded on every page, so a sidecar buys ~55 KB gzipped on
  one cached subresource in exchange for a second copy of the render recipe, assembly, invalidation,
  route and build write.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it, and
  a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code and
  math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the error
  goes into a promise about the reader's time in the wrong direction, on exactly the chapters this tool
  exists for. (The dated-post estimate in `render/mod.rs` is a different surface; `is_article` is
  test-pinned, do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25): measured
  across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the *better* nav
  label. Resolved as documentation, not code.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).
- **2026-07-22 rulings:** DX16 update-nudge = **skip** (a version check is network egress that undercuts
  the offline-first identity); cross-ref label i18n = **defer** (no corpus doc demands it); item 9's
  design questions documented as intentional (the deck serif/sans inversion, no `//| uses:` alias, the
  callout-namespaced / theorem-bare asymmetry).
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables, line-level
  code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt substrate in
  `14-reader-prefs.js`).
- **TODO / FIXME surfacing** (owner ruled 2026-07-10): no `level` concept exists, so a TODO warning would
  fail `check` on every draft. If revived, a preview-only `Diagnostic::info` beats re-plumbing a real
  `level`, and the scan must NOT reuse `prose::strip_inline` (it blanks code, where TODOs live).
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution (the only proposed
  network egress; check-only and off by default if ever revived), the numeric-claim-without-citation hint
  (its own spec rates it FP-prone), and a per-page text/JSON sidecar (redundant).
- **Refuted by measurement (do NOT re-scope):** heading demotion **already ships** (AP9's "12 `<h1>`"
  measured a stale gitignored build artifact; the only multi-`<h1>` corpus docs are decks, exempt by
  design); `build` does not leak forkserver subtrees; the warm pool booting Python on prose-only builds
  is hygiene, not latency; dev attributes are 0.29% of page bytes (don't strip); a `--version -dirty`
  marker is stale-by-construction; the `assets/css` stale-embed claim did not reproduce (re-verify for
  `assets/js` before any touch-render workaround); the 390px `hero:` overflow + theme/video desync are
  fixed; include symlink-loop SIGABRT does not exist (Linux caps at `MAXSYMLINKS=40`); **decks pass path
  parity outright** and `mounts:` differs from direct serving by 4 bytes (boot nonce + ws path).
- **`_redirects`/`_headers` preserved, never generated** (`build.rs` treats them as author-placed deploy
  metadata; `stale_sweep.rs` pins it).
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex, schemars,
  jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
  filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll (not
  pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals.
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1; the
  deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features. When publishing, lead the copy with the **speed moat** (warm server, block-level incremental,
no per-edit rebuild), the single most-repeated Quarto grievance and the most under-marketed asset.
