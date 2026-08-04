# Taliesin diagnostic codes

Every diagnostic `taliesin check` reports carries a stable `TAL-*` code. This catalog expands each into its cause and canonical fix; it is generated from the code table in `crates/core/src/diagnostics/codes.rs`, so do not edit it by hand (regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`). The same text is available offline via `taliesin check --explain <CODE>`.

## TAL-A11Y-ALT

**an image is missing or has placeholder alt text**

An image has no `alt` attribute, or its `alt` is a placeholder like `image`/`photo` that describes nothing, so screen-reader users get no information about it.

To fix: Add alt text that describes the image's content and purpose. Use `alt=""` only for a purely decorative image.

## TAL-A11Y-HEADING

**a heading level skips**

The outline jumps a level (for example h2 straight to h4) with nothing in between, which breaks the structure for screen readers and the table of contents.

To fix: Add an intervening heading, or demote the skipping heading one level so the outline is contiguous.

## TAL-A11Y-LABEL

**a control's accessible name disagrees with its visible text**

A link or button carries an `aria-label` that does not contain the words it visibly reads. Someone driving the page by voice says what they can see ("click Save draft"), and the browser matches against the accessible name — so a control reading `Save draft` but named `Submit` cannot be operated by voice at all, and a screen-reader user hears something different from what a sighted colleague reads. WCAG 2.1 AA, 2.5.3 Label in Name. Text inside an `aria-hidden="true"` descendant does not count as the visible label, which is the sanctioned way to keep a shortcut hint (`<kbd aria-hidden>⌘K</kbd>`) out of the name.

To fix: Make the name CONTAIN the visible text rather than replace it: a control reading `Search` may be named `Search the site`, not `Find`. If the extra markup is decoration rather than label — an icon, a keyboard hint — mark that element `aria-hidden="true"` and leave the label alone. An icon-only control with no visible text is not covered by this rule at all.

## TAL-A11Y-NAME

**an interactive element has no accessible name**

A link or button (native `<a href>` / `<button>`, or a role=link/button/tab element) has no text and no label, so assistive tech announces it as unnamed.

To fix: Give it visible text, or an `aria-label` / `title`. An icon-only control still needs a name.

## TAL-ANCHOR

**a broken in-page link**

A same-page link (`[text](#fragment)`) targets a `#fragment` that no heading or anchor on this page defines, so the click goes nowhere.

To fix: Point the link at a real id on the page, or add `{#fragment}` to the heading you meant.

## TAL-ASSET

**a local asset was not found**

An image or other local asset (`![](path)`, `src=…`) points at a file that is not on disk relative to the document, so it would render broken.

To fix: Fix the path, or add the missing file. Remote `http(s)` assets are not checked.

## TAL-CALLOUT-KIND

**an unknown callout kind**

A `::: {.callout-…}` block names a callout type that is not one of Taliesin's kinds (`note`, `tip`, `important`, `warning`, `caution`), so it would render as a plain fenced div with no callout styling.

To fix: Change the kind to a supported one (the message suggests the nearest match), e.g. `::: {.callout-important}`.

## TAL-CELL-ERROR

**a code cell raised an uncaught exception**

A `{python}`/`{r}` cell ran and threw, so its traceback is baked into the built page where its output should be. The build still writes the page (the traceback is real output, and hiding it would ship a silently wrong document), but the page is not publishable as it stands. `check` never reports this: it does not execute cells.

To fix: Fix the cell's code and rebuild. To see the failure without a browser, `taliesin read --run <file>` prints a `[cell error: …]` line per cell.

## TAL-CELL-OPTION

**an unknown cell option**

A `#|` / `//|` option line on a code cell uses a key Taliesin does not recognize, so it has no effect on how the cell runs or renders.

To fix: Correct the option to a known one (the message suggests the nearest, e.g. `labl` -> `label`), or remove it. See the cell-options reference.

## TAL-CHECK

**an uncatalogued diagnostic**

This diagnostic has not been assigned a specific TAL-* family yet, so it carries the generic code. The message text itself is the guide to what tripped.

To fix: Read the message and its location, then act on what it names. If one kind of problem keeps surfacing this way, that family is a candidate for its own code.

## TAL-CITE-BIB

**a citation without a bibliography**

The document cites sources (`[@key]`) but no `bibliography:` resolves the keys, or a bare `@key` outside brackets did not render as a citation, so the reference cannot be looked up.

To fix: Add a `bibliography:` pointing at your `.bib` and make sure each key exists in it. Wrap a citation you meant as one in brackets: `[@key]`.

## TAL-CITE-KEY

**a citation key that is not in the bibliography**

A `[@key]` cites an entry the resolved `bibliography:` does not define, so the citation renders as a raw key and the reference list has no row for it. Distinct from TAL-CITE-BIB, which is the whole bibliography going missing: here the file is found and this one key is wrong. Nothing is reported when no bibliography resolves at all, since then every key would be `broken`.

To fix: Fix the key to one the bibliography defines (the message suggests the nearest when there is one), or add the entry to your `.bib`.

## TAL-CITE-UNUSED

**a bibliography entry that is never cited**

A `.bib` entry is declared but no `[@key]` cites it, so it is dead weight: it never reaches the reference list and nothing links to it. Reported against whatever declared it — a page's own `bibliography:` is judged against that page, and a project-wide `bibliography:` in `_site.yml` against every page of the site, since a shared entry one page cites is in use however many pages leave it alone.

To fix: Cite it (`[@key]`) or delete the entry. Advice, not a defect: it never fails `check` or a build unless you ask with `--strict`.

## TAL-CODE-LANG

**an unknown code language**

A fenced code block names a language the highlighter does not know, so the block renders as plain text with no syntax highlighting.

To fix: Use a recognized language tag, or leave the info string empty for an unhighlighted block.

## TAL-COLUMN-WIDTH

**a `.column width=` is ignored**

A `.columns` grid lays its `.column` children out in EQUAL columns, so a per-column `width=` (a reveal/Quarto habit, e.g. `::: {.column width="70%"}`) has no effect — the split is silently equalized.

To fix: Remove the `width=` (the columns are equal), or set an explicit column count with `::: {.columns ncol=N}` or `::: {layout-ncol=N}`. Variable-width columns are not supported.

## TAL-DIV-CLASS

**a misspelled feature div class**

A `:::` fenced div carries a class that is a near-miss of one Taliesin implements (`.fragmnet` for `.fragment`, `.theorm` for `.theorem`), so the feature never dispatches and the div renders as a plain container. Div classes are an OPEN vocabulary — a genuinely custom class you style yourself is silent — so this fires only within edit distance 2 of a known name.

To fix: Correct the class to the one the message suggests. If the class really is your own, rename it so it is not a near-miss of a built-in.

## TAL-DIV-PARTS

**a feature div is missing a part it needs**

A `.panel-tabset`, `.code-walkthrough` or `.scrolly` has content but not the part that makes it work: a tabset builds its tabs from `##` headings, a walkthrough pins a code block in its sticky panel, and a scrolly needs both a sticky stage (a figure or `{js}` cell) and `.step` divs to scroll past it. The container still renders, just half-formed — a tab strip with no tabs, an empty sticky panel, a scroller that drives nothing. Distinct from TAL-EMPTY-DIV, which is a feature div with no content at all.

To fix: Add the missing part named in the message: `##` headings inside the tabset, a fenced code block inside the walkthrough, or a stage and `.step` blocks inside the scrolly.

## TAL-DUP-ID

**two headings share an id**

Two headings produce the same slug id, so an in-page link or `@sec-` reference to it is ambiguous and jumps to whichever comes first.

To fix: Give one heading an explicit distinct id (`## Title {#unique-id}`), or reword it so the auto-generated slugs differ.

## TAL-EMPTY-DIV

**an empty feature div renders nothing**

A `:::` fenced div names a real feature (a `.input` reactive control, a `.callout-…`, a `.panel-tabset`, a theorem, …) but has no content between its fences, so it is dropped and renders nothing. The most common case is reaching for `::: {.input name="k"}` as a div — the reactive input control is a shortcode, not a fenced div.

To fix: Put content between the `:::` fences (the callout body, the tabset's `##` headings, the theorem statement), or, for a reactive input, use the shortcode form `{{< input name="k" … >}}` instead of a div.

## TAL-FM-FORMAT

**an unknown `format:` value**

The `format:` field names an output Taliesin does not produce. Taliesin renders HTML only (`html`, `deck`); format names from other tools (`revealjs`, `pdf`, `docx`) have no meaning here.

To fix: Use a format Taliesin supports, or drop the field to accept the default. HTML is the only output target; a slide deck is `format: deck`.

## TAL-FM-KEY

**an unknown key in front matter**

A key in the document's front matter (or a nested `execute:`/`listing:`/`hero:`/`theorems:`/`prose-lint:` block) is not in Taliesin's closed vocabulary. It is a typo, or a key from another tool that Taliesin does not implement, so it would be silently ignored.

To fix: Correct the key to the nearest valid name (`check --format json` carries a `suggestion.replacement` for a near-miss), or remove it. The front-matter reference lists every recognized key.

## TAL-FM-UNSUPPORTED

**a recognized but unsupported key**

This key is one Taliesin knows about but deliberately does not act on (for example `csl:`), so leaving it in implies an effect that never happens.

To fix: Remove the key. The behavior it configures in another tool is not part of Taliesin's HTML output.

## TAL-FM-YAML

**the YAML front matter is malformed**

The block between the opening and closing `---` is not valid YAML (an unterminated quote, bad indentation, or a stray tab), so the strict parse rejected it before any field could be read.

To fix: Fix the YAML at the reported line: close the quote, align the indentation, or replace tabs with spaces. Every value after a parse error is lost, so the parse must succeed first.

## TAL-INPUT-ATTR

**a reactive input is missing a required attribute**

A `{{< input >}}` control declares a valid type but omits something it cannot work without: a `name=`, which is the control's identity in the reactive graph (without one no `{js}` cell can read it, so the control is inert), or, for `type="select"`, the `options=` list that would fill the menu.

To fix: Add the attribute the message names: `name="k"` so cells can read the control, or `options="a,b,c"` on a select.

## TAL-INPUT-TYPE

**an unknown input type**

A reactive `{{< input >}}` (or `//| input`) declares a widget type Taliesin does not provide, so no control can be built for it.

To fix: Use a supported input type (the message suggests the nearest, e.g. `slidr` -> `slider`).

## TAL-KERNEL

**a code cell never ran**

The cell was not executed at all: no kernel could be started for its language (a missing or wrong interpreter path is the usual cause), the kernel exited mid-build, or the execute request itself failed. Nothing is wrong with the cell's code — this is an environment failure, and the page carries a visible diagnostic where the output would be rather than dropping it silently.

To fix: Point Taliesin at a working interpreter (`TALIESIN_PYTHON` / `TALIESIN_R`, or `python:` / `r:` in `_site.yml`) and make sure its Jupyter kernel package is installed (`ipykernel` for Python, `IRkernel` for R). `taliesin doctor` reports what it can find.

## TAL-LINK

**a broken relative link**

A relative link points at a file or page that does not exist. A `.tmd` link is checked against the site's page registry (it rewrites to the built `.html`); a link into a `mounts:` prefix is exempt.

To fix: Correct the path to an existing sibling document or asset. External `http(s)` links and mount prefixes are not checked.

## TAL-LINK-ANCHOR

**a link to a missing anchor on another page**

A link to `other.html#fragment` (or `other.tmd#fragment`) resolves to a real page, but that page has no such `#fragment`.

To fix: Fix the fragment to a real anchor on the target page, or add the id there.

## TAL-LINK-TEXT

**two links on one page read the same but go elsewhere**

Two links on this page have the same accessible name and different destinations, so neither one says where it goes. A screen reader can list a page's links out of context, where the text is all the reader gets — and a sighted reader scanning for the link they already followed cannot tell the two apart either. Destinations are compared ignoring the `#fragment`, so two deep links into one page are deliberately NOT flagged.

To fix: Make the link text name its own destination (`the execution model` rather than a second `this chapter`). Do not paper over it with `aria-label`: a label that disagrees with the visible text breaks voice control (WCAG 2.5.3, Label in Name). This is advice, severity `suggestion`, so it never fails `check`, `build --strict` or `publish` unless you ask with `check --strict`.

## TAL-MATH

**a math expression could not be rendered**

A `$…$` / `$$…$$` expression did not parse as valid KaTeX, so it cannot be typeset and falls back to raw source.

To fix: Fix the LaTeX at the reported location: balance braces and `\left`/`\right`, and use only macros KaTeX supports.

## TAL-MEDIA

**a local video was not found**

A `{{< video clip.mp4 >}}` (or similar) names a local media file that does not exist relative to the document.

To fix: Correct the path or add the file. Remote media URLs are not checked.

## TAL-PROSE-BANNED

**a term this document's own banned list forbids**

The document's `prose-lint: {banned: [...]}` list names this term, so the lint flagged it. The list is yours; nothing is banned by default.

To fix: Use the wording you decided on instead, or drop the term from the `banned` list if the ban no longer applies.

## TAL-PROSE-REPEAT

**the same word twice in a row**

The opt-in prose lint found a word immediately repeated (`the the`, `a a`). Almost always an editing artefact left by a rewritten sentence, and one of the few prose defects that is genuinely objective.

To fix: Delete the duplicate. If the repetition is deliberate (a quoted stutter, a literal), the rule has no exception list — reword or turn the lint off for that document.

## TAL-PROSE-WEASEL

**a hedging word the sentence does not need**

The opt-in prose lint (`prose-lint:` in front matter) found one of a small closed list of hedges — `very`, `simply`, `obviously`, `basically` and friends. They read as emphasis but carry no information, and `obviously` additionally tells a reader who did not find it obvious that they should have.

To fix: Cut the word and read the sentence again; it almost always survives unchanged. This is advice, not a defect: it is severity `suggestion`, so it never fails `check`, `build --strict` or `publish` unless you ask with `check --strict`.

## TAL-PYODIDE-ESCAPE

**a `{pyodide}` cell's source has an ambiguous `<\/script`**

The wrapper escapes a literal `</script` inside a `{pyodide}` cell's source to `<\/script`, so it survives untouched inside the wrapping `<script>` element. The one output mode that cannot ship the 15.7 MiB Pyodide runtime — a single-file `build file.tmd out.html` — degrades the cell to visible highlighted source by reversing that escape, and the reversal cannot tell a real `</script` apart from an author who typed the literal `<\/script` themselves: both produce the identical `<\/script` in the rendered HTML, so in that one artifact the author's own backslash is silently dropped.

To fix: Nothing to change unless you ship this exact page as a single self-contained file: preview and every other build mode ship the real Pyodide runtime and never reverse the escape, so the source stays exact there. This is advice, severity `suggestion`, so it never fails `check`, `build --strict` or `publish` unless you ask with `check --strict`. If a single-file build of this page matters, avoid writing the literal sequence `<\/script` verbatim in the cell's source.

## TAL-REACTIVE

**a broken reactive graph**

An `{js}` reactive cell either reads an input that no cell or `{{< input >}}` defines, or the cells form a dependency cycle so none can run.

To fix: Define the missing input (or fix its name; the message suggests the nearest), or break the cycle so the graph is acyclic.

## TAL-SHAPE-CAPTION

**a numbered figure whose caption is only its label**

The figure is numbered and can be cross-referenced, but its caption is empty or reads only `Figure 2:`. A caption is the most-read text on a page after the heading, and a cross-reference to a figure that describes nothing makes the reference unreadable too.

To fix: Write what the figure shows (`![Fatality rate by manufacturer, 1990-2020](f.png){#fig-rates}`). If it genuinely needs no caption, drop the `{#fig-…}` id so it is not numbered.

## TAL-SHAPE-DUP

**two headings on one page read the same**

Two headings on the same page have identical text, so the table of contents shows two rows a reader cannot tell apart and neither one says which is which. Distinct from TAL-DUP-ID, which is about the emitted anchor rather than the words.

To fix: Make the second heading say what actually distinguishes it (`Model summary` and `Model summary (pooled)`), or merge the two sections if they are one.

## TAL-SHAPE-ECHO

**a body heading repeats the page title**

A heading below the first one restates the document's own `title:`, so it adds a table-of-contents row that tells a reader nothing new. The page's *leading* heading is deliberately exempt — opening a landing page with a heading that matches its title is an ordinary idiom, not a defect.

To fix: Name the section for what that section covers, or drop the heading and let the title carry it.

## TAL-SHAPE-EMPTY

**a heading with no text**

A heading opens a section but carries no words, so the table of contents, the book outline and any cross-reference to it all render a blank row. Usually a heading whose text was cut without cutting the `#` line.

To fix: Give the heading a name, or delete the line. This is advice, not a defect: it is severity `suggestion`, so it never fails `check`, `build --strict` or `publish` unless you ask with `check --strict`.

## TAL-SHAPE-HOLLOW

**a heading with nothing under it**

This heading has neither text nor subsections beneath it, so the section is empty on any reading: a table-of-contents row that leads nowhere. Any content counts — a list, a code cell, a figure or a table, not just a paragraph. A heading followed by DEEPER headings is an ordinary grouping parent and is deliberately exempt: it does have content in the document tree, so asking for an intro paragraph there would be a style opinion.

To fix: Write the section, or delete the heading. If it was meant to group other sections, give it real subsections (deeper headings) rather than siblings.

## TAL-SHORTCODE

**a shortcode taliesin could not read as written**

A `{{< … >}}` invocation names something the tool does not know: an unknown shortcode name, an unknown bare flag or `key=` argument, or a built-in with no source path. Nothing is lost — an unknown name stays on the page as literal text, and a known shortcode still renders with the options it did understand — which is exactly why this used to be silent: the page looked fine and the option you asked for simply never happened.

To fix: Fix the spelling inside the braces; the message names the nearest known spelling when there is one. The built-ins are `{{< include file.tmd >}}`, `{{< embed deck.tmd [title=…] >}}`, `{{< video clip.mp4 [controls] [audio] [dark=] [poster=] [caption=] [captions=] >}}` and `{{< input … >}}`. A shortcode written as an *example* belongs in a code fence or backticks, which are never expanded and never linted.

## TAL-STEP-LINES

**a `.step lines=` uses a step separator**

The `lines=` value on a `.code-walkthrough`/`.scrolly` `.step` contains a `|`. The `|` is the STEP separator of a deck/listing `code-line-numbers="1|2-3"` spec; a `.step` is already one step, so its own `lines=` is parsed as comma-separated ranges only. The `|` matches neither a range nor a number, so the step silently focuses zero lines.

To fix: Use comma-separated ranges within the step (`lines="3-5,8"`), and express multiple reveal states as separate `.step` blocks — one per pipe group.

## TAL-XREF-UNDEF

**a cross-reference points at nothing**

An @-reference (`@fig-…`, `@sec-…`, `@tbl-…`, `@thm-…`) names a label that no figure, section, table, or theorem in the document defines, so it cannot resolve to a number or a link.

To fix: Fix the reference to match a real label (the message suggests the nearest), or add the label to the target you meant to point at.

## TAL-XREF-UNREF

**a label exists but cannot be referenced**

A labeled float or theorem cannot be reached by any @ref: either a hidden cell (`include: false`) drops the output that would carry the anchor, or a theorem id is missing its kind prefix (`math-of-primes` rather than `thm-math-of-primes`).

To fix: Make the anchor reachable: let the cell show its output, or rename the id with the right prefix (`thm-`, `fig-`, …) so `@id` resolves.

