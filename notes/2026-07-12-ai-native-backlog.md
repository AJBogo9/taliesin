# AI-native authoring — grounded backlog (2026-07-12)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Detail file for the **AI-native authoring** initiative (backlog.md §G + Tier 2/3). Turns the
10 Session-2 ideas in [FEATURE-IDEAS.md](FEATURE-IDEAS.md) into grind-ready, code-anchored entries.

> **How this was produced.** Owner-directed audit + brainstorm (the `brainstorming` skill) →
> a 20-agent grounding workflow (one research agent + one adversarial verifier per idea). **Every
> code anchor below was opened read-only and checked against the source**; verdicts were 5
> `confirmed` / 5 `corrected` (anchor line-fixes), 0 `flagged`. Line numbers were accurate as of
> commit `9dfcf76` (branch `publish-hardening`) — re-grep before trusting a line if HEAD has moved
> much; the repo's own rule is *trust the symptom, re-derive the line*.

**Theme.** Taliesin already has excellent machine-facing primitives (`check --format json`,
`vocab`/`schema`/`symbols`, the block model), but they were built for the VS Code companion / CI,
not framed or packaged for a coding agent. The gaps are the **protocol** (how an agent learns the
loop), the **closed loop** (reading its own output without a browser), and the **grain**
(diagnostics an agent can match on and auto-fix). Full framing: FEATURE-IDEAS.md Session 2.

**Invariants that hold throughout** (none of these items violates one): single editing surface (the
`.tmd` is the only edit surface, preview read-only; a CLI that writes `.tmd` *source* is fine, only
the preview is read-only), HTML-only output, offline/self-contained (the one sanctioned exception is
#8's opt-in `--online` citation check, off by default), block-model (`data-block-id` +
`data-sourcepos` on every block). Do-NOT-touch (divs.rs three-pass, cite.rs, includes.rs, numbering
scanners, exec/freeze/kernel) is entered by **no** item except #8/#9's small *additive* read-only
accessor on `cite::Bibliography` (flagged in those entries).

---

## Dependency graph + recommended build order

```
#1 AGENTS.md ─────────────────► #4 skill (soft: crib source; can ship first + retrofit)
#2 read/text-projection ──┬───► #6 MCP (the `read` tool)
                          └───► #9 published-legibility (per-page text sidecar)
#5 map ───────────────────────► #6 MCP (the `map` tool)
#3 diagnostics · #7 scaffolds · #8 alt-nudge · #10 build-json  — no deps (foundational)
```

**Recommended order:**
1. **The loop-closers (Tier 1, recommended first):** #1 AGENTS.md → #2 `taliesin read` → #3
   agent-grade diagnostics. Together they are the whole browser-free authoring loop.
2. **Easy foundational wins (no deps):** #5 `map`, #7 scaffolds, #10 structured build errors, and
   #8's *placeholder-alt* slice (the lowest-controversy validator, no ruling, no network).
3. **After #1 lands:** #4 the Claude Code skill (reuse AGENTS.md as the single crib source).
4. **After #2 + #5 land:** #6 the MCP server (or ship a v1 wrapping only check/symbols/vocab/build
   and add the map/read tools later). Then #9 published-artifact legibility, and #8's opt-in
   `--online` DOI check.

Each item is one branch (repo working method). Owner rulings tagged **[ruling needed]** should be
settled before the branch starts — several are cheap (naming, default-on/off).

---

## Tier 1 — the loop-closers (recommended first)

### 1. Generated `AGENTS.md` (the agent onramp) — size S–M, no deps — *confirmed*

**What.** A core generator `crates/core/src/agents.rs::agents_md() -> String` emits an `AGENTS.md`
that teaches an agent the whole protocol in one file: the four pillars (edit the `.tmd` never the
preview; the `check --format json` gate; `symbols`/`vocab`/`schema` for discovery; the build/publish
commands) plus a **dialect section** whose vocabulary is generated from `taliesin_core::vocab::vocab()`
so it can't drift from what `check` enforces. Ship a golden-locked committed copy, a repo-root
`AGENTS.md`, and have `cmd_init` (and per ruling `cmd_new`) write one into the scaffolded project.
**No new subcommand** (rides the existing `init`/`new` arms, dodging the COMMANDS/completions/env
drift gates).

**Why.** Turns every existing primitive into a discoverable, drift-proof protocol so an agent drives
Taliesin correctly on first contact instead of guessing from stale Quarto priors.

**Verified anchors.**
- `crates/server/src/main.rs:50` — `Some("init") => cli::cmd_init(...)`, `:51` `Some("new") => cli::cmd_new(...)`. Ride these; no new subcommand.
- `crates/server/src/cli.rs:58` — `scaffold_init()` write loop (refuse-before-overwrite pre-check `:65-73`, write `:74-82`, file set = `files` array `:62`). Add `AGENTS.md` to the written set here so a partial scaffold never lands.
- `crates/server/src/cli.rs:287` — `write_new()` writer + pre-check `:289-297` (hook here only if `new` also emits it — ruling below).
- `crates/core/src/vocab.rs:219` — `pub fn vocab() -> Value`; names come from validator consts (`KNOWN_KEYS`/`CELL_OPTION_KEYS`/`CALLOUT_KINDS`/`THEOREM_KINDS`/`INPUT_TYPES`/`XREF_LABELS`). Generate the dialect section from this.
- `crates/core/src/vocab.rs:263` — `vocab_matches_committed` golden-lock test (regen via `TALIESIN_BLESS`) — the exact pattern to mirror for `agents_md_matches_committed`.
- `crates/core/src/lib.rs:42` — `pub mod vocab;` → register `pub mod agents;` beside it.
- `crates/server/src/cli.rs:658` — `every_scaffold_matches_its_corpus_pin` **byte-compares** `new_files()` to `corpus/scaffold/`. **Trap:** do NOT route the generated (vocab-derived) `AGENTS.md` through the byte-pinned `new_files` vec, or every vocab change breaks this fixed pin. Golden-lock it in core instead.

