#!/usr/bin/env python3
"""AP2 fuzzing: generate targeted hostile .tmd cases.

Each case is a named hypothesis about a specific panic/hang in the
parse->render pipeline. Writes files to OUT and a manifest listing
(name, mode) so the driver knows which entry point to feed each into.
"""
import os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "cases"
os.makedirs(OUT, exist_ok=True)

cases = []  # (name, mode, bytes)

def add(name, text, mode="doc"):
    if isinstance(text, str):
        text = text.encode("utf-8")
    cases.append((name, mode, text))

# ---- 1. Unbalanced ::: fences --------------------------------------------
add("fence_open_no_close", "::: {.callout-note}\nbody text\n")
add("fence_close_no_open", "body\n:::\n")
add("fence_only_open", "::::::::::\n")
add("fence_only_close", ":::\n")
add("fence_nested_unclosed", "::: a\n::: b\n::: c\nx\n")
add("fence_mismatched_attr", "::: {.columns}\n::: {.column width=50%}\nx\n")
add("fence_empty_attr", "::: {}\n\n:::\n")
add("fence_attr_no_space", ":::{.note}\nx\n:::\n")
add("fence_bare_colons_wall", ":::\n:::\n:::\n:::\n:::\n:::\n")
add("fence_magic_move_unclosed", "::: {.magic-move}\n```js\na\n```\n")
add("fence_columns_no_columns", "::: {.columns}\n:::\n")
add("fence_step_bad_lines", "::: {.step lines=abc}\nx\n:::\n")
add("fence_step_pipe_lines", "::: {.step lines=1|2|xyz}\nx\n:::\n")
add("fence_columns_ncol_zero", "::: {.columns ncol=0}\nx\n:::\n")
add("fence_columns_ncol_huge", "::: {.columns ncol=999999999}\nx\n:::\n")
add("fence_column_width_bad", "::: {.columns}\n::: {.column width=}\nx\n:::\n:::\n")

# ---- 2. Deep nesting (stack-overflow via recursion) ----------------------
add("nest_div_deep", "".join(f"::: d{i}\n" for i in range(5000)) + "x\n")
add("nest_blockquote_deep", (">" * 20000) + " x\n")
add("nest_list_deep", "".join("  " * i + "- x\n" for i in range(2000)))
add("nest_emphasis_deep", ("*" * 20000) + "x" + ("*" * 20000) + "\n")
add("nest_brackets_deep", ("[" * 20000) + "x" + ("]" * 20000) + "\n")
add("nest_paren_link_deep", "".join("[a](" for _ in range(10000)) + "x")
add("nest_heading_hashes", ("#" * 5000) + " title\n")
add("nest_backtick_wall", ("`" * 20000) + "\n")
add("nest_atx_many", "".join(f"{'#'*((i%6)+1)} h{i}\n\n" for i in range(3000)))

# ---- 3. Garbage YAML front-matter ----------------------------------------
add("yaml_unclosed", "---\ntitle: ok\n\nbody without close\n")
add("yaml_bad_colon", "---\ntitle: ok\nbad: : x\n---\n\nbody\n")
add("yaml_tab_indent", "---\ntitle:\n\tbad: tabbed\n---\n\nbody\n")
add("yaml_billion_laughs",
    "---\n" + "a: &a [x,x,x,x,x,x,x,x,x]\n" +
    "b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\n" +
    "c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\n" +
    "d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\n" +
    "e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\n" +
    "f: [*e,*e,*e,*e,*e,*e,*e,*e,*e]\n---\n\nbody\n")
add("yaml_deep_nest", "---\n" + "".join("  " * i + f"k{i}:\n" for i in range(1000)) + "  " * 1000 + "v: x\n---\n\nb\n")
add("yaml_just_delims", "---\n---\n")
add("yaml_delims_no_body", "---\ntitle: x\n---\n")
add("yaml_only_open_delim", "---\n")
add("yaml_null_title", "---\ntitle: null\n---\n\nb\n")
add("yaml_list_title", "---\ntitle: [a, b, c]\n---\n\nb\n")
add("yaml_map_title", "---\ntitle: {a: 1}\n---\n\nb\n")
add("yaml_huge_scalar", "---\ntitle: " + ("x" * 500000) + "\n---\n\nb\n")
add("yaml_crlf", "---\r\ntitle: ok\r\n---\r\n\r\nbody\r\n")
add("yaml_bom", "﻿---\ntitle: ok\n---\n\nbody\n")
add("yaml_theorems_bad", "---\ntheorems:\n  - kind: 1\n---\n\nbody\n")
add("yaml_format_html", "---\nformat: revealjs\n---\n\nbody\n")

