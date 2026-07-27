# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. Method lessons that
> outlive their item go to [LESSONS.md](LESSONS.md). The "do not re-add" list at the bottom is a
> compact anti-rot guard, **one line per entry**, not a changelog.

## State (2026-07-28)

- **A six-critic adversarial round landed 2026-07-28 and refilled band A, which the previous
  state described as holding no code work.** Method and full findings:
  [2026-07-28-launch-critique.md](2026-07-28-launch-critique.md) (1,088 lines). Six hostile
  critics on disjoint surfaces, then a defender per critic whose job was to refute; only
  findings the defender could not kill became items. **14 commits landed on branch
  `critique-pass-2026-07-27`** (not merged, not pushed) — read that branch before starting, or
  you will re-fix what is already fixed.
- **The round's own lesson is the one to carry ([LESSONS.md](LESSONS.md) candidate — not filed
  there yet, because a parallel session held that file open): a fix lands in one file and misses
  its sibling.** It happened three times in one session, twice to fixes made *during* the round —
  `THIRD_PARTY.md` was corrected to AGPL while `docs/internals/repository.tmd` still said MIT;
  `deny.toml`'s header lost its CI claim while a comment twelve lines below kept one. Both were
  caught only because a defender re-read the fixed files. **Fix the class, grep the repo for the
  shape, and gate on the shape — never on the sentence you happened to fix.**
- **The structural cause is already named in this file and is now item 87.** No gate compares
  committed prose against behaviour, so `check`, `fmt`, `clippy` and 1,673 tests all pass over a
  false sentence. `crates/core/tests/stale_docs.rs` is the only gate that tries, and this round
  found it checked the wrong files with the wrong needles *and* carried a test asserting a live
  23 KB module was deleted. Repaired twice, both mutation-verified — but it still covers only
  deck vocabulary and CI claims.
- **Three of the critics' proposed fixes were wrong in ways that would have shipped a NEW false
  sentence**, and the defender caught each. Before applying any fix text from the findings doc,
  read its correction note. The worst: a proposed defence-in-depth would have dropped `"tmd"`
  from `SKIP_EXT`, which `mirror_assets` uses to *exclude* sources — it would have copied every
  `.tmd` in the project into the deploy.
- **A defender also refuted one of the orchestrator's own findings outright** (the stale
  "mermaid is the sole CDN dep" sentence is in `notes/`, not in the shipped `THIRD_PARTY.md`,
  which is accurate and drift-locked). Findings from this round are *adjudicated*, not assumed.
- **Do not trust this file's freshness.** The author pushes mid-session with no signal here, and
  a scoped prune leaves the rest looking freshly reviewed. **No commit counts and no SHAs are
  recorded** — a count written *into* this file is invalidated by the commit that writes it.
  Ask git: `git log --oneline origin/main..main`.
- **Gates at the last code landing (2026-07-28, the critique branch), re-run before trusting
  them:** full workspace suite with all three interpreter gates and `--test-threads=1` =
  **99 binaries, 1,673 tests, 0 failures, 0 ignored** (zero ignored is the check that the gates
  were live, not skipping); `cargo fmt --check` and `clippy --workspace --all-targets -D
  warnings` clean. The **fourth** gate (`TALIESIN_REQUIRE_CHROME=1 --test read_run_js`) was run.
  Both JS `tsc` gates clean, as was `node --test crates/server/src/assets/_middleware.test.mjs`
  (6 pass). `check` clean on all 7 corpus/docs projects exercised. Built guide + site carry
  **0** leaked `.tmd` sources and **0** unrewritten `.tmd` hrefs (both were 1 before).
- **Nothing is owed by the author except the band C rulings below.** The permanent
  click-to-source coverage gap still stands: the relay harness passes both directions but stops
  at the relay and cannot see whether the editor lands the cursor, so any change to the relay or
  the companion re-opens a manual check.

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
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what
  is unpushed and `git reflog show origin/main` before believing any "not pushed" claim.
- **How this file lies to you:** entries rot. Before picking an item, **grep its named symbol/flag in
  source** and prefer measuring the running product over reading this file. Trust an item's
  *symptom*, never its cause, line number or stated cost. Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. **The full trap catalogue — probes,
  instruments, cargo-mutants scoping, the coverage illusions — is in [LESSONS.md](LESSONS.md); read
  it before writing a probe or a pin.**

## Audit lenses — the menu, since the table in AUDITS.md is not one

[AUDITS.md](AUDITS.md)'s round index is a *record*: a further round needs a lens proposed first.
Ranked; take from the top. **L1, L2, L4 and L5 have run and closed.**

**Standing recommendation — real-device mobile.** The 2026-07-26 round was Chromium emulation, which
does not model WebKit, momentum scroll, the dynamic viewport toolbar or safe-area insets. Everything
it should cover is the "Not measured" list in [2026-07-26-mobile-audit.md](2026-07-26-mobile-audit.md):
real iOS Safari / Android Chrome, a phone screen reader, tablet widths, and the `--host` QR
phone-preview flow, which is a first-class phone feature that got no coverage at all.

