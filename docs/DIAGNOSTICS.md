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

## TAL-CATEGORY

**a near-miss category splits the archive**

A `categories:` value is a case-variant or typo of another category used elsewhere on the site (`Statistics` vs `statistics`), so the listing filter silently forks one topic into two chips.

To fix: Normalize the spelling to match the canonical category (the message names it), so every post on the topic shares one chip.

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

## TAL-CODE-LANG

**an unknown code language**

A fenced code block names a language the highlighter does not know, so the block renders as plain text with no syntax highlighting.

To fix: Use a recognized language tag, or leave the info string empty for an unhighlighted block.

## TAL-DUP-ID

**two headings share an id**

Two headings produce the same slug id, so an in-page link or `@sec-` reference to it is ambiguous and jumps to whichever comes first.

To fix: Give one heading an explicit distinct id (`## Title {#unique-id}`), or reword it so the auto-generated slugs differ.

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

## TAL-INPUT-TYPE

**an unknown input type**

A reactive `{{< input >}}` (or `//| input`) declares a widget type Taliesin does not provide, so no control can be built for it.

To fix: Use a supported input type (the message suggests the nearest, e.g. `slidr` -> `slider`).

## TAL-LINK

**a broken relative link**

A relative link points at a file or page that does not exist. A `.tmd` link is checked against the site's page registry (it rewrites to the built `.html`); a link into a `mounts:` prefix is exempt.

To fix: Correct the path to an existing sibling document or asset. External `http(s)` links and mount prefixes are not checked.

## TAL-LINK-ANCHOR

**a link to a missing anchor on another page**

A link to `other.html#fragment` (or `other.tmd#fragment`) resolves to a real page, but that page has no such `#fragment`.

To fix: Fix the fragment to a real anchor on the target page, or add the id there.

## TAL-MATH

**a math expression could not be rendered**

A `$…$` / `$$…$$` expression did not parse as valid KaTeX, so it cannot be typeset and falls back to raw source.

To fix: Fix the LaTeX at the reported location: balance braces and `\left`/`\right`, and use only macros KaTeX supports.

## TAL-MEDIA

**a local video was not found**

A `{{< video clip.mp4 >}}` (or similar) names a local media file that does not exist relative to the document.

To fix: Correct the path or add the file. Remote media URLs are not checked.

## TAL-REACTIVE

**a broken reactive graph**

An `{js}` reactive cell either reads an input that no cell or `{{< input >}}` defines, or the cells form a dependency cycle so none can run.

To fix: Define the missing input (or fix its name; the message suggests the nearest), or break the cycle so the graph is acyclic.

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