# ---- 4. Pathological Unicode ---------------------------------------------
add("uni_combining_wall", "e" + ("́" * 50000) + "\n")
add("uni_rtl_override", "‮" + "abcdefg" + "‬" + "\n")
add("uni_zero_width", ("​" * 10000) + "\n")
add("uni_zwj_emoji", ("\U0001f469‍\U0001f467" * 5000) + "\n")
add("uni_bidi_mix", "shalom שלום world ‮reversed‬\n")
add("uni_cjk_heading", "# 中文标题 \U0001f600 café\n\nbody\n")
add("uni_control_chars", "".join(chr(c) for c in range(1, 32) if c not in (9, 10, 13)) + "\n")
add("uni_lone_combining", "́̂̃ leading combining\n")
add("uni_nul_byte", "before\x00after\n")
add("uni_high_planes", ("\U0002ffff\U0010ffff" * 1000) + "\n")

# multibyte + sourcepos-sensitive features (prose lint byte-indexing)
add("uni_prose_multibyte",
    "---\nprose-lint:\n  banned: [utilize]\n---\n\ncafé 中文 please utilize this \U0001f600 tail\n")
add("uni_prose_combining",
    "---\nprose-lint:\n  banned: [leverage]\n---\n\né́́ leverage é́ word\n")
add("uni_emphasis_multibyte", "café *中文* and _\U0001f600_ end\n")
add("uni_link_multibyte", "[café中文](http://x/\U0001f600)\n")
add("uni_heading_slug_unicode", "## Café 中文 \U0001f600 !!!\n\n[link](#)\n")

# ---- 5. Truncation --------------------------------------------------------
add("trunc_mid_frontmatter", "---\ntitle: ok\nauthor")
add("trunc_mid_fence", "::: {.callout-note}\nsome text but no")
add("trunc_mid_math_inline", "text $a + b")
add("trunc_mid_math_display", "text $$\\frac{a}{b}")
add("trunc_mid_code", "```python\nprint(1)")
add("trunc_mid_cell", "```{python}\n#| echo: fal")
add("trunc_mid_table", "| a | b |\n| - | - |\n| 1 |")
add("trunc_mid_link", "text [label](http://exa")
add("trunc_mid_image", "![alt text without clos")
add("trunc_mid_footnote", "text[^1]\n\n[^1]: def without")
add("trunc_only_openbrace", "{")
add("trunc_shortcode", "{{< embed ")

# ---- 6. Math edge cases ---------------------------------------------------
add("math_dollar_alone", "$\n")
add("math_double_dollar_alone", "$$\n")
add("math_quad_dollar", "$$$$\n")
add("math_unclosed_display", "$$\n\\frac{1}{0}\n")
add("math_katex_error", "$\\frac{\\unknownmacro}{x}$\n")
add("math_huge_exponent", "$" + "x^" * 5000 + "2$\n")
add("math_deep_braces", "$" + "{" * 10000 + "x" + "}" * 10000 + "$\n")
add("math_backslash_wall", "$" + ("\\" * 20000) + "$\n")
add("math_display_empty", "$$$$\n\ntext\n")
add("math_dollars_in_code", "```\n$$not math$$\n```\n")

