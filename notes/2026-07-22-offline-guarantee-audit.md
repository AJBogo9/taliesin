# Audit: offline-guarantee verification (perspective AP12)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-22. Perspective: AP12 from the backlog "Audit perspectives" section
(offline-guarantee verification). Run as a single-perspective session alongside two live
sessions (a feature session on DX17b and a separate audit), so it touches no source,
builds nothing from the working tree, and writes only findings files. Evidence came from
reading the asset-bundling and build paths plus building a crafted document with the
frozen `taliesin-stable` binary (`/home/bogo/.local/bin/taliesin-stable`, Jul 7), which
needs no `cargo build` and does not contend with the other sessions.

## Why this perspective

"Bundled offline, no CDN, no external request" is a headline invariant (CLAUDE.md, the
marketing site, the `build ... --out <dir>` "portable folder" promise). Every prior audit
took it as given. This perspective tests it: follow every path by which a rendered or
built page could cause the browser to reach the network, and separate what the tool
controls (its own bundled assets) from what it does not (references the author writes).

## Executive summary

The tool's OWN offline story is solid and well-guarded: fonts are local `woff2`, KaTeX is
server-rendered, d3 and Observable Plot are vendored, mermaid is inlined into static
builds, and a test already guards against a reveal.js/jsdelivr regression. Those are
verified negatives below.

The gap is author-introduced external references. A `build ... --out <dir>` "portable"
folder silently retains any external URL the document contains (a remote image, a remote
`{js}` ESM `import()`, an external stylesheet or script) with no diagnostic, so a build
labelled portable can quietly require the network at view time. Separately, live preview
lazy-loads mermaid from a CDN by default even though the library is already vendored, so
editing a mermaid diagram offline fails in preview.

Neither is a leak in the tool's assets; both are the offline promise not being ENFORCED
at the one moment the author would want to know (the build) or experienced where they
would notice (the preview). The fix that fits this tool is a diagnostic, not a downloader:
the tool's proven strength is located "did-you-mean" validators, and "this build is not
self-contained: line X references esm.sh" is the same move.

## Findings

### OFF-1 (medium): a "portable" build silently keeps external runtime dependencies, unflagged

`taliesin build <file> --out <dir>` promises "index.html + copied local assets."
`copy_local_assets` (`crates/server/src/build.rs:707`) recursively bundles LOCAL assets
(a `{js}` cell's `import("./helper.js")` pulls in `helper.js` and transitively `util.js`;
a local `<img src>` is copied), which is good. But the asset collector skips anything
external: `crates/server/src/build.rs:2046` drops any value containing `://`, and the JS
import walker (`build.rs:889`) skips remote and bare specifiers. Those references are left
verbatim in the output, and nothing warns. There is no external-reference diagnostic
anywhere in `build.rs` or `check.rs` (searched).

Empirical proof (frozen binary, no build). A document with a remote image and a remote
ESM import:

```
![a remote image](https://example.com/pic.png)

{js} cell:  const three = await import("https://esm.sh/three@0.163.0");
```

built with `taliesin-stable build off.tmd --out off-out` produced:

```
  built   .../off-out/index.html  ·  0 assets        (exit 0, stderr otherwise empty)
```

and the built `index.html` still contains, as live references:

```
<img src="https://example.com/pic.png" ...>
... await import("https://esm.sh/three@0.163.0");
```

So a folder the tool called portable ("0 assets") depends on `example.com` and `esm.sh`
at view time, and the author is told nothing. Offline, both the image and the chart fail.

The behavior of NOT downloading remote modules is itself correct and already tested
(`build.rs:2300-2339`, `copy_local_assets_bundles_js_cell_imports_recursively`, which
pins "remote import must not be fetched/copied"): auto-downloading arbitrary URLs at build
time would be a worse design (network at build, license/version surprises, a fetch step in
an offline-first tool). The defect is the SILENCE, not the non-download.

Recommendation (build-ready, philosophy-aligned): add a located build/preview diagnostic
that inventories every external runtime reference remaining in the output: external
`<img>/<script>/<link>` `src`/`href`, CSS `url(...)` / `@import` to an external host, and
remote/bare `{js}` `import()` specifiers. Report it in the same located "did-you-mean"
style ("line 8 references esm.sh; a `--out` build will not be self-contained"), gated so
it is informational, not an error (the author may want the external ref). This turns the
offline promise from an unchecked claim into a checked one. Size: M. Files:
`crates/server/src/build.rs` (reuse the `://`-skip site at `build.rs:2046` and the import
walker as the detection points), surface via `crates/server/src/check.rs`.

