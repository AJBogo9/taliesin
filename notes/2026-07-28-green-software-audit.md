# R13 — green software and carbon intensity

**Date:** 2026-07-28
**Round:** Optional tail / R13 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** What is the measured efficiency story, and is it a differentiator worth stating?

**Instrument.** The Software Carbon Intensity idea from ISO/IEC 21031:2024 — carbon **per functional
unit**, not a total. Two units are meaningful here: **per document build** and **per page view**.
Measured locally with `/usr/bin/time` and file sizes; **no telemetry and no network egress were added**,
which is the invariant that produces the good result in the first place. New items are numbered from
**137**.

**Short round, honest verdict:** two of the three units are genuinely strong, one is bad, and the bad
one is the only actionable finding.

---

## Per document build

| Project | Pages | User CPU | Sys CPU | Elapsed | Peak RSS |
|---|---|---|---|---|---|
| `corpus/tech-blog` | 19 | **0.63 s** | 0.04 s | 0.80 s | 57 MB |
| `rust-lang/book` (real external, R11) | 112 | **0.55 s** | 0.07 s | 0.61 s | 49 MB |

**≈4.9 ms of CPU per page, at 49 MB peak RSS, for a 112-page book.** A full rebuild of a real book
costs about half a CPU-second.

**The warm path is better still and is the one that runs all day.** From R8: a warm edit costs 90 ms
wall clock and re-runs zero cells for a prose change, and the freeze cache removes **80%** of a cold
build (3,981 ms → 789 ms on the same project). The architectural decisions that produce this —
a warm process instead of per-edit startup, block-level diffs, a per-cell content-hash cache — were
made for the author's latency, and the efficiency is a by-product.

**Verdict: strong, and honestly claimable.**

## Per page view

| | `rust-lang/book` build |
|---|---|
| HTML per page | **18.8 KB** |
| shared CSS (`app.css`, hashed, cached) | 230 KB |
| shared JS (`app.js`, hashed, cached) | 93 KB |
| **reachable bytes, whole site** | **692 KB** across 113 pages |

**No CDN, no third-party requests, no tracker, no font fetch, no analytics.** A reader downloads
~19 KB of HTML plus one cached CSS/JS pair, from one origin, and the page works offline. The
`_assets/` consolidation the 2026-07-11 audit produced is doing exactly what it was built for (that
round measured one page going from 355,700 to 16,185 bytes).

**Verdict: strong, and honestly claimable.**

## Per deploy — the bad one

### 137. 85% of the bytes a site build emits can never be served. (MEDIUM)

**Measured** on the 113-page `rust-lang/book` build:

| `_assets/` file | Size | Pages referencing it |
|---|---|---|
| `mermaid.<hash>.js` | **3,572,004 B** | **0** |
| `jslibs.<hash>.js` | **487,117 B** | **0** |
| `katex.<hash>.css` | 369,346 B | 1 |
| `app.<hash>.css` | 229,778 B | 113 |
| `app.<hash>.js` | 92,924 B | 113 |

```
_assets total:      4,751,169 B
never referenced:   4,059,121 B   (85%)
actually reachable:   692,048 B
```

**A deploy ships 6.9× more bytes than it can serve.** The book contains no mermaid diagrams and no
`{js}` cells, and both bundles ship anyway.

**Scope, stated precisely so it is not overclaimed.** This is a **deploy and storage** cost, not a
transfer cost: a reader never downloads an unreferenced file, so the *per page view* figure above is
unaffected and remains strong. What it costs is upload bandwidth on every deploy, storage at the host,
and CI/deploy time — which is exactly the axis the SCI specification exists to make visible.

**This corroborates a finding the concurrent `critique-pass-2026-07-27` branch reached independently**
("every site build ships ~4.43 MB of assets no emitted page references"). Two different sessions,
two different projects, the same defect. **It is one item, not two** — whichever branch merges first
owns it, and this round contributes the exact percentage and the per-file breakdown.

**Fix.** The content-gating that already decides whether to *inline* mermaid on a single-doc build
(R6/Wave 1 established `mermaid_url_for()` gates by content) is not applied to the `_assets/` copy
step. Emit a bundle only when some page in the project references it.

**Refuted if** any page in a mermaid-free, `{js}`-free project references either bundle (measured:
zero of 113).

---

## Verdict: is this a differentiator worth claiming?

**Partly, and the honest framing is better than the strong one.**

- **Do claim:** offline by construction, zero third-party requests, ~19 KB of HTML per page, one
  cached CSS/JS pair, a full 112-page book rebuilt for half a CPU-second, and a warm edit loop that
  re-runs nothing it does not have to. Every one of those is measured above or in R8.
- **Do not claim** a carbon number. A real SCI figure needs an energy model, a regional carbon
  intensity and a hardware baseline, none of which were measured here — these are CPU-seconds and
  bytes, which are *inputs* to SCI, not SCI.
- **Do not claim it as a headline.** R10 measured that the demand-backed positioning is
  reproducibility and the single editing surface. Efficiency is a supporting sentence in a README,
  not a pitch, and claiming it loudly while shipping 85% dead bytes would be the kind of overclaim
  R9's ACR guidance warns against.

**Fix item 137 first, then the efficiency story is clean enough to state.**

---

## Not measured

- **No energy measurement.** No RAPL, no wall-power meter, no carbon-intensity data. CPU-seconds and
  bytes are proxies and are labelled as such throughout.
- **No comparison against another tool.** Deliberately: R10's rule is that a competitor's number is a
  hypothesis, not a finding, and no controlled benchmark was run.
- **The preview path's steady-state cost** (a server idling for hours with a warm kernel) was not
  measured, and AUDITS.md already records multi-hour drift as an open gap.
- **The `_freeze/` directory's disk cost** was noted in R8 (1.17 MB for `corpus/tech-blog`, largest
  page 452 KB) but not tracked over time.

## Round bookkeeping

This round wrote only this file. Item 137 follows R10's 135-136. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**The slate is now complete except R12** (real-device mobile, Android), which needs the author's phone
and cannot be run from a session.
