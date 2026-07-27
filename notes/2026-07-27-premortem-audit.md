# Pre-mortem R3 — Taliesin, mid-2027

> **Premise, asserted as fact.** It is July 2027. Taliesin was published. It failed. Not
> "underperformed" — failed: the repository is archived or effectively so, the last
> substantive commit is months old, the issue tracker holds unanswered reports, and the
> author has moved on. This document explains why that happened.
>
> Method order was enforced: (1) assert; (2) **independent silent generation**, unfiltered,
> unclustered, no evaluation, owner rulings deliberately NOT consulted; (3) consolidate;
> (4) cluster, rank, mitigate. The raw list in §2 is evidence and is preserved verbatim.
> Filtering happens in §4 and never in §2.

---

## 1. The three framings

Each framing is run as a separate story, because a pre-mortem that tells one story has
only found one failure mode.

- **(A) Nobody adopted it.** It was published, it got a spike of attention, and the
  attention did not convert into a second user. The moat was real and nobody could reach it.
- **(B) People adopted it and then left.** A first cohort arrived, used it for weeks, and
  churned at a predictable moment. The churn moment was structural, not accidental.
- **(C) It succeeded and became unmaintainable.** Adoption worked, and success killed it:
  one person could not carry the load the success created, and the project's own quality
  machinery was the first thing that broke under it.

---

## 2. RAW UNFILTERED REASON LIST (step 2 — verbatim, kept as evidence)

**Rules of this section, stated so a reviewer can check them:** written before any
evaluation, clustering or filtering. Ordering is generation order, not rank. Entries
contradict each other on purpose. Owner rulings (AGPL, HTML-only, the `exec_pool.rs`
freeze, the "do not re-add" list) were **not** consulted while writing this. Several
entries are impolitic about the author, the development method, or settled decisions;
they are kept exactly as generated. Framing tags: [A] nobody adopted, [B] adopted then
left, [C] succeeded then unmaintainable, [X] cross-cutting.

