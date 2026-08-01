# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git, [AUDITS.md](AUDITS.md) and
> [ROADMAP.md](ROADMAP.md); **delete an item when it lands** — never a `[x]`, never a strikethrough.
> Method lessons that outlive their item go to [LESSONS.md](LESSONS.md), detection gaps to
> [DETECTION-DEBT.md](DETECTION-DEBT.md). "Do not re-add / re-scope" is a compact anti-rot guard —
> **one line per entry**, not a changelog.
>
> **This file has now been cut back twice for the same reason** (1,767 lines on 2026-07-29; 1,298 on
> 2026-08-01, when the "Now" section had become a changelog of *shipped* batches and a third of the
> one-line "Shipped" guards had grown into 40-line reports). If it is growing again, move the detail
> out; do not add a summary at the top.

## Start here

- **P1 is a ranked build queue, not a menu.** Take from the top: the research-publishing cluster
  (183-188), then the large swings. **Read
  [the survey](2026-07-31-research-publishing-survey.md) before starting anything in the cluster** —
  it records what Taliesin already leads on and what was deliberately rejected. **The cross-cluster
  ranking of 181-188 against 164/167 is an owner call that has not been made.**
- **Ask git, never this file, for git state.** No SHA, branch name or commit count is recorded here
  on purpose: the author and parallel sessions both push, and a recorded SHA is the line that rots
  first.

  ```sh
  git log --oneline origin/main..HEAD   # what is unpushed
  git branch -vv                        # what branches still exist
  ```

- **Entries rot: trust an item's *symptom*, never its cause, line number or cost.** Grep the named
  symbol in source before pricing the work. Measured 2026-08-01: **item 182 was filed on 2026-07-31
  as "Taliesin has both link shapes and zero hover machinery (grepped)", while `site/hover.rs` plus
  `code-enhance/12-link-preview.js` had shipped exactly that feature on 2026-07-06** — it was
  deleted, not built. Three filed causes were false in the three batches before it.
- **Two measurement hazards, both of which have cost time.** (1) `target/release/taliesin` is shared
  across sessions and may be built from another branch — check `taliesin --version` against your own
  HEAD before trusting any CLI number. (2) A table-shaped probe whose every cell is negative is a
  **broken probe** until proven otherwise; carry a known-positive row.
- **Nothing is owed by the author** except R12 (real-device Android) and the P3 rulings.

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
- **Tests: four gates, or the suite silently under-tests itself.** Run `./tools/gates.sh`, which arms
  `TALIESIN_REQUIRE_KERNEL` / `_R` / `_NODE` / `_CHROME`, asserts each canary printed `... ok` and
  refuses to be green when one skipped. It needs `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python`
  or it declines to start (exit **2**; a failed gate is exit **1**). Run the workspace suite
  `-- --test-threads=1` as it does: several tests own process-global state (`CHROME_PATH`), so at
  full parallelism a browser test fails in a way that reads exactly like a regression. `cargo test`
  aborts the remaining binaries at the first failure, so re-run before trusting a total.
- **Any new generated block owes the four-projection sweep** — `taliesin read`, `skim.rs`, the search
  index and `llms-full.txt` — or its text leaks into the search index. Four projections in three
  modules; two of item 173's leaks were found only by building a real site and grepping the
  artefacts.
- **A new `data-*` attribute or `--tali-*` token in browser code trips a census test**
  (`token_contract.rs`): expected, one sorted line to fix, and it is also the prompt to namespace the
  attribute. An invented `--tali-*` name renders **nothing** (the browser drops the whole
  declaration), which is why the census exists.
- **LSP/editor ranges are UTF-16.** A non-ASCII character earlier on the same line shifts every byte
  offset after it and the edit lands in the wrong column.
- **A red `exec`/`kernel` probe is real signal, not a coin flip.** The flake was fixed 2026-07-25 (a
  port race in `prepare_connection`; the re-roll lives on `Kernel::start_with_retry`, and
  `crates/server/tests/kernel_start_is_retried.rs` fails if any caller reaches the un-retried
  primitive). Verified 0 failures in 45 post-fix runs under the same load.
- **`corpus/tarn` is the fixture for scale-sensitive work** (12 numbered chapters, 3 parts + a nested
  part) and deliberately carries the shapes the rest of the corpus lacks. **Use it instead of minting
  a fixture.** It is a *documentation* book, not a scale fixture: do NOT grow it toward 200 pages and
  do NOT mint `corpus/longbook` (the walker renders every corpus doc on every `cargo test`).
- **Execution pins do not belong in `corpus/`.** The walker renders every corpus doc on every
  `cargo test` but does **not** execute cells, so a corpus pin for execution behavior pays the render
  cost and exercises nothing. Put them in `crates/server/tests/` against a temp-dir fixture, as
  `executed_output_reproducible.rs` and `progress_bar_collapses.rs` do.
- **Verify a fix by mutation** (restore the bug, watch the *named* test fail), not by a green suite.
  **The full trap catalogue is [LESSONS.md](LESSONS.md); read it before writing a probe or a pin.**

## Open items

**Ranked by what a session should pick up, not by theme.** P1 is buildable today; P2 is filed so it
is not rediscovered as a defect; P3, P4 and P5 are blocked and are listed so they are not re-scoped.
**Item numbers are stable**: never renumbered, and a closed item's number is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation*, browser-verify
anything client-side, and **delete the item from this file when it lands**.

### P1 — build now

**The order below IS the priority order.** It encodes four things: **defects in shipped work first**,
then **dependencies** (184 is substrate for 185/186/187), then **size** (cheap wins before the two
large swings), then the author's standing **feature-first policy** (170, the marketing site, is
deliberately last).

Two conditions apply to every item here. **Each still owes a corpus pin doc** (a capability ships
pinned by a target corpus document added in the same change) — but **do not grow `corpus/` past the
pin a feature needs**. And **promotion is not a design**: several were parked with an open design
question, not just for lack of demand. Those say so; brainstorm before coding.

183. **Footnotes as margin sidenotes.** (S-M, **needs an owner ruling before code**. Survey §4C+§6.)
     Footnotes exist, gathered at the page end (`base.css:779`), and margin divs exist; a `[^note]`
     cannot become a sidenote. Tufte's mechanism is **pure CSS** — a sidenote counter plus a
     `label.margin-toggle` + `:checked` collapse below the breakpoint, no JS. **The ruling is whether
     margin placement is the DEFAULT on a wide screen or a `footnotes:` knob**; minimal config argues
     for the default and against the key. Traps: **the names `.sidenote` and `.marginnote` are
     already taken** as aliases of `::: {.column-margin}` (`vocab.rs:207-229`), so reuse that
     container rather than minting a class; collision with an existing `.column-margin` float; and
     the print track (159) must still print footnotes.

