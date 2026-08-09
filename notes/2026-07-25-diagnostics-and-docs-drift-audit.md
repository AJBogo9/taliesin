# Two non-AP lenses: diagnostics-message quality, and docs-vs-behaviour drift (2026-07-25)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspectives:** the first two of the three lenses proposed on 2026-07-25 that were never
AP-shaped. Run together as the fifth audit of the day. Against `92fc67b`, release binary.
**Nothing was changed**: findings only.

## Headline

**The diagnostic catalogue has a silent, severity-escalating hole, and it is the second time the
same hole has bitten.** Six diagnostics that the tool emits today fall through `classify` to the
uncatalogued `TAL-CHECK` code at **ERROR** severity, purely because nobody added a needle for their
wording. Their catalogued siblings are WARNINGs. Nothing tests for this.

The docs lens, by contrast, came back nearly clean: **2 of 11 user-facing env knobs are
undocumented**, and the naive set-diff that found them also produced seven false positives that
all dissolved on inspection.

---

# Lens 1: diagnostics-message quality

## DIAG-1 (medium): six live diagnostics fall through to the uncatalogued code, at ERROR

`classify` (`diagnostics/codes.rs:141`) is a linear scan of `TABLE`, a list of
`(needle, code, severity)` triples matched as **substrings of the human message**. Anything that
matches no needle returns `(GENERIC, ERROR)`, i.e. `TAL-CHECK` at the harshest severity.

**Measured**, by running `check --format json --strict` over every corpus fixture, every corpus book
and both dogfood books (23 projects/docs, 51 diagnostics, 30 distinct code/severity pairs), then
over a purpose-built fixture for the families the corpus does not trip:

