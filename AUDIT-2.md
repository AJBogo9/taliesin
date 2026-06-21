# qmd-fast — round-2 adversarial audit (2026-06-21)

A second pass after the round-1 backlog (P0–P3) was fixed. Method: **empirical** (a
battery of ~80 hostile/extreme documents actually run through `build`/`preview`, with
panic/segfault/hang/timing capture and output inspection) plus three parallel
adversarial code-reads of the render, execution/server, and site/client layers. None
of these overlap the round-1 `AUDIT-BACKLOG.md` items.

**Status legend:** `CONFIRMED` = reproduced live in this session; `REPORTED` =
high-confidence static finding (tied to specific code), not yet run.

---

## RESOLVED (2026-06-21)

All findings below were fixed and verified, then a 6-agent verification+completeness
workflow re-attacked the patched code and surfaced 16 follow-ups (0 high) — those were
triaged and the real ones fixed in the same batch. 202 tests pass, clippy + fmt clean.

- **#1 output hang:** per-cell output caps (512 KB stream / 4096 items) + interrupt the
  kernel on cap. A 5 MB `print` went **70 s → 1.7 s**. Truncated output is marked and
  kept out of the `_freeze` cache (no stale-truncated replay).
- **#2 in-place build data loss:** `build` refuses when the output dir == source dir;
  `copy_resources` also skips self-copies. Verified: asset preserved.
- **#3 Quarto config wipe:** the whole `website:` block is now read field-by-field
  (lenient), so a bare-string nav item — or any one wrong-typed field — no longer drops
  title/url/favicon/footer. Verified.
- **#4 mount search 404:** mounts now route `search.json` + `feed.xml`. Verified 200.
- **#5 deep-nesting abort:** render runs on a 256 MB-stack worker (falls back to inline
  if the spawn fails under `ulimit -v`). 3000/10000 blockquotes + 5000-deep lists build.
- **#6 duplicate labels:** warn + first-wins (`@x` now agrees with the anchor), in-doc
  and cross-page (deduped). The analogous **duplicate `.bib` key** now warns too.
- **#7 front-matter terminator:** FALSE POSITIVE (code uses `trim_end`, i.e. column-0;
  verified an indented `...` does not truncate). Not changed.
- **#8 category slug collision / #9 listing id-target / #10 comma in category:** fixed +
  verified (merged archives; `tag_end`-based leading-tag match; filter reads badges).
- **#11 shell-reply desync:** the execute_reply drain is now parent-matched.

### Known residuals (deliberately not fully closed; low risk for this tool's scope)
- **Single >~tens-of-MB cell output** still blocks during the ZMQ receive (one giant
  message blocks an uninterruptible read before the cap can fire). Realistic streamed
  output is fixed; a single 50 MB `print` is bounded only by completion / the cell
  timeout. Fully fixing needs frame-level capping in the jupyter/zmq layer.
- **Stack overflow** at absurd nesting (hundreds of thousands deep) still aborts: the
  big stack raises the ceiling ~32×, but a hard depth cap (or iterative emit) was not
  added — comrak overflows during parse, before a post-parse cap could run, and a
  source-level depth heuristic risks false-positives on legitimate docs.
- **Duplicate cross-ref labels** still emit two identical HTML `id=` attributes (the
  number/anchor *disagreement* and the missing warning are fixed; the invalid-but-now-
  consistent duplicate id is not deduped).
- **Mounted sub-sites** don't route category-archive pages or embedded decks, and a
  mount miss serves a bare asset 404 rather than the styled 404 (the shipping mounts are
  books, which have neither).
- **Conflicting shortcode names** across two active extensions silently last-wins
  (author-controlled; rare).

---

## HIGH

### 1. A large cell output hangs the build/preview (super-linear in output size) — CONFIRMED
A `{python}` cell that prints a moderately large string wedges the renderer:
`print('x'*1MB)` → 1.6 s, `print('x'*5MB)` → **>60 s (timed out)**, 20 MB → timed out.
That is catastrophically super-linear (likely O(n²) copying: the output string is
cloned into the output block, the `_freeze` cache, and `state.ran`, plus HTML-escaped
and diffed). There is no cap on accumulated cell output (`kernel.rs` `execute` →
`render_outputs` → `exec.rs`). This is the **most likely real-world failure for a power
user**: `print(big_df)`, a verbose training log, or a large array silently hangs the
preview. → cap per-cell output bytes (truncate with a marker) and avoid the repeated
full-size clones. (`crates/server/src/kernel.rs` execute/render_outputs, `exec.rs`.)