184. **Structured authors + affiliations.** (M. **Substrate for 185/186/187 — land it first.**
     Survey §4E + §6.) `author:` is a flat string list (`frontmatter.rs:25`) consumed only by
     `cite_this.rs`. One structured form feeds **three** consumers: the visible byline,
     `citation_author` + `citation_author_institution`, and JSON-LD `affiliation`. Scalars must stay
     valid or every corpus doc breaks. Traps: **a new front-matter key trips five drift gates**, and
     `cite_this.rs`'s render gate (page author → site author, never site title) must not silently
     change which pages emit a cite box — pin it before touching the parser. Pin: extend
     `corpus/cite-this/`.

185. **Resource-links row + venue/award badges.** (S, after 184. Survey §6.) The one element every
     fork of the project-page template keeps: Paper / arXiv / Code / Supplementary under the byline.
     **Minimal-config shape is URL inference** (`arxiv.org` → arXiv, `github.com` → Code, `*.pdf` →
     Paper), with a `{text:, href:}` override. **Do not overload `hero:`** — it replaces the title
     block and has no icon concept. Icons must be bundled SVG, never a CDN font. `venue:` also feeds
     186's `citation_conference_title`.

186. **Complete the Scholar + social meta.** (S, `citation_author_institution` needs 184. Survey §6.)
     `site/meta.rs:148-161` emits `citation_title`, `citation_author`, `citation_publication_date`,
     `citation_journal_title` and `citation_public_url` (re-measured 2026-08-01). Missing:
     `citation_conference_title`, `citation_doi`, `citation_arxiv_id`,
     `citation_author_institution`, `citation_abstract_html_url`, and `og:image:width`/`height`
     (known at build; LinkedIn needs them for a large card). Add to the **shared** `emit_social`, not
     next door — the page and embedded-deck paths share it by construction. **The inlined-asset
     needle trap applies, in both directions.**

187. **The appendix block: author contributions / acknowledgments / DOI.** (M, after 184. Survey
     §4D + §6.) Distill's `d-appendix` made author-contributions a first-class section rather than
     prose. Follow `cite_this.rs`'s generated-block pattern **including its determinism rule** (no
     accessed-date, no build timestamp, or the byte-identical build and the freeze cache both
     break). Owes the four-projection sweep.

188. **Results gallery + image-comparison slider.** (M, lowest conviction in the cluster. Survey §6.)
     Evidenced need, rejected mechanism: every project page shows N result figures and reaches for a
     carousel, which hides n-1 of them behind a timer. Build `::: {.gallery}` over
     `render/figure.rs` (so `@fig-` still resolves) and a drag-divider comparison slider, which is
     what the carousel is a poor substitute for. **Substrate exists for half of it**: `11-lightbox.js`
     already steps through a multi-image set (`.has-gallery`, prev/next); the slider does not exist.
     Pairs with 181. Trap: scroll/drag browser tests have a documented false-negative pattern (force
     `scroll-behavior: auto`, settle rAF, floor `innerWidth` ~500 px).

164. **`docs-as-spec`.** (L. [ROADMAP.md](ROADMAP.md) Pillar V.) Promote the two dogfooded books to a
     versioned normative spec: an RFC-2119 `.tmd`-dialect reference plus a WebSocket protocol
     reference. Neither exists today (measured 2026-08-01: RFC-2119 language appears only in
     `docs/superpowers/` plans). Upstream said to start only once the validation epic had settled,
     which it has. Value is credibility and adoption, not capability.

167. **`check --online`.** (Opt-in; no `online` flag exists today.) Dead-link checking as the
     **single sanctioned network call**; the default stays offline, deterministic, kernel-free and
     network-free. **Scope note:** the *citation/DOI-existence* half was separately **declined
     2026-07-16** and is not revived by the 2026-07-29 promotion; if it is ever wanted it needs its
     own ruling.

56. **L5-1 residual — what is left is a FEATURE PROPOSAL, not authoring.** (The `description:` half
    shipped 2026-07-26; the backlinks half shipped 2026-07-31.) `glossary`, `term-index` and
    `float-digest` still grep to **zero** across `crates/core/src` + `crates/server/src`, re-measured
    2026-08-01, so "they render empty until an authoring pass happens" has never described authoring
    work: there is no surface to feed. Writing `{.definition}` blocks today feeds only `skim.rs`,
    which reads them as statement heads. **Decide whether any of the three is wanted before writing a
    line of prose for them** — and if so it is a build, with its own spec.

175. **The long-running-cell workflow.** (Parts (a) and (b) shipped 2026-07-30; (c) and (d) are
     open and neither is blocked.) Jupyter's daily-driver property for expensive work is "watch it
     run, then re-run only what you choose". Taliesin now has the *watch it run* half.
     - **(c) No escape hatch for an expensive cell.** Freeze keys on a cumulative hash (this cell +
       all upstream same-language code + interpreter id), so editing *any* upstream cell busts the
       expensive one, and the cell-option set (`validate.rs:18`) has `cache: false` to opt *out* but
       nothing to opt *into* stickiness. Proposal: a `#| checkpoint:` cell whose freeze entry
       survives an upstream edit **but renders a visible stale-relative-to-inputs marker**, with
       `build` warning or refusing. **Jupyter lets the notebook lie silently; this would let you
       defer the re-run while showing the debt.**
     - **(d) No per-cell run and no interrupt.** The preview can only `restart_kernel`
       (`serve/mod.rs:43`), which nukes all state. Per-cell run/interrupt belongs in the **editor**
       (CodeLens), not the preview: it keeps single-editing-surface clean and avoids a second control
       surface in the browser. Same work as `FEATURE-IDEAS.md` idea 86 — do not build it twice.

174. **`serde_yaml` fallback swap.** (Conditional: **no trigger yet**, which is why it ranks here.)
     `Cargo.toml:20-24` names `serde_yml` as the fallback; that fork carries RUSTSEC-2025-0068
     (unsound + unmaintained), and `serde_norway` is over a year stale. The maintained continuation
     is **`serde_yaml_ng`** (v0.10). No urgency: config is trusted and local, and 0.9 still builds.
     **Act when** 0.9 breaks against a future serde or edition, and gate the swap on a test that
     `Error::location().line()` still works (the front-matter linter's located diagnostics depend on
     it). Fix the stale comment whenever this file is touched for any other reason.