1. [A] The install story is `git clone && cargo build --release` against 343 crates with a C
   dependency (oniguruma, pulled in via comrak's syntect feature unification). The first
   thing a prospective user does is wait five to ten minutes for a build they did not want.
   Most of them close the tab instead.
2. [A] No binary releases, no crates.io publish, no Homebrew, no Nix, no Docker image, no
   `cargo install`. Six git tags exist and none of them is a release artifact anyone can
   download.
3. [A] Windows is not supported and nowhere says so. `cfg(unix)` appears across
   `warm_pool.rs`, `exec.rs`, `serve/mod.rs`, `runtime_dirs.rs`, `headless_js.rs`,
   `includes.rs`, `complete.rs`; there is exactly one `cfg!(target_os = "windows")` runtime
   branch and zero Windows-gated implementations. A Windows technical writer clones it, it
   fails, and that person tells other people it is broken rather than that it is unsupported.
4. [A] The name is unsearchable and legally encumbered. TALIESIN is a live registered mark of
   the Frank Lloyd Wright Foundation (reg. 0853377 covers old-style IC 035-042, which reads
   onto modern class 42 software/technology services). "Taliesin" also returns an
   architecture school, a Welsh bard and a preservation charity before it returns anything
   about documents.
5. [X] The `.tmd` extension throws away the entire Markdown tool ecosystem for zero user
   benefit. GitHub will not preview it, Prettier will not format it, no linter knows it, no
   editor other than the author's own companion highlights it. The author paid a real
   ecosystem tax to own a file extension.
6. [A] AGPL-3.0-only. Corporate legal at every company where a technical writer works has a
   standing rule against AGPL, including for authoring tools whose output is not derivative.
   The people best positioned to adopt are the people who structurally cannot.
7. [A] The README states the author "reserves the right to offer Taliesin under other terms,
   including a proprietary hosted service or a commercial license." Read by a would-be
   contributor, this says: your patch funds someone else's future proprietary product, and a
   CLA demand is coming. That sentence costs more contributors than the AGPL does.
8. [A] There is no CONTRIBUTING.md. There is a CLAUDE.md and an AGENTS.md. The repository has
   written instructions for AI agents and none for humans, and that reads exactly as badly as
   it sounds.
9. [X] 1,581 commits from one author between 2026-06-15 and 2026-07-27 — roughly 38 commits a
   day, 90,000 lines of Rust in six weeks. Anyone who checks the git history concludes this is
   an LLM-generated codebase. In 2027 that is a reason to not adopt, fairly or not.
10. [X] And the conclusion is partly right, which is the uncomfortable part: if most of the
    90K lines were agent-authored, the author's own mental model of the codebase is thinner
    than the line count implies. That gap does not show while the author is in daily contact
    with it. It shows the first month he is not.
11. [A] `notes/` is tracked: 63 files, `AUDITS.md` at 130 KB, `backlog.md` at 47 KB, 48 dated
    audit findings docs. A skeptical reader opens `backlog.md` and finds a curated, dated,
    honest list of the tool's known defects. The honesty is admirable and it is also the
    single most effective anti-marketing document in the repo.
12. [A] `docs/superpowers/` is tracked too (69 files). A public repo shipping its own
    AI-agent plan corpus tells the reader what the development method was, and the reader
    forms the opinion in item 9 without having to look at git.
13. [A] The docs are two `.tmd` books that only render with the tool. To read the manual you
    must first build the thing the manual explains. Highest-friction documentation possible:
    the evaluation loop requires committing to the tool first.
14. [A] There is no marketing site live at launch. `site/` exists with placeholders and the
    author's own policy defers it ("feature-first"). A Show HN with no landing page, no demo
    video and no hosted example converts nobody.
15. [A] There is exactly one launch. Attention is a one-shot resource; a project gets one
    front page. Publishing before binaries, before Windows clarity, before a demo, spends it.
16. [X] The moat is invisible. "Block-level incremental updates preserving live component
    state" is architecture; what a user sees is a page that updates fast, which every SSG's
    HMR also claims. The PMF audit already says the problem is legibility and nothing was
    built to fix it (`live-edit-hero-demo` is still unchecked in ROADMAP.md).
17. [A] The measured moat is ~124 ms cold vs ~28 ms warm on one document. That is a
    ~100 ms improvement. Real Quarto users complain about multi-second to multi-minute
    renders on *large* projects — a scale the corpus is explicitly forbidden from
    representing. The benchmark proves the moat on exactly the size where nobody feels pain.
18. [X] Quarto 2 is a Rust rewrite targeting "long-standing performance problems." When it
    ships, the single differentiator that was going to lead the marketing copy is gone, and
    Quarto keeps PDF, LaTeX, Word, an IDE, a company, a docs site and 100k users.
19. [X] Racing a funded incumbent to its own announced roadmap item is the losing move
    available. By the time Taliesin is publicly known, its headline claim is contested.
20. [B] HTML-only is the churn mechanism, not a purity question. The academic user adopts for
    drafting and leaves the day a submission portal demands a PDF. That is not a rare event;
    it is the terminal event of every academic document's life, and it happens to 100% of
    users at the moment of highest stakes.
21. [B] There is no importer. `.qmd`, `.Rmd` and `.ipynb` aliases were deliberately deleted.
    The only realistic adopter population is Quarto users, and their migration path was burned
    on purpose. A user with 200 notebooks cannot move and will not hand-port.
22. [B] There is no exporter either. HTML-only plus no importer equals a roach motel: content
    goes in and cannot come out in any other shape. A careful person refuses on that basis
    alone, and they are being rational.
23. [B] The clean-break, no-deprecation culture is stated in the roadmap: retired keys are
    deleted, not deprecated ("the key is deleted, not deprecated"). Efficient for one author.
    For a user it means an upgrade breaks their site with a diagnostic instead of a shim.
24. [B] `0.2.0`, pre-1.0, no semver commitment, no published compatibility policy. Nobody
    builds a book they need in 2030 on that.
25. [B] Cell execution has a documented stale-hit class already found once (AP4-1, `#| cache:
    false` not propagating on cold build). One unexplained stale figure in a published post
    destroys trust in the whole tool permanently, and cache bugs are the hardest class to
    attribute — the user will blame the tool for something else and leave without reporting.
26. [B] Two known robustness holes are open: a deep-`>` stack overflow and a comrak O(n²)
    bracket hang (AP2-1, AP2-2). A user pastes a big or hostile file, the server dies or
    hangs, and the "warm server" pitch dies with it.
27. [B] Scale is unmeasured and forbidden from being measured in the corpus: `corpus/tarn` is
    12 chapters and the backlog says do not grow it and do not mint `corpus/longbook`. The
    first real textbook author is the scale test, in production, on their book.
28. [B] The warm-page eviction (`MAX_WARM_PAGES` + LRU in `exec_pool.rs`) is frozen and
    documented as "not test-guarded, so it breaks silently." Eviction kills kernel children.
    A 100-page site preview with live kernels is a memory profile nobody has ever run.
29. [B] Real-device mobile was never tested. The whole reader-side value proposition is "share
    a link instead of a PDF" — and half the recipients open that link on an iPhone. Item 76
    just made the drawer a book's only navigation surface, and the drawer's scroll lock was
    measured only on Chromium.
30. [B] The reader-side is the value prop and the author-side is the pitch. The PMF audit says
    this outright. Every marketing sentence sells the author loop; every user's *audience*
    experiences the reader loop; nobody optimized the conversion between them.
31. [B] A kernel wedges, the user loses twenty minutes, and there is no second person to ask.
    One bad session ends a trial.
32. [B] Environment management is one env var. Real data science is per-project environments
    (conda/uv/pixi). `TALIESIN_PYTHON` per shell across five projects is a papercut that
    recurs daily, which is the worst kind.
33. [B] The dialect is five sub-syntaxes marketed as Markdown: `:::` fenced divs, `#|` cell
    options, `//|` JS options, `{{< >}}` shortcodes, `@fig-`/`[@key]` refs, plus YAML front
    matter and `_site.yml`. Learning cost is real and is under-advertised.
34. [B] "Minimal config: prefer a better default over a knob" means, from the outside, "you
    get the author's taste or nothing." Serif body, 46rem measure, no right-rail TOC in books,
    no reader text-size control. Every one of those is defensible and every one of them is a
    reason a specific person leaves.
35. [B] The answer to a feature request will be a well-reasoned refusal citing an internal
    audit document. Correct, and it reads as arrogance to someone who just arrived.
36. [B] "Demand-driven" is a churn engine once users exist. The user asks; the user *is* the
    demand; the policy still says wait. The policy was designed for a world with zero users
    and does not survive contact with two.
37. [B] The preview executes code on open. Opening a `.tmd` from the internet runs it. This is
    documented in SECURITY.md as by-design and it is still one blog post away from being
    "this document tool runs arbitrary code when you open a file."
38. [B] `--host` binds the preview to the LAN behind a per-session token in the URL. Someone
    runs it on café wifi, someone else writes it up, and the trust cost lands on the whole
    project regardless of the token's adequacy.
39. [X] `serde_yaml` 0.9 is archived upstream and sits at the config core; the named
    fallback `serde_yml` carries RUSTSEC-2025-0068. A published project's dependency audit
    surfaces this in week one.
40. [X] `cargo audit` and `cargo deny` are now manual, run by the author, on dependency
    changes he remembers to check. 343 crates. That will rot, and it will rot silently.
41. [X] The offline guarantee has greppable holes. `MERMAID_DEFAULT` is a live jsdelivr URL
    compiled into the binary; the vendored mermaid loader carries the same URL as a fallback
    string in built output; `docs/guide/using/code.tmd` fetches an earth texture from
    unpkg.com in a documented example. Someone greps a built page for "jsdelivr", finds it,
    and posts "the offline-first tool phones home." Being technically unreachable does not
    survive a screenshot.
42. [C] The pre-push hook is on the author's machine via `core.hooksPath`. The moment a
    second person can push, `main` breaks and nobody learns about it until the author pulls.
43. [C] A contributor's PR runs nothing. There is no `.github/` at all. The maintainer *is*
    CI: every PR costs him a full local `cargo test --workspace` plus four environment gates,
    two `tsc` runs, a node test and a browser check. At 5 PRs a week that is the whole week.
44. [C] Worse: a contributor's own green run is meaningless. The kernel suites *skip* silently
    without an interpreter (`TALIESIN_REQUIRE_KERNEL` / `TALIESIN_REQUIRE_R` exist precisely
    because of this). A contributor reports "all tests pass" having run a fraction of them.
45. [C] Contributing a feature requires adding a corpus document, and the corpus is the
    author's personal blog (`corpus/tech-blog` is the forward-facing brand). The contribution
    process requires editing the maintainer's personal writing. Nobody will do that and
    nobody should have to.
46. [C] `body_html_snapshots.rs` couples tests to blog content; `corpus.rs` pins twinned
    corpus sources byte-identical; `makeScene3D` exists in four copies pinned to stay
    duplicated. A newcomer's first instinct (dedupe, tidy) fails the suite, and the failure
    message will not explain why the duplication is load-bearing.
47. [C] No issue templates, no labels, no discussions, no triage. The first hundred issues
    arrive unstructured and half of them are "how do I install it on Windows."
48. [C] SECURITY.md promises an acknowledgement within about a week from one person. One busy
    month — a thesis deadline, a job start — and there is a public advisory with no patch.
49. [C] The audit culture consumed the author's entire capacity at *zero* users. Forty-eight
    findings docs, half a megabyte of notes, in six weeks. At real user load it is the first
    thing to be dropped, and it is the only quality mechanism the project has.
50. [C] And the audit culture was already showing diminishing returns before launch: four
    fresh analytical lenses on 2026-07-26 produced zero HIGH findings, while the author simply
    *using* the tool on a phone produced four. The machinery was producing paperwork, not
    defects. That is worth saying out loud.
51. [C] `notes/` is the externalized memory and it is documented as untrustworthy by its own
    header ("Do not trust this file's freshness", "How this file lies to you: entries rot").
    A maintainer returning after six months, or a successor, cannot tell fresh from stale in
    500 KB of prose. The memory does not transfer.
52. [C] Bus factor 1 with no succession, no second committer, no organization, no funding, no
    governance line. If the author stops, the project stops the same day and there is no
    mechanism by which anyone learns that it stopped.
53. [C] A fork cannot easily continue it: AGPL is fine for forking, but the name is
    trademark-encumbered, so a fork must rename, which suppresses exactly the dynamic that
    keeps abandoned single-author projects alive.
54. [X] The author is a student. A thesis, a graduation, a job with an IP-assignment clause,
    a relationship, a burnout — any one of these ends 38-commits-a-day, and several of them
    are scheduled events, not risks.
55. [X] There is no telemetry, correctly, and therefore zero visibility into what anyone
    actually uses. Post-launch prioritization stays author-taste-driven forever, which is the
    same failure mode as pre-launch but now with users watching.
56. [X] The update-nudge was declined on offline-first grounds (DX16). So there is no
    channel by which a user running a vulnerable build ever learns to upgrade. Users run
    stale binaries indefinitely and the author cannot reach them.
57. [X] i18n/RTL is a stated non-goal in a published roadmap: "deliberate non-gaps for a solo
    English author." Publishing that sentence tells every non-English author they are not
    welcome, and tells every institution with accessibility/localization procurement rules
    that the tool is out of scope.
58. [A] Academics cite tools. There is no DOI, no Zenodo archive, no JOSS paper, no citable
    artifact. An uncitable tool is invisible in the literature and therefore invisible to the
    segment with the most to gain.
59. [B] The "single editing surface" principle forbids the single most-requested thing a new
    user will ask for (edit in the preview), and the answer is a design-philosophy essay. The
    essay is right. It is also the shape of an answer people stop asking for things after.
60. [A] The tool is feature-complete for one user, and the PMF audit says so. That means the
    second user's first blocker is unmapped territory, and it will surface in week one, and
    the hour in which they would have accepted a workaround is an hour the author was asleep.
61. [X] "Corpus-plus-roadmap" means scope grows on the author's judgment of what a corpus doc
    should demand. With users, that reads as an arbitrary gate on other people's needs, and
    the gate is operated by a person with no time.
62. [C] 90K lines of idiosyncratic Rust plus a "Do NOT touch" list plus one standing freeze
    plus four env gates plus a browser gate means contributor onboarding is measured in weeks.
    Nobody spends weeks to fix someone else's typo bug.
63. [X] The whole project is a monument to one person's taste, built at a velocity nobody can
    review, defended by audits nobody else read, documented for agents. That is a description
    of an artwork, not of a product, and artworks do not acquire maintainers.
64. [B] The freeze cache, the warm pool and kernel lifecycle are where silent wrongness lives.
    A reproducibility tool that is ever silently wrong is worse than no tool, and the class of
    bug is exactly the class that a single author with a green suite cannot see.
65. [A] Nothing about the project makes a promise a stranger can verify in under five minutes.
    Every claim (speed, offline, block model, click-to-source) requires building the tool
    first. The evaluation funnel has one step and it is the expensive one.
66. [C] Success would mean the author spends his days on issue triage for a tool he built to
    write with. The thing he loses first is the reason he built it.

---

## 3. Consolidation

66 raw reasons collapse to 21 distinct causes. Merges made:

- 1, 2, 3, 13, 15, 65 → **evaluation funnel has one expensive step** (install/build/read).
- 4, 53 → **name is legally and semantically unclearable**.
- 5, 21, 22, 33 → **the dialect and format boundary have no on-ramp and no off-ramp**.
- 6, 7 → **licensing posture repels both companies and contributors**.
- 8, 43, 44, 45, 46, 47, 62 → **contribution is physically impossible**.
- 9, 10, 12, 51, 63 → **the development method does not transfer to any second person**.
- 11, 12 → **the published tree markets against the product**.
- 14, 16, 17, 30, 55, 60 → **the value proposition is neither legible nor measured where the
  pain is**.
- 18, 19 → **the differentiator is on a funded competitor's announced roadmap**.
- 20 → **HTML-only meets its terminal event in every academic document's life** (settled;
  costed, not reversed).
