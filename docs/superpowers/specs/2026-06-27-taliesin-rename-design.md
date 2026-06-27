# Taliesin: the rename + the final break from Quarto's identity

Status: design / spec (decisions settled, exhaustively inventoried, invariant-checked). No code in this pass.
Date: 2026-06-27
Author: Andreas Bogossian (with a 9-surface change-inventory workflow + adversarial invariant-safety critic).

Grounding evidence: the full per-occurrence inventory + critic output is persisted at
`tasks/w07kujtww.output` (referenced by the implementation plan, not reproduced in full here).

---

## 1. The reframe (why this is coherent, not a contradiction)

The owner's question was "should I completely drop Quarto, drop `.qmd`, and rename the
project?" The load-bearing realization, stated by the owner mid-design: **"drop Quarto" and
"stay close to Markdown" were never in tension.** They only looked like one decision. They
are three:

1. **The engine dependency on Quarto** — already gone. No Quarto binary is invoked; reveal.js,
   the OJS runtime, highlight.js, pandoc, and knitr are all removed. At runtime the project is
   already 100% its own. There is nothing left to "drop" here.
2. **Genuine cruft** — dead `.quarto-*` CSS and comments that misdescribe the native engine as
   "reveal.js"/"Quarto". These mislead about the project's *own* code; delete/fix them freely.
3. **The familiar authoring surface** — `:::` divs, `#|` cell options, `@fig-`/`[@cite]`,
   `{{< >}}` shortcodes, YAML front matter. This is **Pandoc/Markdown lineage, not Quarto
   branding.** Keeping it is staying close to what writers already know; dropping it would be
   "beyond Markdown" (isolation), not "beyond Quarto".

So the project sheds Quarto's **identity** (the name, the "q", the positioning) while keeping
Markdown's **familiarity**. The north star is explicit: **a Markdown user must be able to
switch with near-zero learning cost.** Every decision below is judged against that.

## 2. Settled decisions

| Decision | Value | Rationale |
|---|---|---|
| Drop Quarto (engine) | Already done | No runtime dependency remains |
| Approach | **Native-first, Pandoc-compatible** | Lead with our own spelling; keep standard Markdown syntax untouched |
| Project name | **Taliesin** | The radiant Welsh bard who, given three drops from Ceridwen's cauldron, instantly knew and could sing the true form of any thing — i.e. renders source into its true form (see §3) |
| Crates / binary | `taliesin-core`, `taliesin-server`, binary `taliesin` (+ `tali` shorthand) | Directory names (`crates/core`, `crates/server`) stay; only package `name` fields change |
| Extension | **`.tmd`** | Drops the Quarto "q", keeps the honest markdown "md" (like `.qmd`/`.Rmd`/`.mdx`). One-letter migration. `.qmd` still **accepted as deprecated input** (warn-nudge) |
| Vocabulary | `format: deck` + `define()` lead; `revealjs` / `ojs_define` kept as **nudged aliases** | "deck" is self-evident; Quarto-switchers' existing files keep working |
| `qmd-*` internal/contract prefix | Rename to `tali-*` / `Taliesin` **with back-compat aliases** | Owner chose full coherence; aliases keep every vendored extension working |

## 3. Name vetting (recorded so it is not re-litigated)

- **`quoin` is dead** (was the prior front-runner, deferred 2026-06-16). Re-checked 2026-06-27:
  a `quoin` crate was published on crates.io on 2026-06-20 for "the Quoin programming language,
  a Smalltalk-inspired language on a Rust VM" — a direct, same-niche collision. Also taken on
  PyPI; `Quoin, Inc` is an established software consultancy. Dropped.
