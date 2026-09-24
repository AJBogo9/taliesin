# Changelog

All notable changes to Taliesin are recorded here. From 1.0 this project follows
[semantic versioning](https://semver.org/): a breaking change to the project's
invariants (content-hash block model, click-to-source, single editing surface, HTML-only
output) or to the CLI's six verbs needs a major version. Before 1.0 the policy was looser
and minor versions carried breaking changes; the 0.x entries below were written under it.

## [Unreleased]

### Fixed

- **An author's `404.tmd` now works at any depth.** A host serves `404.html` for every
  unknown path, but the page was built with depth-relative URLs like any root page, so a
  mistyped nested URL (`/a/b/zz` on the tech blog) resolved the stylesheet, script, font,
  favicon, logo, navbar, footer and search against `/a/b/` and got a 404 for each. The
  built page's URLs are now root-absolute, as the generated 404's already were. Every
  other page keeps relative URLs, so a build still opens from disk.
- **A book chapter's prev/next pager lines up with the text again.** It sat outside the
  reading grid, so on a wide screen the previous-chapter link was at the window's left edge
  and the next-chapter link at its right edge. Both links and the rule above them now span
  exactly the text column, including on chapters with margin notes.
- **A single-file build never replaces a different file beside its output.** Two posts built
  into one folder overwrote each other's figures; a clash is now a located error.
- **A site build never publishes what a symlink reaches under a private path.** `vendor ->
  ../.git` shipped `.git/config` and every object; page discovery follows the same rule.
- **The freeze cache no longer stores what a warm kernel re-ran**, so `build --strict` can no
  longer publish a value that a fresh kernel raises `NameError` on.
- **The preview's port takeover** reads at most 64 KiB of a port holder's reply (an endless
  reply drove it past 1 GB) and signals only the process listening on the probed port.
- **The preview refuses a cross-site load that is not a navigation**: another site could read
  every page's text, drafts included, through `/search-index.js`. It also serves only what a
  build would publish (never a dotfile), answers only GET and HEAD, reads `Host` and `Origin`
  as a host plus an optional port, and shows a static `.html` or `.txt` instead of
  downloading it.
- **Front matter is read as YAML everywhere, split by one rule.** Quotes, comments, block
  scalars and wrapped values no longer publish literally, and a `--- ` line no longer
  publishes the block as a heading. A trailing comment on a `#|` cell option is a comment.
- **A value the tool cannot use is reported, not dropped**: a non-boolean `toc:`, `cache:` or
  `draft:` (an unreadable `draft:` holds the page back), a `listing:` value, a `_site.yml`
  number where text belongs, a nav entry with no `href:`, an unknown icon, a BOM.
- **Posts sort, feed and stamp by calendar date**, and a file with lone-CR line endings is
  read like any other (its `draft: true` used to be ignored).
- **What is code is decided by the renderer's parser in every line pass**: a paragraph that
  opens with inline code no longer hides the callouts below it, and a commented-out
  `{{< include >}}` is no longer expanded.
- **Includes and divs report what they used to drop in silence**: an include not alone on its
  line, a partial ending inside an open fence, a `:::` close that closes nothing, a `:::` in
  a list item. Expansion stops at 1,000 includes or 4 MiB, and a page including itself is a
  cycle. In a `layout-ncol` grid each column keeps its own list.
- **Names with `&`, `%20`, `#`, `?` or `%` resolve as the browser resolves them**: no false
  "not found", no image left out of a portable folder, no broken page link.
- **Markup cannot leak through text or values.** Prose that shows `<span class="katex">` no
  longer ships KaTeX; a quote in a div class, callout kind, `_site.yml` value or file name
  cannot write an attribute; `javascript:` is refused in nav, hero and logo hrefs; a `{js}`
  cell's source cannot close its script element; the bib `edition` field is escaped.
- **An HTML comment holding an apostrophe or `>`** no longer corrupts the search text and
  citations after it, and a heading written with character references is slugged from the
  characters it shows.
- **The asset gate and the build agree on what ships.** The gate reports an image or link the
  build cannot publish; a referenced file in an `_`-prefixed folder is deployed; every
  `srcset` candidate is checked and shipped; front-matter `image:` and `_site.yml` `logo:`
  and `favicon:` are checked; draft-only folders and editor residue stay out of a site; a
  portable folder that cannot hold a referenced file is an error.
- **A site build publishes the figures its cells write**, and such a file is no longer
  reported missing on the first build.
- **Book chapters**: `./a.tmd` is built and linked as `a.html`, a chapter listed twice is
  reported, a chapter's `image:` is its unfurl image, and a book no longer judges `nav:`
  links it never renders.
- **An EXIF-rotated, misnamed or percent-encoded image reserves the box it really takes.**
- **A preview of a loose document** resolves includes and bibliographies as its build does.
- **BibTeX from Google Scholar, DBLP, arXiv, Zotero and Mendeley.** Braced accents, von
  particles, Jr parts, hyphenated given names, ties and `AND` split as BibTeX splits them;
  unknown LaTeX commands and math are kept; TeX quotes, dashes and ties print as typeset;
  `doi`, `editor`, `crossref`, `school` and BibLaTeX's `journaltitle`, `date` and `location`
  are read; IEEE punctuation follows the title's own mark; an unclosed entry, an uncitable
  key, an undefined `@string` and a file that is not UTF-8 are reported.
- **Citations**: a locator with `&`, `<` or emphasis keeps its text, a bracket cites only
  when every item starts with `@`, a citation in a caption or in code is not a "bare key",
  `[@key: note]` reads the colon as the separator, and a page inheriting the project
  bibliography is not told none is declared.
- **The live preview keeps the page right after an edit.** An edit touching raw HTML
  re-mounts instead of misplacing blocks; a line shift above a callout or grid patches its
  positions (sliders and open `<details>` keep their state); editing a section's last block
  no longer flashes its headings; scripts an edit brings in run; one failing step no longer
  stops later enhancement; a busy page no longer hides the cell error badge.
- **`{js}` cells**: a consumer above its producer gets the value on load and in the build,
  editing an input's default or removing a control re-runs what reads it, and a superseded
  async run publishes nothing.
- **Preview saves reach every page they affect.** A save is judged by what it changed, so an
  atomic or `git checkout` save of a post's title updates its listing; a page rebuilds on
  every file its render read (a `.bib` created later, a resized image); a `.md` partial
  moving an anchor renumbers the pages citing it; a renamed folder is served and watched;
  exactly the tabs whose chrome moved reload; a tab on the 404 page returns once its page is
  back; a `python:` edited in `_site.yml` or a new `.venv` reaches the kernel.
- **Cell output publishes what the cell showed.** `\r\n` is a newline; a progress bar longer
  than the output caps runs to completion; `clear_output` keeps the last frame; display
  handles update in place; Markdown, LaTeX, JSON and sized images publish as such; OSC 8
  links keep their text; warning and traceback paths publish without the home directory.
- **Cell failures are reported where they happen.** A cell that prints error markup is not a
  failure; a failing `include: false` cell is a located error; the cell that crashed the
  kernel is named; a cell stopped by an output cap says so; one undecodable kernel message
  costs that message, not the cell's output.
- **Kernels stop when they should.** Restart kernel kills the requesting page's own kernel,
  a cell that ignores its interrupt is stopped, and a full-speed output flood in
  `build <file>` stops at its cap (it used to hang).
- **A background thread's output stays with the cell that started it**, a figure drawn
  through pandas follows the page theme, and the package-digest warning no longer fires when
  interpreters alternate or after Restart kernel.
- **An edited cell shows its running badge and live output in the preview.**
- **Captions**: an executed `fig-cap` or `tbl-cap` renders Markdown and math like every other
  caption, a captionless figure has no dangling colon, a figure keeps its image's title,
  wrapped alt text keeps its words apart, a hidden cell's `define(...)` still reaches `{js}`
  cells, and a cell image nothing describes draws a warning.
- **Every verb runs the same page pass.** `--check-only <file>` fails what `build <file>`
  fails; a page built alone renders its `hero:`; the dev menu shows an error as an error;
  project diagnostics are located and counted by `--strict` and `--format json`; a
  diagnostic in an included file names the file; a nested `_site.yml` is reported.
- **Section numbers agree.** A heading and every link to it read the same number, an
  `.unnumbered` section takes none, chapters and cross-page sections are labelled by the
  text their heading shows, an anchor in a quote, list item or footnote is no target, and
  the TOC lists a heading inside a div.
- **Cmd-K search** finds code as typed, forgives a typo beside punctuation, indexes executed
  figure and table captions, leaves out commented-out headings and diagram source, opens a
  closed `<details>` around a hit, and searches the same index in a single-file build as
  in the preview. The palette is a named dialog.
- **Accessibility and print**: the active TOC entry carries `aria-current`, navbar icon links
  are named, highlights show under reduced motion, a diagram prints light from a dark page,
  and the navbar, top bar and back link stay off paper.
- **The language server** reads citations with the render's grammar and the shared
  bibliography, reads block structure as the render does, jumps to the anchor a page
  defines, completes front-matter values and `layout-ncol`, and offers a cell language on a
  four-backtick fence. The VS Code companion starts one server at a time.
- **The CLI reads every verb's arguments by one grammar** (`--flag=value`, a suggestion for
  an unknown flag, an extra argument refused). `build notes.md` suggests renaming to `.tmd`;
  `preview` says when the document is not a page and prints its startup notes after the
  banner; `doctor` recommends a project `.venv` where a system `pip install` is refused;
  `build` prints `built` only for a build that succeeded; `build doc.pdf` names the
  browser's Print to PDF instead of a planned print track; `--help` points at the User
  Guide.
