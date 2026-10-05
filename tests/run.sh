#!/usr/bin/env bash
# Tests for wordl. They run with the bash that runs this file, so to test another
# bash: /path/to/bash tests/run.sh
#
#   1. the game's rules, by sourcing the script and calling its functions
#   2. the word lists
#   3. the layout at every terminal size, and a full render at several
#   4. self-update, against releases built here and served from file:// (needs curl)
#   5. the real game in a detached tmux session (skipped when tmux is missing)
#
# The variables set here are read by the sourced script, which shellcheck can't see,
# and `a && pass || fail` is safe because pass never fails.
# shellcheck disable=SC2034,SC2015,SC2153,SC1091
set -u

cd "$(dirname "$0")/.." || exit 1
ROOT=$PWD
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"; [[ -n ${SOCK:-} ]] && tmux -L "$SOCK" kill-server 2>/dev/null' EXIT

fails=0
pass() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; fails=$((fails + 1)); }
is() { # name expected actual
  if [[ $2 == "$3" ]]; then pass "$1"; else fail "$1: expected '$2', got '$3'"; fi
}

echo "bash $BASH_VERSION"

"$BASH" -n wordl && pass "syntax" || fail "syntax"
is "--version" "wordl $(packaging/version.sh)" "$("$BASH" ./wordl --version)"

# ------------------------------------------------------------- functions ----

# shellcheck source=../wordl
source ./wordl
SELF_DIR=$ROOT STATE_DIR=$TMP/state ANIM=0 TRUECOLOR=1
load_words
set_theme midnight

score() { evaluate "$1" "$2"; echo "$REPLY"; }
is "evaluate: all right" ggggg "$(score CRANE CRANE)"
is "evaluate: mixed" xxgxg "$(score SLATE CRANE)"
is "evaluate: repeated letter, one in the word" xxyxg "$(score EERIE CRANE)"
is "evaluate: repeated letter, green wins" yxgxg "$(score BOBBY ABBEY)"
is "evaluate: repeated letter, yellow once" ygyxx "$(score LLAMA ALLOW)"
is "evaluate: three of a letter" xyxgx "$(score NANNY CRANE)"

# verdict <difficulty> <answer> "<earlier guesses>" <guess> -> ok, or why it is refused
verdict() {
  local g
  GAME_HARD=$1 ANSWER=$2 GUESSES=() MARKS=()
  for g in $3; do
    evaluate "$g" "$ANSWER"
    GUESSES+=("$g") MARKS+=("$REPLY")
  done
  if hard_check "$4"; then echo ok; else echo "$REPLY"; fi
}
is "hard: free letters" ok "$(verdict 1 CRANE SLATE BLAZE)"
is "hard: green stays" "3rd letter must be A" "$(verdict 1 CRANE SLATE COUNT)"
is "hard: yellow is reused" "Guess must contain C" "$(verdict 1 CRANE TRACK BRAVE)"
is "hard: yellow may stay put" ok "$(verdict 1 CRANE TRACK CRACK)"
is "ultra: gray is obeyed" "L is not in the word" "$(verdict 2 CRANE SLATE BLAZE)"
is "ultra: yellow must move" "4th letter can't be C" "$(verdict 2 CRANE TRACK CRACK)"
is "ultra: the answer passes" ok "$(verdict 2 CRANE "SLATE TRACK" CRANE)"
is "ultra: count of a repeated letter" "Only 1 E in the word" "$(verdict 2 CRANE EERIE CREPE)"
is "ultra: repeated letter, answer passes" ok "$(verdict 2 ABBEY BOBBY ABBEY)"
is "ultra: gray spot of a repeated letter" "4th letter can't be B" "$(verdict 2 ABBEY BOBBY ALBBY)"

is_word CRANE && pass "is_word: a word" || fail "is_word: a word"
is_word ZZZZZ && fail "is_word: not a word" || pass "is_word: not a word"

HARD=0
new_game daily; first=$ANSWER
new_game daily
[[ $first =~ ^[A-Z]{5}$ ]] && pass "daily word is five letters" || fail "daily word is '$first'"
is "daily word is the same all day" "$first" "$ANSWER"
is_word "$first" && pass "daily word is in the dictionary" || fail "daily word $first is not in the dictionary"

