# AP6: cross-browser / cross-platform (2026-07-25)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspective:** AP6, the last unrun AP slot, run at the author's request as the fourth audit of the
day. Against `92fc67b`, release-built output served statically. **One repo change**: Playwright
Firefox was added as a devDependency of `tools/ui-audit` so this round is re-runnable (there was no
non-Chromium browser driver in the tree).

## Headline

**No cross-browser divergence found. Firefox and Chromium produced byte-identical results on every
axis measured, with zero console errors in both.**

This is a genuine positive result, not a thin one: the client is hand-written vanilla JS with no
build step and no polyfills, and the deck is a bespoke engine, so "identical in both" was not the
safe prediction. But the coverage is narrower than the entry's scope, and the gaps are listed below
rather than papered over.

## Measured, both engines identical

Two pages (a built book chapter and a built deck) plus three interactive surfaces:

| Axis | Firefox | Chromium |
|---|---|---|
| `hidden="until-found"` support (`onbeforematch`) | true | true |
| `inert` support | true | true |
| copy buttons / heading anchors / TOC links | 18 / 16 / 16 | 18 / 16 / 16 |
| horizontal overflow on `<html>` | none | none |
| `main` text length | 15,680 | 15,680 |
| deck slides / inert slides / `window.TaliesinDeck` | 18 / 18 / object | 18 / 18 / object |
| deck live region at load | `Slide 1 of 19: A Plain Deck` | identical |
| deck live region after ArrowRight | `Slide 2 of 19: What decks are` | identical |
| Cmd-K palette opens / result count | true / 183 | true / 183 |
| theme switch via `tali-theme` | `data-theme=dark`, `rgb(22, 24, 29)` | identical |
| `.tali-scrolly` steps / state / stage | 3 / `trend` / true | identical |
| console errors | **none** | **none** |

The two features AP7 flagged as the concrete cross-browser risk (`hidden="until-found"` on tabset
panels, `inert` on deck slides) are supported in both engines, so the tabset's find-in-page
behaviour and the deck's AT hiding both hold up outside Chromium.

## Not chased (the honest coverage gap)

- **WebKit / Safari.** Not installed, and worth knowing that Playwright's Linux WebKit build is *not*
  Safari: it shares the engine but not the platform integration, so a green run there would be
  partial evidence anyway. Real Safari needs a Mac.
- **Mobile browsers**, and the viewport matrix generally: this ran at 1440x900 only.
- **The preview path.** Everything here is *built* output. The websocket client, the incremental
  block swap and click-to-source were not exercised in Firefox, and that is where the hand-written
  client does its most unusual work. **This is the highest-value follow-up** if AP6 is ever re-run.
- **macOS / Windows**: path handling (`\` vs `/`), file-watch semantics and kernel spawning. Development
  is Linux-only and this round did not touch it. A static grep for path assumptions was not run.

## Method

Playwright `firefox` (build 1538) and Chromium (system Chrome) driven from the same script so the two
runs are literally the same assertions, with a mechanical diff of every collected field at the end.
Built `docs/guide` plus a built `corpus/deck.tmd` and `corpus/explorable/scrolly.tmd`, served over a
local static server. Probe scripts in the session scratchpad.