- **A listing lands in its empty `::: {#id}` block**; a listing whose `id:` matches nothing
  warns, and a page built alone leaves its listing out with a note.

### Added

- **Releases attach the VS Code companion as a `.vsix`.**

### Changed

- **A website post's link back to its listing ("← Blog") now sits above the title**, in
  the size and colour of the date line, instead of closing the page. At the bottom it had
  come loose from the text column: on a wide screen it sat at the window's left edge under
  a rule of its own, just above the footer's rule. At the top it lines up with the title,
  shows which listing a post belongs to as soon as the page opens, and the page ends with
  the text. Books keep their prev/next pager at the end of each chapter.

- **A multi-page site build prefetches the page a reader is about to open.** Every page of a
  `build <dir>` carries a speculation-rules `prefetch` for same-origin `.html` links, so
  hovering a link (or starting a tap) fetches that page and the click finds it already
  downloaded. Measured on the tech blog on 2026-09-23, a click spent 340 to 400 ms waiting on
  the document and 83 to 150 ms rendering it; behind a local server that copies Cloudflare
  Pages' `.html` redirects, a hovered link then painted 60 to 100 ms after the click instead
  of 270 to 320 ms. Prefetch only, never prerender, so no page's `{js}` cells
  run for a hover. The live preview and single-file builds do not carry it, and browsers
  without speculation rules ignore it.

