# DX10-followup — `new deck --tour`, a guided teaching deck

Date: 2026-07-18. Backlog item **DX10-followup** (§6 DX audit batch, Tier 2 — the 4th
sub-part of DX10 "scaffolds that teach", split out and blocked on DX5, now unblocked).
Branch `dx10-followup-deck-tour`. Detail source: `notes/2026-07-18-dx-audit.md` (row 10);
ROADMAP Pillar II item 10.

> **Autonomy note:** author asked me to continue without the interactive gate. The design
> fork (tour content scope, `--tour` on non-deck kinds, how it pins) is resolved below with
> documented defaults, matching the DX5/DX11 pattern, for async review.

## Goal

`taliesin new deck <slug>` today scaffolds a bare 2-slide deck ("A point worth making").
The single most-delightful discovery in Taliesin decks (fragments, incremental reveal,
side-by-side columns, live code magic-move, speaker notes) is invisible until the author
reads the reference. **`new deck --tour`** scaffolds a *guided* deck instead: one slide per
deck feature, each demonstrating the feature **and** explaining it in one line, so the
scaffold itself teaches. The scaffold-as-teacher principle (DX10): the best documentation is
a correct, runnable example the author edits in place.

DX5 unblocked this: a tour must show the columns idiom, and until DX5 `::: {.columns}`
silently stacked (reveal muscle-memory degraded). Now `.columns` aliases to the
`layout-ncol` grid, so a tour can teach the idiom that works.

## Ground truth (grepped + measured against the running product 2026-07-18)

- **The scaffolder is a pure function** ([`new_files`, `cli.rs:257`](../../../crates/server/src/cli.rs)):
  `(kind, slug, today, opts) -> Vec<(PathBuf, String)>`. The `Deck` arm
  ([`cli.rs:357`](../../../crates/server/src/cli.rs)) emits the bare 2-slide deck. `opts`
  is [`NewOpts`](../../../crates/server/src/cli.rs) (`{ draft: bool }` today), `Default` =
  today's behaviour so an unflagged scaffold stays byte-identical.
- **Flag plumbing is in place.** [`cmd_new`, `cli.rs:384`](../../../crates/server/src/cli.rs)
  parses `--dir`/`--json`/`--draft` and sets `opts`; [`NEW_FLAGS`, `cli.rs:462`](../../../crates/server/src/cli.rs)
  drives the unknown-flag did-you-mean. Adding `--tour` is one `NewOpts` field + one
  `NEW_FLAGS` entry + one match arm (exactly the shape `--draft` took).
- **Ran the current product:** `new deck sample` → the bare deck; `check` on it → "no
  problems found". Baseline confirmed.
- **Canonical deck syntax** (pinned by [`corpus/deck.tmd`](../../../corpus/deck.tmd), read
  verbatim): fragments `::: {.fragment}`; a pause `. . .`; incremental `::: {.incremental}`
  around a bullet list; magic-move `::: {.magic-move}` around two plain code blocks; speaker
  notes `::: {.notes}`; a `#` (h1) starts a vertical section, `##` a slide. Columns use the
  DX5 alias `::: {.columns}` with `.column` children (shown in
  [`corpus/media/gallery.tmd:24`](../../../corpus/media/gallery.tmd)).
- **Keys verified from the engine, not from memory** (the deck-audit's key-sheet-drift
  lesson): `s` opens speaker view ([`deck.js:1248`](../../../crates/core/assets/js/deck.js)),
  `?`/`m` opens the control menu whose key sheet lists Speaker view / Fullscreen / Overview
  ([`deck.js:1681`](../../../crates/core/assets/js/deck.js)). So the tour may teach "`?` for
  the key sheet, `s` for speaker view" truthfully.
- **How scaffolds are pinned.** `corpus/scaffold/` holds one fixture per `new` kind
  (`my-talk.tmd` = the deck), rendered + linted by the corpus regression net like any
  document; `crates/server/tests/new_cli.rs` asserts each `new` output is `check`-clean and
  carries the right content. No automated byte-compare between `new_files` and the fixture
  was found, so the fixture is a rendered-and-linted mirror kept in sync by hand — plus the
  behavioural `new_cli.rs` test.

## Resolved decisions (autonomous, documented)

1. **`--tour` is deck-only; using it elsewhere is a friendly error.** `--tour` scaffolds a
   *deck* tour; `new post --tour` / `new paper --tour` have no meaning. Rather than silently
   ignore it (the silent-degradation trap this whole DX batch fights), `cmd_new` rejects it:
   `--tour scaffolds a guided deck; use it with \`new deck <slug>\``. (`--draft` stays
   universal; `--tour` is the first kind-specific flag.)
