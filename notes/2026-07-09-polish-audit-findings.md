# Polish / productivity audit findings (2026-07-09)

Scope: not a bug hunt. The question was "what stops the tool feeling mature, and what
would let the author squeeze more productivity out of a writing session". Method: 6
read-only agents (CLI/DX, authoring surface, live preview, build+publish, editor bridge,
feature ideation) plus hand-verification of every root cause against source and against
the real binary. Nothing here is quoted from an agent without re-derivation: three agent
diagnoses were **refuted** on measurement (section 5) and are recorded so they are not
re-scoped.

Binary: `target/debug/taliesin` + `target/release/taliesin` @ `519cdd9`. Probe sites and
broken fixtures under `/tmp/claude-1000/`. Repo left untouched (a parallel session was
mid-edit on the reader-prefs picker throughout; its 5 files were not touched).

**The one-line summary:** the machinery exists, it is just not wired into the loops the
author uses. Ten static validators run only in `check`. `closest()` did-you-mean runs
everywhere except the two symbols an author types most. The VS Code companion's
diagnostics and completions have never run at all.

---

## 1. The companion has been switched off since the rename

**CONFIRMED. Effort S. Payoff high.** The single largest untapped asset in the repo.

- `editor/vscode/package.json:83` defaults `qmdFast.path` to `"qmd-fast"`. That binary
  ceased to exist at the rename; `which qmd-fast` fails, `taliesin` is what is on PATH
  (`~/.local/bin/taliesin`, plus a `tali` symlink).
- No user override exists (`~/.config/Code/User/settings.json` has no `qmdFast` key), and
  the extension is not installed as a packaged extension.
- Every subsystem keys off that default and fails **silently**: preview (`server.ts:13`),
  completions (`completions.ts:52`, returns `undefined` on spawn error at `:73`),
  diagnostics (`diagnostics.ts:31`, one toast then quiet).
- `editor/vscode/README.md:44` says "ensure the `taliesin` binary is on PATH", directly
  contradicting the shipped default.

**Compounding it:** the committed `.vsix` predates both features. `unzip -l` shows only
`out/{extension,server,ports,paths,webview}.js`; `complete.js`, `completions.js`,
`check.js`, `diagnostics.js` are absent. There is no `vscode:prepublish` script
(`package.json:89-94`), so `vsce package` silently repackages whatever sits in `out/`.

**Fix:** default to `"taliesin"`; add `"vscode:prepublish": "npm run build"`; rebuild the
`.vsix` (or stop committing it). Complete the `qmdFast.*` -> `taliesin.*` namespace rename
while in there (Tier 3 already tracked the rename as cosmetic; it is not, it is why
nothing works).

---

## 2. The confidence gap: `check` is a superset nothing else runs

**CONFIRMED.** `crates/server/src/check.rs:51-61` chains ten validators
(`validate_xrefs`, `validate_duplicate_heading_ids`, `validate_internal_anchors`,
`validate_local_assets`, `validate_local_media`, `validate_local_links`,
`validate_js_reactive_graph`, `validate_a11y`, `validate_math`,
`validate_code_languages`). Grep confirms **none** of them is called from
`crates/server/src/build.rs`, `serve/mod.rs`, or `serve_site/mod.rs`.

Reproduction (a site with one post carrying `![missing](does-not-exist.png)`):

```
$ taliesin check  <site>          -> exit 1, "local asset not found: `does-not-exist.png`"
$ taliesin build  <site> --strict -> exit 0, ships <img src="does-not-exist.png" />
```

`--strict` fails only on `problems`, which is built from `yaml_error` (`build.rs:328`),
`doc.warnings` (`:346`), embeds (`:354`), `validate_xrefs` (`:365`) and cell errors
(`:383`). On a 9-defect fixture: `check` caught 9, `build` caught 5. The four missed are
broken in-page anchor, missing image, broken cross-page link, broken anchor. `publish`
calls `run_site_build` (`publish.rs:188`), so it inherits the same blindness.

**The trap is that `--strict` catches *some* located warnings** (front-matter typos,
broken xrefs), so a green `--strict` build reasonably reads as "safe to ship".