newer() { version_newer "$1" "$2" && echo yes || echo no; }
is "version: patch is newer" yes "$(newer 0.1.1 0.1.0)"
is "version: compares numbers, not text" yes "$(newer 0.10.0 0.9.9)"
is "version: equal is not newer" no "$(newer 1.2.3 1.2.3)"
is "version: never downgrades" no "$(newer 0.1.0 0.2.0)"
is "version: junk is not newer" no "$(newer 1.2.x 0.1.0)"

# why_blocked <SELF_DIR> [MANAGED_BY] -> why that copy won't update itself
why_blocked() {
  local SELF_DIR=$1 MANAGED_BY=${2:-}
  ST=()
  if WORDL_NO_UPDATE='' update_blocked; then echo "$REPLY"; else echo "may update"; fi
}
mkdir -p "$TMP/own/words" "$TMP/Cellar/wordl/1/libexec/words" "$TMP/bare" "$TMP/ro/words"
touch "$TMP/own/wordl" "$TMP/Cellar/wordl/1/libexec/wordl" "$TMP/ro/wordl"
chmod a-w "$TMP/ro/wordl"
is "update: a checkout is left alone" "running from a source checkout; use git pull" "$(why_blocked "$ROOT")"
is "update: a package's copy is left alone" "installed with Homebrew; use brew upgrade wordl" "$(why_blocked "$TMP/own" "Homebrew; use brew upgrade wordl")"
is "update: a Cellar path is left alone" "installed with Homebrew; use brew upgrade wordl" "$(why_blocked "$TMP/Cellar/wordl/1/libexec")"
[[ $(why_blocked "$TMP/bare") == "it was not installed with install.sh"* ]] && pass "update: an unknown layout is left alone" || fail "update: an unknown layout: $(why_blocked "$TMP/bare")"
if [[ -w $TMP/ro/wordl ]]; then
  echo "skip  update: a read-only copy (running as root, everything is writable)"
else
  [[ $(why_blocked "$TMP/ro") == "its directory is not writable"* ]] && pass "update: a read-only copy is left alone" || fail "update: a read-only copy: $(why_blocked "$TMP/ro")"
fi
if command -v curl >/dev/null 2>&1; then
  is "update: an install.sh copy may update" "may update" "$(why_blocked "$TMP/own")"
fi
is "update: WORDL_NO_UPDATE stops the check" "WORDL_NO_UPDATE is set" "$(SELF_DIR=$TMP/own MANAGED_BY='' WORDL_NO_UPDATE=1; update_blocked && echo "$REPLY")"
is "update: asking explicitly ignores WORDL_NO_UPDATE" "running from a source checkout; use git pull" "$(SELF_DIR=$ROOT WORDL_NO_UPDATE=1; update_blocked forced && echo "$REPLY")"
SELF_DIR=$ROOT

# ------------------------------------------------------------ word lists ----

for f in answers allowed; do
  bad=$(grep -cvxE '[a-z]{5}' "words/$f.txt")
  is "$f.txt: only five-letter lowercase words" 0 "$bad"
  LC_ALL=C sort -cu "words/$f.txt" 2>/dev/null && pass "$f.txt: sorted, no duplicates" || fail "$f.txt: not sorted or has duplicates"
