# Code-cell DX brainstorm, 2026-09-24

> **Status (2026-09-25): the first batch is on main (f387488c and before); the rest is recorded
> here so it is not re-derived.** Shipped: forwarding of hover, signature help and
> go-to-definition into cells, a file-backed shadow, per-cell `{js}` scope, the inert-option
> warning, the static `define()` read and language-filtered option completion. Still open: the
> "kept, not built" list below. Parked and killed ideas carry their reasons and revival
> triggers; the killed ones also have an entry in `DO-NOT-REBUILD.md`.

The author's question: the experience of **writing code inside code blocks** had been
neglected. What do developers love about other tools, what does research say makes tools
pleasant and productive, and what should Taliesin do about it?

## Method

A multi-agent pass, then implementation in a second worktree alongside the audit-fixes session:

- **Map:** the editor side and the run side of a cell today, with file:line evidence, plus a
  digest of every earlier ruling on the area (this register, the 07-18 / 07-21 / 07-28 / 08-07
  audits, the handover note).
- **Research:** six slices over notebooks, literate tools, live-feedback tools, DX research,
  AI assistance and environments. Primary sources where possible, 20 web searches.
- **Ideation:** 38 ideas from three lenses (feedback loop, writing the code, subtraction).
- **Verification:** each idea got an adversarial critic that re-read the code, grepped this
  register and ran probes. A completeness critic then added a gap round.
- **Result:** 17 kept with changes, 12 parked, 9 killed.

## What makes developer tools feel good

Each principle carries its evidence grade:

1. **Errors in the author's own file and line, where they are already looking.** Rustc,
   Vite's overlay and Pluto all do this. Reading an error message is as hard as reading source
   (Barik et al., ICSE 2017, peer-reviewed). See also Becker et al. 2019.
2. **Never serve stale output silently.** Pimentel et al. (MSR 2019, peer-reviewed) re-ran
   about 860,000 GitHub notebooks: 36% had out-of-order cells, 24% ran without an error and
   4% reproduced their results. Of the failures, 29% were ImportError or ModuleNotFoundError.
3. **Stay inside perceptual limits.** 0.1 s feels instant, 1 s keeps the train of thought and
   10 s loses attention (Nielsen, after Miller 1968 and Card 1991). Every reduction helps
   (Jaspan and Green 2023).
4. **Give the analyzer each cell's real runtime scope.** Intelligence against the wrong
   scope misleads. marimo, Quarto's virtual documents and Observable issue #882 show it.
5. **Small doses of liveness beat maximal liveness.** A plain inspector often did the job
   (Kubelka et al., ICSE 2018, peer-reviewed).
6. **Trust beats cleverness.** One false "it updated" and developers refresh by hand
   (Abramov, practitioner).
