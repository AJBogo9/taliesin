# Execution-verified audit, 2026-09-24 (Opus 5.5)

> **Status: findings only. Nothing is fixed.** Written at HEAD `9c5bd008` (1.1.0, unreleased).
> Every headline finding below was reproduced by running the release binary, and the 32 marked
> **[re-run]** were reproduced a second time, independently, by the coordinating session with
> its own fixtures. None of those 32 failed to reproduce. The repro blocks are self-contained
> because the auditors' scratch fixtures were session-local and are gone.

## Summary

Fifteen lens-scoped auditors, one coordinator, ~5.2M subagent tokens. Unlike the 2026-09-02
round (291 agents, ~16M tokens, "nothing was reproduced by execution"), the rule here was
**execution first**: a finding without a run was labelled "code-read only" and ranked down.

What matters most, in order:

1. **Four defects that lose data, leak secrets, or publish output the source cannot produce.**
   A single-file build silently overwrites files at its destination; an unreferenced symlink
   publishes a whole `.git` into the deploy; the freeze cache persists warm-kernel state, so
   `build --strict` publishes a value the code raises `NameError` on; the port-identity probe
   has no read cap, so one hostile local listener OOM-kills the preview (and, per the user's
   setup, the editor). Separately, **the release anyone can download (v1.0.1) still overwrites a
   `.tmd` source with HTML**: the fix landed 09-02 and was never released.
2. **One root cause explains the largest share of what was found: one fact, several readers.**
   Front matter is read by a line scanner, by serde_yaml and by comrak, and they disagree. Block
   structure is re-derived by ~10 hand-rolled fence trackers. Section numbers come from three
   places. Assets are judged by a validator, a copier and the preview with three rules. The
   pre-publish gate and the build compute different diagnostic sets. Each is a class, not an
   instance, and the fix in almost every case is **subtractive**: delete the second reader.
3. **The preview's invalidation is enumerated, and the 09-02 rename fix regressed it.** Every
   atomic save (vim, JetBrains, `sed -i`, `git checkout`) of a post's front matter now leaves the
   listing stale, even on a fresh GET.
4. **The gates certify less than they claim, in both directions.** False greens (a dropped book
   part passes `build --strict`; a missing `_images/` picture passes both gates) and false reds
   (a correct citation in a figure caption, an `&` in a filename, a `%20` link all fail the gate
   on working documents).
5. **The first hour for a stranger is the weakest surface**, and the project has no outside
   users yet: 0 stars, 0 issues, 2 repo page views in the last two weeks.
6. **The process records are the stalest files in the repo.** `CLAUDE.md` says CI does not run
   (it runs on every push), the `/preview` command calls a verb that no longer exists, the
   `corpus-verifier` agent runs only the core crate's tests, 12 of 27 DETECTION-DEBT rows describe
   deleted code, and 0 of the 143 leads from 09-02 were acted on in 22 days.

## How this round differs from 09-02, stated honestly

The user asked whether Opus 5.5 finds what Opus 5 could not. The model effect cannot be isolated
from the method, so here is what can be said:

- **Territory.** Five of the most productive lenses (bibtex, config-coercion, single-file seam,
  search-index, image pipeline, live-ops scripting) are exactly the round-3 lenses the 09-02
  completeness critic named and then lost to a usage limit. Much of the yield came from ground
  that round never walked.
- **Method.** Every lens ran the binary: differential testing of the fence scanners against a
  CommonMark oracle (then reproduced through the binary), a preview invalidation matrix over six
  save styles, a real kernel, a real browser with an 85-step random-edit fuzzer, a scaling fit
  over 10 to 500 page books, a stranger's install on a clean `$HOME`. 09-02 read code.
- **Covered territory, still missed.** Several findings sit in areas earlier rounds audited by
  name: the freeze cache (07-22 cache-correctness audit, 09-02 exec lens), front matter (many
  rounds), symlink containment (07-27 untrusted-document audit), the watcher (09-02 fixed two
  cases there and introduced the regression in finding C1).
- **Calibration of 09-02.** Its unvetted tier was reliable: of 86 leads checked, 83 are real,
  1 partly, 1 refuted. But 51% of the real ones are maintainer-only, and none is a P0.

Totals: roughly **150 new findings**, **~110 confirmations** of 09-02 leads, **1 refutation**
(serve/mod.rs:248, arbitrary-pid SIGTERM, blocked by the `/proc/<pid>/exe` check), and 0 of the
coordinator's 32 re-runs failed.

## Context: users and release

- `gh repo view`: 0 stars, 0 forks, 0 issues. Traffic: 2 views in 14 days. Release downloads:
  8 + 2 of the Linux tarball, 0 of either macOS build.
- README's install block pins `VERSION=v1.0.1` (tagged 2026-08-21). 31 commits since, 13 of them
  fixes, including the 09-02 P0s. `git tag --contains c60b224c` is empty.
  `git show v1.0.1:crates/server/src/build.rs` shows `NON_HTML_OUTPUT_EXTS` without `tmd`, so
  `taliesin build *.tmd` still turns `index.tmd` into HTML for anyone who installs today.
- CI runs on every push and is green (`gh run list`: 5 push runs on 2026-09-23).

---

## Part A. Fix first

### A1. Single-file build silently overwrites files at its destination (NEW, data loss) [re-run]