done
is "answers are all accepted as guesses" 0 "$(LC_ALL=C comm -23 words/answers.txt words/allowed.txt | wc -l | tr -d ' ')"
((${#ANSWERS[@]} >= 1000)) && pass "answers: ${#ANSWERS[@]} words" || fail "answers: only ${#ANSWERS[@]} words"

# ---------------------------------------------------------------- layout ----

# Every size from tiny to huge: the game must fit whenever the terminal is at least
# 39x12 (a wide terminal can be shorter, with the keyboard beside the board), and
# nothing may be placed off screen or on top of something else.
problems=0 sizes=0
for ((ROWS = 1; ROWS <= 90; ROWS += (ROWS < 60 ? 1 : 5))); do
  for COLS in 1 10 38 39 40 45 50 59 60 61 68 70 72 79 80 81 90 100 107 108 109 120 132 150 160 190 200 220 250 300 400; do
    sizes=$((sizes + 1))
    layout
    if [[ $LAYOUT == small ]]; then
      ((COLS >= 39 && ROWS >= 12)) && { problems=$((problems + 1)); echo "  ${COLS}x${ROWS}: does not fit"; }
      continue
    fi
    why=''
    bw=$((4 * BPX + BW)) bh=$((5 * BPY + BT)) kw=$((9 * KPX + KW)) kh=$((2 * KPY + KT))
    hh=$((HV == 1 ? 1 : HV + 1))
    ((BX >= 1 && BX + bw - 1 <= COLS)) || why="board is off screen sideways"
    ((BY >= 1 && BY + bh - 1 <= ROWS - 1)) || why="board is off screen"
    ((KX >= 1 && KX + kw - 1 <= COLS)) || why="keyboard is off screen sideways"
    ((KY + kh - 1 <= ROWS - 1)) || why="keyboard overlaps the footer"
    ((HY >= 1 && HX >= 1)) || why="title is off screen"
    ((MSGY >= HY + hh && KY > MSGY)) || why="title, message and keyboard overlap"
    if [[ $LAYOUT == stacked ]]; then
      ((BY >= HY + hh && MSGY >= BY + bh)) || why="board overlaps the title or the message"
    else
      ((RX > BX + bw && HX >= RX && KX >= RX)) || why="board overlaps the side panel"
    fi
    [[ -n $why ]] && { problems=$((problems + 1)); echo "  ${COLS}x${ROWS} ($LAYOUT): $why"; }
  done
done
is "layout: $sizes sizes" 0 "$problems"

# A full render must not print a single error, whatever is on screen.
errors=$(
  {
    new_game practice
    add_guess SLATE
    CUR=CR
    for size in 39x12 80x24 60x20 100x30 120x40 190x50 250x70 30x8; do
      COLS=${size%x*} ROWS=${size#*x}
      layout
      for MODAL in '' help stats; do
        for theme in "${THEMES[@]}"; do
          set_theme "$theme"
          render
        done
      done
    done
    TRUECOLOR=0
    set_theme midnight
    render
  } 2>&1 >/dev/null
)
is "render: no errors at any size, in any theme or dialog" '' "$errors"

# ----------------------------------------------------------- self-update ----

# make_release <version> <dir>: a release of this tree, pretending to be <version>.
make_release() {
  local src=$TMP/src-$1 name
  mkdir -p "$src" "$2"
  cp -R wordl words LICENSE README.md packaging "$src/"
  sed "s/^VERSION=.*/VERSION=\"$1\"/" wordl >"$src/wordl"
  "$src/packaging/dist.sh" "$2" >/dev/null
  name=wordl-$1.tar.gz
  echo "$(sha256_of "$2/$name")  $name" >"$2/SHA256SUMS"
}
installed() { XDG_STATE_HOME="$TMP/ustate" "$BASH" "$TMP/bin/wordl" "$@" 2>&1; }
fresh_install() {
  WORDL_RELEASE_URL="file://$TMP/rel-now" WORDL_HOME="$TMP/home/wordl" WORDL_BIN_DIR="$TMP/bin" sh install.sh >/dev/null
}

if ! command -v curl >/dev/null 2>&1; then
  echo "skip  self-update (curl is not installed)"
else
  NOW=$(packaging/version.sh)
  make_release "$NOW" "$TMP/rel-now"
  make_release 99.0.0 "$TMP/rel-new"
  mkdir -p "$TMP/rel-bad"
  cp "$TMP/rel-new/wordl-99.0.0.tar.gz" "$TMP/rel-bad/"
  echo "0000000000000000000000000000000000000000000000000000000000000000  wordl-99.0.0.tar.gz" >"$TMP/rel-bad/SHA256SUMS"

  fresh_install
  is "install.sh: installs the release" "wordl $NOW" "$(installed --version)"
  is "update: nothing newer" "wordl $NOW is up to date (latest release is $NOW)." "$(WORDL_RELEASE_URL="file://$TMP/rel-now" installed update)"
  [[ $(WORDL_RELEASE_URL="file://$TMP/rel-bad" installed update) == *"checksum mismatch"* ]] && pass "update: refuses a bad checksum" || fail "update: a bad checksum was not refused"
  is "update: a refused update changes nothing" "wordl $NOW" "$(installed --version)"
  [[ $(WORDL_RELEASE_URL="file://$TMP/nowhere" installed update) == *"could not check for updates"* ]] && pass "update: reports an unreachable server" || fail "update: unreachable server"
  [[ $(WORDL_RELEASE_URL="file://$TMP/rel-new" installed update) == *"Updated to 99.0.0."* ]] && pass "update: installs a newer release" || fail "update: did not install the newer release"
  is "update: the new version runs" "wordl 99.0.0" "$(installed --version)"
  is "update: never downgrades" "wordl 99.0.0 is up to date (latest release is $NOW)." "$(WORDL_RELEASE_URL="file://$TMP/rel-now" installed update)"

  # What a package does when it installs: mark the copy as its own.
  fresh_install
  sed 's/^MANAGED_BY=""$/MANAGED_BY="Homebrew; use brew upgrade wordl"/' "$TMP/home/wordl/wordl" >"$TMP/marked"
  cat "$TMP/marked" >"$TMP/home/wordl/wordl"
  is "update: a copy marked by a package refuses" "wordl: this copy can't update itself: installed with Homebrew; use brew upgrade wordl" "$(WORDL_RELEASE_URL="file://$TMP/rel-new" installed update)"
  fresh_install
fi

# --------------------------------------------------------- the real game ----

if ! command -v tmux >/dev/null 2>&1; then
  echo "skip  the game in tmux (tmux is not installed)"
else
  SOCK=wordl-test-$$
  screen() { tmux -L "$SOCK" capture-pane -p 2>/dev/null; }
  expect() { # name text -- waits up to 5 seconds for the text to be on screen
    local i
    for ((i = 0; i < 50; i++)); do
      if screen | grep -qF -- "$2"; then pass "$1"; return; fi
      sleep 0.1
    done
    fail "$1: '$2' never appeared"
    screen | sed 's/^/        | /'
  }
  keys() { tmux -L "$SOCK" send-keys "$@"; }

  tmux -L "$SOCK" new-session -d -x 80 -y 24 \
    "env XDG_STATE_HOME='$TMP/xdg' WORDL_DEBUG_ANSWER=crane WORDL_NO_ANIM=1 '$BASH' '$ROOT/wordl'; echo \"EXIT=\$?\"; sleep 20"
  expect "game: starts" "W   O   R   D   L"
  keys '?'
  expect "game: help opens" "HOW TO PLAY"
  keys q
  expect "game: help closes" "Q   W   E   R   T"
  keys -l qqqqq; keys Enter
  expect "game: rejects a non-word" "Not in word list"
  keys BSpace BSpace BSpace BSpace BSpace
  keys -l slate; keys Enter
  keys -l cran
  # A click on the E key, then on Enter (bottom right): SGR mouse press at column;row.
  keys -l $'\e[<0;30;21M'
  sleep 0.2
  keys -l $'\e[<0;58;23M'
  expect "game: mouse click types and submits; the game is won" "Solved in 2/6"
  expect "game: statistics open after a win" "STATISTICS"
  keys Escape
  sleep 0.3 # Escape followed at once by a key would arrive as Alt+key
  tmux -L "$SOCK" resize-window -x 30 -y 8 2>/dev/null
  expect "game: a tiny window says so" "wordl needs 39x12"
  keys C-q
  expect "game: quits cleanly" "EXIT=0"
  is "game: the win is saved" "practice_wins=1" "$(grep -x 'practice_wins=1' "$TMP/xdg/wordl/stats" 2>/dev/null)"

  # Starting the installed copy when a newer release exists: it updates, then the game
  # starts. Switched off, it starts without updating.
  if command -v curl >/dev/null 2>&1; then
    tmux -L "$SOCK" kill-server 2>/dev/null
    launch() { # name -- starts the installed copy with a newer release on offer
      tmux -L "$SOCK" new-session -d -x 80 -y 24 \
        "env XDG_STATE_HOME='$TMP/ustate' WORDL_RELEASE_URL='file://$TMP/rel-new' WORDL_NO_ANIM=1 '$BASH' '$TMP/bin/wordl'; echo \"EXIT=\$?\"; sleep 20"
      expect "$1" "W   O   R   D   L"
      keys C-q
      expect "$1, and quits" "EXIT=0"
      tmux -L "$SOCK" kill-server 2>/dev/null
    }
    installed update off >/dev/null
    launch "update: switched off, the game starts"
    is "update: switched off, nothing is updated" "wordl $NOW" "$(installed --version)"
    installed update on >/dev/null
    launch "update: the game starts after updating itself"
    is "update: starting the game installed the newer release" "wordl 99.0.0" "$(installed --version)"
  fi
fi

echo
if ((fails)); then
  echo "$fails failed"
  exit 1
fi
echo "all passed"
