# Deck narration: read-optimized notes + reading-time duration estimate

Date: 2026-07-21
Status: design (approved to write; pre-implementation)

## One-line summary

Treat a slide's `::: {.notes}` block as *the script you speak*, and give the
person recording a narrated slide video two small read-side affordances on the
**existing** speaker window: a duration estimate derived from the script's word
count, and a read-optimized layout that makes the script large and primary. No
new mode, no new output format, no write-back.

## Motivation

Recording narrated slide videos (pre-recorded conference talks, course/lecture
videos, async updates, explainer videos) is a real and growing use case, and
reading prewritten narration sounds markedly better than improvising. Taliesin
decks already carry per-slide speaker notes and a presenter window; the gap is
that the presenter window is tuned for *live* presenting (two slide previews own
the top, notes sit in a small 19px box capped at 26vh) rather than for *reading a
script while recording*.

Taliesin's differentiated sliver over a generic web teleprompter or OBS plugin is
narrow but real: the script is already authored per slide in the same source and
auto-advances with the deck, so Taliesin (unlike any generic teleprompter) knows
how long each slide's script is and can show **planned vs. actual** against its
own live timer.

## Non-goals (deliberate scope guards)

These are out of scope and stay out. They are where the "overbloat" risk lives,
and most duplicate tools the author already uses (OBS, QuickTime, Loom):

- **No auto-scroll / speed knob / play-pause pacing engine.** Pacing is
  slide-synced only: you advance slides as you talk, the script swaps with the
  slide. A script too long for one screen is a signal to *split the slide*
  (fragments or sub-slides, already supported), not a reason to build a scroller.