**Never run:**

- **L3. The subsystems that post-date every lens that would own them — PARTIAL.** `headless_js.rs` was
  read; `lsp.rs` (1,922 lines, 07-21), `complete.rs` (1,157), `skim.rs` (647) and `manifest.rs` (303)
  were not, though the mutation campaign has since pinned much of what it would have looked at.
  `lsp.rs` is younger than the security (07-17), DX (07-18), mutation (07-18) and polish (07-19)
  rounds, and only AP10 has read it. The web manifest is a *phone* surface (add-to-home-screen,
  standalone display) the mobile round did not touch.
- **L6. A real external document — BLOCKED** on a repository that is not on this machine. All four
  demand probes were fixtures written for the probe; the FL-weather Quarto book (Tier 3) is the fifth
  and the only one the corpus cannot fake.

**Re-runs, ranked by age × churn measured in each round's own surface (2026-07-26):**

- **The deck audit (07-12) is the most rotted:** 2,510+/1,196- in `deck.rs` + `deck.js` + `deck.css`
  since, and the mode-model was deliberately reshaped after it (reader + PDF deleted, phone feed
  added, motion round 07-24). AUDITS.md already warns the doc describes *outgoing* behaviour. Re-run
  it **crossed with touch**, not as-is.
- **The website/brand audit (07-11):** its headline performance finding measured per-page inlining and
  is now obsolete (hashed `_assets/`), which is itself the signal. Its Lighthouse pass was
  desktop-mode only, which is how it missed the touch-target defects the mobile round found.
- **The security release audit (07-17)** should wait for the flip date it is parked on (item 25),
  **except** `headless_js.rs` and the LSP, which post-date it and spawn or expose processes.
- **Not due:** AP10 (07-23). **Closed, do not re-scope:** the mutation / vacuous-test round — every
  survivor it measured is triaged, and a re-run's only new information would be about code written
  since. Its numbers and method are in [LESSONS.md](LESSONS.md) plus
  [server half](2026-07-27-mutation-server-half-complete.md) and
  [`lsp_nav.rs`](2026-07-27-mutation-lsp-nav-complete.md).

**Unblocked by progress already made:**

- **Real iOS Safari / Android Chrome, a phone screen reader, the `--host` QR flow** — the author is
  now device-testing.
- **Deck touch gestures** (item 4) — device blocker gone, and the mobile round confirmed the feed
  itself works, so pinch/pan is testable.
- **Fuzzing the LSP + MCP request loops** (an AP2 residual). HEALTH-1 shipped, so `serve::guarded`
  wraps both dispatches (`lsp.rs:105`, `mcp.rs:127`): there is finally a survival property to assert.
- **Reader-surface work that needed section extents** — `data-section-end` shipped 07-26, so the four
  skimmability proposals blocked on "zero `<section>` extents" have substrate.
- **Still blocked:** the prune half of the release audit (gated on the public-flip date), and true
  WebKit unless the phone is an iPhone.

**Auditing is otherwise done for now.** Four fresh lenses on 2026-07-26 produced zero HIGH findings,
while the one round that produced four came from the author using the tool on a phone.

## Open items

**Ranked for implementation, not by theme.** Band A is what a session can build today; B is buildable
but not worth a session alone; C, D and E are blocked and are listed so they are not re-scoped.
**Item numbers are stable** and referenced from the findings docs and [AUDITS.md](AUDITS.md): they are
NOT renumbered when the order changes, and a closed item's number is never reused.

**Standing rule for a batch:** branch per batch, verify each fix by *mutation*, browser-verify
anything client-side, and **delete the item from this file when it lands**.

### A. Build now

**Refilled by the 2026-07-28 critique round.** Every item below was conceded by a defender that
tried to refute it, and every one carries a measured repro in
[2026-07-28-launch-critique.md](2026-07-28-launch-critique.md). **Read the branch
`critique-pass-2026-07-27` first** — 14 commits already fixed the deck source leak, the LSP
languageId gate, the stale-prose gate (twice), the AGPL/MIT contradiction, the false CI claims,
the landing page's imaginary pen tool, and ~10 other false doc claims. Ranked.

79. **A block with more than one root element is only half-mounted, which silently inverts the
    headline claim.** `web-client/client.js:1014` `fragment()` returns `firstElementChild`, so
    `update`/`insert`/`remove` (`:1325`/`:1341`/`:1365`) operate on one of N roots. Measured on
    the **shipped** `corpus/descent` explainer: editing a slider's `max` changes the block id —
    so the op *looks* applied — while the DOM keeps the old value, and deleting the block
    strands the extra roots in the page forever. Preview then disagrees with what `build`
    publishes, which is the one thing the block model exists to prevent. Blast radius across 161
    corpus+docs documents: **2 pages, 6 orphan roots** (`corpus/descent/index.tmd`,
    `corpus/diagnostics/a11y.tmd`). **Fix server-side** — give every block exactly one root, the
    invariant `crates/core/src/site/backlinks.rs:338` already asserts for its own emitter. A
    client-side fix re-derives the same invariant in the wrong layer. *This is the one item in
    this band that touches the block model; it deserves its own session and a mutation-verified
    pin that a multi-root block round-trips through a swap.*

