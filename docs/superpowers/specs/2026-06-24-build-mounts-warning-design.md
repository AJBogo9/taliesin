# Design: P3b — `build <site>` warns about unwired `mounts:`

Status: approved 2026-06-24. Branch `feat/build-mounts-warning`. From `backlog.md` Open/next
(Format & structure audit round 3). Build-time only; zero new dependencies.

## Problem

`build_site_async` (`crates/server/src/main.rs`) never reads `site.config.mounts`, so a site
that previews with working `/docs/*` nav links (the `mounts:` feature) deploys with those
links 404'ing — silently. The static build can't auto-mount yet; the agreed first step is to
warn loudly with the exact command to build each mount.

## Fix (`crates/server/src/main.rs`)

- New pure `mount_warnings(mounts: &[qmd_fast_core::site::Mount], root: &Path, out: &Path)
  -> Vec<String>`: one line per mount —
  `mount '/<at>/' is preview-only and not in the static build (its links will 404). Build it:
  qmd-fast build <root.join(path)> --out <out.join(at)>`. Empty slice → empty Vec.
- `build_site_async` logs each via `log::warn` right after `out` is resolved (the in-place
  guard already passed) — "warn first," before the page build, so it's visible.
- Scope = **warn only**. Auto-building each mount into `<out>/<at>/` is explicitly deferred.

## Docs

`docs/internals/sites.qmd` "## Mounts" currently implies the deployed `build` produces the
mount ("a mounted page is rendered on request as static output, mirroring exactly what the
deployed `build` produces"). Correct it: in `preview` mounts are served live; the static
`build` does NOT auto-wire mounts — build each separately into `<out>/<at>/` (the build warns
with the command).

## Test (TDD, `#[cfg(test)]` in `main.rs`)

`mount_warnings(&[Mount { at: "docs".into(), path: "../docs".into() }], root, out)` returns
exactly one string containing `docs`, the word `build`, and the resolved `--out` path
(`out.join("docs")`); `mount_warnings(&[], …)` returns an empty Vec. (`Mount` has pub fields,
constructible in-test.)

## Invariants

Build-time only; no change to render/exec or preview's mount handling; zero new deps;
per-mount warning, never a silent omission.

## Out of scope

Auto-building mounts into the static output (deferred). P3a (residue) and P3c (js imports)
are separate, already shipped.