### OFF-2 (low-medium, partially known): live preview fetches mermaid from a CDN despite the vendored copy

`code_scripts_for` (`crates/core/src/render/mod.rs:1292-1311`) inlines the vendored
`MERMAID_MIN_JS` (~2.5 MB) only in a static `Build`; in `Preview` it ships the lean loader
whose `{{MERMAID}}` resolves to `mermaid_url()` (`mod.rs:1267`), a pinned CDN default unless
`TALIESIN_MERMAID_URL` is set. The code comment states the tradeoff explicitly ("dev-time
network is fine, and inlining 2.5 MB on every save would bloat the payload"). Consequence:
an author editing a mermaid diagram with no network (offline, a plane) sees it fail or
blank in the live preview, even though the library is already bundled in the binary. This
partially overlaps Open-work item 10's mermaid SRI/`crossorigin` note, which is about the
same CDN load but from a supply-chain angle.

Recommendation: make preview offline-complete for mermaid. Options, smallest first: inline
the vendored library in preview only for pages that actually contain `class="mermaid"`
(the same gate `Build` uses), so the 2.5 MB is paid only on mermaid docs and only while
editing one; or lazy-inline once and cache; or, at minimum, surface "mermaid loaded from
network" so an offline author understands the blank. Size: S to M. File:
`crates/core/src/render/mod.rs`.

### OFF-3 (low, by design): external og:image / listing thumbnails are allowed and unlisted

`crates/core/src/site/discovery.rs:34` intentionally allows an absolute/external URL for a
social-card `og:image` or a CDN-hosted listing thumbnail. This is reasonable (an `og:image`
is fetched by a crawler, not the reader, and a thumbnail is the author's choice), so it is
NOT a defect. Noted only so that the OFF-1 inventory INCLUDES these references rather than
special-casing them away: the author should still be able to see, at build time, every
external reference their "offline" output carries, og:image included.

## Verified offline (honest negatives)

- **Fonts are local.** `crates/core/assets/css/fonts.css` uses
  `@font-face { src: url(fonts/newsreader-latin-*.woff2) }`, relative to the bundle. No
  Google Fonts, no Fontsource CDN, no `@import`. The `http://...` strings in
  `assets/fonts/OFL.txt` are license attribution, not fetches.
- **KaTeX is server-rendered** and its CSS/fonts are bundled (`crates/core/src/math.rs`),
  so math has no runtime dependency.
- **d3 and Observable Plot are vendored** (`assets/js/d3.min.js`, `plot.umd.min.js`) and
  inlined for `{js}` cells; the `https://d3js.org` string in the built page is the library
  banner comment inside the inline `<script>`, not a `src=` fetch (verified in the built
  output).
- **Mermaid is fully offline in static builds** (inlined, `mod.rs:1303`); the loader's CDN
  URL is a never-reached fallback once `globalThis.mermaid` is set.
- **A reveal.js/jsdelivr regression is already guarded** (`render/tests.rs:1880` asserts
  the page references neither `jsdelivr` nor `reveal.js@`).
- **The many `https://` strings in `mermaid.min.js` / `d3.min.js`** are license text and
  parser error messages inside the vendored blobs, not runtime fetches.

## Build-ready items to fold into backlog.md "Open work"

- **OFF-1 (M):** a located build/preview diagnostic inventorying external runtime
  references (`<img>/<script>/<link>`, CSS `url()`/`@import`, remote `{js}` imports)
  retained in the output, so a `--out` build's offline claim is checked, not assumed. Do
  not auto-download (that is correctly avoided today); warn. `build.rs` + `check.rs`.
- **OFF-2 (S-M):** make live preview offline-complete for mermaid by inlining the vendored
  library on mermaid pages (gated like the build path), or at least surface the network
  load. Overlaps item 10. `render/mod.rs`.
- **OFF-3:** fold into OFF-1's inventory (include og:image / external thumbs); no separate
  work.

## Method notes for the next AP12-style run

- The fastest proof is the build probe: a two-line `.tmd` with a remote image and a remote
  `{js}` import, `taliesin-stable build ... --out`, then grep the built `index.html` for
  `://` and read stderr for any warning ("0 assets" + no warning is the finding).
- Filter out the vendored-library banners (`d3js.org`, the `chevrotain.io` / `lodash.com`
  strings in `mermaid.min.js`): they are attribution and error text, not fetches. Only
  `src=`/`href=`/`url(...)`/`import(...)` to an external host is a real runtime dependency.