- **A save reaches the open tab once its events stop** (15 ms of quiet) instead of after a
  fixed 80 ms, and the preview rebuilds only the pages a tab is watching.

- **Whole-project passes do less.** The cross-reference harvest typesets no math and
  highlights no code, renders run on parked worker threads, the scans run across cores, the
  search index is rebuilt after the open pages, and the language server holds the page
  registry instead of rendering the project on every save. A preview's memory no longer grows
  with each save that moves an anchor.

- **Only a `{#fig-…}` label makes a numbered figure.** A standalone image with alt text stays
  an image, and a figure's image carries `alt=""` beside its caption.

### Removed

- **`listing: type: grid`**, which rendered the same as `list` since 2026-08-15.
- **The `ojs` highlighting alias and the bare `::: classname` div form**: write a `js`
  fence for a highlighted listing and `::: {.classname}` for a div.
- **A hero action's `class:`**, which was reported as unknown yet still honoured; use
  `primary:`.

## [1.0.1] - 2026-08-21

No code changed. This release exists because v1.0.0's Linux binary could not run on most
Linux machines, and because nothing in the project pointed a reader at the binaries.

### Fixed

- **The Linux release binary is now statically linked against musl**
  (`x86_64-unknown-linux-musl`, replacing `x86_64-unknown-linux-gnu`). A gnu build inherits
  the runner image's glibc as a floor; `ubuntu-latest` had become 24.04, so the v1.0.0 asset
  required `GLIBC_2.39` and failed to start on Ubuntu 22.04, Debian 12, RHEL 9 and Amazon
  Linux 2023. It went unseen because the only machine that had ever run a released binary
  was new enough to load it. A static binary has no glibc requirement.

