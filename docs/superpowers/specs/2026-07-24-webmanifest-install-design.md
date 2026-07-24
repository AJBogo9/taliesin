# Web app manifest: installable sites and books

Date: 2026-07-24. Status: designed, not implemented.
Supersedes the "installable PWA" half of `notes/FEATURE-IDEAS.md` item 14.

## Problem

A reader who wants to come back to a Taliesin book or docs site has only a browser
bookmark. There is no icon on a phone home screen, no app entry in a desktop dock or
taskbar, and no chrome-free window. The two motivating readers:

- a developer who relies on a framework documented with Taliesin and wants the docs one
  click away, like an app;
- a student reading a course book on a commute who wants to tap an icon on their phone.

## What already exists

A **book** build already emits `<book>.zip` at its output root and links it from the book
topbar (`crates/server/src/zip.rs`, packed at `crates/server/src/build.rs:1865`, linked at
`crates/core/src/site/chrome.rs:253-260`, pinned by
`crates/server/tests/book_offline_archive.rs`). It is deliberately `file://`-hardened:
`web-client/search.js:216-217` loads the search index as a `<script>` subresource rather
than `fetch()` specifically so Cmd-K keeps working when the book is opened from disk.

So **offline is already solved for books**, permanently and reader-owned, with no cache to
version and no support burden. What the zip does not solve:

- **Re-entry.** `~/Downloads/my-guide.zip` is not an icon. Getting back in means locate,
  unzip, find `index.html`, open.
- **Phones.** The archive lands in the iOS Files app, and browsing an unzipped multi-page
  site over `file://` from there is unreliable. The commuting-student case is the one the
  zip serves worst.
- **Non-book projects.** The download link lives in the book sidebar
  (`crates/core/src/site/mod.rs:644`, `book.then(...)`), so a website or a docs site
  without `chapters:` has no archive at all.
- **Freshness.** A snapshot never tells the reader the docs moved on.

This design covers the re-entry half only. The zip keeps the offline half.

## Decision

Emit a `manifest.webmanifest` per site build, plus the matching `<head>` tags. Do not ship
a service worker.

### Why no service worker

A service worker is the only piece of this feature space that is a one-way door. Once
registered it lives in the reader's browser independent of the pages, so a bug keeps being
served after the site is fixed, and un-shipping requires deploying a replacement worker
that unregisters itself and then waiting for every reader to return. It also brings
irreproducible "the page is stale" reports, an update protocol to own forever, silent
multi-MB writes against a roughly 50MB iOS cache budget, and a coupled obligation to ship
the OFF-1 external-reference diagnostic from
`notes/2026-07-22-offline-guarantee-audit.md`, because an author's remote image would
otherwise break a page that the tool promised worked offline.

The zip already delivers the offline value that a service worker would have bought, so the
door stays closed and both halves of the feature stay dumb and reversible. The manifest is
also a prerequisite for a service worker, so this decision is not foreclosed, only deferred.

### Browser reality this targets

| Platform | Effect |
|---|---|
| Chrome/Edge desktop | Omnibox "Install", app window, dock/taskbar entry |
| Chrome/Edge/Firefox/Samsung on Android | "Install app", home-screen icon, standalone window |
| iOS/iPadOS Safari (also Chrome/Edge/Firefox on iOS 16.4+) | Share → Add to Home Screen, manual only, correct name and icon instead of a screenshot thumbnail and a bare domain |
| macOS Safari 17+ | "Add to Dock", picks up the manifest name and icon |
| Firefox desktop | Ignores manifests. No effect, no cost |

Chrome removed the service-worker requirement for menu installation (Chrome 108 mobile,
112 desktop), so installability needs only the manifest, HTTPS, and PNG icons.

## Design

### Emission

- **`crates/core/src/site/manifest.rs`** (new, roughly 80 lines): `&SiteConfig` plus the
  resolved icon paths in, manifest JSON string out. Sibling of `feed.rs` / `llms.rs` /
  `seo.rs`, following their shape. Strings interpolated into the JSON are escaped the same
  way `site::search::json_str` (`crates/core/src/site/search.rs:178`) escapes the search
  index, which already has a test for the `</script>` and control-character cases.
- **`crates/server/src/build.rs`**: written in the sidecar block at lines 1718-1745,
  alongside `sitemap.xml` / `robots.txt` / `llms.txt`. One deliberate difference: that
  block is gated on `url:` because feeds, sitemaps and JSON-LD need absolute URLs. **The
  manifest is not gated on `url:`**, because every URL inside it is relative.