**Fix:** hoist `check.rs`'s `collect_diagnostics` into a shared entry point; call it from
`build --strict`, from `publish`, and from `compute_diagnostics` in both dev servers. No
new validators. Debounce the filesystem-touching lints in the live loop so a
half-typed image path does not nag mid-sentence.

### 2a. An error-level diagnostic never turns the status dot red

**CONFIRMED (browser-driven).** Broken YAML front matter leaves `devDotState:"live"`
(green) with the only signal a small amber badge on the collapsed `</>` button. The
full-screen `#tali-error` overlay (`client.js:281-309`) fires only on `msg.type ===
"error"`, which the servers send only for file-read failures and renderer panics
(`serve/mod.rs:1152,1276`). A YAML parse error travels the *diagnostics* channel instead,
and `setStatus("error")` (`client.js:1005,1043`) is never reached. Severity exists in the
payload; the UI ignores it.

**Fix (S):** any `level:"error"` diagnostic reddens the dot. **(M):** promote the framed
YAML error (it already carries file/line/frame) into the existing overlay.

### 2b. A `.bib` edit does not rebuild the page in a **site** preview

**CONFIRMED (reproduced).** Edited `references.bib`; page kept the stale citation, no
rebuild queued. `serve_site/mod.rs:1087-1109` filters open pages by
`deps = page.input union includes::dependencies(src)`, and `includes.rs:155`
`dependencies()` tracks only `{{< include >}}`. `bibliography:` / `csl:` / `css:` never
enter the dep set. `.bib` *is* a watched extension, so the event arrives and matches no
page. The **single-doc** server is unaffected (it rebuilds on any relevant event).

**Fix (M):** widen the per-page dep set with the page's resolved `bibliography`/`csl`/`css`
paths. Reads `frontmatter` + `cite` config; must not alter their logic (Do-NOT-touch
adjacent).

### 2c. One broken include emits two diagnostics

**CONFIRMED.** Badge reads "2" for one problem: the dependency-existence check
(`serve_site/mod.rs:913-921`, `serve/mod.rs:870-878`) and the render pass's
`IncludeWarning` in `doc.warnings` both fire. Keep the located `IncludeWarning` (it carries
file/line), drop the other. **Effort S.** Pairs with 2a, which makes the badge load-bearing.

---

## 3. Two leaks of preview-only metadata into published output

Both reproduce on the real blog (`target/release/taliesin build corpus/tech-blog`).

### 3a. The author's absolute home path is in published HTML

**CONFIRMED.** `_site/posts/pca-geometry/index.html` carries
`data-source-file="/home/bogo/Documents/personal/taliesin/corpus/tech-blog/_includes/three-scene.tmd"`,
while a sibling page emits the relative `data-source-file="_includes/publications.md"`.

Root cause `crates/core/src/includes.rs:240` `label_for()`:

```rust
match target.strip_prefix(&primary) {
    Ok(rel) => rel.to_string_lossy().into_owned(),
    Err(_)  => target.to_string_lossy().into_owned(),   // <- absolute path
}
```

`strip_prefix` is against the *primary document's own directory*, so an include reached via
`../` into a sibling dir under the project root falls through to the absolute path. It is
also the only source of cross-machine build nondeterminism (same-machine builds are
byte-identical; verified by diffing two builds).

**Fix (S):** compute the label relative to the project root (`containment_root`, same file).

### 3b. Twelve `.tmd` source files are published into `_site/`

**CONFIRMED.** `find _site -name '*.tmd'` -> 12 files, including every post's `index.tmd`.

Root cause `crates/server/src/build.rs:423` `local_refs()`:

```rust
for attr in ["src=\"", "href=\""] {          // plain substring search
    while let Some(pos) = html[i..].find(attr) { ... }
```

`data-qmd-src="posts/a/index.tmd"` (the click-to-source attribute on listing cards,
`site/mod.rs:1120`) **contains the substring `src="`**. So `local_refs` yields the `.tmd`,
and `deploy_referenced_sources` (`build.rs:460`) dutifully ships it, because `.tmd` is in
`SKIP_EXT` and that function exists precisely to deploy referenced `SKIP_EXT` files. Its
own doc comment states the intent: "a linked `.md` download, a `.scss` offered for
inspection". Publishing sources is not that.