### Changed

- **The install instructions lead with the download.** README, the User Guide's getting
  started page and the marketing site all opened by telling a reader to clone the repository
  and compile 229 crates, and none mentioned the three prebuilt tarballs with checksums on
  the releases page. `readme_install_command_names_the_current_version` now fails the suite if
  the version in that command drifts from the workspace version.

## [1.0.0] - 2026-08-20

This is the first public release. 1.0 marks the feature set as final for this tool's one
use case. Alongside the scope-closing cuts below, this release also ships a full visual
redesign and a new public gallery site.

### Added

- **A full visual redesign** ("Instrument"): the tool now ships its own typography (Literata
  for prose, JetBrains Mono for code and labels, replacing system fonts) and colour (two
  palettes designed rather than inverted from each other, its own syntax-highlighting
  palette replacing a borrowed one, every text colour shipped with a computed WCAG contrast
  ratio). Reading measure, heading scale and spacing scale are all derived from one system
  instead of copied per component, and the favicon now follows the reader's device like
  every other painted surface.
- **A public gallery site**, its own project and domain: five self-contained one-page demos
  (a gradient-descent explainer, a parametric meshed-gears model, an executed data report,
  an API-documentation craft piece, and a molecules demo), plus a `listing:` index with
  theme-aware card thumbnails.

### Changed

- **The project is public**, and version 1.0 closes the scope. Feature requests are closed
  from here; bug reports are welcome. See "Project status" in `README.md`.
- **The CLI is six subcommands**: `preview`, `build`, `init`, `doctor`, `lsp`, `help`.
- **The four public sites deploy separately** instead of composing into one domain's
  subpaths: the marketing site, both docs books, and the gallery each build, preview and
  publish as their own project on their own domain, linking to each other by absolute URL.

### Removed

Continuing the 2026-08 reduction campaign:

- The `lang:` and `csl:` front-matter keys, `page-layout: full`, and link attribute blocks
  (`[text](url){.class}`).
- The `theme:` key and all author theme control. Both palettes always ship and the reader's
  device selects one at paint.
- The `taliesin new` verb: `init` now scaffolds the same dated starter post directly, which
  is why the CLI above is six subcommands rather than the seven `0.3.0` shipped.
- The seven retired theorem cross-reference prefixes (already unable to resolve to
  anything), and the `head:` and `external-prefixes:` site config keys.
- The retirement registers that echoed another tool's spelling for a withdrawn key, verb,
  or class. An unrecognized one now gets a plain did-you-mean or "unknown", the same
  treatment a typo already got.
- The reader-facing code-download aside on pages with code cells, which offered every
  cell's source as a script; the source was already on the page and in view-source.
- The missing-local-video lint and the uncited-entry lint.
- Five VS Code companion features: the first-kernel-failure doctor hint, the build/check
  tasks and their Problems-panel matchers, the Diagnose Setup command, the Get Started
  walkthrough, and the bundled `_site.yml` schema copy. The terminal path replaces the task
  provider: every located diagnostic line in the integrated terminal is clickable.

### Fixed

- `README.md` no longer advertises constructs the tool deleted, and a test now keeps it
  that way.

## [0.3.0] - 2026-08-10

The scope-reduction release: eighteen CLI verbs become seven, and the document
feature set is cut roughly in half. This release is breaking. Every retired verb
still answers with a line saying what replaced it.

### Changed