- 23, 24, 34, 35, 36, 59, 61 → **the policies that made solo velocity possible become the
  churn engine with users**.
- 25, 26, 27, 28, 64 → **unmeasured scale and silent-wrongness classes**.
- 29 → **the reader surface, which is the value prop, is unverified on the devices readers
  use**.
- 31, 32 → **environment and kernel papercuts recur daily**.
- 37, 38, 48 → **security posture is one write-up plus one slow week from a crisis**.
- 39, 40, 41 → **supply chain and offline claims are unattended and greppable**.
- 42, 49, 50 → **the only quality gate is local to one machine, and the quality culture was
  already past its yield**.
- 52, 54, 56 → **bus factor 1 with scheduled interruptions and no reach-back channel**.
- 57 → **stated non-goals exclude whole populations in writing**.
- 58 → **not citable, therefore invisible to the segment with most to gain**.
- 66 → **success destroys the author's own use case**.

---

## 4. Clustered failure register

Plausibility = probability this cause materially contributed, given what is in the repo
today. Impact = how much of the failure it explains on its own. Both on 1-5.

### Cluster 1 — The evaluation funnel (framing A). Plausibility 5, Impact 5.

Raw: 1, 2, 3, 13, 15, 65, 14, 16.
The single most likely proximate cause of "nobody adopted it." Everything a stranger would
find compelling is behind a 343-crate release build on a Unix-only tree with no downloadable
artifact, no hosted docs and no demo. Attention is one-shot and it was spent on a funnel with
one very expensive step. **This is the cluster that decides the whole outcome and it is
almost entirely fixable with mechanical work.**

