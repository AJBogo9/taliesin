#!/usr/bin/env bash
# Publish the Taliesin sites. FOUR separate Taliesin projects, FOUR Cloudflare Pages
# projects, four domains. Nothing is composed across them.
#
#   tools/publish.sh                  # check, build and deploy all four
#   tools/publish.sh guide internals  # just these
#   tools/publish.sh --check          # THE GATE: lint + build --no-exec into temp dirs,
#                                     #   gallery exhibit links asserted, nothing deployed
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
# THE GALLERY IS THE ONE EXCEPTION, AND IT IS CONTAINED. Its three exhibits are separate
# Taliesin projects written UNDER its own output, because a gallery is a collection by
# definition. That is the only composition left in the tree, it never crosses a domain, and
# this script RESOLVES every exhibit link against the built output before deploying, because
# a shipped page whose primary link 404s is a failure this project has already had once
# (item 149).
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

# The gallery's exhibits: source project -> URL prefix under the gallery root.
GALLERY_EXHIBITS=(
    "corpus/tarn:tarn"
    "corpus/descent:descent"
    "corpus/analyst:analyst"
)

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

# Build one target into $1=outdir. The gallery additionally composes its exhibits.
#
# THE PARENT IS BUILT FIRST AND THAT ORDER IS LOAD-BEARING: `build` sweeps stale output,
# deleting anything under its output directory it did not itself write, so an exhibit built
# before the gallery would be silently swept away.
build_target() {
    local target=$1 out=$2
    shift 2
    local extra=("$@")
    local src
    src=$(src_of "$target")

    $TALIESIN build "$src" --out "$out" "${extra[@]}"

    if [ "$target" = "gallery" ]; then
        local entry exhibit_src prefix
        for entry in "${GALLERY_EXHIBITS[@]}"; do
            exhibit_src=${entry%%:*}
            prefix=${entry#*:}
            $TALIESIN build "$exhibit_src" --out "$out/$prefix" "${extra[@]}"
        done
    fi
}

# Resolve every exhibit link written in gallery/ against the composed output. A directory
# link (and an extensionless one) resolves to index.html.
assert_gallery_links() {
    local out=$1 missing=() t candidate targets_found n prefixes alt
    # The prefixes to scan for are DERIVED from GALLERY_EXHIBITS rather than spelled again
    # here, so adding an exhibit updates the scan with it. A second copy would go stale and
    # report "the gallery does not link to everything it ships" when the truth is that this
    # scan stopped looking for the new one.
    prefixes=("${GALLERY_EXHIBITS[@]#*:}")
    alt=$(
        IFS='|'
        echo "${prefixes[*]}"
    )
    # `|| true`: under `set -o pipefail` a grep that matches nothing fails the whole
    # pipeline, which would abort here with no message at all. An empty result is a real
    # answer and it is caught right below, loudly — the anti-vacuity check that a bare
    # `exit 1` would have hidden.
    targets_found=$(grep -rhoE "(\(|href: \")($alt)/[A-Za-z0-9._/-]*" \
        gallery/_site.yml gallery/*.tmd | sed -E 's/^(\(|href: ")//' | sort -u || true)
    n=$(printf '%s\n' "$targets_found" | grep -c . || true)
    if [ "$n" -lt "${#GALLERY_EXHIBITS[@]}" ]; then
        echo "publish: REFUSED. found $n exhibit link(s) in gallery/ but ${#GALLERY_EXHIBITS[@]}" \
            "exhibit(s) are published — the gallery does not link to everything it ships," >&2
        echo "         or this scan stopped matching the links (an empty scan passes forever)." >&2
        exit 1
    fi
    for t in $targets_found; do
        candidate="$out/$t"
        case "$t" in
        */) candidate="$out/${t}index.html" ;;
        *.*) ;;
        *) candidate="$out/$t/index.html" ;;
        esac
        [ -f "$candidate" ] || missing+=("$t -> ${candidate#"$out"/}")
    done
    if [ ${#missing[@]} -gt 0 ]; then
        echo "publish: REFUSED. ${#missing[@]} gallery exhibit link(s) have nothing behind them:" >&2
        printf '  %s\n' "${missing[@]}" >&2
        exit 1
    fi
    echo "publish: gallery ok - ${#GALLERY_EXHIBITS[@]} exhibits, $n link(s) resolve."
}

if [ "$check_only" -eq 1 ]; then
    echo "publish: checking ${#targets[@]} project(s) (--no-exec, temp output, nothing deployed)"
    for target in "${targets[@]}"; do
        src=$(src_of "$target")
        $TALIESIN build "$src" --check-only --no-exec
        out=$(mktemp -d -t "tali-publish-$target-XXXXXX")
        build_target "$target" "$out" --no-exec
        if [ "$target" = "gallery" ]; then
            assert_gallery_links "$out"
        fi
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
    build_target "$target" "$out"
    if [ "$target" = "gallery" ]; then
        assert_gallery_links "$out"
    fi
    $WRANGLER pages deploy "$out" --project-name="$project"
done

echo "publish: done - ${#targets[@]} project(s) deployed."
