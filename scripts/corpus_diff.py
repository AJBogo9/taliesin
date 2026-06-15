#!/usr/bin/env python3
"""Structural diff of qmd-fast render output against the Quarto baseline.

The baselines in corpus/expected/*.html are Quarto's HTML, kept as a STRUCTURAL
reference, not a byte-exact oracle (see corpus/README.md). Quarto wraps content
in nav/sidebar/TOC chrome and vendored-lib classes; qmd-fast emits a lean body
of blocks. A raw `diff` is therefore useless noise. This script reduces each
side to a normalized skeleton of content-bearing block elements and diffs those,
so what you see is real structural divergence (a missing section, a wrong
element type, an absent References list), not whitespace or class-name cosmetics.

Usage:
    python3 scripts/corpus_diff.py <doc-key>     # one doc
    python3 scripts/corpus_diff.py --all         # every doc, summary only
    python3 scripts/corpus_diff.py --list        # show doc keys

Doc keys: born-machines, em-algorithm, pca-geometry, liquid-glass, bayesian-book
"""

import sys
import re
import subprocess
import difflib
from pathlib import Path

try:
    from bs4 import BeautifulSoup, Tag
except ImportError:
    sys.exit("error: BeautifulSoup (bs4) is required: pip install beautifulsoup4")

ROOT = Path(__file__).resolve().parent.parent

# The corpus is the spec and finite, and the qmd->html mapping is not derivable
# from the stem (example.qmd -> liquid-glass.html), so it is listed explicitly.
DOCS = {
    "born-machines": "corpus/posts/born-machines.qmd",
    "em-algorithm":  "corpus/posts/em-algorithm/index.qmd",
    "pca-geometry":  "corpus/posts/pca-geometry/index.qmd",
    "liquid-glass":  "corpus/liquid-glass-slides/example.qmd",
    "bayesian-book": "corpus/bayesian-book/index.qmd",
}

# Block elements that carry text we want to compare directly.
LEAF_TAGS = {"h1", "h2", "h3", "h4", "h5", "h6", "p", "li", "td", "th",
             "dt", "dd", "figcaption", "pre", "blockquote"}
# Structural containers worth a marker line (children supply the text).
CONTAINER_TAGS = {"ul", "ol", "table", "thead", "tbody", "tr", "figure", "dl"}
VOID_TAGS = {"hr"}
# A div/section is transparent (recurse, no line) unless it carries one of these
# semantic markers — that is the signal we care about when matching Quarto.
KEEP_CLASS = re.compile(r"callout|column|references|csl-entry|footnote|"
                        r"layout|theorem|proof|definition|panel-tabset")


def render_qmd_fast(qmd: Path) -> str:
    binary = ROOT / "target" / "debug" / "qmd-fast"
    if binary.exists():
        cmd = [str(binary), "render", str(qmd)]
    else:
        cmd = ["cargo", "run", "-q", "-p", "qmd-fast-server", "--", "render", str(qmd)]
    out = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"error rendering {qmd}:\n{out.stderr}")
    return out.stdout


def content_root(soup: BeautifulSoup) -> Tag:
    # Quarto: real content lives in main#quarto-document-content; qmd-fast has
    # no wrapper, so fall back to <body>.
    for sel in ("main#quarto-document-content", "main.content", "main"):
        node = soup.select_one(sel)
        if node:
            return node
    return soup.body or soup


def kept_classes(tag: Tag) -> str:
    classes = [c for c in tag.get("class", []) if KEEP_CLASS.search(c)]
    return "." + ".".join(sorted(set(classes))) if classes else ""


def norm_text(tag: Tag) -> str:
    txt = tag.get_text(" ", strip=True)
    txt = re.sub(r"\s+", " ", txt)
    txt = txt.replace("¶", "").strip()  # drop Quarto heading anchor pilcrows
    return txt[:80]


def skeleton(root: Tag) -> list[str]:
    """Depth-first list of normalized block markers."""
    lines: list[str] = []

    def walk(node: Tag, depth: int):
        for child in node.children:
            if not isinstance(child, Tag):
                continue
            tag = child.name
            if tag in ("script", "style", "head", "nav"):
                continue
            indent = "  " * depth
            if tag in VOID_TAGS:
                lines.append(f"{indent}{tag}")
            elif tag in LEAF_TAGS:
                lines.append(f"{indent}{tag}{kept_classes(child)}: {norm_text(child)}")
                # don't recurse into leaf text (pre/li children are part of it)
            elif tag in CONTAINER_TAGS:
                lines.append(f"{indent}{tag}{kept_classes(child)}")
                walk(child, depth + 1)
            elif tag in ("div", "section"):
                kc = kept_classes(child)
                if kc:  # semantic block -> marker + recurse
                    lines.append(f"{indent}{tag}{kc}")
                    walk(child, depth + 1)
                else:    # transparent wrapper -> recurse only
                    walk(child, depth)
            else:
                walk(child, depth)

    walk(root, 0)
    return lines


def counts(lines: list[str]) -> dict:
    def starts(pat):
        return sum(1 for ln in lines if re.match(rf"\s*{pat}\b", ln))
    return {
        "headings":   starts(r"h[1-6]"),
        "paragraphs": starts("p"),
        "code blocks": starts("pre"),
        "lists":      starts("(ul|ol)"),
        "tables":     starts("table"),
        "figures":    starts("figure"),
        "callouts":   sum(1 for ln in lines if "callout" in ln),
        "references": sum(1 for ln in lines if "references" in ln or "csl-entry" in ln),
    }


def summary_table(qf: dict, qt: dict) -> str:
    rows = ["  element        qmd-fast   quarto   ",
            "  -------------  ---------  -------  "]
    for k in qf:
        flag = "" if qf[k] == qt[k] else "  <-- differs"
        rows.append(f"  {k:<13}  {qf[k]:>9}  {qt[k]:>7}{flag}")
    return "\n".join(rows)


def diff_one(key: str, summary_only: bool = False) -> bool:
    qmd = ROOT / DOCS[key]
    expected = ROOT / "corpus" / "expected" / f"{key}.html"
    if not expected.exists():
        print(f"[{key}] no baseline at {expected.relative_to(ROOT)}, skipping")
        return True

    qf_html = render_qmd_fast(qmd)
    qf_lines = skeleton(content_root(BeautifulSoup(qf_html, "html.parser")))
    qt_lines = skeleton(content_root(BeautifulSoup(expected.read_text(), "html.parser")))

    qf_c, qt_c = counts(qf_lines), counts(qt_lines)
    aligned = qf_c == qt_c and qf_lines == qt_lines

    print(f"\n=== {key} ===")
    print(summary_table(qf_c, qt_c))

    if not summary_only:
        diff = list(difflib.unified_diff(
            qt_lines, qf_lines, fromfile="quarto (expected)",
            tofile="qmd-fast", lineterm=""))
        if diff:
            print("\n  structural diff (- quarto / + qmd-fast):")
            for ln in diff:
                print("  " + ln)
        else:
            print("\n  structure matches the baseline skeleton.")
    return aligned


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    if argv[0] == "--list":
        for k, v in DOCS.items():
            print(f"  {k:<14} {v}")
        return 0
    if argv[0] == "--all":
        # list comp, not all(generator): we want every doc evaluated, not a
        # short-circuit on the first one that diverges.
        results = [diff_one(k, summary_only=True) for k in DOCS]
        return 0 if all(results) else 1
    key = argv[0]
    if key not in DOCS:
        sys.exit(f"unknown doc '{key}'. Keys: {', '.join(DOCS)}")
    return 0 if diff_one(key) else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