### Cluster 2 — Contribution is physically impossible (framings A + C). Plausibility 5, Impact 4.

Raw: 8, 43, 44, 45, 46, 47, 62, 42.
No CONTRIBUTING, no runnable gate off the author's machine, silently-skipping test gates, and
a contribution contract that requires adding a corpus document to the maintainer's personal
blog. Under framing A this means the project never grows a second person. Under framing C it
means every PR is billed to the maintainer's own week, which is the burnout mechanism.

### Cluster 3 — The method does not transfer (framing C). Plausibility 4, Impact 5.

Raw: 9, 10, 12, 51, 63, 49, 50.
90K lines in six weeks, an agent-facing memory corpus that documents its own rot, and a
quality culture whose yield was already measurably falling before launch. The project's
knowledge lives in one head plus 500 KB of prose that says not to trust it. This is the
cause that makes recovery impossible after any gap, and it is the one nobody will name to
the author's face.

### Cluster 4 — Policies that were assets become the churn engine (framing B). Plausibility 4, Impact 4.

Raw: 23, 24, 34, 35, 36, 59, 61, 21, 22.
Clean-break-no-deprecation, demand-driven scope, prefer-a-default-over-a-knob and the
single-editing-surface rule are all correct for one author and all read as refusal to a
cohort. Combined with no importer and no exporter, the first cohort has high switching cost
in and high switching cost out, which is the profile of a user who leaves loudly.