- **Registry vetting** ran ~60 candidates across crates.io / npm / PyPI / domains / web
  collision. `.com` is squatted for essentially every real word, obscure or not, so domain
  availability was deliberately **not** allowed to drive the identity choice. Eliminations by
  collision: `tympan` (OSS hearing-aid project), `quadrat` (Quadratic/Quadrant cluster),
  `deckle` (an active novel-publishing app), `liveproof` (Onfido trademark), `makeready`
  (Xerox FreeFlow Makeready®), `verdandi` (INRIA data-assimilation library), `kvasir` (Cisco
  pentest tool). `frisket` was clean but read too close to "brisket".
- **`taliesin`**: crate free; software-clear (the only real association is Frank Lloyd Wright's
  estate — architecture, not a competing tool). `.com` squatted like everything else; ship on
  `taliesin.dev`-style. Owner chose it for the story fit.
- **`.tmd`**: real but used only by unrelated legacy/binary formats (TextMaker docs, MySQL temp,
  PS1 3D models, terrain) — no dev-markdown tooling conflict; the VS Code companion sets its own
  language association regardless.

## 4. The design, by surface

The repo splits into surfaces by **who sees each one**. "Easy to learn" decides each.

### 4.1 Author-visible surface — change only the brand

- **Extension `.qmd` → `.tmd`.** Add one central constant module in `taliesin-core`
  (`crates/core/src/ext.rs`): `SOURCE_EXT = "tmd"`, `SOURCE_EXT_DOT = ".tmd"`,
  `ACCEPTED_SOURCE_EXTS = ["tmd", "qmd"]` (native first, deprecated last), plus helpers
  `is_source_ext`, `strip_source_ext`, `source_to_output`, `output_to_source_candidates`
  (yields **both** `{stem}.tmd` and `{stem}.qmd` for the reverse broken-link checker), and
  `is_index_page` (replaces the exact `rel == "index.qmd"` literal in `book.rs:114`). Route
  every **recognition** site through the slice (page-walker `site/mod.rs:956`, `is_qmd`
  `serve_site.rs:946`, watcher `serve.rs:1051`, `SKIP_EXT` `main.rs:1146`, check-walker
  `main.rs:1496`, test walkers) and every **mapping** site through the strip helpers
  (`qmd_to_html` `site/mod.rs:970`, `deck_href` `extension/mod.rs:730`, book titles). New
  emission (init scaffold `main.rs:119`, generated URLs) uses `.tmd` only. A single
  `warn_if_deprecated_source_ext(path)` fires once per CLI invocation when the input is `.qmd`.
- **Keep all Pandoc/Markdown syntax untouched**: `:::`, `#|` / `//|` / `%%|`, `@fig-`/`@sec-`,
  `[@cite]`, `{{< >}}`, the verb shortcode *names* `include`/`embed`/`video`/`input`, `. . .`
  fragment breaks, YAML front matter.
- **Two Quarto-proprietary words get a clearer native default, old spelling retained:**
  - `format: revealjs` → **`format: deck`**: widen the single gate `is_reveal_format`
    (`render/mod.rs:906`) to also accept `deck` / `-deck`, and add `"deck"` to `extension_ref`'s
    base list (`extension/mod.rs:56`). When the old spelling matches, push a located,
    click-to-source deprecation Warning. The internal enum `DocFormat::Reveal` may be renamed
    `Deck` (pure internal refactor, ~10 match arms) or left as-is.
  - `ojs_define()` → **`define()`**: in `kernel.rs` `OJS_DEFINE_PREAMBLE`, define the body once
    and bind both `globals()["define"]` and `globals()["ojs_define"]` (the latter a thin alias,
    optionally warning once via Python `warnings.warn`).

### 4.2 Brand surface — rename fully

- **Crates / binary / env.** Package names `qmd-fast-core`/`qmd-fast-server` →
  `taliesin-core`/`taliesin-server` (directories stay); the lib import path `qmd_fast_core` →
  `taliesin_core` follows mechanically (~90 internal use-paths + the compiled rustdoc doctests
  at `render/mod.rs:77,986`, which fail `cargo test` if not moved in lockstep). Binary
  `qmd-fast` → `taliesin`; `tali` shorthand as a PATH symlink (not a second compiled `[[bin]]`).
  The 13 live `QMD_FAST_*` env vars → `TALIESIN_*` through **one** shared
  `env_compat(new, old)` helper that reads the new name, falls back to the old, and warns once;
  the two internal `set_var` writers (`main.rs:153`, `kernel.rs:834`) must write the **new**
  name the reader prefers. `QMD_FAST_GIT_SHA` (build-time codegen) renames with no alias.
