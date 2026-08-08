# Is the binary there?

The companion is a thin client. Every language feature you see — completion, diagnostics,
hover, go-to-definition, folding, the Run-Cell lens, is answered by `taliesin lsp`, so the
extension needs the `taliesin` binary on your `PATH`.

Run **Taliesin: Diagnose Setup** to see what it found: the binary, its version, and which
A Jupyter kernel is available for `{python}` cells.

If it is not found, set `taliesin.path` in your **user** settings. It is machine-scoped on
purpose: a repository you open cannot redirect the binary this extension executes.
