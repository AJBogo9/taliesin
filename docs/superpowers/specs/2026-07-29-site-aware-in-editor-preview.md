# Phase A2 — site-aware in-editor preview (backlog item 150)

**Status: DONE 2026-07-30.** The staleness half landed 2026-07-29; the wiring half (§1-4
below) landed 2026-07-30 and is kept here as the record of *why*. Three things came out
differently from the plan and are worth carrying forward:

- **§4's navigation is on the passive cursor path too**, not only on the explicit reveal.
  That does not reopen the yank the reveal/mark split exists to prevent: a cursor in the page
  already on screen is `cursorTarget`'s first branch and never navigates, so "scroll to
  compare two figures, then type one character" still leaves the preview alone. Only a cursor
  in a page the preview is *not showing* moves it.
- **A fifth thing needed fixing, and it was not in the spec:** `data-tali-src` (nav, footer,
  sidebar, listing cards) is **site-root-relative**, and the webview branch that handles it
  sent no anchor at all. It was unreachable while the companion only previewed single files —
  `doc.root` is unset there — and §1 makes it reachable on every chrome Ctrl-click. Verified
  on a nested chapter, where the two anchors differ: with it, `docs/guide/_site.yml`; without
  it, `docs/guide/using/_site.yml`, which does not exist.
- **The panel is now titled for the project**, not the chapter it was opened at, because it
  no longer stays on that chapter.

## The problem

Opening a book chapter in the companion previews the single file, so the author gets an
orphan page: no nav, no breadcrumb, and every cross-page link dead. The preview a book
author actually wants is `taliesin preview <projectDir>` opened at that chapter's URL —
which the server has supported all along.

## Ground truth (measured 2026-07-29, not assumed)

These were verified against the tree before the design below was written. Each is the kind of
fact the backlog warns rots, so each carries how to re-check it.

| Fact | Verified how |
|---|---|
| `taliesin map <dir> --format json` emits `pages[]` with `rel` **and** `url` per page | ran it on `docs/guide`; first entry `{"rel":"index.tmd","url":"index.html",…}` |
| `serve_site` emits `window.TALIESIN_DOC = { path, baseDir, root }` **per page** | `crates/server/src/serve_site/mod.rs:789` |
| `data-source-file` is defined relative to the **primary document's directory** | `crates/core/src/includes.rs:665` and its named test |
| the preview client already holds `doc.baseDir` where it posts `tali-goto` | `web-client/client.js`, both `postMessage` sites |
| the companion spawns `taliesin preview <file>` and points the iframe at `/` | `editor/vscode/src/server.ts`, `src/extension.ts` |

## What is done

**The anchor now travels with the message.** `source_file` is relative to the *currently
loaded page's* directory. The host was resolving it against `dirname(docPath)` — the document
the preview was **opened for** — which is correct only until the webview navigates. The
moment a site preview follows a cross-page link, a click on chapter B resolved against
chapter A's directory; with the book convention of one `index.tmd` per chapter directory that
does not error, it **opens a real file that is the wrong one**.

- `web-client/client.js` adds `base_dir` + `doc_path` (read from `window.TALIESIN_DOC`) to
  both `tali-goto` posts.
- `resolveSourceFile(docPath, sourceFile, anchor?)` prefers the anchor and falls back to the
  old behaviour when it is absent, so an older preview client against a newer host, or the
  reverse, behaves exactly as before.
- `projectRootFor(docPath, exists?)` implements the resolution rule: nearest ancestor with a
  `_site.yml`, **never `.git`** (a repository boundary is not a project boundary — item 70).
  `exists` is injected so the walk is testable without a filesystem.
- Four unit tests, mutation-checked (ignoring the anchor fails the named test).

This is useful on its own and carries no behaviour change for a single-file preview.

## What is left

### 1. Spawn the project, not the file

In `openPreview`, compute `projectRootFor(docPath)`. When it is non-null, start
`taliesin preview <root>` instead of `taliesin preview <file>` and open the iframe at that
page's URL rather than `/`.

`PreviewServer.start` currently takes a file and uses `dirname(file)` as cwd; it needs to
take the *target to serve* plus the cwd separately, since for a project they differ.

### 2. Find the page's URL

Run `taliesin map <root> --format json` once at preview start and look up the entry whose
`rel` matches the document's path relative to `root` (POSIX separators). Its `url` is the
iframe path. **TS must not derive the URL itself** — the `.tmd`→`.html` mapping, book chapter
numbering and `index` handling all live in Rust, and a second implementation is what the LSP
rewrite existed to delete.

Failure modes to handle explicitly, none of which should lose the preview:
- `map` fails or is not valid JSON → fall back to today's single-file preview.
- the document is a **draft** or is otherwise not in `pages[]` → fall back likewise; a page
  the project does not publish has no URL to open.
- the document is an `{{< embed >}}`-referenced **deck**, which is deliberately kept out of
  `site.pages` → same fallback.

### 3. Registry keying

`PreviewRegistry` is keyed by document path, and one project preview now serves many
documents. Key it by **project root** when there is one, so opening a second chapter of the
same book reveals the existing preview (navigated to that chapter) instead of spawning a
second server on a second port. `previewFor` must map a buffer to the preview whose root
contains it, before its current "the only open preview" fallback.

### 4. Reverse sync must select the page, then the block

`relativeKey(docPath, editorPath)` has the mirror of the staleness problem: it computes the
cursor's key relative to the opened document, so moving the cursor into another chapter sends
a key the shown page cannot match, and the mark silently lands nowhere.

The page-side fix is symmetrical to the one already built: the preview client should announce
the loaded page (a `tali-page` message carrying `baseDir`/`doc_path` on load and after each
navigation), the host caches it per preview, and `relativeKey` keys off that. When the cursor
moves to a document belonging to a *different* page of the same project, the host should
first navigate the iframe to that page's URL (from the map in §2) and only then send
`tali-cursor`.

Navigation itself needs a host→iframe message, since the host cannot set
`iframe.contentWindow.location` cross-origin: add a `tali-navigate` the relay in
`webview.ts` forwards, handled by the preview client.

## Verification the implementation owes

- Unit: registry keyed by root reuses one server across two chapters of one book; a document
  outside any project still gets a single-file preview.
- e2e (`npm run test:e2e`, which does run headless here despite the README): open a
  `docs/guide` chapter, assert the preview shows nav; click-to-source from a *different*
  chapter opens that chapter's file, not the opened one.
- Manual, because click-to-source has no end-to-end automated coverage (the harness stops at
  the relay): Ctrl-click a block in chapter B of a book preview and confirm the editor lands
  in chapter B's source.

## Non-goals

- No preview→source **writing**: this stays a navigation bridge (the single-editing-surface
  invariant).
- No new user-facing setting. Whether a document is in a project is a fact about the tree,
  not a preference; if the inference is ever wrong the fix is the inference or an
  `_site.yml`, not a knob.
