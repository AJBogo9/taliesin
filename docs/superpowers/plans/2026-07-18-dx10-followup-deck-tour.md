# `new deck --tour` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** `taliesin new deck <slug> --tour` scaffolds a guided, `check`-clean deck that
demonstrates and explains each deck feature (fragments, incremental, columns, magic-move,
notes), so the scaffold itself teaches.

**Architecture:** One new `NewOpts.tour` field + one `NEW_FLAGS` entry + a `--tour` arm in
`cmd_new` (with a deck-only rejection) + a `--tour` branch in the pure `new_files` `Deck` arm.
The tour body is a raw-string const (`TOUR_SLIDES`) appended to an interpolated front matter,
so the many `::: {.feature}` braces need no format-string escaping. A `corpus/scaffold/`
fixture (generated from the binary) pins that the tour renders clean; a drift-guard test keeps
the fixture and the CLI in lockstep.

**Tech Stack:** Rust 2024; `crates/server/src/cli.rs` (bin crate); integration tests in
`crates/server/tests/new_cli.rs`; corpus fixture under `corpus/scaffold/`.

## Global Constraints

- **House style:** no em/en dashes in any emitted user-facing string.
- **Default scaffold byte-unchanged:** the no-`--tour` `new deck` output must not change (the
  new body is `opts.tour`-gated); `corpus/scaffold/my-talk.tmd` + the "default unchanged"
  tests stay green.
- **`check`-clean + dependency-free:** no images, citations, xrefs, `{js}`/`{{< input >}}`, or
  executed cells; every `:::` class is a known deck feature (DX5 `DIV_FEATURE_CLASSES`).
- **Server is a bin crate:** unit tests run via `cargo test -p taliesin-server --bin taliesin`;
  integration tests shell out through `env!("CARGO_BIN_EXE_taliesin")`.
- **The exact tour bytes** (front matter interpolates `title` from the slug + the optional
  `{draft}` splice; `TOUR_SLIDES` is the constant remainder):

```
---
title: "<Title From Slug>"
subtitle: "A guided tour of Taliesin decks"
format: deck
---

## Welcome to your deck

Every `##` heading starts a new slide. Use the arrow keys or swipe to move
between them; press `?` for the key sheet and `s` for speaker view.

- Edit this file and the preview re-renders the slide you changed
- Delete these tour slides when you write your own

## Reveal one thing at a time

Put a pause wherever you want to stop and talk.

. . .

Then keep going. A whole block can wait for its own step:

::: {.fragment}
This aside appears when you press forward.
:::

## Build a list step by step

::: {.incremental}
- First this point
- then this one
- and finally this
:::

## Show two things side by side

::: {.columns}
::: {.column}
The left column: a claim beside its evidence, or a before beside an after.
:::

::: {.column}
Writing `::: {.columns}` with `.column` children lays them out side by side.
:::
:::

## Refactor code live

::: {.magic-move}
```python
def area(r):
    return 3.14 * r * r
```

```python
import math

def area(r):
    return math.pi * r ** 2
```
:::

The first version morphs into the second as you step forward.

## Speak from notes only you can see

Press `s` for speaker view: your notes, a timer, and the upcoming slide.

::: {.notes}
Only the presenter sees this. Put your talking points here.
:::

## Make it yours