### 2. Building with the output dir == source dir destroys source assets (data loss) — CONFIRMED
`output: "."` in `_quarto.yml` (or `build <dir> --out <same dir>`) makes `mirror_assets`
`fs::copy(p, dest)` every asset onto itself; `fs::copy` opens the dest `O_TRUNC` first,
so each file is truncated to 0 bytes. Reproduced: `logo.png` 34 → **0 bytes**. The
single-doc path has a `same_file` guard; the site path does not. → error out when
`out.canonicalize() == root.canonicalize()`, and skip self-copies in `mirror_assets`.
(`crates/server/src/main.rs` `build_site`/`mirror_assets`.)

### 3. One bare-string nav item silently wipes the entire Quarto `website:` config — CONFIRMED
A Quarto-shaped `_quarto.yml` with `navbar: left: [Home, …]` (bare strings — extremely
common in real Quarto configs) fails to deserialize `NavItem` (it wants a map), and
because the shim deserializes the whole `website:` block atomically and falls back to
`Website::default()` on any error, the site loses its **title, url, description,
favicon, footer, and navbar**. Reproduced: title absent from output. The native config
path handles bare strings; only the Quarto-compat path is fragile. → deserialize
nav/footer leniently (per-item, like the native `nav_item`). (`crates/core/src/site/config/quarto.rs` `section`/`Website`.)