- **`crates/core/src/render/page.rs`**: the `<head>` tags, emitted next to the existing
  `{favicon}` slot (`page.rs:279`) and built by a helper beside `favicon_link`.

### Build-only gating

`crates/core/src/site/mod.rs:727-729` already documents `render_page_doc_external` as "the
multi-page build path ... the one place the offline-download link is wired", passing
`downloads: self.is_book()`. The manifest link rides the same gate, as a second
build-only chrome flag on the same call path.

Preview must never emit it. If it did, Chrome would offer to install `localhost:4388`, and
the installed app would break permanently the moment the dev server stopped. Gating here
also means the warm server, block diff, click-to-source and hot reload are untouched,
which is what makes this change safe.

### Manifest contents

```json
{
  "name": "<title:, else the project directory name>",
  "short_name": "<name up to the first colon>",
  "description": "<description:, omitted when unset>",
  "start_url": "./",
  "scope": "./",
  "display": "standalone",
  "theme_color": "#ffffff",
  "background_color": "#ffffff",
  "icons": [
    { "src": "icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "icon-512.png", "sizes": "512x512", "type": "image/png" },
    { "src": "icon-maskable-512.png", "sizes": "512x512", "type": "image/png",
      "purpose": "maskable" }
  ]
}
```

Derivation rules:

- `start_url` and `scope` are `"./"`, resolved against the manifest's own URL. A
  root-hosted site and a book deployed under `/docs/guide/` both come out correct with no
  dependency on `url:`.
- `id` is omitted. Its spec default is `start_url`, which is what we want.
- `short_name` is the name truncated at the first `:`, trimmed. "Taliesin: The User Guide"
  becomes "Taliesin". If the result still exceeds 30 characters the full name is used and
  the OS ellipsizes.
- `display: standalone` is what removes browser chrome, and on iOS it is also what exempts
  a home-screen app from Safari's 7-day script-writable-storage purge.
- Colours are the light `--tali-bg` value from `crates/core/assets/css/tokens.css:18`. The
  default theme mode is `auto` (`crates/core/src/render/theme.rs:56-62`), so no single
  static value can be right for both modes. A manifest permits only one. The live UI is
  handled instead by paired head tags, which browsers honour over the manifest:

```html
<meta name="theme-color" media="(prefers-color-scheme: light)" content="<light --tali-bg>">
<meta name="theme-color" media="(prefers-color-scheme: dark)"  content="<dark --tali-bg>">
```

  Known limitation, accepted: a dark-mode reader may see a light launch splash, because
  `background_color` has no media-query form.
- The colour values are the project's own `--tali-bg` tokens, not new colours invented for
  this feature. Because a manifest is JSON, the values must exist as Rust constants rather
  than as `var(--tali-bg)`, so they are duplicated from
  `crates/core/assets/css/tokens.css:18` and `tokens-dark.css:10` into one commented pair
  of constants, and a test (below) asserts the constants still match the CSS so the
  duplicate cannot drift.

No new `_site.yml` keys, so `NATIVE_KEYS` and the typo validator are unchanged.

### Head tags

Added on the build path only, per page, at the correct relative depth:

```html
<link rel="manifest" href="<depth>manifest.webmanifest">
<link rel="apple-touch-icon" href="<depth>icon-192.png">
<meta name="apple-mobile-web-app-title" content="<short_name>">
<meta name="theme-color" media="(prefers-color-scheme: light)" content="...">
<meta name="theme-color" media="(prefers-color-scheme: dark)" content="...">
```

`apple-touch-icon` reuses the 192px asset rather than adding a fourth 180px file. iOS
scales it, which is a marginal quality cost in exchange for one fewer asset to generate,
ship and keep in sync.

### Icons

Manifests do not accept SVG, and the default favicon is `web-client/favicon.svg`, so PNGs
are required. Resolution is by convention, with no config key, and it is **all-or-nothing
per source** so the two sets can never be mixed:

1. If **both** `icon-192.png` and `icon-512.png` sit next to `_site.yml`, the author's set
   is used. `icon-maskable-512.png` is optional: present, it is added as the `maskable`
   entry; absent, that entry is simply omitted.
2. Otherwise the full bundled Taliesin set is used, including its maskable icon.