### Cluster 5 — Scale, silent wrongness and the reader surface (framing B). Plausibility 4, Impact 4.

Raw: 25, 26, 27, 28, 64, 29, 31, 32.
Two open hang/overflow classes, an already-found stale-cache class, a frozen and
untest-guarded eviction path, a corpus capped at 12 chapters by policy, and a reader surface
never tested on the devices readers use. Each of these is a single bad session, and a single
bad session ends a trial when there is no support channel.

### Cluster 6 — The differentiator is contested and unproven where it matters. Plausibility 4, Impact 4.

Raw: 17, 18, 19, 30, 55, 60.
The measured moat is ~100 ms on a small document; the pain that drives Quarto users to look
elsewhere is on large projects the corpus is forbidden from representing; and Quarto 2's Rust
rewrite targets exactly that. Leading the copy with speed, as the backlog recommends, aims at
a claim the incumbent has announced it will take.

### Cluster 7 — The published tree markets against the product (framing A). Plausibility 3, Impact 3.

Raw: 11, 12, 57, 41.
`notes/` (130 KB of audits, a dated defect roadmap), `docs/superpowers/`, `.claude/`,
CLAUDE.md/AGENTS.md, a roadmap that writes "deliberate non-gaps for a solo English author",
and greppable CDN URLs in built output. Every one is honest. Together they hand a skeptical
reader a ready-made case.

### Cluster 8 — Legal and identity overhang. Plausibility 3, Impact 4.

Raw: 4, 6, 7, 53, 58.
A trademark-encumbered name, AGPL plus a publicly reserved right to relicense with no stated
contribution terms, and no citable artifact. Two of these (AGPL, HTML-only) are settled and
costed here rather than argued. The name and the contribution terms are not settled and get
strictly more expensive after the first community forms.

### Cluster 9 — Bus factor with scheduled interruptions. Plausibility 5, Impact 5.

Raw: 52, 54, 56, 48, 66.
One author, a student, no succession, no second committer, no funding, no reach-back channel
to users, a security promise of a one-week ack, and a success case that destroys his own
reason for building it. **Highest plausibility × impact in the register**, and the only
cluster whose mitigations are all social rather than technical, which is why it will be the
one skipped.

### Cluster 10 — Supply chain unattended. Plausibility 3, Impact 3.

Raw: 39, 40, 37, 38.
An archived YAML parser at the config core, 343 crates, `cargo audit`/`cargo deny` now manual,
and a documented arbitrary-code-execution-on-open trust model. Not a launch-day killer;
a very good month-six killer.

---

## 5. Mitigations

Mapped to clusters. Ordered by (impact ÷ cost), not by cluster number.

