#!/bin/sh
# End-to-end tests: the built program, run the way a user runs it.
#
#   tests/e2e.sh [path to the wordl binary]     default: target/release/wordl
#
#   1. the command line
#   2. install.sh and the self-updater, against releases made up here and served
#      from file:// (needs curl)
#   3. the game itself in a detached tmux session (skipped when tmux is missing)
#
# The rules, the layout and the drawing are covered by `cargo test`; this is for what
# only shows when the real program meets a real terminal.
set -u

cd "$(dirname "$0")/.." || exit 1
ROOT=$PWD
BIN=${1:-target/release/wordl}
case $BIN in /*) ;; *) BIN=$ROOT/$BIN ;; esac
[ -x "$BIN" ] || { echo "e2e.sh: $BIN is not built (cargo build --release)" >&2; exit 1; }

TMP=$(mktemp -d)
SOCK=wordl-e2e-$$
trap 'tmux -L "$SOCK" kill-server 2>/dev/null; rm -rf "$TMP"' EXIT

fails=0
pass() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; fails=$((fails + 1)); }
is() { # name expected actual
    if [ "$2" = "$3" ]; then pass "$1"; else fail "$1: expected '$2', got '$3'"; fi
}
has() { # name text-to-find text
    case $3 in *"$2"*) pass "$1" ;; *) fail "$1: no '$2' in '$3'" ;; esac
}

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)

# ---------------------------------------------------------- command line ----

has "--version" "wordl $VERSION (" "$("$BIN" --version)"
has "--help" "wordl update" "$("$BIN" --help)"
has "--licenses names SCOWL" "Kevin Atkinson" "$("$BIN" --licenses)"
has "an unknown option is refused" "unknown option" "$("$BIN" --nonsense 2>&1)"
has "an unknown theme is refused" "unknown theme" "$("$BIN" --theme plaid 2>&1)"
has "needs a terminal" "needs an interactive terminal" "$("$BIN" </dev/null 2>&1)"
has "a checkout never updates itself" "running from a source checkout" "$("$BIN" update 2>&1)"

# ------------------------------------------------- install and self-update ----

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64 | Linux-amd64) ASSET=wordl-x86_64-unknown-linux-musl ;;
    Linux-aarch64 | Linux-arm64) ASSET=wordl-aarch64-unknown-linux-musl ;;
    Darwin-arm64) ASSET=wordl-aarch64-apple-darwin ;;
    Darwin-x86_64) ASSET=wordl-x86_64-apple-darwin ;;
    *) ASSET= ;;
esac

# make_release <dir> <version> <file to publish as this platform's binary>
make_release() {
    mkdir -p "$1"
    cp "$3" "$1/$ASSET"
    echo "$2" > "$1/VERSION"
    echo "$(sha256_of "$1/$ASSET")  $ASSET" > "$1/SHA256SUMS"
}

INST=$TMP/inst/bin/wordl
installed() { XDG_STATE_HOME="$TMP/ustate" "$INST" "$@" 2>&1; }
# The copy a user would have: outside any checkout, in a directory they own.
fresh_copy() {
    rm -rf "$TMP/inst"
    mkdir -p "$TMP/inst/bin"
    cp "$BIN" "$INST"
}

if ! command -v curl >/dev/null 2>&1 || [ -z "$ASSET" ]; then
    echo "skip  install and self-update (no curl, or no release binary for this platform)"
else
    # A "newer release" whose binary is a script, so that it is plain which one runs.
    printf '#!/bin/sh\necho "wordl 99.0.0 (fake)"\n' > "$TMP/fake"
    chmod 755 "$TMP/fake"
    make_release "$TMP/rel-now" "$VERSION" "$BIN"
    make_release "$TMP/rel-new" 99.0.0 "$TMP/fake"
    make_release "$TMP/rel-old" 0.0.1 "$TMP/fake"
    make_release "$TMP/rel-bad" 99.0.0 "$TMP/fake"
    echo "0000000000000000000000000000000000000000000000000000000000000000  $ASSET" > "$TMP/rel-bad/SHA256SUMS"

    out=$(WORDL_RELEASE_URL="file://$TMP/rel-now" WORDL_BIN_DIR="$TMP/inst/bin" sh install.sh 2>&1)
    has "install.sh: installs the release" "wordl $VERSION (" "$(installed --version)"
    has "install.sh: says where" "Installed $INST" "$out"
    out=$(WORDL_RELEASE_URL="file://$TMP/rel-bad" WORDL_BIN_DIR="$TMP/inst2/bin" sh install.sh 2>&1)
    has "install.sh: refuses a bad checksum" "checksum mismatch" "$out"
    if [ -e "$TMP/inst2/bin/wordl" ]; then fail "install.sh: installed despite a bad checksum"; else pass "install.sh: a refused download installs nothing"; fi
    WORDL_BIN_DIR="$TMP/inst/bin" sh install.sh --uninstall >/dev/null 2>&1
    if [ -e "$INST" ]; then fail "install.sh: --uninstall left the binary"; else pass "install.sh: --uninstall removes it"; fi

    fresh_copy
    has "update: nothing newer" "wordl $VERSION is up to date (latest release is $VERSION)." "$(WORDL_RELEASE_URL="file://$TMP/rel-now" installed update)"
    has "update: never downgrades" "is up to date (latest release is 0.0.1)." "$(WORDL_RELEASE_URL="file://$TMP/rel-old" installed update)"
    has "update: refuses a bad checksum" "checksum mismatch" "$(WORDL_RELEASE_URL="file://$TMP/rel-bad" installed update)"
    has "update: a refused update changes nothing" "wordl $VERSION (" "$(installed --version)"
    has "update: reports an unreachable server" "could not check for updates" "$(WORDL_RELEASE_URL="file://$TMP/nowhere" installed update)"

    # What a package does when it installs: a marker beside the binary's directory.
    mkdir -p "$TMP/inst/share/wordl"
    echo "Homebrew; use brew upgrade wordl" > "$TMP/inst/share/wordl/managed-by"
    is "update: a package's copy refuses" "wordl: this copy can't update itself: installed with Homebrew; use brew upgrade wordl" "$(WORDL_RELEASE_URL="file://$TMP/rel-new" installed update)"
    has "update: a package's copy is untouched" "wordl $VERSION (" "$(installed --version)"

    # Reached through a link, as Homebrew's bin directory does it: still refused.
    mkdir -p "$TMP/link"
    ln -s "$INST" "$TMP/link/wordl"
    has "update: a package's copy refuses through a link too" "installed with Homebrew" "$(WORDL_RELEASE_URL="file://$TMP/rel-new" XDG_STATE_HOME="$TMP/ustate" "$TMP/link/wordl" update 2>&1)"

    fresh_copy
    has "update: installs a newer release" "Updated to 99.0.0." "$(WORDL_RELEASE_URL="file://$TMP/rel-new" installed update)"
    is "update: the new version is what runs" "wordl 99.0.0 (fake)" "$(installed --version)"

    # Through a link, the real file is replaced and the link is left alone.
    fresh_copy
    WORDL_RELEASE_URL="file://$TMP/rel-new" XDG_STATE_HOME="$TMP/ustate" "$TMP/link/wordl" update >/dev/null 2>&1
    if [ -L "$TMP/link/wordl" ] && [ "$("$INST" --version)" = "wordl 99.0.0 (fake)" ]; then
        pass "update: through a link, the real file is replaced and the link survives"
    else
        fail "update: through a link, the link was replaced or the file was not"
    fi

    fresh_copy
    has "update off" "The update check at startup is off." "$(installed update off)"
    is "update off is remembered" "update=0" "$(grep -x 'update=0' "$TMP/ustate/wordl/stats")"
    has "update on" "The update check at startup is on." "$(installed update on)"
fi

# -------------------------------------------------------------- the game ----

if ! command -v tmux >/dev/null 2>&1; then
    echo "skip  the game in tmux (tmux is not installed)"
else
    screen() { tmux -L "$SOCK" capture-pane -p 2>/dev/null; }
    expect() { # name text -- waits up to 5 seconds for the text to be on screen
        i=0
        while [ "$i" -lt 50 ]; do
            if screen | grep -qF -- "$2"; then pass "$1"; return; fi
            sleep 0.1
            i=$((i + 1))
        done
        fail "$1: '$2' never appeared"
        screen | sed 's/^/        | /'
    }
    keys() { tmux -L "$SOCK" send-keys "$@"; }
    # start <state dir> <command and arguments>: a fresh session running the game
    start() {
        tmux -L "$SOCK" kill-server 2>/dev/null
        state=$1
        shift
        tmux -L "$SOCK" new-session -d -x 80 -y 24 \
            "env XDG_STATE_HOME='$state' WORDL_NO_UPDATE=1 WORDL_DEBUG_ANSWER=crane $*; echo \"EXIT=\$?\"; sleep 20"
    }

    start "$TMP/xdg" WORDL_NO_ANIM=1 "'$BIN'"
    expect "game: starts" "W   O   R   D   L"
    keys '?'
    expect "game: help opens" "HOW TO PLAY"
    keys q
    expect "game: help closes" "Q   W   E   R   T"
    keys -l qqqqq
    keys Enter
    expect "game: rejects a non-word" "Not in word list"
    keys BSpace BSpace BSpace BSpace BSpace
    keys -l slate
    keys Enter
    keys -l cran
    # A click on the E key, then on Enter (bottom right): SGR mouse press at column;row.
    keys -l "$(printf '\033[<0;30;21M')"
    sleep 0.2
    keys -l "$(printf '\033[<0;58;23M')"
    expect "game: mouse clicks type and submit; the game is won" "Solved in 2/6"
    expect "game: statistics open after a win" "STATISTICS"
    keys Escape
    sleep 0.3 # Escape followed at once by a key would arrive as Alt+key
    tmux -L "$SOCK" resize-window -x 30 -y 8 2>/dev/null
    expect "game: a tiny window says so" "wordl needs 39x12"
    tmux -L "$SOCK" resize-window -x 150 -y 46 2>/dev/null
    expect "game: a big window gets block letters" "█▀▀▀"
    keys C-q
    expect "game: quits cleanly" "EXIT=0"
    is "game: the win is saved" "practice_wins=1" "$(grep -x 'practice_wins=1' "$TMP/xdg/wordl/stats" 2>/dev/null)"

    # Giving up: a question first, then the answer, then on to the next word.
    start "$TMP/xdg2" WORDL_NO_ANIM=1 "'$BIN'" --ultra
    expect "give up: game starts" "Ultra Hard"
    keys -l slate
    keys Enter
    keys -l blaze
    keys Enter
    expect "ultra hard: refuses a guess that ignores a gray clue" "L is not in the word"
    keys C-g
    expect "give up: asks first" "GIVE UP?"
    keys Escape
    sleep 0.3
    expect "give up: Esc keeps playing" "Q   W   E   R   T"
    keys C-g
    expect "give up: asks again" "GIVE UP?"
    keys Enter
    expect "give up: shows the answer on the board" "█ C █  █ R █  █ A █  █ N █  █ E █"
    expect "give up: says the word" "The word was CRANE"
    expect "give up: statistics follow" "You gave up"
    keys Enter
    sleep 0.3
    if screen | grep -qF "S      L      A"; then fail "give up: Enter did not start the next word"; else pass "give up: Enter starts the next word"; fi
    is "give up: saved as a loss" "practice_played=1" "$(grep -x 'practice_played=1' "$TMP/xdg2/wordl/stats" 2>/dev/null)"

    # With animations on: a guess is revealed, and keys typed meanwhile are kept.
    start "$TMP/xdg3" "'$BIN'"
    expect "animation: game starts" "W   O   R   D   L"
    keys -l slate
    keys Enter
    keys -l crane
    keys Enter
    expect "animation: keys typed during a reveal are not lost" "Solved in 2/6"
    keys C-q
    expect "animation: quits cleanly" "EXIT=0"

    # Starting the installed copy when a newer release exists: it updates and restarts
    # as the new version. Switched off, it starts the game without updating.
    if command -v curl >/dev/null 2>&1 && [ -n "$ASSET" ]; then
        launch() {
            tmux -L "$SOCK" kill-server 2>/dev/null
            tmux -L "$SOCK" new-session -d -x 80 -y 24 \
                "env XDG_STATE_HOME='$TMP/ustate' WORDL_RELEASE_URL='file://$TMP/rel-new' WORDL_NO_ANIM=1 '$INST'; echo \"EXIT=\$?\"; sleep 20"
        }
        fresh_copy
        installed update off >/dev/null
        launch
        expect "update: switched off, the game starts" "W   O   R   D   L"
        keys C-q
        expect "update: switched off, it quits cleanly" "EXIT=0"
        has "update: switched off, nothing is updated" "wordl $VERSION (" "$(installed --version)"
        installed update on >/dev/null
        launch
        expect "update: starting the game updates it and runs the new version" "wordl 99.0.0 (fake)"
    fi
    tmux -L "$SOCK" kill-server 2>/dev/null
fi

echo
if [ "$fails" -gt 0 ]; then
    echo "$fails failed"
    exit 1
fi
echo "all passed"