7. **With AI, verification is the bottleneck, not generation.** The best AI feature is the
   fastest run that puts errors at the right line (Mozannar et al., CHI 2024; Stack Overflow
   2025 survey: 46% distrust AI accuracy). An executable check wired in as a hook teaches an
   agent the dialect (McNutt et al., CHI 2023; marimo's agent docs).

**Taliesin's core bet is validated.** "The source file in your own editor is the truth, and the
browser is a read-only view" is where marimo, Clerk, Observable Framework and Pluto converged.
Document-order execution designs out most hidden state. The loop (prose in 90 ms, a cell settled
in 152 ms) sits inside the "instant" band. The neglect was concentrated in two places: the
editor side of a cell, and errors that do not name a line.

## Shipped (on main)

| Commit | What |
|---|---|
| 073df477 | Hover, signature help and go-to-definition inside `{python}`/`{js}` cells, answered by Pylance or the TypeScript server through `embedded.ts`'s shadow. The shadow became a file in a private `mkdtemp` directory, written only with `workspace.fs.writeFile`. That fixed three pre-existing defects of the untitled shadow: background `Untitled-N` tabs, hot exit restoring them every session, and Pylance warnings leaking into Problems. |
| 55fd0da8 | A located warning for a cell option that does nothing: `echo` on `{js}`, a bare `label: setup`, `echo` on a listing, a repeated key. The dangling-`//| input:` check reads `define(name=...)` keywords statically, so it runs on every blog post that uses the bridge instead of a-star alone. |
| b3e03cb7 | Each `{js}` cell has its own scope in the shadow: a `wrap` on `taliesin/cellRegions` from `render::JS_CELL_PARAMS`, which is pinned to `tali-js.js`. `executable` is gone. |
| 3ca613dc | Cell-option completion offers only keys that act on the cell's language and that it has not set; `label:` offers `fig-`/`lst-`/`tbl-` only; `cache` counts as inert off kernel languages; the did-you-mean suggests only keys that act. |
| 98fdd884, f387488c | One `is_rendered_cell` predicate; the portability census re-run after the corpus cleanup. |

**Measured in VS Code 1.126 with Pylance:**
- numpy hover: 16 to 23 ms warm.
- Signature help: about 190 ms, nearly all of it Pylance.
- From an edit to a hover that reflects it: about 120 ms.
- No tab and no Problems entry for a shadow, under default or strict settings.

**Known limits of what shipped:**
- Display fences (```` ```js ````, ```` ```python ````) are projected unwrapped.
- A `{js}` cell whose fence is line 0 with no option line stays unwrapped: line 0 holds
  `// @ts-nocheck`.
- F12 on `tali` lands on the cell's fence line.
- TypeScript shows "(loading...)" until a real `.js` file has been opened.
- An overload picked with the arrow keys resets on the next keystroke, because
  `executeSignatureHelpProvider` always sends a fresh Invoke.

## Fixed by the audit-fixes session

These came out of the same research and were landed by the audit-fixes session:

- **A3, the freeze cache persisting warm-kernel state.** Both sessions found it independently;
  a stale output published with `build --strict` exit 0.
- **Prose completions inside cells** (WP10).
- **The `define()` bridge now survives `include: false`** (WP3). Confirmed on a live kernel.
- **Failures are carried as data**, and the crashing cell is named (WP3).
- **The guide teaches a project `.venv`**, and `doctor` suggests one (WP5).

## Kept by review, not built yet

Each of these is small, and each problem was confirmed in the code on 2026-09-24:

- **Stop at the first failing Python cell.** Later cells get a NOT_RUN "the cell at X raised"
  placeholder. Measured: one typo gave 3 tracebacks. Two corrections from review: keep the
  failed cell in `ran`, and do not count a truncation marker as a failure. The audit session
  declined it as a behaviour change, so it is the author's call.
- **Name the exception in the cell-error line** (`NameError: name 'grid' is not defined`), and
  in `--format json`. The file and line are there now; the text is not.
- **`//# sourceURL=` per `{js}` cell**, named after `//| name` or its ordinal, with no padding,
  so DevTools stack frames and breakpoints name the cell.
- **Dev-panel cell-error rows open the cell in the editor** (`openSource`, `client.js`), the
  same way diagnostic rows do.
- **Delete `tali.container` and `tali.invalidation` from `makeApi`.** They duplicate the cell's
  parameters and have zero uses.
- **A `TaliCell` typedef on `makeApi`**, checked by the assets `tsc` gate and served to the
  shadow, so `tali.` completes members. Today `tali` is a parameter typed `any`.
- **A sibling `.js` module a `{js}` cell imports becomes a page dependency**, and saving it
  reloads the tab. Hoist `build.rs`'s `relative_specifiers` into one resolver.
- **Grammar:** map markdown-basics' embedded languages (Ctrl+/ in a ```` ```sh ```` fence gives
  `#`, not `<!--`), and remove the alias rot (`javascript|ojs` painted as a live `{js}` cell,
  `dot` as mermaid, `\.?`) along with the dotted `{.lang}` read.
