# Vendored assets (offline)

These were previously fetched from a CDN at render time; they are now vendored so
the corpus deck renders fully offline (qmd-fast's "bundled offline" principle).

- **`fonts/inter-*.woff2`** — Inter (latin subset), weights 300/400/500/600.
  SIL Open Font License 1.1. <https://github.com/rsms/inter>. Replaces the former
  `@import url('https://fonts.googleapis.com/css2?family=Inter…')`.
- **`assets/bg-default.jpg`** — background photo (1280px), Unsplash License.
  Source: <https://unsplash.com/photos/photo-1464822759023-fed622ff2c3b>. Replaces
  the former `url('https://images.unsplash.com/photo-1464822759023…')` in both the
  theme CSS default and `example.qmd`'s `include-in-header` override (now removed).
