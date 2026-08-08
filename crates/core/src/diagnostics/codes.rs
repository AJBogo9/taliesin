//! Stable diagnostic codes + severities for agent-matchable `check --format json` output.
//!
//! An agent (or CI rule) wants to match a diagnostic on a stable identifier and read a
//! machine-usable fix, not scrape prose. This catalog maps each diagnostic family to a
//! stable `TAL-<FAMILY>` code and a severity, and lifts the inline "did you mean `X`?"
//! hint into a structured replacement. It is derived from the diagnostic *message* at the
//! `check` boundary (not threaded through every validator), so `--format human` stays
//! byte-identical and no validator call site changes. The trade-off is that the mapping is
//! keyed on stable message prefixes; `check_cli.rs` pins representative codes so a wording
//! change that would silently reclassify a family fails a test.

/// Severity strings (an agent triages on these). They also decide the **exit** gate:
/// `check` fails on an error or a warning by default, `--errors-only` narrows that to
/// errors, and `--strict` widens it to everything. See [`gates_at`].
pub const ERROR: &str = "error";
pub const WARNING: &str = "warning";
/// Advice, not a defect: printed like any other diagnostic but **never** fails a gate
/// unless the run asks for it (`check --strict`). This is what lets an opt-in style rule
/// ("weasel word `simply` (consider cutting)") exist at all — as an error it made
/// `check` and `build --strict` fail on a suggestion to
/// reword a sentence, so the only way to keep a green gate was to not turn the rule on.
pub const SUGGESTION: &str = "suggestion";

/// How severe a diagnostic is, as a number: **higher is more severe**. The one ordering
/// both crates use, so a gate cannot disagree with a summary about what outranks what.
/// An unknown severity string ranks with [`ERROR`], because a diagnostic nobody
/// classified is not something to silently stop failing on.
pub fn severity_rank(severity: &str) -> u8 {
    match severity {
        SUGGESTION => 0,
        WARNING => 1,
        _ => 2,
    }
}

/// Whether a diagnostic of `severity` fails a run whose floor is `floor_severity`.
/// Both are severity strings, compared by [`severity_rank`].
pub fn gates_at(severity: &str, floor_severity: &str) -> bool {
    severity_rank(severity) >= severity_rank(floor_severity)
}

/// The fallback code for a diagnostic whose family isn't catalogued yet. Non-empty and
/// stable, so `.diagnostics[].code` is always a usable string.
pub const GENERIC: &str = "TAL-CHECK";

