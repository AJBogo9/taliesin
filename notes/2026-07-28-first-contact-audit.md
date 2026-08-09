# R2 — first contact and cognitive walkthrough

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Date:** 2026-07-28
**Round:** Wave 3 / R2 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** What does a stranger hit in the first ten minutes, and where does the interface fail its
own users?

**Why novel here.** Every prior round began from the author's machine, the author's knowledge and a
warm environment. This one started from zero. Instruments: a real cold start, then Nielsen's ten
heuristics and a cognitive walkthrough of two goals.

**Environment.** Every command below ran as `env -i HOME=<tmp> PATH=/usr/bin:/bin`, i.e. **no
`TALIESIN_PYTHON`, no launcher on `PATH`, no prior config, no shell profile**, in an empty directory.
The binary was rebuilt from this branch first (`taliesin 0.2.0 (9e80286)`) after the shared
`target/` was found to hold a build from the concurrent branch. New items are numbered from **120**.

---

## Headline

**A stranger who follows the tool's own printed instructions, in the order the tool prints them,
reaches a broken result in under two minutes.** Browser-verified, not inferred.

```
$ taliesin init .
Scaffolded a Taliesin site. Preview it:
  taliesin preview .                       ← instruction 1

$ taliesin new deck my-talk                ← the scaffold's own "Next steps" suggests this
  built   ./my-talk.tmd
Preview it:
  taliesin preview ./my-talk.tmd           ← instruction 2, a DIFFERENT command

$ taliesin preview .                       ← the user follows instruction 1
  warn    my-talk.tmd: declares a revealjs deck but is a loose page in the site;
          it will render as a flat article. …
```

The deck renders as a flat web page. Screenshot confirms the failure is visible and self-contradicting:
the scaffolded slide content reads **"Each `##` heading starts a new slide"** while the page displays
every `##` stacked as one article. Meanwhile `taliesin preview ./my-talk.tmd` on the same file serves a
real deck (`class="tali-deck"`, `class="tali-slides"`, two `data-level="2"` slides, measured).

Nothing is broken in the engine. **Two scaffold commands print two different next-step commands, and
the tool does not reconcile them.**

---

## The cold-start timeline

| Step | Elapsed | Result |
|---|---|---|
| `taliesin --help` | 0:00 | 45+ lines before the first actionable command. Dense but genuinely complete |
| `taliesin init .` | 0:10 | 5 files written, clear next step. **Good** |
| read `index.tmd` | 0:20 | "Next steps" list is well judged: post/paper/deck, more pages, `_site.yml`, a code cell |
| `taliesin new deck my-talk` | 0:40 | deck scaffolded in the site root, prints a *different* preview command |
| `taliesin preview .` | 0:50 | **first warning**, and the deck is a flat article |
| first page paint | 0:50 | HTTP 200 in **1.9 ms**, 0 console errors |
| `taliesin check typo.tmd` | 1:30 | **excellent**: `unknown front-matter key 'authr' (did you mean 'author'?)` |
| `taliesin check cell.tmd` | 2:00 | **"no problems found"** — on a document whose only code cell cannot run |
| `taliesin build cell.tmd` | 2:10 | two clear warnings about the missing kernel, plus a fix |
| `taliesin doctor .` | 2:30 | **excellent**: exact interpreter, exact missing package, exact fix command |

**Where a stranger gives up: they do not.** No step is fatal and nothing crashes. The failure mode
here is not abandonment, it is **a wrong mental model formed in the first minute**: the user's first
deck does not look like a deck, and the tool's explanation names a format value the user never typed.

---

## Items

### 120. Following `init`'s instruction after `new deck` produces a warning and a flat article. (HIGH for first contact, LOW in engineering cost)

**Measured**, transcript above, browser-verified at `http://127.0.0.1:4390/my-talk.html`.

**Cognitive-walkthrough diagnosis.** Of the four walkthrough questions, this fails the fourth: the
user *will* try to preview, *will* notice the command, *will* recognise it as right (the tool printed
it), and then **will not understand the feedback** — because the feedback describes a configuration
they did not knowingly make, in vocabulary they have not seen.