2. **Tour content = the five named features + presenter essentials + a "make it yours"
   closer, ~7 slides.** The roadmap names fragments / incremental / columns / magic-move /
   notes. Add the navigation basics (one slide per `##`, arrow/swipe, `?`, `s`) on the title
   slide and a `. . .` pause on the fragments slide (both one line, high teaching value). A
   closing slide tells the author to delete the tour and how (`##` = slide, `#` = section).
   **YAGNI:** no images (would need a `media/` asset), no `{js}`/`{{< input >}}` reactive
   slide (adds runtime surface), no auto-animate / backgrounds / code-line-numbers (bloat).
   The tour is a *starting point the author edits*, not the exhaustive feature catalog
   (`corpus/deck.tmd` already is that). Each demoed feature carries a one-line teaching
   sentence; teaching is inline (no doc links, which rot).
3. **Composes with `--draft` and `--json`.** The tour front matter splices the same `{draft}`
   line; `--json` reports the one written file as usual. No special handling.
4. **The default `new deck` (no `--tour`) is byte-unchanged.** The tour is a new branch taken
   only when `opts.tour`; the existing `Deck` arm, `corpus/scaffold/my-talk.tmd` pin, and the
   "default scaffold unchanged" tests are untouched.
5. **Dependency-free + `check`-clean.** No external refs, images, citations, or xrefs, so no
   broken-ref / missing-asset warning; every `:::` class is a known deck feature (DX5's
   `DIV_FEATURE_CLASSES`), so no did-you-mean fires. The code blocks are plain ` ```python `
   (highlighted, not executed), so no kernel is needed to preview.

## The tour deck (the emitted content)

Front matter carries `title` (from slug), the optional `{draft}` splice, `subtitle`, and
`format: deck`. Seven slides:

1. **Welcome** — one slide per `##`; arrow/swipe to move; `?` key sheet, `s` speaker view;
   "edit this file and the preview re-renders the changed slide; delete these when ready".
2. **Reveal one thing at a time** — a `. . .` pause + a `::: {.fragment}` block.
3. **Build a list step by step** — `::: {.incremental}` around a bullet list.
4. **Show two things side by side** — `::: {.columns}` with two `::: {.column}` children (the
   DX5 idiom), each with a one-line "what columns are for".
5. **Refactor code live** — `::: {.magic-move}` around two `python` code blocks, "the first
   morphs into the second as you step".
6. **Speak from notes only you can see** — "press `s` for speaker view" + `::: {.notes}`.
7. **Make it yours** — delete the tour; `##` = slide, `#` = section (down-arrow to descend).

The exact bytes are authored in the plan and mirrored to `corpus/scaffold/deck-tour.tmd`.

## Changes

### `crates/server/src/cli.rs`
- `NewOpts`: add `pub(crate) tour: bool` (Default `false`).
- `NEW_FLAGS`: add `"--tour"`.
- `cmd_new`: add a `"--tour" => opts.tour = true` arm; after `kind` is parsed, reject
  `opts.tour && kind != NewKind::Deck` with the friendly deck-only error.
- `new_files`: in the `Deck` arm, branch on `opts.tour` — the guided tour body when set, the
  existing bare body otherwise (default path byte-unchanged).

### `corpus/scaffold/deck-tour.tmd`
- The mirrored tour fixture, so the corpus regression net renders + lints it (the capability
  pin: every demoed deck feature renders clean).

### `crates/server/tests/new_cli.rs`
- `new_deck_tour_scaffolds_a_guided_deck`: `new deck t --tour` succeeds, writes `t.tmd`,
  `check` is clean, and the body contains each feature's syntax (`.fragment`, `.incremental`,
  `.columns`, `.magic-move`, `.notes`) + `format: deck`.
- `tour_is_rejected_on_a_non_deck_kind`: `new post p --tour` fails with a message naming
  `--tour` and `deck`; nothing is written.
- (Optional drift guard) compare the emitted `t.tmd` bytes to `corpus/scaffold/deck-tour.tmd`
  (read via `CARGO_MANIFEST_DIR/../../corpus/...`) so the fixture and the CLI can't diverge.

## Verification

- `cargo test -p taliesin-core -p taliesin-server` (the new tests + the whole net; the new
  corpus fixture is rendered + linted), `cargo fmt --check`, `cargo clippy -D warnings`.
- **Browser check (this is UI):** `taliesin preview` the tour deck; via chrome-devtools
  confirm each feature actually works — fragments reveal on step, the incremental list builds,
  the two columns lay out side-by-side (not stacked — the DX5 payoff), magic-move morphs the
  code, and `::: {.notes}` is hidden from the audience view. Console clean.

## Non-goals

- **Reactive / media slides** (`{js}`, `{{< input >}}`, images) — added runtime/asset surface
  for a starter scaffold; the reference deck already showcases them.
- **A `--tour` for other kinds** — deck-only by design (decision 1).
- **Exhaustive feature coverage** — `corpus/deck.tmd` is the catalog; the tour is a curated
  starting point.

## Invariant safety

Scaffolder + one corpus fixture only. No render-pipeline change, no output-format change, no
CDN, no preview write-back. The default `new deck` output is byte-identical (new branch is
`--tour`-gated); `data-block-id`/`data-sourcepos`, `MAX_WARM_PAGES` + `exec_pool.rs` LRU all
untouched. The tour uses only existing, already-pinned deck constructs.
