# Deep multi-agent audit, 2026-09-02

> **STATUS, same day: all 23 majors are fixed on `audit-fixes-2026-09-02`.** Every one was
> reproduced before being fixed, every fix carries a test written first, and every test was
> mutation-checked (fix reverted alone, named test fails, fix restored). `./tools/gates.sh`
> reports `PASSED — every gate ran and passed (13 gates)`, the same verdict line as the
> pre-audit baseline. **Nothing below was refuted**, but four descriptions were wrong in
> detail and are corrected in place: findings 17, 20, 21 and 23. The 143 unvetted leads in
> the appendix are untouched and still unvetted.

A whole-repo audit run as a 3-round agent workflow: **37 lens-scoped finders**, each finding
faced by **3 adversarial verifiers** (technical evidence, fit with recorded decisions, real-world
impact), 2-of-3 uphold required to confirm. 291 agents completed, ~16M subagent tokens.

**Baseline first:** `./tools/gates.sh` ran green immediately before the audit, its own verdict
line reading `PASSED — every gate ran and passed (13 gates)`. Every finding below is therefore
something the current regression net does not catch.

**Result: 23 confirmed major findings, 0 refuted, 0 critical.** A further 143 minor/advice
findings were collected but deliberately left **unvetted** (see "What this audit did not do").

## Read this first: three systemic clusters

Ten of the 23 are not ten independent bugs. They are three root causes, and fixing each root
fixes its whole cluster.

### Cluster A: post-fold passes are blind to folded content (4 findings)

Every pass that runs **after** `group_divs` and reads `b.cell` or iterates only top-level blocks
cannot see anything the author put inside a `:::` container. Two of the four make
`build --check-only` **fail on a working document**, which is the worst kind: the tool calls a
correct page broken.

**Correction: `Block::cells()` is the fix for only two of the four.** `divs.rs` records a folded
cell on `Block::nested` only when the KERNEL runs it, because that is the class of cell needing an
output slot spliced back inside the container. A folded `{js}` cell therefore has no `Block` and
no `Cell` anywhere in the model, so `cells()` is just as blind to it as `b.cell` is (findings 8
and 10 both turn on this). Its wiring survives only in the emitted
`<script data-name/data-viewof/data-inputs>`, which is what the browser's own `buildGraph` reads,
so that is where the validator now reads it too.

### Cluster B: the include source map is bypassed at three seams (3 findings)

FA15 introduced `BufLine` so a post-include buffer line could not be formatted into a diagnostic.
Three seams still ship raw buffer lines: the shortcode pass (twice) and `lsp_project::walk`. The
result is the exact hazard the newtype was built to prevent, a diagnostic N lines off in a real
openable file, and in the partial-authored case the **wrong file entirely**.

### Cluster C: `_site.yml` shapes that silently drop a book part (2 findings)

Two YAML slips (a scalar where a list belongs, a missing `-` merging two entries) each drop a
whole part from a published book while `build --check-only --strict` exits **0**. The validator
already emits located "DROPPED" diagnostics for neighbouring shapes; these two are gaps in it.

---

## P0: act on these first

### 1. `build` overwrites a source `.tmd` with rendered HTML (data loss)

`crates/server/src/build.rs:95`

`taliesin build *.tmd` in a directory holding `about.tmd` and `index.tmd` expands to
`build about.tmd index.tmd`. The second positional is taken as the **output path**, so
`index.tmd`'s source is replaced by rendered HTML, exit 0, log line `built index.tmd`.
Unrecoverable without git.

The guard that should stop this, `non_html_output_error`, is a **denylist**
(`NON_HTML_OUTPUT_EXTS` = pdf, docx, md, ...) and `tmd` is not in it. I verified this directly:
`ACCEPTED_SOURCE_EXTS = ["tmd"]` (`crates/core/src/ext.rs:11`), and the denylist's own comment
says an unlisted target is "the author's deliberate choice", which is exactly wrong for a source
file. This is a sibling of the known Batch-1 data-loss item in the 2026-08-13 queue.

**Fix:** reject a second positional whose extension is in `ACCEPTED_SOURCE_EXTS`, in the same
message shape DX11 already prints. One condition.

### 2. Fence language token reaches a class attribute unescaped (injection)

`crates/core/src/render/emit.rs:60`

`format!(" class=\"language-{l}\"")` interpolates the fence token with no escaping. I verified
`code_lang` (`crates/core/src/render/cell_extract.rs:206`) applies **no character filter**: it
splits on comma/space/tab and returns the raw token. So a fence opening
```` ```x"onclick=alert(1)// ```` emits `<code class="language-x"onclick=alert(1)//">`, and the
browser parses a real `onclick` attribute. The `safe_url` guard on the same emit path exists to
stop this input class one attribute over.

Realistic vector is an included third-party markdown file rather than the author's own typing,
but the cost of the fix is one call.

**Fix:** wrap the token in the existing `escape_attr` at both sites (here and
`cell_numbered.rs:148`).

**Correction to the suggested alternative.** "Delete the class, nothing consumes `language-*`" is
right about this tree's own CSS and JS but wrong as a conclusion: `highlight_langs.rs`,
`tech_blog.rs` and `render/tests.rs` all pin the attribute, and `<pre><code class="language-x">`
is the form every external consumer of the published HTML expects. Cutting it is a separate call
about shipped output, not a cheaper way to close an injection.

