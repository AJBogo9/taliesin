# DX14 — `init` templates + interactive `new`/`init` wizard

**Date:** 2026-07-19
**Status:** design approved (owner), ready for implementation plan
**Backlog:** DX audit batch §6, Tier 2, item DX14 (`notes/backlog.md`)

## Problem

`taliesin new` and `taliesin init` are flags-only today. Two gaps:

1. **No multi-page scaffolder.** `new` writes single documents (`post`/`page`/`deck`/`paper`);
   `init` writes a minimal *one-page* site. A user who wants a **book** (a `chapters:`
   project) or a **multi-page site** (nav + pages) has to hand-write `_site.yml` from scratch.
2. **No interactive path.** `taliesin new` with missing args dead-ends on a usage line;
   there is no guided way in.

## Decisions (owner-ruled during brainstorming, do not re-litigate)

- **Site/book are `init` templates, not `new` kinds.** `init` scaffolds a *project*; `new`
  scaffolds a *document into* a project. A book/site is a project. This keeps the clean split
  (`init` = new project, `new` = new page), matching `cargo new`.
- **Wizard uses real arrow-key navigation via a dependency (`dialoguer`).** The owner
  explicitly chose arrow-keys over a zero-dependency line-based prompt. Mitigations below keep
  it offline-safe and never triggered outside a human TTY.
- **Ship both `site` and `book` templates.** The `site` template is trimmed to index + about +
  nav (no blog listing / sample post).

## Half A — `init` gains templates

### CLI

```
taliesin init [dir] [--template basic|site|book] [-y|--yes] [--json]
```

- `[dir]` stays the first positional (default `.`), so `taliesin init myproj` is unchanged.
- `--template <t>` selects the starter. Omitted + non-interactive (or `-y`) ⇒ `basic`.
- `-y`/`--yes`: never prompt; use defaults (`basic` into the given/`.` dir). Preserves today's
  scripted behavior exactly.
- `--json`: unchanged receipt (`{created, preview}`); never prompts (machine output).

### Templates

Every template also emits the shared onramp files exactly as today: `AGENTS.md`
(`taliesin_core::agents::AGENTS_MD`) and `.taliesin/{tali-site,tali-frontmatter}.schema.json`
(the bundled schema constants), with the schema modeline as `_site.yml`'s first line. Every
scaffold is `check`-clean and refuses to overwrite any target before writing any.

- **basic** — *byte-identical to today.* `_site.yml` (modeline + `title: My site`) + `index.tmd`
  (the current hello-world). A drift pin guards the exact bytes.
- **site** (trimmed) — a multi-page site:
  - `_site.yml`: modeline + `title` + a `nav.left` of Home (`index.tmd`) and About (`about.tmd`).
  - `index.tmd`: home page explaining the multi-page/nav model.
  - `about.tmd`: a stub About page.
  - nav hrefs resolve to real pages ⇒ `check`-clean.
- **book** — a `chapters:` project:
  - `_site.yml`: modeline + `title` + `author` + `toc: true` + `chapters: [index.tmd, intro.tmd,
    methods.tmd]`.
  - `index.tmd`: a short landing/preface. The B2 book landing auto-TOC appends the chapter list
    automatically (no hand-written TOC).
  - `intro.tmd`, `methods.tmd`: two starter chapters (`# Heading` + prose, the book-chapter idiom).

### Structure

Refactor `scaffold_init` into a pure `init_files(template: InitTemplate) -> Vec<(PathBuf, String)>`
mirroring the existing `new_files`, so:

- the emitted bytes are corpus-pinnable and the CLI stays a thin wrapper;
- the "refuse to overwrite any before writing any, create parent dirs" writer is shared with
  `write_new` (extract a `write_scaffold(root, files)` helper both call).

`InitTemplate` is a small `enum { Basic, Site, Book }` with a `parse` that returns a
did-you-mean on an unknown value (mirroring `NewKind::parse`).

### Corpus pins (corpus-leads rule)

