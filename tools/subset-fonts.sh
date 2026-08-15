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
# into a scratch dir rather than mutating the project venv. `.venv` itself is
# gitignored, so a fresh clone needs one before this script can run:
#   python3 -m venv .venv && .venv/bin/pip install --quiet fonttools==4.63.0
#
# The CDN URLs below are version-pinned (@5.2.8 for both faces, 2026-08-14): an
# unpinned `@fontsource-variable/...` URL resolves to whatever jsDelivr serves that
# day, which makes "rebuild from upstream" not actually reproducible. Bump the pin
# deliberately and record the new version in THIRD_PARTY.md in the same change.
#
# The COMPRESSOR is pinned for the same reason, and it is the easier one to overlook:
# woff2 is brotli, so a different brotli produces different bytes from identical glyph
# data — every metric unchanged, every hash changed. Unpinned, this script could not
# reproduce its own output, and the resulting failure of
# `the_body_face_is_the_one_the_measure_was_measured_on` reads exactly like a face swap.
# fontTools is the remaining loose input: it comes from `.venv`, which was 4.63.0 on
# 2026-08-15. Pin added 2026-08-15 and NOT verified to reproduce the bytes already on
# disk — re-vendoring is a later plan's work, because it moves the hash and the measure
# constant together.
#
#   tools/subset-fonts.sh            # rebuild assets/fonts/ from upstream
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

OUT=crates/core/assets/fonts
WORK=$(mktemp -d -t tali-fonts-XXXXXX)
trap 'rm -rf "$WORK"' EXIT

LATIN="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,\
U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD"

.venv/bin/pip install --quiet --target="$WORK/pylibs" brotli==1.2.0

fetch() { curl -sSLf -o "$WORK/$1" "$2"; }
sub() { # <in> <out> <layout-features>
    PYTHONPATH="$WORK/pylibs" .venv/bin/pyftsubset "$WORK/$1" \
        --output-file="$OUT/$2" --flavor=woff2 --unicodes="$LATIN" \
        --layout-features="$3" --no-hinting --desubroutinize
}

CDN=https://cdn.jsdelivr.net/npm
fetch lit.woff2      "$CDN/@fontsource-variable/literata@5.2.8/files/literata-latin-wght-normal.woff2"
fetch lit-it.woff2   "$CDN/@fontsource-variable/literata@5.2.8/files/literata-latin-wght-italic.woff2"
fetch jbm.woff2      "$CDN/@fontsource-variable/jetbrains-mono@5.2.8/files/jetbrains-mono-latin-wght-normal.woff2"

# tnum is kept: the theme sets table figures tabular. rvrn is kept too: it drives the
# GSUB FeatureVariations that substitute glyphs (e.g. `$`/`¢`) by weight, and it is on
# by default in pyftsubset's own --layout-features list — a default our explicit list
# below overrides, so it must be named or the substitution silently stops above ~wght
# 600 (checked 2026-08-14 against the upstream variable font; a prior version of this
# script also named `onum` and `smcp` here, but upstream Literata's GSUB table defines
# neither, so requesting them was always a no-op).
sub lit.woff2    literata-latin-wght-normal.woff2      "kern,ccmp,mark,mkmk,tnum,rvrn,liga"
sub lit-it.woff2 literata-latin-wght-italic.woff2      "kern,ccmp,mark,mkmk,tnum,rvrn,liga"
# NO calt, NO liga on the mono. See the header. (JetBrains Mono has no FeatureVariations
# table at all, so rvrn is not applicable here.)
sub jbm.woff2    jetbrains-mono-latin-wght-normal.woff2 "kern,ccmp,mark,mkmk"

echo "vendored:"
for f in "$OUT"/*.woff2; do printf "  %8s  %s\n" "$(stat -c%s "$f")" "$(basename "$f")"; done