- **`doctor`'s install line spells `uv pip install --python ...`** when the venv is a uv venv.
- **Blog:** replace the six hand-built slider/select `viewof` cells with `{{< input >}}`, and
  add one line to the blog's CLAUDE.md so the pattern does not come back.
- **An author-side Claude Code hook** in the blog's settings that runs
  `taliesin build <file> --check-only --format json` on every `.tmd` an agent edits
  (about 0.25 s).

## Parked (real, no evidence of need yet)

- **Rewind names bound by stale cells before a warm re-run** (a rename fails in the preview,
  not at build).
  *Revive:* the author twice hits a NameError at build that the warm preview hid, or restarts
  the kernel as a pre-publish ritual. Try a "warm state, restart to verify" badge first.
- **Warn on `breakpoint()` in an executed cell**, where it is a silent no-op.
  *Revive:* the author loses time to it, or a post ships one. Make it a static check.
- **Sticky scroll and folding inside long cells.**
  *Revive:* the author loses their place in a long cell. Step one needs no code: set
  `"[taliesin]": {"editor.stickyScroll.defaultModel": "indentationModel"}` for a week.
- **Lock the blog's environment with uv.**
  *Revive:* a machine move, a Python minor upgrade, or a re-run that changes a published
  number. Lock what the posts import, not `requirements.txt`.
- **A line and code frame for `{js}` SyntaxErrors.**
  *Revive:* a syntax break the author did not just type forces bisecting a long cell. It is
  cheap once `sourceURL` lands.
- **Keep the last good `{js}` render under an error.**
  *Revive:* after located errors and an autosave cadence, the author still loses a figure
  mid-fix. Consider a compile-before-teardown variant that also covers WebGL.
- **A per-language autosave recipe in the guide.**
  *Revive:* the author keeps `"[taliesin]": {"files.autoSave": "afterDelay"}` after a week.
  Avoid `autoSaveWhenNoErrors`.
- **Interrupt without restart.**
  *Revive:* an uncached post run over about 30 s, a cell over about 10 s, or Restart used to
  escape a runaway cell. Build it with stop-at-first-failure.
- **Forward Pylance diagnostics onto cell lines** (Python only, syntax errors and undefined
  names).
  *Revive:* the author loses a save-and-rerun cycle to a NameError. Note that the shadow now
  carries `# type: ignore` on purpose, so this means filtering Pylance diagnostics rather
  than reading them.
- **Plot and d3 typings in the shadow.**
  *Revive:* the author looks up Plot options or d3 signatures mid-edit, and a probe shows an
  absolute-path type import resolving in the shadow.
- **One way to read a reactive value** (fold `tali.get` and `tali.defines` into
  `tali.value`).
  *Revive:* a wrong-reader bug ships, or the typed cell surface wants one reader.

## Killed

Each of these also has an entry in `DO-NOT-REBUILD.md`:

- **An inspector for a `{js}` cell's discarded return value:** DevTools already does it.
- **stderr as an author-only channel:** it would strip truncation markers and real warnings.
- **A pre-run import check:** a second interpreter path with known false positives, and
  `_freeze` restores upstream cells anyway.
- **"Why did this cell re-run":** the warming chip and the badges already say it.
- **Runtime errors pushed into the editor from TypeScript:** that is a second diagnostics
  publisher outside Rust.
- **A viewof value that survives editing its own cell:** `{{< input >}}` already does it.
- **An ETA and an OS notification for long runs:** no post runs longer than 3 s.
- **Defaulting `{js}` to `echo: false`:** `echo` is inert there.
- **Keeping auto-import edits:** they are off in Pylance's default.
- **A cell formatter:** the author's layout is deliberate, and a reformat re-runs downstream
  cells.

Ruled out in the first discussion, before the research:
- **A notebook view of a `.tmd`**, or editing a cell in a side `.py` tab. Either is a second
  editing surface.
- **Every squiggle from the cell language.** The notebook idioms (a bare last expression,
  top-level `await`, magics, display samples) false-fire in a `.py` or `.js` view. The
  filtered, Python-only version is parked above.
