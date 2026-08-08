# A project is what `_site.yml` declares

Date: 2026-08-08. Status: design approved, not yet implemented.

## Problem

Taliesin infers that any directory of `.tmd` files is a website, and wraps a lone
document in synthesized site chrome. Neither is declared by the author, and the result
is a header that navigates nowhere and a preview that disagrees with the build.

### Measured, in this repo

`corpus/agent/` holds two independent fixtures and no `_site.yml`. Previewing it or the
document inside it produces this header:

```html
<header class="tali-site-nav" data-tali-src="_site.yml">
  <nav class="tali-nav-inner" aria-label="Primary">
    <a class="tali-nav-brand" href="executed-read.html">Home</a>
    <button class="tali-nav-burger" aria-label="Menu" aria-controls="tali-nav-links">…</button>
    <div class="tali-nav-links">
      <span class="tali-nav-spacer"></span>
      <button class="tali-search-btn" aria-keyshortcuts="Control+K Meta+K">…</button>
```

Four things in six lines are wrong for a document that is not part of a site:

1. The brand link points at the page you are already on (`href="executed-read.html"`).
2. It is labelled "Home", a generic fallback, not the document's title.
3. The burger opens a nav list with no nav links in it.
4. `data-tali-src="_site.yml"` credits a file that does not exist. The same run warns
   `no _site.yml at corpus/agent`.

**Preview and build disagree about the same file.** Measured by grepping the emitted
HTML for `tali-site-nav`:

| invocation | site chrome |
| --- | --- |
| `build corpus/agent/executed-read.tmd` | no |
| `build … --out <dir>` (portable folder) | no |
| `build corpus/agent/` (directory) | yes |
| `preview corpus/agent/executed-read.tmd` | **yes** |
| `preview corpus/agent/` (directory) | yes |

Rows 1 and 4 are the same document. The preview shows a header the build will never
produce, which breaks the premise that a preview previews the built page.

**A second silent wrong thing.** `Site::discover_scoped` takes its argument as the
project root and never ascends (`crates/core/src/site/mod.rs`). So
`taliesin build corpus/tech-blog/posts/` today builds those eight posts as their own
detached site: no blog nav, no cross-page links home, no listing that owns them. Nothing
reports this.

**Four verbs disagree about what a bare directory means:**

| verb | behavior on a directory with no `_site.yml` |
| --- | --- |
| `read` | refuses, with guidance (`crates/server/src/query.rs`) |
| `build` | warns, builds a site anyway |
| `preview` | warns, serves a site anyway |
| `check` | suppresses the warning entirely (`crates/server/src/check.rs`) |

The refusal is therefore not a new stance. `read` already takes it, pinned by
`read_of_a_non_site_directory_is_rejected_with_guidance` in
`crates/server/tests/read_book.rs`.

### Why this is worth fixing

The author of this repo could not tell, from the tool's behavior, whether `corpus/agent/`
was a book or two unrelated single pages. The tool could not tell either. It guessed
"website" and rendered navigation for a site nobody declared.

Every comparable tool declares the project type instead of inferring it, and treats
"no config" as "standalone document" rather than "website":

| tool | how the type is declared | what "no config" means |
| --- | --- | --- |
| Quarto | `_quarto.yml` → `project: type: website \| book \| manuscript \| default` | `quarto render doc.qmd` gives bare standalone HTML |
| mdBook | `book.toml` plus a `SUMMARY.md` listing chapters | hard error; it refuses to guess |
| Sphinx | `conf.py` plus an explicit `toctree` | not a project |
| Docusaurus | `sidebars.js` declares doc ordering | not a project |
| Hugo, Jekyll | config file required; "site" is the only concept | not a project |
| Pandoc | nothing; single document is the whole model | standalone document |

Quarto's `type: default` names exactly the thing Taliesin cannot express: a folder of
documents rendered individually, with no shared navigation.

## Design

### The rule

**A directory is a project, and a project is what `_site.yml` declares.**

Verbs that **render** a project (`build`, `preview`) require an `_site.yml` when handed
a directory. Verbs that **analyse a file set** (`check`, `map`, `features`) keep working
on any directory: they emit no chrome, so they have no project to be wrong about.

That line is load-bearing rather than arbitrary. The existing `read` error points the
author at `taliesin map <path>` for a bare directory's outline, so `map` must stay
permissive or that guidance breaks.

`read` is unchanged; it already refuses.

### Change 1: a directory with no `_site.yml` is an error for `build` and `preview`

The message names both fixes. When
[`walk_up_for_site_yml`](../../../crates/core/src/site/mod.rs) finds an enclosing
project, the message leads with that instead, because it is nearly always what the
author meant:

```
error  corpus/agent/ has no _site.yml, so it is not a project.
       to preview one document:   taliesin preview corpus/agent/executed-read.tmd
       to make it a site or book: add a _site.yml
```

```
error  corpus/tech-blog/posts/ has no _site.yml.
       its ancestor corpus/tech-blog is a project. did you mean:
         taliesin preview corpus/tech-blog
```

The helper already exists and is used for file targets, so this reuses the one walk both
spellings share rather than adding a second.

This also removes the dead end that prompted the work: `preview corpus/agent/` currently
serves a 404 page at `/` whose only link ("Back to the site") points at `/`, and which
mounts neither the live client nor the dev menu the CLI just told the author to open. An
actionable error is strictly better than a page with no way out.