**Nielsen heuristics violated:** #2 *match between system and the real world* (the message speaks
`revealjs`, the user typed `deck`), #5 *error prevention* (the tool created the bad state itself), #4
*consistency and standards* (two scaffolders, two preview idioms).

**The engine is not at fault and the site rule is correct.** A loose deck in a site genuinely would
flatten, and warning about it is right. The defect is that `taliesin new deck <slug>` inside a site
project creates exactly the state the site rule forbids, and says nothing.

**Fix, smallest version:** `new deck` should detect a `_site.yml` in the target directory and either
(a) say so — "this deck sits in a site project; reference it with `{{< embed my-talk.tmd >}}` from a
page, or preview it directly with `taliesin preview ./my-talk.tmd`" — or (b) scaffold the embedding
line into `index.tmd`. **(a) is preferred**: it is words, not a write to a file the user owns, and it
keeps the single-editing-surface spirit.

**Refuted if** `new deck` already warns inside a site project (it does not: measured, its entire
output is two lines).

### 121. The first-run warning names `revealjs`, a format value the parser rejects. (MEDIUM)

**Measured.** `site/mod.rs:391` emits "declares a **revealjs** deck". But `is_reveal_format`
(`fm_extract.rs:116-119`) accepts only `deck` or `*-deck`:

```rust
fn is_reveal_format(name: &str) -> bool {
    let n = name.trim().trim_matches(['"', '\'']);
    n == "deck" || n.ends_with("-deck")
}
```

So **`format: revealjs` would not produce a deck at all**, and the message names the one value that
cannot cause the condition it is reporting. Further measured: `revealjs` appears **zero** times in
`docs/`, so a stranger has nothing to look it up in; `retired_names.rs` does not police `reveal`; and
**26** `revealjs` references remain across `crates/*/src`.

