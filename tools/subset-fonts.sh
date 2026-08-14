#!/usr/bin/env bash
# Produce the vendored woff2 faces from upstream, reproducibly.
#
# The font bytes in assets/fonts/ are a MEASURED artifact, not a download: they are
# subset to Latin, stripped of hinting, and — for the mono — stripped of `calt`, which
# halves it (37,828 -> 19,768 B measured 2026-08-14) AND makes code ligatures
# impossible to re-enable by a stray CSS rule. Code ligatures misrepresent the source
# characters, so this is a correctness choice as much as a size one.
#
# Needs fontTools + brotli. fontTools is in .venv; brotli is not, so it is installed
# into a scratch dir rather than mutating the project venv.
#
#   tools/subset-fonts.sh            # rebuild assets/fonts/ from upstream
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

OUT=crates/core/assets/fonts
WORK=$(mktemp -d -t tali-fonts-XXXXXX)
trap 'rm -rf "$WORK"' EXIT

LATIN="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,\
U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD"

.venv/bin/pip install --quiet --target="$WORK/pylibs" brotli

fetch() { curl -sSLf -o "$WORK/$1" "$2"; }
sub() { # <in> <out> <layout-features>
    PYTHONPATH="$WORK/pylibs" .venv/bin/pyftsubset "$WORK/$1" \
        --output-file="$OUT/$2" --flavor=woff2 --unicodes="$LATIN" \
        --layout-features="$3" --no-hinting --desubroutinize
}

CDN=https://cdn.jsdelivr.net/npm
fetch lit.woff2      "$CDN/@fontsource-variable/literata/files/literata-latin-wght-normal.woff2"
fetch lit-it.woff2   "$CDN/@fontsource-variable/literata/files/literata-latin-wght-italic.woff2"
fetch jbm.woff2      "$CDN/@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2"

# onum/tnum/smcp are kept: the theme sets table figures tabular and uses small caps.
sub lit.woff2    literata-latin-wght-normal.woff2      "kern,ccmp,mark,mkmk,onum,tnum,smcp,liga"
sub lit-it.woff2 literata-latin-wght-italic.woff2      "kern,ccmp,mark,mkmk,onum,tnum,liga"
# NO calt, NO liga on the mono. See the header.
sub jbm.woff2    jetbrains-mono-latin-wght-normal.woff2 "kern,ccmp,mark,mkmk"

echo "vendored:"
for f in "$OUT"/*.woff2; do printf "  %8s  %s\n" "$(stat -c%s "$f")" "$(basename "$f")"; done