The all-or-nothing rule exists because per-file fallback would produce a mixed-brand result
(the author's mark in the launcher, the Taliesin mark in Android's adaptive-icon slot), and
because a lone `icon-512.png` would emit an icon list missing the 192px entry that Chrome
requires for installability, silently costing the install prompt.

Author-supplied icons need no new copy path: a PNG in the project root is already mirrored
into the output (pinned by the `keep.png` case at `crates/server/src/build.rs:2589`). The
bundled defaults are three PNGs committed under `crates/core/assets/icons/`, emitted into
the output only when used, and `include_bytes!`'d like other bundled assets.

**The bundled PNGs are rasterized once, by hand, and committed.** No rasterizer dependency
(`resvg` or otherwise) enters the build. The generating command is recorded in a comment
next to the committed files so they can be regenerated if the mark changes.

`taliesin check` emits a note in two cases: a project that sets `favicon:` but ships no
icons at all, and a project whose icon set is incomplete (for example only
`icon-512.png`), which silently falls back to the bundled mark under the rule above. Both
say that the installed app will wear the default Taliesin mark and which files to add. It
is a `check` note only, never a `build` warning, so an author who does not care about
installing never sees it during normal work.

## Non-goals

- No service worker, and no claim anywhere in the UI or docs that this makes a site work
  offline. The zip owns that claim.
- No push notifications, no background sync, no `shortcuts` or `screenshots` manifest
  members.
- No manifest for single-file `build file.tmd` output (nowhere to put a sidecar) or for
  `build --out <dir>` portable folders (opened over `file://`, where manifests do not
  apply).
- No manifest in `preview`.
- No new user-facing configuration.

## Testing

Corpus-plus-roadmap asks each feature to ship pinned by a corpus document, but this feature
renders no document: it is build packaging. The honest pin is a build-level integration
test, exactly as `crates/server/tests/book_offline_archive.rs` pins the zip. This is a
reasoned exception of the same kind already recorded for preview-workflow features in
`docs/superpowers/specs/2026-07-03-quarto-design-decisions-catalog.md:179`, not a silent
skip.

New `crates/server/tests/webmanifest.rs`:

1. A site build emits `manifest.webmanifest` at the output root, parsing as JSON and
   carrying `name`, `icons` (192 and 512), `start_url`, `scope`, `display`.
2. Every built page links it at the correct relative depth: `manifest.webmanifest` from the
   root page, `../manifest.webmanifest` from a nested one, mirroring the nested-depth
   assertion the zip test already makes.
3. A complete author set (`icon-192.png` + `icon-512.png`) is referenced when present, and
   the bundled default set is emitted and referenced when absent.
4. An **incomplete** author set (only `icon-512.png`) falls back to the full bundled set
   rather than emitting a mixed list or one missing the 192px entry.
5. It is emitted for a project with no `url:` set, unlike `sitemap.xml` and the feeds.
6. Both a book and a website get one.
7. The preview render path emits **no** manifest link and no `apple-touch-icon`.
8. `short_name` derivation: colon truncation, and the over-30-characters fallback.
9. The two colour constants still equal the `--tali-bg` values in the bundled
   `tokens.css` / `tokens-dark.css`, parsed from the compiled-in CSS at test time, so the
   duplication cannot drift silently.

## Documentation

A section in the `docs/guide` publishing/reference material covering the icon file-name
convention, what each browser does, and an explicit statement that installing does not
make the site available offline, with a pointer to the book download for that. Both
`docs/guide` and `docs/internals` become installable as a result, which dogfoods the
feature.

## Effort and risk

Small. One new core module of roughly 80 lines, head-tag wiring in `page.rs`, one emission
call in `build.rs`, three committed PNGs, one test file, one docs section.

Risk is low and the change is fully reversible: the manifest is a static file plus head
tags, so deleting the code makes the next build behave as though the feature never
existed. Nothing is registered in any reader's browser, no load-bearing invariant is
touched (the block model, `data-sourcepos`, the single editing surface and HTML-only output
are all unaffected), and the preview path is excluded by construction.

The one honest caveat on value: for a desktop reader, the zip plus an ordinary bookmark
already covers most of this, and the manifest adds a proper icon, a real app name, a
chrome-free window, and an install Chrome actively offers rather than one the reader has to
know how to trigger. The value concentrates on phones, which is precisely where the zip
fails.
