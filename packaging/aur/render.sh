#!/bin/sh
# Fills in the AUR package templates for one release.
#
#   render.sh <version> <SHA256SUMS file> <LICENSE file> <SCOWL-COPYRIGHT file> <output dir>
#
# .SRCINFO is rendered from its own template rather than with `makepkg --printsrcinfo`
# because releases are built on Ubuntu, which has no makepkg. Keep the two templates
# in step; `makepkg --printsrcinfo | diff - .SRCINFO` on an Arch machine checks them.
set -eu

[ $# -eq 5 ] || { echo "usage: render.sh <version> <SHA256SUMS> <LICENSE> <SCOWL-COPYRIGHT> <outdir>" >&2; exit 1; }
version=$1 sums=$2 license=$3 scowl=$4 out=$5
here=$(dirname "$0")

sum_of() {
    sum=$(awk -v f="$1" '$2 == f || $2 == "*" f { print $1 }' "$sums")
    [ -n "$sum" ] || { echo "render.sh: no checksum for $1 in $sums" >&2; exit 1; }
    echo "$sum"
}

x86_64=$(sum_of wordl-x86_64-unknown-linux-musl)
aarch64=$(sum_of wordl-aarch64-unknown-linux-musl)
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}
license_sum=$(sha256_of "$license")
scowl_sum=$(sha256_of "$scowl")

mkdir -p "$out"
for pair in PKGBUILD.in:PKGBUILD SRCINFO.in:.SRCINFO; do
    sed -e "s/@VERSION@/$version/g" -e "s/@SHA_LICENSE@/$license_sum/g" -e "s/@SHA_SCOWL@/$scowl_sum/g" \
        -e "s/@SHA_X86_64@/$x86_64/g" -e "s/@SHA_AARCH64@/$aarch64/g" \
        "$here/${pair%%:*}" > "$out/${pair##*:}"
done
# The AUR asks for a license covering the package files themselves (0BSD); this is
# not the game's license, which the package installs from the release tag.
cp "$here/LICENSE" "$out/LICENSE"