This is a straight instance of the class Wave 1 and the concurrent critique round both found: **prose
that no gate compares against behaviour** (R7's F1, RPN 405).

**A related fixture is misleading, and the mutation says so.** `site/mod.rs:2125` writes
`format: revealjs` into `draft_on_an_embedded_deck_is_not_reported_as_unpublished`. Changing it to
`format: html` and running the named test:

```
test site::tests::draft_on_an_embedded_deck_is_not_reported_as_unpublished ... ok
```

The test passes either way, because deck discovery there runs through the `{{< embed >}}` reference,
not through the format value. **The test is genuine for what it tests**; the fixture just documents a
format that does not exist. (The mutation was reverted by inverse edit; `git diff` is clean.)

**Fix.** Reword to name what the user actually wrote (`format: deck`), and consider adding `reveal` to
`retired_names.rs` so the 26 remaining references cannot leak into another user-facing string.

### 122. `check` says "no problems found" on a document whose code cell cannot run, and the Environment section is shown only to a user who already knew to ask. (MEDIUM)

**Measured**, same cold environment, on a document whose only content is a `{python}` cell:

| Command | Output |
|---|---|
| `taliesin check cell.tmd` | `no problems found`, exit 0. **Nothing else.** |
| `taliesin check cell.tmd --require-kernel` | `no problems found` **+ `Environment (kernels not ready): python: python3, ipykernel MISSING` + `run 'taliesin doctor'`**, exit 1 |
| `taliesin build cell.tmd out.html` | two warnings naming the interpreter, the missing module and two fixes |

`check` is the command the project positions as "a green `check` means the document is publishable"
(`diagnostics/mod.rs:6-7`), and it is the pass an agent runs first on an unknown project (Wave 1's
due-diligence round). It reports a clean bill on a document that will not produce the output it was
written for, while `build` on the same file warns twice.

**Nielsen #1 (visibility of system status).** The information exists and is well written. It is gated
behind a flag that only a reader of `--help` would find, and the plain path gives no hint the flag
exists.

**Deliberately not proposed:** making `--require-kernel` the default. That would make `check` fail on
every machine without a kernel, which is exactly the kernel-free property the command exists to have.
**The proposal is the Environment section, unconditionally, whenever the document contains a code
cell** — informational, exit code unchanged. One line of output, no new knob, which is the shape this
project's "perfect the default" rule asks for.

**Refuted if** plain `check` prints an environment line on a cell-bearing document (measured: it
prints nothing).

### 123. `init` writes a 5 KB `AGENTS.md` into a stranger's new project, unasked and unexplained. (LOW)

`taliesin init .` writes five files. Three are obvious (`_site.yml`, `index.tmd`, `.taliesin/*.json`
schemas). The fourth is **`AGENTS.md`, 5,049 bytes**, an AI-assistant instruction file. The scaffolded
`index.tmd`'s "Next steps" does not mention it, and neither does `init`'s own output.

A user who does not use coding agents gets an unexplained file in the root of their new project on
their first command. A user who does gets a genuinely useful one with no idea it is there.

**Not a defect in the file** — the AI-native round shipped it deliberately and it is good. The defect
is a silent write. **Fix:** one line in `init`'s output naming it, or one bullet in the scaffolded
"Next steps".

---

## Heuristic evaluation — what came back clean

Recorded because a confirmation is a valid result and because three of these are unusually strong.

- **#9 Help users recognise, diagnose and recover — exceptional.** The front-matter typo rule prints
  `unknown front-matter key 'authr' (did you mean 'author'?)` with the line number. The kernel-absent
  warning names the interpreter, the exact missing module, *and* both fix routes
  (`TALIESIN_PYTHON` or `_site.yml python:`), then points at `doctor`.
- **`taliesin doctor` is the best first-contact artefact in the tool.** Interpreter, version, missing
  package, exact install command, virtualenv state, config validity, and a plain-English summary of
  the consequence ("python cells will render as source"). Nothing in this round improved on it.
- **#1 visibility of system status, in the preview** — the terminal prints `ready`, the URL, the watch
  root and the page count; the browser badge shows the diagnostic count. Both surfaced the loose-deck
  warning.
- **#7 flexibility and efficiency of use** — `preview` has `dev`/`serve` aliases, `--open`, `--host`
  with a QR code, and a default port that steps aside for a running instance.
- **Performance is a non-issue at first contact.** First paint HTTP 200 in **1.9 ms**; **zero** console
  errors or warnings on the scaffolded page.
- **Graceful degradation is real.** With no kernel at all, `build` still produced a page, exit 0, cells
  rendered as source, with a warning. Nothing crashed anywhere in this round.

**#8 aesthetic and minimalist design** is the one heuristic with a genuine tension and no item filed:
`--help` runs 45+ lines before the first thing a beginner needs. It is *complete*, which this project
values, and splitting it into a short default plus `--help --all` is a real design change, not a bug.
Recorded as an observation for the author, not as a defect.

---

## Not measured

- **No install step was tested.** R4 covered building from a clean clone; this round used a
  pre-built binary, so "time to first render" excludes the 343-crate release build that the pre-mortem
  identified as the expensive funnel step.
- **The editor companion was not part of the walkthrough.** A first-time user's `.tmd` editing
  experience in VS Code is a separate surface.
- **Only the site path and the single-file path were walked.** `taliesin new post`, `paper`, and the
  book scaffold were not.
- **No second human.** A cognitive walkthrough performed by the party that read the source is weaker
  than one performed by a stranger; every finding above is therefore anchored to a transcript rather
  than to an impression.

## Round bookkeeping

This round wrote only this file. Items 120-123 follow R7's 117-119. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**Incidental, and worth the author's attention:** the shared `target/release/taliesin` was built from
the concurrent branch (`1f72853`), so the first R14 measurements were taken against the wrong tree.
They were **re-run after rebuilding from this branch and the headline held** (`check <dir>` and
`build --strict` both still exit 0 on a defective embedded deck). A shared `target/` between two
sessions is a measurement hazard: **check `taliesin --version` against your own HEAD before trusting a
CLI measurement.**