| # | Mitigation | Cluster | Cost | Notes |
|---|---|---|---|---|
| M1 | Ship downloadable binaries for Linux + macOS (x86_64 + aarch64) at the flip; state the platform matrix in README | 1 | S | Removes the single most expensive funnel step. Needs a release process, not CI minutes. |
| M2 | One committed script that runs every gate and **fails on a skipped gate** | 2 | S | The four env gates skip silently today; that is the whole problem. Same script CI would call later if Actions ever returns. |
| M3 | CONTRIBUTING.md stating the corpus-pin contract, the off-limits corpus areas, and the contribution licensing terms | 2, 8 | S | Also the place to state that `corpus/tech-blog` is not a contribution surface. |
| M4 | Decide the name before the flip; clear it in classes 9 and 42 | 8 | S | Strictly cheaper now than after a community forms around the name. |
| M5 | Decide what of `notes/` + `docs/superpowers/` + `.claude/` is published | 7 | S | This is item 25's parked question; the pre-mortem's contribution is that it is launch-blocking, not optional. |
| M6 | Hosted docs + one 60-second recorded demo of the live-edit moat before the flip | 1, 6 | M | `live-edit-hero-demo` is already an unchecked ROADMAP item; the pre-mortem re-prices it from "nice" to "the funnel". |
| M7 | A "coming from Quarto" mapping page generated from the existing vocabulary consts | 4 | S | Not a reversal of the alias deletion; a documentation on-ramp built from data that already generates AGENTS.md. |
| M8 | Measure the scale ceiling with a runtime-generated fixture that is **not** a corpus doc | 5 | M | Respects the standing rule against growing `tarn` and against `corpus/longbook` (whose stated reason is the corpus walker). |
| M9 | Real-device reader verification (iOS Safari, Android Chrome, a phone screen reader, the `--host` QR flow) | 5 | M | Already the standing recommendation; re-priced as launch-blocking because the drawer is now a book's only nav surface. |
| M10 | Grep every built artifact for external URLs, not just `bare` | 7, 10 | S | `corpus.rs:923` checks one surface. The claim is about all of them. |
| M11 | A stated support posture: what response time a user should actually expect, and what happens if the author stops | 9 | S | Cannot fix bus factor 1; can stop it from being discovered as a surprise. |
| M12 | Pin the supply chain: run `cargo audit`/`cargo deny` as part of M2's script, and decide the `serde_yaml` successor | 10 | S | Manual-and-remembered is the same as not-run at month six. |
| M13 | Reframe the launch copy off raw speed and onto state-preserving incremental update + click-to-source | 6 | S | Speed is the claim Quarto 2 has announced it will contest; the block model is the one it cannot copy without the same architecture. |
| M14 | An archived, versioned, citable release (Zenodo DOI) | 8 | S | Cheap; unlocks the one segment that cites tools. |

**Costed, not reversed** (settled rulings): AGPL-3.0-only carries an enterprise-adoption cost
and a contributor-chilling cost that M3 can reduce but not remove. HTML-only carries the
academic terminal-event churn in raw item 20, which nothing in this register mitigates; the
honest move is to state the boundary in the README so the churn happens before adoption
rather than after. The `exec_pool.rs` eviction freeze carries raw item 28's memory risk; M8
measures it without touching it.

---

## 6. Cheap to insure against now, expensive later

Ordered by the ratio of later-cost to now-cost. Every item here is measured in hours today
and in months (or in impossibility) after the flip.

1. **The name.** Hours now: a trademark search in classes 9 and 42. After a community, a
   package name, a URL, a VS Code marketplace listing and inbound links exist, a rename costs
   everything the launch bought. Irreversible in practice.
2. **Contribution licensing terms.** A paragraph now. After the first merged PR from a
   stranger, the author cannot relicense without tracking that person down, and asking for a
   CLA retroactively is the single most trust-destroying move available to an OSS maintainer.
3. **What of the planning corpus is published.** A `git rm` now. After the flip, `notes/` is
   in every clone, every fork and the Wayback Machine; the defect roadmap cannot be unpublished.
4. **The platform statement.** One README line now. Later it is a hundred duplicate Windows
   issues and a reputation for being broken rather than for being Unix-only.
5. **A gate a contributor can run.** One script now. Later it is the maintainer personally
   executing CI for every PR, which is the mechanism by which maintainers quit.
6. **Binary releases.** A release process now, while there is one platform matrix and no
   compatibility history. Later it must also carry back-compat, signing and packagers.
7. **Silently-skipping test gates.** A hard-fail flag now. Later, every external
   "tests pass" report is worthless and the author cannot tell which ones to trust.
8. **The scale ceiling.** A generated fixture and one measurement now. Later it is a
   production incident inside someone's textbook, reported publicly, at the worst moment.
9. **Offline-claim greps over built output.** One test now. Later it is a screenshot on
   social media and a claim the project cannot retract.
10. **A citable archived release.** One Zenodo deposit now. Later, every paper that could
    have cited it did not, and the citation graph does not backfill.
11. **A stated support posture.** A paragraph now. Later it is the gap between what users
    inferred and what the author can deliver, discovered during a security report.

---

## 7. Proposed items

Numbered from 89. Each states its measurement or its reasoning and what would refute it, per
the shared contract. Checked against the "Do not re-add / re-scope" list at the bottom of
`notes/backlog.md`: none of these re-opens the update-nudge (DX16), `check --online`, deck
presenter tools, reader text-size controls, TODO surfacing, the book right-rail TOC, deck PDF
export, CAD-as-code, or the reading-time label. None adds network egress, a new output format,
preview write-back, or a config knob. None touches `exec_pool.rs`. Item 96 is deliberately
shaped to respect the standing ban on growing `corpus/tarn` and on minting `corpus/longbook`.

