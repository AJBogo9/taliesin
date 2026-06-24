# Design: P3a — stop `build <site>` leaking residue into `_site/`

Status: approved 2026-06-24. Branch `feat/build-residue-skip`. From `backlog.md` Open/next
(Format & structure audit round 3). Build-time only; zero new dependencies.

## Problem

`mirror_assets`'s `walk()` (`crates/server/src/main.rs`) copies every file not starting
with `_`/`.` into `_site/`. Dotfiles (`.RData`, `.Rproj`, `.gitignore`) are already excluded,
but these leak into the deploy:
- build-cache **dirs** that contain but don't start with `_`: `index_cache/`, `*_files/`
  (knitr/RMarkdown/Quarto artifacts);
- source-only files the rendered HTML never references: `references.bib`, `*.Rproj`.

## Approach (chosen: targeted skip, zero-dep)

In `walk()`, after the existing `_`/`.` skip:
- **Cache dirs:** if `p.is_dir()` and its file name ends with `_cache` or `_files` → skip
  (don't recurse/copy). Collect the skipped dir names.
- **Source-only files:** generalize the current single `.qmd` skip into a small set of
  skipped extensions — `qmd`, `bib`, `Rproj`.
- **Not silent:** `mirror_assets` returns the skipped cache-dir names alongside the copied
  count; `cmd_build`'s caller logs one line (e.g. `skipped 2 build-cache dir(s):
  index_cache, report_files`) when non-empty.

Rejected: honoring `.gitignore` via the `ignore` crate (heavier dependency on a deliberately
lean codebase). Consequence accepted: arbitrary private files that don't start with `_`/`.`
and aren't a cache dir / `.bib` / `.Rproj` (e.g. `notes.md`) are STILL copied — the `_`/`.`
naming convention remains the documented way to mark those private.

## Interface change

`mirror_assets(root, out) -> usize` becomes `-> (usize, Vec<String>)` (copied count, sorted
unique skipped cache-dir names), or keeps `usize` and takes a `&mut Vec<String>` out-param.
Chosen: return a small struct/tuple `(usize, Vec<String>)` to keep the caller simple. The
sole caller (`cmd_build`, `main.rs:361`) destructures it and logs the skipped names.

## Test (TDD, `#[cfg(test)] mod tests` in `main.rs`)

Using the crate's temp-dir convention (`std::env::temp_dir().join(format!("qmd-mirror-{}",
std::process::id()))`, `remove_dir_all` + `create_dir_all`): build a tree —
`keep.png`, `notes.md`, `index_cache/x`, `report_files/y`, `refs.bib`, `_freeze/z`, `.RData`
— run `mirror_assets` to an out dir, then assert:
- COPIED: `keep.png`, `notes.md` (plain non-residue files);
- NOT copied: `index_cache/`, `report_files/` (cache dirs), `refs.bib` (.bib), `_freeze/`
  (underscore), `.RData` (dot);
- the returned skipped list contains `index_cache` and `report_files`.

## Invariants

Build-time only; no change to the block model, render, or the `_`/`.` convention; zero new
dependencies; `.qmd` sources still skipped (rendered separately).

## Out of scope

P3b (`mounts:` ignored by `build`) and P3c (single-doc `build --out` drops `{js}` local
imports) are separate backlog items. Honoring `.gitignore`. Catching arbitrary private files.