| Message | Classified as | Should be near |
|---|---|---|
| ``broken citation: @bishop2006patern (did you mean `@bishop2006pattern`?)`` | `TAL-CHECK` / **error** | `TAL-CITE-BIB` (warning) |
| ``unknown div class `fragmnet` (did you mean `fragment`?)`` | `TAL-CHECK` / **error** | `TAL-CALLOUT-KIND` (warning) |
| ``.scrolly` has no `.step` divs to scroll through`` | `TAL-CHECK` / **error** | its own code |
| ``.panel-tabset` has no headings, so it renders no tabs`` | `TAL-CHECK` / **error** | its own code |
| ``.input` needs a `name=` to feed the reactive graph`` | `TAL-CHECK` / **error** | `TAL-INPUT-TYPE` (warning) |
| ``.input type=select` needs `options="a,b,c"`` | `TAL-CHECK` / **error** | `TAL-INPUT-TYPE` (warning) |

For contrast, in the same run ``unknown callout kind `nonsense` `` classifies correctly as
`TAL-CALLOUT-KIND` / warning, because someone wrote the needle `"unknown callout kind"`. The
adjacent message `"unknown div class …"` has no needle and is therefore an **error**.

**Three consequences, all real:**

1. **Severity is decided by whether someone remembered a needle.** Two sibling authoring typos gate
   differently: an unknown *callout* kind is a warning, an unknown *div class* is an error that
   fails `check`, `build --strict` and `publish`.
2. **`check --explain` is useless for exactly these.** All six resolve to the `TAL-CHECK`
   explanation, whose title is literally *"an uncatalogued diagnostic"*, and whose `docs_url`
   anchors at the generic section of `docs/DIAGNOSTICS.md`. Five of the six carry a
   ``did you mean `X`?`` hint, i.e. they are the *most* fixable diagnostics the tool has.
3. **It reaches the machine-facing surface**, since `--format json` emits the same `code` and
   `docs_url`.

**This already happened once.** The backlog records that the opt-in `prose-lint:` rules were
classified `TAL-CHECK`/ERROR by this same fallback until 2026-07-25, so
``weasel word `simply` (consider cutting)`` failed `check`, `build --strict` and `publish`: a green
gate cost you the rule. That was fixed by adding three needles. **The failure mode was never fixed,
only its instance**, and six more are live now.

**Why nothing caught it.** The only test touching the fallback,
`uncatalogued_message_gets_a_stable_generic_code` (`codes.rs:709`), asserts that the synthetic
string `"something entirely new"` classifies to `(GENERIC, ERROR)`. It pins that the fallback
*works*; nothing pins that no real message *reaches* it. The `Explanation`-per-code completeness
test has the same shape: it guards `TABLE` against the docs, not the emitted messages against
`TABLE`.

**Fix shape.** A test that renders the diagnostics fixtures (plus one new fixture for the four
widget-validation families) and asserts **zero** diagnostics classify to `GENERIC` is the guard that
makes this class extinct rather than fixing six instances. Adding the needles without that test just
resets the clock. Note the ordering constraint the file already documents: shape rows must precede
prose rows in `TABLE`, because every shape message quotes the author's own heading back.

---

# Lens 2: docs-vs-behaviour drift

## DOCS-1 (low): two user-facing env knobs are undocumented

The user guide (`docs/guide/**/*.tmd`) documents nine environment variables: `TALIESIN_CELL_TIMEOUT`,
`TALIESIN_HOST`, `TALIESIN_MERMAID_URL`, `TALIESIN_NO_CACHE`, `TALIESIN_NO_CLEAR`, `TALIESIN_NO_EXEC`,
`TALIESIN_OPEN`, `TALIESIN_PYTHON`, `TALIESIN_R`. Two more are real user-facing knobs and appear
nowhere in it:

- **`TALIESIN_RENDER_TIMEOUT`** (`render/mod.rs:295`): the render watchdog budget, `0` disables.
  Shipped with AP2's hardening.
- **`TALIESIN_JS_TIMEOUT`** (`headless_js.rs:211`): the per-page settle budget for headless `{js}`
  observation. Shipped with DX17b.

Both were added by feature work that never touched the reference page, which is the same drift shape
the CLI-flag gate was built to catch (that gate found 9 undocumented flags where an audit had filed
2). **Fix shape:** document the two, and consider extending the existing mechanical gate to env vars
so the next one cannot ship undocumented.

## Refuted: seven other apparent drifts

A naive two-set diff (`TALIESIN_*` in `crates/` vs in `docs/`) reported nine discrepancies. Seven
dissolved on inspection and are recorded so they are not re-found:

- `TALIESIN_BOOT`, `TALIESIN_SSR_GEN`, `TALIESIN_SEARCH_LOAD_FAILED`, `TALIESIN_LABELS` are **not
  environment variables at all**: they are `window.*` JS globals. The regex conflated the two
  namespaces.
- `TALIESIN_COLD_REAP_CHILD` is an internal child-process marker (`kernel.rs`, a `CHILD_ENV` const),
  not a user knob.
- `TALIESIN_BIN` is real but belongs to the test harnesses (`tools/ui-audit`, the VS Code e2e
  suite), not to the binary's user surface.
- `TALIESIN_EDITOR_URI` exists only inside a historical design-decisions catalog under
  `docs/superpowers/specs/`, which is not user documentation.

**Method note worth keeping:** a name-set diff across two directories is a *lead generator*, not a
finding. Seven of nine leads here were noise, and each needed one `grep -rl` to kill.

## Not chased

- **Behavioural claims** in the two dogfood books (what a flag *does*, not whether it exists). That
  is the expensive half of this lens and it is untouched.
- **`docs/DIAGNOSTICS.md` freshness** against the `Explanation` set (there is a generator and a
  completeness test, so it is likely fine, but it was not verified here).
- **AP1's unchased residuals** (kernel RSS drift, multi-hour warm RSS), the third proposed lens, was
  not run at all.

## Method

`check --format json --strict` over 23 corpus/dogfood targets, parsed and tallied by code and
severity; a purpose-built fixture to trip the widget-validation families the corpus never exercises;
a static scan of `Warning::new`/`format!` message literals across `crates/` simulated against the
real `TABLE` needles to find latent fall-throughs, with every candidate then **verified by actually
tripping it** rather than reported from the scan (the scan alone over-counted badly, because it
could not distinguish `check` diagnostics from `build.rs` console logs). Env-var comparison by
directory-scoped grep, with every discrepancy resolved by locating the symbol.