**89. Close the install cliff before the public flip: shipped binaries + a stated platform
matrix + a fork-continuity artifact.**
*Reasoning:* the evaluation funnel currently has exactly one step and it is
`cargo build --release` over 343 `Cargo.lock` entries including a C regex engine (oniguruma
enters via comrak's syntect default features, per the `Cargo.toml` comment). Six git tags
exist; none is a downloadable artifact. Every verifiable claim the project makes sits behind
that build.
*Measurement:* time a cold `cargo build --release` in a clean container with an empty cargo
registry, and count the non-Rust prerequisites a first-time user must already have.
*Refuted if:* the cold build completes in under ~90 s on commodity hardware with no system C
toolchain required — in which case the cliff is imagined and the funnel problem is purely
documentation.
*Secondary value:* a signed, archived release is also the only artifact from which a fork
could continue the project if the author stops (cluster 9).

**90. Decide and state the platform matrix; treat silent Windows failure as a launch-day
reputational loss.**
*Measurement:* `cargo check --workspace --target x86_64-pc-windows-msvc`. Today the tree has
~20 `#[cfg(unix)]` sites across `warm_pool.rs`, `exec.rs`, `serve/mod.rs`, `runtime_dirs.rs`,
`headless_js.rs`, `includes.rs` and `complete.rs`, and exactly one runtime
`cfg!(target_os = "windows")` branch (`serve/mod.rs:559`) with no `#[cfg(windows)]`
implementation anywhere.
*Reasoning:* the cheap insurance is the honest sentence in the README, not the port. A user
whose build fails tells people the tool is broken; a user who reads "Linux and macOS" tells
people it is not for them, which costs nothing.
*Refuted if:* the workspace already cross-compiles clean for Windows and the kernel/warm-pool
paths have a Windows story — then this is a testing item, not a labelling one.

**91. One committed script that runs every gate and fails loudly on a skipped gate, plus the
CONTRIBUTING that points at it.**
*Reasoning:* `.githooks/pre-push` runs on the author's machine via `core.hooksPath` and gates
only pushes that include `main`. A fork or PR runs nothing. Worse, the four environment gates
(`TALIESIN_REQUIRE_KERNEL`, `_R`, `_NODE`, `_CHROME`) exist precisely because the suites
*skip silently* without them, so an outside "all tests pass" is not evidence. The maintainer
therefore has to personally re-run everything for every PR, which is cluster 2's burnout path.
*Measurement:* run `cargo test --workspace` with no Python, no R, no node and no Chrome
present and record how many of the 1,671 tests actually execute versus skip.
*Refuted if:* a bare `cargo test --workspace` already hard-fails on every missing
interpreter — then the skip risk is imagined and only the CONTRIBUTING half remains.
*Scope note:* this is a script, not a GitHub Actions workflow; the Actions-minutes ruling
stands, and the script is what a workflow would call if that ever changes.

**92. State the contribution licensing terms before the first outside PR.**
*Reasoning:* README currently pairs AGPL-3.0-only with "the author ... reserves the right to
offer Taliesin under other terms, including a proprietary hosted service or a commercial
license." With no stated contribution terms and no CLA, that sentence is either legally
inoperative for contributed code or implies a future CLA demand. Both readings deter the
contributor; only one is true, and the author should pick which.
*Measurement/refutation:* not measurable — this is a reasoning item. It is refuted if the
author's intent is to accept no outside code at all, in which case the correct cheap action
is to say *that* instead, which is equally effective at setting expectations and costs one
line. Either way the expensive path is discovering the answer during a dispute.
*Ruling respected:* the AGPL choice itself is settled and is not questioned here; only the
undeclared contribution terms are.

**93. Clear the name in software classes before the flip.**
*Measurement:* a USPTO + EUIPO live-mark search for TALIESIN in Nice classes 9 and 42. Known
today: TALIESIN reg. 0853377 (filed 1966, Frank Lloyd Wright Foundation) covers old-style
IC 035-042, which reads onto modern class 42 technology services; reg. 4150375 covers class 20
furniture. The Foundation actively licenses and publishes a trademarks page.
*Refuted if:* no live mark covers software or technology services and the Foundation's live
registrations are confined to furniture, architecture and education — in which case this
closes in an afternoon and the item was cheap insurance that paid nothing, which is the
correct outcome for insurance.
*Why now:* a rename after a package name, a marketplace listing and inbound links exist costs
the entire value of the launch.

**94. Make the publish-surface decision for the agent-facing planning corpus a pre-flip
checklist item, not an open question.**
*Reasoning:* tracked today: `notes/` (63 files, `AUDITS.md` 130 KB, `backlog.md` 47 KB, 48
dated findings docs), `docs/superpowers/` (69 files), `.claude/` (7 files), plus CLAUDE.md and
AGENTS.md. `backlog.md` is an honest, dated, curated list of the tool's known defects and open
robustness holes; it also documents its own unreliability. Published, it is simultaneously a
hostile reader's shopping list and the strongest available evidence for the
"agent-workbench, not a product" reading.
*This does not re-scope item 25*, which already parks exactly this question on the flip date;
it re-prices it from "re-ask when a date is set" to "the date cannot be set until this is
answered."
*Measurement:* sample 20 entries at random from `backlog.md` + the findings docs and classify
each as (a) weaponizable against the tool, (b) reads as churn/rework, (c) neutral or positive.
*Refuted if:* fewer than ~3 of 20 land in (a) or (b) — then the tree is safe to publish as-is
and the honesty is a net asset.

**95. Assert the offline guarantee over every built artifact, not one.**
*Measurement:* `crates/core/tests/corpus.rs:923` asserts no `cdn.jsdelivr` / `unpkg.com` in
`bare` output only. Today a live jsdelivr URL is compiled into the binary
(`render/mod.rs:1532` `MERMAID_DEFAULT`), the vendored mermaid loader carries the same URL as
a fallback string that reaches built output, and `docs/guide/using/code.tmd:318` fetches a
texture from unpkg.com in a documented example.
*Reasoning:* the code path is documented as unreachable in-tree, and that is true and
irrelevant — the failure here is a stranger grepping a shipped page and posting a screenshot.
The claim is "no network egress"; the test should cover every artifact the claim covers.
*Refuted if:* a grep for external hosts over every mode's built output (single-file build,
site build, deck build, standalone archive) already returns nothing but the documented example
— then only the example needs a note.
*Scope guard respected:* this removes reachable egress and adds none.