170. **Marketing site.** (Last by the author's own feature-first policy.) The `live-edit-hero-demo`
     clip (= `ROADMAP.md` Wave 2's unshipped deliverable), swapping the `site/_site.yml`
     placeholders, a demo-led hero rebuild, mobile embed refinement, and deploy. **The deploy half is
     additionally flip-gated** and overlaps 149's launch-presentation group in P3; do not build the
     same thing twice from both entries.

### P2 — filed so it is not rediscovered as a defect

Not worth a session on its own. Each is a record or a known cost, not a task.

131. **The cold-build cliff: 3,981 ms vs 789 ms warm.** (LOW, and probably correct as-is.) Kernel
     *variable* state is never cached — the property that makes the cache trustworthy — so a cold
     start genuinely cannot skip work unless the whole document is unchanged. **The waste is inherent
     to a correctness guarantee worth keeping.**

129. **Shape inventory from two real external documents — the durable half of R11.** What real
     documents contain that `corpus/` has nowhere: `lang,attr` fences (734 occurrences), ` ```console `
     (209), links with a non-`.tmd` extension (128), a `SUMMARY.md`-driven chapter spine, **112 pages
     in one flat directory** (the largest corpus project is 14), and chapter files with **no front
     matter at all**. **Do NOT grow `corpus/` toward these.** Only the two that earned a pin got one
     (127 and 128, both shipped 2026-07-28); the rest are recorded so a later round does not
     re-derive them.

152. **The companion e2e's `EMFILE` failure was an inotify limit, not the code.** `fs.inotify.max_user_instances`
     was still the kernel default of 128 while the desktop session already held ~154; raised to 512
     via `/etc/sysctl.d/99-inotify.conf`. Kept because the same limit throttles `taliesin preview`'s
     own watchers: if previews stop hot-reloading, or VS Code refuses to start, check
     `find /proc/*/fd -lname 'anon_inode:inotify' | wc -l` against
     `/proc/sys/fs/inotify/max_user_instances` before suspecting the code.

192. **Two companion e2e list-continuation tests are pre-existing failures.** Verified against a
     worktree at `origin/main` (27 pass / 2 fail there, 33 / 2 on a branch), and they fail at load
     ~2-3.4, not the "load ~6-7" this file once recorded. Treat both `pressEnterAfter` tests as
     unreliable at any load, and **always compare against an alternating baseline run** rather than
     against a recorded number. Also: the e2e runs `target/debug/taliesin`, which `cargo test` does
     **not** rebuild — `cargo build --bin taliesin` before believing any e2e result.

193. **Moving the cursor into another chapter moves the preview to that chapter** (150, 2026-07-30),
     on the passive path, not only on the explicit reveal. Deliberate — a preview showing a chapter
     you are not editing is stale, and the yank the reveal/mark split guards against is a cursor in
     the page *already* on screen, which still never navigates — but it has not been lived with. If
     it turns out wrong, the answer is a better default, not a knob.

### P3 — blocked on an owner ruling (not a task until then)

100. **The public flip: RULED 2026-07-28 — "archive plus fresh public", and it is specced.** See
     [2026-07-28-public-flip-audit-design.md](../docs/superpowers/specs/2026-07-28-public-flip-audit-design.md).
     The ruling threads the needle both earlier routes missed: **the history IS published** (the
     single-author commit record is the evidence a grant applicant wants), and `git rm` in a new
     commit leaves a file in every commit that ever held it. Mechanism: relocate the purged docs to
     `~/Documents/personal/taliesin-private/`, rewrite history, rename this remote to
     `taliesin-private-archive` (stays private, complete backup), create a **new public**
     `AJBogo9/taliesin` and push the rewritten history there. No force-push, no destructive remote
     op, and the private blobs never reach the public repo at all. Zero forks and never having been
     public is what makes it cheap.
     **Kept, not purged:** security audits, `.claude/`, `docs/superpowers/`, `AGENTS.md`,
     `LESSONS.md` — for the stated goal those are the exhibit. **Purged:** money and strategy
     documents only (`notes/STARTUP-PLAN.md`, `notes/FUNDING-RESEARCH.md` — both git-**tracked**
     while their own headers say they must not be), plus ~11 commit subjects that name them.
     **Execution status: NOT STARTED, and not to be started without a separate instruction.** Phase 1
     is a read-only audit and is safe whenever wanted; **Phase 2 is irreversible** and is
     additionally gated on Phase 1's findings being signed off. What still lands on this item:
     - The spec's own D-checks, including the provenance check on corpus documents.
     - Its rule that a still-open finding reading as an **exploit recipe** is reported for individual
       judgement, default keep — which meant items 79-81, and those are **fixed and on `main`**
       (verified 2026-08-01, `a2d05657`), so that clause is discharged.
     - **Whether tags travel to the new public repo** (five local MIT tags were deleted on
       2026-07-28; none had ever been pushed).
     - **Whether to prune `notes/` + `docs/superpowers/`.** Scale, measured: `git ls-files notes/` =
       63, `docs/superpowers` = 69, and the largest is
       `2026-07-03-quarto-design-decisions-catalog.md` at **1,129,387 bytes** of adversarial
       self-critique sitting under `docs/`, which a visitor reads as "the manual".
     - **A procedure collision to fix in the same change:** `***REMOVED***
       (fresh repo), while this file and `2026-07-17-security-release-audit.md:217-218` sequence the
       `oss-*` items to "whenever the repo actually flips public". Fix the losing document or the
       next session follows it.
     - **`git grep -Il "/home/bogo"` → 11 files** (measured 2026-07-28), against a former line in
       this file claiming the tracked paths were scrubbed. The 2026-07-17 scrub was scoped to four
       paths under `docs/superpowers/*`; low impact (the username is public via git author metadata)
       but it is the failure mode `LESSONS.md` warns about. *(Item **25**, the pre-public flip
       procedure, is folded into this item; its number is retired below. `oss-4` was ruled
       2026-07-25: deferred, "I'll do it at the end of summer". All five of its code items shipped
       2026-07-25.)*

102. **Decide what to do about constructs that render elsewhere and silently do not here.** (Ruling.)
     Detail in [adoption friction](2026-07-27-adoption-friction-audit.md).

103. **Clear the name in software classes before the flip.** (Ruling, legal not code.) Trademark
     search in the relevant classes; the name is the retained optionality per the product stance.

148. **Distribution: the binary channel has a mechanism but no artifact; the package managers are
     untouched.** `.github/workflows/release.yml` builds Linux x86-64, macOS arm64 and macOS x86-64
     on a `v*` tag and attaches a tarball + `.sha256` with `LICENSE` + `THIRD_PARTY.md` inside; the
     README states the matrix (Windows explicitly unsupported). **Still true:** no tag has been cut,
     the workflow is guarded inert until the repo is public, and crates.io `taliesin` /
     `taliesin-core` / `taliesin-server` are all 404 (all three names free); no Homebrew, Nix, or
     install script. Remaining work: **cut a tag after the flip**, then decide about crates.io / brew
     / nix separately.
     - **`cargo publish` will reject this workspace as-is:** `Cargo.toml:14` declares
       `taliesin-core = { path = "crates/core" }` with **no `version`** (re-measured 2026-08-01).
       Also blank: `keywords`, `categories`, `readme`, `homepage`, `documentation` in every manifest.
       Watch `crates/core` = 7.3 MiB tracked against the 10 MiB `.crate` cap.
     - **Cold build: 2m11s, 268 crates, 2.6 GB peak RSS at `-j4`** for one ~38 MB binary (measured
       2026-07-28; the filed 2m59s was a different machine or job count, not a regression). The
       audience for a documentation tool is not the population that will install a Rust toolchain and
       wait it out, which is exactly why the release workflow exists.

149. **Launch presentation, all gated on the flip.** Grouped because none is actionable until the
     repo is public, and each is small once it is.
     - **The README does not lead with the speed moat.** Measured 2026-08-01: Quarto appears **once**
       in the README (the earlier "zero times" is rot) and **none** of
       `tools/live-edit-bench/RESULTS.md`'s numbers (cold 123,994.9 µs vs warm 28,425.1 µs, 83×
       smaller payload) appear anywhere in it. The ruling says *lead with the moat*; it does not say
       *name Quarto*.
     - **The GitHub repo is a dead first impression:** the description defines Taliesin in terms of
       Taliesin, `homepageUrl` empty, one topic ("rust"), zero releases, and the README's only image
       is the licence badge — while four screencasts demonstrating the moat sit committed in
       `site/assets/` and appear on no page a visitor sees. (They are MP4; a GIF conversion or an
       uploaded asset URL is needed, not a one-line embed.)
     - **Still absent: a code of conduct and GitHub issue templates**, both only worth doing once the
       repo is public. (`CONTRIBUTING.md` with the inbound relicensing grant and the platform matrix
       both shipped 2026-07-28.)
     - **`taliesin.dev` resolves to nothing** (registered; NS + SPF + a google-site-verification TXT,
       zero web records) and is baked into every canonical URL, `og:url`, sitemap and feed.
       `site/README.md:11-12` already flags it as a placeholder.
     - **The name** (surfaced, not a task): TALIESIN is a live registered mark of the Frank Lloyd
       Wright Foundation (Reg. 4150375). Software is outside the recited goods so legal risk is low;
       the cost is permanent SEO invisibility, and `github.com/taliesin` + `/taliesins` are both
       taken. Renaming twice is worse than a bad search name — if keeping it, always publish as
       "Taliesin — the `.tmd` dev server" so the disambiguator travels.

### P4 — blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

4. **Deck engine mobile polish:** mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode); drop `fitSlide` from the resize path (needs a lazy fit-on-show refactor first). *(The
   desktop trackpad half shipped 2026-07-24.)* **Partly measured 2026-07-27** with synthetic touch
   events: swipe navigation works, a two-finger pinch-in opens the overview, and an overview
   one-finger pan neither navigates nor exits. **What is unmeasured is what emulation cannot reach**:
   a real finger, and overview pan while zoomed *past* fit — at fit scale `clampOv` has nothing to
   pan, so the probe proved only that pan does not misfire. Chromium touch emulation is not evidence
   for a pinch on glass.

41. **R graphics cannot follow the page theme; matplotlib figures can** (M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    The `alt="output"` half shipped 2026-07-29. The theming half was **built and then reverted** on
    2026-07-29, so start from what that measured:
    - **The interception point is NOT the value's repr, and NOT a global `print` override.** ipykernel
      asks the returned figure for a representation; **IRkernel does not**. Printing a ggplot DRAWS
      it, and IRkernel captures the graphics DEVICE and publishes that as `image/png`. Measured:
      `repr::repr_html(p)` returned a correct themed pair while the page still carried one un-themed
      `<img>`; a `print.ggplot` in `globalenv()` never fired for an auto-printed plot; registering
      `repr_html`/`repr_png` for both S7 class names did not reach it; pushing the method into base's
      S3 table **broke a passing test and hung the suite**.
    - **The remaining question is narrow:** which IRkernel seam publishes an auto-displayed plot, and
      can it be given a `text/html` twin-PNG pair. `render_media` already prefers `text/html`.
    - **The twin render is solved:** a `ggplot2::theme()` override of the colour slots plus
      `ggsave(bg="transparent")` at the reader's `repr.plot.*` size produces two genuinely different
      PNGs. Emit the same `tali-fig-light`/`tali-fig-dark` pair the Python side does. **Do NOT
      confuse this with AN-2a, which is fixed.**

18. **Demand-probe residual: the inline-SVG figure path.** (Detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md).)
    So `![](x.svg)` inherits `--tali-*`. Deliberately not built, and the reason is the design problem
    itself: inlining an authored SVG puts its `<style>` selectors and element ids into the page,
    where `.label` / `.ink` / `.axis` from two figures collide with each other and with page CSS. It
    needs a **selector-scoping strategy**, not a change of emitter, and that wants its own spec.
    Edits `render/figure.rs`. *(F-03 and F-02's documented-convention half shipped 2026-07-29.)*

70. **A project with no `_site.yml` declares no boundary.** `build <dir>` accepts a bare directory, so
    a single-document render of one of its pages roots at that page, and the site path's own
    inference can still widen to `.git`. Nothing can infer an undeclared boundary; the fix is for the
    author to declare one. Live instance: `corpus/posts/pca-geometry/` sits under no project marker,
    so `build` of it warns `include not resolved` — true since PT-2 shipped and **now uncovered by
    any test**, since the corpus pin moved to the tech-blog copy. Decide whether that warning is
    correct behaviour or wants a better message before writing code.

104. **Three Wave 1 items whose own round could not verify them.** (Do not build until measured.)
     The `.gitattributes` line that makes `.tmd` behave like `.md` on GitHub (needs linguist-override
     behaviour confirmed); the Jupyter on-ramp that already exists outside the project (needs
     `nbconvert` output confirmed to survive the rename); and the scale ceiling, measured with a
     **runtime-generated fixture that never enters the corpus walker**.

105. **The headless `--no-sandbox` rationale rests on an assumption this round retired.** (LOW.) The
     justification assumed only author-written documents reach the headless path; item 79's family
     says otherwise. Re-derive the rationale before changing the flag.

10. **Two kernel limitations with no clean fix** (dev-facing):
    - **R cold kernels still orphan on ungraceful parent death.** IRkernel has no `ParentPollerUnix`
      equivalent, so there is nothing to arm; PDEATHSIG is the only other lever and is hazardous. R
      is rarely the cold single-doc path, and the warm-pool, cold-Python and `/tmp`-sweep halves all
      landed. `kernel.rs`.
    - **A tens-of-MB cell output blocks ZMQ receive before the cap fires.** `kernel.rs`.

12. **i18n / Unicode: done bar a demand-driven residual.** The LSP UTF-16 fix shipped 2026-07-22
    (detail: [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)).
    *Residual, do not spin up without a real ask: RTL layout, CJK line-breaking, non-ASCII
    heading-slug collisions.*

### P5 — frozen, do not spin up

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused
  2026-07-17. Eviction drops the executor and kills its kernel child processes, so this is kernel
  lifecycle, not a constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not
  missing) interpreter costs ~161 s recovery, downstream of the bounded `interp_id` probe in the
  warm-pool forkserver READY wait + kernel-start retries.
  `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not` shows the *missing*
  case is handled and the *hanging* one is not. *(Aside, pre-existing + load-bearing:
  `crates/server/Cargo.toml` doesn't list tokio's `process` feature though `kernel.rs` /
  `warm_pool.rs` / `exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite
  runs, only when the build is cold. Measured, unexplained, argued test-only. Worth an hour only if a
  real kernel is ever seen outliving its pool.
- **D72 bare `@key`:** declined for now (the diagnostic already ships, so nothing renders wrong
  silently, which makes it a feature question not a defect). Edits `crates/core/src/cite/`, needs
  sign-off if revived.

## Tier 3 — demand-driven (build only when a real user asks)

**This tail held 17 lines until 2026-07-29, when the author promoted every one but the first into the
P1 queue as items 153-174.** Do not re-file a promoted item here: it has a number now, and the number
is where its detail lives.

- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto-to-Taliesin migration +
  portability stress test. **Explicitly declined 2026-07-29** as unnecessary, and kept only because
  the *class* of defect it would surface is real (the same class the external-document audit found,
  item 129). Revive on a concrete portability doubt, not on capacity.

189. **`{{< pdf >}}` embed and a `license:` front-matter key.** (Both S; the research-publishing
     cluster's tail. Detail:
     [2026-07-31-research-publishing-survey.md](2026-07-31-research-publishing-survey.md).) Every
     project-page template embeds a poster PDF in an `<iframe>` (the surveyed template loads the
     Adobe DocumentCloud SDK for this and then never uses it — a plain iframe does the work), and
     states reuse terms in the footer. **Both are knobs**, which is why they sit here: minimal config
     says perfect the default first, and neither has a demanded use yet. `license:` would feed a
     footer badge plus JSON-LD `license` + `isAccessibleForFree`.

**Also deliberately NOT filed** (survey §5): the al-folio / academicpages **publications list** (a
`.bib` rendered as a page with per-entry PDF/code/bibtex badges). It is the personal-homepage job,
distinct from `cite_this.rs`'s outbound single-page citation, and filing it would widen scope on
speculation. Revisit only if the author wants Taliesin to host their own academic homepage.

## Audit lenses — closed, do not open a new round

[AUDITS.md](AUDITS.md) is the round index and a *record*, not a menu. The 14-round slate
([spec](../docs/superpowers/specs/2026-07-27-audit-slate-design.md)) is **complete except R12**,
real-device mobile on Android, which needs the author's phone. Its priority order is in the spec: the
book drawer scroll lock first, then the `--host` QR flow, momentum scrolling and the dynamic viewport
toolbar, tablet widths, TalkBack. **Record explicitly that an Android round does not cover
WebKit/iOS**, or it will later read as full mobile coverage. **An audit's value decays to zero if its
findings never ship** — three waves have shipped, and the P1 queue is now the work.

**Two lenses remain un-run and both are blocked, not declined.** L3: `lsp.rs`, `complete.rs`,
`skim.rs` and `manifest.rs` post-date every lens that would have owned them, though the mutation
campaign has since pinned much of what one would look at. L6: a real external document, blocked on a
repository that is not on this machine.

Durable artefacts, so a later round does not rebuild them: the deck exemption register (R14), the
sensitivity/tradeoff register (R6), the D≥8 detection cluster (R7, now
[DETECTION-DEBT.md](DETECTION-DEBT.md)), the draft ACR (R9, now published in the guide) and the
external-document shape inventory (R11, item 129).

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the
asset and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive
summary is misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict
was overruled; Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** Detail lives in git, in [AUDITS.md](AUDITS.md), in the dated findings docs and
in [LESSONS.md](LESSONS.md) — look there rather than re-expanding this list. A batch's date and
branch are enough to find its commits.

### Shipped

- **2026-08-01 pyodide residuals + layout escapes** (190, 191, 181): a `{pyodide}` cell now runs in
  **every** delivery mode with room for the runtime — verified in a browser running real NumPy, not
  just asserted: `--out <dir>`, a site root page, live deck preview, and a deck inside a site build.
  **The load-bearing find was a defect in 158's shipped work that item 190 did not name:**
  `import(base + "pyodide.mjs")` got a page-relative `_assets/…`, which is a **bare module
  specifier**, so the ROOT page of every site build failed at boot while nested pages worked (their
  `../` is a valid relative specifier) — invisible to every server-side test. `pyodide.js` resolves
  against `document.baseURI` now. **Do not re-file 191: it was already fixed** on 2026-07-31 by
  `49d592c5` and the entry was rot; what was actually missing was the pin, which now exists
  (measured 34.6px). `.column-page`/`.column-screen` are plain classes with **no render path**, and
  there are **FIVE container modes, not the three the item claimed** (single-doc `body`,
  `body.has-toc`, `.tali-site-main`, `.tali-site-main.has-toc`, `.tali-book-main`). **Do not
  "simplify" the two TOC grids back to page-centred:** the rail is text with no background, and a
  centred escape put content under it (measured: escape right edge 1331 vs rail left 1111), so on
  those two modes an escape grows LEFT with its right edge flush to the prose. `overflow-x: **clip**`
  (never `hidden`, which would make `<html>` a scroll container and kill every sticky element) is
  scoped by `:has(.column-screen)`, and `.column-screen`'s gutter is what stops the ~7px full-bleed
  overshoot from clipping the author's own text.
- **2026-07-31 Pyodide cells** (158): a `{pyodide}` fence runs Python in the reader's browser off a
  vendored offline Pyodide + NumPy, in `preview` and a **site** build only; every single-document
  output degrades it to a listing. **`publish` is deliberately NOT on the cell `api`** (it is
  `setup()`'s fourth argument — a masking shield leaked through `Object.getPrototypeOf`), and
  **all cells share ONE interpreter with a global `setStdout`**, so execution is FIFO-serialized
  page-wide. Do not "tidy" either back. Residuals are item 190.
- **2026-07-31 print/PDF track** (159): `taliesin pdf` renders a typeset PDF *from the built HTML*
  via paged.js + CDP. **paged.js is load-bearing, not a fallback** (Chrome 150 implements `@page`
  margin boxes and `counter(page)` but NOT `string-set` or `target-counter()`, measured), it
  **cannot be driven from the Chrome CLI** (`--print-to-pdf` truncates at 2 pages at every
  `--virtual-time-budget`), and `auto: false` fires `config.after` **before** any pagination — stamp
  completion from the `preview()` promise. `eager_media()` exists because a paginated render never
  scrolls, so `loading="lazy"` images never load.
- **2026-07-31 reader affordances** (171, 172, 173, 56's backlinks half): live-HTTP mounts coverage,
  `publish --init`, the three C-READ/C-NAV affordances, and the first cross-chapter references either
  dogfooded book has carried. **Do not re-file C-READ-2's data half** (it is `{{< dataset >}}`, item
  176) and **do not re-file 173**. `@view-transition` moved into `base.css` and
  `corpus/tech-blog/custom.css` was deleted (finishing the 2026-07-11 audit's `#custom-css-mostly-dead`
  prescription); **do not re-add the two dropped prefetch mechanisms.**
- **2026-07-30 long-running cells** (175a + 175b): a cell is capped on **silence**
  (`TALIESIN_CELL_SILENCE`, default 600 s) instead of wall-clock, and a running cell **streams its
  output**. **Do not re-add a wall-clock default** on the theory that runaways are unguarded — a
  streaming runaway never goes silent and is caught by the output caps. Consecutive chunks of one
  stream now merge into a single output (measured: zero drift across the whole corpus).
- **2026-07-30 image optimization** (169): the build derives AVIF rungs behind a `_freeze/img/` cache
  and wraps the byte-identical `<img>` in a `<picture>`. **Do not re-file the WebP half** —
  `image-webp` encodes lossless only, so AVIF is the only pure-Rust lossy encoder. **Never depend on
  `ravif` directly** (it enables rav1e's `asm` feature, which hard-fails the release runners); use
  `image`'s own `avif` feature. `sweep_stale` deletes every output not in `keep`, and keep entries
  must be normalized — `image_derivatives_survive_the_sweep.rs` is the only test that catches it.
- **2026-07-30 editor scope completion** (`FEATURE-IDEAS.md` Session 3 ideas 75-80, 85): the editor
  scope is **CLOSED**. Ideas **67, 72, 74 and 83 were CUT on measured evidence** and **81 by owner
  ruling**; the only editor-surface idea still open anywhere is **86**, filed as item 175(d). The
  engine floor is `^1.101.0` with **`@types/vscode` pinned EXACTLY** (a caret resolves to latest and
  reopens the gap a test now closes).
- **2026-07-30 editor authoring gestures** (ideas 73, 84, 82): six paste/drop gestures,
  rename-repairs-references in both directions, clickable `file:line:` in the terminal. Two custom
  requests (`taliesin/insertEdit`, `taliesin/renameFileEdits`) hold every piece of `.tmd` knowledge;
  the TypeScript owns only the clipboard, the file write and undo grouping. **Do not re-file the
  ideas** or re-derive the four corrections written back into the ideas file. A pasted image lands
  *beside* the document (measured: 24 image refs beside vs 7 in a subdirectory).
- **2026-07-30 editor commands + dev panel** (165, 166, 162, 161): section move / heading promote,
  Format Document, the dev menu's edit annotations and the draft's per-section pass. **166's recorded
  blocker was rot** (`BlockOp::SetMeta` had always handled line shifts); the prose *reflow* is
  declined on measured grounds instead — 86 of 174 corpus documents are hand-wrapped, so there is no
  house style to enforce. `ctrl+shift+k` deliberately shadows `editor.action.deleteLines`.
- **2026-07-30 LSP editor ergonomics** (178, 177): inlay hints, folding, document highlight, selection
  ranges, plus visible math delimiters, with **zero new TypeScript** — the capabilities reach
  Zed/Neovim/Helix too. `didChange` is coalesced in a 120 ms window (measured: one publish on the
  largest guide page is 33 ms in a debug build, so debouncing alone sufficed and the anchor scan got
  no memo); `pending` is a **list**, or an edit to B discards the diagnostics owed to A.
- **2026-07-30 transclusion, datasets, site preview** (179, 180, 176, 160, 150): a chapter under a
  `_site.yml` previews as its *project*. **180 is closed by ruling** — all three inlay-hint kinds
  stay on by default.
- **2026-07-29/30 site bibliography + SEO board correction** (163, 168): `bibliography:` in
  `_site.yml`, merged UNDER each page's own, plus the unused-entry lint (site-wide by necessity).
  **168 was already shipped and the entry was rot** — `sitemap.xml`, `robots.txt` and the JSON-LD all
  emit; **grep `site/seo.rs` first.** `render_single_doc` now reads the nearest `_site.yml`'s key, so
  `preview post.tmd` and `preview <dir>` render one document.
- **2026-07-29 explorable cluster** (153-157): the cell-language registry with `{glsl}` as its proof,
  the `num` global (**keep it curated** — what a *drawing* cell needs, not a numeric library),
  `animate` + `point`, `tali.state`, and `tali.tex` / `tali.table` (**not** KaTeX-the-parser; only
  its CSS + fonts are bundled). The `animate` tick is a `type="number"` field, not `type="hidden"` —
  the latter hands every downstream cell the *string*. Two coverage illusions were deleted rather
  than shipped and live in [DETECTION-DEBT.md](DETECTION-DEBT.md).
- **2026-07-29 ruled-and-built batch** (101, 122, 71, 78, 149's buildable half, 18's doc halves, 41's
  `alt` half, 150's risk half): `LICENSE-OUTPUT-EXCEPTION.md` is an **additional permission under
  AGPL §7** covering what Taliesin *emits* (deliberately **no** per-asset licence headers); `check`
  names the interpreter it would use and still spawns nothing; both deck-on-touch behaviours; figure
  text on a data fill keeps its own colour.
- **2026-07-29 the demand tail's "LSP for language intelligence" line was DELETED as stale, not
  promoted** — `taliesin lsp` exists, the companion was rewritten as a thin client over it on
  2026-07-28, and the `#| label:` completion drift it cited is closed and pinned. **Do not re-file
  it.**
- **2026-07-29 first-hour + positioning** (144, 151, 87, 88, 94, 95, 96, 135, 136): eight CLI /
  diagnostic / LSP residuals, the two lying ui-audit probes, the first-run execution notice, and
  `docs/guide/using/choosing.tmd`. **Three filed causes were false.**
- **2026-07-29 `taliesin build site` no longer 404s its own CTA:** `site/build.sh` builds all 8
  projects into one tree, and **the ordering is load-bearing** — the parent build's `sweep_stale`
  deletes anything under the output dir it did not write, so a mount built first is silently swept
  away, and re-running `taliesin build site` alone afterwards puts you back to the broken tree.
  Pinned by `site_build_script.rs`.
- **2026-07-28 block model + docs gate** (138, 146, 143's path half): every block has exactly one root
  element; prose is gated against the tree rather than a needle list. **`notes/` and
  `docs/superpowers/` are excluded from that gate and must stay excluded** — they are dated records.
- **2026-07-28 deck harness** (112, 125, 113, 111): `deck.js` has a browser test; deck content is
  auditable at 0 violations across 100% of slides. It found **two shipped layout defects on its first
  run**, neither visible to any emission test. The eleven deck shapes 113 listed stay deliberately
  unbuilt.
- **2026-07-28 honesty + build cost** (91, 110, 115, 119, 126, 134, 143): `chromiumoxide` is an opt-in
  `headless-js` feature, off by default; not linting `draft:` pages is **ruled correct** and the
  defect was the silence; the ACR is published; DETECTION-DEBT.md is the live register.
- **2026-07-28 verified sweep** (85, 86, 97, 98, 99, 114, 123, 130): a `theme:` extension bundle is
  contained; no built page fetches off-origin; a shortcode source is a path, not a URL; both
  `jsconfig.json` include lists are globbed.
- **2026-07-28 critique-round client/LSP/manifest** (139, 140, 141, 142): rename validates the new
  name; `toc_html` stopped double-escaping an explicit heading id; the Cmd-K palette locks the
  background scroller; the web manifest stops shipping Taliesin's brand. **The splash colour is
  deliberately one light value** — a manifest cannot express an OS-conditional colour.
- **2026-07-28 reader cost** (150's Phase B half, 137, 124): the body typeface ships as
  content-hashed files, not base64 in the render-blocking sheet (**125 KB gzipped off every page's
  critical path**); the three conditional blobs are written only when something links them.
- **2026-07-28 publication readiness** (84, 89, 90, 92, 93): `tools/gates.sh`, `CONTRIBUTING.md` with
  the inbound relicensing grant, `ci.yml` + `release.yml` **guarded inert until the repo is public**,
  the measured install expectation and platform matrix, and "Coming from Quarto".
- **2026-07-28 launch blockers** (79-82, 109, 117, 118, 120, 121, 127, 128): `mounts:` is contained;
  `check` does not spawn a project-supplied interpreter; `--no-exec` covers `{js}`; a deck in a site
  is validated; comma fences highlight; a migrated link gets a did-you-mean. **`--no-exec` is
  deliberately NOT a sanitizer** (2026-07-03 CSP ruling).
- **2026-07-28 item 83 — the five pre-relicence MIT tags are deleted** (owner-approved; none had ever
  been pushed). All five commits stay reachable from `main`. **Never tag before the licence is
  settled**, and cut a release tag only from a tree whose `LICENSE` matches `Cargo.toml`.
- **2026-07-27 item 76 — a book has no right-rail TOC** (owner ruling, reversing 2026-07-06). The gate
  is `Site::page_toc`, ahead of the page's own `toc:`. **Do not re-scope as "give books their TOC
  back" or as "delete the rail everywhere"** — websites and single documents keep it.
- **2026-07-27 item 77** (the 72-75 residuals): shortcode arguments linted against a closed
  vocabulary; `TAL-SHORTCODE` is its own WARNING family; `favicon:` resolves like `logo:`. **A book
  with neither title nor logo still emits no brand link, deliberately.**
- **2026-07-27 mutation campaign** (58-69): every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned. **Do not
  re-run it against the same scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66** (`404.html` links the shared `_assets/` bundle; its hrefs are root-absolute
  on purpose) and **item 67** (the `~/.local/bin/taliesin` launcher exits early for `__complete` only,
  24.3 s → 0.024 s per tab press; **`completions` is deliberately NOT exempt**).
- **2026-07-26 deck weight + headless bounding** (52, 55): a site deck went 4.6 MB → 7 KB via a
  separate `deck.<hash>.{css,js}` pair. **A deck cannot link the page's `app.js`** — `search.js` would
  steal Cmd-K. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path parity** (50, 51, 57): `render_single_doc` decides the single-document containment
  root once (nearest `_site.yml`, **never `.git`**); `TOC_SHEET_MARKUP` is the one copy all four
  assemblers emit. **Do not re-scope as "give the single-file build the inferred root"** — that is a
  revert of `9359a2c`.
- **2026-07-26, earlier**: migration UX (53, 54); the mobile batch (42-49); owner rulings 24, 17 (book
  breadcrumb ruled **no**) and 2 (deck presenter tools declined); reporting surfaces 39, 40; demand
  probe #4 (`corpus/analyst`); AP1-R1 and DOCS-2/3/4/5.
- **2026-07-25 and earlier, closed:** AP7-1..5 (a11y), AP3-1, AP11-1 (`TAL-KERNEL`), DIAG-1, DOCS-1,
  AP3-3, PA-M3, PA-M13, PA-H1's residuals, the backlink-context + resume batch, book wayfinding, the
  hardening batch, book-level `theorems:`, live-executor mounts (F-04), book-aware `read`, AP8-1's
  output scrub, DET-1, the DX audit batch, `taliesin lsp`, DX17(a)+(b), the deck audit, the polish
  audit batch, the PMF builds, corpus coverage, the machine-facing audit, AI-native packaging, the
  R/Python ANSI leak, ungraceful-death reaping, and the `assets/js` `tsc` gate.

### Numbers retained, never reused

Each closed by a ruling or folded into another item, kept so a later round does not re-derive them.
**25** — the pre-public flip procedure, folded into **100** (its options (a)/(b)/(c) were settled by
that ruling). **116** — the positional cascade vs a Python DAG, CLOSED, do not build; reactivity is
marimo's well-made claim while reproducibility is unclaimed by anyone, so tell the cascade story
instead. **132**/**133** — R8's value-stream pricing; a deck's defects are found by an *audience*,
the latest and most expensive point in the stream. **145** — retired into 137. **147** — retired
into 101. **151** — closed 2026-07-29. **182** — hover previews for citations and cross-refs; **filed
2026-07-31 against a false measurement and deleted 2026-08-01 unbuilt**, because `site/hover.rs` +
`code-enhance/12-link-preview.js` shipped it on 2026-07-06 (server-rendered, cross-page, with section
headings deliberately excluded).

### Decided against

- **"Adjacent slides bleed into the deck's letterbox" (DT-5, filed and RETRACTED 2026-07-27):**
  **false — the letterbox is empty.** The probe intersected each neighbour with the **viewport**
  instead of with its **clipping ancestor**. **Do not re-file it from a rect measurement**; the only
  valid evidence is a rendered pixel.
- **Deck presenter tools** (one-command publish, laser/spotlight, auto-advance): declined 2026-07-22
  and **re-declined 2026-07-26** on the same grounds — no real speaker ask. Revive only when the
  author actually presents from Taliesin. (`footer:`/`logo:` from that item did ship.)
- **A prose reflow / hard-wrap formatter** (2026-07-30, item 166): 86 of 174 corpus documents are
  hand-wrapped and 379 prose lines pass 100 columns, so there is no house style to enforce. The
  render-identical subset shipped instead, gated by `formatting_the_whole_corpus_renders_identical_html`.
- **WS op-message batching** (declined 2026-07-25 **on measurement, premise confirmed**): the worst
  case is 55 ops in one frame, but a warm edit is 32.2 ms of which the diff is 0.94 ms, so batching
  saves ~220 bytes on a 32,303-byte payload. Reopen only if render cost drops far enough that framing
  is measurable.
- **Item 29's reduction residuals R1 + T2** (closed 2026-07-25 without code): R1's `text_content` /
  `indexable_text` fork is deliberate and equalizing them would leak raw entities into `llms.txt`;
  T2's "three modules pre-scan" is partly rotted.
- **Deck-motion, whole item** (detail: [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)):
  Option A + residuals shipped; **(3) no-change** ruled; **(4) Option C (shared-element FLIP)
  declined — do not re-cost it a third time**. A coverage-weighted refinement of (5) measured *worse*.
- **A separate per-page outline artifact for the book drawer** (declined 2026-07-25 while building
  it): the index it would duplicate is already lazy-loaded on every page.
- **`drawer-typeahead`** (declined 2026-07-25): Cmd-K plus the drawer's collapsible outline covers it,
  and a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (2026-07-25): `prose::word_count` excludes fenced code
  and math, so a code-heavy chapter is understated — and reading code is *slower* than prose, so the
  error goes into a promise about the reader's time in the wrong direction. (`is_article` is
  test-pinned; do not touch it.)
- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (resolved 2026-07-25):
  measured across every book in the repo, only 3 of 48 chapters differ and in 2 the `# H1` is the
  *better* nav label. Resolved as documentation, not code.
- **CAD-as-code** (`{openscad}` / CadQuery cell → live 3-D preview; researched 2026-07-23, NOT built):
  technically feasible and legally green, killed on **demand**. **Do not bundle openscad-wasm (GPL).**
  Five named revisit triggers in [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md).
- **2026-07-22 rulings:** DX16 update-nudge = **skip** (a version check is network egress that
  undercuts the offline-first identity); cross-ref label i18n = **defer**; item 9's design questions
  documented as intentional.
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables,
  line-level code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt
  substrate in `14-reader-prefs.js`).
