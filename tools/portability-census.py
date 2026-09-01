#!/usr/bin/env python3
"""Census the corpus for constructs beyond plain CommonMark.

This produces the "your source stays yours" number in README.md and
docs/guide/using/choosing.tmd. It exists because that claim was published as
"measured rather than asserted" while the script that measured it was never
committed — so by 2026-08-03 the documents/lines figures no longer reproduced and
nothing could tell you whether the percentage had drifted with them.

Run it from the repository root:

    python3 tools/portability-census.py

Family definitions, which are the whole argument and are therefore explicit:

  Fenced div            a line opening or closing a `:::` block.
  Attribute block       a Pandoc `{...}` carrying `#id` or `.class`, anchored at end
                        of line and preceded by `)`, a backtick, or whitespace. The
                        anchor matters: without it, JavaScript object literals and
                        Python f-strings inside `{js}`/`{python}` cells match, which
                        over-counted this family by ~2.9x when measured 2026-08-03.
  Executable fence      a fence opening with a `{lang}` attribute.
  Cell option           a `#|` line, counted only inside a fence.
  Citation / cross-ref  `[@key]`, or a bare `@fig-`/`@sec-`/`@tbl-`/... reference.
  Shortcode             `{{< ... >}}`.

Lines inside fenced code are excluded from every prose family, because a construct
is only Pandoc vocabulary if it is not inside a code block. A line carrying two
different families counts once in the total and once in each family, so the family
rows sum to more than the total.
"""

import collections
import pathlib
import re
import sys

FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
EXEC = re.compile(r"^\s*(?:`{3,}|~{3,})\{")
DIV = re.compile(r"^\s*:::")
OPT = re.compile(r"^\s*#\|\s")
SHORT = re.compile(r"\{\{<.*?>\}\}")
ATTR = re.compile(r"(?:\)|`|^|\s)\{[^{}\n]*(?:#[\w-]+|\.[\w-]+)[^{}\n]*\}\s*$")
CITE = re.compile(
    r"\[@[A-Za-z0-9_:.-]+\]"
    r"|(?<![\w/])@(?:fig|sec|tbl|eq|lst|thm|lem|cor|prp|def|exm|rem)-[A-Za-z0-9_-]+"
)

ORDER = [
    "Fenced div",
    "Attribute block",
    "Executable fence",
    "Cell option",
    "Citation / cross-reference",
    "Shortcode",
]


def census(root: pathlib.Path) -> tuple[int, int, int, collections.Counter]:
    counts: collections.Counter = collections.Counter()
    docs = lines = beyond = 0
    for path in sorted(root.rglob("*.tmd")):
        docs += 1
        in_fence = False
        tok = ""
        for line in path.read_text(encoding="utf-8", errors="replace").split("\n"):
            lines += 1
            fams = set()
            m = FENCE.match(line)
            if m and not in_fence:
                in_fence, tok = True, m.group(1)[:3]
                if EXEC.match(line):
                    fams.add("Executable fence")
            elif m and in_fence and line.strip().startswith(tok):
                in_fence = False
            elif in_fence:
                if OPT.match(line):
                    fams.add("Cell option")
            else:
                if DIV.match(line):
                    fams.add("Fenced div")
                elif ATTR.search(line):
                    fams.add("Attribute block")
                if SHORT.search(line):
                    fams.add("Shortcode")
                if CITE.search(line):
                    fams.add("Citation / cross-reference")
            for fam in fams:
                counts[fam] += 1
            if fams:
                beyond += 1
    return docs, lines, beyond, counts


CHOOSING = "docs/guide/using/choosing.tmd"
README = "README.md"