Replace these slides with your talk. Each `##` starts a slide; a single `#`
starts a new section you drop into with the down arrow.
```

---

### Task 1: `--tour` scaffolder + deck-only rejection

**Files:**
- Modify: `crates/server/src/cli.rs` (`NewOpts`, `NEW_FLAGS`, `cmd_new`, `new_files`)
- Test: `crates/server/tests/new_cli.rs`

**Interfaces:**
- Produces: `NewOpts { draft: bool, tour: bool }`; `new_files(kind, slug, today, opts)`
  emits the tour body when `kind == Deck && opts.tour`; `cmd_new` exits FAILURE on
  `opts.tour && kind != Deck`.

- [ ] **Step 1: Write the failing tests** (append to `crates/server/tests/new_cli.rs`)

```rust
/// `new deck --tour` scaffolds a guided deck: it demonstrates each deck feature and stays
/// check-clean, so the scaffold itself teaches (DX10 "scaffolds that teach").
#[test]
fn new_deck_tour_scaffolds_a_guided_deck() {
    let dir = tmp("tour");
    let (ok, stdout, stderr) = run(&[
        "new", "deck", "my-talk", "--tour", "--dir", dir.to_str().unwrap(),
    ]);
    assert!(ok, "`new deck --tour` should succeed; stderr: {stderr}");
    let written = dir.join("my-talk.tmd");
    assert!(written.exists(), "writes my-talk.tmd; stdout: {stdout}");
    let src = std::fs::read_to_string(&written).unwrap();
    assert!(src.contains("format: deck"), "it is a deck: {src}");
    for feature in ["::: {.fragment}", "::: {.incremental}", "::: {.columns}", "::: {.column}", "::: {.magic-move}", "::: {.notes}"] {
        assert!(src.contains(feature), "tour demonstrates `{feature}`:\n{src}");
    }
    let (clean, diagnostics) = check_is_clean(&written);
    assert!(clean, "a fresh --tour deck must check clean, got:\n{diagnostics}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--tour` scaffolds a deck; using it on another kind is a friendly error (not a silent
/// no-op), naming the flag and the deck kind. Nothing is written.
#[test]
fn tour_is_rejected_on_a_non_deck_kind() {
    let dir = tmp("tour-wrong");
    let (ok, _, stderr) = run(&[
        "new", "post", "wip", "--tour", "--dir", dir.to_str().unwrap(),
    ]);
    assert!(!ok, "`new post --tour` must fail");
    assert!(
        stderr.contains("--tour") && stderr.contains("deck"),
        "the error names the flag and the deck kind: {stderr}"
    );
    assert!(
        !dir.join("posts/wip/index.tmd").exists(),
        "a rejected --tour writes nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --test new_cli tour`
Expected: FAIL (`--tour` is an unknown flag today → both `new deck --tour` and
`new post --tour` error, so the deck test fails on `ok`, and the rejection test may pass for
the wrong reason — the unknown-flag path, not the deck-only guard). That is fine for RED;
Step 4 makes the deck test pass and the rejection test pass for the *right* reason.

- [ ] **Step 3: Implement in `crates/server/src/cli.rs`**

Add the `tour` field to `NewOpts`:

```rust
#[derive(Clone, Copy, Default)]
pub(crate) struct NewOpts {
    /// `--draft`: mark the scaffold `draft: true`, holding it out of the published build.
    pub(crate) draft: bool,
    /// `--tour`: scaffold a *guided* deck (one slide per feature, each explained) instead of
    /// the bare starter. Deck-only; `cmd_new` rejects it on any other kind.
    pub(crate) tour: bool,
}
```

Add the `TOUR_SLIDES` const (place it just above `new_files`). Use a raw string so the
`:::`-brace content needs no escaping:

```rust
/// The guided-tour deck body (everything after the front matter): one slide per deck feature,
/// each with a one-line teaching sentence, demonstrating fragments / a pause / incremental /
/// columns (the DX5 alias) / magic-move / speaker notes. A raw literal (not a format string),
/// so the many `::: {.feature}` braces and the ```code fences need no escaping. Kept in sync
/// with `corpus/scaffold/deck-tour.tmd` by the drift-guard test in `new_cli.rs`.
const TOUR_SLIDES: &str = r####"
## Welcome to your deck

Every `##` heading starts a new slide. Use the arrow keys or swipe to move
between them; press `?` for the key sheet and `s` for speaker view.

- Edit this file and the preview re-renders the slide you changed
- Delete these tour slides when you write your own

## Reveal one thing at a time

Put a pause wherever you want to stop and talk.

. . .

Then keep going. A whole block can wait for its own step:

::: {.fragment}
This aside appears when you press forward.
:::

## Build a list step by step

::: {.incremental}
- First this point
- then this one
- and finally this
:::

## Show two things side by side

::: {.columns}
::: {.column}
The left column: a claim beside its evidence, or a before beside an after.
:::

::: {.column}
Writing `::: {.columns}` with `.column` children lays them out side by side.
:::
:::

## Refactor code live

::: {.magic-move}
```python
def area(r):
    return 3.14 * r * r
```

```python
import math

def area(r):
    return math.pi * r ** 2
```
:::

The first version morphs into the second as you step forward.

## Speak from notes only you can see

Press `s` for speaker view: your notes, a timer, and the upcoming slide.

::: {.notes}
Only the presenter sees this. Put your talking points here.
:::

## Make it yours

Replace these slides with your talk. Each `##` starts a slide; a single `#`
starts a new section you drop into with the down arrow.
"####;
```

In `new_files`, replace the `NewKind::Deck` arm so it branches on `opts.tour`:

```rust
        NewKind::Deck => {
            let body = if opts.tour {
                // A guided tour: interpolate the front matter, then the constant slides.
                format!(
                    "---\n\
                     title: \"{title}\"\n{draft}\
                     subtitle: \"A guided tour of Taliesin decks\"\n\
                     format: deck\n\
                     ---\n{TOUR_SLIDES}"
                )
            } else {
                format!(
                    "---\n\
                     title: \"{title}\"\n{draft}\
                     subtitle: \"A subtitle\"\n\
                     format: deck\n\
                     ---\n\
                     \n\
                     ## The first slide\n\
                     \n\
                     - A point worth making\n\
                     - Another one\n\
                     \n\
                     ## The second slide\n\
                     \n\
                     Each `##` heading starts a new slide.\n"
                )
            };
            (PathBuf::from(format!("{slug}.tmd")), body)
        }
```

In `cmd_new`, add the flag parse arm (beside `--draft`):

```rust
            // `--tour` scaffolds a guided deck (deck-only; rejected below on other kinds).
            "--tour" => opts.tour = true,
```

Add `"--tour"` to `NEW_FLAGS`:

```rust
const NEW_FLAGS: &[&str] = &["--dir", "--json", "--draft", "--tour"];
```

Add the deck-only rejection in `cmd_new`, right after `kind` is resolved (after the
`NewKind::parse` block, before `validate_slug`):

```rust
    // `--tour` scaffolds a *deck* tour; on any other kind it has no meaning. Reject it
    // (a friendly error, not a silent no-op) before writing anything.
    if opts.tour && kind != NewKind::Deck {
        log::error("--tour scaffolds a guided deck; use it with `new deck <slug>`");
        return ExitCode::FAILURE;
    }
```

(Requires `NewKind` to derive `PartialEq`; if it does not already, add `PartialEq` to its
`#[derive(...)]`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --test new_cli tour`
Expected: PASS (both tests). Then the full new_cli suite: `cargo test -p taliesin-server --test new_cli` (the "default unchanged" + every-scaffold-clean tests stay green).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/cli.rs crates/server/tests/new_cli.rs \
        docs/superpowers/specs/2026-07-18-dx10-followup-deck-tour-design.md \
        docs/superpowers/plans/2026-07-18-dx10-followup-deck-tour.md
git commit -m "feat(new): new deck --tour scaffolds a guided teaching deck (DX10-followup)"
```

---

### Task 2: corpus fixture + drift guard

**Files:**
- Create: `corpus/scaffold/deck-tour.tmd` (generated from the binary, not hand-typed)
- Test: `crates/server/tests/new_cli.rs`

**Interfaces:**
- Consumes: Task 1's `new deck <slug> --tour` output.

- [ ] **Step 1: Generate the fixture from the built binary**

```bash
cargo build -p taliesin-server
TMP=$(mktemp -d)
target/debug/taliesin new deck deck-tour --tour --dir "$TMP" >/dev/null
cp "$TMP/deck-tour.tmd" corpus/scaffold/deck-tour.tmd
rm -rf "$TMP"
```

(The corpus regression net renders + lints every `corpus/**/*.tmd`, so this fixture becomes
the capability pin: the tour deck must render clean.)

- [ ] **Step 2: Add the drift-guard test** (append to `new_cli.rs`)

```rust
/// The `--tour` deck the CLI emits must stay byte-identical to the pinned
/// `corpus/scaffold/deck-tour.tmd` fixture (which the corpus net renders + lints), so the
/// two can never drift.
#[test]
fn tour_deck_matches_the_corpus_fixture() {
    let dir = tmp("tour-fixture");
    let (ok, _, stderr) = run(&[
        "new", "deck", "deck-tour", "--tour", "--dir", dir.to_str().unwrap(),
    ]);
    assert!(ok, "stderr: {stderr}");
    let emitted = std::fs::read_to_string(dir.join("deck-tour.tmd")).unwrap();
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/scaffold/deck-tour.tmd");
    let fixture = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", fixture_path.display()));
    assert_eq!(
        emitted, fixture,
        "`new deck --tour` drifted from corpus/scaffold/deck-tour.tmd; regenerate the fixture"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 3: Run the tests + the corpus net**

Run: `cargo test -p taliesin-server --test new_cli` then `cargo test -p taliesin-core`
Expected: PASS (drift guard green; the corpus net renders/lints the new fixture clean).

- [ ] **Step 4: Commit**

```bash
git add corpus/scaffold/deck-tour.tmd crates/server/tests/new_cli.rs
git commit -m "test(new): pin the --tour deck as a corpus fixture + drift guard"
```

---

### Task 3: full gate + browser verification

- [ ] **Step 1: Full gate**

Run: `cargo test -p taliesin-core -p taliesin-server`, `cargo fmt --check`,
`cargo clippy -p taliesin-server --bin taliesin -- -D warnings`.
Expected: all green.

- [ ] **Step 2: Browser check (this is UI)**

Rebuild (`cargo build -p taliesin-server`), scaffold a tour into the scratchpad, and
`taliesin preview` it. Via chrome-devtools confirm, at desktop width:
- the fragment + `. . .` pause reveal on step (not all at once);
- the incremental list builds one item per step;
- the two columns lay out **side by side** (the DX5 payoff), not stacked;
- magic-move morphs the code from v1 to v2;
- `::: {.notes}` is absent from the audience view (visible only in speaker view via `s`).
Console clean. Screenshot the columns slide as evidence.

- [ ] **Step 3: (no commit — verification only)**

---

## Self-Review

- **Spec coverage:** flag plumbing (Task 1), deck-only rejection (Task 1), tour content /
  five features + basics + closer (Task 1 `TOUR_SLIDES`), default-unchanged (Task 1 else
  branch + existing tests), corpus fixture capability pin (Task 2), drift guard (Task 2),
  browser verification of the DX5 columns payoff (Task 3). All spec sections map to a task.
- **Placeholder scan:** none — every step has exact code/commands.
- **Type consistency:** `NewOpts.tour: bool`, `TOUR_SLIDES: &str`, `NewKind::Deck`,
  `NewKind` needs `PartialEq` (flagged in Task 1 Step 3). `new_files` signature unchanged.
- **Risk/rollback:** additive; one `--tour`-gated branch + one fixture. `git revert` the two
  commits fully undoes it; default `new deck` untouched.