80. **`textDocument/rename` corrupts source, and reaches outside the document.** Two defects in
    the feature `CLAUDE.md` calls *the* sanctioned way to edit source.
    - **No validation.** `lsp.rs:523-526` rejects only a blank name. Measured: `F2` → `my
      section` emits a 3-edit `WorkspaceEdit` producing `{#my section}` (not an anchor — see
      `is_id_char`, `lsp_complete.rs:320`) and rewrites every reference to match; a newline in
      the name splits the heading line in two. Fix: validate against the anchor grammar and
      return a `ResponseError` so the editor surfaces it in the rename box. Leave
      `resolve_prepare_rename` alone.
    - **It rewrites external URL fragments.** `lsp_nav.rs:310` `is_anchor_site` treats any `#`
      before the id as a definition sigil, so renaming a section retargets
      `[x](https://example.com/other.html#sec-a)` to a fragment on someone else's page. Gate the
      `'#'` arm on the sigil actually opening a `{#…}` attribute. **Note the mutation campaign
      measured 29 mutants / 0 survivors here** — that proved the implemented rule is faithfully
      pinned, not that the rule is right. The missing shape is a fixture with an outbound link.

81. **The web manifest is a phone surface no audit had read, and all three defects ship.**
    (`crates/core/src/site/manifest.rs`.)
    - **An installed site shows *Taliesin's* logo on the author's home screen.** `resolve_icons`
      (`:58-72`) looks only for literal `icon-192.png`/`icon-512.png`; with `favicon: acme.svg`
      set, the emitted `icon-192.png` is byte-identical to the bundled one. `check` says "no
      problems found" and the convention is documented nowhere. A trademark artifact on a
      stranger's phone.
    - **`start_url: "./"` is emitted for a site with no `index.html`** (`:121-128`), so an app
      installed from a subpage cold-launches into a 404 — and `display: standalone` removes the
      address bar, so the reader cannot navigate out.
    - **`theme_color`/`background_color` are hard-wired `#ffffff`** (`:17`, `:124`) while the
      page default resolves dark on a dark phone: white splash, then `#16181d`. The existing pin
      `manifest_color_matches_the_tali_bg_token` (`:279`) asserts the light value is faithfully
      duplicated, which is the wrong invariant to be asserting.

82. **Two TOC escaping defects, one of which ships a dead link in the published build.**
    - **`toc_html` double-escapes an explicit heading id** (`render/mod.rs:2502` `escape_attr`
      over an id `toc_items` already read out of escaped HTML). `## R&D notes {#r&d-notes}` →
      anchor `r&amp;d-notes`, href `#r&amp;amp;d-notes`. Dead in the **build**. The comment three
      lines above at `:2498` already documents this exact hazard for `text` and was never
      applied to `id`. Auto-slugs are unaffected, which is why it stayed invisible.
    - **`buildToc()` interpolates a decoded `h.id` into an `href` then assigns `innerHTML`**
      (`web-client/client.js:868`, `:873`); confirmed code execution in preview. Calibrate
      honestly: a `.tmd` body already passes raw HTML through, so for your own document this is
      not privilege escalation — but any `{#id}` with `"`/`<`/`&` corrupts the nav markup, and it
      is the one place the client re-serializes DOM text into HTML (`search.js:694` deliberately
      does not). Fix with `createElement` + `setAttribute`, as
      `code-enhance/19-book-outline.js:150` already does.

83. **The Cmd-K palette claims `aria-modal="true"` and never locks the background scroller.**
    Measured with a real PageDown: the page scrolled 787 px underneath an open palette, on a
    published site. Two sibling overlays already do it right
    (`code-enhance/11-lightbox.js:77`, the book drawer). Restore a *saved* overflow value, not
    `''`, or it unlocks a drawer that was already open.

84. **The docs-vs-behaviour sweep: ~20 false claims across the guide, the Internals book and
    ROADMAP.** All measured, all with paste-ready replacements in the findings doc.
    **Read each item's correction note first — three of the critic's proposed fixes were wrong.**
    Highest value first: Mermaid is documented as a CDN dependency in 4 places though it is
    vendored and offline on every path (**and `TALIESIN_MERMAID_URL` is preview-only and inert
    in `build`, which none of them say**); `_site.yml` problems are documented as located
    warnings but are unlocated **errors** (3 places in `configuration.tmd`, 2 in the Internals
    book) — **but `build --strict` does NOT fail on them, only `check` does**; `format:`
    sub-keys are documented as deliberately unlinted in a third live copy at
    `validation.tmd:44-45` and in `frontmatter.rs:17-18`, though the validator lints them and the
    code says no extension mechanism exists; the Internals book misdescribes the diff mask,
    prints a `SiteApp` struct and a `PageIncludes.resources` field that do not exist, undercounts
    the protocol's twelve message types as nine, and describes the loopback-Origin allowance as
    always-on when `--host` drops it (**edit `protocol.tmd:318`; `:321` is TRUE**); ~20 stale
    module paths remain in the Internals book; "book has a sidebar" survives in **11 live
    instances across 5 files** (do NOT re-scope as "restore the rail", item 76); ROADMAP's
    *normative* guardrails section still says `.qmd`/`qmdEnhancers` (**the named corpus pin docs
    DO exist as `.tmd`** — do not "fix" this by authoring new corpus documents); and `init` is
    documented as writing two files in **four** places while it writes five plus `.taliesin/`.

