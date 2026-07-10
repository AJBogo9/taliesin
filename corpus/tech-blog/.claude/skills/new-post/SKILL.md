---
name: new-post
description: Scaffold a new blog post under posts/<slug>/ with correct Taliesin frontmatter, a references.bib, and this blog's house conventions. Use when the user wants to start writing a new blog post, add an article, or create a new entry under posts/.
---

# Scaffold a new blog post

Create a new post in this Taliesin blog following the established conventions.

The tool owns the scaffold now; this skill owns only what the tool cannot know, which is
this blog's house style.

## Steps

1. **Determine the slug.** Derive a short kebab-case slug from the topic (e.g. "The Fourier Transform" -> `fourier-transform`). Confirm it with the user if ambiguous.

2. **Run `taliesin new post <slug>`.** It writes `posts/<slug>/index.tmd`, dated today, with front-matter keys the validator accepts by construction, and refuses to overwrite an existing file. Do not hand-write the front matter: a scaffold typed from memory is how `format:` blocks and misspelled keys get in.

3. **Add this blog's keys** to the front matter the command wrote:

   ```
   image: "thumbnail.webp"
   image-alt: "<Descriptive alt text for the thumbnail>"
   bibliography: references.bib
   ```

   - **Reuse existing categories** for filter consistency. The current pool: `algorithms`, `graph theory`, `probabilistic ML`, `variational inference`, `signal processing`, `information theory`, `statistics`, `hypothesis testing`, `linear algebra`, `machine learning`. Only invent a new category if nothing fits. (A near-miss category is caught by `taliesin check`, but a plausible-looking new one is not.)
   - `image` is almost always `thumbnail.webp` (one post uses a named file). The thumbnail itself is created manually later, so do not generate it. Leave a note reminding the user to add it.

4. **Create an empty `posts/<slug>/references.bib`** (BibTeX, IEEE style via `../ieee.csl`). Add entries as the post cites sources.

5. **Write the body** if the user gave enough to start; otherwise leave the stub the command wrote. Match the house style of existing posts:
   - Open with a concrete, motivating example or question, not a definition.
   - Math uses KaTeX: inline `$...$`, display `$$...$$`. Reference figures with `@fig-label`.
   - Python cells are ```` ```{python} ```` and execute via Jupyter (`echo: true`, so code shows). Interactive visuals use ```` ```{js} ```` cells with the vendored `Plot`/`d3` globals (or `import()` Three.js); a Python cell can bridge values to `{js}` with `define(name = value)`.
   - Collapsible notation tables use `::: {.callout-note collapse="true"}`.

## House rules (do not violate)

- **No em dashes or en dashes anywhere.** Use commas, colons, parentheses, or restructure.
- `_metadata.yml` already applies the CC BY license, author, citation, and Google Scholar. Do not repeat those in the post.
- Do not commit. Leave the new files as uncommitted changes for the user to review.
- After scaffolding, remind the user to add `thumbnail.webp`, then run `taliesin preview .` to check rendering and `taliesin check .` to catch a broken reference before it ships.