`_site.yml` is safe by luck: `chrome.rs:77` also emits `data-qmd-src="_site.yml"`, but `yml`
is not in `SKIP_EXT`, so `deploy_referenced_sources` skips it and `mirror_assets` drops
`_`-prefixed entries.

**Fix (S):** match on an attribute boundary (preceding char is whitespace or `<`), not a
bare substring. Regression test: a listing card must not deploy its `.tmd`; an explicit
`[source](index.tmd)` link still must.

---

## 4. Reader-facing artifact weight

**CONFIRMED (measured on the built `corpus/tech-blog`).**

| page | size |
|---|---|
| `posts/em-algorithm/index.html` | 1.72 MB |
| `posts/pca-geometry/index.html` | 1.67 MB |
| `posts/KL-divergence/index.html` | 712 KB |

Composition of the KL-divergence page: inlined `<style>` 453 KB (64%), of which **339 KB is
base64 KaTeX woff2** (48% of the page); inlined `<script>` 106 KB (15%); actual content
153 KB (21%). **Seven pages carry that identical 339 KB font block.**

Inlining is correct for `build file.tmd` (portable, must work over `file://`). For
`build <dir>` it means a returning reader re-downloads ~97% of every page. Extract to
content-hashed `app.<hash>.css` / `app.<hash>.js` / `katex.<hash>.css`, linked once, and
minify while there. **Effort L, payoff high**, reader-facing rather than
author-productivity, so it sequences after the tiers above.

Also absent: `sitemap.xml`, `robots.txt`, JSON-LD. OG/Twitter/`canonical` and the scholarly
`citation_*` meta **are** present and correct (`site/meta.rs`). This is exactly the
un-started roadmap item `build-seo-completeness`; its scope is right.

---

## 5. REFUTED. Do not re-scope these.

Three plausible diagnoses died on measurement. Recorded because the symptom-vs-cause trap
has bitten this project before.

- **"`build` leaks forkserver subtrees."** FALSE. Controlled experiment (snapshot pids ->
  build -> wait 3s -> snapshot), run twice, once with `TALIESIN_NO_CACHE=1` forcing real
  cell execution: **zero** new survivors both times. The 2026-07-08 process-group reaping
  fix holds on the graceful path.
- **"The warm pool boots Python on prose-only builds, costing latency."** The boot is real
  (`build.rs:926` warms before knowing any page needs a kernel, and it happens even under
  `TALIESIN_NO_EXEC=1`, which neither `build.rs` nor `warm_pool.rs` consults). The **latency
  claim is false**: prose-only site, 3 runs each, 0.25 s default vs 0.27 s with
  `TALIESIN_NO_EXEC=1`. The boot is off the critical path. This is a resource-hygiene item,
  not a perf item.
- **"Dev attributes (`data-block-id`/`data-sourcepos`/`data-source-file`/`data-qmd-src`)
  bloat published pages."** FALSE: 2104 bytes on a 712 KB page = **0.29%**. Section 3 is a
  correctness problem, not a weight problem. Do not propose stripping them for size.

Also worth re-checking rather than working around: `CLAUDE.md` warns that editing
`assets/css/*` needs a `cargo build` or you measure stale CSS. Cargo tracks `base.css` in
dep-info (`target/debug/taliesin.d`) and a marker appended to it **did** appear in the
freshly built binary. The documented claim (rebuilding only the *site* re-emits the old
bundled CSS) is trivially true; any stronger claim that `cargo build` silently embeds stale
assets was not reproducible for `assets/css/`.

---

## 6. Resource hygiene: the ungraceful path

**CONFIRMED, but the cause is not what it looks like.** Right now: **2 orphaned forkserver
subtrees** (one alive since 08:39, reparented to `systemd --user` pid 7935, so its
`taliesin` parent is gone) and **21 leftover `/tmp/tali-*` dirs, 16 with no live process,
77 MB on disk**.

