# wordl

A Wordle-style word game for the terminal: one bash script (`wordl`) plus two word
lists. Targets macOS and Linux, bash 4.4 or newer. `README.md` is for players; this file
is for whoever changes the code.

## Commands

```sh
./wordl                       # run from the checkout (uses words/ beside the script)
tests/run.sh                  # everything: rules, word lists, layout sweep, the game in tmux
/path/to/bash tests/run.sh    # the same with another bash (the tests use the bash that runs them)
shellcheck wordl tests/run.sh install.sh packaging/*.sh packaging/aur/render.sh .github/scripts/*.sh
packaging/dist.sh <dir>       # build wordl-<version>.tar.gz, the release archive
tools/build-words.py <scowl>  # regenerate words/ from a SCOWL download
tools/screenshot.py <tmux socket> assets/screenshot.png   # redraw a README picture
```

CI fails on any shellcheck finding, including notes. `shellcheck` is not installed on
the user's Arch machine by default (a static binary from its GitHub releases works).

## Workflow

- **`main` only accepts pull requests** (GitHub ruleset "Main"): no direct pushes, no
  force-pushes, no deletion, no bypass for anyone. A PR needs these checks to pass,
  matched by job name: `test (ubuntu-latest)`, `test (macos-latest)`, `bash 4.4`,
  `release checklist`. Renaming a CI job means updating the ruleset or PRs wait forever.
  PRs are squash-merged.
- **Local layout.** The user keeps this repo as a bare clone with one worktree per
  branch: `~/Developer/GitHub/anwarahmed/wordl/main` plus a sibling directory per
  feature branch (`git worktree add -b <branch> <branch> origin/main` from the bare
  repo). Remove the worktree and branch after the PR merges, then fast-forward `main`.
- **Releasing** is merging a bump of `VERSION` at the top of `wordl`; a merge without
  one publishes nothing. **Follow [RELEASING.md](RELEASING.md) every time, every
  step.** The release PR's description must carry its checklist with every line ticked
  (`gh pr create --body` does not add it for you), or the `release checklist` check
  fails. Tick a line only after doing what it says. Then do its "After merging" steps
  and report each one.
- **Sibling repo:** https://github.com/anwarahmed/homebrew-tap holds the generated
  Homebrew formula (`Formula/wordl.rb`, written by its `scripts/formulae/wordl.sh`). It
  takes direct pushes, because its bot commits formulae to `main`.

## How the script is laid out

One file, in sections marked by `# ---- name ----` rulers, top to bottom:

| Section   | What is in it |
|-----------|---------------|
| fonts     | Two bitmap fonts (5x5 and 5x7 pixels per letter) and the Enter/Backspace icons |
| colors    | `set_theme`: every theme fills the `F[...]`/`B[...]` tables (foreground/background SGR codes per role) |
| drawing   | `block` (one tile or key), `glyph` (bitmap to half-block rows, cached), `put_center` |
| layout    | `dims`, `fit`, `layout`: pick sizes and positions for the current terminal size |
| rendering | `draw_title`, `draw_tile`, `draw_keyboard`, `draw_msg`, `draw_footer`, `render` |
| modals    | Help and statistics dialogs, built as lists of lines |
| game      | Word lists, `evaluate`, `hard_check`, `new_game`, statistics, save files |
| animation | `shake`, `reveal`, `celebrate` |
| actions   | What keys and clicks do: `submit`, `do_cmd`, `click`, `handle_key` |
| update    | Self-update: `update_blocked`, `latest_release`, `install_release`, `update_before_start`, `update_command` |
| input     | `read_key`: one key or mouse event per call |
| main      | Options, terminal setup and teardown, the event loop |

### Patterns to keep

- **Everything drawn goes through `OUT`.** Functions append escape sequences to the
  global `OUT` and `flush` prints it in one write, inside synchronized-output markers
  for full renders. Don't `printf` to the terminal from drawing code.
- **Sizes are levels.** A level (1, 2, 3, 5..16) is the height of a tile or key in
  rows; `dims` turns it into width and spacing. 1 to 3 are text (a character on a
  colored cell); 5 and up are pixel art. There is no level 4: it has too few rows for
  the 5-pixel font and one too many for text.