`build post.tmd elsewhere/p.html` copies each local image next to the output with `std::fs::copy`,
guarded only by `same_file`. A pre-existing `elsewhere/img/a.png` is replaced; two posts built
into one folder clobber each other's figures. The log says only `built elsewhere/p.html`.
Root: `crates/server/src/build.rs:458` -> `copy_local_assets` (`:1123`). Sibling of 09-02 #1.
Fix: refuse to overwrite an existing file with different content (name it), and report the copy
count. Alternative consistent with the mermaid ruling: inline local rasters in single-file mode.
The guide's "one self-contained HTML file" (`docs/guide/index.tmd:34`) is false for any document
with a local image either way.

```sh
mkdir -p desk/img post/img && echo 'USER FILE' > desk/img/a.png
cp any.png post/img/a.png
printf -- '---\ntitle: P\n---\n\n![A square.](img/a.png)\n' > post/p.tmd
taliesin build post/p.tmd desk/p.html --no-exec; head -c 4 desk/img/a.png   # now PNG bytes
```

### A2. Unreferenced symlinks publish checkout-private files, including a whole `.git` (NEW) [re-run]

`mirror_assets` copies every non-skipped file under the root and follows symlinks whose target
stays inside the enclosing checkout. The `.`/`_` privacy rule is tested against the LINK's name,
never its target. A symlink no page references (`vendor -> ../.git`) ships `.git/config` (a
token in a remote URL) and every object. `--check-only --strict`: "no problems found". The
preview refuses the same paths. Threat: a co-author's PR or a cloned project adding one link.
Root: `crates/server/src/build.rs:2402-2470`. Relates to 07-27 untrusted-document §2.4.
Fix (subtractive): refuse a followed link when any component of its canonical target below the
boundary starts with `.` or `_`.

```sh
mkdir -p co/.git/objects/ab co/blog && echo 'url = https://TOKEN@x/y' > co/.git/config
echo SECRET > co/.deploysecret; cd co/blog && printf 'title: B\n' > _site.yml
printf -- '---\ntitle: B\n---\nhi\n' > index.tmd && ln -s ../.git vendor && ln -s ../.deploysecret data.txt
taliesin build . --out ../../out --no-exec; grep -rl -e TOKEN -e SECRET ../../out
```

### A3. The freeze cache persists warm-kernel state: the published page shows output the code cannot produce (NEW) [re-run]

In the preview, a re-run of cells `[shared, run_end)` executes in a kernel that still holds state
from the previous run, and `exec.rs:1017-1023` persists those outputs. Rename a variable and
leave a dangling use: the preview shows the old value, `_freeze/` records it, and a later
`build --strict` restores it, exit 0. A fresh kernel raises `NameError`. This is the one
contradiction `_freeze` exists never to persist (same argument as `exec.rs:987-996`).
`docs/internals/execution.tmd:189` ("What is never persisted") omits it.
Fix: persist a run's outputs only when the kernel never executed any cell at index >= `shared`
(a high-water mark on `LangState`). Cost: the first build after a preview session re-runs from
the earliest edited cell, the cold-start cliff DO-NOT-REBUILD already accepts.

```sh
printf -- '---\ntitle: W\n---\n\n```{python}\nthreshold = 10\n```\n\n```{python}\nprint(f"threshold is {threshold}")\n```\n' > w.tmd
taliesin preview w.tmd --port 4733 &   # open the page, wait for _freeze/w.json
sed -i 's/^threshold = 10$/limit = 10/' w.tmd; sleep 6; kill %1
taliesin build w.tmd --strict --stdout | grep 'threshold is 10'     # published, exit 0
TALIESIN_NO_CACHE=1 taliesin build w.tmd --stdout | grep NameError # the truth
```

### A4. The identity probe reads a port-holder's reply with no size cap (CONFIRMS LEAD serve/mod.rs:175, severity raised) [re-run: code]

When its port is taken, `preview` probes ten ports concurrently and `read_to_end`s each reply,
bounded only by a 2 s timeout. A local listener streaming whitespace drove taliesin past a 2 GB
cap in about a second (OOM-killed). The user's global notes record that an OOM in the VS Code
snap scope takes the editor down, and a preview launched from its terminal runs in that scope.
Fix: `(&mut sock).take(64 * 1024).read_to_end(&mut raw)`; a real answer is under 300 bytes.

### A5. Cut 1.1.0 once A1 to A4 land

The README's pinned release carries the `build *.tmd` data-loss bug and the fence-token attribute
injection that 09-02 fixed. The stale-docs gate already pins the README version to an existing
tag, so the release is a tag plus the README bump.

---

## Part B. One fact, several readers (the dominant root cause)

`CLAUDE.md` already states the principle ("one policy, two readers"). What this round shows is
how many live instances remain, and that each fix so far has closed an instance while the class
kept producing. Each sub-part below names the readers, the findings they cause, and the reader
to delete.

### B1. Front matter: a line scanner, serde_yaml, and two splitters (NEW; found independently by three lenses) [re-run]

The page renderer does not read front matter with YAML. `render/mod.rs:683-716` reads `title`,
`subtitle`, `date`, `description` with `fm_extract::extract_field` (`fm_extract.rs:133-150`, which
trims every quote character from both ends), and `detect_toc`, `detect_title_block_hidden`,
`detect_execute_cache` are the same kind of scanner. The site layer (og tags, cards, feed) and
`author:` use serde_yaml. comrak has its own front-matter splitter; `frontmatter::front_matter_block`
is a third that accepts `--- ` and `...`. Consequences, all with `--check-only --strict` exit 0:

