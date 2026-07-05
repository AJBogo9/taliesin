---
name: new-project
description: Scaffold a new project page under projects/<slug>/ with correct Taliesin frontmatter and this blog's project-page conventions. Use when the user wants to add a project, document something they built, or create a new entry under projects/.
---

# Scaffold a new project page

Create a new entry in the Projects listing. Projects differ from blog posts: they document something the user built (what it does, how it works, takeaways) and usually link to a GitHub repo.

## Steps

1. **Determine the slug.** Short kebab-case, matching the project/repo name (e.g. `supercollider-mcp`). The page lives at `projects/<slug>/index.qmd`.

2. **Create `projects/<slug>/index.qmd`** with this frontmatter:

   ```
   ---
   title: "<project-name>"
   date: "YYYY-MM-DD"
   description: "<One sentence on what it is and why it's interesting.>"
   image: "thumbnail.png"
   image-alt: "<Descriptive alt text for the thumbnail>"
   categories:
     - <tech tag>
   ---

   [GitHub](https://github.com/AJBogo9/<repo>){.btn .btn-outline-secondary .btn-sm}
   ```

   - Use today's date unless told otherwise. The listing sorts by `date desc`.
   - Categories here are tech/tooling tags (e.g. `python`, `MCP`, `music`), not the academic-topic tags used by blog posts.
   - `projects/_metadata.yml` only sets the author; no bibliography by default. Add `bibliography: references.bib` only if the project page cites sources.
   - The thumbnail (`thumbnail.png`) is created manually later — do not generate it; remind the user to add it.

3. **Suggested structure** for the body (adapt to the project): the problem it solves, how it works, what was built, what the user took away. Concrete and first-person, matching the existing project page.

## House rules (do not violate)

- **No em dashes or en dashes anywhere.**
- Do not commit. Leave the new files as uncommitted changes for review.
- After scaffolding, remind the user to add `thumbnail.png` and run `quarto preview`.