- **Relicensed from MIT to AGPL-3.0-only.** The Affero copyleft closes the
  "SaaS loophole" so a modified network deployment must share its source. As the
  sole copyright holder, the author reserves the right to offer Taliesin under
  other terms (a proprietary hosted service or a commercial license); the
  dependency license policy in `deny.toml` stays permissive-only to keep that
  option open. The VS Code editor companion under `editor/vscode` remains MIT.

- **One preview per project.** Re-running `taliesin preview` on a project that
  is already being previewed now replaces that server rather than binding the
  next free port. Previews answer a `/__taliesin` identity endpoint (canonical
  root, pid, version), so a launch can tell its own project's preview from an
  unrelated server holding the port; an unrelated holder still falls back to the
  next port, so two projects can be previewed side by side. Previously, every
  surplus preview kept its own file watcher and kernel subtree re-executing the
  same sources, on a port nobody was looking at. Any local user can bind a
  loopback port, so the pid a holder reports is treated as untrusted:
  non-positive values (`kill(-1, ...)` reaches every process the user owns) are
  rejected outright, and on Linux the pid is checked against `/proc/<pid>/exe`
  before it is signalled, so answering the probe cannot get an unrelated process
  terminated.

### Removed

- **Eighteen CLI subcommands became seven**, across a thirteen-wave scope-reduction
  campaign (2026-08-08 to 2026-08-09) that also cut the document feature set roughly in
  half. What ships is `preview`, `build`, `init`, `new`, `doctor`, `lsp` and `help`. The
  goal was a surface small enough to polish before release; features come back when real
  users need them.

  Every retired verb is still recognized when typed exactly. `taliesin <verb>` answers
  with the line below rather than a did-you-mean over the remaining verbs, because a wrong
  suggestion is worse than none when the person typing it is following an older page:

  | Removed | What replaces it |
  |---|---|
  | `check` | `build <file\|dir> --check-only` lints without writing, and takes `--strict` and `--format json` the same way |
  | `render` | `build <file.tmd> --stdout --no-exec` writes the same page to stdout |
  | `run` | `preview <file.tmd>` executes the same cells against the same warm kernel and writes the same `_freeze/`, so a later `build` still replays without one |
  | `publish` | `build <dir> --out <dir>` writes a plain folder any static host serves (Netlify, GitHub Pages, Cloudflare Pages, rsync) |
  | `serve` | use `preview` |
  | `dev` | use `preview` |
  | `blocks` | `taliesin lsp` publishes the block model now |
  | `symbols` | `taliesin lsp` completes cross-reference targets after `@` |
  | `vocab` | `taliesin lsp` serves the same vocabulary as completions |
  | `map` | nothing on the CLI; `taliesin lsp` answers `taliesin/siteMap` for your editor |
  | `schema` | nothing on the CLI; the VS Code companion bundles the `_site.yml` schema |
  | `features` | `build <dir> --check-only --format json` is the machine surface |
  | `mcp` | `build <dir> --check-only --format json`, run from your agent |
  | `skim` | nothing; read the `.tmd` source |
  | `read` | nothing; read the `.tmd` source |
  | `pdf` | nothing; print the built HTML to PDF from your browser |
  | `completions` | nothing; type the subcommand out, or bind your own shell alias |

  The table is the same data the binary answers from (`RETIRED_COMMANDS` in
  `crates/server/src/main.rs`), so each retired verb gets the same sentence here and at
  the prompt.

- **The `{r}` cell language, and the R kernel behind it.** `{python}` is the only
  executable kernel language. `TALIESIN_R` and `TALIESIN_REQUIRE_R` are gone with it.

### Fixed

- **The symlink containment check no longer fails open on a bare filename.**
  `safe_join`'s canonical-path check was skipped whenever the containment root
  could not be canonicalized, and `taliesin build index.tmd` (no directory
  component) hit that case: the doc's base dir is the empty path,
  `std::path::absolute("")` is an error, so the base stayed relative and the root
  came out empty. An in-tree symlink pointing outside the project was then read
  and inlined verbatim into the page. The same document was correctly refused when
  invoked as `./index.tmd` or as a site, so the working directory decided whether
  an out-of-tree file leaked. The empty path now resolves against the cwd, and a
  boundary that cannot be canonicalized refuses instead of falling through.