- `title: 'It''s here'` publishes `It''s here`; `"Say \"hi\""` keeps the backslashes; `# comment`
  is published as part of the title; `description: >-` publishes `>-` (also into
  `<meta name=description>`); a title wrapped onto a second line is truncated. og:title, the
  card and the feed show the correct value, so one page carries two titles.
  **The guide's own annotated example (`docs/guide/reference/frontmatter.tmd:299`) triggers it.**
- `toc: false  # comment` still shows the TOC; `execute: cache: false  # live data` keeps the
  cache on.
- **Cell options too:** `#| echo: false  # hide setup` publishes the hidden cell's code.
- A trailing space on a delimiter (`--- `) or a `...` closer: comrak does not see front matter,
  so the YAML renders as a heading; with a later `---` rule, comrak's node runs to that rule and
  **every paragraph in between vanishes from the page**. quarto-cli's own tests contain 8 files
  with a trailing-whitespace delimiter.
- A lone-CR file bypasses the line-ending normalization in site discovery, so `draft: true` is
  ignored and the draft is published (`site/frontmatter.rs:37`).

```sh
printf -- '---\ntitle: My Post\n--- \n\nFirst paragraph.\n\n## Setup\n\nInstall it.\n\n---\n\nAfter.\n' > fm.tmd
taliesin build fm.tmd --stdout --no-exec | grep -c -e 'First paragraph' -e 'Install it'  # 0
printf -- '---\ntitle: T\n---\n\n```{python}\n#| echo: false  # hide\nSECRET = 1\n```\n' > e.tmd
taliesin build e.tmd --stdout --no-exec | grep -c SECRET                                   # 1
```

Fix (subtractive): parse the block once with serde_yaml (already done for `author:`), read every
field from that value, delete the line scanners; strip the block with `front_matter_block` before
comrak and turn off comrak's front-matter extension, so one splitter exists; normalize line
endings in the one raw-source reader every `.tmd` read goes through. For `#|` options, drop a
trailing ` #` comment before `yaml_bool_word`.

### B2. Block structure: ~10 hand-rolled fence trackers vs comrak (NEW class; several instances were leads) [re-run]

Trackers: `render/divs.rs:119` (`code_fence`), a private copy at `includes.rs:312`, the shortcode
pass in `render/extension/mod.rs`, `prose.rs:50`, `site/mod.rs:1610` (anchor scan),
`xref::cell_label_anchors`, `lsp_outline.rs:51`, `lsp_fold`, `lsp_cells.rs:71`,
`lsp_complete.rs:740`, `lsp_nav.rs:105` (`scan_math`). A census found **zero divergences in the
current repo and tech-blog**, so the exposure is future documents. Reproduced classes:

- **A line starting with ```` ```pip install x``` ```` (inline code, a paragraph to comrak) opens
  a fence in every tracker.** Every later `:::` callout ships as literal text, includes stop
  expanding, cross-page anchors below vanish (false broken-xref errors), and in the editor the
  outline, folds, cell regions and `#|` completion stop. Single-doc gate: clean.
- **HTML comments are invisible to every scanner.** A commented-out `{{< include _draft.md >}}` is
  expanded; the partial's own `<!-- -->` closes the outer comment early and **draft text is
  published visibly**. Commented-out headings remain live cross-page targets and shift numbers; a
  commented-out `# Old Title` becomes the chapter's sidebar label and `<title>`.
- A code sample in a 4-space-indented list item is rewritten (shortcodes expanded into it, `:::`
  lines deleted) and draws a false "empty callout" error that fails the gate (CONFIRMS
  divs.rs:30/208, includes.rs:314, widened to any single-level 4-space list).
- A `:::` callout inside a list item loses its wrapper and draws a wrong diagnostic.
- LSP: setext headings and ATX headings indented 1-3 spaces are missing from the outline; math
  completion disappears after a `~~~` block containing a backtick fence; cellRegions marks
  commented-out and indented-code cells executable.

```sh
printf -- '---\ntitle: T\n---\n\n```pip install x``` installs it.\n\n::: {.callout-note}\nNote.\n:::\n' > bt.tmd
taliesin build bt.tmd --stdout --no-exec | grep '::: {.callout-note}'   # published literally
printf 'Draft.\n\n<!-- TODO -->\n\nMore draft text.\n' > _draft.md
printf -- '---\ntitle: C\n---\n\n<!--\n{{< include _draft.md >}}\n-->\n' > cm.tmd
taliesin build cm.tmd --stdout --no-exec | grep 'More draft text'       # visible paragraph
```

Fix: one line classifier in core, derived from comrak (parse with the renderer's options, walk
the AST once: fence opener/body/closer with info, indented code, HTML block types 1-5 as opaque,
container depth, headings including setext). ~100-150 lines; deletes ~8 trackers and their tests.
`:::` stays hand-recognized but only on lines the classifier approves (depth 0, not opaque). The
include pass classifies each file alone and warns when a partial ends inside an open fence. The
LSP uses the same parse-only classifier, memoized per (uri, version); comrak parses half-typed
buffers fine. Cheap: a 40 KB document's whole build is 5.5 ms.

### B3. Reading finished HTML: the walker returns undecoded values, and substring scans remain (NEW + leads) [re-run]