---

## Wrong published output

### 3. `group_divs` silently drops a sibling div's wrapper

`crates/core/src/render/divs.rs:383`

```
::: {.outer}
::: {.foo}
:::
::: {.callout-note}
Inside.
:::
:::
```
publishes `<div class="outer">…<p>Inside.</p></div>`. The callout wrapper, its title bar and its
class are **gone**, and no diagnostic fires (the callout has content so the empty-div check skips
it; `.foo` is a custom class so its own check is silent). The same shape breaks a `layout-ncol`
grid whose first column is an empty placeholder.

**Fix:** interleave the two loops so a stale empty span cannot halt the open pass. No new state.

### 4. `{js}` cell source is published into the Cmd-K search index

`crates/core/src/render/mod.rs:3481`

`strip_tags_inner` skips `<math>` subtrees but pushes `<script>` bodies as text, so a client
cell's JavaScript source becomes indexable section text. **This is live on
gallery.taliesin.sh right now**: `descent.html`'s section body is dominated by its
gradient-descent cell's source, so ordinary queries ("const", "width", a variable name) return
snippets of code that appears nowhere on the page.

This is the standing "any new generated block owes the search-index sweep" convention being
violated by an old block.

**Fix:** make `strip_tags_inner` skip `RAW_TEXT_ELEMENTS` bodies the way it already skips
`<math>`. Every consumer inherits the fix.

### 5. `abs_page_url` truncates any page whose stem ends in "index"

`crates/core/src/site/feed.rs:74`

`search-index.tmd` builds to `search-index.html` but publishes `<loc>https://site/search-</loc>`
into the sitemap, and the same truncated URL into `og:url` and the Atom entry `<id>` and
`<link>`. Every machine consumer follows a 404 while `--check-only` stays green.

**Fix:** strip `index.html` only when the URL is exactly `index.html` or ends `/index.html`.
One line in the one shared helper.

### 6. Corporate author containing " and " is split into two mangled names

`crates/core/src/cite/author.rs:13`

`author = {{Food and Drug Administration}}` publishes as "Food and D. Administration". Same for
NIST, CDC, any brace-protected corporate author. Silent: the key resolves, no diagnostic.

**Fix:** split on " and " only at brace depth 0, the idiom `parse.rs` already uses.

---

## Cluster A: post-fold blindness

### 7. `apply_table_captions` misses cells folded into containers

`crates/core/src/render/mod.rs:2692`

A `{python}` cell with `#| label: tbl-x` inside `::: {.callout-note}` publishes a caption reading
**"Table : Latencies"** (empty number). Any `@tbl-x` renders unnumbered and draws an
error-severity broken-cross-reference, so **`build --strict` fails on a valid document**.

**Fix:** iterate `blocks[i].nested[j].cell.table` in the first arm. The executor already reads
the nested copy, so setting the number there is the whole fix.

### 8. Reactive validator cannot see a `{js}` cell inside a container

`crates/core/src/diagnostics/reactive.rs:30`

A viewof control in `::: {layout-ncol=2}` consumed by a top-level cell runs perfectly in preview
and build, but the validator reports error-severity "unknown reactive input `n`" and
**`build --check-only` exits 1 on a working page**. The inverse also holds: a genuinely dangling
input among folded cells goes unreported.

**Fix:** collect the wiring from the emitted HTML's `data-name`/`data-viewof`/`data-inputs`.
Per SCRIPT TAG, through `render::tags`/`attrs` rather than `attr_values`: pooling a container's
names into one node would invent cycles between cells that are merely neighbours. Verified both
directions, and that a folded dangling input is now reported at the cell's own line.

### 9. `runtime_defines` misses a kernel `define()` cell inside a container

`crates/core/src/diagnostics/reactive.rs:89`

Same family: a bridge cell in a callout executes and publishes the name at runtime, but the
dangling-input check stays armed and the page draws a false error.

**Fix:** `b.cells().any(|c| client_lang(&c.lang).is_none() && c.code.contains("define("))`.
One line.

### 10. `number_chapter_headings` skips folded headings, desyncing section numbers

`crates/core/src/site/chapter.rs:25`

A `## Beta {#sec-beta}` inside `::: {.column-page}` renders with **no visible number**, while the
next heading visibly takes the number Beta was assigned. Every `@sec-beta` then says
"Section 2.2" and lands on an unnumbered heading, while a different heading on the page displays
"2.2".

**Correction: the subtractive fix is not available.** Making the render walk skip headings inside
`:::` spans would withdraw a shape the tree already supports and pins, `@sec-note` pointing at a
`## Important {#sec-note}` inside a callout (`render/tests.rs`), and `.column-page` exists
precisely so a section can be wide. **Fix taken:** `number_chapter_headings` finds headings
through `render::tags` instead of testing the block's ROOT element, so it sees every heading the
page shows, in document order. The walker knows tag from text and skips `<script>` bodies, so a
heading spelled in a code sample or a `{js}` cell is still not one.

---

## Cluster B: include source map bypassed

### 11 and 12. Shortcode diagnostics ship raw buffer lines

`crates/core/src/render/mod.rs:265` and `crates/core/src/render/extension/mod.rs:120`

With a 40-line partial included at the top, a shortcode typo at source line 50 reports **line
89**: the LSP squiggle, the preview panel and `--check-only` all land 39 lines off, possibly past
EOF. A shortcode error written **inside the partial** is attributed to the parent page at a
buffer offset, so wrong file and wrong line.

