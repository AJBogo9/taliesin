#!/usr/bin/env bash
# Build the composed taliesin.sh deploy: the marketing site, with the two docs books and
# the three gallery exhibits written underneath it.
#
# This script replaces the `mounts:` key that `site/_site.yml` used to carry (cut
# 2026-08-09). That key made `preview` serve the sub-projects under a URL prefix and made
# `build` recurse into them, which cost a config vocabulary, a routing layer in the dev
# server and a cycle guard in the build, all to compose one deploy. The composition is
# release infrastructure, so it lives here.
#
# THE ONE THING THAT MUST NOT ROT. build.rs used to record that the shell-script
# alternative had already failed once in production: this project's own site shipped with
# its primary call-to-action 404ing, because the script was not run. So this script does
# not merely build; it VERIFIES, and `--check` is wired into .githooks/pre-push. Every
# cross-project link written in site/ is resolved against the composed output, and a link
# with nothing behind it fails the script (and the push).
#
#   tools/build-site.sh            # the real deploy -> site/_site (code cells execute)
#   tools/build-site.sh --check    # the gate: --no-exec, into a temp dir, links asserted
#
# Override the binary with TALIESIN=... (default: cargo run -q -p taliesin-server --).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

check_only=0
if [ "${1-}" = "--check" ]; then
    check_only=1
elif [ $# -gt 0 ]; then
    echo "usage: tools/build-site.sh [--check]" >&2
    exit 2
fi

TALIESIN=${TALIESIN:-cargo run -q -p taliesin-server --}

# Sub-project -> URL prefix under the site root. This list IS the former `mounts:` block;
# site/_site.yml's nav, the gallery cards and the index hero all link into these prefixes.
subprojects=(
    "docs/guide:docs/guide"
    "docs/internals:docs/internals"
    "corpus/tarn:gallery/tarn"
    "corpus/descent:gallery/descent"
    "corpus/analyst:gallery/analyst"
)

if [ "$check_only" -eq 1 ]; then
    out=$(mktemp -d -t tali-site-check-XXXXXX)
    trap 'rm -rf "$out"' EXIT
    extra=(--no-exec)
    echo "build-site: checking the composed deploy (--no-exec) in $out"
else
    out="$PWD/site/_site"
    extra=()
    echo "build-site: building the composed deploy into $out"
fi

# The parent FIRST, always. `build` sweeps files under its output that it did not write,
# so a sub-project built before the site would be swept away by the site's own build.
#
# `build site` warns "broken link" once per cross-project link below: from site/'s own point
# of view those targets are no page of this project, and that is true — nothing composes them
# but this script. The link assertion at the bottom is what actually decides, against the
# composed output rather than against one project's page registry.
echo "build-site: (the site's own 'broken link' warnings for docs/ and gallery/ are expected;"
echo "            this script resolves those links against the composed output below)"
$TALIESIN build site --out "$out" "${extra[@]}"
for entry in "${subprojects[@]}"; do
    src=${entry%%:*}
    prefix=${entry#*:}
    $TALIESIN build "$src" --out "$out/$prefix" "${extra[@]}"
done

# ---------------------------------------------------------------------------
# The verification the config key used to give for free. Every link written in site/ that
# points into a sub-project prefix is resolved against the composed output. Directory
# links (and extensionless ones) resolve to index.html.
# ---------------------------------------------------------------------------
missing=()
targets=$(grep -rhoE '(\(|href: ")(docs|gallery)/[A-Za-z0-9._/-]*' site/_site.yml site/*.tmd |
    sed -E 's/^(\(|href: ")//' | sort -u)
for t in $targets; do
    candidate="$out/$t"
    case "$t" in
    */) candidate="$out/${t}index.html" ;;
    *.*) ;;
    *) candidate="$out/$t/index.html" ;;
    esac
    [ -f "$candidate" ] || missing+=("$t -> ${candidate#"$out"/}")
done
if [ ${#missing[@]} -gt 0 ]; then
    echo "build-site: REFUSED. ${#missing[@]} cross-project link(s) have nothing behind them:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    exit 1
fi

n=$(printf '%s\n' "$targets" | grep -c . || true)
echo "build-site: ok - ${#subprojects[@]} sub-projects composed, $n cross-project link(s) resolve."