`render::attrs`/`attr_value` (`render/mod.rs:3308`, `:3398`) return values still entity-encoded.
A file named `R&D.png` draws "local asset not found: `img/R&amp;D.png`" (the gate fails), is
omitted from `build --out`, loses width/height, and the LCP `fetchpriority` moves elsewhere; a
page `R&D.tmd` draws three false broken-link errors. Cmd-K ids are read by `split_once("id=\"")`
(`site/search.rs:195`), so a hit on `{#r&d-notes}` goes nowhere. Substring scans CLAUDE.md bans
are still live: `ship_katex` (`page.rs:610`, +371 KB of KaTeX on a math-free page that shows
`class="katex"` in code), the mermaid gate (+3.5 MB), skip links (`page.rs:301`), and the cell
error marker (`exec.rs:1409`, `build.rs:635`): a cell that prints `<pre class="tali-error">` fails
`--strict` as an uncaught exception and is never cached.
Fix: decode character references once inside the walker's `Attr::value` (keep the raw span for
`rewrite_attr_in_tags`); route every remaining scan through the walker; carry "errored" as data
on the Cell instead of reading it back out of HTML. Make `escape_html` also escape `"` (measured
+0.104% bytes on the tech-blog build), which makes the four escape helpers one function and
closes the text half of the spoofing class.

### B4. One page pass, one diagnostic set, one severity (NEW) [re-run]

Five per-verb pipelines re-sequence the same page pass: `build.rs:760` (single file),
`build.rs:1428` (site), `lint.rs:333`, `lint.rs:456`, `serve_site/mod.rs:1310` + `:740`
(`yaml_error` handled at 5 sites, `page_static_diagnostics` called at 5, executor setup 3 times).
And two diagnostic types: `protocol::Diagnostic` and `lint::Diagnostic`. Consequences:

- **The preview shows every error-severity defect as a warning.** `preview_diag.rs:17` and
  `serve_site/mod.rs:772`, `:1422` call `Diagnostic::warn` and never read `w.severity`; the
  dev menu goes red only on `"error"`. Verified over the websocket: "local asset not found"
  arrives as `"level":"warning"` while `--check-only` calls it an error and exits 1.
- **`build --strict` passes a `_site.yml` that drops a book part** (`"diagnostics": []`, exit 0,
  chapter missing), while `--check-only` reports it: the 09-02 Cluster C fix reached one gate.
  `publish.sh` runs check-only first, so the author's deploys are safe; nobody else's are.
- `build <file> --check-only --strict` exits 0 on a cross-page reference that
  `build <file> --strict` then fails (`build.rs:816` vs `lint.rs:348`).
- Every `Site::warnings` string becomes an error attributed to `_site.yml` with `line: null` in
  `--check-only` (including page-level ones), and nothing at all in `build --strict`.
- For a lone document, the preview renders `hero:` and `listing:`; `build <file>` drops them.
- Include-located diagnostics print a path relative to the wrong directory on every CLI surface
  (`lint.rs:146` passes `w.file` through unrebased; the LSP rebases correctly).
- `build <file> --out` with an unbundled project asset: warning only, `--strict` exits 0.

Fix: one `page_pass(site, page, src, exec)` that every verb wraps; give `build <file>` a
`Site::discover_single`/`discover_scoped` like the preview; make `Site::warnings` `render::Warning`
values; serialize `lint::Diagnostic` onto the wire and map severity once.

### B5. Section numbers and chapter labels (NEW) 

Three numbering sites (displayed number via the HTML walker, same-page links via the AST
registry, cross-page links via a source scan) disagree on setext headings, `##\t`, headings in
blockquotes and lists, raw `<h2>`, and commented or indented-code headings. The blockquote/list
divergence was **introduced by 985dd96f** (the 09-02 fix for finding 10). Chapter labels are a
slice of the source line (`book.rs:231-241`): inline code, emphasis and entities publish raw into
the drawer, pager, `<title>`, og:title and search; `# Appendix {-}` publishes a literal `{-}`;
`# The {x} set` truncates to "The".
Fix: number once from one heading list taken from the harvest render (which already renders every
page); take labels from the rendered heading. Deletes `heading_levels` and half of
`scan_page_anchors`.

### B6. Assets: validator, copier and preview use three rules (NEW) [re-run]

- **Images in `_`-prefixed folders** (`_images/`, beside an `_includes/` partial) are served by
  the preview and pass both gates, but `mirror_assets` skips `_`-dirs and
  `deploy_referenced_sources` ships only source types (`build.rs:1156`): the deploy has no image.
  Same for `logo:`, cards and og:image. Fix (subtractive): delete the `SKIP_EXT` filter there.
- `srcset` and `<picture><source>` are neither validated nor copied (`build.rs:2646` harvests
  `src`/`href`/`poster`; `site/links.rs:101` already parses `srcset`).
- Front-matter `image:`, `logo:` and `favicon:` are never checked for existence; `image:` with
  `%20` is double-encoded into og:image (`site/meta.rs:55`).
- The validator accepts `../outside.png`; the site build never copies it (`assets.rs:53`).
- A site build publishes draft-directory assets and editor residue: `index.tmd~`, `#index.tmd#`,
  `.orig`, `.py`, `.ipynb`, `Makefile`.
- A book chapter written `- ./a.tmd` is built, then deleted by the stale sweep; the book links to
  it and the sitemap lists `https://site/./a.html`; strict gate clean (`book.rs:180` never
  normalizes). [re-run]
- EXIF-rotated photos get transposed width/height (a 373 px layout shift); the decoder is picked
  by extension (`image_meta.rs:120`).
Fix: one shared list of URL-bearing attributes plus the `srcset` splitter, and one containment +
publication rule consulted by the validator, the copier and the preview.

### B7. Captions: exec and core render them differently (NEW)