**Fix:** in `render_doc_with_includes_impl`, map each shortcode warning through the `origins` vec
already in scope before extending `doc.warnings`. The include-warning block directly above shows
the pattern.

### 13. Cross-page go-to-definition lands on expanded buffer lines

`crates/server/src/lsp_project.rs:211`

`walk` discards what `resolve` returns, so F12 on `@sec-analysis` opens the target chapter ~29
lines below the heading, or clamped at EOF. The corpus's own shapes hit this
(single-page-report has 7 includes).

**Fix:** keep the `origins` vec, map each `ScannedAnchor` back to its origin file and line, and
point `ProjectAnchor.path` at the partial when the anchor lives there.

---

## Cluster C: silent book drops

### 14. A non-sequence `chapters:` value drops a part, exit 0

`crates/core/src/site/config/mod.rs:265`

`chapters: deep.tmd` (scalar instead of a one-item list) means `push_group` never recurses:
`deep.tmd` is never built, gets no nav entry, no number, and `--check-only --strict` exits **0**.
At top level the same slip silently turns a book into a plain website.

### 15. An entry carrying both `file:` and `part:` drops the whole part

`crates/core/src/site/config/mod.rs:252`

A missing `-` merges two entries into one valid-YAML mapping with only recognized keys. The built
book contains `index.tmd` but no "Basics" part and no `a.tmd` at all. Zero diagnostics.

**Fix for both:** extend `validate_chapters` to push the same located "DROPPED from the book"
diagnostic the neighbouring shapes already get.

---

## Preview shows something the source does not say

### 16. Editing the shared `.bib` never rebuilds anything

`crates/server/src/serve_site/mod.rs:1771`

With `bibliography: refs.bib` in `_site.yml`, fixing a wrong year and saving fires the event but
matches no rebuild condition: not `_site.yml` by name, not structural, in no page's dep set. The
tab keeps the stale citation, and **even a full browser reload serves the stale cached
PageState**.

**Fix:** insert the project's resolved shared paths (`site.bibliography`, already held under the
same lock) into `deps` in `rebuild_project`'s filter.

### 17. Renaming a `.tmd` page is never classified structural

`crates/server/src/serve_site/mod.rs:1567`

