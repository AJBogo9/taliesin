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


def main() -> int:
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "corpus")
    if not root.is_dir():
        print(f"no such directory: {root}", file=sys.stderr)
        return 1
    docs, lines, beyond, counts = census(root)
    if not lines:
        print(f"no .tmd documents under {root}", file=sys.stderr)
        return 1
    print(f"{docs} documents, {lines} lines under {root}/")
    print(f"{beyond} lines ({beyond / lines * 100:.1f}%) carry a construct beyond CommonMark")
    print()
    print("| Construct | Lines | Share |")
    print("|---|---:|---:|")
    for name in ORDER:
        n = counts[name]
        print(f"| {name} | {n} | {n / lines * 100:.1f}% |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