Python `fig-cap`/`tbl-cap` publish literal `*emph*` and `$x^2$` and an italic "Figure N"; mermaid,
js, `lst-cap` and image captions render Markdown and KaTeX with an upright label. Live on the
guide (Figure 5.2 vs 5.1/5.3). `exec.rs:1614-1630` bypasses `render::numbered_caption`. Fix: one
caption function. Also: any image with alt text becomes a numbered "Figure N" (shifting every
`@fig-` number after it) and is read twice by screen readers (`figure.rs:57`, `:101`); only a
`#fig-` id should make a figure.

### B8. Search: two extractors, and tag boundaries inside code (NEW) [re-run]

- `strip_tags_inner` pushes a space at every tag, including each syntect `<span>`, so the index
  holds `matplotlib . pyplot` and **no code on any page can be found by typing it** (`plt.show()`,
  `np.linspace`, `{{< include`). Headings with inline code show `( exec.rs )`. Fix: no boundary
  inside `<pre>`/`<code>`; extract titles with the TOC's non-separating `strip_tags`.
- `build file.tmd` searches with search.js's DOM extractor, not the Rust index: it indexes raw
  TeX from MathML annotations, `<script>` bodies (the unfixed half of 09-02 #4), mermaid's SVG
  CSS, and caps sections at 1500 chars. Fix: inline the index `discover_single` already builds;
  delete the DOM branch.
- An HTML comment containing an odd number of apostrophes empties the rest of its section in the
  index; a commented-out heading becomes a result pointing at an id not on the page.
- Executed figure and table captions are never searchable (the index render does not execute).
- The typo tier fails on any word touching punctuation (76.6% recall, 99.6% with a Unicode word
  split) and re-splits every body per keystroke (5-8 ms, contradicting `search.rs:232`).

### B9. Citation grammar and bibliography scope (NEW + lead)

The LSP re-derives the citation grammar and misses `[@k, p. 3]`, both keys of `[@a; @b]`,
`[-@k]`, keys with `/`, `+` or non-ASCII, and paren-delimited entries (9 of 11 probes); it ignores
the shared `_site.yml` bibliography entirely (no completion, hover or F12). Fix: expose core's
`is_cite_key_char`, a key-at-position function and a key-to-offset lookup; delete three scanners.

---

## Part C. Preview invalidation: derive it, do not enumerate it

Measured by an invalidation matrix over six save styles (in place, gedit rename-over, `sed -i`,
vim-rename, JetBrains safe-write, delete+create) against a `build` oracle.

- **C1. Regression from 09-02 #17.** Since every rename-over save is "structural", the structural
  path rediscovers and reseeds the front-matter digest before `front_matter_moved` runs
  (`serve_site/mod.rs:1734-1757`). A `title:`/`date:` edit saved atomically, or a `git checkout`,
  leaves open listing pages stale, and **a fresh GET serves the same stale body**. [re-run]

  ```sh
  # project with index.tmd (listing: contents: posts) and posts/a.tmd; preview, GET both pages
  sed -i 's/Old Title/New Title/' posts/a.tmd; sleep 1.5
  curl -s localhost:PORT/index.html | grep -o 'Old Title'   # stale; an in-place write updates it
  ```
- **C2.** An in-place edit of a chapter's leading `# H1` never rediscovers: `front_matter_digest`
  hashes only the `---` block while discovery also reads the H1. The drawer, pager and **every
  chapter's section, figure and equation numbers** stay wrong, on fresh GETs too.
- **C3.** Renaming or moving a directory (folder-per-post blogs) 404s the new URL and **orphans
  the watch until restart**: notify drops a moved directory's watch, and `relevant_path` requires
  a file extension.
- **C4.** A shared `bibliography:` declared before the file exists is never picked up.
- **C5.** Open tabs never receive chrome changes (drawer and pager sit outside `#tali-root`); the
  comment at `:1745-1756` claims they do.
- **C6-C8 (confirm leads).** `.md` partial anchor renumber (`touches_source` checks `.tmd` only);
  image add/replace never re-renders and stale width/height survive reload; `python:` edits never
  reach the kernel, even through Restart kernel.
- **C9.** Every atomic save pays a full rediscover: 195 ms vs 369 ms at 221 pages.
- Also: a front-matter save fans out to every page ever visited (453 ms to 2544 ms at 221 pages);
  CSS/JS/JSON are watched and then dropped; `.venv` is watched (2,334 of 2,372 watched directories
  in the real tech-blog).

Fix: decide "structural" by whether the page set actually changed (compare `path.exists()` with
the front-matter record), not by event kind; this retires `may_change_page_set` and fixes C1, C9
and the atomic-save cost in Part F. Make the digest cover what discovery reads (C2). Record every
path a render reads, including failed reads, and invalidate on those (C4, C6, C7; retires
`includes::dependencies`, `resource_dependencies` and the shared-bib special case). After a
rediscover, reload exactly the tabs whose chrome HTML changed (C5).

---

## Part D. Live DOM patching (browser, 85-step fuzzer)

- **D1.** A block typed right after an HTML comment or a closing raw-HTML tag lands **above the
  title**: those emit no block id, the insert's `after_id` is absent from the DOM, and
  `client.js:1017-1019` falls back to `root.prepend`. Editing a raw wrapper's opening line deletes
  its contents from the preview. [re-run: protocol half] Fix: the server sends `full_render` when an
  op anchors on a zero-root or unclosed-root block; the client re-mounts on a missing anchor or
  target instead of prepending or ignoring.
- **D2.** Updating a `:::` container can leave two elements with one id; a later op in the burst
  edits the wrong one (`diff.rs:40` sees top-level ids only; `elById` takes the first descendant).
  The fuzzer's only failure class. Fix: resolve `:scope > [data-block-id]` first.
- **D3.** An async `{js}` run in flight when its block is replaced keeps publishing: stale value
  wins permanently, or the visible slider is detached. Fix: a `disposed` flag, about three lines.
- **D4.** A consumer above its producer shows `undefined` on load **and in the built page**, while
  the live preview shows the right value; gate clean. Fresh cells run in document order, not
  `graph.order` (`tali-js.js:633-641`).
- **D5.** Any line-shifting edit above a `:::` container re-mounts it, resetting sliders and
  `<details>` inside (`diff.rs:115-141`: multi-sourcepos blocks always take a full Update).
- **D6.** One throw in `afterChange` (toc-spy on a `%` id) stops copy buttons, diagrams and `{js}`
  for the rest of the session. Wrap each step.
- **D7.** `data-section-end` has no reader and makes edits re-emit and flash untouched headings.
  Delete it (with its pins, same commit).
- Also: input default edits and renames do not re-run consumers; `<script>` in cell HTML output
  or raw blocks never runs live; a ticker cell starves the error badge; mermaid parse errors are
  unhandled rejections.

---

## Part E. Kernel output protocol and cache (real ipykernel 7.3.0)

- **E1.** CRLF line endings erase stream text: `csv.writer` output renders as blank lines and is
  cached (`kernel.rs:1304-1320` treats the `\r` of `\r\n` as "clear line"). [re-run]
- **E2.** A `tqdm` loop is SIGINTed at 4096 redraws (about 7 minutes at its default rate): the
  flood caps count raw messages and bytes, not the collapsed stream. Downstream cells run on
  partial state. Contradicts "a long job that reports progress runs to completion"
  (`docs/guide/using/code.tmd:34-37`, CLAUDE.md).
- **E3.** `clear_output(wait=True)` is ignored: every animation frame is published, and past about
  260 plot frames the rich cap interrupts training. [re-run]
- **E4.** A failing `#| include: false` cell is silent everywhere (page, console, `--strict`,
  JSON) and disables caching for the rest of the document (`exec.rs:645`).
- **E5.** `update_display` is dropped; `display(Markdown/Latex/JSON)` publishes
  `<IPython.core.display.Markdown object>`; `Image(width=)` is ignored.
- **E6.** Restart kernel waits behind every downstream cell; after an ignored interrupt every later
  cell is sent to the wedged kernel (about 610 s each at defaults). Fix both by killing, not
  interrupting, a kernel that is being discarded.
- **E7.** The edited cell never shows its running badge or live output (only unedited downstream
  cells stream).
- Also: thread output lands in whichever cell is running and defeats the silence cap; library
  warnings publish the author's absolute home path; one undecodable iopub message discards the
  cell's output; the crashing cell is not identified; OSC-8 links are garbled; false
  "different package set" warnings repeat (CONFIRMS freeze.rs:148, packages.rs:57); a pandas
  `.plot()` with no earlier matplotlib import is unreadable in dark mode.

---

## Part F. Performance at scale (10 to 500 page books; real projects too)

Real projects are fine: guide and tech-blog saves land in 88 to 111 ms, and 80 ms of that is a
fixed sleep.

- **F1. Math cache cliff.** `MathCache` is FIFO with `CACHE_CAP = 8192`, and a hit does not refresh
  position. Whole-project passes read math cyclically, so past 8,192 distinct expressions the hit
  rate is 0: a preview body save goes from 128 ms to **10.7 s**, an LSP publish takes 19.6 s, a
  cold build 7.45 s to 33.4 s. The author's projects are far below the cap (tech-blog 181).
  Fix: the harvest keeps only `xref_numbers` and need not call KaTeX; stop inserting when full.
- **F2. The LSP rediscovers the whole project on every save** for data it never reads
  (`lsp_project.rs:137-150` validates by every page's mtime; discovery always runs the harvest and
  the search rebuild). 309 ms at 500 pages; a hover sent 130 ms after a save waits 176 ms. Fix: a
  registry-only discovery for the LSP (about 10 ms at 500 pages).
- **F3.** The fixed 80 ms debounce (`serve_site/mod.rs:1640`) is 80-92% of every save on real
  projects; measured event bursts finish within about 1 ms for every save style. The published
  "Warm edit 2.6 ms" table (`choosing.tmd:56-64`) never mentions the floor.
- **F4.** Whole-project passes use about 2 of 16 cores: every render spawns a detached 256 MB-stack
  thread (518 `clone3` per save at 500 pages), and the harvest runs `cite::process`,
  `dedup_element_ids` and the image annotator (74% of render samples) only to keep `xref_numbers`.
  The same churn is the likely cause of RSS growing 87 to 272 MB over 150 heading edits (glibc
  arena fragmentation, not a leak).
- **F5.** An anchor-moving save costs two whole-project passes; `tools/live-edit-bench` times one,
  omits the sleep, the edited page's own build, rediscovery and the LSP, and never exceeds 17
  pages. Add those rows before quoting a number.
- Scaling: body saves cross 100 ms at about 103 pages, heading/title/atomic saves at 37 to 45.

---

## Part G. BibTeX against real exports (Google Scholar, DBLP, arXiv, Zotero/BBT, Mendeley)

- **G1.** A name whose first token starts with a braced accent (how every exporter writes
  accents) is published unformatted, as a corporate author: `Öztürk, Ayşe`, `Ørsted, Hans`
  (`author.rs:81` tests only the first character). [re-run]
- **G2.** "First von Last" particles become initials: `Laurens van der Maaten` publishes
  `L. V. D. Maaten` (DBLP and arXiv use this order for every name). [re-run]
- **G3.** An entry missing its closing brace silently swallows the next one, and the diagnostic
  offers a **machine-applicable fix that cites a different paper** (`@smith2020` to `@smith2019`).
  Across the shared bibliography, an unclosed last entry in `a.bib` eats `b.bib`'s first
  (`parse.rs:80-86`). [re-run]
- **G4.** A correct citation in a figure caption renders correctly but fails the gate with an
  error (`diagnostics/bibliography.rs:109` substring-scans finished HTML, attributes included).
  [re-run]
- **G5.** Unknown LaTeX control words are deleted: `{$\alpha$}-Synuclein` publishes
  `$$-Synuclein`, `{\TeX}book` publishes `book`, `$O(n \log n)$` loses `\log`.
- **G6.** BibLaTeX exports lose year and journal on every article (`date`, `journaltitle` unread);
  `doi`, `editor`, `crossref`, `school` are dropped; IEEE punctuation doubles after `?`.
- Also: false "no `bibliography:` is declared" on pages using the shared file; a Latin-1 `.bib`
  is reported as "not found"; text before `@` in a group is silently dropped and emails become
  citations; `&` in a locator truncates it (CONFIRMS cite/render.rs:442, :456); keys with `&` or
  `'` are stored truncated with no fields; hyphenated given names lose an initial.

---

## Part H. Security (by execution; realistic threat is a cloned project or a co-author's PR)

A2 and A4 are above. Also:

- A loose-document preview roots its server at the document's parent directory, so
  `preview ~/scratch.tmd` serves the whole home directory (dotfiles included, every HTTP method)
  to any local process. Fix: 404 any path with a `.`/`_` component, as the build already treats
  them.
- `/search-index.js` (drafts included) is a cross-site-readable script. Chrome's Local Network
  Access blocks it by default; Firefox untested. Fix: refuse cross-site `Sec-Fetch-Site` for
  non-navigation requests (about 5 lines).
- A div class or callout kind injects attributes: `::: {.callout-note"onclick="alert(1)}`
  produces a real `onclick` (`divs.rs:558`, `:562`, `:588`), the exact shape of 09-02 P0 #2 at a
  sibling site. Also unescaped: nav/footer hrefs, favicon, filename-derived hrefs and
  `window.TALIESIN_PAGE_URL`, `edition`, Atom dates. `{js}` source containing `</SCRIPT>` or
  `<!--<script>` escapes or swallows its script element. [re-run: div class]
- The takeover SIGTERM can target any process running this binary (for example the author's own
  `taliesin lsp`), because the pid is never tied to the listening socket (code-read).
- Include expansion is unbounded: a diamond chain doubles per level (16 files at depth 15: 1.3 s,
  82 MB, "no problems found"). [re-run] Seed the cycle stack with the primary document too.
- Sound: traversal, the Host and Origin guards, `--no-exec` gating, `safe_url`, include and bib
  containment.

---

## Part I. A stranger's first hour (clean `$HOME`, release asset, docs followed literally)

1. The README links six raw `.tmd` files, which GitHub shows as plain text, and never links
   guide.taliesin.sh; `taliesin --help` points at the GitHub repo. [re-run]
2. The first screen says "Built for one author's workflow", never says what `.tmd` is, and never
   names Quarto, MyST or Jupyter Book, though the differentiators are real.
3. `python3 -m pip install ipykernel` (README, troubleshooting, and doctor's own fix line) fails
   on Ubuntu 24.04, Debian 12 and Homebrew Python (PEP 668). The venv route works, and a project
   `.venv` is auto-discovered, which no doc says.
4. The install block fails on a fresh account (`~/.local/bin` does not exist).
5. The blog recipe's listing lands at the bottom of the page: an empty `::: {#recent}` emits no
   element and the fallback appends silently.
6. "Self-contained" single-file builds leave local images as relative paths (see A1).
7. GitHub Pages from `docs/` needs `.nojekyll` for `_assets/`; nothing says so.
8. The hosted-builder advice is wrong: without a kernel `build` exits 1, and a committed
   `_freeze/` does not help because the key includes the interpreter.
9. A venv created while the preview runs is not picked up, though troubleshooting says no restart
   is needed.
10. `type: grid` renders the same as `list` (its CSS was removed 2026-08-15).
11. The companion is build-from-source only (no Marketplace, Open VSX or `.vsix` in releases);
    the Neovim snippet lacks the filetype registration.

---

## Part J. Process, instructions and registers

- **CLAUDE.md** says "pre-push is the only gate that runs automatically" and that CI triggers skip
  while private; the repo is public and CI runs on every push. `stale_docs.rs:179-188` enforces
  the false premise. The rebuild rule omits `web-client/*.js` (also `include_str!`-compiled). The
  interpreter order is wrong (a project `.venv` beats `TALIESIN_PYTHON`). `emit.rs` "code
  line-wrapping" does not exist. CLAUDE.md regrew from 3,413 to 4,237 words since the 08-17 trim,
  mostly narrative; ~1,200 words can go with every rule kept.
- **`.claude/commands/preview.md`** runs `taliesin serve` (unknown verb) on `docs/index.tmd` (does
  not exist). [re-run]
- **`.claude/agents/corpus-verifier.md`**, the agent that answers "do the tests pass", runs only
  `cargo test -p taliesin-core`: the whole server suite and `gates.sh` are skipped. [re-run]
  `tali-explorer` and `rust-reviewer` and the `audit-tali` workflow still describe decks and the
  reversed corpus-pin rule; `settings.json` allowlists cut verbs.
- **`build.rs:118`** tells users "a real print/PDF track is planned (ROADMAP Pillar IV)"; the
  roadmap marks it CUT, and `strict_robustness.rs:339` pins the string. [re-run]
- **DETECTION-DEBT.md**: 12 of 27 rows describe deleted subjects (deck.js, image_opt.rs, sidebar
  tree, paste provider, `--host`, `mounts:`, ...), 4 scores are wrong. [re-run: 7 of 7 sampled
  subjects gone] Fold its ~9 live rows into LESSONS.md "What the test net structurally cannot see"
  and delete the file.
- **backlog.md** describes a working-copy layout that no longer exists (item 100), a `RETIRED_KEYS`
  register that was cut, four drift gates (three exist), and points at an 08-13 queue that is
  spent. ROADMAP.md says "STILL PAUSED" while CLAUDE.md says "unpaused".
- **notes/**: ~44k lines; no test reads it; most dated files are referenced only by an index row
  (and six recent rounds are missing from that index).
- **Follow-through.** 0 of 143 leads from 09-02 were acted on in 22 days; cost per confirmed
  finding rose about 4.5x between 08-13 and 09-02. Recommendation: no new whole-repo round until
  this file's Part A to C land; after that, diff-scoped review by default; unvetted leads expire
  after 14 days.

---

## Part K. Cuts available (lean directive)

~900 to 1,300 removable product lines (about 2% of `src`). The large files are large mainly from
inline tests (40-60% of lsp.rs, exec.rs, site/mod.rs, lint.rs) and comments (about a third of
implementation lines), not residue.

- The vocab JSON layer: an interchange format whose only consumer is in-process; two silent-Null
  bugs already (`frontmatterValues` key gone, so front-matter value completion never answers).
  Expose the typed tables instead (~150-200 lines).
- The five page pipelines and the second diagnostic type (B4; ~150-250 lines).
- taliesin-core's page API shaped for cut library consumers: seven layered page entry points,
  ~120 test-only public functions (~100-130 lines plus mechanical test re-points).
- `SiteApp`/`Project` two-level struct, residue of `mounts:` (~50-70).
- `Executor::langs` as a map: the recorded reason no longer holds (the freeze key is a pinned
  string). `FreezeCache::packages` must stay a map, but because it is on-disk format; re-record
  that reason.
- `PageIncludes` (three slots from cut injection keys, one never written), `run_site_build`
  layering and `BuildResult`'s one variant, `CLIENT_LANGS` with one entry, the protocol's
  `rewrite_html` parameter and wire `page` field, four copies of the sourcepos parser,
  `is_local_ref` twins, the runtime base64 encoder for a compile-time constant.
- Delete: `data-section-end` (D7), the dead `headings.rs` validator, `code_scripts()`,
  `type: grid` (or restore its CSS), the `{-}` chapter recognition the renderer never honoured,
  the `hero.actions` `class:` read that the lint already calls unknown.

---

## Calibration: the 09-02 leads

| | Count |
|---|---|
| Checked by the calibration lens | 86 |
| Still real | 83 |
| Partly real | 1 (anchors.rs:46: comrak does not percent-encode; a hand-written `%C3%BC` link is still a false error) |
| Refuted | 1 (base.css:96, skip link overflow) |
| Already fixed | 0 |
| Reader-visible / author-visible / maintainer-only | 21% / 27% / 51% |

The remaining 57 leads were owned by the lens auditors; nearly all were confirmed there, one
(serve/mod.rs:248, arbitrary-pid SIGTERM) was refuted.

## Checked and found sound (do not re-audit)

The 09-02 fixes hold (#4 live on gallery, #18, #22, #16, #17's page-set half, P0 #2's two sites).
Traversal, Host/Origin guards, `--no-exec` gating, `safe_url`, include/bib containment.
Copy buttons and mermaid SVGs exactly once through every op kind; reconnects and save bursts;
`{js}` teardown releases timers. Cold replay equals warm display; interpreters never cross in
`_freeze`; the silence and wall-clock caps fire; `input()` and `sys.exit()` behave. Cold build is
linear to 500 pages with sublinear RSS. Search-index anchors: 0 missing, 0 duplicates across six
builds; builds byte-deterministic. Duplicate YAML keys and tabs are located errors; `yes`/`no` are
handled. Accent macros, case protection, `@string`, paren entries, corporate authors in BibTeX.
Intrinsic image dimensions match the files for png/jpg/gif/webp.

## Not covered

Real-device mobile, Safari, Windows, macOS watchers. Real editor saves (sequences were emulated;
`sed -i` verified). ipywidgets comms (not installed). The VS Code companion webview and
click-to-source's last hop. Compile time and test-suite runtime (not measured to avoid disturbing
the performance lens). The highlight cache likely has the same cyclic-eviction cliff as F1
(code-read only). Real deploys to GitHub Pages or Cloudflare.

## Suggested sequence

1. Part A (A1 to A4, each small), then tag 1.1.0.
2. B1 (front matter: one reader; about ten findings from three lenses) and C1 (the regression).
3. B4 (one page pass and diagnostic set): makes the gates agree, which every later fix leans on.
4. B2 (the line classifier) and B3 (walker decoding): the two largest subtractions.
5. Part I items 1 to 5: an afternoon, and the only part that addresses the user count.
6. Part J: fix `/preview`, `corpus-verifier`, the CLAUDE.md CI paragraph and its gate, then trim.
7. Everything else as the areas are next touched, verifying each fix by mutation.
