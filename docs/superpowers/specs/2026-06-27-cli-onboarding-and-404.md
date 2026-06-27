# CLI onboarding + the 404 build clobber (lane-e-cli)

Date: 2026-06-27
Branch: `lane-e-cli`
Owner files: `crates/server/src/main.rs`, `crates/core/src/site/` (additive, 404-only),
`README.md`, plus a minimal `closest()` re-export from `crates/core`.

## Problem

Two release-hardening gaps:

1. **Site `build` clobbers the author's `404.qmd`.** A multi-page site build
   discovers every `.qmd` (so a root `404.qmd` renders to `404.html` *and* is indexed
   into the Cmd-K search index), and then the build path **unconditionally** writes the
   built-in 404 template to `out/404.html` — overwriting the author's page. So an author
   who writes a custom 404 loses it, and the original leaks into search.

2. **Near-zero onboarding.** No README install/prereqs section, no scaffold command, the
   `<dir>` = site capability is hidden in `usage()`, there is no docs URL, and an
   unknown subcommand prints usage with no "did you mean" hint.

## Design

### 1. Honor the author's `404.qmd`

The author's 404 page, if present, is a root-level `404.qmd` whose output URL is
`404.html`. It is *already* rendered to `out/404.html` by the normal page loop. The fix
is purely about not clobbering it and not indexing it:

- **`Site::has_author_404()`** (new, `crates/core/src/site/mod.rs`): true when some
  discovered page has `url == "404.html"`. Localized, additive.
- **Build path (`build_site_async` in `main.rs`)**: only write the built-in
  `render_404_page()` template when `!site.has_author_404()`. The author's `404.qmd`
  flows through the normal page loop unchanged.
- **Search index (`search::build_index_json`)**: skip a page whose `url == "404.html"`
  so the not-found page never appears in Cmd-K results. (A 404 is navigation chrome, not
  content.)

Nav is already safe: the navbar is built from explicit `_site.yml` `nav:` items, not
auto-discovered pages, so a `404.qmd` never appears in nav unless the author opts in.

The preview server already serves `render_404_page()` only on a genuine miss *after*
the asset lookup; once the author's `404.qmd` builds to `404.html` it is reachable as a
normal page, and the built-in template still backs true misses — no preview change
needed.

### 2. Onboarding

- **README install/prereqs section**: build from source (`cargo build --release` /
  the `qmd-fast` launcher), Jupyter-kernel prereqs (`{python}` → ipykernel, `{r}` →
  IRkernel), and the `QMD_FAST_*` env vars (PYTHON, R, CELL_TIMEOUT, NO_CACHE).
- **`qmd-fast init [dir]`**: scaffold a minimal single-page **site** so a new user can
  immediately `qmd-fast preview <dir>`. Writes `_site.yml` + `index.qmd` (refuses to
  overwrite existing files; creates the dir if missing). Wired into the `main()`
  dispatch as a new arm.
- **`usage()`**: advertise `<dir>` = site for `preview`/`build`, add the docs URL in the
  banner, add the new `init` line, and keep the one-line-per-subcommand help.
- **Unknown-command did-you-mean**: on an unrecognized subcommand, suggest the nearest
  valid command via the existing core Levenshtein `closest()` helper (re-exported as
  `qmd_fast_core::closest`).

## Tests (TDD)

In `crates/server/src/main.rs` `mod mirror_tests` (appended at the end):

- `init_scaffolds_a_previewable_site` — `scaffold_init` into a temp dir writes
  `_site.yml` + `index.qmd`, and refuses to overwrite an existing file.
- `closest_command_suggests_nearest` — `"biuld"` → `Some("build")`, a far string →
  `None`.

In `crates/core/src/site/mod.rs` tests (appended):

- `author_404_is_honored_and_excluded_from_search` — a site with a root `404.qmd` sets
  `has_author_404()` and the search index JSON contains no `"u":"404.html"` entry; a
  site without one reports `false`.

## Out of scope / handoff

- `docs/guide/reference/cli.qmd` is owned by another lane this round; `init` is
  documented in README instead. **Coordinator: add `init` to `cli.qmd`.**
- No change to `collect_diagnostics` / `cmd_check` or their tests (another lane).