- **CLI strings + help + colophon** (~43 `"qmd-fast"` literals in `main.rs`, incl. the help
  self-test assertion `main.rs:2328` which must be updated to the new name or it goes red).
- **README + docs identity lines** reframed around Taliesin (drop "a focused replacement for
  Quarto"); **migration guidance kept** (reframed as "how your existing Quarto/Jupyter docs
  map"). The two manual books are themselves `.qmd` → `git mv` to `.tmd`.
- **Favicon** `web-client/favicon.svg` (the bundled "qmd-fast mark", `page.rs:419`) — brand swap.
- **In-repo tooling:** `.claude/settings.json` allowlist globs `Bash(qmd-fast render *)` etc.
  must change with the binary rename or every call re-prompts; verify `.claude/hooks/cargo-fmt.sh`
  hardcodes no `-p` crate name.
- **Out-of-repo launchers** (`~/.local/bin/qmd-fast`, `qmd-fast-stable`, `qmd-promote`): rename
  the files and leave `qmd-fast*` symlinks so the frozen stable channel + muscle memory survive.
  Manual step (cannot be done from inside the repo); capture as `scripts/rename-launchers.sh`.
- `.qmd` files stay **accepted as deprecated input** so nothing breaks on upgrade day.

### 4.3 Internal/contract surface — rename `qmd-*` → `tali-*` with aliases

Invisible to authors; the value is full coherence. Mechanisms differ by failure mode:

- **CSS classes (~218 distinct, ~1,680 occurrences across 4 stylesheets + JS + emitters).**
  Emitters output **only** the new `tali-*` class (clean HTML). For vendored/third-party
  extension CSS that targets the old names verbatim (the renamer cannot rewrite a user's
  stylesheet — the real `liquid-glass-revealjs` extension hard-codes 80+ `.qmd-*` classes), add
  a **zero-specificity `:where(.tali-foo, .qmd-foo)` alias layer** in the framework stylesheets,
  so old selectors keep matching without inflating specificity. The narrow exception: **dual-emit
  both classes** on the structural deck containers (`class="tali-deck qmd-deck"`,
  `tali-slides qmd-slides`, `tali-slide qmd-slide`) so combinator selectors like
  `.qmd-deck > .qmd-slides` keep matching.
- **CSS custom properties (~46 distinct `--qmd-*`, 377 occurrences in CSS, 10 emitted by
  `theme.rs:74-213`).** Rename to `--tali-*` as the source of truth. **Critical:** preserve
  author/extension overrides by reading the legacy var as a *fallback in the consuming property*:
  `color: var(--tali-bg, var(--qmd-bg, default))`. Do **NOT** write `--tali-bg: var(--qmd-bg)`
  at `:root` — that inverts precedence and silently drops a vendored theme's `--qmd-bg` override.
- **JS globals (~1,384 occurrences, 3 tiers).** The 3 documented public globals
  (`window.QmdDeck`, `window.qmdEnhancers`, `window.qmdJs`) get **two-way same-object aliases**:
  assign `window.TaliesinDeck = window.QmdDeck` (same live object, so every method + every
  spec-added method is reachable through either name), wrapped in a `defineProperty` getter that
  `console.warn`s once on the old name. The cell-local `qmd` API object gains a `tali` alias
  parameter (`qmd-js.js:128`). Internal-contract globals (`QMD_*` config flags, theme/reader
  helpers, TOC hooks) rename in lockstep (no external consumer). Private state (`__qmdjs`) renames
  freely. **localStorage keys** (`qmd-theme`, `qmd-reader-*`, `qmd-read:`, `qmd-deck-theme`) must
  not change without a migrate-read, or readers lose saved state.
- **`qhl-` highlight scope prefix** (`highlight.rs:23` `ClassStyle::SpacedPrefixed{prefix:"qhl-"}`
  → `.qhl-*` in `base.css` + assertion `highlight.rs:70`). A sibling internal namespace, **omitted
  from the 218-class count**. Recommendation: rename `qhl-` → `tali-hl-` in lockstep (emitter +
  base.css + test) for coherence; it is independent and low-risk. (Alternative: keep `qhl-` as a
  documented exception — explicit decision required.)
- **Schema self-references** (`schema.rs` + `assets/schema/*.schema.json`): the bundled filenames
  `qmd-frontmatter.schema.json` / `qmd-site.schema.json` are `include_str!` paths
  (`schema.rs:13,16`) AND drift-test path strings, and the embedded JSON `title` strings carry
  "qmd-fast". `schema.rs` *generates* the JSON and a **byte-equality bless test**
  (`schema.rs:156`, `QMD_FAST_BLESS`) asserts it against the committed file — so any rename must
  change generator + committed file together and re-bless, plus the `$schema=.qmd/...` modeline
  examples and the `.qmd/` output dir → `.tali/`.

### 4.4 Cleanup (rides along, no name dependency)

- Delete the dead `.quarto-*` selectors in `corpus/tech-blog/custom.css` + `theme.scss` (verified:
  the renderer emits `qmd-listing`/`qmd-card`/`qmd-cat`, never `quarto-*`, so they match nothing).
- Fix comments/docstrings that misdescribe the native engine: `render/model.rs:119` ("reveal.js
  slide deck"), `includes.rs:1` ("Quarto include"), `divs.rs` (`parse_pandoc_attrs` /
  "pandoc/Quarto fenced-div"), plus ~40 internal stray mentions.

## 5. Invariant-safety analysis (from the adversarial critic; verified against real code)

The critic's verdict: **green-light**, with five gaps to close and one self-inflicted hazard to
avoid. No load-bearing invariant (data-block-id/sourcepos, deck contract, click-to-source,
offline-first) is unrecoverably threatened if atomic-commit ordering and the data-* exclusion
are respected.

- **Freeze cache — do NOT bump `FORMAT_VERSION`.** Verified: `cumulative_hashes(interp, codes)`
  (`freeze.rs:64`) keys only on interpreter-id + cell-code bytes, so the `.qmd`→`.tmd` rename and
  the prefix renames do **not** bust caches. The real hazard is the vocabulary surface's *proposed*
  `FORMAT_VERSION 2→3` bump for the `qmd-define`→`tali-define` wire rename — that is a global
  invalidator that would cold-replay every author's entire cache. **Mitigation:** do not bump;
  make the `qmd-js.js` consumer accept **both** script types
  (`querySelectorAll('script[type="tali-define"], script[type="qmd-define"]')`), so cached
  `_freeze` blobs holding legacy `qmd-define` HTML re-bind with zero cache loss.
- **`data-block-id` / `data-sourcepos` corruption via over-broad replace.** These carry no `qmd`
  prefix and must stay byte-identical (corpus.rs invariants + reverse-sync totality). Acute trap:
  `mod.rs` reuses class *strings* as block-id **values** — `data-block-id="qmd-title-block"`
  (`mod.rs:667`) and `data-block-id="qmd-footnotes"` (`mod.rs:581`). A scoped `s/qmd-/tali-/` on
  class literals **will** hit these. **Mitigation:** exclude `data-block-id`/`data-sourcepos`/
  `data-source-file` from all substitution, and keep those two synthetic id *values* legacy for
  diff continuity (ids are opaque). Run `cargo test -p taliesin-core` before and after.
- **Deck contract self-consistency** across `deck.rs` emitter ↔ `deck.css` ↔ `deck.js` ↔
  `client.js:448`. CSS classes are stringly-typed: a missed occurrence drops behavior with no
  compile error. **Mitigation:** the dual-class structural containers + `:where()` alias layer
  (§4.3); keep `window.QmdDeck` a live same-object reference; lock in via `extensions.rs:470`
  (legacy `.qmd-deck` still themes) + a new `tali-*` presence assertion + a headless deck smoke
  test.
- **Click-to-source wire protocol** (`qmd-goto` / `qmd-cursor` postMessage) is co-owned by
  `web-client/client.js`, the VS Code extension (`extension.ts`, `webview.ts`, `relay-harness.cjs`),
  and is *the* click-to-source invariant. Renaming one side alone silently kills it.
  **Mitigation:** either freeze `qmd-goto`/`qmd-cursor` as permanent internal IPC strings
  (invisible, recommended), or rename in **one** coupled commit with both receivers accepting
  old-or-new. Never rename the payload keys `source_file`/`sourcepos` (they mirror the data-*
  invariants). Verify with the headless relay-harness, both directions.
- **CSS var fallback precedence** — covered in §4.3 (read legacy as fallback in the consuming
  property; never alias at `:root`).

### Five surfaces the inventory missed (now folded in)

1. Schema self-references + bless byte-equality (§4.3).
2. The `qhl-` highlight prefix (§4.3).
3. `.claude/settings.json` allowlist globs + cargo-fmt hook (§4.2).
4. The bundled favicon (§4.2).
5. The two `data-block-id` *values* that are `qmd-` class strings (above).

## 6. Phasing and ordering

Each phase is independently shippable; within a phase, the listed atomic units must land in one
commit or the tree is red.

- **Phase 0 — Cleanup.** Delete dead `.quarto-*` CSS; fix misleading comments. No name dependency.
- **Phase 1 — Author vocabulary + deprecations.** `revealjs`→`deck`, `ojs_define`→`define`,
  `.r-stretch`→`.tali-stretch` (CSS dual-selector). Each pinned by a corpus doc. **Must be live
  before** any corpus/docs deck flips to `format: deck` (else `{{< embed tour >}}` in
  `guide/index.qmd` fails to render).
- **Phase 2 — Brand rename.**
  - *2a (atomic):* package names + all `qmd_fast_core`→`taliesin_core` use-paths + compiled
    doctests + `Cargo.lock` regen (`cargo build`). Binary rename + `tali` symlink. `env_compat`
    helper + the two `set_var` writers. `.claude/settings.json` globs. Launcher scripts (manual).
  - *2b:* `.qmd`→`.tmd` central constant + recognition slice + reverse-checker candidates +
    `is_index_page` + warn-nudge + init scaffold; `git mv` corpus + docs to `.tmd` (keep **≥1**
    corpus `.qmd` to pin the deprecated-input path + the nudge).
  - *2c:* docs/README identity reframe (keep migration headings), favicon swap, schema dir
    `.qmd`→`.tali` + filenames + titles + **re-bless**.
- **Phase 3 — `qmd-*` → `tali-*` prefix (with aliases).**
  - *3a (atomic per rename):* CSS classes — emitter string + 4 CSS files + JS asset + web-client
    together; `:where()` alias layer; dual-class structural deck containers; exclude the two
    synthetic block-id values.
  - *3b:* CSS vars — `theme.rs` emits `--tali-*`; consuming properties read `var(--tali-x,
    var(--qmd-x, default))`.
  - *3c:* JS globals — two-way same-object aliases for the 3 public globals + deprecation getter;
    internal globals in lockstep; **dual-accept** `qmd-define` + `qmd-goto`/`qmd-cursor`; cell
    `tali` alias arg; preserve localStorage keys. **No `FORMAT_VERSION` bump.**
  - *3d:* `qhl-` → `tali-hl-` (emitter + base.css + test). Independent.

**Ships independently:** the `env_compat` helper, the `.qmd`→`.tmd` recognition slice, the
favicon swap, and the `qhl-` rename are each self-contained.

## 7. Deliberate keeps (do NOT "de-Quarto" these)

- `.quarto` cache-dir skip (`serve.rs:1060`, `main.rs:1151`) and the `_quarto.yml` migration
  breadcrumb (`main.rs:343-352`) — they detect the **user's** real Quarto artifacts; only the
  self-name token in the message changes.
- `config.rs:69` (`quarto_shaped_config_is_no_longer_parsed_and_warns`) + `stale_docs.rs` +
  `validation.qmd:62-69` "clean break / no-tolerance-tier" stance — keep tests + stance; rebrand
  only the self-name. The "Coming from Quarto" / "Quarto migration" doc **headings** keep the word
  Quarto (that is where switchers land); only **identity** lines are reframed.
- All Pandoc/Markdown syntax + verb shortcode names (`include`/`embed`/`video`/`input`).
- Structural dir names `_site.yml` / `_freeze/` / `_extensions/` / `_book/`.
- The third-party `liquid-glass-revealjs` extension folder keeps its `-revealjs` suffix (external
  repo), so `extension_ref` must keep accepting `-revealjs` **indefinitely**.
- `third_party.rs:43` (reveal.js/highlight.js stay gone from `THIRD_PARTY.md`) and `tech_blog.rs:221`
  (no `quarto-ojs-runtime`/`window._ojs`) — keep green; the rename must not re-introduce those
  literals while explaining deck-engine history.
- `data-block-id` / `data-sourcepos` / `data-source-file` attribute names + the wire payload keys
  `source_file`/`sourcepos`.

## 8. Verification (corpus-plus-roadmap)

- The whole corpus + docs `git mv` to `.tmd` must still render **green**; `cargo test -p
  taliesin-core` before and after every atomic commit.
- Tests pin the back-compat guarantee both directions: (a) `.qmd` input + `format: revealjs` +
  `ojs_define` + legacy `.qmd-deck`/`--qmd-*`/`window.QmdDeck` still work **with** a deprecation
  nudge; (b) `.tmd` / `deck` / `define` / `tali-*` are the documented path. Keep ≥1 corpus doc on
  each old spelling; add a corpus doc on each new spelling in the same change.
- Headless `relay-harness` exercises click-to-source both directions; a browser deck smoke test
  asserts the deck still mounts and a vendored `.qmd-deck` selector still themes it.
- `cargo build` regenerates `Cargo.lock`; the schema **bless** byte-equality test stays green.

## 9. Open questions (resolve in the implementation plan)

- Is `.qmd` recognition permanent-with-nudge, or time-boxed (removed in release vN)? The slice
  makes either trivial; it only changes the nudge wording.
- Stem-collision policy when `x.qmd` and `x.tmd` coexist mid-migration: recommend prefer `.tmd`
  + warn (keep an in-progress migration building) over a hard error.
- `qhl-` prefix: rename to `tali-hl-` (recommended, coherence) or keep as a documented exception?
- How many releases does the `QMD_FAST_*` / `qmd-*` deprecation window live before removal?
- `tali` shorthand: PATH symlink (recommended — one compiled target) vs a second `[[bin]]`.
- Rename the `qmd-goto`/`qmd-cursor` IPC strings to `tali-*`, or freeze them as permanent internal
  protocol (recommended — invisible, highest breakage risk for zero author benefit)?

## 10. Why this is the right "Beyond Quarto"

Quarto's identity is something qmd-fast already left behind at the engine. This change finishes
the job at the level of *name and vocabulary* without charging the author a relearning tax:
Taliesin leads with its own spelling and stands as its own tool, while every Markdown convention a
writer already knows — and every existing Quarto/Jupyter doc — keeps working. Power and
familiarity stop being a trade-off, which is the whole point of doing the rename natively rather
than as a clean break that punishes the people you most want to switch.
