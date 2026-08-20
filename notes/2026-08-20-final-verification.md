# Final verification of the 1.0 tree, 2026-08-20

Every claim here is quoted from the instrument that produced it.

## Gates

```
$ export TALIESIN_PYTHON="$PWD/.venv/bin/python"
$ ./tools/gates.sh
```

```
════ gates ════
  pass     cargo fmt --check
  pass     cargo clippy -D warnings
  pass     cargo test --workspace (both gates)
  pass     tsc: web-client
  pass     tsc: bundled assets JS
  pass     VS Code companion
  pass     cargo audit
  pass     cargo deny check
  pass     build docs/guide --check-only
  pass     build docs/internals --check-only
  pass     tools/publish.sh --check
  pass     portability census --verify

PASSED — every gate ran and passed (12 gates).
```

Exit code 0. `tools/publish.sh --check` and the portability census are both gates, so the
plan's separate Steps 1 and 2 are discharged by this one run.

## Version, pin and census

| Check | Command | Output |
|---|---|---|
| Version | `taliesin --version` | `taliesin 1.0.0 (c7617ac8)` |
| README pin | `cargo test -p taliesin-core --lib the_readme_does_not_advertise_withdrawn_constructs` | `test result: ok. 1 passed; 0 failed` |
| Census | `python3 tools/portability-census.py --verify` | exit 0 |

## Workflows, against the final tree

| Run | Workflow | Conclusion | Duration |
|---|---|---|---|
| 32395506257 | CI | **success** | 7m13s |
| 32395509519 | Release | **success** | 3m30s |

CI's six jobs and Release's four jobs (including both macOS targets) all green. This is the
third consecutive green pair, and the first against the tree that will actually flip.

## Browser verification

Driven through the chrome-devtools MCP against a live `preview` of each project.

| Project | Checked | Result |
|---|---|---|
| `site` | 900x1440 and 390x844, console | Renders correctly at both. Live WebGL hero scene runs. Mobile collapses to a hamburger and a single column. **0 console errors or warnings.** |
| `docs/guide` | 900x1440, all images, console | **0 broken images of 3.** `figures/loss.png` loads at its true 1135x865, which is direct proof the asset rescued in Task 1c renders in a real page. 0 console errors. |
| `gallery` | 900x1440, console | Listing cards render with live thumbnails (report, molecules, gears, descent). **0 console errors.** |
| `docs/internals` | Structure and images | Title, `h1` and 8 nav links correct. **0 broken images of 2.** |

The 900x1440 portrait band, the one the plan calls out as where layout defects show, was
checked on all three projects that have full page chrome.

## What this does NOT establish

Named explicitly, because a verification record that omits its own limits is worth less than
none.

- **The macOS binaries were built but never executed.** The release matrix proves they
  compile, link and package on both Apple targets. It does not prove they run.
- **`release.yml` has never fired on a real `v*` tag.** Both runs were `workflow_dispatch` on
  a branch, so the tag-derived naming path is unexercised and artifacts were named for the
  branch rather than for a version. ~~No GitHub Release has ever been published.~~
  **CORRECTED 2026-08-20: two were.** `rehearse-2` (Latest) and `rehearse-workflows`, each
  with all six expected assets. The create/upload/package path is proven; only the `v*`
  trigger and the tag-derived naming are untested.
- **The four sites were built and previewed, never deployed.** taliesin.sh has no DNS A
  record, and the Cloudflare Pages custom-domain binding is unexercised.
- **The 1440x900 desktop band was not screenshotted** for every project. The design is
  desktop-first and the two harder bands (mobile and portrait) were checked instead.
- **The rewritten history was verified in a clone, not published.** Nothing has been pushed
  to any public repository.
