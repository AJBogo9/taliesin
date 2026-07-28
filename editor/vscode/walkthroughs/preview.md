# The preview is a view, not an editor

Press `Ctrl+Shift+K` (`Cmd+Shift+K` on macOS) on any `.tmd` file to open a live preview
beside it. Edits flow one way: you change the source, the preview re-renders the block you
touched — not the whole page, and without restarting anything.

The bridge back is **Ctrl+click**: click a block in the preview and the editor jumps to the
line that produced it. The preview never writes to your file.
