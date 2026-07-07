# Theming Taliesin

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

## Mermaid diagrams

Mermaid bakes its colours into the SVG when it renders, so they can't be restyled
with ordinary CSS. Instead, Taliesin reads the diagram config from CSS variables
(re-rendering on a light/dark switch), so you can theme diagrams from your theme
file with no JavaScript:

| Variable | Role | Default |
|---|---|---|
| `--qmd-mermaid-theme` | mermaid theme name (`default`, `dark`, `neutral`, `forest`, `base`) | `dark` in dark mode, else `default` |
| `--qmd-mermaid-bg` | diagram background | mermaid theme's |
| `--qmd-mermaid-node` | node fill | mermaid theme's |
| `--qmd-mermaid-node-border` | node border | mermaid theme's |
| `--qmd-mermaid-text` | node text | mermaid theme's |
| `--qmd-mermaid-line` | edges / arrows | mermaid theme's |

For full colour control, set `--qmd-mermaid-theme: base` and the colour variables
(mermaid's `base` theme is the one built to be customised). To match the rest of
your palette, point them at your other variables:

```css
html[data-theme="dark"] {
  --qmd-mermaid-theme: base;
  --qmd-mermaid-bg: var(--qmd-bg);
  --qmd-mermaid-node: var(--qmd-code-bg);
  --qmd-mermaid-node-border: var(--qmd-border);
  --qmd-mermaid-text: var(--qmd-fg);
  --qmd-mermaid-line: var(--qmd-muted);
}
```

Set nothing and diagrams just follow the built-in light/dark themes. The zoom
(click-to-enlarge) backdrop uses `--qmd-bg`, so an enlarged diagram matches.

## Sharing a theme

Drop it at `_extensions/<name>/theme.css` in your project and reference it with
`theme: <name>`. Bundling that `_extensions/<name>/` in a git repo makes it
installable by others (a `Taliesin add <repo>` fetcher is planned; until then,
`git clone` into `_extensions/`).