**Pin.** New golden-lock unit test `agents_md_matches_committed` in `crates/core/src/agents.rs`
(mirrors `vocab.rs:263`): generated markdown must equal committed `crates/core/assets/agents/AGENTS.md`,
regen only via `TALIESIN_BLESS`. Plus `crates/server/tests/agents_md_cli.rs`: `taliesin init <tmp>`
writes an `AGENTS.md` whose body contains vocab-sourced dialect terms (a callout kind, `#| label:`,
`[@key]`) AND the literal "edit the .tmd"/never-the-preview rule AND the `check --format json` gate.

**New files.** `crates/core/src/agents.rs`, `crates/core/assets/agents/AGENTS.md`, `AGENTS.md` (repo
root), `crates/server/tests/agents_md_cli.rs`.

**First step.** Write the failing golden-lock test: add `agents.rs` with `pub fn agents_md()` sourcing
dialect vocab by walking `crate::vocab::vocab()`; add `#[test] agents_md_matches_committed` (compares to
committed asset, blesses under `TALIESIN_BLESS`) + assert the body contains a known vocab term and the
four pillars. Red until the asset is blessed and the pillars written.

**[ruling needed]** (a) Does `new` also emit `AGENTS.md`, or only `init`? (`new` can target an existing
project that may already have one.) (b) Default-on vs an opt-out flag for the scaffold write. (c) Scope
of the Quarto-divergences narrative (which divergences, inline vs own section — this part is authored
prose; `vocab()` has no divergence field). (d) Confirm golden-lock-in-core over the `corpus/scaffold`
byte-pin as the pinning home.

**Gotcha.** `divClasses` is hand-enumerated in `vocab.rs:176` (not a validator const), so "cannot drift
from check" is bounded by `vocab()` itself, not by a `check` const — fine, but don't overclaim it.

---

### 2. `taliesin read <page>` — text projection of the built page — size M, no deps — *corrected*

**What.** A deterministic, screen-reader-like TEXT projection of a rendered doc: a new block-model
emitter that walks `doc.blocks` and turns the already-built block HTML into plain structured text
(headings, resolved "Figure N"/xref numbers, figcaptions + img `alt`, callout kinds, code with its
language, listings, math as raw TeX, references). Expose as a read-only CLI arm (`taliesin read
<file.tmd>`), parse-only like `render`/`symbols`. Core logic in a new `render/text.rs`; a
`RenderedDoc::body_text()` mirrors `body_html()`. **A VIEW, not a new output format** (HTML-only holds).

**Why.** Closes the see-what-you-made loop that today only a human-with-browser gets — an agent (or a
blind author) can read what it produced and diff it, no browser, no HTML noise.

