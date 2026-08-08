# The editor knows the format

Completion comes from the same vocabulary the validator enforces, so it cannot suggest
something `check` will reject:

- `::: {.` — callout kinds and the structural div classes
- `::: {.callout-note ` — the attributes *that class* actually reads
- `$\al` — LaTeX commands, every one of them verified to render through KaTeX
- `#| ` — cell options, and their values
- `[text](`, `bibliography:` — real files on disk

One more worth knowing: **Insert Math Symbol** (`Ctrl+Alt+M`) searches by name, glyph or
category, for the symbols you cannot spell.