### 4. Cmd-K search is dead inside any mounted sub-site (`mounts:`) — CONFIRMED
A mounted book page emits `QMD_SEARCH_URL="search.json"`, which resolves to
`/<mount>/search.json` → **404** (only the parent's `/search.json` route exists).
Reproduced on the shipping `site/` (mounts `/docs/guide` + `/docs/internals`): the
mounted page returns 200 but `GET /docs/guide/search.json` is 404. Search silently
returns nothing on every mounted-book page. → add a per-mount search route (and feed),
or point `QMD_SEARCH_URL` at the parent. (`crates/server/src/serve_site.rs` mount
handler + `site/mod.rs` chrome.)

---

## HIGH-ish (crash)

### 5. Deeply nested blockquotes/lists abort the process via stack overflow — CONFIRMED
A document with ~3000 nested `>` blockquotes (or ~3000-deep nested list) overflows the
stack and the process aborts (`fatal runtime error: stack overflow`, exit 134 / core
dump). Depth 1500 is fine, 3000 is not; nested `:::` divs do **not** overflow (they're
preprocessed iteratively). The recursion is in the parse/emit of the deep AST. A single
pathological (or pasted) document crashes `build` outright, or crashes the **preview
server** mid-session. → cap nesting depth (reject/flatten beyond N) or render
iteratively; at minimum catch and report instead of aborting. (`crates/core/src/render/emit.rs` recursive emit; comrak parse.)

---

## MEDIUM

### 6. Duplicate cross-reference labels resolve silently to the wrong target — CONFIRMED (single-doc) / REPORTED (cross-page)
Two figures both labelled `{#fig-dup}` produce **two elements with `id="fig-dup"`**
(invalid HTML) and `@fig-dup` renders "Figure 2" while the `#fig-dup` anchor points at
Figure 1 — the number and the link destination disagree, with no warning. Cross-page,
`scan_xref_targets` uses `entry().or_insert()`, so the first page defining an anchor
wins project-wide and later redefinitions are silently ignored. → warn on duplicate
labels; dedupe ids. (`crates/core/src/render/mod.rs` xref registry, `site/xref.rs:43`.)

### 7. `front_matter_block` ends early on a `---`/`...` line inside a multi-line value — REPORTED
The terminator scan stops at the first line trimming to `---`/`...`, so a YAML block
scalar (e.g. a `description: |` whose body contains a `...` line) truncates the front
matter for the linter, the site parser, and `parse_extensions` (which disagree with
comrak's own parse) → later keys (`theme:`, `categories:`, …) are silently lost.
(`crates/core/src/frontmatter.rs:154`.)

### 8. Colliding category slugs overwrite each other's archive pages — REPORTED
Archive URLs key on `slugify(name)` but the index keys on the raw name, so two names
that slugify identically (`"Machine Learning"` vs `"machine-learning"`) write the same
`categories/<slug>/index.html` (second overwrites first) and posts under the losing
name link to the wrong archive. No warning. (`crates/core/src/site/mod.rs`
category_pages/render_category_page.)

### 9. Listing `id:` target matches the first substring occurrence of `id="x"` — REPORTED
`listing: { id: x }` finds its placeholder with `b.html.contains("id=\"x\"")`, so a
code sample or any block that merely *contains that text* (the docs books show HTML)
captures the cards instead of the intended `::: {#x}`. (`crates/core/src/site/mod.rs` `expand_page`.)

### 10. A category name containing a comma breaks the client-side filter — REPORTED
Card categories are serialized comma-joined into `data-categories` and the filter splits
on `,`, so a category literally named `"A, B"` becomes two tokens and its own chip never
matches → the card is hidden whenever a filter is active. (`site/mod.rs` +
`assets/js/code-enhance.js`.)

### 11. Cell output / stale-reply robustness — REPORTED
A cell that ignores SIGINT past the timeout can leave the shell channel one
`execute_reply` behind (the shell drain isn't parent-matched), desyncing later cells
until a manual Restart. (`crates/server/src/kernel.rs:493/552`.)

---

## LOW (selected; full list in the agent transcripts)

- **Citations rewritten inside rendered math:** the cite/xref rewriter's skip-list
  omits KaTeX spans, so `$\text{@fig-x}$` becomes a link inside the equation. (`cite.rs` SKIP.)
- **Quadratic citation scan** on a run of unmatched `[` (e.g. ASCII art): O(n²) per
  render, i.e. per keystroke in preview. (`cite.rs` rewrite_text.)
- **`execute: {echo: false}` (YAML flow form) ignored** — only the block form is parsed,
  so cells echo despite the front matter. (`render/mod.rs` detect_execute_defaults.)
- **First-visit TOCTOU**: the HTTP GET and the websocket open both queue a build for a
  fresh page → duplicate first-paint builds. (`serve_site.rs` ensure_and_render_page.)
- **RSS `<link>`/`<guid>` use `…/index.html`** while canonical/`og:url` use the clean
  directory URL → reader/analytics mismatch. (`feed.rs` vs `meta.rs`.)
- **XML escaping omits C0 control chars** → a title with `\x0c` yields an invalid feed. (`feed.rs` xml.)
- **`window.QMD_PAGE_URL` interpolates `page.url` unescaped** into a JS string (other
  script values go through `json_str`). (`site/mod.rs`.)
- **No mount validation:** an empty `at:` shadows the parent; `path:` can point outside
  the root (only `is_dir` checked). (`config/mod.rs` mounts_from.)
- **Single-doc watcher** misses an out-of-tree `{{< include >}}` added after startup.
  (`serve.rs` watch_dirs.)
- **`force_next` leaks** when a Restart happens on a doc with no code cells. (`exec.rs`.)
- **Blocking `<prog> --version`** runs on a tokio worker (no timeout). (`exec.rs` interp_id.)
- **`code-line-numbers=` substring-matched** anywhere in a fence info string. (`emit.rs`.)
- **Shortcode template `{{n}}` substitution** is order-dependent / can double-expand. (`extension/mod.rs`.)
- **`insert` op with an unresolved `after_id`** prepends the block to the top. (`web-client/client.js`.)

---

## What held up under attack (the negative space)
Attribute/`<html lang>`/title/`js_str` escaping all held against injection payloads
(`"`, `>`, `</script>`, `</title>` were correctly escaped — the round-1 fixes work).
Kernel **crash** handling is solid: `os._exit`, a segfault (`ctypes.string_at(0)`), and a
raised exception all recover gracefully (the kernel respawns, downstream cells re-run);
an infinite loop is SIGINT'd at the timeout and the doc continues. Unicode headings
(emoji / RTL / combining / zero-width / CJK) slug and dedupe correctly with no
non-char-boundary panic. Malformed math degrades to error blocks. Huge flat documents
(8000 paragraphs, 3000-row table) render in <0.2 s — no quadratic there. The diff and
the cumulative-hash execution plan are sound under reorder/insert/delete/`cache:false`.