# ---- 7. Code / cell options ----------------------------------------------
add("code_unclosed", "```python\nx = 1\n")
add("code_cell_unclosed", "```{python}\nx = 1\n")
add("code_lang_empty", "```{}\nx\n```\n")
add("code_cell_bad_opt", "```{python}\n#| echo:\nx\n```\n")
add("code_cell_lines_bad", "```{python}\n#| code-line-numbers: a-z\nx\n```\n")
add("code_very_long_line", "```\n" + ("a" * 200000) + "\n```\n")
add("code_tabs_wrap", "```python\n" + ("\t" * 500) + "x = 1  # long tabbed line " + ("y" * 300) + "\n```\n")
add("code_fence_tildes", "~~~\ncode\n~~~\n")
add("code_nested_fences", "````\n```\ninner\n```\n````\n")
add("code_lang_unicode", "```中文\nx\n```\n")
add("code_uses_directive", "```{python}\n#| uses: nonexistent\nx\n```\n")

# ---- 8. Tables ------------------------------------------------------------
add("table_ragged", "| a | b | c |\n| - | - |\n| 1 | 2 | 3 | 4 | 5 |\n")
add("table_no_body", "| a | b |\n| - | - |\n")
add("table_huge_cols", "|" + "c|" * 5000 + "\n|" + "-|" * 5000 + "\n|" + "1|" * 5000 + "\n")
add("table_pipe_only", "|\n|\n")
add("table_align_weird", "| a |\n|:-:|\n| x |\n")
add("table_escaped_pipe", "| a \\| b | c |\n| - | - |\n| 1 | 2 |\n")

# ---- 9. Footnotes / refs / cites -----------------------------------------
add("fn_no_def", "text[^missing] more\n")
add("fn_recursive", "a[^1]\n\n[^1]: refers to [^1] itself\n")
add("fn_empty_label", "a[^] b\n")
add("fn_dup_def", "a[^1]\n\n[^1]: one\n[^1]: two\n")
add("cite_empty", "text [@] end\n")
add("cite_dangling", "see [@nonexistent2024] here\n")
add("cite_multi", "see [@a; @b; @c] and @fig-x and @sec-y\n")
add("xref_dangling_fig", "See @fig-nope and @sec-nope and @tbl-nope.\n")
add("xref_self", "## Heading {#sec-a}\n\nSee @sec-a.\n")

# ---- 10. Links / images / shortcodes -------------------------------------
add("link_unclosed", "[label](http://x\n")
add("link_empty_url", "[label]()\n")
add("link_ref_no_def", "[text][undefined]\n")
add("img_no_alt", "![](img.png)\n")
add("img_unclosed", "![alt\n")
add("shortcode_embed_missing", "{{< embed nonexistent.tmd >}}\n")
add("shortcode_video_bad", "{{< video >}}\n")
add("shortcode_unknown", "{{< frobnicate a b c >}}\n")
add("shortcode_unclosed", "{{< embed x.tmd\n")
add("shortcode_nested", "{{< embed {{< video x >}} >}}\n")

# ---- 11. Whitespace / empties / degenerate --------------------------------
add("empty_file", "")
add("single_newline", "\n")
add("only_spaces", "     ")
add("only_tabs", "\t\t\t\t")
add("many_blank_lines", "\n" * 100000)
add("no_trailing_newline", "just one line no newline")
add("single_char", "x")
add("single_hash", "#")
add("single_colon", ":")
add("crlf_everywhere", "line1\r\nline2\r\n\r\nline3\r\n")
add("cr_only", "line1\rline2\rline3")
add("null_only", "\x00")
add("huge_single_line", "a" * 5000000)

# ---- 12. Definition lists / setext / misc markdown edge -------------------
add("setext_heading", "Title\n=====\n\nbody\n")
add("setext_empty", "\n=\n")
add("hr_variants", "---\n\n***\n\n___\n\n- - -\n")
add("html_block_unclosed", "<div>\n<span>\nno close\n")
add("html_comment_unclosed", "<!-- comment never closes\n\nmore\n")
add("raw_html_passthrough", "```{=html}\n<script>alert(1)</script>\n```\n")

# ---- write out -----------------------------------------------------------
manifest = []
for i, (name, mode, data) in enumerate(cases):
    fn = f"{i:03d}_{name}.tmd"
    with open(os.path.join(OUT, fn), "wb") as f:
        f.write(data)
    manifest.append(f"{fn}\t{mode}")

with open(os.path.join(OUT, "MANIFEST.tsv"), "w") as f:
    f.write("\n".join(manifest) + "\n")

print(f"wrote {len(cases)} cases to {OUT}/")