**96. Measure the scale ceiling with a runtime-generated fixture that is not a corpus
document.**
*Reasoning:* `corpus/tarn` is 12 chapters and the standing rule forbids growing it toward 200
pages and forbids minting `corpus/longbook` — the stated reason being that the corpus walker
renders every corpus doc on every `cargo test`. That reason does not apply to a fixture
generated at runtime behind an env gate (or in `tools/live-edit-bench`), which never enters the
walker. Meanwhile two robustness classes are open (AP2-1 deep-`>` stack overflow, AP2-2 comrak
O(n²) bracket hang) and the warm-page eviction path is frozen and documented as not
test-guarded, so nothing in the tree characterises behaviour at book scale.
*Measurement:* generate a 200-chapter book plus one pathological document per known class,
then record wall time, peak RSS, warm-pool occupancy and whether the build completes.
*Refuted if:* build time stays sub-linear in chapter count and peak RSS stays under a stated
bound — then the ceiling is above any plausible real document and the item closes with a
number instead of a fix.
*Constraints respected:* no corpus doc is added, `corpus/tarn` is unchanged, `exec_pool.rs` is
read but not touched.

**97. Generate a "coming from Quarto" mapping page from the vocabulary consts that already
generate AGENTS.md.**
*Reasoning:* the only large population that would adopt is Quarto users, and their migration
path was deliberately closed (aliases deleted, `.qmd` retired). That ruling is not questioned
here — the aliases stay gone. What is missing is the *documentation* on-ramp, and the data for
it already exists: `taliesin vocab` emits every closed-set construct, `check` already names
retired keys with the scope they were retired from (items 53/54), and AGENTS.md is already
generated from the live validator vocabulary so it cannot drift.
*Measurement:* take the front-matter and `_site.yml` key set of a real Quarto project and count
how many keys produce a diagnostic that names the Taliesin spelling versus a bare "unknown
key".
*Refuted if:* that count is already near-total — then the on-ramp exists and only needs to be
findable, which is a link, not a page.
*Scope guard respected:* documentation generated from existing consts; no new knob, no shim, no
alias revival, no egress.

**98. Re-price real-device reader verification as launch-blocking.**
*Reasoning:* this is the backlog's existing standing recommendation and is filed here only to
change its priority class, not to re-find it. The reader surface *is* the value proposition —
"share a link, not a PDF" — and a meaningful share of recipients open that link on iOS Safari,
which the 2026-07-26 round could not model (Chromium emulation does not cover WebKit, momentum
scroll, the dynamic viewport toolbar or safe-area insets). Item 76 removed the book's
right-rail TOC, so the drawer is now a book's **only** navigation surface, and the drawer's
`overflow: hidden` scroll lock on the root was measured only on Chromium — where it is known to
hold *less* completely on iOS Safari.
*Measurement:* the "Not measured" list in `notes/2026-07-26-mobile-audit.md`, plus the `--host`
QR phone-preview flow, which is a first-class phone feature with zero coverage.
*Refuted if:* the drawer lock, drawer navigation and the QR flow all hold on real iOS Safari
and Android Chrome — then the launch-blocking framing is wrong and it reverts to a normal
verification round.

---

## 8. What this round did not cover

- **No dynamic evidence.** Per the task constraints, no `cargo build` or `cargo test` was run,
  so every timing, memory and platform claim above is a hypothesis with a stated measurement,
  not a result. Items 89, 90, 91 and 96 are all currently unmeasured.
- **No competitive verification.** Quarto 2's Rust rewrite and its timeline were supplied as
  premise, not checked. Cluster 6's plausibility depends entirely on that premise holding.
- **No user evidence exists to gather.** Every framing-B claim about churn is inference from
  the tool's structure, because the tool has one user. That is itself the finding in raw
  item 60.
- **The trademark reading is a lay reading** of two Justia records. Item 93's measurement is
  the actual clearance search; nothing here is legal advice.