- **Pixels are half cells.** A terminal cell is about twice as tall as wide, so `▀` and
  `▄` give two square pixels per cell. A block's first and last rows are half rows
  (`▄` on top, `▀` at the bottom, in the tile's color on the screen background), which
  is what puts a gap between tiles without spending a row on it. `glyph` renders a
  letter once per size into colorless rows and caches them in `GC`; the color is set
  around the row when it is drawn.
- **Layout is recomputed, never patched.** A resize sets a flag (`trap ... WINCH`); the
  loop then runs `get_size`, `layout`, `render`. `layout` tries the keyboard under the
  board ("stacked") and beside it ("side") and keeps whichever allows bigger tiles.
- **Themes carry roles, not colors.** Code uses `${F[g]}`, `${B[key]}`, `${F[dim]}`;
  only `set_theme` knows RGB values. `terminal` uses ANSI palette codes, so anything
  drawn must work when a role's "color" is the terminal default.
- **Click targets are rectangles recorded while drawing.** `HITS` (game) and `MHITS`
  (open dialog) hold `x1 y1 x2 y2 action`; `click` looks the point up. A full `render`
  rebuilds them; partial redraws must not add to them (`draw_keyboard` takes a flag).
- **Letters are never commands.** Every letter key types, so commands are Ctrl keys or
  `?`. Inside a dialog letters are free (`C` copies, `N` starts a game).
- **Animations sleep with `nap`**, which reads from a spare file descriptor, not from
  the keyboard, so keys typed during an animation queue up instead of being eaten or
  split mid escape sequence.
- **Saved files are parsed, not sourced.** `load_kv` accepts only `key=value` lines of
  plain characters. Values end up in arithmetic, where bash would otherwise evaluate
  whatever a file contains.
- **Dialog lines are at most 35 characters**, so a dialog fits the narrowest supported
  terminal (39 columns). `ml` takes the visible length separately because lines carry
  escape codes.
- **The file can be sourced.** `main` only runs when the script is executed; that is
  how `tests/run.sh` calls its functions.

## Decisions and why

- **Bash, one file.** The user asked for a bash-based TUI. Staying in one script with
  no dependencies beyond `stty` keeps it installable anywhere bash is.
- **Bash 4.4 is the floor** (associative arrays, namerefs, `${var^^}`, fractional
  `read -t`). macOS ships 3.2, so the script checks the version on its first lines,
  before anything 3.2 can't parse, and the Homebrew formula depends on `bash` and
  rewrites the shebang to it. CI's `bash 4.4` job runs the tests on exactly 4.4; raise
  the check and the job together.
- **Scaling by block art.** Terminal text can't be resized, and the user asked for the
  game to fill the terminal, so big letters are bitmaps. Two fonts give more usable
  sizes than one: `font_for` picks whichever font and whole-number scale fills a tile
  best.
- **Keyboard beside the board on wide terminals.** Height is the scarce dimension; a
  side keyboard lets the board use all of it.
- **Backspace is bottom-left and Enter bottom-right**, the reverse of Wordle. The user
  asked for this.
- **Every launch is a new random word.** The user asked for this after the first
  version resumed the last game. Practice games are never saved. The daily puzzle is
  the exception and resumes, because there is one word a day; its word is picked by
  `(day * 7919 + 104729) % number of answers`, with the day counted in local time, so
  changing `words/answers.txt` changes the daily word.
- **Three difficulties**, specified by the user: Normal; Hard (green stays, yellow is
  reused); Ultra Hard (also: yellow must move, gray is obeyed). For repeated letters,
  gray means "no more copies than this guess showed as green or yellow". A game under
  way can be made easier but not harder, since earlier guesses weren't held to the
  stricter rule.
- **Own word lists, from SCOWL.** Not the original game's lists: SCOWL is permissively
  licensed and its notice ships in `words/SCOWL-COPYRIGHT`. `answers.txt` is filtered
  by rule (plurals, inflections) plus a block list in `tools/build-words.py`, so an odd
  word can still slip through; add it to the block list and rebuild.
- **Guess lookup is a substring match** on the whole list held in one string. It takes
  under a millisecond and loads much faster than an associative array of 11,000 keys.
- **Default theme is `midnight`** with its own background, so the game looks the same
  everywhere; `terminal` is there for following the terminal's theme (the user runs
  Omarchy, which themes the terminal). 256-color terminals get the nearest palette
  color (`pal`).
- **State lives in `$XDG_STATE_HOME/wordl`** on both platforms (`stats` and `daily`),
  not `~/Library` on macOS.
- **Releases are versioned; a release is cut by merging a version bump**, as in the
  user's typeshelf project: the repo is public, so other people get a stable target.
  `VERSION` in the script is the single source; `packaging/version.sh` reads it.
- **The release is one archive**, `wordl-<version>.tar.gz`, unpacking to
  `wordl-<version>/` with `wordl`, `words/`, `LICENSE` and `README.md`. The installer,
  the AUR package and the Homebrew formula all rely on that name and layout.
- **The script finds its word lists** in `$WORDL_DATA_DIR`, then `words/` beside
  itself (after resolving symlinks: checkout, install script, Homebrew's `libexec`),
  then `../share/wordl` (the AUR package: `/usr/bin` and `/usr/share/wordl`).
- **Self-update on start** (asked for by the user, as in typeshelf; 0.1.0 had none).
  `update_before_start` reads the latest release's `SHA256SUMS` (3 s timeout, silent
  when offline); the archive's name in it gives the version. If that is higher than
  `VERSION` it downloads the archive, checks the checksum, runs the new script's
  `--version` as a sanity check, copies the game's files over this copy's one by one
  (written beside the target and renamed, script last), and re-execs with
  `WORDL_NO_UPDATE=1` so it can't loop. It replaces files, never the directory, in
  case the user put the script somewhere with other things in it. Any failure keeps
  the current version. It never downgrades. `wordl update` does the same on demand and
  says why when it can't; `wordl update off` stores `update=0` in the stats file.
