#!/bin/sh
# Builds the release archive wordl-<version>.tar.gz and prints the version.
#
#   dist.sh <output dir>
#
# The archive unpacks to wordl-<version>/ holding everything the game needs to run:
# the script, the word lists, and the licenses. Its name and layout are relied on by
# install.sh, the AUR package and the Homebrew formula; change them together.
set -eu

[ $# -eq 1 ] || { echo "usage: dist.sh <output dir>" >&2; exit 1; }
out=$1
root=$(cd "$(dirname "$0")/.." && pwd)
version=$("$root/packaging/version.sh" "$root/wordl")
[ -n "$version" ] || { echo "dist.sh: no VERSION line in wordl" >&2; exit 1; }

name=wordl-$version
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$name/words" "$out"
cp "$root/wordl" "$root/LICENSE" "$root/README.md" "$stage/$name/"
cp "$root/words/answers.txt" "$root/words/allowed.txt" "$root/words/SCOWL-COPYRIGHT" "$stage/$name/words/"
chmod 755 "$stage/$name/wordl"
# COPYFILE_DISABLE keeps macOS tar from adding ._* files.
COPYFILE_DISABLE=1 tar -czf "$out/$name.tar.gz" -C "$stage" "$name"
echo "$version"