- **TODO / FIXME surfacing** (owner ruled 2026-07-10): no `level` concept exists, so a TODO warning
  would fail `check` on every draft. If revived, a preview-only `Diagnostic::info` beats re-plumbing
  a real `level`, and the scan must NOT reuse `prose::strip_inline`.
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution, the
  numeric-claim-without-citation hint, and a per-page text/JSON sidecar.
- **Refuted by measurement (do NOT re-scope):** heading demotion **already ships**; `build` does not
  leak forkserver subtrees; the warm pool booting Python on prose-only builds is hygiene, not
  latency; dev attributes are 0.29% of page bytes; a `--version -dirty` marker is
  stale-by-construction; the `assets/css` stale-embed claim did not reproduce (re-verify for
  `assets/js` before any touch-render workaround); include symlink-loop SIGABRT does not exist;
  **decks pass path parity outright**, and `mounts:` differs from direct serving by 4 bytes.
- **`_redirects`/`_headers` preserved, never generated** (`build.rs` treats them as author-placed
  deploy metadata; `stale_sweep.rs` pins it).
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex,
  schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face`
  extras filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll
  (not pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals.
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1;
  the deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features. When publishing, lead the copy with the **speed moat** (warm server, block-level
incremental, no per-edit rebuild), the single most-repeated Quarto grievance and the most
under-marketed asset.
