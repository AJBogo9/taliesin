---
name: new-post
description: Scaffold a new blog post under posts/<slug>/ with correct Taliesin frontmatter, a references.bib, and this blog's house conventions. Use when the user wants to start writing a new blog post, add an article, or create a new entry under posts/.
---

# Scaffold a new blog post

Create a new post in this Quarto blog following the established conventions.

## Steps

1. **Determine the slug.** Derive a short kebab-case slug from the topic (e.g. "The Fourier Transform" → `fourier-transform`). Confirm it with the user if ambiguous. The post lives at `posts/<slug>/index.qmd`.

2. **Create `posts/<slug>/index.qmd`** with this frontmatter, filling in real values:

   ```
   ---
   title: "<Title Case Title>"
   date: "YYYY-MM-DD"
   description: "<One sentence, plain language, what the reader will understand by the end.>"
   image: "thumbnail.webp"
   image-alt: "<Descriptive alt text for the thumbnail>"
   bibliography: references.bib
   categories:
     - <category>
   ---
   ```

   - Use today's date unless the user gives one.
   - **Reuse existing categories** for filter consistency. The current pool: `algorithms`, `graph theory`, `probabilistic ML`, `variational inference`, `signal processing`, `information theory`, `statistics`, `hypothesis testing`, `linear algebra`, `machine learning`. Only invent a new category if nothing fits.
   - `image` is almost always `thumbnail.webp` (one post uses a named file). The thumbnail itself is created manually later — do not generate it. Leave a note reminding the user to add it.

3. **Create an empty `posts/<slug>/references.bib`** (BibTeX, IEEE style via `../ieee.csl`). Add entries as the post cites sources.

4. **Write the body** if the user gave enough to start; otherwise leave a one-line stub. Match the house style of existing posts:
   - Open with a concrete, motivating example or question, not a definition.
   - Math uses KaTeX: inline `$...$`, display `$$...$$`. Reference figures with `@fig-label`.
   - Python cells are ```` ```{python} ```` and execute via Jupyter (`echo: true`, so code shows). Interactive visuals use ```` ```{js} ```` cells with the vendored `Plot`/`d3` globals (or `import()` Three.js); a Python cell can bridge values via `ojs_define(...)`.
   - Collapsible notation tables use `::: {.callout-note collapse="true"}`.

## House rules (do not violate)

- **No em dashes or en dashes anywhere.** Use commas, colons, parentheses, or restructure.
- `_metadata.yml` already applies the CC BY license, author, citation, and Google Scholar. Do not repeat those in the post.
- Do not commit. Leave the new files as uncommitted changes for the user to review.
- After scaffolding, remind the user to: add `thumbnail.webp`, then run `quarto preview` to check rendering.
