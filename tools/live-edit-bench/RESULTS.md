# live-edit benchmark results (indicative)

> Numbers from the author's machine (16 threads), measured **2026-09-24**, release build,
> with no other build or test running (1-minute load average 1.15 before the run, from the
> desktop session; it is recorded in `RESULTS.json`). Absolute times vary by machine and
> load. Regenerate with `cargo build --release -p taliesin-server` and then `cargo run
> --release -p live-edit-bench`, which rewrites `RESULTS.json`; without the release binary the
> end-to-end table is skipped. The structural rows (op counts, payload bytes, payload
> ratio, DOM preservation) are deterministic and gated
> (`tools/live-edit-bench/tests/regression.rs`); the timing rows are not, because a wall
> clock measures the machine.
>
> **Three tables, three questions.** The first is one document's edit through the
> render+diff seam. The second is the whole-project passes a save in a site preview runs
> around that edit. The third is what the author waits for: the save to the first message
> a real `taliesin preview` sends, and to the `publishDiagnostics` a real `taliesin lsp`
> sends. Before 2026-09-24 the bench stopped at the first two and at 17 pages, so a
> published per-save figure had no instrument for the watcher's debounce, rediscovery, the
> edited page's own build, the search index a moved anchor rebuilds, the language server,
> or a project large enough to show the per-page slope (audit 2026-09-24, F5). The
> synthetic books (`write_synthetic_book`, 100 and 500 pages) give it that size.
>
> **Best-of-twelve is the binary's job, not the reader's**, for the first table: `BEST_OF`
> is a constant in `main.rs`. The cold-render row is deliberately exempt: only the first
> render in a process is cold (the syntax set and the other lazy statics are built on first
> use), so best-of-twelve there would publish a warm render as a cold one. The other two
> tables publish the median of ten (`RUNS`).

What this shows, for one keystroke-sized edit to a paragraph above the cells in a real
post: the warm server re-renders and diffs in a fraction of the cold-start time (lazy
syntax-highlight and math init are amortized), it sends a payload roughly 89x smaller
than the full page a reload would re-fetch, and 54 of the 55 emitted ops are `SetMeta`,
which patches a block's `data-sourcepos` in place without touching its DOM node, so the
live state of every one of those blocks survives the edit. None of these are things a
batch compiler's cold-pass-plus-full-reload model (Jupyter/nbconvert, R Markdown/knitr,
Quarto, MyST) can match.

## live-edit benchmark: `corpus/tech-blog/posts/em-algorithm/index.tmd`

| metric | value |
|---|---|
| cold full render | 107352.2 us |
| warm edit (render + diff) | 2855.4 us |
| diff only | 448.6 us |
| ops emitted | 55 (insert 1, set_meta 54, update 0, remove 0) |
| full page HTML | 287683 bytes |
| warm-edit payload | 3241 bytes |
| payload shrink vs full reload | 89x smaller |
| open `<details>` survives as same DOM node | yes |

## project-scale save: the whole-project passes

Median of the runs, in-process, release build. A save of a page runs
`refresh_xrefs`; one that moves an anchor also rebuilds the search index; one that
changes the page set or what discovery reads of a page (its front matter and leading
`# H1`) runs `discover` instead. The language server runs `discover_registry` on every
save.

| project | pages | save: refresh_xrefs | anchor moved: + search index | front matter: discover | language server: registry |
|---|---|---|---|---|---|
| `docs/guide` | 16 | 3.0 ms | 3.4 ms | 8.0 ms | 2.4 ms |
| `docs/internals` | 6 | 1.6 ms | 2.6 ms | 4.5 ms | 1.3 ms |
| `corpus/tech-blog` | 17 | 2.2 ms | 5.3 ms | 8.4 ms | 2.0 ms |
| `synthetic book` | 100 | 4.6 ms | 10.4 ms | 17.4 ms | 3.2 ms |
| `synthetic book` | 500 | 23.5 ms | 54.9 ms | 87.0 ms | 14.7 ms |

What the second table shows. Every save of a page runs `refresh_xrefs`, whose harvest renders
every page for its cross-page numbers and heading titles. Since 2026-09-24 that render
typesets no math and highlights no code (`render_numbers_scoped_with_site`), runs on
long-lived render threads rather than one spawned per page, and follows a source scan that
runs across cores; before, past the math memo's capacity every save re-typeset the whole
project's math on the one KaTeX thread. A save that moves an anchor also rebuilds the search
index, which needs the served text, so that pass still renders every page in full. Both are
still O(pages): content-hashing each page's harvest, which would make the save flat, stays
cut (`notes/DO-NOT-REBUILD.md`, FA23).

## end to end: save to the first message

Median of the rounds, against the release binary. From the file write to the first
websocket message the preview sends, so the watcher's debounce, rediscovery and the
edited page's own build are all inside; the language server column is the save to
`publishDiagnostics`, its 120 ms coalescing window inside.

| project | pages | body | heading (moves anchors) | title | atomic save | preview RSS after | language server |
|---|---|---|---|---|---|---|---|
| `docs/guide` | 16 | 26 ms | 30 ms | 31 ms | 27 ms | 134 MB | 129 ms |
| `corpus/tech-blog` | 17 | 35 ms | 47 ms | 48 ms | 35 ms | 146 MB | 135 ms |
| `synthetic book` | 100 | 25 ms | 36 ms | 41 ms | 24 ms | 88 MB | 127 ms |
| `synthetic book` | 500 | 45 ms | 105 ms | 114 ms | 45 ms | 132 MB | 138 ms |

What the third table shows. Every preview column includes the watcher's wait for a save's
events to stop (15 ms of quiet); compare a row with the second table to see what the project
adds. The body and atomic rows are the same prose edit written in place and renamed over the
file, and a save is judged by what it changed, not how it was written, so neither
rediscovers. The heading row moves anchors, so it rebuilds the search index on top of
`refresh_xrefs`. The title row rediscovers the project; where the title shows in the page's
chrome (a book's drawer and pager), the first message is the `reload` that tab is sent, and
the bench reconnects as the browser would. The language server column includes its 120 ms
coalescing window. The RSS column is the preview's resident memory after all forty saves:
until 2026-09-24 every search index rebuild kept its per-page fragments alive, scattered through
the allocator arenas of the threads that built them, and a preview's memory grew with each
save that moved an anchor.

The per-project rows are **not gated**: they are wall clocks, and wall clocks measure the
machine, so by this project's own rule they carry a date and get re-measured before a
release rather than pinned by a test that fails on a slower laptop.

## Where the payload goes

All 55 ops together weigh 3,241 bytes: 54 `SetMeta` patches and the one `Insert` for the
newly typed paragraph. The document's `::: {.callout-note collapse="true"}` fenced div, the
only block whose html carries more than one `data-sourcepos`, is one of the 54: its patch
carries the new position of every inner block as well, and the client writes them onto the
div's own descendants in order, so Ctrl-click inside it stays exact and an opened callout
stays open.

From 2026-06-30 (`6cdbc218`) until 2026-09-24 that div took a full `Update` instead,
because `SetMeta` could then patch only the outer element and would have left the inner
positions stale. That one op was 90% of a 32 KB payload, this file published 9x, and the
re-render closed an opened callout (and re-mounted any `{js}` cell inside a fenced div) on
every edit above it. The 83x published before 2026-06-30 had today's shape but patched the
outer position only. `regression.rs` pins the op shape exactly, so the next change to this
contract fails a test instead of quietly invalidating a published number.
