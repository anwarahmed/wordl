#!/bin/sh
# Installs the latest release of wordl for the current user, or updates it.
#
#   curl -fsSL https://raw.githubusercontent.com/anwarahmed/wordl/main/install.sh | sh
#   ./install.sh --uninstall
#
# The game goes into ~/.local/share/wordl (WORDL_HOME) and a link to it into
# ~/.local/bin (WORDL_BIN_DIR). Nothing else is touched: no shell profile is edited and
# bash is not installed for you. WORDL_RELEASE_URL points at another download location
# (a directory holding SHA256SUMS and the archive; file:// works, which is how this
# script is tested).
set -eu

REPO="anwarahmed/wordl"
home=${WORDL_HOME:-$HOME/.local/share/wordl}
bin=${WORDL_BIN_DIR:-$HOME/.local/bin}
base=${WORDL_RELEASE_URL:-https://github.com/$REPO/releases/latest/download}

die() { echo "install.sh: $*" >&2; exit 1; }

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
    else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

uninstall() {
    # Only remove the link if it is ours.
    if [ -L "$bin/wordl" ] && [ "$(readlink "$bin/wordl")" = "$home/wordl" ]; then rm -f "$bin/wordl"; fi
    rm -rf "$home"
    echo "wordl is removed. Statistics are kept in ${XDG_STATE_HOME:-$HOME/.local/state}/wordl; delete that folder to remove them too."
}

# Everything runs from this function, called on the last line, so the whole script has
# arrived before any of it runs when it is piped into sh.
main() {
    case ${1:-} in
        --uninstall) uninstall; exit 0 ;;
        "") ;;
        *) die "unknown option '$1' (the only option is --uninstall)" ;;
    esac

    for tool in curl tar awk; do
        command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed"
    done
    command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || die "neither sha256sum nor shasum is installed"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT

    curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" || die "could not download $base/SHA256SUMS"
    archive=$(awk '{ sub(/^\*/, "", $2); if ($2 ~ /^wordl-[0-9][0-9A-Za-z.]*\.tar\.gz$/) { print $2; exit } }' "$tmp/SHA256SUMS")
    [ -n "$archive" ] || die "the release lists no wordl archive"
    want=$(awk -v f="$archive" '{ sub(/^\*/, "", $2); if ($2 == f) print $1 }' "$tmp/SHA256SUMS")
    version=${archive#wordl-}
    version=${version%.tar.gz}

    curl -fsSL "$base/$archive" -o "$tmp/$archive" || die "could not download $base/$archive"
    got=$(sha256 "$tmp/$archive")
    [ "$got" = "$want" ] || die "checksum mismatch for $archive (expected $want, got $got)"

    tar -xzf "$tmp/$archive" -C "$tmp"
    [ -f "$tmp/wordl-$version/wordl" ] || die "$archive does not hold wordl-$version/wordl"

    mkdir -p "$bin" "$(dirname "$home")"
    rm -rf "$home"
    mv "$tmp/wordl-$version" "$home"
    chmod 755 "$home/wordl"
    ln -sf "$home/wordl" "$bin/wordl"
    echo "wordl $version is installed: $bin/wordl"

    case ":$PATH:" in
        *":$bin:"*) ;;
        *) echo "Note: $bin is not on your PATH. Add it, or run $bin/wordl." ;;
    esac
    # shellcheck disable=SC2016 # the single quotes are for bash, which expands this itself
    if ! bash -c '((BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4)))' 2>/dev/null; then
        echo "Note: wordl needs bash 4.4 or newer and the bash on your PATH is older. On macOS: brew install bash"
    fi
}

main "$@"