- **Package managers switch self-update off by rewriting one line.** The user asked
  for the Homebrew tap to be what disables it. The script has `MANAGED_BY=""`; the
  Homebrew formula (`inreplace`) and the AUR `PKGBUILD` (`sed`) set it to their name and
  upgrade hint when they install, and `update_blocked` then refuses with "installed
  with ...". Both fail the install if the line is not found, so **never reformat that
  line** (nor the shebang, which the formula also rewrites). Behind that sit fallbacks:
  a real path containing `Cellar` (symlinks resolved first; typeshelf once replaced
  Homebrew's link on macOS because it had not resolved them), a `.git` beside the
  script (a checkout), no `words/` beside it, or files that are not writable. The
  tap's workflow runs `wordl update` on a Homebrew install on both platforms and fails
  unless it refuses.
- **Actions are pinned to commit hashes** with the version in a trailing comment;
  `.github/dependabot.yml` proposes newer pins monthly. Pin any action you add.
- **Packaging.** Three channels, all fed by the release:
  - *Install script* - `install.sh`, POSIX sh: latest release into
    `~/.local/share/wordl`, link in `~/.local/bin`, checksum verified. This is the one
    copy that updates itself. `WORDL_RELEASE_URL` points both the installer and the
    updater at another location (a directory with `SHA256SUMS` and the archive;
    `file://` works). The tests build two fake releases that way and update between
    them, and the release workflow installs from one on every packaging PR.
  - *Homebrew* - `anwarahmed/homebrew-tap`. The `TAP_TOKEN` secret here lets a release
    start the tap's workflow and wait for the formula; without it the release run
    carries a "Homebrew tap not notified" warning and the tap catches up on its
    three-hourly schedule or by hand.
  - *AUR* - package `wordl`, rendered by `packaging/aur/render.sh` from `PKGBUILD.in`
    and `SRCINFO.in` (change them together; check with
    `makepkg --printsrcinfo | diff - .SRCINFO` on Arch). **Not published:** the user has
    no AUR account (registration was closed in October 2026). Until then the `PKGBUILD`
    attached to each release is installed with `makepkg -si`. The workflow pushes to
    the AUR once an `AUR_SSH_PRIVATE_KEY` secret exists; then switch the README to
    `yay -S wordl`.

## Verifying changes

`tests/run.sh` covers the rules, the layout arithmetic at every size, a full render in
every theme and dialog (it fails on anything printed to stderr, which is how a bash
"bad substitution" inside a drawing function shows up), and a short real game in tmux
with a mouse click and a resize.

It cannot judge how the screen looks. For that, run the game in a detached tmux
session with a throwaway state directory and read the screen, or draw it:

```sh
X=$(mktemp -d)
tmux -L w new-session -d -x 150 -y 46 \
  "XDG_STATE_HOME=$X COLORTERM=truecolor WORDL_DEBUG_ANSWER=crane ./wordl"
tmux -L w send-keys -l slate; tmux -L w send-keys Enter
tmux -L w capture-pane -p            # add -e to see colors
tools/screenshot.py w /tmp/shot.png  # a PNG of the pane
tmux -L w resize-window -x 39 -y 12  # then look again
tmux -L w kill-server
```

`WORDL_DEBUG_ANSWER` fixes the practice word; `WORDL_NO_ANIM=1` skips animations.
A checkout never updates itself, so to try the updater by hand use an installed copy:
`WORDL_HOME=$X/home WORDL_BIN_DIR=$X/bin sh install.sh`, then run `$X/bin/wordl` with
`WORDL_RELEASE_URL` pointing at a directory made the way `make_release` in
`tests/run.sh` makes one. A
mouse click is `tmux send-keys -l $'\e[<0;COL;ROWM'`.

Traps when scripting tmux: `send-keys Escape` immediately followed by another key
arrives as Alt+key, so pause between them; `pkill -f wordl` also matches the shell
running it; and never test the copy function against the real clipboard (put a fake
`wl-copy` first on `PATH`).

## Known gaps and ideas

- Never run on a real Mac or in macOS Terminal.app; CI runs the tests on a macOS
  runner with Homebrew's bash, which is not the same as someone looking at it.
- The README pictures are drawn by `tools/screenshot.py` from tmux's cell data, not
  captured from a terminal window.
- A resize during an animation is handled when the animation ends.
- The update check runs on every start and adds a network round trip (about 0.4 s
  measured on the user's machine) before the game appears; there is no once-a-day limit.
- Copies of 0.1.0 have no updater; they need `install.sh` run once more.
- At the smallest sizes (level 1) tiles in a column touch; there is no room for gaps.
- Dialogs taller than the terminal lose their last lines.
- No hover effects, no key-press flash on the on-screen keyboard.
- `answers.txt` is machine-filtered and has not been read through by a person.
- Not done: other word lengths, other languages, sharing as an image.