### Change 2: a standalone document renders no site header

`Site::discover_single` stays. It is what makes link rewriting, asset resolution and deck
serving work for a lone file, and removing it would orphan those. What changes is that a
**synthesized one-page project emits no `<header class="tali-site-nav">`**.

The target is not a matter of taste, because `build <file>` already defines it. Chrome
inventory of `build corpus/agent/executed-read.tmd`, by grep count:

| marker | count |
| --- | --- |
| `tali-site-nav` | 0 |
| `tali-nav-brand` | 0 |
| `tali-nav-burger` | 0 |
| `tali-search-btn` | 0 |
| `tali-site-footer` | 0 |
| `tali-theme-toggle` | 2 |
| `tali-toc` | 0 |

A bare class-name grep counts CSS rules as well as markup, and `tali-toc` is a case in
point: `grep -c` reports 4 there, but all four are the bundled TOC stylesheet
(`.tali-toc-expanded`, `.tali-toc-active`, …), not emitted markup, so the row above is
the markup count instead. This is the same flaw `project_required.rs`'s live-preview
test already caught for `tali-site-nav`/`tali-site-footer` (a bare `contains` matches
their CSS rules too, chrome or not), so a bare substring grep should never be trusted
alone for this kind of measurement.

The footer matters as much as the header: `preview <file>` emits `tali-site-footer`
twice where `build <file>` emits it zero times. A site footer on a document that
belongs to no site is the same category of error as the "Home" link, so the standalone
gate covers `navbar_html` **and** `footer_html`.

A standalone document keeps the reader affordances (theme toggle, table of contents) and
drops every navigational element. The theme toggle staying is consistent with the
`CLAUDE.md` rule that reader-local a11y preferences are personal, not document config.
Cmd-K search is dropped, since `build` drops it.

The contract becomes: **`preview <file>` and `build <file>` produce the same chrome.**

Dropping the header also removes the self-linking "Home", the empty burger, and the
`data-tali-src="_site.yml"` attribution to a nonexistent file, as a consequence rather
than as four separate fixes.

## Testing

Three tests. The last is the one that would have caught the original bug.

1. **Refusal with guidance.** `build` and `preview` on an orphan directory exit non-zero
   and name both fixes. Mirrors the existing `read` test, so it should read like it.
2. **Ancestor hint.** `corpus/tech-blog/posts/` names `corpus/tech-blog` in the error.
3. **Preview equals build.** For a document with no ancestor `_site.yml`, the chrome
   markup `preview` serves matches what `build` writes. Asserting the absence of
   `tali-site-nav` in the preview response is the minimum; comparing the header region
   of both outputs is better.

`corpus/agent/` is already a natural orphan-directory fixture, so **no new corpus
document is required**. This is a behavior correction, not a new capability, so the
"ships pinned by a target corpus document" rule does not apply.

## Blast radius

Measured over `corpus/` and `docs/`, counting directories that hold `.tmd` files and no
`_site.yml` of their own:

| error form | count | examples |
| --- | --- | --- |
| plain message | 23 | `corpus/`, `corpus/agent`, `corpus/refs`, `corpus/posts/em-algorithm` |
| ancestor hint | 14 | 12 under `corpus/tech-blog`, plus `docs/guide/using` and `docs/guide/reference` |

None are built as directories by the test suite; the corpus regression sweep builds every
document as a file. A stale `corpus/posts/_site/` on disk shows the directory form has
been used by hand at least once, which is exactly the case the refusal is meant to catch.

`docs/` deserves separate mention. It holds no `.tmd` of its own, so it is not in either
count, but `taliesin build docs/` would refuse under this rule, and should. `website_pages`
walks recursively, so today that command sweeps the Guide and the Internals book into one
undifferentiated site. `CLAUDE.md` already warns that the books are siblings "because the
page-walker would otherwise swallow a nested book's pages"; the refusal turns that written
warning into an enforced one.

## Documentation to update

- `CLAUDE.md`: the `preview <file.tmd>` resolution paragraph currently documents
  `discover_single` as producing "a project of just that document", which stops being
  true of the rendered output.
- The user guide's `build` and `preview` reference pages.
- Check whether `MISSING_CONFIG_PREFIX` (`crates/core/src/site/config/mod.rs`) becomes
  dead once `build` and `preview` stop reaching it. `check` suppresses it, so it may have
  no remaining caller.

## Out of scope

- The `.tali-stretch` build defect found during the corpus browser test: an AVIF
  `<picture>` wrapper breaks the deck's direct-child stretch CSS, so a stretched raster
  image overflows the slide in `build` output while `preview` is correct. Separate fix,
  separate spec.
- Any change to `check`, `map` or `features`. Both `map` and `features` were confirmed to
  exit 0 on `corpus/agent/`, so the `read` guidance keeps working. One cosmetic wart is
  left behind deliberately: `map` labels a bare directory `(untitled) (site) → _site`,
  which is the same inferred-website assumption in a verb this change does not touch.
  Worth a follow-up, not worth widening this one.
- Adding an explicit `type:` key to `_site.yml`. Presence of the file is enough to
  separate the three cases today (`_site.yml` plus `chapters:` is a book, `_site.yml`
  alone is a website, neither is a standalone document), and `CLAUDE.md` asks for a
  better default before a new knob.