- **A symlinked resource may point anywhere inside the repository.** The symlink
  check reused the *lexical* containment root, so a book whose `_site.yml` bounds
  it to `book/` could not symlink `references.bib` to the `paper/references.bib`
  beside it in the same checkout: the canonical target left `book/` and the file
  was refused. Symlinks are now bounded by the enclosing repository (nearest
  `.git`), falling back to the lexical root outside a checkout. The lexical rule is
  unchanged, so `../../etc/passwd` in the document text is still refused, as is a
  symlink whose target actually leaves the checkout. The rules differ because
  document text is what an untrusted `.tmd` controls, whereas a symlink is a
  filesystem fact placed by whoever owns the checkout.

- **The build's asset passes are bounded by the repository too.** `mirror_assets`
  walks the source tree directly rather than resolving paths through `safe_join`, so it
  applied no boundary: a directory or file symlinked out of the checkout was mirrored
  straight into `_site/`. The two ref-driven passes (the portable `--out` bundle, and the
  deploy of linked `.md`/`.scss` sources) checked only that the *ref* was lexically
  in-tree, which says nothing about what an in-tree path resolves to: `<img src="fig.png">`
  where `fig.png` is a symlink passed that test and shipped the target. All three now hold
  a symlink to the same repository boundary the document paths use, and the single-doc
  bundle warns on what it drops.

- **A symlink under the output no longer breaks the walk.** The build emits no symlinks,
  so one under `_site/`/`_book/` is the author's own mount (the stale sweep leaves them in
  place for that reason) and its contents belong in the deploy, but the archive and
  linked-source passes followed a mount pointing back up the tree without a cycle guard.
  The book archive failed outright (`FilesystemLoop` from the walk, taking the whole
  offline download with it); the linked-source pass re-walked the deploy once per level,
  re-copying what it had already shipped (41 deploys of one file in the regression
  fixture). Both descend into each directory once.

- **Site page discovery is bounded by the repository too.** The page walker reads
  directories directly rather than resolving paths through `safe_join`, so it applied
  no containment: a `.tmd` symlinked out of the tree was walked, rendered, and
  published as its own page in `_site/`. It now follows a link only while the target
  stays inside the repository. Directory links are additionally deduplicated by
  canonical path: a link back up the tree used to recurse until the path outgrew
  `PATH_MAX`, emitting one duplicate copy of every page per level (41 copies of a
  single page in the regression fixture).

- **A refused resource no longer reports itself as missing.** A `bibliography:`
  that resolved outside the project root warned "bibliography file not found" for a
  file that was on disk, and the page then rendered every reference as a
  bare BibTeX key. Containment refusals now say so and name which boundary was hit.

- **SIGHUP no longer leaks the kernel subtree.** `shutdown_signal` raced only
  SIGINT and SIGTERM, so closing a terminal tab took the preview down via
  SIGHUP's default disposition, skipping the teardown that reaps the Jupyter
  kernel and its forkserver children. SIGHUP now takes the same graceful path.

## [0.2.0] - 2026-06-27

The release-hardening release. Two waves of correctness, accessibility, and
authoring-trust work, each landed behind a full test + adversarial-review gate.
The aim was that a green `taliesin check` (and build) should mean publishable.

### Added

- **`taliesin check` is now a pre-publish gate.** Static, kernel-free, and
  deterministic, it flags (each click-to-source): broken internal/relative
  links and cross-page anchors, missing local video files, dangling `//| input`
  names and reactive-graph cycles, and a built-in accessibility audit
  (`validate_a11y`): heading-level skips, `<img>` missing `alt`, and
  `<a>`/`<button>` with no accessible name. `--format json` stays valid JSON
  even on its own errors (an unreadable path emits `{"error": ...}` on stdout),
  so it pipes cleanly into `jq` and CI.
- **Onboarding.** `taliesin init [dir]` scaffolds a minimal previewable site; a
  README install/prerequisites section; per-subcommand `--help` (focused
  synopsis + flags + example for `preview`/`build`/`check`/`render`/`schema`/
  `blocks`/`init`); unknown-command did-you-mean; the top-level usage now
  advertises the `<dir>` site mode.