85. **Diagnostic and CLI residuals a first-hour user hits.** Each small, each measured.
    A timeout-killed cell is reported to the console as "raised an uncaught exception" because
    the timeout has no `NOT_RUN_` kind (`kernel.rs:866` bypasses the marker) — **and the guard
    test at `build.rs:3027` passes vacuously for the live path, so fix the test in the same
    commit**; single-doc `build` prefixes diagnostics with a bare `file_stem()` that no editor
    can open (thread a display label — `fallback` is load-bearing for the freeze path *and* the
    page title, so do not just swap it); the missing-kernel warning prints twice, the short form
    adding nothing; a missing input file reports `(os error 2)` with no did-you-mean though
    `closest` already exists; `skim` is missing from `taliesin --help` (add the `COMMANDS` ↔
    `usage()` parity gate modelled on `env_help_lists_every_runtime_env_var`); `codeAction`
    builds quick fixes from **any** provider's diagnostic and ignores the requested range; a
    message after `shutdown` exits 1 (editors read that as a crash) while a bare `exit` exits 0;
    CRLF `documentSymbol` ranges run one column long.

86. **Every site build ships ~4.43 MB of assets no emitted page references** — mermaid
    (3,572,004 B) + jslibs (487,117 B) + katex (369,346 B), i.e. **92%** of a prose-only
    `_site/`. Verified by href, not by grep: the only `_assets/` references in a prose page are
    `app.css` + `app.js`. **The "uniform bundle" defence fails in the author's own hand** —
    `write_asset_bundle` already gates the deck pair conditionally with the comment "a site
    without a deck should not pay for a file nothing links" (`build.rs:1247-1249`), four members
    away. **But the critic's proposed fix cannot work in place**: the predicates
    (`ship_katex`/`has_js_cells`/`has_mermaid`, `render/page.rs:276-290`) are evaluated against
    the *rendered body*, and `write_asset_bundle` runs at `build.rs:1506`, before any page is
    rendered. Either hash eagerly and flush lazily during the page loop (guard against the
    `--jobs N` determinism pins), or pre-scan page *sources* for the three triggers — the latter
    is over-inclusive, which is the safe direction, since under-inclusive means a live 404.

