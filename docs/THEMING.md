# Theming qmd-fast

A theme is just CSS that overrides a handful of custom properties. There are two
built-in themes (`light`, the default, and `dark`); everything else is a file or
an installed extension you write yourself.

## Using a theme

```yaml
---
theme: dark            # a built-in (light | dark)
# theme: ./my-theme.css  # a custom file, relative to the document
# theme: my-theme        # an installed _extensions/my-theme/theme.css
---
```

`theme:` resolves in that order: built-in name → a `.css`/`.scss` path → an
`_extensions/<name>/theme.css` bundle. A list (`theme: [dark, custom.scss]`,
Quarto's form) uses the first entry as the base.

## Writing a theme

A theme overrides these CSS variables (their defaults are the light theme):

| Variable | Default | Role |
|---|---|---|
| `--qmd-bg` | `#ffffff` | page background |
| `--qmd-fg` | `#1a1a1a` | body text |
| `--qmd-muted` | `#555` | captions, TOC, blockquotes |
| `--qmd-accent` | `#4c8dff` | highlight outline / callout accents |
| `--qmd-link` | `#2563eb` | links |
| `--qmd-code-bg` | `#f5f5f5` | code block background |
| `--qmd-border` | `#e3e3e3` | table / blockquote borders |
| `--qmd-font-body` | serif stack | body font (`size/line-height family`) |
| `--qmd-font-head` | sans stack | heading font |
| `--qmd-font-mono` | mono stack | code font |
| `--qmd-maxw` | `46rem` | content width |

A minimal theme is one `:root` block:

```css
/* my-theme.css */
:root {
  --qmd-bg: #0d1117;
  --qmd-fg: #e6edf3;
  --qmd-accent: #f78166;
  --qmd-link: #58a6ff;
  --qmd-code-bg: #161b22;
  --qmd-border: #30363d;
}
```

Need more than variables (e.g. dark syntax-highlight colours, callout tints)? Add
ordinary CSS rules after the `:root` block, the theme is loaded last, so it wins.
The built-in **dark** theme (in `crates/core/src/render.rs`, `THEME_DARK`) is the
reference: copy it and change the values.

## Sharing a theme

Drop it at `_extensions/<name>/theme.css` in your project and reference it with
`theme: <name>`. Bundling that `_extensions/<name>/` in a git repo makes it
installable by others (a `qmd-fast add <repo>` fetcher is planned; until then,
`git clone` into `_extensions/`).