**Verified anchors.**
- `crates/server/src/main.rs:42` — `Some("render") => query::cmd_render` is the exact sibling to copy for `Some("read") => query::cmd_read(...)`.
- `crates/server/src/main.rs:90` (COMMANDS, add `"read"`), `:161` (usage() help line), `:234` (`subcommand_help()` — add a `"read"` arm). The gates `every_dispatched_command_is_listed_in_commands` (`:530`) and `subcommand_help_covers_documented_commands` (`:548`) FAIL without all three.
- `crates/server/src/query.rs:18` — `cmd_render` is the exact template for `cmd_read` (directory_rejection, read_to_string, `serve::guarded(...)`, print to stdout). `:50` is the "render does not execute cells" warn to mirror if `read` is parse-only.
- `crates/core/src/render/model.rs:253` — `RenderedDoc::body_html()` (pushes `b.html` + `\n` per block) → add a sibling `body_text()` concatenating the per-block text projection.
- `crates/core/src/render/mod.rs:1870` — `fn strip_tags` (KaTeX-aware HTML→visible-text; drops the `<math>` subtree keeping glyphs). Reuse it, but **override math handling** — you want raw TeX, and `strip_tags` currently drops the `<annotation>` TeX.
- `crates/core/src/cite/render.rs:90` — `cite::process` resolves `@fig-`/`@sec-`/`[@key]` into linked "Figure 3" text **in `b.html` before render returns**, so projecting from `block.html` yields resolved numbers for free. `:181` `transform_html` (SKIP = pre/code/script/style/**annotation**) is the tag-walk model; note `annotation` is skipped so raw TeX survives to the projector.
- `crates/core/src/render/figure.rs:104`/`:121` — figure `<img ... alt="{alt}">` and figcaption "Figure&nbsp;{num}: …". Projector must read the alt **attribute** (`strip_tags` drops attrs) and emit the figcaption text.
- `crates/core/src/render/divs.rs:443` — **[corrected from :398]** the callout class literal `<div class="callout callout-{kind} ...">` (`:447` plain). Read `{kind}` from the class to label callouts.
- `crates/core/tests/body_html_snapshots.rs:55` — `assert_snapshot` + `UPDATE_SNAPSHOTS` pattern to mirror (existing snapshots are `.html`; a new `.txt` is fine).

**Pin.** `crates/core/tests/text_projection.rs`: byte-exact `.txt` snapshot of `body_text()` over a
rich doc with headings + a labelled figure + a callout + a code cell + math + a cross-reference. Reuse
`corpus/reader/hovercards.tmd` (has `@fig-flow` + `{#sec-why}` + `%%| label: fig-flow`; NOT one of the
`{js}` docs `body_html_snapshots.rs` pins, so safe) or add `corpus/reader/text-projection.tmd`.
Satisfies the Session-2 pin ("lists every heading/figure/xref number"). Optional `crates/server/tests/read_cli.rs` mirroring `symbols_cli.rs`.

**First step.** Add `pub fn body_text()` to `RenderedDoc` (beside `body_html` at `model.rs:253`)
delegating to `render::text::project(&self.blocks)`; register `mod text;` in `render/mod.rs`. Write the
failing unit test in `render/text.rs`: render `# Heading\n\n![cap](x.png){#fig-a}` and assert the
projection contains `# Heading` and the resolved `Figure 1: cap` (proves it reads post-cite `block.html`,
not the AST).

**[ruling needed]** (a) Naming: `taliesin read <file>` vs overloading `render --format text` (render has
no `--format` today; FEATURE-IDEAS leads with `read`; `read` also settles the "VIEW not format" framing).
(b) Kernel exec: MVP parse-only/static like `render`/`symbols` (python/r cells project as source, outputs
empty, warn) vs execute-then-project like `build` (pulls in the exec/freeze path). **Recommend `read` +
parse-only** for the first cut.

---

### 3. Agent-grade diagnostics (codes / severity / suggestion) — size M, no deps — *corrected*

> The grounding agent rated this **tier-2** (foundational/additive). Kept in the recommended-first trio
> because it completes the loop (#1 protocol → edit → **#3 machine-matchable check** → #2 read). Owner's
> call whether it's literally "third" or "soon after".

**What.** Promote every `check --format json` diagnostic from `{file,line,message}` to
`{code, severity, file, line, column?, message, suggestion}`: stable per-family codes (e.g.
`TAL-XREF-UNDEF`, `TAL-FM-KEY`, `TAL-A11Y-ALT`) an agent can match on, plus today's inline "did you
mean X" also surfaced as a structured `suggestion{replacement}`. Additive to `Warning` (core) and
`Diagnostic` (check.rs); **`--format human` output stays byte-identical** (the load-bearing new invariant).

**Verified anchors** (all 11 confirmed accurate).
- `crates/core/src/render/model.rs:146` — `struct Warning{message,file,line}`; add `code`/`Severity`/`Option<Suggestion>` (+`column`), init in `Warning::new` (`:153`) and `.at` (`:162`), add `.code()`/`.suggest()` builders.
- `crates/server/src/check.rs:19` — `struct Diagnostic` (derives `Serialize`); add the new fields; `diag_from` (`:25`) copies them off the Warning; `format_json` (`:304`) serializes.
- `crates/server/src/check.rs:104` — the NON-Warning diagnostic sources that build `Diagnostic{}` directly (`yaml_error` `:104`, missing-config `:133`, unreadable page `:141`, site yaml `:148`) — each must assign a stable code at this boundary (no Warning carries one).
- `crates/server/src/check.rs:320` — `format_human` — **must stay unchanged** (byte-identical human output). `format_json` is `:304`.
- `crates/core/src/frontmatter.rs:456` — `unknown_key_message` (the shared did-you-mean builder; `closest()` `:443`) — emit a structured suggestion alongside the prose.
- `crates/core/src/cite/validate.rs:33` — `validate_xrefs` broken-xref did-you-mean (`suggest()` `:73`) — the "bad xref" half; `TAL-XREF-UNDEF` + `suggestion.replacement`.
- `crates/core/src/diagnostics/reactive.rs:90` — `closest_owned` inline `(did you mean X?)` — representative dynamic-suggestion site (also a11y/anchors/assets under `diagnostics/`).
- `crates/core/src/diagnostics/mod.rs:18` — module set (`:33+` pub use); natural home for a new `codes.rs` catalog + `Severity` enum.
- `crates/server/src/main.rs:221` — the `check` help block; the `--format json` line (`:229`) currently misdescribes JSON as an array — it's an **object** `{diagnostics,environment}`. Fix while here.
- `crates/server/tests/symbols_cli.rs:22` — `CARGO_BIN_EXE_taliesin` + parse-stdout pattern for the new `check_cli.rs`.

**Pin.** `crates/server/tests/check_cli.rs`: run `taliesin check corpus/diagnostics/typos.tmd --format
json` (typo'd keys `treme`/`eccho`/… → codes + non-null `suggestion.replacement`, e.g. `treme`→`theme`)
and `check-superset.tmd` (codes across anchor/asset/bib/math/code-lang/dup-id/theorem), asserting each
`.diagnostics[].code` is a stable non-empty string and typos carry `suggestion.replacement`, PLUS a
committed golden that `--format human` stdout is byte-identical. **Caveat:** `check-superset.tmd` fires
no `TAL-XREF-UNDEF` suggestion (its `@`-refs are backticked/citations), so to pin a structured xref
suggestion add a `@fig-reslts` near a `fig-results` label to `typos.tmd` (or a tiny new doc).

**New files.** `crates/server/tests/check_cli.rs`, `crates/core/src/diagnostics/codes.rs`.

**First step.** Failing test in `check_cli.rs`: `check corpus/diagnostics/typos.tmd --format json`, assert
`parsed["diagnostics"][0]["code"]` is a non-empty string and the `treme` diagnostic carries
`suggestion["replacement"] == "theme"`. Fails today (no fields), pinning the JSON shape first.

**[ruling needed]** (a) Code scheme + granularity (`TAL-<FAMILY>-<SPECIFIC>`, per-family vs per-message,
golden-pinned catalog?). (b) Severity taxonomy (error/warning/info) and whether it ever affects the exit
code (today any finding → exit 1; likely keep all `error`, exit unchanged). (c) Did-you-mean: keep inline
AND duplicate to structured (safe, byte-identical) vs move-out-and-re-append (single source, riskier).
(d) Include `column` now or defer. (e) Expose the code catalog via `taliesin vocab`?

---

## Tier 2 — packaging, scaffolds, guardrails

### 4. Taliesin Claude Code skill/plugin — size S–M, soft dep #1 — *confirmed*

**What.** A distributable Claude Code plugin whose `taliesin` skill teaches any Claude Code user (not
just this repo) the loop — edit `.tmd` → `check --format json` → fix → `build --strict` — plus the
Pandoc/Quarto dialect crib with Taliesin's divergences and the "edit source, never the preview" rule.
Greenfield markdown driving existing CLI seams; no engine change. The command list + dialect crib are
**pinned against the live binary** so they can't rot the way the retired external scaffolder did.

**Verified anchors.**
- `crates/server/src/main.rs:41` — the match-dispatch; the skill must reference only these real verbs (`check`/`build`/`preview`/`symbols`/`vocab`/`new`) and their real flags. `:90` `const COMMANDS` (private to the bin — not importable by a test; drive the binary instead).
- `crates/server/src/check.rs:304` — `format_json` → `{diagnostics,environment}` (Diagnostic `:19` = `{file,line,message}`) — the shape the skill tells the agent to parse.
- `crates/core/src/vocab.rs:15` — `VOCAB_JSON` (golden `include_str`, regen via `TALIESIN_BLESS`) — the drift-free source for the dialect/divergence crib. Derive, don't hand-author.
- `corpus/tech-blog/.claude/skills/deploy/SKILL.md:1` — the only repo-authored SKILL.md precedent (mirror its frontmatter shape). NOTE it's project-local, not a distributable plugin; the new one must be packaged for `--plugin-dir` install. (Its body still says "Quarto" — copy only the frontmatter shape.)
- `crates/server/tests/new_cli.rs:4` — the docstring recording the exact rot-trap to avoid: the prior hand-written scaffolder emitted `.qmd`/`quarto preview` and rotted because it lived outside the binary. Mandates the drift-guard pin.

**Pin.** `crates/server/tests/skill_freshness.rs`: read the shipped SKILL.md, extract every `taliesin
<verb>`, assert each is dispatchable against `CARGO_BIN_EXE_taliesin`, and forbid stale tokens (`.qmd`,
`quarto`, `revealjs`, `.reveal`, `Reveal.js`). If the crib is generated from `vocab`, add a golden-file
assert (`TALIESIN_BLESS`-style).

**New files.** `editor/claude-code/.claude-plugin/plugin.json` (confirm the current plugin schema first),
`editor/claude-code/skills/taliesin/SKILL.md`, `crates/server/tests/skill_freshness.rs`, optional design
doc under `docs/superpowers/specs/`.

**First step.** Failing `skill_freshness.rs::skill_names_only_real_subcommands`: read the (not-yet-existing)
SKILL.md, regex `` `taliesin (\w+) ``, run each verb's `--help` asserting exit 0 (an unknown verb exits
non-zero). Fails on the missing file → author SKILL.md + manifest until green. **Note:** not every verb has
a dedicated `--help` page; the `taliesin completions bash` name-list is the more robust dispatch check.

**[ruling needed]** (a) Plugin dir + name (`editor/claude-code/` mirroring `editor/vscode/` vs a top-level
`.claude-plugin/`). (b) Skill-only vs full plugin (bundle a `/preview` slash command too?). (c) In-repo
`--plugin-dir` install vs a published marketplace entry. (d) Wait on #1 (single crib source) or ship first
and retrofit.

### 5. `taliesin map --format json` (project outline) — size M, no deps — *corrected*

**What.** A read-only `taliesin map <dir> [--format json|human]` that emits the whole-project outline
in one call: title, is_book, output_dir, the page list (rel/url/title/date/description/categories/
page_layout, in nav/chapter order), nav + mounts config, the cross-reference graph (forward
`xref_targets` + reverse `backlinks`), and embedded decks. Reuses `Site::discover` (no kernel); new
serde structs in `query.rs` mirroring how `cmd_symbols` serializes `Symbol`.

**Verified anchors.**
- `crates/server/src/main.rs:48` — dispatch (sibling to `Some("symbols")`); add `Some("map") => query::cmd_map(&args)`. `:90` COMMANDS (add `"map"` or the `:530` gate fails), `:170` usage() line, `:279` `subcommand_help` `"map"` arm (or the `:548` gate fails).
- `crates/server/src/query.rs:232` — `cmd_symbols` is the template for `cmd_map` (flag parse `:238`, serde structs, guarded render `:282`, json to stdout `:290`). **`map` takes a DIRECTORY** so it calls `Site::discover`, NOT `directory_rejection` (`:309`). `:186` `struct Symbol` (+ `collect_symbols` `:206`) is the serde precedent for `ProjectMap`/`PageEntry`.
- `crates/core/src/site/mod.rs:137` — `struct Site` pub fields: `config` `:139`, `pages` `:140`, `book` `:142`, `xref_targets` `:145`, `backlinks` `:151`, `decks` `:176`. Exported as `taliesin_core::Site`. `Site::discover` `:229`, `is_book()` `:349`, `output_dir()` `:355`.
- `crates/core/src/site/mod.rs:36` — `struct Page` pub fields → `PageEntry` (field is `page_layout`, **not** `layout`).
- `crates/core/src/site/config/mod.rs:28` — `SiteConfig` (`output_dir`/`title`/`nav`/`mounts`; `Mount` `:71`, `Navbar` `:90`, `NavItem` `:110`).
- `crates/core/src/site/discovery.rs:21` — **draft trap:** `if fm.draft { ... }` excludes the page, so `Site.pages` excludes drafts; surfacing a `drafts` field needs a separate raw walk or an additive `Site.drafts` list.
- `crates/core/src/site/xref.rs:12` — `struct XrefTarget { url, number }` (forward); `Site.backlinks` is the reverse.
- `crates/server/tests/symbols_cli.rs:32` — the JSON-CLI test template for `map_cli.rs` (no `insta` in the workspace — structural asserts, not `.snap`).

**Pin.** `crates/server/tests/map_cli.rs`: `map corpus/demo-book --format json` → assert `is_book==true`
and `pages[].url` equals the `chapters:` order `[index,intro,methods,results,summary].html` (verified in
`_site.yml`) and the xref-graph key is populated (demo-book has `@sec-methods`/`@sec-setup`/`@thm-kl`);
plus `map corpus/tech-blog --format json` (nav order Blog/Publications/Projects/CV); plus one temp fixture
(mirror `check.rs:442` `tmp()`) exercising `draft:` and `mounts:` (no corpus site has either).

**First step.** Failing `map_cli.rs::map_book_lists_chapter_order_and_xref_graph` against
`corpus/demo-book`, then add `query::cmd_map` and dispatch from `main.rs:48`.

**[ruling needed]** (a) Default `--format` — human (consistency) vs json (map's customer is an agent).
(b) Scope of an `assets` field (config-level only, or a full per-page local-asset scan). (c) Include a
`drafts` list (additive `Site.drafts` vs a server re-walk)? (d) Reject a single file, or emit a one-page
map for a `.tmd`?

### 7. Correct-by-construction scaffolds + `--json` on `new`/`init` — size S–M, no deps — *confirmed*

**What.** Richer check-clean `new` templates that pre-wire citations (a `paper`/`research-report` kind
whose `index.tmd` declares `bibliography: [references.bib]`, cites a real `[@key]`, and ships a matching
`references.bib` in the same scaffold; optionally a `book` kind), and a `--json` flag on `new` and `init`
printing `{kind, slug, created:[paths], preview}` so an agent knows exactly what it made and where.

**Verified anchors.**
- `crates/server/src/cli.rs:178` — `new_files(kind,slug,today) -> Vec<(PathBuf,String)>` — THE seam; return type is already a `Vec` so a Paper arm returns TWO entries (`index.tmd` + `references.bib`).
- `crates/server/src/cli.rs:87` — `enum NewKind { Post, Page, Deck }` (add `Paper`/…); `:94` `NEW_KINDS` + `:97`/`:104` parse + None text must list new kinds or the did-you-mean unit test (`:645`) goes stale.
- `crates/server/src/cli.rs:233` — `cmd_new` arg loop (parse `--json`; add to `NEW_FLAGS` `:283`); under `--json` suppress the human hint (`:272`) + `log::built` (`:269-271`) so stdout is pure JSON. `:33`/`:45` `cmd_init` same flag.
- `crates/server/src/cli.rs:658` — `every_scaffold_matches_its_corpus_pin` iterates a HARDCODED `[Post,Page,Deck]` (`:660`) and reads `corpus/scaffold/`; add the new kind+slug AND create the corpus subdir, or the pin panics (fixed date `"2026-07-10"` at `:666`).
- `crates/server/tests/new_cli.rs:46` — `every_scaffold_passes_check_with_no_diagnostics` (loop `:47-51`) runs the real binary + `check` on each scaffold; the `[@key]` must resolve against the shipped `references.bib` and declare `bibliography:` or check fails.
- `crates/core/src/diagnostics/bibliography.rs:8` — `citations_without_bibliography` (a `[@key]` with no `bibliography:` warns — declaring it silences). `crates/core/src/render/mod.rs:887` — a DECLARED-but-missing bib file warns, so the scaffold MUST write `references.bib`.
- `crates/core/src/frontmatter.rs:19` — `KNOWN_KEYS`: `bibliography`/`author`/`subtitle` known; there is **NO `abstract` key** — a paper template must not use one.
- `corpus/posts/cite-coverage/index.tmd:1` — the working reference pattern to copy (bibliography front matter + `[@key]` + `# References`, already check-clean, ships its own `references.bib`).

**Pin.** Byte pin: new `corpus/scaffold/posts/my-paper/{index.tmd,references.bib}` enforced by
`cli.rs:658` (add kind+slug to its loop). Check-clean: extend `new_cli.rs:46`. `--json` shape: a new
`new_cli.rs` snapshot asserting the parsed `{kind,slug,created,preview}` (model after `symbols_cli.rs`/
`schema_cli.rs`).

**First step.** Add a row `("paper","my-paper","posts/my-paper/index.tmd")` to the `new_cli.rs:47` loop,
run it (fails: unknown kind). Then add `NewKind::Paper` + its two-file `new_files` arm, extend
`NEW_KINDS`/parse, and create the corpus subdir so the byte pin passes.

**[ruling needed]** (a) Which templates ship + their kind names/slugs — the "perfect the default before a
knob" convention may push back on template proliferation; get a ruling on how many kinds land vs one
citation-wired `paper`. (b) `--json` key names + whether it lands on `init` too (default-off assumed).
(c) Book template layout (multi-chapter dir vs single doc).

### 8. Sharpen `check` as the LLM-mistake catcher — size L (sliced) — no deps — *corrected*

**What.** Three validators aimed at LLM co-author mistakes:
- **(b) placeholder-alt nudge** — STATIC, default-on: flags non-empty but useless alt (`alt="image"`,
  `"photo"`, `"figure"`, `"screenshot"`, or a bare filename echo) that today passes because
  `validate_a11y` only catches a *missing* alt. **The lowest-controversy slice — do this first (S–M).**
- **(c) numeric/quoted-claim-without-citation hint** — SOFT, opt-in on the existing `prose-lint` channel
  (FP-prone, never default-on). M.
- **(a) `check --online`** — opt-in network: resolve each bib entry's `doi`/URL (the ONE sanctioned
  network call, off by default, check-only so `build`/`publish` stay offline). M–L.

**Verified anchors.**
- `crates/server/src/check.rs:69` — `page_static_diagnostics` (the check-superset, shared by check AND build --strict); add the STATIC alt-nudge after the `validate_a11y` call at `:86`. **Do NOT add the network check here** (it would make build phone home).
- `crates/server/src/check.rs:332` — `CHECK_FLAGS` + parse (`:337-360`); add `--online`/`--verify-citations` and thread a flag through `collect_*` (currently flagless).
- `crates/server/src/check.rs:540` — `check_superset_has_no_false_positives_across_corpus` needle list (`:544-558`); a new default-on rule's message substring MUST be added here and the corpus stay clean (`diagnostics/` exempt via skip-list `:578-588`). `:732` `corpus_a11y_pin_doc_trips_each_rule_through_check` is the pin-test template.
- `crates/core/src/diagnostics/a11y.rs:284` — the raw-`<img>`-no-alt loop (`:284-305`); extend the family: read the alt VALUE via `helpers::tag_attr(tag,"alt")` (`helpers.rs:47`) and match a placeholder word-list / filename echo. Must NOT re-flag `alt=""` (the sanctioned decorative marker, message at `:297`).
- `crates/core/src/prose.rs:55` — `lint()` / `scan_line` (`:202`): the opt-in, front-matter-gated linter the SOFT numeric-claim hint rides. `crates/core/src/render/mod.rs:221` — where prose lint feeds the located channel (a prose-family hint joins here, NOT `page_static_diagnostics`).
- `crates/core/src/cite/mod.rs:46` — **[corrected]** `Bibliography{entries}` + `Entry{kind,fields}` are **PRIVATE**; `keys()` is only `pub(crate)` (unreachable from the server crate). The online check therefore CANNOT read `Entry.fields` as-is — it needs a small **additive read-only accessor** on `Bibliography` (e.g. an entries-with-doi/url iterator), and the DOI extraction must live in **core** (this touches the `cite` Do-NOT-touch machine; additive, not a rewrite — flag it in review).
- `crates/core/src/vocab.rs:81` — **[corrected]** `KNOWN_KEYS`/`PROSE_LINT_KEYS` live in `vocab.rs` (golden-file-locked `assets/vocab/tali-vocab.json`), NOT `frontmatter.rs`; a new opt-in key needs `TALIESIN_BLESS` regen.
- `crates/server/src/build.rs:400` (+`:792`) — `page_static_diagnostics` callers in `build --strict`; proof that a network call in the superset would break the offline invariant in build/publish. Static additions here are fine; network must not.

**Pin.** `corpus/diagnostics/llm-mistakes.tmd` (+ `llm-mistakes.bib` with one good `doi` + one dead) under
the guard-exempt `corpus/diagnostics/`; a check-side test mirroring `check.rs:732` asserts placeholder-alt
+ numeric-claim fire through real `collect_diagnostics`; the network path pinned by an `#[ignore]`d
`crates/server/tests/check_online.rs`.

**First step.** Failing unit test in `crates/core/src/diagnostics/tests.rs`: `![image](photo.png)` and a
filename-echo alt each produce a "placeholder/auto-generated alt text" warning while a descriptive alt and
`alt=""` stay clean; implement by extending `validate_a11y` (`a11y.rs:284`) with `helpers::tag_attr` + a
small word-list. Lowest-controversy slice — no ruling, no network.

**[ruling needed]** (1) Name + default of the network flag + explicit offline-invariant sign-off (sole
sanctioned egress) + resolver choice (doi.org HEAD vs Crossref) + sign-off on the additive
`cite::Bibliography` accessor. (2) Does the SOFT numeric-claim hint ship at all (FP risk; opt-in under
`prose-lint` or a new key, default-off recommended)? (3) Placeholder-alt default-on (recommended) or opt-in;
confirm the word-list (image/photo/figure/picture/screenshot/graphic + filename echo).

### 10. `build`/`publish` structured errors (`--format json`) — size M, no deps — *confirmed*

**What.** Add `--format json` to `build` and `publish` so a failing build in an agent/CI context emits
check's `{diagnostics:[{file,line,message}]}` to stdout instead of a human stderr log. The static-lint
diagnostics are ALREADY computed on the build path (`page_static_diagnostics`) but flattened to log
strings and discarded — retain them structured and serialize, reusing check.rs's exact shape so the two
channels can't drift.

**Verified anchors.**
- `crates/server/src/check.rs:17` — `Diagnostic` + `diag_from` (`:25`) + `format_json` (`:304`) + `json_error` (`:316`), all private today; make `pub(crate)` and reuse. `page_static_diagnostics` (`:69`) is ALREADY `pub(crate)` and called by build.rs at `:400`/`:792`.
- `crates/server/src/build.rs:50` — `struct BuildArgs` (add `format`); `BUILD_FLAGS` `:62` (add `--format`); `parse_build_args` `:68-121` (parse + reject bad value exactly as check does at `check.rs:365-370`).
- `crates/server/src/build.rs:338` — `build_page_executing` (single-doc): today counts `problems` + logs each via `locate()` (`:353/372/381/392/407/430`); COLLECT the located Warnings as `Vec<Diagnostic>` and return them up to `cmd_build`. `:226` `locate()` is the human formatter (don't re-parse the structured path back apart).
- `crates/server/src/build.rs:746` — `struct PageOutcome{warnings:Vec<String>}` (already `locate()`-flattened); carry Warning/Diagnostic structs too, preserving page-order replay (`build_site_async:1061-1070`; determinism pinned by `tests/parallel_build_determinism.rs`).
- `crates/server/src/build.rs:868` — `run_site_build -> bool` — SHARED by `cmd_build`'s dir branch AND `publish.rs:212`. Change its return to carry `Vec<Diagnostic>` (e.g. `SiteBuildOutcome{ok,diagnostics}`) so both serialize — **coupled edit across build.rs + publish.rs, one change**. `:903` `build_site_async` logs at `:920`/`:1054` (log::**error**, page panic)/`:1062`/`:1256`/`:1260` — collect them structured in deterministic order.
- `crates/server/src/publish.rs:29` — `PUBLISH_FLAGS`/`PublishArgs`/`parse` (add `--format`); `cmd_publish` `:131` calls `run_site_build` `:212` (thread structured diagnostics out; respect `--dry-run`).
- `crates/server/src/log.rs:1` — all `log::` goes to stderr, so build/publish stdout is free for JSON (mirror `check.rs:396-398`).
- `crates/server/tests/strict_robustness.rs:20` — the `CARGO_BIN_EXE_taliesin` harness (+ `tmp_dir` `:13`) to copy; the `--formt` did-you-mean precedent is `check_rejects_unknown_flag_with_suggestion` (`:220-242`).

**Pin.** `crates/server/tests/structured_build_errors.rs`: write a tmp doc tripping static lints (dup
heading id `{#dup}` + `![x](missing.png)`, kernel-free — both in the `Scope::Standalone` set at
`check.rs:78/80`), run `build --strict --format json`, assert stdout parses to
`{diagnostics:[{file,line,message}…]}` and that check's diagnostics for the same doc are a SUBSET of
build's (build is a superset — adds embed + cell-error outputs). Add a `publish <dir> --dry-run --format
json` case.

**First step.** Failing `structured_build_errors.rs`: `build <tmpdoc> --strict --format json`,
`serde_json::from_slice`, assert `parsed["diagnostics"]` is a non-empty array of `{file,line,message}`.
Fails today (`--format` is an unknown build flag). Then implement: `pub(crate)` the check.rs helpers, thread
structured collection through `build_page_executing` + `PageOutcome` + `run_site_build`, add flag + help.

**[ruling needed]** (1) Exit code under `--format json` — keep build's `--strict`-gated exit (recommended)
or adopt check's exit-1-on-any-diagnostic? (2) Include check's informational `environment` block (build
runs kernels, so it could) or just `{diagnostics}`? (3) Flag on both build AND publish, or build only?
(4) Suppress human log/progress on stderr in json mode?

---

## Tier 3 — demand-driven

### 6. `taliesin-mcp` MCP server — size M–L, soft deps #2 + #5 — *confirmed*

**What.** A local, offline, stdio JSON-RPC MCP server exposing Taliesin's read/validate surfaces
(check, symbols, vocab, map, read) plus build as MCP tools + resources, so MCP hosts drive the loop
without shelling out. **Recommended seam:** a `taliesin mcp` subcommand backed by a new
`crates/server/src/mcp.rs` that WRAPS the existing collection fns (not a re-implementation, not a
shell-out to itself). **READ/VALIDATE/BUILD ONLY** — no write/edit tool exists; the `.tmd` stays the
agent's direct edit surface (the single-editing-surface guardrail, pinned by the tools/list assertion).

**Verified anchors.**
- `crates/server/src/main.rs:49` — clone the `check` arm → `Some("mcp") => mcp::cmd_mcp(&args)`; `:90` COMMANDS (add `"mcp"` or the `:530` gate fails); `:187` `subcommand_help` `"mcp"` arm (or `:548` fails); `:10` `mod` block → add `mod mcp;`.
- `crates/server/src/check.rs:36` — `collect_diagnostics` (private) + `format_json` (`:304`, `Diagnostic` `:19`): the MCP `check` tool reuses these — promote to `pub(crate)`, don't duplicate the superset.
- `crates/server/src/query.rs:206` — `collect_symbols` (private) backs the `symbols` tool; `cmd_vocab`/`VOCAB_JSON` (`:179-180`) backs `vocab`.
- `crates/server/tests/symbols_cli.rs:21` — the spawn-the-binary + parse-JSON pattern for the stdio pin.
- `crates/server/Cargo.toml:26` — `serde_json` (`:26`) + `tokio` (`:28`) already present; a newline-delimited JSON-RPC stdio loop needs no new crate (hand-roll like `check::format_json`). Only if the owner picks the official `rmcp` SDK does a vendored-offline dep get added (workspace members `Cargo.toml:3`).

**Pin.** `crates/server/tests/mcp_stdio.rs`: spawn `taliesin mcp`, drive JSON-RPC over stdin
(`initialize` → `tools/list` → `tools/call check` on a bad-xref fixture like `corpus/diagnostics/refs.tmd`).
Assert (a) tools/list is exactly {check,symbols,vocab,map,read,build} with **NO write/edit/preview tool**
(the single-editing-surface pin), (b) the check tool returns the same `{diagnostics,environment}` as
`check --format json`, (c) valid JSON-RPC on stdout, log noise on stderr only.

**First step.** Failing `mcp_stdio.rs::tools_list_exposes_read_validate_build_only` — spawn `taliesin mcp`,
`initialize` + `tools/list`, assert no write/edit tool and the read/validate/build set — before `mcp.rs`
or the dispatch arm exist.

**[ruling needed]** (1) Surface — `taliesin mcp` subcommand vs a separate `crates/mcp` binary. (2)
Transport/dep — hand-rolled JSON-RPC-over-stdio (zero deps, offline-guaranteed) vs vendoring `rmcp` (needs
offline vetting). (3) Is `build` (writes to disk) in-scope for a "read/validate/build-only" server, and its
out-dir policy? (4) May a v1 wrap only existing check/symbols/vocab/build (deferring map/read until #5/#2
land) or wait for all six?

### 9. Strengthen published-artifact AI-legibility — size M–L, dep #2 — *corrected*

**What.** Three additive, url-gated build outputs for reader-side machines: **(A)** a clean per-page
text/JSON sidecar (reuses #2's projection; falls back to `Site::page_prose` until it lands); **(B)**
schema.org `ScholarlyArticle`/`Dataset` JSON-LD for research posts, upgrading the existing `BlogPosting`
branch in `meta.rs`; **(C)** a per-page BibTeX/CSL-JSON export of only the entries a page actually cited.

**Verified anchors.**
- `crates/server/src/build.rs:1163` — the `if site.config.url.is_some()` SEO-sidecar block + `emit` closure (`:1164`); global per-page sidecars (A/C) slot here (files via `emit` auto-kept by the stale-sweep `:1242`). `:1194` the per-page OG-card loop is the pattern to mirror; `:1242` a sidecar NOT written via `emit`/`seo_written` must be added to `keep` or the same build deletes it.
- `crates/core/src/site/meta.rs:133` — `jsonld_head`; the `page.date.is_some()` BlogPosting branch (`:144-164`) is where the ScholarlyArticle/Dataset upgrade goes. `:209` inline `jsonld_tests` (template `post_emits_blogposting` `:214`) for the failing unit test.
- `crates/core/src/site/meta.rs:84` — the existing scholarly `citation_*` (Highwire) block, gated on `page.date.is_some() && !page.authors.is_empty()`. **CAUTION:** NO corpus tech-blog post sets `author:`, so this fires on ZERO corpus posts today.
- `crates/core/src/site/llms.rs:169` — `page_prose(&self, page)` (the existing per-page clean-text primitive) to reuse for (A) until #2 lands.
- `crates/core/src/cite/render.rs:67` — `process(blocks, bib, xrefs)` builds `order: Vec<String>` = cited keys in citation order (exactly the per-page export input) but returns only `Vec<Warning>`; surface `order` additively for (C) — a signature change, so its one call site (`render/mod.rs:664`) changes too. `crates/core/src/render/model.rs:213` — `xref_numbers` is the precedent for adding a `cited_keys: Vec<String>` field the build reads.
- `crates/core/src/cite/mod.rs:46` — add a read-only `to_bibtex(&[keys])`/`to_csl_json(&[keys])` serializer here (additive; touches the cite Do-NOT-touch machine — flag in review).
- `crates/core/tests/tech_blog.rs:687` — THE integration pin: the SEO-assertion block (`:687-736`, JSON-LD at `:724-736`) over `corpus/tech-blog` (the only `url:`-bearing corpus site).

**Pin.** Extend `tech_blog.rs:687-736` to assert (B) `"@type":"ScholarlyArticle"` on
`posts/em-algorithm/index.tmd` and (C) a per-page cited-refs sidecar contains its cited keys. Plus an
inline `meta.rs jsonld_tests` unit test (mirror `post_emits_blogposting`).

**Corpus fact (verified).** `posts/em-algorithm` IS dated (2026-04-14), sets `bibliography: references.bib`,
and cites `[@key]` — but has **NO `author:`** (no tech-blog post does; `page.authors` has no site-config
fallback). So the (B) pin is only achievable once the trigger ruling lands.

**First step.** Failing unit test in `meta.rs jsonld_tests` (mirror `post_emits_blogposting`, uses
`write_site`) asserting a scholarly post emits `"@type":"ScholarlyArticle"`, then implement inside
`jsonld_head`'s `page.date.is_some()` branch. Sub-part B is cheapest — do it first, then C (surface `order`
through process + RenderedDoc + a serializer), then A (`page_prose` sidecar). **Decide the trigger BEFORE
writing the tech_blog.rs pin.**

**[ruling needed] (BLOCKING for the B pin).** (i) What TRIGGERS ScholarlyArticle vs BlogPosting? Mirroring
the `meta.rs:84` gate (dated + has authors) → NO corpus post qualifies, so the pin needs EITHER an
author-free trigger (e.g. "dated + `bibliography:` present", which em-algorithm satisfies) OR adding an
`author:` to em-algorithm (which drifts `body_html_snapshots.rs`) OR an explicit `type: article`/`schema:`
key. Pick before writing the pin. (ii) Sidecar format/extension + default-on vs opt-in (must avoid the
source `references.bib` name). (iii) Is `Dataset` in scope at all (no dataset-shaped corpus pages — likely
YAGNI until a pin doc justifies it)? (iv) Build-only or also serve in preview.

---

## Provenance

Grounded 2026-07-12 from FEATURE-IDEAS.md Session 2 via a 20-agent workflow (per-idea research +
adversarial anchor verification). Full raw entries (all anchors, gotchas, verifier notes) were in the
run journal; the load-bearing anchors + pins + first steps + rulings are preserved here. Nothing is
committed until it lands pinned. The queue lines live in `backlog.md` §G (Tier 1) + Tier 2/3.
