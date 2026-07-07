---
description: Build, serve a .qmd, and verify it live in the browser
argument-hint: "[path/to/file.qmd] (defaults to docs/index.qmd)"
---
Preview and verify a `.qmd` document end to end. Steps:

1. `cargo build -p taliesin-server` (report any compile error and stop).
2. Pick the target file: `$ARGUMENTS` if given, else `docs/index.qmd`.
3. Free port 4388 if busy (`fuser -k 4388/tcp`), then start the server detached:
   `./target/debug/taliesin serve <file> 4388` (run_in_background). Wait for HTTP 200.
4. In chrome-devtools: open `http://127.0.0.1:4388/`, wait ~1.5s for the client to
   mount and lazy assets (mermaid/OJS) to run, then take a viewport screenshot.
5. Report: any console errors, any failed network requests, and any diagnostics
   banner shown in the preview. Note that code cells need a kernel
   (`TALIESIN_PYTHON`); without one they render as source and the diagnostics banner
   will say so.
6. Leave the server running so I can keep iterating, unless I ask you to stop it.