/// `(message-substring, code, severity)`, matched in order (first hit wins), so a more
/// specific needle must precede a more general one (`broken link anchor` before
/// `broken link`).
const TABLE: &[(&str, &str, &str)] = &[
    // Document-shape lints — structural advice, so SUGGESTION, and FIRST in the table for
    // the same reason the prose rules are: every one of these messages quotes the author's
    // own heading or caption text back, so a section literally titled "Math" or
    // "Bibliography" would otherwise classify as TAL-MATH / TAL-CITE-BIB. Pinned by
    // `a_shape_diagnostic_outranks_a_needle_inside_the_authors_own_heading`.
    ("empty heading", "TAL-SHAPE-EMPTY", SUGGESTION),
    ("duplicate heading text", "TAL-SHAPE-DUP", SUGGESTION),
    ("repeats the page title", "TAL-SHAPE-ECHO", SUGGESTION),
    ("has no content under it", "TAL-SHAPE-HOLLOW", SUGGESTION),
    ("caption is only its label", "TAL-SHAPE-CAPTION", SUGGESTION),
    // Link-text advice, up here for the same reason: the message quotes the author's own
    // link text, so a link reading "math" or "bibliography" would classify as TAL-MATH /
    // TAL-CITE-BIB. Also ahead of `broken link`, whose needle it contains as a substring
    // ("ambiguous link text" does not, but a link whose TEXT is "broken link" would).
    ("ambiguous link text", "TAL-LINK-TEXT", SUGGESTION),
    // Opt-in prose lint (`prose-lint:`) — style advice, so SUGGESTION, and ahead of every
    // catalogued family on purpose: the needles below include ones as generic as
    // `("math", …)`, and a weasel-word message naming a word that contains a generic needle
    // ("weasel word `mathematically`") would otherwise be classified as a math diagnostic.
    ("weasel word", "TAL-PROSE-WEASEL", SUGGESTION),
    ("repeated word", "TAL-PROSE-REPEAT", SUGGESTION),
    ("banned term", "TAL-PROSE-BANNED", SUGGESTION),
    // Front matter.
    ("not valid YAML", "TAL-FM-YAML", ERROR),
    ("valid YAML", "TAL-FM-YAML", ERROR),
    ("unknown front-matter key", "TAL-FM-KEY", WARNING),
    ("unknown execute key", "TAL-FM-KEY", WARNING),
    ("unknown listing key", "TAL-FM-KEY", WARNING),
    ("unknown hero key", "TAL-FM-KEY", WARNING),
    ("unknown theorems key", "TAL-FM-KEY", WARNING),
    ("unknown prose-lint key", "TAL-FM-KEY", WARNING),
    // A recognized key taliesin reads and then ignores (`csl:`). Must precede the
    // citation needles below: the message names `bibliography`-adjacent concepts and
    // would otherwise classify as TAL-CITE-BIB.
    (
        "is recognized but not supported",
        "TAL-FM-UNSUPPORTED",
        WARNING,
    ),
    // Body constructs.
    // Every shortcode diagnostic — an unknown name, an unknown flag or `key=`, a built-in
    // with no source path. Keyed on the `{{<` opener rather than on the word "shortcode",
    // because each message quotes the author's own invocation and none of them share a
    // phrase. One family, not three: the surface is the same and so is the edit (fix the
    // spelling inside the braces). It is a WARNING for the reason the `.input` and div
    // families are — the page still renders, so a one-letter typo must not gate
    // `build --strict`, which is what the old `(GENERIC, ERROR)` fall-through did.
    // Above the generic needles below because the quoted invocation is author-controlled
    // text (`{{< video math-intro.mp4 >}}` would otherwise classify as TAL-MATH).
    ("{{<", "TAL-SHORTCODE", WARNING),
    ("unknown callout kind", "TAL-CALLOUT-KIND", WARNING),
    // A bad value inside `theorems: shared:`. Its own family rather than TAL-FM-KEY: the
    // key is fine and the fix edits a list ENTRY, not the key. Above the generic needles
    // because the message quotes the author's own kind name.
    ("unknown theorem kind", "TAL-THM-KIND", WARNING),
    // The `.callout-…` row's sibling: a near-miss of any other feature/theorem div class.
    // Separate family because the fix is a different edit (the class, not the callout kind),
    // and above the generic needles below because the message quotes the author's own class.
    ("unknown div class", "TAL-DIV-CLASS", WARNING),
    ("unknown cell option", "TAL-CELL-OPTION", WARNING),
    ("unknown input type", "TAL-INPUT-TYPE", WARNING),
    // A reactive control missing an attribute it cannot work without. Distinct from
    // TAL-INPUT-TYPE (an unknown type): the type is fine, the declaration is incomplete.
    ("needs a `name=` to feed", "TAL-INPUT-ATTR", WARNING),
    ("needs `options=", "TAL-INPUT-ATTR", WARNING),
    // A feature div that HAS content but is missing a part it cannot render without — the
    // partial sibling of TAL-EMPTY-DIV, one row per container that can be built half-formed.
    (
        "has no headings, so it renders no tabs",
        "TAL-DIV-PARTS",
        WARNING,
    ),
    ("has no code block to show", "TAL-DIV-PARTS", WARNING),
    ("has no sticky stage", "TAL-DIV-PARTS", WARNING),
    ("has no `.step` divs", "TAL-DIV-PARTS", WARNING),
    // A `.step lines=` spec carrying a `|` (the `code-line-numbers=` step separator),
    // which a step's own comma-only parser silently focuses to zero lines.
    ("step separator", "TAL-STEP-LINES", WARNING),
    // An empty div that names a real feature (`.input`, `.callout-*`, `.panel-tabset`, …),
    // which is dropped and renders nothing.
    ("no content between the", "TAL-EMPTY-DIV", WARNING),
    // A `.column width=` a reveal/Quarto author expects to honour, silently equalized.
    (
        "lays its columns out equal-width",
        "TAL-COLUMN-WIDTH",
        WARNING,
    ),
    // Cross-references + links + anchors.
    ("broken cross-reference", "TAL-XREF-UNDEF", ERROR),
    // A `[@key]` with a bibliography present but no matching entry. Filed with the other
    // reference families rather than with TAL-CITE-BIB because it is the same mistake as a
    // broken `@fig-` (fix the key), not the missing-bibliography one (add the file) — and
    // because it MUST outrank both `bibliography` and the link needles below: the message
    // embeds the author's own key, and its no-suggestion variant literally ends "(not in the
    // bibliography)", which is what previously split one family across two codes.
    ("broken citation", "TAL-CITE-KEY", WARNING),
    // A DEFINITION no `@ref` can reach: a hidden cell's `label:` (the executor drops the
    // output that would carry the anchor) or a theorem id with no kind prefix. Distinct
    // from TAL-XREF-UNDEF, which is the reference site's complaint. Both messages embed
    // the author's own label and `classify` is first-hit-wins over the whole string, so
    // this MUST stay above the generic `math`/`bibliography` needles below —
    // otherwise `fig-math-model` classifies as TAL-MATH and `tbl-bibliography-counts` as
    // TAL-CITE-BIB. Pinned by `an_unreferenceable_label_outranks_a_needle_in_its_own_label`.
    ("cannot be cross-referenced", "TAL-XREF-UNREF", WARNING),
    ("duplicate heading id", "TAL-DUP-ID", ERROR),
    ("broken in-page link", "TAL-ANCHOR", ERROR),
    ("broken link anchor", "TAL-LINK-ANCHOR", ERROR),
    ("broken link", "TAL-LINK", ERROR),
    // Assets + media.
    ("local asset not found", "TAL-ASSET", ERROR),
    ("local video not found", "TAL-MEDIA", ERROR),
    // Reactive `{js}` graph.
    ("unknown reactive input", "TAL-REACTIVE", ERROR),
    ("reactive dependency cycle", "TAL-REACTIVE", ERROR),
    // Accessibility.
    ("heading level skips", "TAL-A11Y-HEADING", WARNING),
    ("has no accessible name", "TAL-A11Y-NAME", WARNING),
    // 2.5.3 Label in Name. Distinct code from TAL-A11Y-NAME because the fix is the
    // opposite: that one says *add* a name, this one says the name you added contradicts
    // what the control reads. The message quotes the author's own label, so it sits with
    // the other quoting rows above the generic needles.
    ("disagrees with its visible text", "TAL-A11Y-LABEL", WARNING),
    ("missing alt text", "TAL-A11Y-ALT", WARNING),
    ("looks like a placeholder", "TAL-A11Y-ALT", WARNING),
    // Execution (`build` only — `check` never runs a cell). Both messages embed
    // the page label, so they sit above the generic needles like every other row that
    // quotes author-controlled text: a page called `math.tmd` would otherwise be TAL-MATH.
    // Two codes, not one, because the fix is in a different place: TAL-CELL-ERROR is the
    // author's code raising, TAL-KERNEL is the environment failing to run it at all.
    (
        "code cell raised an uncaught exception",
        "TAL-CELL-ERROR",
        ERROR,
    ),
    ("code cell did not run", "TAL-KERNEL", ERROR),
    ("code cell did not complete", "TAL-KERNEL", ERROR),
    // Citations, math, code.
    // Dead weight in a `.bib`: declared and never cited. SUGGESTION, not WARNING — the
    // page renders exactly right, so failing `check` on it would make a shared
    // bibliography (whose whole point is that most pages cite a few of it) unusable; it
    // still gates under `--strict`. MUST precede both the generic `bibliography` needle it
    // contains and the generic `math` one, because the message embeds the author's own
    // citation keys and a key like `mathworks2020` would otherwise classify as TAL-MATH.
    ("declared but never cited", "TAL-CITE-UNUSED", SUGGESTION),
    ("citations are present", "TAL-CITE-BIB", WARNING),
    ("bibliography", "TAL-CITE-BIB", WARNING),
    ("math", "TAL-MATH", WARNING),
    // Ahead of `unknown code language` only for clarity; the two needles cannot both match,
    // since the retirement arm returns before the generic one is reached. WARNING, not ERROR,
    // so it lands at exactly the severity of the generic unknown-language case it replaces.
    // Measured 2026-08-04 on a leftover `{pyodide}` cell: unclassified it fell through to
    // `(GENERIC, ERROR)`; classified, a plain `build` exits 0 and `--strict` exits 1, which is
    // byte-for-byte what a ```pyton typo already does. The point of the row is that parity,
    // not an exemption — a located WARNING fails `--strict` either way.
    ("is a retired cell language", "TAL-CELL-RETIRED", WARNING),
    ("unknown code language", "TAL-CODE-LANG", WARNING),
];

/// The `(code, severity)` for a diagnostic message: the first catalogued family whose
/// substring the message contains, or `(GENERIC, ERROR)` when none match.
pub fn classify(message: &str) -> (&'static str, &'static str) {
    for (needle, code, severity) in TABLE {
        if message.contains(needle) {
            return (code, severity);
        }
    }
    (GENERIC, ERROR)
}