Since `build` provably reaps (section 5), the orphans come from **ungraceful death**:
SIGKILL, a closed terminal, a crash. No `Drop` handler can catch those. Confirmed absent:

- `grep -rn "PDEATHSIG\|prctl" crates/server/src/` -> nothing. A SIGKILL'd parent orphans
  the whole pool.
- no startup sweep of stale `/tmp/tali-warmpool-*` / `/tmp/tali-kernel-*` dirs.

**Fix (S/M):** set `PR_SET_PDEATHSIG` on the warm-pool helper (Linux; the helper already
gets its own process group), and sweep stale `/tmp/tali-*` dirs whose owner pid is dead at
startup. Independently, make `build` skip the warm pool when `TALIESIN_NO_EXEC` is set or
when no page has an uncached kernel cell. **Do-NOT-touch zone: exec/kernel. Careful.**

---

## 7. CLI and terminal papercuts (each CONFIRMED)

- **`preview <missing file>` serves a blank page.** `serve/mod.rs:331`
  `read_to_string(&app.path).ok()?` turns a missing file into an empty doc. Prints `ready`
  + `watch`, serves HTTP 200 with an empty `<main>`. Every other command errors and exits 1
  (`query.rs:69`, `build.rs:164`, `check.rs:45`). The create-it-later workflow **does** work
  (verified: creating the file mid-session fired the watcher), so keep the behavior and just
  say so: one `log::warn`. **S, high.**
- **`preview <dir>` with 0 pages** binds a port, 404s `/`, and boots the kernel pool for
  nothing, while `check <same dir>` exits 1 with "no `.tmd` pages found". The two front
  doors disagree. **S.**
- **No `--port <N>`.** Port is positional-only (`cli.rs:98,127`); `--port 4400` errors with
  `did you mean --host?`, pointing at the wrong flag. **S.**
