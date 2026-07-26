# Mutation re-run, `crates/server` half — partial (`lsp_nav.rs` only)

**Status: stopped at 338 of 444 mutants, deliberately, not finished.** The run was killed when the
session ended rather than left burning four cores overnight. Its output lived in a session scratch
directory under `/tmp`, which does not survive a reboot, so the part worth keeping is written down
here instead.

`cargo-mutants` has no resume, so a continuation re-runs this file from scratch (~3.5 h at these
settings). **The value of this document is that the survivor list below no longer has to be
re-derived to decide whether the file is worth the compute** — it is, and this says why.

## What was run

```sh
# from a `git archive HEAD` snapshot outside the repo, so the working tree stayed free
cargo mutants -f crates/server/src/lsp_nav.rs -j 4 --minimum-test-timeout=120 --output <outside-the-tree>
```

Snapshot commit: `848b85d`. `lsp_nav.rs` has not changed since, so these results still describe the
current file.

**Package scoping is correct here and only here.** `cargo-mutants` defaults to testing the package
containing the mutant; for a *server* mutant that is `taliesin-server`, which builds and runs
`crates/server/tests/*`. The core half's 53%-false-MISSED disaster came from scoping a *core* mutant
to `-p taliesin-core`, which cannot reach the server tests that pin core behaviour. Nothing tests
server code from `taliesin-core`, so no workspace recheck is owed on these.

## Measured

| outcome  | count |
|----------|-------|
| caught   |   282 |
| missed   |    36 |
| timeout  |    16 |
| unviable |     4 |
| **run**  |   338 of 444 |

Throughput was **~2.3 mutants/min** at `-j 4` with the machine otherwise in use, matching the 2.2
estimate. A full `lsp_nav.rs` is ~3.5 h; the remaining eleven server files are ~708 more mutants.

## The 36 survivors are one shape, and it is the shape the backlog predicted

Every one is a **boundary comparison or a cursor arithmetic operator inside a click-to-source
position classifier**. Not one is a business rule. Grouped by function:

- `classify_target` (7): `76:17 < → <=`, `76:13 + → *`, `77:25 && → ||`, `80:21 < → <=`,
  `83:18 > → >=`, `83:30 && → ||`, `83:35 < → <=`, `83:39 && → ||`, `111:22 > → >=`
- `classify_include` (12): `139:13 + → *`, `140:25 && → ||`, `140:33 + → *`, `140:45 && → ||`,
  `142:21 < → <=`, `146:21 < → <=`, `150:38 == → !=`, `152:25 < → <=`, `155:22 > → >=`,
  `157:29 < → <=`, `160:26 > → >=`, `160:39 && → ||`
- `classify_frontmatter_key` (5): `177:26 || → &&`, `182:13 < → <=`, `187:13 < → <=`,
  `191:16 > → >=`, `192:14 < → <=`
- `nested_parent_of` (3): `231:37 - → /`, `232:24 < → <=`, `239:33 && → ||`
- `definition_site` (2): `277:32 > → >=`, `279:25 > → >=`
- `is_anchor_site` (2): `311:14 match guard is_ws(c) → true`, `313:21 > → >=`
- `anchor_occurrences` (1): `336:25 + → *`
- `is_cite_key_char` (2): `56:28 || → &&`, `56:40 || → &&`

**Read that as one finding, not thirty-six.** The classifiers are exercised only from positions
squarely inside the thing being classified, so every *edge* of every span — one character before the
`[@`, the last character of the key, the closing `)` of an include — is unpinned. That is exactly
the gap the backlog names when it says click-to-source has no end-to-end coverage: the feature's
whole job is deciding what the cursor is on, and the tests never put the cursor on a boundary.

**The tractable fix is one table-driven test, not thirty-six tests.** Take a fixture line for each
construct, walk the cursor across every byte of it, and assert the classification at each offset.
That kills most of this list at once and is the kind of test that stays meaningful on its own — which
is the bar this round exists to enforce, as opposed to chasing the number green.

`56:28`/`56:40` in `is_cite_key_char` may be equivalent (the predicate's ranges may not overlap in a
way any input distinguishes); check before writing a test for them.

## The 16 timeouts are detections, not gaps

All sixteen are `+= → *=`, `+= → -=` or `-= → /=` on a **scan cursor**: `classify_target` (81, 94,
109, 120), `classify_include` (143, 148, 153, 158, 170), `classify_frontmatter_key` (183, 188),
`definition_site` (280, 288), `is_anchor_site` (314), `anchor_occurrences` (340, 342). A loop whose
cursor stops advancing spins instead of returning a wrong answer, so the suite hanging **is** the
detection. Same pattern as the core half's 7. Do not write tests for these.

## What is still owed

The other eleven server files, ~708 mutants: `lsp_complete.rs` (294), `complete.rs` (149),
`lsp.rs` (98), then `doctor.rs`, `headless_js.rs`, `interactive.rs`, `lsp_outline.rs`,
`lsp_pos.rs`, `runtime_dirs.rs`, `zip.rs`.

Note that `headless_js.rs` gained two tests on 2026-07-26 (item 55), so a mutation run over it now
measures a different file than the one this scope was drawn against.