def verify(docs: int, lines: int, beyond: int, counts: collections.Counter) -> int:
    """Assert the published prose still says what the instrument measures.

    A doc-comment is not a control. This file's own header records the first time these
    numbers rotted, and names committing the script as the remedy; six days later they
    were wrong again — 82/7157/7.0% measured against a published 133/11,534/7.1%, with a
    six-row table in which every cell was wrong and the largest family misidentified. The
    page hands the reader this exact command, so a mismatch is self-refuting rather than
    merely stale.

    The rows are matched on the NUMBER PAIR, never on the family name, so the table's
    keeper text (the `(`::: {.callout-note}`)` parentheticals) and its sort order are free
    to change without loosening the check.

    Commas are stripped from the prose before matching, so `11,534` and `11534` are the
    same token: the published figures are thousands-separated and the instrument's are not.
    """
    bad: list[str] = []
    ch = pathlib.Path(CHOOSING).read_text(encoding="utf-8").replace(",", "")
    rd = pathlib.Path(README).read_text(encoding="utf-8").replace(",", "")
    pct = beyond / lines * 100
    for name in ORDER:
        n = counts[name]
        share = f"{n / lines * 100:.1f}%"
        if not any(f"| {n} |" in ln and f"| {share} |" in ln for ln in ch.split("\n")):
            bad.append(f"{CHOOSING} has no table row for {name}: {n} / {share}")
    # The complement is published too (":18" and ":137"), and it is the half that drifted
    # into three mutually inconsistent values inside one file.
    for tok in (str(docs), str(lines), str(beyond), f"{pct:.1f}%", f"{100 - pct:.1f}%"):
        if tok not in ch:
            bad.append(f"{CHOOSING} is missing {tok}")
    for tok in (str(docs), str(lines), f"{pct:.1f}%"):
        if tok not in rd:
            bad.append(f"{README} is missing {tok}")
    # Absence as well as presence. A stale percentage BESIDE the current one is the rot
    # the presence checks cannot see: the page then carries two figures and nothing tells
    # the reader which one the instrument stands behind ("the 6.8%" sat twenty lines
    # under the gated 6.7% exactly this way). So every percentage token in the page must
    # be one the census computes NOW: the total, its complement, or a family share.
    current = {f"{pct:.1f}%", f"{100 - pct:.1f}%"}
    current.update(f"{counts[name] / lines * 100:.1f}%" for name in ORDER)
    for tok in re.findall(r"\d+(?:\.\d+)?%", ch):
        if tok not in current:
            bad.append(
                f"{CHOOSING} carries the percentage {tok}, which the census does not "
                "compute today: a stale or foreign figure"
            )
    for line in bad:
        print(line, file=sys.stderr)
    if bad:
        print(
            f"\n{len(bad)} published figure(s) disagree with the census. "
            "Re-run `python3 tools/portability-census.py` and copy its output into "
            f"{README} and {CHOOSING}.",
            file=sys.stderr,
        )
        return 1
    return 0


def main() -> int:
    argv = [a for a in sys.argv[1:] if a != "--verify"]
    verify_only = "--verify" in sys.argv
    root = pathlib.Path(argv[0] if argv else "corpus")
    if not root.is_dir():
        print(f"no such directory: {root}", file=sys.stderr)
        return 1
    docs, lines, beyond, counts = census(root)
    if not lines:
        print(f"no .tmd documents under {root}", file=sys.stderr)
        return 1
    if verify_only:
        return verify(docs, lines, beyond, counts)
    print(f"{docs} documents, {lines} lines under {root}/")
    print(f"{beyond} lines ({beyond / lines * 100:.1f}%) carry a construct beyond CommonMark")
    print()
    print("| Construct | Lines | Share |")
    print("|---|---:|---:|")
    # Descending by count, so the printed table is in the same order as the published one
    # and a reader running this command can read down both at once. Sorted rather than
    # emitted in `ORDER`, because the ranking moves: the largest family was the fenced div
    # when this was first published and is the attribute block now, and a hand-sorted
    # literal would have to be re-sorted by hand every time that happened. `ORDER` is the
    # SET of families and the tie-break, nothing more.
    for name in sorted(ORDER, key=lambda n: (-counts[n], ORDER.index(n))):
        n = counts[name]
        print(f"| {name} | {n} | {n / lines * 100:.1f}% |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
