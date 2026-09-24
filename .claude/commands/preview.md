---
description: Build, serve a .tmd, and verify it live in the browser
argument-hint: "[path/to/file.tmd or project dir] (defaults to docs/guide)"
---
Preview and verify a `.tmd` document end to end. Steps:

1. `cargo build -p taliesin-server` (report any compile error and stop).
2. Pick the target: `$ARGUMENTS` if given, else `docs/guide` (the User Guide book). A
   `.tmd` inside a `_site.yml` project opens the whole project; its page is served at the
   same path with `.html` (for example `docs/guide/using/code.tmd` is `/using/code.html`).
3. Free port 4388 if busy (`fuser -k 4388/tcp`), then start the server detached:
   `./target/debug/taliesin preview <target> 4388` (run_in_background). Wait for HTTP 200
   from `http://127.0.0.1:4388/`.
4. Verify in the browser with the chrome-devtools MCP (`mcp__chrome-devtools__*`, never the
   plugin's twins), or with the `chrome-devtools` CLI when it takes more than two steps:
   open the page, wait about 1.5 s for the client to mount and mermaid diagrams to render,
   then take a viewport screenshot.
5. Report: any console errors, any failed network requests, and any diagnostics the
   preview shows. `{python}` cells need a Python with `ipykernel` (a project `.venv` is
   found on its own, or set `TALIESIN_PYTHON`); without one they render as source and the
   preview shows a "kernel unavailable" diagnostic.
6. Leave the server running so I can keep iterating, unless I ask you to stop it.