Add two real, buildable projects: `corpus/scaffold-site/` and `corpus/scaffold-book/`, each the
byte-exact **authored** output of its template (`_site.yml` + the `.tmd` pages; the generated
`AGENTS.md`/`.taliesin/*` are asserted equal to their source constants, not duplicated into the
corpus — matching how `new`'s pin omits them).

- `collect_qmd` (corpus.rs) already renders + lints every `.tmd` individually ⇒ each scaffolded
  page is regression-covered for free.
- A byte-compare test (extending `new_cli`-style coverage) asserts `init_files(t)` authored files
  match the corpus mirror, and the basic template matches its own drift pin.
- `Site::discover` tests (modeled on `corpus.rs:924 book_discovers_chapters...` and
  `tech_blog_site_discovers...`) assert the book's chapters resolve/number and the site's nav
  parses + links resolve.

## Half B — interactive wizard

### Trigger (ALL must hold; else today's exact behavior)

1. the command is under-specified (missing `kind`/`slug` for `new`; missing `--template` for
   `init`), **and**
2. `std::io::stdin().is_terminal()` (a human TTY), **and**
3. not `-y`/`--yes`, **and**
4. not `--json`.

If any fails: `new` prints its usage error (non-TTY / `-y`); `init` scaffolds `basic` (non-TTY /
`-y`). **This is the load-bearing safety property:** every existing test, CI job, pipe, and agent
runs non-TTY, so the wizard is never reached there and all current behavior is preserved.

### Prompts

- `taliesin new` (missing kind and/or slug):
  - `Select` the kind (post / page / deck / paper) — only if `kind` missing.
  - `Input` the slug — only if `slug` missing; validated with the existing `validate_slug`,
    re-prompting on an invalid entry (`dialoguer::Input::validate_with`).
- `taliesin init` (no `--template`):
  - `Select` the template (basic / site / book).
  - `Input` the dir (default `.`) — only if `dir` not already given.

`--draft`/`--tour` are NOT asked (they stay flags; YAGNI — the wizard fills only what blocks the
scaffold).

### `-y`/`--yes` semantics

"Never prompt; use defaults where one exists, else fail as today." `init -y` ⇒ basic into `.`
(today's fast path). `new -y` with no slug ⇒ usage error (no slug default exists).

### Isolation & testability

- A thin `crate::interactive` module holds every `dialoguer` call (the only untestable,
  side-effecting surface). It returns the resolved kind/slug/template/dir; the callers
  (`cmd_new`/`cmd_init`) then run the same pure scaffold path.
- The *decision logic* (what is missing, whether to prompt, defaults, validation) stays pure and
  unit-tested.
- A `Command`-driven integration test pins that a **non-TTY** `new` (stdin not a terminal) with
  missing args returns the usage error and does not hang, and non-TTY `init` scaffolds basic.
  Mutation check: removing the `is_terminal` gate changes this observable behavior.

### Dependency

`dialoguer = { version = "0.12", default-features = false }` added to
`[workspace.dependencies]`, consumed only by `taliesin-server`. `default-features = false` drops
`editor`/`password`/`tempfile`/`zeroize`/`fuzzy-*`; only `Select`/`Input` are used. Backend is
`console` (already in the registry cache) over `libc` termios (already a direct dep). No runtime
network. Justified by the owner's explicit arrow-key choice; a comment in `Cargo.toml` records the
rationale, matching the file's convention.

## Testing plan (TDD)

1. `init_files(Basic)` byte-equals the current `INIT_SITE_YML`/`INIT_INDEX_TMD` (drift pin) —
   write first, refactor `scaffold_init` to satisfy it.
2. `init_files(Site)` / `init_files(Book)` byte-equal the new corpus mirrors.
3. `InitTemplate::parse` unknown ⇒ did-you-mean (mirror `an_unknown_kind_suggests_the_nearest`).
4. `Site::discover(corpus/scaffold-book)` resolves chapters; `...scaffold-site` parses nav.
5. Corpus net (`collect_qmd`) renders + lints the new pages (front matter clean, no unknown keys).
6. `check`-clean for both scaffolded projects (integration, via the real binary).
7. Non-TTY missing-args behavior unchanged for `new` and `init` (integration).
8. CLI flag parsing: `--template`, `-y`/`--yes` accepted; unknown flags still did-you-mean.

Each new test is mutation-checked (mutate the code → watch the named test fail → revert). No
browser check (terminal-only); the wizard is exercised manually in a real PTY.

## Non-goals / YAGNI

- No wizard for commands other than `new`/`init`.
- No `--draft`/`--tour` prompting in the wizard.
- No blog-listing/sample-post in the `site` template (trimmed by owner).
- No config knobs beyond `--template` and `-y` (perfect the default before adding a knob).

## Load-bearing constraints honored

- **Offline:** no runtime network; the dep is offline. **Minimal config:** two flags, sensible
  defaults, non-interactive stays first-class.
- **HTML-only / single editing surface:** untouched (scaffolding writes source files the author
  then edits; the preview stays read-only).
- **Do-NOT-touch:** no change to `MAX_WARM_PAGES` / `exec_pool.rs` eviction.