- **`log::info` reuses the green `built` tag** (`log.rs:173` `line(Style::Built, msg)`), so
  Ctrl-C prints `built shutting down (reaping kernel)` and a build *start* prints
  `built building with up to 14 parallel page(s)`. Give `info` its own neutral tag; route
  the two shutdown lines through `log::kernel`. **S.** (Distinct from the "CLI/docs
  microcopy" item closed 2026-07-09, which was about prose, not tags.)
- **`check` on a folder with no `_site.yml`** reports "1 problem" and exits 1 for an
  advisory. Do not count it toward the failing tally. **S.**
- **`taliesin help build`** prints top-level usage; only `build --help` works
  (`main.rs:60` matches `help` before the after-subcommand intercept at `:33`). **S.**
- **Build summary has no elapsed time** (`build.rs:212,1102`), while `ready` proudly prints
  `75ms`. **S.**
- **`--version` has no `-dirty` flag**, though the launcher rebuilds on every source edit.
  **S.**
- **`TALIESIN_MERMAID_URL`** (`render/mod.rs:914`) is user-facing but missing from the `ENV:`
  block in `usage()`. **S.**
- **Cold multi-page builds stream unlabeled `cell k/n`** from concurrently-running pages
  (`exec.rs:539`), unattributable to a page. **S.**
- **No shell completions**, and no seam (hand-rolled CLI, no clap). 12 stable command names;
  a static bash/zsh/fish script is ~120 lines. Precedent exists: `schema`/`vocab` already
  feed *editor* autocomplete. **M, med.**

Already excellent, leave alone: `publish`'s error messages name the exact fix and the
one-time setup (`publish.rs:144-151,237-244`); stdout/stderr hygiene is clean
(`render > out.html` is unpolluted); the unknown-command/flag did-you-mean is consistent.

---

## 8. Authoring-surface friction

- **A typo'd category silently forks the listing filter.** Fed `statistics`, `Statistics`,
  `statstics` to one post: three separate chips, each count 1
  (`data-cat="statistics"|"Statistics"|"statstics"`). No validation of category values
  anywhere. A `check` warning via `closest()` over the site's category vocabulary is the
  obvious fix, and matches the machinery used for config keys. **S, med.**
- **`<title>` falls back to the filename, never the leading H1.** A front-matter-less doc
  starting `# My Great Post` renders `<title>notitle</title>` (`page.rs:247` uses
  `fallback_title` = the file stem, `build.rs:169`). 9 `.tmd` files in-tree have an H1 and no
  front matter. This is a "better default over a knob" fix. **S, med.**
- **Missing alt on `![](x.png)`** is treated as decorative (a11y convention), so a *forgotten*
  alt gets no nudge. Defensible; noted, not filed.
- **The date renders verbatim** (`render/mod.rs:755-760`): `2026-04-14`, not "14 April 2026".
  Taste. **S, low.**

---

## 9. Feature proposals (writing + publishing productivity)

Ranked by payoff / effort. Each verified against the invariants (HTML-only; single editing
surface; minimal config; Do-NOT-touch machinery; must name a corpus pin).

1. **Did-you-mean for `@fig-` / `[@cite]`** (S, med-high). Renaming a label is the most
   common way an author silently breaks their own document. `cite/validate.rs:28` builds
   `broken cross-reference: @fig-reslts` with no suggestion, while `closest()`
   (`frontmatter.rs:414`) is used for CLI commands and front-matter keys. The candidate set
   (registered anchors; parsed bib keys) is in hand at warn time. Guard with the existing
   edit-distance-2 ceiling; suggest only within the page's label namespace.
   *Pin:* near-miss `@fig-` + `[@key]` in `corpus/diagnostics/`.
2. **`taliesin new <post|page|deck> <slug>`** (S/M, high). The blank-page tax. The author
   already worked around it: `corpus/tech-blog/.claude/skills/new-post/SKILL.md` is a
   hand-built scaffolder living **outside** the tool, and it is stale (still emits `.qmd`,
   still says `quarto preview`). Emit keys from the same `frontmatter::KNOWN_KEYS`/schema
   consts the validator enforces, so the scaffold is correct by construction; reuse `init`'s
   refuse-before-overwrite guard (`cli.rs:58`). *Pin:* `corpus/scaffold/post/`, asserted to
   render and pass `check` clean.
3. **`taliesin symbols <file> --format json`** (M, med-high). The companion's completion
   harvests `{#id}` anchors with a JS regex (`complete.ts:86-90`), so it misses every figure
   labeled via `#| label: fig-scree`. Corpus count: **34 cell-labeled `fig-`/`tbl-`/`lst-`
   targets vs 43 brace-anchored ids, i.e. ~44% of cross-ref targets are invisible to
   autocomplete.** Emit the resolved xref registry (`render/mod.rs:1392` `register_xref`)
   plus real bib keys (`cite/parse.rs`) from Rust, riding the `query.rs` dispatch beside
   `blocks`/`vocab`. This is the same no-drift discipline `vocab.rs:1-9` exists for. The
   cheap alternative (widen the JS regex) reimplements Rust knowledge in JS and still misses
   auto-numbers and cross-page anchors.
4. **`.tmd` snippets in the companion** (S, med). No `contributes.snippets` today. Corpus
   volume: 184 code cells, 520 fenced-div openers, 108 front-matter blocks, 64 callouts, 57
   `#| label:` lines. Top 8: cell, front matter, callout, figure, theorem, tabset, margin,
   xref/cite. Descriptions can reuse `vocab.rs` text so they cannot drift.
5. **`llms.txt` + `llms-full.txt` at build** (M, med-high). The old deploy ritual generated
   this (`corpus/tech-blog/.claude/skills/deploy/SKILL.md:24` runs `generate_llms_full.py`);
   the migration silently dropped the capability. The block model already separates clean
   prose from code and math (`client.js:50` proves the extraction path), so it would be more
   accurate than the Python scraper it replaces. A plain-text sidecar is the same category as
   `sitemap.xml`, not a new output format; fold into `build-seo-completeness`.
   *Pin:* a `tech_blog.rs` assertion that `llms.txt` lists discovered pages and
   `llms-full.txt` excludes drafts.
6. **Site-level shared bibliography + bib hygiene** (M, med-high). `bibliography:` is
   per-document only (`cite/mod.rs:42`); a growing blog retypes keys per post and nothing
   reports an unused or duplicate entry. Allow `bibliography:` in `_site.yml`, merged under
   each page's own; add two **read-only** diagnostics over the parsed registry ("entry never
   cited", "duplicate key"). Explicitly does **not** touch the BibTeX parser/CSL formatter.
   Keep "unused entry" at info level or `check`-only, since a working bib runs ahead of the
   prose. *Pin:* a small site with a site-level bib, one entry cited from two pages, one
   uncited.
7. **Author structure panel** (M/L, high). A read-only preview sidebar: heading tree with
   per-section word count (the dev panel already counts, `client.js:50-58`) and a badge per
   node for unresolved xref / TODO / over-goal length. Click to scroll, and under the
   companion move the editor cursor via existing cursor sync. This is the *revision* view,
   not the reader TOC. Scope as an annotation layer on the dev panel, not a new component.
   *Pin:* `corpus/layout/structure.tmd` (name already reserved by FEATURE-IDEAS #26).
8. **TODO / FIXME surfacing** (S, med). `prose.rs::lint` already returns markdown-aware,
   code/math-skipping located `(line, message)` pairs. A `TODO|FIXME|XXX` scan surfaced as
   info-level located diagnostics makes a draft's loose ends visible without leaving the
   editor. Never writes back to source.
9. **Session revision digest** (M, med). Surface the `BlockOp` stream the client already
   receives: session word delta (`+340 / -180`) and a feed of the last N ops, each
   click-to-source. Cashes the diff moat; no batch compiler has a diff to show. Honest
   caveat: this is a behavioral pin (`tools/live-edit-bench` assertion), not a corpus doc.
10. **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M, med). Reuse a section
    across a series without copy-paste drift. Must ride **on top of** the `includes.rs`
    source-map pass (resolve the fragment to a block range, hand the existing machinery a
    sub-slice), never rewrite it. Hard merge gate: the source map must not perturb. Defer
    until a real series needs it.

### Architecture recommendation: LSP for the intelligence, browser for the view

Everything the language intelligence needs already exists in Rust: `check`, `vocab`, the
`register_xref` registry, the bib parser, `closest()`. An LSP is write-once and serves
Neovim/Helix/Zed/VS Code, and it **removes the drift** that causes proposal 3's gap (JS
regexes reimplementing Rust knowledge). An LSP cannot render the preview, and it does not
need to: the preview is already editor-agnostic (any browser; the sync surface is two
`postMessage` shapes specced in `docs/internals/protocol.tmd:325-350`). The only thing
binding it to VS Code is the hardcoded `vscode://` open scheme; generalize that and other
editors get forward sync for free. Do not rebuild the preview as an LSP; do not invest
further in the webview beyond keeping section 1 fixed.