- **No webcam PiP, no recording, no video export.** Already ruled out of scope in
  `notes/2026-07-12-deck-audit.md` (line ~487, "Presenter webcam recording /
  video export — a de-facto new output format"). Recording happens entirely in
  the user's OS/OBS, outside Taliesin. This design produces no new artifact.
- **No mirror mode** (for physical teleprompter glass) — niche hardware knob.
- **No write-back to source.** The read view only *displays* notes. Single
  editing surface is preserved.
- **No words-per-minute config knob.** One sensible constant (see below), tunable
  in code from real planned-vs-actual feedback. Consistent with the project's
  minimal-config rule: a reading-rate constant is an authoring aid, not a
  reader-local a11y preference.

## Approach (chosen)

Enhance the existing `?qmd=speaker` window in place (rejected alternatives: a
dedicated `?qmd=teleprompter` mode duplicates ~80% of the speaker window and adds
a mode branch to maintain; merely enlarging the notes box leaves the previews
hogging the top and does not solve "read comfortably"). The speaker window
already owns notes extraction, cross-window slide sync, snapshot previews, and a
live timer/clock, so both affordances are additive to one file's view logic.

## Component 1: reading-time duration estimate

### Model

Per slide: `seconds = round(word_count(notes_text) / WPM * 60)`, where
`word_count` counts whitespace-separated tokens of the slide's `.notes` plain
text (code/markup stripped to text; a fenced code block inside notes is unusual
and, if present, counts its visible words). Deck total = sum over slides that
have notes.

- **WPM default: 130** (deliberate narration with pauses; slower than silent
  reading ~200-250 and slower than casual speech ~150-160). A single named
  constant in Rust, not a config field.
- Slides **without** notes contribute 0 and are excluded from the "scripted"
  count, so the total is honestly "at least this much for the narrated portion,"
  never a false precision over unscripted slides.

### Where it is computed: server-side, in Rust

Computed at render in `crates/core/src/render/deck.rs` (in/around
`render_section`, where the `<section>` open tag is already assembled, see
`deck.rs:397`). For each slide whose content contains a `.notes` div, count the
notes' words and emit the per-slide estimate as a data attribute on the
`<section>`:

```
<section ... data-script-secs="35">
```

A slide **without** notes emits **no** `data-script-secs` attribute at all (the
attribute's absence is the "no script" signal, both to the client and the corpus
test). It is not emitted as `data-script-secs="0"`.

Rationale for server-side (vs. counting in JS):

1. It is testable by the corpus regression net (which asserts on emitted HTML);
   a JS-only computation is not.
2. The WPM logic lives in exactly one place.
3. It lets the `preview`/`build` console print a summary line without the browser.

A deck total is derivable by summing `data-script-secs` across sections; the
render pipeline also exposes the total + scripted-count so the CLI can log it
(exact plumbing to the console line is an implementation-plan detail; the data
source is these attributes).

### Where it is shown

1. **Speaker window** (`deck.js`, `.tali-speaker`), next to the existing timer
   (`deck.js:1072` `updateSpeakerClock`, header built at `deck.js:1083`):
   - deck total, as **planned vs. actual**: `planned ~8:40 · elapsed 0:00`
     (elapsed is the existing live timer; planned is the summed
     `data-script-secs`).
   - current slide: `slide 3 / 12 · script ~35s` (read from the current
     section's `data-script-secs`; "no script" when the slide has no notes).
   The speaker window already reads the live deck DOM as its data source
   (`deck.js:1080` keeps the hidden deck for notes/counts), so the attributes are
   in hand with no extra wiring.

2. **`preview` / `build` console**, one line via the dev-server logger
   (`crates/server/src/log.rs`), e.g.:
   `narration ~8:40 across 12 slides (9 scripted)`. The slide count includes the
   front-matter title slide, so it agrees with the speaker window's "slide X / N"
   rather than reporting a different total.

### Honest caveats (surfaced, not hidden)

Word-count estimates are rough: demos, code walkthroughs, and dramatic pauses
push actual past estimate. The planned-vs-elapsed readout in the speaker window
is precisely what lets the author see and calibrate this. The console line says
`(9 scripted)` so an author with mostly-unscripted slides is not misled by the
total.

## Component 2: read-optimized notes layout

A toggle in the speaker window flips it between two layouts:

- **Present** (existing default): current preview + next preview + small notes box
  + timer. Unchanged.
- **Read** (new): the script becomes large and primary and sits high in the
  window (so eyes rest near a webcam when recording off the same screen); the
  current slide shrinks to a small orientation thumbnail; the "next" preview is
  dropped; `A- / A+` controls adjust script text size. The duration readout
  (Component 1) stays visible in both layouts.

### Surface

- Activation: a small header button in the speaker window plus a keyboard
  shortcut (routed through the existing speaker key handling, `onKey`). The
  chosen layout persists for the window session (in-memory; no config file).
- CSS: `.tali-speaker` gains a `read` variant (e.g. `.tali-speaker.read`) in
  `crates/core/assets/css/deck.css` (speaker styles begin at `deck.css:367`).
  Reuse existing tokens; no new palette.
- JS: `initSpeaker` / `updateSpeakerUI` (`deck.js:1058`, `deck.js:1078`) gain the
  toggle + text-size handlers and, in read layout, skip/shrink the preview
  snapshot work.

Text-size `A- / A+` is reader-local ergonomics (like theme/text-size elsewhere in
the project) and is exempt from the minimal-config rule; it is a runtime control,
not document config.

## Corpus pin (required by corpus-plus-roadmap)

Every new capability ships pinned by a corpus document + test in the same change.

- **Doc:** extend `corpus/deck.tmd` (which already has a `::: {.notes}` slide at
  line 115) with at least one more slide carrying notes and at least one slide
  *without* notes, so the pin exercises both the estimate and the scripted-count
  exclusion.
- **Test (`crates/core/tests/corpus.rs`):** assert that a slide with notes emits
  `data-script-secs` with the value implied by its word count at the default WPM,
  that a slide without notes emits no `data-script-secs` attribute and is excluded
  from the scripted count, and that the deck total equals the sum of the emitted
  values. This makes the reading-time math a regression-guarded invariant rather
  than display-only JS.

The read-layout view is browser-verified (chrome-devtools MCP screenshot per the
project's UI-testing rule) rather than corpus-asserted, since it is
presentation-only chrome; the load-bearing, testable contract is the
`data-script-secs` emission.

## Files touched (anticipated)

- `crates/core/src/render/deck.rs` — count notes words, emit `data-script-secs`,
  expose deck total + scripted count.
- `crates/core/assets/js/deck.js` — speaker window: duration readout
  (planned vs. elapsed + per-slide), read-layout toggle, text-size controls.
- `crates/core/assets/css/deck.css` — `.tali-speaker.read` layout + duration
  readout styles.
- `crates/server/src/log.rs` (and its caller in `main.rs`/serve) — one console
  summary line on preview/build.
- `corpus/deck.tmd` — pin slides (with + without notes).
- `crates/core/tests/corpus.rs` — assert `data-script-secs` + total.

Remember: editing `assets/css|js` requires a `cargo build` before the change
shows up in a `build` (they are `include_str!`-compiled).

## Adjacent idea, explicitly deferred

A "N min read" badge for blog posts/articles uses the same word-count math on body
text and is a well-worn blog convention. It is a separate, also-small change and
is **not** part of this pass; it can be a sibling later if wanted.

## Success criteria

1. A deck with per-slide notes renders `data-script-secs` per scripted slide and
   the corpus test passes.
2. `preview`/`build` prints one honest duration line (`~M:SS across N slides
   (K scripted)`).
3. The speaker window shows planned-vs-elapsed + per-slide script seconds.
4. The speaker window's read layout makes the script the large primary element
   with working `A- / A+`, verified by a browser screenshot.
5. No new mode/URL, no new output artifact, no source write-back, no wpm config
   field. Existing deck + speaker behavior is otherwise unchanged (corpus green).