87. **Widen the prose-vs-behaviour gate — the structural item this whole round argues for.**
    `crates/core/tests/stale_docs.rs` is repaired and mutation-verified in both directions, but
    still covers only deck vocabulary and CI claims. Every stale-string finding in this round is
    a symptom of the same gap this file already named ("no gate compares prose against
    behaviour"). Candidates that would have caught real defects here: a retired-front-matter-key
    needle set built from `RETIRED_KEYS` (would have caught `about:` in the README and the
    marketing site); a check that no doc names a module path that is not a file; a check that
    documented CLI flags exist in `COMMANDS`/`usage()`. **Follow this file's own rule
    (`:558-559`): mutation-check every new needle against exactly the shape it guards, or the
    gate is worse than none.**

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

### B. Buildable, but low yield on its own

**Empty.** Item 77's four residuals were the last occupants and shipped 2026-07-27. The band's own
lesson held again: an item here is cheap to build and therefore easy to build *without asking whether
it should be*, and **one of the four closed on evidence rather than code** (77's scree plot was filed
as unreadable-on-a-light-page and measured perfectly readable, while the figure it never named was
the broken one). Refile here only after re-deriving the cause from source.

### C. Blocked on an owner ruling (not a task until then)

71. **Two deck-on-touch behaviours that are working-as-written, and may be working-as-wrong**
    (DT-3 + DT-4, detail: [2026-07-27-deck-touch-audit.md](2026-07-27-deck-touch-audit.md)).
    Neither is a bug; both are a choice someone made that the touch crossing put a number on.
    - **A slow swipe does nothing.** Measured: 200 px in ~30 ms navigates, the same 200 px over
      **750 ms** does not (`deck.js:1859`, `dt > 600`). A swipe's time bound normally separates a
      swipe from a pan/scroll — but in stepped mode there is no competing one-finger gesture to
      separate from (`deck.feed` returns at `:1798`, `deck.overview` at `:1799`, both above it, and
      the stepped stage does not scroll), and the 50 px distance floor already rejects a tap. So in
      the only mode where the bound is live it can *only* reject input the reader meant, and what it
      rejects is the slow deliberate swipe a motor-impaired reader makes. Proposed: drop `dt` in
      stepped mode, keep the distance floor. **No real user has been observed failing on it.**
    - **The share panel says "Point a phone here" — to a phone.** The QR takes most of the card and
      is the one useless half on the device reading it; Copy is the action that works and is
      secondary. Panel geometry is otherwise correct at 390 px (nothing clipped, QR legible).
      `navigator.share` was absent under emulation, **so the Web Share option was not measured and
      is not claimed**.

88. **What licence governs a page a user publishes?** Raised by a fix that landed
    2026-07-28, so it is a *new* obligation, not a deferred one. `THIRD_PARTY.md` said "Taliesin
    itself is MIT licensed" while `LICENSE` is AGPL-3.0-only; correcting it (`6f68386`) forced
    the question the contradiction had been hiding. **Taliesin's own runtime JS is `include_str!`'d
    into every page a user builds** (`render/mod.rs:1658-1660` and the `code-enhance/` fragments):
    a measured probe page is 1.2 MB with 13 `taliEnhancers` hits and **zero** licence statements.
    If that runtime is AGPL, arguably every page a user publishes is an AGPL work — an adoption
    tax far larger than §13's, and one that lands on *document authors* rather than on a hosted
    competitor. The standard remedy is an explicit output exception (the GCC runtime-library /
    Bison-output pattern) or an MIT carve-out for the emitted runtime. **Decide before publishing
    anything**, because the first published page fixes the answer in the wild.
    - Second, separable question the same finding raised: **AGPL vs MPL-2.0.** §13 is *not*
      inapplicable here (a `--host` LAN preview is network interaction, `LICENSE:542-548` +
      `SECURITY.md:44-47`), so the critic's "nobody stands in that hole" framing is wrong — but
      the tax lands on the adopters `:577` says the project currently needs. MPL keeps file-level
      copyleft, passes most corporate bans, and ***REMOVED***
      `deny.toml` protects. **Whichever is chosen, the reservation at `README.md:156-158` is
      fiction the moment one outside PR merges without a CLA or DCO.**

89. **Distribution: there is no way to get this except `cargo build --release`.** Measured:
    `gh release list` empty, `git ls-remote --tags origin` **empty** (the local `v0.2.0` and four
    `stable-*` tags were never pushed), and crates.io `taliesin` / `taliesin-core` /
    `taliesin-server` all 404 — i.e. all three names are free. No Homebrew, Nix, or install
    script. The audience for a documentation tool is not the population that will install a Rust
    toolchain and wait out a measured **2m59s** cold build.
    - **Prerequisite the critic missed and the defender found:** `cargo publish` will *reject*
      this workspace as-is. `Cargo.toml:14` declares `taliesin-core = { path = "crates/core" }`
      with **no `version`**; add `version = "0.2.0"` first.
    - Also blank on crates.io without it: no `keywords`, `categories`, `readme`, `homepage` or
      `documentation` in any manifest, so the crate pages would carry one description line and
      nothing else. Watch `crates/core` = 7.3 MiB tracked against the 10 MiB `.crate` cap.

90. **Launch presentation, all gated on the flip.** Grouped because none is actionable until the
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
    - **No platform statement anywhere**, while `/proc` is read directly in five places with
      `#[cfg(not(unix))]` fallbacks that `LESSONS.md:88` records as never executed by any test.
      One honest README line converts every non-Linux issue into an expectation.
    - **No CONTRIBUTING / CoC / issue templates / CLA or DCO**, while `README.md:156-158`
      reserves a relicensing right that the first merged outside PR silently ends.
    - **`taliesin.dev` resolves to nothing** (registered, NS + SPF + a google-site-verification
      TXT, zero web records) and is baked into every canonical URL, `og:url`, sitemap and feed.
      `site/README.md:11-12` already flags it as a placeholder.
    - **`taliesin build site` 404s its own primary CTA** — `docs/guide/` and five `gallery/*`
      mounts are preview-only, and `site/README.md` documents an 8-command build that nothing
      runs. Worse than filed: `--strict` exits **0** on it and `check` says "no problems found",
      so both automated gates bless a deploy the tool has already warned about. A `site/build.sh`
      is the cheap fix; counting mount warnings as `--strict` problems is the durable one.
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

### D. Blocked on a device, a real user, or working-as-intended

Kept visible so they are not re-scoped. Revive on a real signal, not on capacity.

78. **The figure recolour has no notion of "text sitting on a data fill", so it can *cause* the
    contrast failure it exists to prevent** (P3, filed 2026-07-27 while fixing item 77's fourth
    residual; item 41's family). `MPL_THEME_PREAMBLE`'s `_tali_recolour` sets **every** `Text` in a
    figure to the reader's foreground. That is right for titles, axis labels and ticks, which sit on
    the transparent page background — and wrong for an annotation drawn *inside* a data-coloured
    mark, whose background does not change with the theme. **Measured** on
    `corpus/tech-blog/posts/pca-geometry/`'s covariance heatmap: the `1.00` cells are near-black
    `#67000d`, so in the **light** render the annotation is recoloured to near-black `#1a1a1a` on
    near-black and is effectively illegible; the dark render is fine. The author cannot fix it in the
    document — an explicit `color=` on the annotation is exactly what the recolour overrides, which
    is what makes this a tool item and not a corpus one.
    **Not obvious how to fix, which is why it is filed rather than done.** Matplotlib does not mark
    which `Text` is "on" a mark, so candidates are all heuristics: skip a `Text` whose axes-fraction
    position lands inside a filled artist; skip `Text` parented to a `QuadMesh`/`AxesImage`; or pick
    per-annotation black/white from the *underlying* fill's luminance instead of the page
    foreground (what matplotlib's own `annotate` helpers do). **Do NOT "fix" it by dropping the
    recolour** — that reinstates the baked-foreground bug the preamble exists for.

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

18. **Demand-probe (interactive-explainer) residuals** (P3; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md)):
    - **F-02 (gap, P3):** an authored numbered figure is emitted as `<img src="fig.svg">`, and an
      `<img>`-embedded SVG is style-isolated: it can't see `--tali-*` or the theme toggle, only the
      **OS** `prefers-color-scheme`. So a reader who forces the page theme opposite their OS gets the
      figure in the wrong palette. Inline `{js}`/SVG graphics on the same page track the toggle fine.
      Candidates: an inline-SVG figure path so `![](x.svg)` inherits page vars, or a documented
      neutral-palette convention. Edits `crates/core/src/render/figure.rs`.
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the
      cell body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the
      first paint. Gate teardown on `invalidation`, not DOM attachment. Candidate: a doc line in the
      `{js}`-cell reference, or an optional post-mount hook.

41. **R graphics cannot follow the page theme; matplotlib figures can** (P3, M; detail:
    [2026-07-26-corpus-demand-probe-analyst.md](2026-07-26-corpus-demand-probe-analyst.md), AN-2b).
    Taliesin renders every inline matplotlib figure **twice** (light + dark foreground) and swaps them
    on the theme toggle (`kernel.rs`'s `MPL_THEME_PREAMBLE`); measured on `corpus/analyst/` the Python
    figure emits two genuinely different PNGs and the ggplot figure emits one, so a mixed-language
    report has half its figures track the reader's theme and half baked. **Blocked on being a feature,
    not a fix:** a real version re-renders the figure twice against two foregrounds. **Do NOT confuse
    this with AN-2a, which is fixed** — the R device no longer paints opaque white under a transparent
    figure; the *ink* is still baked at one colour, and that is what is left. The documented workaround
    (a neutral mid-grey palette) is the second instance of the convention named in item 18's F-02.
    Minor and separable: an R figure is emitted `<img alt="output">` where the Python pair is `alt=""`;
    both sit inside a captioned `<figure>`, so `alt=""` is right and `"output"` is noise read aloud.

70. **A project with no `_site.yml` declares no boundary** (P3, filed 2026-07-27 from the path-parity
    batch's "surfaced, not fixed"). `build <dir>` accepts a bare directory, so a single-document render
    of one of its pages roots at that page, and the site path's own inference can still widen to
    `.git`. Nothing can infer an undeclared boundary; the fix is for the author to declare one. Live
    instance: `corpus/posts/pca-geometry/` (the loose twin of the tech-blog page, byte-identical to it
    and pinned so by `twinned_corpus_sources_stay_byte_identical`) sits under no project marker, so
    `build` of it warns `include not resolved` — true since PT-2 shipped and **now uncovered by any
    test**, since the corpus pin moved to the tech-blog copy. Decide whether that warning is correct
    behaviour or wants a better message before writing code.

### E. Gated, not actionable now (do not spin up)

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

## Tier 3: demand-driven (below every band above; build only when a real user asks)

**Waits on demand, not on capacity.** The PMF audit's verdict is that what is missing is **real users,
not more features**, so nothing here is scheduled. One line each; the reasoning lives in the linked
audits.

- **An end-to-end live-HTTP test for `mounts:` serving.** The F-04 work unit-pins the pure
  `match_mount`/`resolve_project`/`classify_change` helpers and live mount serving is browser-verified;
  what is missing is only the bin-crate gap of a real `reqwest`/`TcpListener` harness. Mounts are
  preview-only, so this waits for a reason to exist.
- **Companion (Phase 2):** editor commands (insert block / reorder slide) — strictly `.tmd`-buffer text
  transforms in the editor, never preview gestures.
- **LaTeX hover-preview in the VS Code editor** — a sub-case of the LSP item below: a `HoverProvider`
  resolving `@fig-2` to "Figure 2", a front-matter key's doc, or a `[@key]` reference, over data
  `vocab`/`symbols` already carry.
- **`.tmd` format-on-save** (open question). A source pretty-printer would write the editor *buffer*
  (the allowed surface) but must preserve `data-sourcepos` line stability for click-to-source.
  Brainstorm whether the reflow is worth the click-to-source risk before any work.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test (exercises `book.rs`, includes, the freeze cache, file-mode portability). If
  it renders clean, consider pinning a reduced version under `corpus/`.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic, kernel-free and
  network-free).
- **`taliesin publish` follow-ups:** an optional `--init` wrapper for the one-time `wrangler` setup.
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote one only with a
  corpus pin).
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec (RFC-2119
  dialect + protocol reference), `{glsl}` cell-language registry, SEO completeness (sitemap/robots/JSON-LD
  at publish with `url:`).
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only, so a growing
  blog retypes keys per post and nothing reports an unused or duplicate entry. Allow `bibliography:` in
  `_site.yml` merged under each page's own, plus two **read-only** diagnostics ("entry never cited",
  "duplicate key"). Explicitly does not touch the BibTeX parser / CSL formatter.
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
  count and a badge per node for unresolved xref / TODO / over-goal length; click to scroll. This is the
  *revision* view, not the reader TOC. Scope it as an annotation layer on the dev panel, or it grows to L.
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
  word delta (`+340 / -180`) plus a feed of the last N ops, each click-to-source. Honest caveat: the pin
  is behavioural (a `tools/live-edit-bench` assertion), not a corpus doc.
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series
  without copy-paste drift. Must ride **on top of** the `includes.rs` source-map pass (resolve the
  fragment to a block range, hand the existing machinery a sub-slice), never rewrite it. Hard merge
  gate: the source map must not perturb.
- **LSP for the language intelligence, browser stays the view** (L). Everything an LSP needs is already
  in Rust (`check`, `vocab`, `register_xref`, the bib parser, `closest()`); it is write-once for
  Neovim/Helix/Zed/VS Code and removes the drift that causes the `#| label:` completion gap (JS regexes
  reimplementing Rust knowledge). An LSP cannot render the preview and does not need to.
- **Image optimization** (large): WebP/AVIF transcode + responsive `srcset` + lazy-load behind a
  content-hashed asset cache. Deferred until posts get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild; mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** the `Cargo.toml` workspace comment names `serde_yml`, which
  carries RUSTSEC-2025-0068 (unsound + unmaintained); `serde_norway` is 1+ yr stale. The maintained
  continuation is **`serde_yaml_ng`** (v0.10). No urgency (trusted local config; 0.9 still builds). If
  0.9 ever breaks against a future serde/edition, swap, gated on a test that `Error::location().line()`
  still works. Fix the stale comment when touched.
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C): a
  document-level reader show/hide-code toggle, a reader code+data download affordance, instant
  client-side navigation polish. Each waits on a real ask.

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Do not re-add / re-scope