A same-filesystem `mv a.tmd b.tmd` (shell, file manager, or the companion's own rename flow)
leaves `structural` false: the site keeps listing `a.tmd`, the open tab gets a permanent "cannot
read" error, and `/b.html` 404s.

**Fix:** widen the structural predicate to match `Modify(ModifyKind::Name(_))`.

**Correction (verified against a live notify 8.2 watcher).** A same-filesystem rename emits
`Modify(Name(From))`, `Modify(Name(To))` and `Modify(Name(Both))`, never Create or Remove, so
the premise holds. But "the site keeps listing `a.tmd`" and "`/b.html` 404s" are true only for
a page with NO front-matter block: with one, `front_matter_moved` fires by accident (the
vanished path's digest collapses to the empty string's) and the `Site` is swapped anyway. What
is never reached in either case is `reload_open_tabs`, so the tab on the vanished page is never
told and every other tab keeps chrome pointing at the old page. Only ONE site needed widening,
not two: the outer `matches!` already accepts `Modify(_)`.

### 18. Editing a `//| name` producer never re-runs its consumers

`crates/core/assets/js/tali-js.js:604`

Change a producer cell's body from `** 2` to `** 3` and save: only that cell re-runs, so the sink
paragraph keeps displaying the old value until the reader happens to move a slider. The live
preview, whose core promise is correct block-level incremental update, displays a page that
contradicts its own source.

**Fix:** after the pass over `fresh`, run one downstream pass per fresh cell that has a `defines`,
reusing the existing cycle-checked `scheduleFrom`.

---

## Execution and cache

### 19. Warm-prefix error escapes the downstream-persist guard

`crates/server/src/exec.rs:997`

Cell B fails (flaky fetch), C runs clean against a fallback. Edit C: `plan` puts B in the warm
prefix, and the `failed_at` scan covers only the executed range, so C's output is **frozen to
disk as if it followed a successful upstream**. That is the one contradiction `_freeze` is
designed never to persist.

**Fix:** widen the scan to the whole restored-plus-executed prefix, `(0..run_end)`.

### 20. A kernel-aborted cell renders as a successful empty cell

`crates/server/src/kernel.rs:944`

With `stop_on_error: true` and the `execute_reply` status never read, a cell aborted by ipykernel
after an upstream deferred `KeyboardInterrupt` returns zero outputs and is treated as success.

**Fix:** send `stop_on_error: false`; taliesin already owns its continuation policy in the
executor, so delegating abort policy to the kernel is both redundant and wrong here.

**Correction.** There is no explicit `stop_on_error: true` at that line: it arrives from
`ExecuteRequest::new`'s `Default` (jupyter-protocol 2.0.1). The effect is as described and was
confirmed in ipykernel 7.3.0 (`kernelbase.py:868` aborts the queue on an error reply, then
`_send_abort_reply` publishes an idle status, which the read loop takes for completion). Reading
the reply status was considered and rejected: it could only REPORT the swallowed cell, and not
reliably, since the shell drain gives up after 5 s on exactly the path that reaches this.

### 21. Single-file build ignores the project's `python:` pin

`crates/server/src/build.rs:846`

`build posts/p.tmd` inside a project resolves with `field=None` and falls back to bare `python3`,
while `preview` and the site build both honour `python:`. The doctor's own fix line recommends
setting the key that this path ignores.

**Fix:** when `enclosing_site_root` returns `Some`, read that project's `python:` and pass it,
through `Site::discover_scoped` so the page set is narrowed to the page being built.

**Correction.** It does not fall back to bare `python3` in general: `<root>/.venv` and
`TALIESIN_PYTHON` still resolve, and only the pin is skipped (`doctor`'s own report prints
`1. _site.yml python:  not set`). The unreported half is worse than the reported one: the
single-file build deliberately SHARES the project's `_freeze/<page>.json`, so the mismatch seeds
a different interpreter id and the two verbs permanently miss each other's entries.

---

## Remaining two

### 22. Listing blocks are untargetable by the ops that update them

`crates/core/src/site/mod.rs:1489`

`diff_blocks` emits `Update { target_id: "listing-0-posts" }` but the `<ul>` in the DOM carries no
`data-block-id`, so the client silently drops the op and the open index keeps showing the old
card. Breaks the `data-block-id` invariant the whole incremental model keys on.

**Fix:** emit the id on the wrapper the block already has.

### 23. Wide tables get `role="region"`, destroying table semantics

`crates/core/assets/js/code-enhance/16-scroll-a11y.js:22`

Screen readers announce "scrollable table region" but expose content as flat text: table
navigation, row/column announcement and header association all stop working, on exactly the wide
tables where they matter most (this hits `docs/guide/reference/frontmatter.html`).

**Fix:** keep `tabindex="0"`, drop the role, and let a `<caption>` name the table; reserve
`role="region"` for `<pre>`, whose half AP7 verified sound.

**Correction.** "This hits `docs/guide/reference/frontmatter.html`" is only half true: at a
1080px viewport no table overflows and the enhancer never fires. It fires below roughly 700px.
Do not check this at a desktop width and conclude it is a false positive. Two things the finding
missed, both settled by A/B-ing Chrome's accessibility tree on the live page: `base.css` sets
`table { display: block }` unconditionally, which historically stripped table semantics and would
have made the fix a no-op (current Chrome computes the role from the DOM regardless, so it does
work); and `aria-label` outranks `<caption>` in the name computation, so restoring the role
without standing the label aside would have replaced an author's caption with "Scrollable
table".

---

## What this audit did NOT do

Stated plainly so nothing here is read as broader than it is.

1. **The minor/advice tier is unvetted.** 143 findings (91 minor, 52 advice) were collected but
   never faced a verifier: the run was re-scoped mid-flight to spend only on critical/major.
   They are listed in the appendix as **leads, not findings**. Expect a meaningful refutation
   rate among them.
2. **Round 3 never ran.** The completeness critic proposed 6 further lenses (`search-index-build`,
   `config-silent-coercion`, `image-annotation-pipeline`, `bibtex-alphabet`,
   `single-file-project-seam`, `script-gating-live-ops`) and all 6 finders died on a model usage
   limit. The audit is **not saturated**; the critic still had territory to name.
3. **Zero of 23 majors were refuted**, which is unusual enough to state. The verifiers did quote
   specific code at specific lines and the two I re-checked by hand (findings 1 and 2) held up
   exactly as described, but a 100% survival rate is weak evidence of verifier independence.
   Treat each fix as needing its own mutation check per the standing rule.
4. **Nothing was reproduced by execution.** Every finding is derived from reading code. No
   failing test was written, no document was built to witness a defect. The reproduction commands
   in the scenarios are proposals, not transcripts.
5. ~~**No fix was applied.**~~ Superseded the same day: all 23 are fixed (see the status note
   at the top). What that closes and what it does not:
   - Every finding was reproduced BEFORE its fix, which retires caveat 4 for the 23 majors.
     Three were reproduced in a browser or against a live watcher rather than a unit test.
   - Every fix was mutation-checked individually, which is the answer to caveat 3: a 100%
     survival rate is still weak evidence about the verifiers, but each fix now has a test
     that provably fails without it.
   - Caveats 1 and 2 stand unchanged. The 143 leads are still unvetted, and round 3's six
     lenses were never run, so the audit is still **not saturated**.
   - Four descriptions were wrong in detail (17, 20, 21, 23) and four of the audit's own
     suggested fixes were wrong or unavailable (2, 8, 10, and Cluster A's summary). All are
     corrected in place above. That is the useful signal about this method: the finders were
     reliable about WHAT was broken and much less reliable about WHY and about what to do.

## Suggested sequence

1. Finding 1 (data loss) and finding 2 (injection): both are small, and one destroys work.
2. Cluster A as one branch (4 findings, one root cause, two of them false `--check-only`
   failures).
3. Cluster B as one branch (3 findings, one root cause, restores the FA15 guarantee).
4. Findings 3 to 6: wrong published output, each independent and small.
5. Cluster C, then the preview-staleness three, then execution, then the last two.

Per the standing rule, verify each by **mutation** (restore the bug, watch the named test fail),
not by a green suite.

---

## Appendix: unvetted leads (143)

**These did not face a verifier.** Treat as a grep list, not a defect queue.

> Four of these were fully verified 3/3 in the audit's first window, before the
> re-scope, so they carry more weight than the rest: `math_vocab.rs:498` (a drift gate
> that cannot fail, it checks `tali-math-error` but KaTeX parse errors render as
> `katex-error`), `render/page.rs:597` (`ship_katex` decided by a bare substring scan,
> the banned FA11-FA13 family, so a code sample showing `<span class="katex">` ships
> ~369 KB of fonts to a math-free page), and the two dev-server takeover-probe items
> at `serve/mod.rs:248` and `serve/mod.rs:175`.


### minor (91)

- `.githooks/pre-push:156` , The suite serialization gates.sh calls load-bearing is absent from both automatic runners, and a server unit test's env mutation makes the raced mode genuinely unsound
- `.github/workflows/ci.yml:11` , ci.yml has drifted from gates.sh twice (census gate, README VERSION-pin gate) and the trio cross-check cannot see it
- `corpus/README.md:47` , corpus/README says "All 82 documents" but the corpus holds 81
- `crates/core/assets/css/base.css:43` , prefers-reduced-motion erases the search-hit highlight entirely (fill-forwards jumps to the transparent end state)
- `crates/core/assets/css/base.css:96` , The 'Skip to table of contents' link overflows the viewport when focused on narrow screens
- `crates/core/assets/js/tali-js.js:183` , bindDefines re-runs cycle-diagnosed cells (overwriting their diagnostic) and runs in registration order, not the document order its comment claims
- `crates/core/src/cite/format.rs:96` , Edition field is the one BibTeX field interpolated into HTML unescaped
- `crates/core/src/cite/render.rs:442` , Bracketed citation scanner checks no @ word boundary, contradicting at_word_boundary's own comment
- `crates/core/src/cite/render.rs:456` , Citation locator and fallback text are double-escaped (esc over already-escaped HTML)
- `crates/core/src/diagnostics/a11y.rs:48` , Heading-skip scan is blind to headings inside ::: containers, reporting skips the DOM outline does not have and missing ones it does
- `crates/core/src/diagnostics/anchors.rs:46` , Anchor validator compares comrak's percent-encoded fragment against the raw id, so an in-page link to a non-ASCII heading is a false broken-link Error
- `crates/core/src/diagnostics/bibliography.rs:105` , Bare-@key validator fires on citation keys shown in inline code, disagreeing with cite::process's own definition of prose
- `crates/core/src/diagnostics/links.rs:75` , Link validator never percent-decodes an href, so a %20-spelled link to an existing file is a false broken-link Error
- `crates/core/src/frontmatter.rs:234` , date: lint is silent for non-string YAML scalars that the feed and sitemap read and then mis-stamp or drop
- `crates/core/src/frontmatter.rs:323` , An unknown hero.actions key that shares its name with a top-level front-matter key is located at that unrelated key's line
- `crates/core/src/includes.rs:111` , The include cycle guard fires one full lap late: the primary document is never on the stack
- `crates/core/src/includes.rs:149` , Include expansion resets fence state per file, diverging from comrak after an included file's unclosed ```
- `crates/core/src/includes.rs:201` , Non-cyclic include expansion is unbounded and runs outside every resource guard (diamond amplification)
- `crates/core/src/includes.rs:314` , An include directive inside a 4-space-indented code block (or a fence indented ≥4 in a nested list) is expanded, not left literal
- `crates/core/src/math_vocab.rs:498` , The math-vocabulary drift gate cannot fail: it checks the engine-failure class, but parse errors render as katex-error
- `crates/core/src/render/divs.rs:30` , Div fences and shortcodes inside INDENTED code blocks are treated as live markup - both line scanners track only fenced code, and parse_fence accepts unlimited indentation
- `crates/core/src/render/divs.rs:208` , An indented (4+ space) ::: fence or include directive is honored where comrak reads indented code
- `crates/core/src/render/divs.rs:211` , An unmatched ::: close is silently blanked while the mirror unmatched open gets a located warning
- `crates/core/src/render/emit.rs:45` , Display-only {.lang} fences get cell treatment: their #| lines are silently stripped
- `crates/core/src/render/emit.rs:538` , Image alt text glues words together across a soft line break
- `crates/core/src/render/extension/mod.rs:114` , The unknown-shortcode double-warn suppression rests on a false premise: a not-alone-on-its-line include ships literal with zero diagnostics
- `crates/core/src/render/figure.rs:129` , A captionless #fig- figure publishes a dangling 'Figure N: ' - emit_figure drifts from numbered_caption on the empty-caption case its own comment says cannot drift
- `crates/core/src/render/image_meta.rs:111` , image_meta bypasses asset_fs_path, so a percent-encoded image ref gets no width/height and skews the LCP designation
- `crates/core/src/render/mod.rs:1235` , dedup_element_ids is not actually last: cite::process emits `id="ref-<key>"` elements after it runs
- `crates/core/src/render/mod.rs:1768` , map_span's clamp compares file labels only, so the same partial included on adjacent lines can still emit an inverted data-sourcepos
- `crates/core/src/render/mod.rs:2098` , Static-build content gates are needle scans on finished HTML, the class the one-walker rule exists to kill
- `crates/core/src/render/page.rs:597` , ship_katex is decided by a bare substring scan of the finished body, the banned FA11-FA13 scan family
- `crates/core/src/render/theme.rs:144` , Printing from dark mode keeps dark-baked mermaid SVGs on white paper: the beforeprint light-swap silently skips the one renderer that needs a re-render
- `crates/core/src/site/book.rs:16` , A chapter listed twice in chapters: is neither diagnosed nor deduped: the sidebar numbers it twice, the page renders as the first, prev/next honors only the first
- `crates/core/src/site/book.rs:236` , chapter_heading_in parses heading attributes with its own looser rules than the render: `{-}` is honored in the sidebar but published as literal text, and any mid-text `{` truncates the chapter label
- `crates/core/src/site/book.rs:257` , A book chapter's front-matter image: is silently inert, book_pages hardcodes card_image: None, so og:image is never emitted
- `crates/core/src/site/chrome.rs:234` , validate_chrome_links fails the pre-publish gate on a book's nav: items, chrome a book never renders
- `crates/core/src/site/config/mod.rs:329` , key_line locates a chapter/nav-item diagnostic at the first occurrence of the key name anywhere in _site.yml, which is routinely the wrong section's line
- `crates/core/src/site/feed.rs:94` , A page with multiple listings syndicates only its first uncapped listing, contradicting atom_feeds' own contract
- `crates/core/src/site/feed.rs:152` , Feed URLs skip percent-encoding: <id>, self link and autodiscovery href ship the raw path while og:url for the same page is encoded
- `crates/core/src/site/feed.rs:222` , <updated>/<published> are the only unescaped interpolations in build_atom, and the unvalidated time half can reach them
- `crates/core/src/site/mod.rs:340` , discover_single parses every sibling under the parent directory and surfaces their warnings, contradicting its own documented contract
- `crates/core/src/site/mod.rs:1348` , Unpadded date: breaks newest-first order in feeds and listings, the sort compares raw strings, unlike every other date: reader
- `crates/core/src/site/mod.rs:1424` , Listing card link's accessible name leads with the thumbnail alt; the emitter never took the fix its own stylesheet prescribes
- `crates/core/src/site/mod.rs:1549` , Anchor scan's naive fence toggle desyncs on nested fences, minting phantom xref targets
- `crates/core/src/site/search.rs:105` , {js}/client-cell source leaks into the Cmd-K search index: the text extractor keeps <script> bodies
- `crates/core/src/site/search.rs:178` , headings_with_pos reads heading markup inside {js} script bodies as real sections
- `crates/core/src/site/xref.rs:72` , Duplicate cross-reference label warning prints an include-expanded buffer line as the page's own line
- `crates/core/src/site/xref.rs:418` , rewrite_cross_refs duplicates or flattens block content when an <a href="#…"> has no closing </a>
- `crates/server/src/build.rs:184` , --flag=value grammar exists only for preview --port; build/doctor reject --format=json with no did-you-mean
- `crates/server/src/build.rs:239` , --check-only --jobs auto/0 slips past the refusal the help text promises
- `crates/server/src/build.rs:318` , build <dir> [out.html] silently discards the second positional and exits 0
- `crates/server/src/cli.rs:333` , preview's missing-file error skips the shared did-you-mean and wears a retired verb's prefix
- `crates/server/src/doctor.rs:300` , doctor treats single-dash flag typos as directory names, against the recorded leading-dash rule
- `crates/server/src/exec.rs:829` , Kernel death between the two liveness polls resurrects the recorded empty-warm-prefix wedge
- `crates/server/src/exec.rs:881` , Run loop keeps queueing cells onto a kernel already reported wedged by interrupt_ignored
- `crates/server/src/exec.rs:1409` , is_uncacheable substring-scans finished HTML; a cell that prints class="tali-error" is treated as failed forever
- `crates/server/src/freeze.rs:148` , One digest per language cannot label a freeze file whose history entries span package eras
- `crates/server/src/interpreter.rs:81` , doctor never shows the found-but-outranked ancestor venv the trail records for it
- `crates/server/src/kernel.rs:1034` , Flood-cap grace expiry without Idle is read as success, silently abandoning a still-running cell with only a truncation notice
- `crates/server/src/lsp.rs:1218` , vocab["frontmatterValues"] is read in the completion dispatch but the key was deleted from vocab.rs in the 2026-08-17 theme cut, leaving Ctx::FrontmatterValue a permanent silent no-op
- `crates/server/src/lsp_nav.rs:47` , LSP re-derives the citation-key grammar narrower than core, so hover/F12 silently fail on keys the renderer resolves
- `crates/server/src/lsp_nav.rs:352` , offset_to_line_col counts only \n, so F12 on an xref or citation lands on the wrong line in a lone-CR buffer
- `crates/server/src/lsp_outline.rs:93` , lsp_fold and lsp_outline fence tracking ignores fence width and the no-info-string close rule, desyncing folds on shipped docs and inventing phantom outline headings
- `crates/server/src/packages.rs:57` , Process-wide manifest memo survives Restart kernel, so the package digest is stamped stale and the honesty warning fires falsely
- `crates/server/src/serve/mod.rs:175` , Identity probe reads the responder's body with an uncapped read_to_end
- `crates/server/src/serve/mod.rs:248` , Takeover SIGTERM trusts the port-holder's root and pid claims
- `crates/server/src/serve/mod.rs:317` , content_type has no text/html arm, so a static .html asset downloads in preview
- `crates/server/src/serve_site/mod.rs:566` , Nothing pins the host/origin guards being installed on the live router
- `crates/server/src/serve_site/mod.rs:1131` , Editing python: in _site.yml silently never reaches the ExecPool until the server restarts
- `crates/server/src/serve_site/mod.rs:1351` , Every page's first build takes a warm-pool slot, so cell-free pages evict genuinely warm kernels
- `crates/server/src/serve_site/mod.rs:1430` , In-flight build resurrects page state that reload_open_tabs cleared, blocking the corrective rebuild
- `crates/server/src/serve_site/mod.rs:1733` , A front-matter change arriving as create/remove never refreshes open listing tabs, and a reload cannot recover
- `crates/server/src/serve_site/mod.rs:1746` , Front-matter edit fans out to every ever-visited page; the comment claims a MAX_WARM_PAGES cap that does not exist
- `crates/server/src/serve_site/mod.rs:1775` , Fixing a missing image never clears its error diagnostic or the broken image in the preview
- `crates/server/src/serve_site/mod.rs:1812` , The refresh_xrefs gate skips .md include partials, contradicting its own justification
- `crates/server/src/serve_site/mod.rs:1901` , A page removed from the site keeps its warm kernel in the ExecPool with no path to reclaim it
- `docs/guide/reference/cli.tmd:313` , CLI reference promises a lint for a page image: with no site url:, which no code emits
- `docs/guide/reference/frontmatter.tmd:392` , Configuration reference documents the uncited-bib-entry lint cut on 2026-08-20
- `docs/guide/using/choosing.tmd:158` , Choosing page's dogfooding figure (112 tracked .tmd files, 12,904 lines) is stale and carries no date or instrument
- `docs/internals/block-model.tmd:65` , Block-model chapter says every gap pair becomes an Update; emit_gap now demotes moved-block pairs
- `docs/internals/execution.tmd:3` , Execution chapter description advertises "kernels forked before you need them" after the fork pool was cut
- `editor/vscode/src/client.ts:52` , Unserialized LSP restarts orphan a second server or clobber the live client
- `editor/vscode/src/client.ts:77` , Every language-server start creates a workspace FileSystemWatcher nothing disposes
- `editor/vscode/src/extension.ts:124` , A project preview's page map is latched at open; a chapter added later never syncs
- `editor/vscode/src/server.ts:44` , PreviewServer.start leaves the losing race promise to reject unobserved
- `web-client/client.js:997` , Per-op mountedGen advancement lets a partially delivered burst satisfy the reconnect gen-skip, silently freezing stale blocks
- `web-client/search.js:305` , Cmd-K palette is aria-modal with no dialog role or accessible name on the container
- `web-client/search.js:1012` , Ctrl-click on the chrome's search button fires both click-to-source and the Cmd-K palette at once
- `web-client/toc-spy.js:41` , toc-spy's decodeURIComponent throws on a heading id containing a bare %, killing the scrollspy and the preview's whole afterChange tail
- `web-client/toc-spy.js:75` , TOC scrollspy exposes the current section visually only; no aria-current, unlike every other 'you are here' marker in the product

### advice (52)

- `.githooks/pre-push:145` , pre-push's hollow-gate preflight covers the Python half of the suite's silent skips and is silent on the Node half
- `CLAUDE.md:57` , CLAUDE.md credits emit.rs with code line-wrapping that was cut in wave 7
- `Cargo.toml:89` , The workspace manifest's image-dependency comment still claims the server transcodes AVIF, cut in wave 4
- `crates/core/assets/css/base.css:578` , base.css's sidenote print comment describes `taliesin pdf` in the present tense and cites a PDF verification, but the verb was cut in the 18-to-6 CLI reduction
- `crates/core/assets/css/base.css:1135` , Dead interactive chrome prints: the search button and book drawer button reach paper
- `crates/core/assets/css/dark.css:9` , dark.css line 9 is a dead rule: base.css already makes pre.mermaid transparent in every theme, and the file's own header says it keeps syntax scopes only
- `crates/core/assets/js/code-enhance/04-focus-trap.js:2` , 04-focus-trap.js's comments direct the reader to 13-reader-menu.js, a fragment deleted with the reader menu, including 'see that file's own comment'
- `crates/core/src/diagnostics/headings.rs:20` , validate_duplicate_heading_ids is dead code: both render-side dedup passes rename every duplicate before it can ever see one
- `crates/core/src/frontmatter.rs:218` , Two comments in the lens reference machinery deleted months ago (the TAL-* catalogue and validate_unsupported_keys)
- `crates/core/src/highlight.rs:75` , The ojs highlight alias honors another tool's spelling with no in-tree witness
- `crates/core/src/includes.rs:327` , includes.rs carries a private byte-for-byte copy of the fence state machine divs.rs exists to centralize
- `crates/core/src/math.rs:68` , Math memo clones the rendered KaTeX HTML String while holding the global cache mutex, on the concurrent-harvest hot path
- `crates/core/src/math.rs:94` , The Mutex around the KaTeX worker Sender is dead weight: mpsc::Sender has been Sync since Rust 1.72
- `crates/core/src/render/divs.rs:175` , The bare '::: classname' fence spelling is an unused second spelling of the braced form - cut candidate
- `crates/core/src/render/emit.rs:74` , data-tali-cell is emitted for a reader control deleted 2026-08-04
- `crates/core/src/render/mod.rs:2079` , code_scripts() is dead public API: zero callers in the entire tree
- `crates/core/src/render/mod.rs:2997` , data-section-end has had zero consumers since it shipped; every render pays for it
- `crates/core/src/render/model.rs:205` , Block::nested doc overstates: the folded copies' html goes stale after dedup and cite rewrite the container
- `crates/core/src/render/page.rs:175` , PageParts.mode doc names a `Bare` OutputMode that was cut in wave 11
- `crates/core/src/render/page.rs:753` , favicon_link escapes its href with escape_html, which leaves `"` unescaped inside a double-quoted attribute
- `crates/core/src/schema.rs:21` , schema.rs's recorded keep-justification is spent: the companion consumer is cut, SITE_SCHEMA has zero readers, and the schema's delivery path is 'copy the file out of the repository'
- `crates/core/src/site/book.rs:186` , Each book chapter file is read three times and YAML-parsed twice per discover, justified by a comment naming a feature cut in August
- `crates/core/src/site/chrome.rs:194` , A navbar icon link ignores its configured text: for the accessible name while the footer honors it
- `crates/core/src/site/feed.rs:26` , nav_ordered's nav-label half has been dead since llms.rs was cut: its only caller discards it
- `crates/core/src/site/links.rs:202` , Four copies of the sourcepos start-line parser in taliesin-core, one of them a self-described 'local copy'
- `crates/core/src/site/search.rs:12` , search_sections' per-page keying and both of its doc comments promise a single-page refresh nothing implements and the whole-index rule forbids
- `crates/core/tests/common/mod.rs:85` , Dead _extensions test fixture helper survives the 2026-08-17 cut
- `crates/core/tests/stale_docs.rs:261` , docs_do_not_promise_a_ci_that_enforces_gates now enforces the pre-publication state: CI has been live since 2026-08-20 but the gate still forbids seven files from crediting it
- `crates/server/Cargo.toml:23` , serde and serde_json bypass the workspace dependency table the manifest itself mandates
- `crates/server/src/build.rs:2604` , is_local_ref is byte-identical in both crates with nothing pinning agreement between the lint and the build copier
- `crates/server/src/cli.rs:119` , Extra positionals are silently dropped (init, preview, build) or last-wins (doctor)
- `crates/server/src/exec.rs:265` , Empty `impl CellRef {}` block is dead residue
- `crates/server/src/exec.rs:775` , The 'skipped' cell-state arm is unreachable dead code left over from the cut `taliesin run` verb
- `crates/server/src/exec.rs:1344` , Doc comments across four sites name executor functions that no longer exist
- `crates/server/src/lsp_complete.rs:745` , in_code_cell counts only backtick fences while its own module and lsp_cells accept tilde fences, so a ~~~{python} cell gets language completion but never cell-option completion
- `crates/server/src/lsp_nav.rs:82` , scan_math accumulates, sorts and returns every math span with a start offset when its only caller reads only whether a span is still open, finishing half of a trim its own doc comment records
- `crates/server/src/lsp_project.rs:142` , Every project-file save makes the LSP's next publish re-pay Site::discover's two whole-project render passes, whose outputs no LSP path reads
- `crates/server/src/serve/mod.rs:538` , The asset/style half of relevant_path's EXTS is dead weight: its only consumer can never match those events
- `crates/server/src/serve_site/mod.rs:80` , Stale symbol references in load-bearing comments on the op-broadcast path
- `crates/server/src/serve_site/mod.rs:591` , ConnectInfo plumbing survives with no consumer since the --host cut
- `crates/server/src/serve_site/mod.rs:689` , Dead search-index.js fallback branch kept alive by a stale mounts comment
- `crates/server/src/serve_site/mod.rs:1414` , Stale cost comment says each preview save re-renders the whole site (~27 ms) on a path that no longer does
- `crates/server/tests/parallel_build_determinism.rs:280` , The two concurrency-determinism kernel tests are the only kernel-gated tests without the TALIESIN_REQUIRE_KERNEL escalation
- `docs/guide/reference/accessibility.tmd:26` , ACR's own staleness line names 1.0.0 as the current release; v1.0.1 is the newest tag
- `editor/vscode/package.json:29` , Marketplace metadata advertises slides and notebooks Taliesin does not have
- `editor/vscode/src/test/manifest.test.ts:479` , The engine floor is pinned to APIs the companion no longer calls
- `notes/DETECTION-DEBT.md:35` , DETECTION-DEBT's headline D=10 row (warm-page LRU unpinned) was false when re-measured: the eviction cap and order have in-module tests, missed by counting test files instead of test bodies
- `tools/gates.sh:372` , The census gate runs bare `python3` while preflight validates only `$PY`, breaking the everything-missing-reported-up-front contract
- `tools/gates.sh:415` , gates.sh's stanza numbering has rotted again: 13 gates run under 12 numbered stanzas, and the verdict comment says to read against 'the twelve stanzas'
- `tools/live-edit-bench/src/lib.rs:133` , The committed save-cost instrument times only half of an anchor-moving save: the whole-index search rebuild is unmeasured
- `web-client/client.js:162` , scanCellErrors assigns tali-cellerr-N ids that nothing reads, mutating rendered content and minting duplicate DOM ids
- `web-client/client.js:752` , Client block-op apply is O(ops x document): every op locates its target with a full-subtree querySelector