### Owner-gated (need a ruling, do not build blind)

- **Draft-aware preview.** `draft: true` currently hides a page from the site *preview* as
  well as the build (`site/discovery.rs:19-23`), so a half-written post cannot be seen among
  its own listings and nav until it is un-drafted, which is exactly when the author wants to
  see it. Proposed better default: **preview includes drafts** (quiet DRAFT badge),
  **build/publish exclude them** and print `2 drafts not published: ...`. It flips an
  established default and widens a discovery code path, hence the gate.
- **Reading time in the built page.** Word count + reading time are computed but trapped in
  the author-facing dev panel (`client.js:50-58`), and `corpus/tests` pins their *absence*
  from the built page (`corpus.rs:530-533`) as a deliberate decision. Promoting them is a
  reader-facing flip of that decision, not a bug fix.
- **`taliesin publish --public`.** `cmd_publish` unconditionally calls `inject_gate`
  (`publish.rs:194`) and `_middleware.js:9` fails closed (503 when `PASSWORD` is unset). So a
  **public** blog cannot use `publish` at all, which is why the real blog deploys via a
  side-channel `deploy` skill. A `--public` / `publish.gate: false` mode would let the actual
  workflow use the actual command.
- **Shared cacheable asset bundle** (section 4). Large, reader-facing, changes the shape of
  the build output.
</content>
</invoke>
