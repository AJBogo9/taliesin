# Extension system: a "Beyond Quarto" redesign

Status: design / brainstorm (audit + direction). No code in this pass.
Date: 2026-06-27
Author: Andreas Bogossian (with a research + audit workflow: 8 ecosystem studies, 7 surface audits, adversarial verification, synthesis + critic)
Scope decisions (settled with the owner): include Direction B now (experimental tier); distribution stays vendoring-only (no registry); this deliverable is the spec, not an implementation plan.

---

## 1. Goal and the reframe

The owner's goal, verbatim in intent: the extension system should let an individual user extend qmd-fast so they are **not bottlenecked by the maintainer's decisions or velocity**. Apply the "Beyond Quarto" mentality to extensibility itself, make it as good, easy, and powerful as possible, and learn from the mistakes of other ecosystems.

The load-bearing insight from the audit: **that goal is already structurally true.** Extensions are vendored files under `_extensions/<name>/`, resolved by walking up the directory tree from the page (`find_extension_dir`, `extension/mod.rs:74`). A user drops their own `_extensions/<name>/` into their own repo and it works: no registry, no fetch, no pull request to the maintainer, no gate. The maintainer is already out of the loop.

So the job is **not** to build a marketplace. The job is to make that already-decentralized path:

1. **Never fail silently** (the project's "diagnostics not silence" doctrine, applied to extensions).
2. **Documented** (today there is zero extension-authoring documentation).
3. **More powerful** through the browser runtime (the genuine Beyond-Quarto capability), without a code-execution escape hatch.

A bonus that falls out for free: keeping zero distribution surface keeps qmd-fast immune to the entire supply-chain incident class the research surfaced (the GlassWorm self-propagating worm of Oct 2025, the 100+ leaked Marketplace publisher tokens Wiz found, malicious silent auto-updates, name-reuse attacks). None of those can exist without a registry plus auto-update plus publish credentials, none of which qmd-fast has. **Vendoring-only is therefore promoted to a documented invariant**, the same way single-editing-surface is.

---

## 2. Invariants this design must respect

Every proposal below is checked against these. Nothing here may weaken them.

1. **Single editing surface.** The `.qmd` file is the only editing surface; the preview is read-only and never writes back. No extension mechanism may become a second write path or a preview gesture (the removed drag-to-reorder lesson).
2. **Click-to-source + block model.** Every emitted block carries `data-block-id` (content hash) + `data-sourcepos`. The incremental block-swap, reverse cursor sync, and live-state preservation key off this. Any host-emitted-on-behalf-of-an-extension HTML must carry these.
3. **Offline-first.** Everything bundled, no runtime CDN. An extension must not silently introduce a network dependency.
4. **HTML-only output.** No new output formats. Power means richer browser behavior in a live HTML view.
5. **Native deck contract.** Slides run on `window.QmdDeck` + `.qmd-deck`/`.qmd-slide`. There is no `window.Reveal`. This is the only deck promise.
6. **Do-NOT-touch discipline.** `cite.rs`, the exec/kernel path, and the `:::` fenced-div machine are fragile. Design around them.

---

## 3. Audit: verified findings

Each finding below was produced by a surface audit against the real binary, then adversarially re-checked by a second agent. Mechanism descriptions are corrected to the actual code (the critic caught two synthesis errors, noted inline).

| # | Finding | Evidence | Severity | Lens |
|---|---------|----------|----------|------|
| 1 | **`resources:` copies a file but never injects it.** A listed `.js`/`.css` is copied next to the output; nothing emits a `<script>`/`<link>`. The `liquid-glass` deck theme lists `liquid-glass.js` in `resources:` and is therefore inert, with zero warnings. `tabset` works only because it *also* hand-writes `<script src="tabset.js">` in `body-end`. | `apply_contribution`, `extension/mod.rs:226-234` (resources pushed to copy set only); `tabset/_extension.yml:5-6` | critical | robustness, ergonomics |
| 2 | **The dead deck JS targets reveal.js.** `corpus/liquid-glass-slides/.../liquid-glass.js` calls `deck.on('slidechanged')`, `deck.getSlides()`, `document.querySelector('.reveal-viewport')`. The native engine emits `.qmd-deck`/`.qmd-slide` and exposes `window.QmdDeck`. It fails as a silent `null`/no-op (the `.reveal-viewport` query returns `null`), **not** a thrown ReferenceError (critic correction). | `liquid-glass.js:13,55,72`; `window.QmdDeck` facade at `deck.js:1563` | high | robustness |
| 3 | **No host-version gate.** `version:` is metadata, never compared (`NATIVE_MANIFEST_KEYS`, `extension/mod.rs:145`). An extension needing a newer host half-applies instead of skipping loudly. There is no semver parse/compare anywhere in `extension/mod.rs` (so the gate is net-new code, not "reuse"; critic correction). | `extension/mod.rs:145-158` | high | robustness |
| 4 | **Zero authoring docs.** No reference page for `_extension.yml`, no worked tutorial, no written deck contract. Newcomers reach for the wrong API because the right one is undocumented, even though the code *intends* it as public: `deck.js:5-6` ("the `window.QmdDeck` API ... that ... theme extensions bind to") and `mermaid.js:2` ("the public `window.qmdEnhancers` API, exactly how a third-party extension"). | no `extension`/`_extension.yml` page under `docs/guide`; the intent comments above | high | docs |
| 5 | **Silent no-ops cluster.** `extensions: tabset` (scalar where a sequence is required) yields empty and does nothing (`parse_extensions` requires `.as_sequence()`, `extension/mod.rs:309`); a missing `head:`/`body-end:` `file:` becomes a buried HTML comment, no Warning; `format:` sub-keys (e.g. `slide-number: true`) are discarded. The `extensions:` *shape* is never seen by `validate_manifest` (which only checks unknown top-level manifest keys), so a "warn on scalar-where-list" lives in a different code path (critic correction). | `extension/mod.rs:309`; `validate_manifest`, `extension/mod.rs:161-174` | med | robustness |
| 6 | **Shortcode-name collisions overwrite silently.** `shortcode_templates()` collects every active extension's shortcodes into one `HashMap` (`extension/mod.rs:324,350-355`); a later extension silently overwrites an earlier `youtube`. The shipped guide `embed` extension already defines `youtube`/`vimeo`. | `extension/mod.rs:324,350-355`; `embed/_extension.yml:4-6` | med | robustness |
| 7 | **No arg escaping in shortcode templates.** `render_shortcode` substitutes via raw `html.replace("{{1}}", v)` with no escaping (`extension/mod.rs:526-532`), so a template using an arg in attribute context (`href="{{url}}"`) can be broken or abused by the arg value. | `extension/mod.rs:526-532` | med | robustness |
| 8 | **`--qmd-*` / `--deck-*` token surface undocumented.** An extension can re-theme via `theme:` layers, but which custom properties are stable and overridable is not written down. | `render/theme.rs`, `assets/css/*` | med | docs, breadth |
| 9 | **`.scss` not compiled.** Only `.css` theme layers are inlined (`resolve_theme_layers`); porting a Quarto extension's `.scss` requires a manual compile. | `apply_contribution`, `extension/mod.rs:218-220` | low | ergonomics |
| 10 | **Quarto-shaped manifest is a silent near-no-op.** A `contributes: formats: revealjs:` manifest parses, every key is unknown, so it warns per key and contributes nothing. There is no migration breadcrumb pointing at the flat native shape. | `validate_manifest`, `extension/mod.rs:161-174`; old `liquid-glass/_extension.yml` (additional working dir) | low | ergonomics |

Mechanism note (critic correction to internal reasoning, not user-facing behavior): `PageIncludes::merge` (`model.rs:245`) is string concatenation of include fragments; "last writer wins" for a duplicate `--qmd-accent` is a property of CSS cascade order, not of `merge`. The design's published injection order is what actually decides precedence.

---

## 4. Principles (ranked, from the research)

These are the transferable lessons, each already mapped to qmd-fast. Full provenance in the workflow output.

1. **No silent failure.** Every declared manifest key produces an observable effect or a located, click-to-source diagnostic. "Declare an asset" is one action that both copies and wires it. (VS Code: the host reads a closed, schema-validated manifest; a malformed key is caught at load, not at runtime.)
2. **Declarative data + browser-only JS; never a build-host code-exec / install-script / Lua-filter hook.** The whole worm/RCE class needs install-time or build-time code execution that qmd-fast structurally lacks. The weak `{{1}}`/`{{key}}` templating and "JS runs only in the reader's browser" are *features* that bound the frozen surface a solo maintainer supports forever. When more power is wanted, add more **named declarative kinds**, never an arbitrary-code hook.
3. **Publish a small, explicitly-versioned contribution surface; treat everything else as not-a-contract.** The block model, exact DOM nesting, private CSS classes, and the `data-block-id` hash stay private. The public promise is exactly: the manifest keys, the shortcode vars, the `window.QmdDeck` methods + `.qmd-deck`/`.qmd-slide` DOM, the `--qmd-*`/`--deck-*` tokens, `window.qmdEnhancers.register`, and the documented `window.qmdJs` seams. A checked `qmd-fast:` version field guards it. (JetBrains/VS Code: an explicit experimental lane buys freedom to evolve.)
4. **Stay vendored + git-reviewed + content-hash-pinned; never grow a registry or auto-update channel.** This is qmd-fast's biggest structural security asset. Discovery, if ever wanted, is a curated index keyed by repo URL + pinned commit + dir hash, hosting nothing and fetching nothing at runtime.
5. **Make ordering, namespacing, and target-scoping declared and deterministic, not list-position accidents.** Publish one total injection order; warn on duplicate activation and on id/class/shortcode collisions; prefix extension-emitted ids/classes; add an `applies-to: [deck|article|site]` scope. Do not add fine-grained cross-extension `after: X` constraints yet (the Vite RFC #13174 lesson: only needed at ecosystem scale).
6. **Expose the narrow imperative power authors actually need as a documented, versioned bridge; make the safe path the ergonomic path.** The capable seams (`window.QmdDeck.registerPlugin`, `window.qmdEnhancers.register`, the `{js}` reactive graph) already work but are undocumented, so authors reach for the wrong API. Document them and add the two tiny seams below.
7. **A `--safe`/audit lens: capability is declared, then the host reports on it (declare-then-enforce, coarse not granular).** Injecting `<script>` becomes a declared capability, not a side effect. The trust unit is the extension dir, advisory-but-loud, not a per-feature prompt matrix (the VS Code Workspace-Trust lesson).

---

## 5. Design

The owner chose to include Direction B now. The design therefore covers three layers that ship as one coherent contract: **A** (harden the flat manifest, v1), **C** (write down + extend the runtime), **B** (a named-kinds manifest v2 with capabilities and a block hook, experimental, flat v1 parsing in parallel). The manifest forms a smooth ladder: a one-line theme works in v1; richer extensions opt into v2.

### 5.1 Direction A: harden the flat manifest (v1)

Additive, reuses the existing closed-key validator + located-Warning channel + the Draft-2020-12 schema CLI. Touches nothing in `cite.rs`/exec/kernel/the `:::` machine.

- **`scripts:` and `styles:` keys that copy AND auto-inject.** `scripts: [x.js]` copies `x.js` into the resource set *and* emits `<script src="x.js" defer>` at `body-end`; `styles: [x.css]` copies *and* emits `<link rel="stylesheet">` in `<head>`. This kills finding #1. `resources:` stays for passive files (fonts, images, data) that are referenced by other assets, and gains a warning when a copied `.js`/`.css` resource is **never referenced** by any injected tag (the dead-resource detector).
- **Asset-tree copying (critic gap).** `scripts:`/`styles:`/`theme:` assets that reference sibling files (a `.css` with `url(fonts/inter.woff2)`, a deck theme shipping `assets/bg.jpg`) must have their `fonts/`/`assets/` trees copied and path-rewritten on `build --out`. Without this a built deck silently breaks offline, the same dead-theme class this design is killing. (liquid-glass ships both `fonts/` and `assets/`.)
- **A checked `qmd-fast:` version gate.** `qmd-fast: ">=0.2"` is parsed and compared against the running version. An extension needing a newer host is **skipped entirely with a loud located diagnostic**, never half-applied. Default to a forward-compatible, open-ended upper bound (avoid the JetBrains `until-build` treadmill for a one-person project). See the one-directional-versioning caveat in §6.
- **Validator fills** (all on the existing Warning channel, all located/click-to-source where a source line exists):
  - scalar-where-sequence on `extensions:` and on list-valued manifest keys (finding #5), surfaced at the front-matter or manifest line.
  - missing `head:`/`body-end:`/`file:` target becomes a Warning, not a buried comment (finding #5).
  - a **reveal-vocabulary detector**: injected deck JS containing `Reveal.`, `.reveal-viewport`, `Reveal.initialize`, etc. emits a did-you-mean pointing at `window.QmdDeck`/`.qmd-deck` (finding #2). Static string scan, no JS execution.
  - **shortcode-name collision** across active extensions warns instead of silently overwriting (finding #6).
  - **duplicate activation** (the same extension named by both `format:` and `extensions:`) warns (double-injection).
  - a **remote-URL lint** (critic offline-first gap): a shortcode template or injected head containing an absolute `http(s)://` origin warns unless the extension declares the `remote-fetch` capability (§5.3). This stops a "self-contained" page from silently depending on the network (the shipped `embed` extension's `youtube`/`vimeo` iframes are the live example).
  - a **zero-width / Unicode-variation-selector scan** on templates and injected head (the GlassWorm stealth vector), warned as suspicious.
- **`applies-to: [deck | article | site]` scope.** A deck theme declares `applies-to: [deck]`; activating it on an article page is a cheap warned no-op rather than indiscriminate CSS/JS injection (finding via Principle 5). Default (absent) = all.
- **Argument escaping** for shortcode templates: arg values substituted into attribute/text context are HTML-escaped by default, with an explicit opt-out token for templates that intentionally inject markup (finding #7). This is **new behavior**; the `mediapack` fixture's escaping assertions start red (§7).
- **One published injection-phase order** (documented, deterministic): theme base CSS, then extension `theme:` layers, then `styles:`/`css:`, then `head:`, then `body-start:`, then `body-end:`/`scripts:`, then shortcode expansion. Document front-matter always wins last (qmd-fast already inlines theme ahead of the header for exactly this).

### 5.2 Direction C: write down and extend the runtime

The most powerful seams already exist and work. This layer is mostly documentation plus two tiny additive JS seams.

- **Document the stable public runtime contract** (a new `docs/guide/reference/extensions.qmd` plus an internals page), versioned as part of the `qmd-fast:` surface:
  - `window.QmdDeck`: `registerPlugin({ id, init(deck) })` (handles late registration, `deck.js:1303`), the event list (`ready`, `slidechanged`, `fragmentshown`, `fragmenthidden`), `getSlides()`, `getCurrentSlide()`, `on(evt, cb)`. The `.qmd-deck`/`.qmd-slides`/`.qmd-slide` DOM.
  - `window.qmdEnhancers.register(fn)`: `fn(root)` runs after every (re)mount; idempotent; the documented way a non-deck extension decorates rendered DOM. (`mermaid.js` and `tabset.js` are the worked references.)
  - The `--qmd-*` and `--deck-*` token sets that are stable and overridable (finding #8).
- **Add two tiny, additive reactive-graph seams** (`window.qmdJs`):
  - `qmdJs.define(name, value)`: push a named value into the `{js}` reactive graph and reschedule **only downstream consumers** via `scheduleFrom`. This is **net-new** and must be scoped to `scheduleFrom`; it must **not** reuse the existing `bindDefines` path, which calls `runSequentially(r.cells)` (a full document re-run, `qmd-js.js:57-79`). The critic flagged that today's only `define` ingest is a full rebuild; the new seam is precisely the selective alternative.
  - `qmdJs.registerInput(name, el)`: a public wrapper over the existing internal `registerInput(r, name, el)` (`qmd-js.js:41`) so an extension can bind a custom control into the graph without poking `window.__qmdjs`.
  - Net capability: an extension can feed **live external data** (geolocation, a websocket/SSE feed, a custom control) into `{js}` cells. This is the genuine Beyond-Quarto, web-native power, with no server-side code-exec.

### 5.3 Direction B: named-kinds manifest v2 + capabilities + a block hook (experimental)

Shipped behind an experimental marker; **flat v1 keeps parsing in parallel** (anti-Manifest-V3; no hard break ever). The loader detects a `contributes:` and/or `capabilities:` block and takes the v2 path; otherwise v1.

**Manifest v2 shape (illustrative):**

```yaml
name: glasspane
qmd-fast: ">=0.2"
applies-to: [deck]
contributes:
  themes:    [[dark, glasspane.css]]   # base + layer
  styles:    [glasspane.css]           # copy + auto-inject <link>
  scripts:   [glasspane.js]            # copy + auto-inject <script>
  head:      [{ file: meta.html }]
  shortcodes:
    figurelink: '<a class="gp-figlink" href="{{url}}">{{1}}</a>'
  deck-plugins: [glasspane.js]         # declares a QmdDeck plugin (=> needs deck-hook)
  block-enhancers:                     # see the block hook below
    ".chart": chartEnhancer
capabilities: [inject-script, deck-hook]
```

- **Named kinds.** Each contribution kind (`themes`, `styles`, `scripts`, `shortcodes`, `head`, `deck-plugins`, `block-enhancers`) gets its own sub-schema and its own validator + did-you-mean, mirroring VS Code's `contributes.themes` vs `contributes.grammars` separation. This makes the closed set first-class instead of a flat bag.
- **`capabilities:` block (declare-then-report, Principle 7).** Coarse, per-extension: `inject-head`, `inject-script`, `deck-hook`, `js-graph`, `remote-fetch`. `check`/`build`/preview gain a `--safe`-style audit that **lists** which active extensions inject script vs are pure data, and warns on undeclared capabilities (a `scripts:` present but `inject-script` not declared) and on `remote-fetch` use. Advisory-but-loud, not a runtime sandbox (matches "green check means publishable"). The trust unit is the extension dir.
- **The block hook (the one piece that nears Do-NOT-touch).** Two variants; the design recommends variant 1 as the default experimental hook and gates variant 2 further.
  - **Variant 1 (client-side, recommended): `block-enhancers:` = a declared selector to a registered enhancer.** The host emits the block normally (so `data-block-id`/`data-sourcepos` are intact); the extension's registered JS (`qmdEnhancers.register`) decorates matching rendered blocks. This is the existing, proven enhancer pattern made **declarative and per-block-type**. Zero Rust-emission risk, zero `:::`-machine touch, block model preserved by construction.
  - **Variant 2 (server-side declarative template, further-gated): `block-templates:` keyed on fenced-code language.** A declarative template with a `{{body}}` slot wraps the rendered content of a fenced code block whose info-string matches (e.g. every ```` ```chart ````). The host emits through a helper that **stamps `data-block-id`/`data-sourcepos` on the container**; the template only shapes the inner HTML. **Scoped to fenced-code languages only**, explicitly **never** the `:::` div machine (Do-NOT-touch). This gives Hugo-style render-hook power (custom code-fence languages, server-rendered, JS-free) the invariant-safe way. It is the most powerful and the riskiest, so it ships last within B and only if variant 1 proves insufficient.
- **Experimental marking.** v2 kinds are documented as `experimental:` (the VS Code proposed-API tier). The contract can evolve; the flat v1 surface is the only "stable forever" promise.

### 5.4 Distribution: vendoring-only (a documented invariant)

No registry, no install command, no auto-update. Extensions are vendored files resolved by walk-up. This is documented as an invariant (the security rationale in §1). The "user not bottlenecked by the maintainer" goal is met by vendoring itself: a user adds their own `_extensions/<name>/` and it works. A future curated **index** (an Obsidian/Neovim-style list keyed by repo URL + pinned commit + dir content-hash, re-consenting on hash change, hosting nothing) is noted as a possible later affordance and is **out of scope for this design**.

---

## 6. Invariant-safety analysis (addressing the critic)

- **Offline-first vs declarative remote URLs.** A purely declarative shortcode can bake a `youtube.com` iframe into a "self-contained" page (the shipped `embed` extension does this). Addressed by the **remote-URL lint** (§5.1) gated on the `remote-fetch` capability (§5.3): a network dependency becomes declared and warned, never silent.
- **Offline-first vs asset trees.** Addressed by **asset-tree copy + path-rewrite** on `build --out` (§5.1). A fixture asserts a built deck theme references only local assets.
- **Click-to-source / block model vs injected HTML.** Extension-injected `head`/`body`/`script` chunks live outside the block flow and carry no `data-block-id`/`data-sourcepos` by design (they are page chrome, like the existing theme/script injection). The new risk is **variant 2** of the block hook: it is fenced to emit through a host helper that stamps the attributes, and a fixture asserts `reverse_sync_sourcepos_is_total` still holds with a block-template active. Variant 1 sidesteps this entirely (host emits normally; the extension only decorates post-render DOM).
- **Single-editing-surface vs the block hook.** Stated explicitly: a block hook must **never** round-trip, accept authoring state, or become a source-mutating / preview-gesture path. It is read-only emission/decoration only. (The drag-to-reorder lesson.)
- **Do-NOT-touch vs `qmdJs.define`.** The new seam is scoped to `scheduleFrom` (selective downstream recompute), **not** the existing `bindDefines`/`runSequentially` full-rebuild. It is browser JS in `qmd-js.js`, distinct from the Rust exec/kernel path. A `setInterval`-driven `define` therefore reschedules only consumers, not the whole doc; the fixture asserts this.
- **Native-deck contract vs `applies-to`.** `applies-to: [deck]` introduces a deck-vs-article decision in the extension layer. Risk is mis-scoping leaking deck CSS onto articles; mitigated by making the absent default = all, and by the deck engine remaining the sole authority on what `.qmd-deck` means.
- **Versioning is one-directional (critic gap, acknowledged).** The `qmd-fast:` gate guards "extension needs a newer host." The harder rot mode is the host moving forward and an old extension mis-behaving against a changed facade. Mitigation, recorded as a standing rule rather than built now: the public facades get a **deprecation channel** (a console + `check` warning when a removed/renamed facade is referenced, with an alias where cheap), so a frozen-forever surface still has an escape valve. This is the deprecation mechanism the synthesis omitted.

---

## 7. Example extensions (regression fixtures)

Three fixtures, each pinning a different contribution channel, so a future renderer/deck refactor fails the maintainer's CI rather than a user's render. Assertions that pin **current** behavior are green from day one; assertions that pin **new** behavior (auto-inject, escaping, selective recompute) start red and drive the TDD. They are labeled.

### E1. `glasspane-deck` (replaces the dead liquid-glass)

- Channel: deck theme on `window.QmdDeck` + `--deck-*` tokens + `scripts:` auto-inject.
- Manifest: `{ qmd-fast: ">=0.2", applies-to: [deck], themes: [[dark, glasspane.css]], scripts: [glasspane.js], capabilities: [inject-script, deck-hook] }`.
- `glasspane.js`: `QmdDeck.registerPlugin({ id: 'glasspane', init(deck) { var paint = () => deck.getCurrentSlide().classList.add('glasspane-active'); deck.on('slidechanged', paint); paint(); } })`.
- `glasspane.css`: overrides `.qmd-deck { --deck-accent: #8fd; }` and styles `.glasspane-active`.
- Test assertions:
  - (new) built HTML contains the injected `<script src="glasspane.js">` (proves `scripts:` auto-inject).
  - (green) built HTML contains `QmdDeck.registerPlugin` and the `--deck-accent` override; the `.qmd-deck` root is present.
  - (green) built HTML contains **no** `Reveal.` / `.reveal-viewport` token (the reveal-vocabulary guard).
  - (optional headless, mirrors `relay-harness`) after a programmatic `deck.next()`, `window.QmdDeck.getCurrentSlide()` has `.glasspane-active`.

### E2. `mediapack-shortcodes`

- Channel: declarative `{{< >}}` template pack.
- Manifest: `{ qmd-fast: ">=0.2", shortcodes: { figurelink: '<a class="ml-figlink" href="{{url}}">{{1}}</a>', callout-note: '<aside class="ml-note">{{1}}</aside>' } }`.
- Corpus doc uses `{{< figurelink url="/x" Label >}}`, one instance inside a fenced block (must stay literal), and one `{{< figurelink Label >}}` with `url` omitted.
- Test assertions:
  - (green) the rendered `<a href="/x">` appears with `data-sourcepos` on its block.
  - (green) the fenced instance is escaped literal text, not an `<a>`.
  - (new) a located Warning fires for the missing `url` slot (no raw `{{url}}` leaks).
  - (new) an arg value like `url="javascript:&quot;..."` is attribute-escaped (pins §5.1 escaping).
  - (new) a second extension also declaring `figurelink` triggers the collision warning.

### E3. `livefeed-enhancer`

- Channel: CSS+JS enhancer via `window.qmdEnhancers.register` + the new `window.qmdJs.define` bridge.
- Manifest: `{ qmd-fast: ">=0.2", applies-to: [article, site], styles: [livefeed.css], scripts: [livefeed.js], capabilities: [inject-script, js-graph] }`.
- `livefeed.js`: `window.qmdEnhancers.register(root => root.querySelectorAll('.livefeed').forEach(decorate))` and `window.qmdJs.define('feedTick', 0)` then a `setInterval` calling `window.qmdJs.define('feedTick', ++n)`.
- Corpus doc has a `{js}` cell `//| input: feedTick` rendering the value.
- Test assertions:
  - (new) built HTML auto-injects `<script src="livefeed.js">` and inlines `livefeed.css`.
  - (headless) `qmdEnhancers` ran (a `.livefeed-decorated` class appears).
  - (new, headless) `window.qmdJs.define('feedTick', 1)` updates **only** the consuming cell's container (snapshot the other cells' `data-block-id` unchanged), proving the selective `scheduleFrom` path and that the new seam exists.

---

## 8. Corpus pinning and test plan

Each capability ships pinned by a target corpus doc in the same change (corpus-plus-roadmap discipline):

- New `corpus/extensions/` (or extend the existing example dirs) holding `glasspane-slides`, `mediapack`, `livefeed`.
- Rust tests (extending `crates/core/tests/` and `render/tests.rs`): the green assertions above plus, critically, a `reverse_sync_sourcepos_is_total` assertion with a block-template (variant 2) active.
- The reveal-vocabulary guard gets a unit test (a fixture JS with `Reveal.` produces the did-you-mean Warning).
- The `--safe` capability audit gets a test (an extension shipping `scripts:` without `inject-script` warns).
- Headless browser checks reuse the existing chrome-devtools MCP + `relay-harness` patterns for E1/E3.
- The dead liquid-glass corpus extension is replaced by `glasspane-deck` as the canonical "right way" (closes the backlog "liquid-glass is dead" item).

---

## 9. Settled decisions and open questions

Settled:
- Include Direction B now, experimental tier, flat v1 parsing in parallel forever (no Manifest-V3 hard break).
- Distribution is vendoring-only; vendoring-only is a documented invariant. No registry. A curated index is a possible later affordance, out of scope here.
- The block hook defaults to variant 1 (client-side, declarative `block-enhancers:`); variant 2 (server-side `block-templates:` on fenced-code languages, host-stamped) ships last within B and only if variant 1 proves insufficient. Neither touches the `:::` div machine.
- `qmd-fast:` version gate is forward-compatible / open-ended upper bound (no `until-build` treadmill).

Open (to resolve in the implementation plan, not now):
- Exact `qmd-fast:` semver-range grammar (a tiny parser vs a minimal `>=x.y` comparison).
- Whether `scripts:` defaults to `defer` or `module` and the precise body-end ordering vs document `{js}` cells.
- The deprecation-channel surface (console only, or also a `check` rule) for evolving a public facade.
- Whether the `--safe` audit is a flag on `check` or always-on advisory output.

---

## 10. Why this is "Beyond Quarto"

Quarto's extension system is build-host-coupled (Lua/Python filters run at compile time, a known RCE and supply-chain surface) and its power comes from arbitrary code. qmd-fast goes the other way: **declarative data + browser-only JS against a small documented runtime**, vendored with zero distribution surface. That is structurally safer *and*, via the `window.QmdDeck` plugin API and the new `qmdJs.define` bridge, lets an author ship genuinely web-native, live, interactive extensions (a custom deck theme that reacts to slide changes; a control that streams live data into a reactive `{js}` chart) without ever running code on anyone's build machine. Power and safety stop being a trade-off, which is the whole point of doing it natively.
