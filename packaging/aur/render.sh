#!/bin/sh
# Fills in the AUR package templates for one release.
#
#   render.sh <version> <SHA256SUMS file> <output dir>
#
# .SRCINFO is rendered from its own template rather than with `makepkg --printsrcinfo`
# because releases are built on Ubuntu, which has no makepkg. Keep the two templates
# in step; `makepkg --printsrcinfo | diff - .SRCINFO` on an Arch machine checks them.
set -eu

[ $# -eq 3 ] || { echo "usage: render.sh <version> <SHA256SUMS> <outdir>" >&2; exit 1; }
version=$1 sums=$2 out=$3
here=$(dirname "$0")

archive=wordl-$version.tar.gz
sum=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1 }' "$sums")
[ -n "$sum" ] || { echo "render.sh: no checksum for $archive in $sums" >&2; exit 1; }

mkdir -p "$out"
for pair in PKGBUILD.in:PKGBUILD SRCINFO.in:.SRCINFO; do
    sed -e "s/@VERSION@/$version/g" -e "s/@SHA_ARCHIVE@/$sum/g" \
        "$here/${pair%%:*}" > "$out/${pair##*:}"
done
# The AUR asks for a license covering the package files themselves (0BSD); this is
# not the game's license, which the package installs from the release archive.
cp "$here/LICENSE" "$out/LICENSE"