/// The replacement from an inline "did you mean `X`?" hint, e.g. `treme` -> `theme`,
/// `@fig-reslts` -> `@fig-results`. `None` when the message carries no such hint.
pub fn extract_suggestion(message: &str) -> Option<String> {
    let key = "did you mean `";
    let at = message.find(key)? + key.len();
    let rest = &message[at..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

// ---- `check --explain <CODE>`: turn a code into cause + canonical fix -----------------

/// The canonical, offline home a `docs_url` anchors into: a committed catalog generated from
/// [`EXPLANATIONS`] (`docs/DIAGNOSTICS.md`), so a per-code `#tal-…` anchor resolves on GitHub
/// without the tool shipping a production docs domain. See [`docs_url`].
pub const DIAGNOSTICS_DOC_URL: &str =
    "https://github.com/AJBogo9/taliesin/blob/main/docs/DIAGNOSTICS.md";

/// The prose expansion of one diagnostic code: what it means, why it fired, and the one
/// canonical edit that clears it (rustc `--explain` style). One per code, kept next to the
/// [`TABLE`] it explains so the two can't drift (a completeness test enforces the pairing).
#[derive(Debug, Clone, Copy)]
pub struct Explanation {
    pub code: &'static str,
    pub title: &'static str,
    pub cause: &'static str,
    pub fix: &'static str,
}

/// One entry per distinct code in [`TABLE`] plus [`GENERIC`]. Grounded in the real
/// validators (the wording of each cause/fix tracks the shipped diagnostic message). A code
/// added to `TABLE` without a row here fails `every_code_has_a_nonempty_explanation`.
const EXPLANATIONS: &[Explanation] = &[
    Explanation {
        code: GENERIC,
        title: "an uncatalogued diagnostic",
        cause: "This diagnostic has not been assigned a specific TAL-* family yet, so it \
                carries the generic code. The message text itself is the guide to what \
                tripped.",
        fix: "Read the message and its location, then act on what it names. If one kind of \
              problem keeps surfacing this way, that family is a candidate for its own code.",
    },
    Explanation {
        code: "TAL-SHAPE-EMPTY",
        title: "a heading with no text",
        cause: "A heading opens a section but carries no words, so the table of contents, \
                the book outline and any cross-reference to it all render a blank row. \
                Usually a heading whose text was cut without cutting the `#` line.",
        fix: "Give the heading a name, or delete the line. This is advice, not a defect: it \
              is severity `suggestion`, so it never fails `check` or `build --strict` \
              unless you ask with `check --strict`.",
    },
    Explanation {
        code: "TAL-SHAPE-DUP",
        title: "two headings on one page read the same",
        cause: "Two headings on the same page have identical text, so the table of contents \
                shows two rows a reader cannot tell apart and neither one says which is \
                which. Distinct from TAL-DUP-ID, which is about the emitted anchor rather \
                than the words.",
        fix: "Make the second heading say what actually distinguishes it (`Model summary` \
              and `Model summary (pooled)`), or merge the two sections if they are one.",
    },
    Explanation {
        code: "TAL-SHAPE-ECHO",
        title: "a body heading repeats the page title",
        cause: "A heading below the first one restates the document's own `title:`, so it \
                adds a table-of-contents row that tells a reader nothing new. The page's \
                *leading* heading is deliberately exempt — opening a landing page with a \
                heading that matches its title is an ordinary idiom, not a defect.",
        fix: "Name the section for what that section covers, or drop the heading and let the \
              title carry it.",
    },
    Explanation {
        code: "TAL-SHAPE-HOLLOW",
        title: "a heading with nothing under it",
        cause: "This heading has neither text nor subsections beneath it, so the section is \
                empty on any reading: a table-of-contents row that leads nowhere. Any \
                content counts — a list, a code cell, a figure or a table, not just a \
                paragraph. A heading followed by DEEPER headings is an ordinary grouping \
                parent and is deliberately exempt: it does have content in the document \
                tree, so asking for an intro paragraph there would be a style opinion.",
        fix: "Write the section, or delete the heading. If it was meant to group other \
              sections, give it real subsections (deeper headings) rather than siblings.",
    },
    Explanation {
        code: "TAL-SHAPE-CAPTION",
        title: "a numbered figure whose caption is only its label",
        cause: "The figure is numbered and can be cross-referenced, but its caption is empty \
                or reads only `Figure 2:`. A caption is the most-read text on a page after \
                the heading, and a cross-reference to a figure that describes nothing makes \
                the reference unreadable too.",
        fix: "Write what the figure shows (`![Fatality rate by manufacturer, 1990-2020](f.png){#fig-rates}`). \
              If it genuinely needs no caption, drop the `{#fig-…}` id so it is not numbered.",
    },
    Explanation {
        code: "TAL-LINK-TEXT",
        title: "two links on one page read the same but go elsewhere",
        cause: "Two links on this page have the same accessible name and different \
                destinations, so neither one says where it goes. A screen reader can list \
                a page's links out of context, where the text is all the reader gets — and \
                a sighted reader scanning for the link they already followed cannot tell \
                the two apart either. Destinations are compared ignoring the `#fragment`, \
                so two deep links into one page are deliberately NOT flagged.",
        fix: "Make the link text name its own destination (`the execution model` rather \
              than a second `this chapter`). Do not paper over it with `aria-label`: a \
              label that disagrees with the visible text breaks voice control (WCAG 2.5.3, \
              Label in Name). This is advice, severity `suggestion`, so it never fails \
              `check` or `build --strict` unless you ask with `check --strict`.",
    },
    Explanation {
        code: "TAL-PROSE-WEASEL",
        title: "a hedging word the sentence does not need",
        cause: "The opt-in prose lint (`prose-lint:` in front matter) found one of a small \
                closed list of hedges — `very`, `simply`, `obviously`, `basically` and \
                friends. They read as emphasis but carry no information, and `obviously` \
                additionally tells a reader who did not find it obvious that they should \
                have.",
        fix: "Cut the word and read the sentence again; it almost always survives unchanged. \
              This is advice, not a defect: it is severity `suggestion`, so it never fails \
              `check` or `build --strict` unless you ask with `check --strict`.",
    },
    Explanation {
        code: "TAL-PROSE-REPEAT",
        title: "the same word twice in a row",
        cause: "The opt-in prose lint found a word immediately repeated (`the the`, `a a`). \
                Almost always an editing artefact left by a rewritten sentence, and one of \
                the few prose defects that is genuinely objective.",
        fix: "Delete the duplicate. If the repetition is deliberate (a quoted stutter, a \
              literal), the rule has no exception list — reword or turn the lint off for \
              that document.",
    },
    Explanation {
        code: "TAL-PROSE-BANNED",
        title: "a term this document's own banned list forbids",
        cause: "The document's `prose-lint: {banned: [...]}` list names this term, so the \
                lint flagged it. The list is yours; nothing is banned by default.",
        fix: "Use the wording you decided on instead, or drop the term from the `banned` \
              list if the ban no longer applies.",
    },
    Explanation {
        code: "TAL-FM-YAML",
        title: "the YAML front matter is malformed",
        cause: "The block between the opening and closing `---` is not valid YAML (an \
                unterminated quote, bad indentation, or a stray tab), so the strict parse \
                rejected it before any field could be read.",
        fix: "Fix the YAML at the reported line: close the quote, align the indentation, or \
              replace tabs with spaces. Every value after a parse error is lost, so the \
              parse must succeed first.",
    },
    Explanation {
        code: "TAL-FM-KEY",
        title: "an unknown key in front matter",
        cause: "A key in the document's front matter (or a nested `execute:`/`listing:`/\
                `hero:`/`theorems:`/`prose-lint:` block) is not in Taliesin's closed \
                vocabulary. It is a typo, or a key from another tool that Taliesin does not \
                implement, so it would be silently ignored.",
        fix: "Correct the key to the nearest valid name (`check --format json` carries a \
              `suggestion.replacement` for a near-miss), or remove it. The front-matter \
              reference lists every recognized key.",
    },
    Explanation {
        code: "TAL-FM-UNSUPPORTED",
        title: "a recognized but unsupported key",
        cause: "This key is one Taliesin knows about but deliberately does not act on (for \
                example `csl:`), so leaving it in implies an effect that never happens.",
        fix: "Remove the key. The behavior it configures in another tool is not part of \
              Taliesin's HTML output.",
    },
    Explanation {
        code: "TAL-CALLOUT-KIND",
        title: "an unknown callout kind",
        cause: "A `::: {.callout-…}` block names a callout type that is not one of Taliesin's \
                kinds (`note`, `tip`, `warning`), so it would render as a plain fenced div \
                with no callout styling. Two prior kinds, `important` and `caution`, were \
                retired on 2026-08-03: three kinds cover the distinctions a reader can \
                actually decode, so the message explains the removal rather than guessing \
                at a rename.",
        fix: "Change the kind to a supported one (a typo draws the nearest match; a retired \
              kind's message names what to use instead), e.g. `::: {.callout-warning}`.",
    },
    Explanation {
        code: "TAL-THM-KIND",
        title: "an unknown theorem kind in `theorems: shared:`",
        cause: "The `shared:` list names theorem kinds that should draw ONE counter, and an \
                entry here is not one of Taliesin's five (`theorem`, `lemma`, `corollary`, \
                `definition`, `proof`). An unrecognized kind is simply skipped, so the \
                counter you asked to share silently stays separate and the numbering is \
                wrong in a way nothing on the page announces. `proposition`, `example` and \
                `remark` were retired on 2026-08-03 with their div classes, so a list \
                carried over from before then names kinds that no longer exist.",
        fix: "Drop the entry, or replace it with the surviving kind the message names \
              (`theorem` covers a proposition — both render in the same `plain` style). A \
              typo draws the nearest match instead.",
    },
    Explanation {
        code: "TAL-DIV-CLASS",
        title: "a misspelled or retired feature div class",
        cause: "A `:::` fenced div carries a class that is a near-miss of one Taliesin \
                implements (`.fragmnet` for `.fragment`, `.theorm` for `.theorem`), so the \
                feature never dispatches and the div renders as a plain container. Div \
                classes are an OPEN vocabulary — a genuinely custom class you style yourself \
                is silent — so a near-miss fires only within edit distance 2 of a known \
                name. A class Taliesin used to implement and has since removed (`.columns`; \
                `.sidenote`/`.marginnote`/`.aside`, retired 2026-08-03 in favor of the single \
                `.column-margin` spelling; or `.proposition`/`.example`/`.remark`, retired \
                2026-08-03 along with their theorem kinds) fires unconditionally instead, \
                with a removal note rather than a guessed rename.",
        fix: "Correct the class to the one the message suggests, or — for a retired class — \
              to the replacement its removal note names. If the class really is your own, \
              rename it so it is not a near-miss of a built-in.",
    },
    Explanation {
        code: "TAL-DIV-PARTS",
        title: "a feature div is missing a part it needs",
        cause: "A `.panel-tabset`, `.code-walkthrough` or `.scrolly` has content but not \
                the part that makes it work: a tabset builds its tabs from `##` \
                headings, a walkthrough pins a code block in its sticky panel, and a scrolly \
                needs both a sticky stage (a figure or `{js}` cell) and `.step` divs to \
                scroll past it. The \
                container still renders, just half-formed: a tab strip with no tabs, an \
                empty sticky panel, a scroller that drives nothing. Distinct from \
                TAL-EMPTY-DIV, which is a feature div with no \
                content at all.",
        fix: "Add the missing part named in the message: `##` headings inside the tabset, a \
              fenced code block inside the walkthrough, or a stage and \
              `.step` blocks inside the scrolly.",
    },
    Explanation {
        code: "TAL-CELL-OPTION",
        title: "an unknown cell option",
        cause: "A `#|` / `//|` option line on a code cell uses a key Taliesin does not \
                recognize, so it has no effect on how the cell runs or renders.",
        fix: "Correct the option to a known one (the message suggests the nearest, e.g. \
              `labl` -> `label`), or remove it. See the cell-options reference.",
    },
    Explanation {
        code: "TAL-EMPTY-DIV",
        title: "an empty feature div renders nothing",
        cause: "A `:::` fenced div names a real feature (a `.input` reactive control, a \
                `.callout-…`, a `.panel-tabset`, a theorem, …) but has no content between its \
                fences, so it is dropped and renders nothing. The most common case is reaching \
                for `::: {.input name=\"k\"}` as a div — the reactive input control is a \
                shortcode, not a fenced div.",
        fix: "Put content between the `:::` fences (the callout body, the tabset's `##` \
              headings, the theorem statement), or, for a reactive input, use the shortcode \
              form `{{< input name=\"k\" … >}}` instead of a div.",
    },
    Explanation {
        code: "TAL-COLUMN-WIDTH",
        title: "a `.column width=` is ignored",
        cause: "A `.columns` grid lays its `.column` children out in EQUAL columns, so a \
                per-column `width=` (a reveal/Quarto habit, e.g. `::: {.column width=\"70%\"}`) \
                has no effect — the split is silently equalized.",
        fix: "Remove the `width=` (the columns are equal), or set an explicit column count with \
              `::: {.columns ncol=N}` or `::: {layout-ncol=N}`. Variable-width columns are not \
              supported.",
    },
    Explanation {
        code: "TAL-STEP-LINES",
        title: "a `.step lines=` uses a step separator",
        cause: "The `lines=` value on a `.code-walkthrough`/`.scrolly` `.step` contains a `|`. \
                The `|` is the STEP separator of a `code-line-numbers=\"1|2-3\"` \
                spec; a `.step` is already one step, so its own `lines=` is parsed as \
                comma-separated ranges only. The `|` matches neither a range nor a number, so \
                the step silently focuses zero lines.",
        fix: "Use comma-separated ranges within the step (`lines=\"3-5,8\"`), and express \
              multiple reveal states as separate `.step` blocks — one per pipe group.",
    },
    Explanation {
        code: "TAL-INPUT-TYPE",
        title: "an unknown input type",
        cause: "A reactive `{{< input >}}` (or `//| input`) declares a widget type Taliesin \
                does not provide, so no control can be built for it.",
        fix: "Use a supported input type (the message suggests the nearest, e.g. \
              `slidr` -> `slider`).",
    },
    Explanation {
        code: "TAL-INPUT-ATTR",
        title: "a reactive input is missing a required attribute",
        cause: "A `{{< input >}}` control declares a valid type but omits something it \
                cannot work without: a `name=`, which is the control's identity in the \
                reactive graph (without one no `{js}` cell can read it, so the control is \
                inert), or, for `type=\"select\"`, the `options=` list that would fill the \
                menu.",
        fix: "Add the attribute the message names: `name=\"k\"` so cells can read the \
              control, or `options=\"a,b,c\"` on a select.",
    },
    Explanation {
        code: "TAL-SHORTCODE",
        title: "a shortcode taliesin could not read as written",
        cause: "A `{{< … >}}` invocation names something the tool does not know: an unknown \
                shortcode name, an unknown bare flag or `key=` argument, or a built-in with \
                no source path. Nothing is lost — an unknown name stays on the page as \
                literal text, and a known shortcode still renders with the options it did \
                understand — which is exactly why this used to be silent: the page looked \
                fine and the option you asked for simply never happened.",
        fix: "Fix the spelling inside the braces; the message names the nearest known \
              spelling when there is one. The built-ins are `{{< include file.tmd >}}`, \
              `{{< video clip.mp4 [controls] [audio] \
              [dark=] [poster=] [caption=] [captions=] >}}` and `{{< input … >}}`. A \
              shortcode written as an *example* belongs in a code fence or backticks, \
              which are never expanded and never linted.",
    },
    Explanation {
        code: "TAL-XREF-UNDEF",
        title: "a cross-reference points at nothing",
        cause: "An @-reference (`@fig-…`, `@sec-…`, `@tbl-…`, `@thm-…`) names a label that no \
                figure, section, table, or theorem in the document defines, so it cannot \
                resolve to a number or a link.",
        fix: "Fix the reference to match a real label (the message suggests the nearest), or \
              add the label to the target you meant to point at.",
    },
    Explanation {
        code: "TAL-XREF-UNREF",
        title: "a label exists but cannot be referenced",
        cause: "A labeled float or theorem cannot be reached by any @ref: either a hidden \
                cell (`include: false`) drops the output that would carry the anchor, or a \
                theorem id is missing its kind prefix (`math-of-primes` rather than \
                `thm-math-of-primes`).",
        fix: "Make the anchor reachable: let the cell show its output, or rename the id with \
              the right prefix (`thm-`, `fig-`, …) so `@id` resolves.",
    },
    Explanation {
        code: "TAL-DUP-ID",
        title: "two headings share an id",
        cause: "Two headings produce the same slug id, so an in-page link or `@sec-` \
                reference to it is ambiguous and jumps to whichever comes first.",
        fix: "Give one heading an explicit distinct id (`## Title {#unique-id}`), or reword \
              it so the auto-generated slugs differ.",
    },
    Explanation {
        code: "TAL-ANCHOR",
        title: "a broken in-page link",
        cause: "A same-page link (`[text](#fragment)`) targets a `#fragment` that no heading \
                or anchor on this page defines, so the click goes nowhere.",
        fix: "Point the link at a real id on the page, or add `{#fragment}` to the heading \
              you meant.",
    },
    Explanation {
        code: "TAL-LINK-ANCHOR",
        title: "a link to a missing anchor on another page",
        cause: "A link to `other.html#fragment` (or `other.tmd#fragment`) resolves to a real \
                page, but that page has no such `#fragment`.",
        fix: "Fix the fragment to a real anchor on the target page, or add the id there.",
    },
    Explanation {
        code: "TAL-LINK",
        title: "a broken relative link",
        cause: "A relative link points at a file or page that does not exist. A `.tmd` link \
                is checked against the site's page registry (it rewrites to the built \
                `.html`); a link into a `mounts:` prefix is exempt.",
        fix: "Correct the path to an existing sibling document or asset. External `http(s)` \
              links and mount prefixes are not checked.",
    },
    Explanation {
        code: "TAL-ASSET",
        title: "a local asset was not found",
        cause: "An image or other local asset (`![](path)`, `src=…`) points at a file that is \
                not on disk relative to the document, so it would render broken.",
        fix: "Fix the path, or add the missing file. Remote `http(s)` assets are not checked.",
    },
    Explanation {
        code: "TAL-MEDIA",
        title: "a local video was not found",
        cause: "A `{{< video clip.mp4 >}}` (or similar) names a local media file that does \
                not exist relative to the document.",
        fix: "Correct the path or add the file. Remote media URLs are not checked.",
    },
    Explanation {
        code: "TAL-REACTIVE",
        title: "a broken reactive graph",
        cause: "An `{js}` reactive cell either reads an input that no cell or `{{< input >}}` \
                defines, or the cells form a dependency cycle so none can run.",
        fix: "Define the missing input (or fix its name; the message suggests the nearest), \
              or break the cycle so the graph is acyclic.",
    },
    Explanation {
        code: "TAL-A11Y-HEADING",
        title: "a heading level skips",
        cause: "The outline jumps a level (for example h2 straight to h4) with nothing in \
                between, which breaks the structure for screen readers and the table of \
                contents.",
        fix: "Add an intervening heading, or demote the skipping heading one level so the \
              outline is contiguous.",
    },
    Explanation {
        code: "TAL-A11Y-NAME",
        title: "an interactive element has no accessible name",
        cause: "A link or button (native `<a href>` / `<button>`, or a role=link/button/tab \
                element) has no text and no label, so assistive tech announces it as unnamed.",
        fix: "Give it visible text, or an `aria-label` / `title`. An icon-only control still \
              needs a name.",
    },
    Explanation {
        code: "TAL-A11Y-LABEL",
        title: "a control's accessible name disagrees with its visible text",
        cause: "A link or button carries an `aria-label` that does not contain the words it \
                visibly reads. Someone driving the page by voice says what they can see \
                (\"click Save draft\"), and the browser matches against the accessible name — \
                so a control reading `Save draft` but named `Submit` cannot be operated by \
                voice at all, and a screen-reader user hears something different from what a \
                sighted colleague reads. WCAG 2.1 AA, 2.5.3 Label in Name. Text inside an \
                `aria-hidden=\"true\"` descendant does not count as the visible label, which \
                is the sanctioned way to keep a shortcut hint (`<kbd aria-hidden>⌘K</kbd>`) \
                out of the name.",
        fix: "Make the name CONTAIN the visible text rather than replace it: a control \
              reading `Search` may be named `Search the site`, not `Find`. If the extra \
              markup is decoration rather than label — an icon, a keyboard hint — mark that \
              element `aria-hidden=\"true\"` and leave the label alone. An icon-only control \
              with no visible text is not covered by this rule at all.",
    },
    Explanation {
        code: "TAL-A11Y-ALT",
        title: "an image is missing or has placeholder alt text",
        cause: "An image has no `alt` attribute, or its `alt` is a placeholder like \
                `image`/`photo` that describes nothing, so screen-reader users get no \
                information about it.",
        fix: "Add alt text that describes the image's content and purpose. Use `alt=\"\"` \
              only for a purely decorative image.",
    },
    Explanation {
        code: "TAL-CELL-ERROR",
        title: "a code cell raised an uncaught exception",
        cause: "A `{python}`/`{r}` cell ran and threw, so its traceback is baked into the \
                built page where its output should be. The build still writes the page (the \
                traceback is real output, and hiding it would ship a silently wrong \
                document), but the page is not publishable as it stands. `check` never \
                reports this: it does not execute cells.",
        fix: "Fix the cell's code and rebuild. To see the failure without a browser, \
              `taliesin build <file> --strict` fails the run and names the cell.",
    },
    Explanation {
        code: "TAL-KERNEL",
        title: "a code cell never ran",
        cause: "The cell was not executed at all: no kernel could be started for its \
                language (a missing or wrong interpreter path is the usual cause), the \
                kernel exited mid-build, or the execute request itself failed. Nothing is \
                wrong with the cell's code — this is an environment failure, and the page \
                carries a visible diagnostic where the output would be rather than dropping \
                it silently.",
        fix: "Point Taliesin at a working interpreter (`TALIESIN_PYTHON` / `TALIESIN_R`, or \
              `python:` / `r:` in `_site.yml`) and make sure its Jupyter kernel package is \
              installed (`ipykernel` for Python, `IRkernel` for R). `taliesin doctor` \
              reports what it can find.",
    },
    Explanation {
        code: "TAL-CITE-KEY",
        title: "a citation key that is not in the bibliography",
        cause: "A `[@key]` cites an entry the resolved `bibliography:` does not define, so \
                the citation renders as a raw key and the reference list has no row for it. \
                Distinct from TAL-CITE-BIB, which is the whole bibliography going missing: \
                here the file is found and this one key is wrong. Nothing is reported when \
                no bibliography resolves at all, since then every key would be `broken`.",
        fix: "Fix the key to one the bibliography defines (the message suggests the nearest \
              when there is one), or add the entry to your `.bib`.",
    },
    Explanation {
        code: "TAL-CITE-BIB",
        title: "a citation without a bibliography",
        cause: "The document cites sources (`[@key]`) but no `bibliography:` resolves the \
                keys, or a bare `@key` outside brackets did not render as a citation, so the \
                reference cannot be looked up.",
        fix: "Add a `bibliography:` pointing at your `.bib` and make sure each key exists in \
              it. Wrap a citation you meant as one in brackets: `[@key]`.",
    },
    Explanation {
        code: "TAL-CITE-UNUSED",
        title: "a bibliography entry that is never cited",
        cause: "A `.bib` entry is declared but no `[@key]` cites it, so it is dead weight: \
                it never reaches the reference list and nothing links to it. Reported \
                against whatever declared it — a page's own `bibliography:` is judged \
                against that page, and a project-wide `bibliography:` in `_site.yml` \
                against every page of the site, since a shared entry one page cites is in \
                use however many pages leave it alone.",
        fix: "Cite it (`[@key]`) or delete the entry. Advice, not a defect: it never fails \
              `check` or a build unless you ask with `--strict`.",
    },
    Explanation {
        code: "TAL-MATH",
        title: "a math expression could not be rendered",
        cause: "A `$…$` / `$$…$$` expression did not parse as valid KaTeX, so it cannot be \
                typeset and falls back to raw source.",
        fix: "Fix the LaTeX at the reported location: balance braces and `\\left`/`\\right`, \
              and use only macros KaTeX supports.",
    },
    Explanation {
        code: "TAL-CODE-LANG",
        title: "an unknown code language",
        cause: "A fenced code block names a language the highlighter does not know, so the \
                block renders as plain text with no syntax highlighting.",
        fix: "Use a recognized language tag, or leave the info string empty for an \
              unhighlighted block.",
    },
    Explanation {
        code: "TAL-CELL-RETIRED",
        title: "a retired cell language",
        cause: "The cell names a language Taliesin used to run and has since withdrawn. It \
                is not a typo, so the spelling advice the generic unknown-language warning \
                gives would be wrong: the capability is gone. The cell now renders as an \
                ordinary unhighlighted block, and nothing executes it.",
        fix: "Port the cell to the replacement the message names. Severity is `warning`, the \
              same an unrecognized language token gets: a plain `build` still succeeds and \
              writes the page, while `check` and `build --strict` report it and exit \
              non-zero, so an unmigrated document does not silently ship as if nothing \
              changed.",
    },
];

/// The [`Explanation`] for `code`, case-insensitively (`tal-fm-key` == `TAL-FM-KEY`), or
/// `None` for a code with no catalogued explanation.
pub fn explain(code: &str) -> Option<&'static Explanation> {
    let want = code.to_ascii_uppercase();
    EXPLANATIONS.iter().find(|e| e.code == want)
}

/// Every distinct diagnostic code (the [`TABLE`] families plus [`GENERIC`]), sorted +
/// deduped. The candidate set for `--explain`'s did-you-mean, the `--explain` index, and the
/// completion of `check --explain <TAB>`.
pub fn all_codes() -> Vec<&'static str> {
    let mut codes: Vec<&'static str> = TABLE.iter().map(|(_, code, _)| *code).collect();
    codes.push(GENERIC);
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// The canonical `docs_url` for a code: the committed catalog anchored by the lowercased
/// code (`…/DIAGNOSTICS.md#tal-fm-key`). Computed, so it can never drift from the code.
pub fn docs_url(code: &str) -> String {
    format!("{DIAGNOSTICS_DOC_URL}#{}", code.to_ascii_lowercase())
}

/// The `docs/DIAGNOSTICS.md` catalog, rendered from [`EXPLANATIONS`] in [`all_codes`] order.
/// One `## <CODE>` heading per code (so GitHub's heading anchors back [`docs_url`]) with its
/// title, cause, and fix. The committed file is drift-locked to this by
/// `diagnostics_md_matches_committed`.
pub fn diagnostics_markdown() -> String {
    let mut s = String::new();
    s.push_str("# Taliesin diagnostic codes\n\n");
    s.push_str(
        "Every diagnostic `taliesin check` reports carries a stable `TAL-*` code. This \
         catalog expands each into its cause and canonical fix; it is generated from the \
         code table in `crates/core/src/diagnostics/codes.rs`, so do not edit it by hand \
         (regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`). The \
         same text is available offline via `taliesin check --explain <CODE>`.\n\n",
    );
    for code in all_codes() {
        let e = explain(code).expect("all_codes is a subset of EXPLANATIONS");
        s.push_str(&format!(
            "## {}\n\n**{}**\n\n{}\n\nTo fix: {}\n\n",
            e.code, e.title, e.cause, e.fix
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_families_with_stable_codes() {
        assert_eq!(
            classify("unknown front-matter key `treme` (did you mean `theme`?)"),
            ("TAL-FM-KEY", WARNING)
        );
        assert_eq!(
            classify("broken cross-reference: @fig-x (did you mean `@fig-y`?)"),
            ("TAL-XREF-UNDEF", ERROR)
        );
        assert_eq!(
            classify("image is missing alt text"),
            ("TAL-A11Y-ALT", WARNING)
        );
        assert_eq!(
            classify("local asset not found: `x.png`"),
            ("TAL-ASSET", ERROR)
        );
    }

    #[test]
    fn a_shape_diagnostic_outranks_a_needle_inside_the_authors_own_heading() {
        // Every shape message quotes the author's heading back verbatim, and `classify` is
        // first-hit-wins over the whole string. A section titled "Math" or "Bibliography"
        // is entirely ordinary, so without the shape rows sitting ahead of the generic
        // needles these would classify as TAL-MATH / TAL-CITE-BIB. Hostile headings on
        // purpose.
        assert_eq!(
            classify(
                "heading `Math` has no content under it: the next thing on the page is \
                 another heading, so the section is a TOC row that leads nowhere"
            ),
            ("TAL-SHAPE-HOLLOW", SUGGESTION)
        );
        assert_eq!(
            classify(
                "duplicate heading text `Bibliography`: an earlier heading on this page \
                 reads the same, so the TOC shows two rows a reader cannot tell apart"
            ),
            ("TAL-SHAPE-DUP", SUGGESTION)
        );
        assert_eq!(
            classify(
                "heading `Broken link` repeats the page title: it adds a TOC row that tells \
                 a reader nothing the title did not already say"
            ),
            ("TAL-SHAPE-ECHO", SUGGESTION)
        );
        // Advice, never a gate: none of these may reach the `build --strict` floor.
        for (_, code, sev) in TABLE.iter().filter(|(_, c, _)| c.starts_with("TAL-SHAPE-")) {
            assert_eq!(*sev, SUGGESTION, "{code} must stay advisory");
        }
    }

    #[test]
    fn an_unreferenceable_label_outranks_a_needle_in_its_own_label() {
        // These two messages embed the author's anchor VERBATIM, and `classify` scans the
        // whole string first-hit-wins — so a perfectly ordinary label that happens to
        // contain a later family's needle would hijack the code. Measured before the
        // needle existed: `fig-math-model` classified as TAL-MATH, and the theorem
        // warning (which had no family at all) did the same. Hostile labels on purpose.
        assert_eq!(
            classify(
                "figure label \u{201c}fig-math-model\u{201d} cannot be cross-referenced: \
                 `include: false` drops the cell's output, so nothing carries the anchor \
                 and `@fig-math-model` won't resolve"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
        assert_eq!(
            classify(
                "table label \u{201c}tbl-bibliography-counts\u{201d} cannot be cross-referenced: \
                 `include: false` drops the cell's output, so nothing carries the anchor \
                 and `@tbl-bibliography-counts` won't resolve"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
        // The theorem-prefix warning is the same family and was previously uncatalogued
        // (GENERIC/ERROR), so it hijacked the same way.
        assert_eq!(
            classify(
                "theorem id \u{201c}math-of-primes\u{201d} cannot be cross-referenced \
                 (`@math-of-primes` won't resolve); use `thm-math-of-primes`"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
    }

    #[test]
    fn a_broken_reference_and_an_unreachable_definition_are_different_families() {
        // Two sides of one mistake, and an agent triages them differently: one edits the
        // `@ref`, the other edits the cell that defines it.
        assert_eq!(
            classify("broken cross-reference: @fig-x (no such figure/section/\u{2026})").0,
            "TAL-XREF-UNDEF"
        );
        assert_eq!(
            classify(
                "figure label \u{201c}fig-x\u{201d} cannot be cross-referenced: `include: false` \
                 drops the cell's output, so nothing carries the anchor and `@fig-x` won't resolve"
            )
            .0,
            "TAL-XREF-UNREF"
        );
    }

    #[test]
    fn more_specific_needle_wins() {
        assert_eq!(classify("broken link anchor `#x`").0, "TAL-LINK-ANCHOR");
        assert_eq!(classify("broken link: `x.tmd`").0, "TAL-LINK");
    }

    #[test]
    fn unsupported_key_outranks_the_generic_citation_needles() {
        // The `csl:` message names citation concepts, so it would classify as TAL-CITE-BIB
        // if its needle were ordered after them. It is a front-matter defect (delete the
        // key), not a bibliography one, so pin both the code and the ordering.
        // Produced by the real rule at its real home (the render path), so this keeps
        // pinning the shipped message rather than a copy that could drift from it.
        let csl: Vec<_> = crate::frontmatter::validate_front_matter(
            "---\ntitle: T\ncsl: apa.csl\n---\n\nBody.\n",
        )
        .into_iter()
        .filter(|w| w.message.contains("is recognized but not supported"))
        .collect();
        assert_eq!(csl.len(), 1, "the csl warning: {csl:?}");
        assert_eq!(
            classify(&csl[0].message),
            ("TAL-FM-UNSUPPORTED", WARNING),
            "csl classifies as an unsupported front-matter key: {}",
            csl[0].message
        );
        // No replacement exists, so no structured suggestion may be lifted: an agent must
        // not be handed a fix to apply.
        assert_eq!(extract_suggestion(&csl[0].message), None);
    }

    #[test]
    fn a_broken_citation_is_one_family_whether_or_not_it_has_a_suggestion() {
        // DIAG-1. These two are the SAME defect (a `[@key]` the bibliography does not
        // define) and differ only in whether a near-miss was found. Before the needle
        // existed they classified as two different codes at two different severities: the
        // no-suggestion variant ends "(not in the bibliography)" and so hit the generic
        // `bibliography` needle as TAL-CITE-BIB/warning, while the *more* actionable
        // did-you-mean variant matched nothing and fell through to TAL-CHECK/**error**,
        // failing `check` and `build --strict` on a typo'd citation key.
        assert_eq!(
            classify("broken citation: @bishop2006patern (did you mean `@bishop2006pattern`?)"),
            ("TAL-CITE-KEY", WARNING)
        );
        assert_eq!(
            classify("broken citation: @nosuchkey (not in the bibliography)"),
            ("TAL-CITE-KEY", WARNING),
            "the trailing `bibliography` must not outrank the citation-key family"
        );
        // The missing-bibliography family is still its own thing: a different fix (add the
        // file) for a different defect.
        assert_eq!(
            classify("citations are present but no bibliography resolves them").0,
            "TAL-CITE-BIB"
        );
    }

    #[test]
    fn widget_and_div_families_outrank_the_needles_inside_the_authors_own_names() {
        // DIAG-1's other half. Each of these embeds author-controlled text (a class name, an
        // input name) or names a construct whose words collide with later generic needles, so
        // they are ordered above them. Hostile spellings on purpose: `mathz` and `bibliografy`
        // are exactly the near-misses this rule exists to catch, and both would classify as
        // TAL-MATH / TAL-CITE-BIB if the div row sat below the citation/math needles.
        assert_eq!(
            classify("unknown div class `mathz` (did you mean `math`?)"),
            ("TAL-DIV-CLASS", WARNING)
        );
        assert_eq!(
            classify("unknown div class `fragmnet` (did you mean `fragment`?)"),
            ("TAL-DIV-CLASS", WARNING)
        );
        // One family for "the container is half-built", one row per container.
        for m in [
            "`.panel-tabset` has no headings, so it renders no tabs (add `##` headings)",
            "`.code-walkthrough` has no code block to show in the sticky panel",
            "`.scrolly` has no sticky stage (add a figure or `{js}` cell)",
            "`.scrolly` has no `.step` divs to scroll through",
        ] {
            assert_eq!(classify(m), ("TAL-DIV-PARTS", WARNING), "{m}");
        }
        // A missing attribute is not an unknown type: same surface, different edit.
        assert_eq!(
            classify("`.input` needs a `name=` to feed the reactive graph"),
            ("TAL-INPUT-ATTR", WARNING)
        );
        assert_eq!(
            classify("`.input type=select` needs `options=\"a,b,c\"`"),
            ("TAL-INPUT-ATTR", WARNING)
        );
        assert_eq!(
            classify("unknown input type `slidr` (did you mean `slider`?)").0,
            "TAL-INPUT-TYPE",
            "the unknown-type family keeps its own code"
        );
        // None of these may gate a release: every one is an authoring slip whose page still
        // renders, which is what made the ERROR fall-through wrong rather than merely untidy.
        for m in [
            "unknown div class `fragmnet` (did you mean `fragment`?)",
            "`.scrolly` has no `.step` divs to scroll through",
            "`.input` needs a `name=` to feed the reactive graph",
        ] {
            assert_eq!(classify(m).1, WARNING, "{m}");
        }
    }

    #[test]
    fn shortcode_authoring_slips_are_one_warning_family_not_the_generic_error() {
        // Item 77 residual. Shortcode diagnostics had no family at all, so every one fell
        // through to `(GENERIC, ERROR)` — and ERROR is the wrong severity twice over: the
        // page still renders (the shortcode keeps the options it understood, or stays
        // literal text), and `build --strict` gates on errors, so a one-letter
        // typo blocked a release. Same class as the `.input` / div families above.
        for m in [
            "unknown `{{< video >}}` option `control` (did you mean `controls`?) at line 5",
            "unknown `{{< video >}}` argument `postr=` (did you mean `poster=`?) at line 5",
            "`{{< video >}}` at line 5 has no source path (write `{{< video file >}}`)",
            "unknown shortcode `{{< vidoe >}}` at line 7 (left as literal text)",
        ] {
            assert_eq!(classify(m), ("TAL-SHORTCODE", WARNING), "{m}");
        }
        // The needle must not be so broad it swallows unrelated prose that merely says the
        // word: the message body a validator emits about a fenced div is not a shortcode.
        assert_ne!(
            classify("`.input` is the shortcode, not a fenced div").0,
            "TAL-SHORTCODE",
            "the family keys on the `{{< … >}}` form, not on the word"
        );
    }

    #[test]
    fn uncatalogued_message_gets_a_stable_generic_code() {
        let (code, sev) = classify("something entirely new");
        assert_eq!((code, sev), (GENERIC, ERROR));
        assert!(!code.is_empty());
    }

    #[test]
    fn extracts_a_did_you_mean_replacement() {
        assert_eq!(
            extract_suggestion("unknown front-matter key `treme` (did you mean `theme`?)")
                .as_deref(),
            Some("theme")
        );
        assert_eq!(
            extract_suggestion(
                "broken cross-reference: @fig-reslts (did you mean `@fig-results`?)"
            )
            .as_deref(),
            Some("@fig-results")
        );
        assert_eq!(extract_suggestion("no hint here").as_deref(), None);
    }

    // ---- DX6: `check --explain <CODE>` catalog ---------------------------------------

    #[test]
    fn every_code_has_a_nonempty_explanation() {
        // The load-bearing drift guard: a new diagnostic family (a code added to TABLE) with
        // no EXPLANATIONS entry fails here, so `check --explain` can never learn a code it
        // can't explain. GENERIC is included: `TAL-CHECK` is a real emitted code.
        for code in all_codes() {
            let e = explain(code).unwrap_or_else(|| panic!("no explanation for {code}"));
            assert!(!e.title.is_empty(), "{code}: empty title");
            assert!(!e.cause.is_empty(), "{code}: empty cause");
            assert!(!e.fix.is_empty(), "{code}: empty fix");
            assert_eq!(e.code, code, "explanation.code echoes its key");
        }
    }

    #[test]
    fn no_orphan_explanations() {
        // Every explained code is a real code (in TABLE or GENERIC), and no code is
        // explained twice. Catches an entry left behind after a code is removed/renamed.
        let codes = all_codes();
        let mut seen = std::collections::BTreeSet::new();
        for e in EXPLANATIONS {
            assert!(
                codes.contains(&e.code),
                "{} is explained but is not a real code",
                e.code
            );
            assert!(seen.insert(e.code), "{} is explained twice", e.code);
        }
    }

    #[test]
    fn all_codes_is_sorted_deduped_and_holds_generic() {
        let codes = all_codes();
        assert!(codes.contains(&GENERIC), "all_codes includes TAL-CHECK");
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes, sorted, "all_codes is sorted + deduped");
    }

    #[test]
    fn explain_is_case_insensitive() {
        assert_eq!(
            explain("tal-fm-key").map(|e| e.code),
            explain("TAL-FM-KEY").map(|e| e.code)
        );
        assert_eq!(explain("TAL-FM-KEY").map(|e| e.code), Some("TAL-FM-KEY"));
        assert!(explain("TAL-NOPE").is_none());
    }

    #[test]
    fn docs_url_is_the_computed_lowercase_anchor() {
        assert_eq!(
            docs_url("TAL-XREF-UNREF"),
            format!("{DIAGNOSTICS_DOC_URL}#tal-xref-unref")
        );
        // Every code's url anchors into the same committed catalog.
        for code in all_codes() {
            assert!(docs_url(code).starts_with(DIAGNOSTICS_DOC_URL));
            assert!(docs_url(code).ends_with(&code.to_ascii_lowercase()));
        }
    }

    /// Assert `docs/DIAGNOSTICS.md` equals the generated catalog, OR rewrite it under
    /// `TALIESIN_BLESS=1` (mirrors `schema.rs::bless_or_assert`). `rel` is relative to the
    /// core crate root.
    #[test]
    fn diagnostics_md_matches_committed() {
        let rel = "../../docs/DIAGNOSTICS.md";
        let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
        let generated = diagnostics_markdown();
        if std::env::var("TALIESIN_BLESS").is_ok() {
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed {rel}");
        } else {
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(
                generated, committed,
                "diagnostics catalog drift in {rel}; regenerate with \
                 `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`"
            );
        }
    }
}
