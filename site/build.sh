#!/usr/bin/env bash
#
# Build the marketing site AND every project it `mounts:`, into one deployable tree.
#
# Why this script exists (item 149): `taliesin build site` renders this project's own
# pages and nothing else. `mounts:` is a PREVIEW feature — it serves the sibling books and
# the gallery exhibits live under their URL prefixes so the nav resolves while you work —
# and the static build does not wire it. So a plain `build site` produces a tree whose
# "Guide", "Internals" and every gallery link 404. That is what shipped, because the
# warning `build` printed was one nobody's CI acted on.
#
# ORDER IS LOAD-BEARING. The parent build sweeps stale output: it deletes everything under
# the output directory that it did not itself write (`sweep_stale` in build.rs, which skips
# only dotfiles, `_`-prefixed names and symlinks). `_site/docs/guide/` is none of those, so
# building a mount first and the parent second silently deletes the mount. Parent first,
# mounts second, always — and re-running the parent alone puts you back to the broken tree,
# which is why this script rebuilds the whole thing rather than offering a partial mode.
#
# Nothing here runs `--strict`, deliberately. The parent's mount warnings are exactly the
# condition this script goes on to resolve, and `--strict` counts them (by design — a
# one-shot `build site --strict` really is producing 404s). The mounts are not strict either,
# because their loudest warning is "no kernel was available for its language": whether this
# machine has a python with ipykernel is not a property of the tree being deployed, and a
# deploy script that fails on it would just be run with the flag removed. Linting is
# `taliesin check`'s job; this script's job is a complete tree, and it verifies that directly
# by asserting each mount actually wrote an index.
#
# Usage:  ./site/build.sh [OUT]        # OUT defaults to _site at the repo root
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
out="${1:-$repo/_site}"
# `taliesin` on PATH is the auto-rebuilding launcher; fall back to a release build in-tree.
tali="$(command -v taliesin || echo "$repo/target/release/taliesin")"

# Every `mounts:` entry in site/_site.yml, as "<at> <path-relative-to-site/>" pairs.
# `mounts_match_the_site_config` (crates/server/tests/site_build_script.rs) fails the suite
# if this list and `_site.yml` disagree, so adding a mount without adding it here is caught.
mounts=(
  "docs/guide         ../docs/guide"
  "docs/internals     ../docs/internals"
  "gallery/course     ../corpus/course"
  "gallery/tarn       ../corpus/tarn"
  "gallery/descent    ../corpus/descent"
  "gallery/graphics3d ../corpus/graphics3d"
  "gallery/analyst    ../corpus/analyst"
)

echo "==> site -> $out"
"$tali" build "$here" --out "$out"

for entry in "${mounts[@]}"; do
  read -r at path <<<"$entry"
  echo "==> $at -> $out/$at"
  "$tali" build "$here/$path" --out "$out/$at"
  # The check that matters for this script: the prefix the nav links to now has a page
  # behind it. A mount that built nothing is the 404 this whole script exists to prevent.
  [ -f "$out/$at/index.html" ] || { echo "FAILED: $at built no index.html" >&2; exit 1; }
done

echo
echo "built $((${#mounts[@]} + 1)) projects into $out"
echo "note: re-running \`taliesin build site\` alone will sweep the mounts back out."