- **Accessibility (rendered output).** Distinguishing `aria-label`s on every nav
  landmark (primary nav, book chapters, pager, table of contents), one
  consistent `:focus-visible` ring, deck slide roles + "Slide N of M" with a
  polite live region, `forced-colors`/`prefers-contrast` styles, and a
  server-side skip-to-content link + focusable `<main>` + real image `alt`
  (previously only present after JavaScript ran).
- **Citations.** LaTeX accents render as Unicode (Müller, Erdős), brace-
  protected corporate authors stay whole (e.g. `{World Health Organization}`),
  `@string` macros resolve, and `@inbook`/`@incollection` render their
  `booktitle` + pages. A manual `# References` section suppresses the
  auto-generated one.
- **`{{< video >}}`** accepts query strings in the source (`clip.mp4?token=…`),
  and figures honor `height=`.
- A `_quarto.yml`-only directory now gets a migration hint instead of a
  confusing "no `_site.yml`".

### Changed

- **Mermaid offline behavior.** A diagram whose library can't load now shows a
  visible `[data-mermaid-error]` banner (with the source below) instead of
  failing silently; the library URL is configurable via `TALIESIN_MERMAID_URL`
  for a fully self-hosted, offline build.
- Site `build` now honors an author's `404.qmd` (it is no longer overwritten
  by the built-in template, and is kept out of the search index).
- The build "kernel unavailable" hint names the right interpreter
  (`TALIESIN_R` for an R cell, not always `TALIESIN_PYTHON`).
- `THIRD_PARTY.md` now gives an accurate offline/CDN inventory.

### Fixed

- The cross-page link checker no longer false-flags intra-site links that carry
  a `?query` string.
- `taliesin init` (current directory) prints a runnable preview hint
  (`taliesin preview .`).

### Internal

- `cargo clippy` is warning-free; `cargo fmt` and the full `cargo test` suite
  (core + server, including the corpus invariant + no-false-positive guards)
  are green. Each batch was landed via isolated worktree lanes and verified by
  an adversarial pre-merge review.

### Known limitations

> **All four are resolved since this release** and are kept only as the record of
> what 0.2.0 shipped with. Current state: the `_extensions/` showcase no longer
> exists in the tree; `.github/workflows/` holds `ci.yml` and `release.yml`, restored
> 2026-07-28, but every `ci.yml` job is guarded on `repository.private != true`, so
> while this repository is private CI is inert and `.githooks/pre-push` is the only
> gate that runs automatically (`./tools/gates.sh` runs everything, by hand);
> Mermaid is vendored and inlined, so nothing fetches it; warm
> pages are evicted by a `MAX_WARM_PAGES` LRU, and a book has no sidebar to stack
> (navigation is a sticky topbar plus an off-canvas drawer).

- The bundled `_extensions/` showcase (liquid-glass) is currently non-functional
  against the native deck engine; an extension-ecosystem audit is the next
  dedicated pass.
- CI runs `fmt`/`test`/`clippy` plus a weekly `cargo-audit` advisory scan
  (`.github/workflows/ci.yml`); `cargo-deny` is configured (`deny.toml`) but its
  CI step is not wired yet.
- Mermaid still loads from a CDN by default (now configurable and non-silent,
  but not yet vendored).
- Long-running previews can grow memory unboundedly (visited pages are not
  evicted); the book sidebar stacks rather than drawers at ~900px (laptop
  portrait).

## [0.1.0]

Baseline: a fully native `.qmd`-to-HTML dev server, without legacy compat shims,
reveal.js or the Observable runtime. All four output shapes render and deploy:
blog post, slide deck (Taliesin's own engine), book, and multi-page site, with a
warm Jupyter kernel, block-level incremental updates with DOM-state
preservation, a `_freeze` execution cache, click-to-source, reverse cursor sync,
located diagnostics, and Cmd-K search.

[0.2.0]: https://github.com/AJBogo9/taliesin/releases/tag/v0.2.0
[0.1.0]: https://github.com/AJBogo9/taliesin/releases/tag/v0.1.0
