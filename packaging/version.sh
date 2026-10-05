#!/bin/sh
# Prints the version of the wordl script: its VERSION="..." line.
#
#   version.sh [file]     the file defaults to ./wordl; - reads standard input
set -eu
sed -n 's/^VERSION="\(.*\)"$/\1/p' "${1:-wordl}" | head -n 1