**One line per entry.** The detail is in git, in [AUDITS.md](AUDITS.md), and in the dated findings
docs; look there rather than re-expanding this list.

### Shipped

- **Deck PDF export: already deleted (2026-07-12 deck audit, A2), do not re-scope "remove it."** Asked
  again 2026-07-27; pinned gone by `render/tests.rs:1950`. What survives is ~25 lines of `@media print`
  in `deck.css:522` that keep a stray Cmd/Ctrl+P legible — **that is a don't-emit-garbage guard, not
  PDF export, and it is already free, so keep it.** (The stale *marketing claims* are live work: item 75.)
- **2026-07-27 item 76 — a book has no right-rail TOC.** The gate is `Site::page_toc`, ahead of the
  page's own `toc:`, so a page-level `toc: true` cannot reinstate it and all four assemblers share one
  decision. **Do not re-scope as "give books their TOC back"** (owner ruling, reversing 2026-07-06)
  **or as "delete the rail everywhere"**: websites and single documents keep the rail, `toc-spy.js`
  and the shared `TOC_SHEET_MARKUP` (still the one copy — a book simply never reaches it). `toc:` is a
  website key now, and `validate_toc_scope` tells a book author the key is inert.
- **2026-07-27 the drawer marks which section of the open chapter you are in** (author-asked, the
  natural completion of 76: the expanded chapter row was the only section-level surface a book had
  left). `.tali-book-section-active` + `aria-current="location"` on the current chapter's panel only,
  off the same `scroll-margin-top` activation line as `toc-spy.js`. **Do not re-scope as "give the
  drawer a scrollspy"** — it is computed on each open, deliberately: the drawer locks the root
  scroller, so nothing can move while it is on screen and a scroll listener would watch a dead event.
