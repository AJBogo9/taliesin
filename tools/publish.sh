#!/usr/bin/env bash
# Publish the Taliesin sites. FOUR separate Taliesin projects, FOUR Cloudflare Pages
# projects, four domains. Nothing is composed across them.
#
#   tools/publish.sh                  # check, build and deploy all four
#   tools/publish.sh guide internals  # just these
#   tools/publish.sh --check          # THE GATE: lint + build --no-exec into temp dirs, nothing deployed
#
# WHY FOUR PROJECTS AND NOT ONE COMPOSED TREE. Cloudflare Pages has no subpath deploy:
# `wrangler pages deploy <dir>` uploads that directory as the ENTIRE site for its project,
# and every deployment is a complete snapshot. Publishing the Guide to taliesin.sh/docs/guide
# would therefore mean assembling all four projects into one local tree and re-uploading the
# whole thing on every change. Separate Pages projects cost nothing (100 per account, 100
# custom domains each on the free plan), and each site then builds, previews and deploys
# alone. Cross-site links are absolute URLs, which need no composition to resolve and no
# exemption from the link checker. This replaced `mounts:` (cut 2026-08-09) and the composed
# single-tree deploy that followed it (the composition script, deleted with this).
#
# Override the binary with TALIESIN=... (default: cargo run -q -p taliesin-server --) and
# the deployer with WRANGLER=... (default: npx wrangler).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

TALIESIN=${TALIESIN:-cargo run -q -p taliesin-server --}
WRANGLER=${WRANGLER:-npx wrangler}

# target -> source project directory
src_of() {
    case "$1" in
    site) echo "site" ;;
    guide) echo "docs/guide" ;;
    internals) echo "docs/internals" ;;
    gallery) echo "gallery" ;;
    *) return 1 ;;
    esac
}

# target -> Cloudflare Pages project name
pages_project_of() {
    case "$1" in
    site) echo "taliesin-site" ;;
    guide) echo "taliesin-guide" ;;
    internals) echo "taliesin-internals" ;;
    gallery) echo "taliesin-gallery" ;;
    *) return 1 ;;
    esac
}

ALL_TARGETS=(site guide internals gallery)

check_only=0
targets=()
for arg in "$@"; do
    case "$arg" in
    --check) check_only=1 ;;
    site | guide | internals | gallery) targets+=("$arg") ;;
    *)
        echo "usage: tools/publish.sh [--check] [site|guide|internals|gallery ...]" >&2
        exit 2
        ;;
    esac
done
[ ${#targets[@]} -eq 0 ] && targets=("${ALL_TARGETS[@]}")

if [ "$check_only" -eq 1 ]; then
    echo "publish: checking ${#targets[@]} project(s) (--no-exec, temp output, nothing deployed)"
    for target in "${targets[@]}"; do
        src=$(src_of "$target")
        $TALIESIN build "$src" --check-only --no-exec
        out=$(mktemp -d -t "tali-publish-$target-XXXXXX")
        $TALIESIN build "$src" --out "$out" --no-exec
        rm -rf "$out"
    done
    echo "publish: ok - ${#targets[@]} project(s) check clean."
    exit 0
fi

for target in "${targets[@]}"; do
    src=$(src_of "$target")
    project=$(pages_project_of "$target")
    out="$PWD/$src/_site"

    echo "publish: === $target ($src -> $project) ==="
    # Lint before building for real: a broken link or a bad reference is cheaper to find
    # here than in a deployment that is already live.
    $TALIESIN build "$src" --check-only --no-exec
    $TALIESIN build "$src" --out "$out"
    $WRANGLER pages deploy "$out" --project-name="$project"
done

echo "publish: done - ${#targets[@]} project(s) deployed."