- **2026-07-27 item 77 (the four 72-75 residuals):** shortcode arguments are linted against a closed
  vocabulary with did-you-mean, and shortcode diagnostics became the **`TAL-SHORTCODE` WARNING**
  family instead of falling through to `(TAL-CHECK, ERROR)`, where a one-letter typo blocked
  `build --strict`/`publish`. `favicon:` resolves through `chrome::site_asset_href` like `logo:`
  (site-absolute and external pass through unprefixed). A book brands on `logo:` alone; **a book with
  neither title nor logo still emits no brand link, deliberately.** The fourth was refuted — see State.
- **2026-07-27 mutation campaign (items 58-69):** every measured survivor in `crates/core`'s five
  post-07-18 files, the ten `crates/server` files and `lsp_nav.rs` is triaged and pinned; the
  unkillable ones are recorded in the two findings docs' tables. **Do not re-run it against the same
  scope.** Method in [LESSONS.md](LESSONS.md).
- **2026-07-27 item 66:** `404.html` links the shared `_assets/` bundle (355,700 → 16,185 bytes on
  `corpus/tarn`); its hrefs are root-absolute on purpose, so a project-subpath deploy degrades to
  unstyled rather than mislinking. The preview keeps the self-contained form.
- **2026-07-27 item 67** (outside the repo, `~/.local/bin/taliesin`): the launcher exits early for
  `__complete` only — 24.3 s → 0.024 s per tab press. **`completions` is deliberately NOT exempt**
  (run by hand, generates a shim from the binary's own command list, so stale is wrong there).
- **2026-07-26 deck weight + headless bounding (items 52, 55):** a site deck went 4,583,261 → 6,962
  bytes via a separate `deck.<hash>.{css,js}` pair (**a deck cannot link the page's `app.js`** —
  `search.js` would steal Cmd-K); every headless browser phase is bounded with teardown kept
  reachable. The standalone artifact stays 4.4 MB and self-contained on purpose.
- **2026-07-26 path-parity batch (items 50, 51, 57, PP-1..3):** one document now renders the same
  whichever command renders it. `render_single_doc` decides the single-document containment root once
  (nearest `_site.yml`, else the doc's own directory); `TOC_SHEET_MARKUP` is the one copy of the
  mobile-sheet chrome all four assemblers emit; the single-doc preview ships Cmd-K. **Do not re-scope
  as "give the single-file build the inferred root"** — that is a revert of `9359a2c`.
- **2026-07-26 migration UX (items 53, 54):** a pre-rename `_quarto.yml` is no longer silently
  defaulted, and retired keys carry the scope they were retired from. Both messages append to the
  classified prefix, so neither needed a new diagnostic code.
- **2026-07-26 mobile batch (items 42-49, MOB-1..8):** the tree now asks what device it is on
  (`hover`/`pointer` media features; it had none). Deck menu drops its keyboard legend + hint badges
  and gates Speaker view on capability instead of orientation; the ⌘K badge is hidden on touch at any
  width; copy-code shows and the heading anchor dims on touch; the book drawer locks page scroll and
  keeps focus through outline hydration; touch nav targets grow by overlay; the sticky book topbar
  truncates instead of wrapping.
- **2026-07-26 owner rulings (items 24, 17, 2):** `section-extents` shipped as option (b) —
  `data-section-end` on every heading block, extents nesting, heading-inclusive, stopping before
  generated furniture, decks excluded; `book-breadcrumb` ruled **no** (D114 stands); a vendored MIT
  PowerShell `.sublime-syntax` consulted last; deck presenter tools declined again.
- **2026-07-26 reporting surfaces (items 39, 40):** AN-5 (an unnumbered cross-page `@sec-` now names its
  target instead of rendering the bare word "Section"), AN-6 (per-document validation no longer reports
  valid cross-page refs as `TAL-XREF-UNDEF`; scope, not severity), AN-3 + AN-4 documented.
- **2026-07-26 demand probe #4, the analyst** (`corpus/analyst/`, the only corpus project running two
  languages in one document): AN-1 (a labelled `tbl-` cell with no `<table>` no longer emits a dangling
  xref) and AN-2a (`KernelSpec::r` carries `options(repr.plot.bg = "transparent")`) fixed.
- **2026-07-26 audit batch:** AP1-R1 (the freeze cache was capped by entry *count*, never by bytes; a
  16 MB `MAX_BYTES` budget now bounds it) and DOCS-2/3/4/5 (`about:` purged from 28 places across 6
  guide pages nine days after its removal, plus three smaller drifts).
- **2026-07-25 band-A batch:** AP7-1..5 (a11y), AP3-1 (a bypass lane for cell-free rebuilds), AP11-1
  (`TAL-KERNEL`), DIAG-1 (eight diagnostics catalogued + a zero-`GENERIC` gate), DOCS-1.
- **2026-07-25 band-B batch:** AP3-3 (the kernel port re-roll), PA-M3 (listing list semantics), PA-M13
  (`image:` without `image-alt:` warns), PA-H1's residuals (deck `theme-color` + social meta).
- **Earlier, closed:** the backlink-context + resume batch, the book-wayfinding batch, the hardening
  batch, book-level `theorems:`, live-executor mounts (F-04), structure-preserving book-aware `read`,
  AP8-1's output scrub, the DET-1 reproducibility guard, the DX audit batch, `taliesin lsp`, DX17(a)+(b)
  headless executed output, the deck audit, the polish audit batch, the PMF builds, corpus-coverage, the
  machine-facing audit, AI-native packaging, the R/Python ANSI leak, ungraceful-death reaping, and the
  `assets/js` `tsc` gate.

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
