# wordl

A keyboard-first Wordle-style word game for the terminal (TUI), with mouse support. It
fills the terminal and rescales with it. Rust + ratatui, targets macOS and Linux.
`README.md` is for players; this file is for whoever changes the code.

## Commands

```sh
cargo run --release                          # play (from a checkout it never updates itself)
cargo test                                   # unit tests: rules, word lists, layout sweep, drawing, store, update
cargo clippy --all-targets -- -D warnings    # CI fails on any warning
cargo fmt                                    # rustfmt.toml: max_width 160
cargo build --release && tests/e2e.sh        # the built program in tmux, install.sh, the updater
tools/build-words.py <scowl>                 # regenerate words/ from a SCOWL download
tools/screenshot.py <tmux socket> assets/screenshot.png   # redraw a README picture
```

`install.sh` (POSIX sh, macOS + Linux) downloads the latest release binary into
`~/.local/bin` (`WORDL_BIN_DIR` overrides) and verifies its checksum. `--source` builds
instead (the checkout it is in, else a fresh clone), and it falls back to that when no
binary exists for the platform. `--link` symlinks to the checkout's build, `--uninstall`
removes it. It never installs Rust and never edits shell profiles. `WORDL_RELEASE_URL`
points it, and the self-updater, at another download base (a `file://` directory holding
`VERSION`, `SHA256SUMS` and a binary; `tests/e2e.sh` makes such directories).

## Workflow

- **`main` only accepts pull requests** (GitHub ruleset "Main"): no direct pushes, no
  force-pushes, no deletion, no bypass for anyone. A PR needs these checks to pass,
  matched by job name: `test (ubuntu-latest)`, `test (macos-latest)`, `msrv`,
  `release checklist`. Renaming a CI job means updating the ruleset or PRs wait forever.
  PRs are squash-merged.
- **Local layout.** The user keeps this repo as a bare clone with one worktree per
  branch: `~/Developer/GitHub/anwarahmed/wordl/main` plus a sibling directory per
  feature branch (`git worktree add -b <branch> <branch> origin/main` from the bare
  repo). Remove the worktree and branch after the PR merges, then fast-forward `main`.
- **Releasing** is merging a version bump; a merge without one publishes nothing.
  **Follow [RELEASING.md](RELEASING.md) every time, every step.** The release PR's
  description must carry its checklist with every line ticked (`gh pr create --body`
  does not add it for you), or the `release checklist` check fails. Tick a line only
  after doing what it says. Then do its "After merging" steps and report each one.
- **Sibling repo:** https://github.com/anwarahmed/homebrew-tap holds the generated
  Homebrew formula (`Formula/wordl.rb`, written by its `scripts/formulae/wordl.sh`). It
  takes direct pushes, because its bot commits formulae to `main`.
- **Sibling repo:** https://github.com/anwarahmed/funwordl is a second game, more
  playful (sounds, hints, stars, many bright themes, an Easy level that takes any
  five letters as a guess), built on this crate's library half (see "A library
  and a game" below). It names this repository at a commit in its `Cargo.toml`, so a
  change to `game.rs`, `store.rs`, `update.rs`, `words.rs` or `words/` reaches it only
  when that commit is raised there. Before merging a change to what those files make
  public, build funwordl against the branch (a `[patch]` or a `path` dependency in a
  funwordl worktree) so it is not left unable to move forward.

## Architecture

One crate, no async, in two halves: a library (`lib.rs`) holding what does not depend
on the screen, and the game (`main.rs`), which is one user of it. One file per concern
in `src/`:

| File        | Half    | Role |
|-------------|---------|------|
| `lib.rs`    | library | Declares the four library modules and says what they are for |
| `game.rs`   | library | The rules: `evaluate`, `check_clues` (the difficulties), `Game` for one game |
| `words.rs`  | library | The two word lists and the definitions, embedded with `include_str!` |
| `store.rs`  | library | `Stats` (statistics and settings) and the saved daily puzzle, as `key=value` files |
| `update.rs` | library | Startup self-update and `wordl update`, for whichever `Program` asks |
| `main.rs`   | game    | CLI options, terminal setup/teardown, event loop |
| `app.rs`    | game    | `App` state, all key and mouse handling, what each action does, animation timing |
| `ui.rs`     | game    | All drawing, and the geometry that says what a click landed on. Pure functions of `&App` |
| `layout.rs` | game    | `layout(cols, rows)`: sizes and positions for a terminal size |
| `font.rs`   | game    | Two bitmap fonts and `glyph`, which turns a letter into rows of half blocks |
| `theme.rs`  | game    | Color themes as roles; 256-color fallback |

`main.rs` imports the library modules at its top (`use wordl::{game, store, words}`),
so the game's modules reach them as `crate::game` and so on.

### Patterns to keep

- **The library does not know its name.** funwordl compiles the same code, so nothing
  in the four library modules may say "wordl" where the program's own name is meant,
  or read `CARGO_PKG_VERSION` (inside a dependency that is wordl's version, not the
  game's). The binary describes itself in a `update::Program` (name, version,
  repository), from which the updater derives the release address, the asset names
  (`<name>-<rust target>`), the environment variables (`<NAME>_NO_UPDATE`,
  `<NAME>_RELEASE_URL`) and the marker file (`share/<name>/managed-by`);
  `store::state_dir` takes the name too. `build.rs`'s commit stamp is likewise read
  only in `main.rs`.
- **What the library makes public is a promise to funwordl.** Add to it freely;
  renaming or removing something public means changing funwordl in step.
- **State / view split.** `app.rs` owns state and input; `ui.rs` only reads. Nothing is
  remembered about the screen: every frame is drawn from the state and the size.
- **The rules are pure.** `game.rs` has no I/O and no clock, so it is unit-tested
  directly. Keep the terminal, the disk and time out of it.
- **Layout is recomputed, never patched.** `layout::layout` is a pure function of the
  terminal size, called every frame; a resize needs no handling of its own. It tries
  the keyboard under the board ("stacked") and beside it ("side") and keeps whichever
  allows bigger tiles. Its test walks every size up to 420x100 and checks nothing
  overlaps or leaves the screen: run it after touching the arithmetic.
- **Sizes are levels.** A level (1, 2, 3, 5..16) is the height of a tile or key in
  rows; `Dims::of` turns it into width and spacing. 1 to 3 are text (a character on a
  colored cell); 5 and up are pixel art. There is no level 4: it has too few rows for
  the 5-pixel font and one too many for text.
- **Pixels are half cells.** A terminal cell is about twice as tall as wide, so `▀` and
  `▄` give two square pixels per cell. A block's first and last rows are half rows
  (`▄` on top, `▀` at the bottom, in the tile's color on the screen background), which
  is what puts a gap between tiles without spending a row on it. `Canvas::block` draws
  every tile, key and title letter.
- **Filled blocks are pixel art with a color per pixel** (asked for by the user after
  comparing nine ways of drawing tiles in a demo; since 0.2.6). `Canvas::sprite` draws
  a revealed tile, a key or a title letter with a lit top and left edge, a dark bottom
  and right edge, and a letter that casts a shadow. Each cell is `▀` in the color of
  its upper pixel on a background in the color of its lower pixel, so it needs no
  more than the characters the game already used and works in any terminal with
  enough colors. `Theme::shades` derives the lighter and darker colors from a role's
  RGB. Things to keep:
  - The `terminal` theme has no RGB to shade (`Paint::rgb` is `None`), so its blocks
    stay flat: the code path before `sprite` in `Canvas::block` must keep working.
  - Frames (empty and typed tiles) are not shaded; only filled blocks are.
  - A key known not to be in the word is drawn `sunken` (edges swapped, no shadow).
  - At level 3 there is one row of text, so only the top and bottom edges are shaded.
  - A dark letter on a bright tile (the `neon` and `contrast` themes) gets a light
    "shadow" instead of a dark one.
  - Real images (the kitty graphics protocol) were tried in the demo and rejected:
    the user's usual terminal does not show them.
- **All coordinates are `i32` and every write is clipped.** `Canvas::put` drops what is
  off screen, so drawing code never checks bounds and never panics on a tiny window.
  It also resets a cell before writing, so a dialog drawn over bold tiles is not bold.
- **Clicks use the drawing's geometry.** `ui::keyboard_keys`, `ui::footer_items` and
  `dialog_box` position things both for `draw` and for `ui::action_at`. Don't compute a
  position in two places.
- **Keys and clicks become `Action`s** and both run through `App::act`.
- **Letters are never commands.** Every letter key types, so commands are Ctrl keys or
  `?`. Inside a dialog letters are free (`C` copies, `N` starts a game).
- **Animations are functions of time.** `App` stores when one started; `ui` works out
  the frame from `app.now`. While one runs the event loop draws at ~60 fps and leaves
  keys in the terminal's queue, so typing ahead is kept and nothing acts before the
  player has seen the result. The loop stops draining keys when one starts an animation.
- **Themes carry roles, not colors.** Code uses `th.g`, `th.key`, `th.dim`; only
  `theme.rs` knows RGB values. A role is a `Paint` with a foreground and a background
  form, because in the `terminal` theme they can differ (an absent key is gray text on
  the terminal's own background). Anything drawn must work when a color is `Reset`.
- **Frames reach the terminal in one write.** ratatui draws into `main::FrameWriter`,
  which sends a frame at once, as a synchronized update, and skips one identical to
  the last. Don't write to stdout between frames (the clipboard's OSC 52 fallback is
  the one exception).
- **Dialog lines are at most 35 characters**, so a dialog fits the narrowest supported
  terminal (39 columns). A test checks it.
- **Saved files are parsed strictly.** `store::read_pairs` accepts only `key=value`
  lines of plain characters and ignores the rest.
- **Tests live beside the code** in `#[cfg(test)] mod tests`. They use throwaway
  directories under the system temp directory, never the real state directory.

## Decisions and why

- **A library and a game** (since the commit after 0.2.8). The user wanted a second,
  more playful game for the children, funwordl, without giving up this one, and asked
  for the shared core to stay in sync. So the core is a library here and funwordl
  depends on it: a wrong word or a clumsy definition is fixed once, in this
  repository, and funwordl picks it up by pointing at the newer commit. What is
  shared is the rules, the word lists and definitions, the saved-file format and the
  updater; what a game looks like and how it is played (`app`, `ui`, `layout`,
  `font`, `theme`) is each game's own. Considered and not chosen: a third repository
  holding only the core (a third set of rulesets and releases, and every word fix
  touching three repositories), and one repository building both games (it gives up
  the separate repository the user asked for and muddles "merging a version bump is
  a release"). The one rule added for funwordl's sake is the number of guesses:
  `Game::tries`, six unless `with_tries` says otherwise, saved with the daily puzzle
  as `tries` (absent means six, so older files and older versions are unaffected).
  Here it is always six. funwordl's Easy level had eight guesses up to its 0.1.2;
  since 0.1.3 every funwordl game has six too, and it uses `tries` only to open a
  daily puzzle that an older copy saved with eight. Keep the field: those saved
  files exist.
- **Rust + ratatui, like the user's typeshelf.** The first version (0.1.x) was a bash
  script, because the request said "bash-based"; the user later clarified that meant
  "runs in a terminal", not "written in bash", and asked for a rewrite in Rust keeping
  everything. A single static binary removed the need for bash 4.4 (macOS ships 3.2),
  and input, mouse and resize handling no longer had to be done by hand. The bash
  script is in the git history up to v0.1.2; the screen it drew is what this version
  draws, cell for cell.
- **Scaling by block art.** Terminal text can't be resized and the user asked for the
  game to fill the terminal, so big letters are bitmaps. Two fonts (5x5 and 5x7 pixels)
  give more usable sizes than one: `font_for` picks whichever font and whole-number
  scale fills a tile best.
- **Keyboard beside the board on wide terminals.** Height is the scarce dimension; a
  side keyboard lets the board use all of it.
- **Backspace is bottom-left and Enter bottom-right**, the reverse of Wordle. The user
  asked for this.
- **Every launch is a new random word.** The user asked for this after the first
  version resumed the last game. Practice games are never saved. The daily puzzle is
  the exception and resumes, because there is one word a day; its word is
  `(day * 7919 + 104729) % number of answers`, with the day counted in local time, so
  changing `words/answers.txt` changes the daily word.
- **The result dialog stays up until `N`, `C` or `Esc`; `N` starts the next word**
  (asked for by the user, in two steps). Enter used to start the next word, but Enter
  also submits guesses and opens the statistics of a finished game, so one press too
  many threw away the result, and with it the word's meaning, before anyone had read
  it. 0.2.7 made Enter do nothing there and gave the next word to `N`; any other key
  still closed the dialog, and the user then asked for that to stop too (0.2.8). So in the
  statistics dialog of a finished game every other key is ignored, and so is a click
  outside the dialog; its three buttons can be clicked. `Ctrl-Q`, `Ctrl-T` and `Ctrl-L`
  work as everywhere. The statistics of a game under way (`Ctrl-S`) and the help still
  close on any key. The give-up question keeps Enter: it is asked for with `Ctrl-G`,
  never reached by accident.
- **Three difficulties**, specified by the user: Normal; Hard (green stays, yellow is
  reused); Ultra Hard (also: yellow must move, gray is obeyed). For repeated letters,
  gray means "no more copies than this guess showed as green or yellow". A game under
  way can be made easier but not harder, since earlier guesses weren't held to the
  stricter rule.
- **Giving up** (`Ctrl-G`, asked for by the user: on Hard and Ultra Hard a game can get
  stuck). It asks first, because one stray key would otherwise end a game. It counts as
  a loss (a free way out would make the statistics meaningless), shows the answer in
  the next empty row and in the message, and leads to the statistics dialog, where
  `N` starts the next word. `Game::gave_up` is separate from `Status::Lost` only so
  the screen can say so and so a given-up daily puzzle is not resumed.
- **Own word lists, from SCOWL.** Not the original game's lists: SCOWL is permissively
  licensed and asks for its notice to travel with copies, so `wordl --licenses` prints
  it and the AUR package installs it. `answers.txt` is filtered by rule (plurals,
  inflections) plus a block list in `tools/build-words.py`, about 500 words grouped by
  reason. **The game is played by children** (the user said so): up to 13 years old,
  the older ones very well read, the younger ones usually playing with the older ones
  helping. The user also sees it as **an opportunity for them to learn new words**. The
  whole list was read through at the user's request and adjusted over 0.2.2 to 0.2.4.
  So the answers are base words that are safe to hand a child, and hard ones are
  welcome:
  - no inflections at all: plurals, past tenses and participles even when irregular
    (`began`, `wrote`), comparatives even when everyday (`older`, `safer`);
  - nothing sexual, vulgar or bodily, no slurs, no insults even mild (`dunce`), no
    remarks about bodies (`obese`, `pudgy`), no labels for kinds of people (`pagan`);
  - no harm or crime as a theme (`abuse`, `arson`, `rifle`), no drink, tobacco, drugs
    or gambling (`vodka`, `cigar`, `poker`), nothing romantic or suggestive (`lover`,
    `naked`);
  - no names, slang, British-only words or jargon (`halon`, `infix`);
  - **difficult vocabulary stays in** (`skein`, `tacit`, `abhor`, `wrest`). 0.2.3
    blocked 220 such words on a misreading of "hard"; 0.2.4 brought nearly all back
    once the user described the players. A word is not blocked for being hard, only
    for being unsuitable or not worth learning.
  Spooky and everyday-serious words stayed (`ghost`, `skull`, `death`, `sword`,
  `thief`). When unsure whether a word is *suitable*, block it; when unsure whether it
  is *too hard*, keep it. To block one, add it to the fitting group in the script and
  rebuild. Blocked words stay valid guesses. Changing the list changes which word each
  day's puzzle is. Both lists must stay sorted:
  guesses are looked up by binary search, and a test checks the order.
- **Every puzzle word has a definition, written for this game** (asked for by the
  user, so that the children learn the words they meet; since 0.2.5).
  `words/definitions.txt` is `word<TAB>meaning`, one line per answer in the same order
  as `answers.txt`; a test fails if the two differ, and `tools/build-words.py` lists
  what is missing after a rebuild. When a game ends the statistics dialog shows the
  word with its meaning (`ui::dialog`, wrapped by `ui::wrap`).
  - *Why not a dictionary:* WordNet was tried. Its license allows bundling and it
    covers all but 16 of the words, but its first sense is often the wrong one for a
    child ("crane": "stretch the neck"; "swear": "utter obscenities"), adult senses
    are mixed in ("screw"), and the wording is a dictionary's. Simple English
    Wiktionary is share-alike. Every entry would have needed rewriting by hand
    anyway, so they were written from scratch, with no license to carry.
  - *How to write one:* at most 70 characters (it must fit the 35-column dialog in
    three lines with the word in front; a test checks), lowercase, no full stop.
    Plain words a young child knows. The sense worth learning, and a second one after
    a semicolon only when both are common ("a tall bird with long legs; a machine
    that lifts"). Verbs start with "to". Never the unsuitable sense: "swear" is "to
    make a solemn promise". Don't define a word with itself.
  - Adding or unblocking an answer means writing its definition in the same change.
- **The state files are the bash version's.** `$XDG_STATE_HOME/wordl/stats` and `daily`
  kept their `key=value` format so nobody's statistics were lost in the rewrite. XDG
  paths on macOS too, not `~/Library`.
- **Ten themes, four of them bright** (`daylight`, `paper`, `sky`, `candy`; the user
  asked for more themes and for bright backgrounds in 0.2.7, which added `ocean`,
  `ember` and the last three). A theme is 23 colors in `theme::theme` plus its name in
  `NAMES`, the README and `USAGE`. A test checks that text stands out from what it is
  on in every theme, but only looking shows whether it is pleasant, in truecolor and
  without `COLORTERM` (the nearest of 256 colors can turn cream into pink).
- **Default theme is `midnight`** with its own background, so the game looks the same
  everywhere; `terminal` is there for following the terminal's theme (the user runs
  Omarchy, which themes the terminal). Terminals that don't announce truecolor
  (`COLORTERM`) get the nearest of 256 colors.
- **The mouse is captured** for clicks on keys and buttons. Capture is turned off on
  exit and in the panic hook, since `ratatui::restore` doesn't do it. The cost:
  selecting text in the terminal needs shift (option in macOS Terminal) while it runs.
- **Releases are versioned; a release is cut by merging a version bump.** The repo is
  public, so other people get a stable target. To release: bump `version` in
  `Cargo.toml` (and `Cargo.lock`, via any cargo command) in a PR. On merge,
  `.github/workflows/release.yml` sees there is no `v<version>` tag, builds four
  binaries, and publishes a GitHub release with them, `SHA256SUMS`, `VERSION` and the
  rendered `PKGBUILD`. The same workflow runs build-only on PRs that touch packaging.
- **Release assets are bare binaries**, named `wordl-<rust target>`, not archives:
  `x86_64-` and `aarch64-unknown-linux-musl` (static, so one file runs on every
  distro), `aarch64-` and `x86_64-apple-darwin`. Plus `VERSION`, a text file holding
  the version. Renaming assets breaks installed copies' updates, the install script,
  the Homebrew formula and the AUR package at once. (0.1.x published one archive,
  `wordl-<version>.tar.gz`; copies of 0.1.x cannot update themselves past it and need
  `install.sh` run once.)
- **Self-update on start** (asked for by the user). `update::before_start` downloads
  `VERSION` from the latest release (3 s timeout, silent when offline); if it is higher
  than `CARGO_PKG_VERSION` it downloads the asset for this platform, checks it against
  `SHA256SUMS`, renames it over the running binary and re-execs with
  `WORDL_NO_UPDATE=1` so it can't loop. Any failure keeps the current version. It never
  downgrades. It does not use the GitHub API: without a token the API allows 60
  requests an hour per address, which a check on every start can use up on a shared
  network, and a refused check is silent. A plain download has no such limit, and it
  lets the tests point the updater at a `file://` directory. (typeshelf's updater did
  use the API; it was changed to this in its 0.2.5, along with the marker file below.
  The two updaters are now the same design: fix a flaw in one, fix it in both.)
  - *At most one check a day* (asked for by the user; since 0.2.1). Checking on every
    start put a network round trip, about 0.4 s, in front of the game each time, for
    releases that come rarely. A check that gets an answer and finds nothing newer
    writes the time to `last-update-check` in the state directory, and for 24 hours
    the start skips the check. Only an answer is noted: a failed check (offline) is
    tried again at the next start, and so is a failed install. A noted time in the
    future (the clock was set back) does not count. `wordl update` always checks. To
    see the check happen again, delete the file.
- **Package managers switch self-update off with a marker file.** The user asked for
  the Homebrew tap to be what disables it. A package installs
  `share/wordl/managed-by` (one line: its name and how to upgrade) beside the `bin`
  directory its binary is in; `update::managed_by` finds it at
  `../share/wordl/managed-by` from the binary's real directory, and the game then
  refuses with "installed with ...". The tap's formula and the AUR `PKGBUILD` both
  install it, and the tap's workflow fails unless `wordl update` refuses on a Homebrew
  install. Behind that sit fallbacks: `Cellar` in the real path, a checkout's
  `target/`, a directory that is not writable. All checks use the binary's real path
  (`store::real_exe`): on macOS `current_exe` returns the symlink the program was
  started by, and typeshelf once replaced Homebrew's link because of that.
- **Actions are pinned to commit hashes** in every workflow, with the version in a
  trailing comment, because the release build's output is what users install.
  `.github/dependabot.yml` opens a monthly grouped PR with newer pins. Pin any action
  you add.
- **Packaging.** Three channels, all fed by the release:
  - *Install script* - the universal path, above. Its copy is the one that updates
    itself.
  - *Homebrew* - `anwarahmed/homebrew-tap`. The `TAP_TOKEN` secret here (set by the
    user on 2026-10-05) lets a release start the tap's workflow and wait for the
    formula. If it is missing the release run carries a "Homebrew tap not notified"
    warning; if it has expired the run fails at that step, after the release is
    already published, and the user has to create a new token.
  - *AUR* - package `wordl-bin`. `packaging/aur/render.sh` fills `PKGBUILD.in` and
    `SRCINFO.in` per release. `.SRCINFO` has its own template because releases build on
    Ubuntu, which has no `makepkg`; if you change one template change the other, and
    check with `makepkg --printsrcinfo | diff - .SRCINFO` on Arch.
    `packaging/aur/LICENSE` (0BSD) covers the package files, as the AUR guidelines
    ask. **Not published:** the user has no AUR account (registration was closed in
    October 2026). Until then the `PKGBUILD` attached to each release is installed
    with `makepkg -si`. The workflow pushes to the AUR once an `AUR_SSH_PRIVATE_KEY`
    secret exists; then switch the README to `yay -S wordl-bin`.
- **Minimum Rust is 1.88** (`rust-version` in `Cargo.toml`): the code uses let-chains
  and ratatui 0.30 needs it. CI's `test` jobs use latest stable, so a separate `msrv`
  job builds and tests on exactly 1.88; raise both together.
- **Few dependencies:** ratatui (with its re-exported crossterm; do not add a separate
  crossterm), chrono for the local date, ureq and sha2 for the updater. (The library
  half needs only the last two, but a crate's dependencies are not split by half;
  funwordl uses ratatui and chrono itself, so nothing extra is built for it.) No
  argument-parsing, serde or random-number crates: the CLI is a dozen flags, the files
  are `key=value`, and picking a word needs only a xorshift.

## Verifying changes

`cargo test` covers the rules, the layout at every size, and drawing every theme and
dialog at a dozen sizes into ratatui's `TestBackend` (which is also how a test reads
the screen as text). `tests/e2e.sh` runs the built binary: the command line,
`install.sh`, the updater against made-up releases, and the game in a detached tmux
session with mouse clicks, a resize, giving up, and animations on.

Neither can judge how the screen looks. For that, run the game in a detached tmux
session with a throwaway state directory and read the screen, or draw it:

```sh
X=$(mktemp -d)
tmux -L w new-session -d -x 150 -y 46 \
  "XDG_STATE_HOME=$X COLORTERM=truecolor WORDL_DEBUG_ANSWER=crane target/release/wordl"
tmux -L w send-keys -l slate; tmux -L w send-keys Enter
tmux -L w capture-pane -p            # add -e to see colors
tools/screenshot.py w /tmp/shot.png  # a PNG of the pane
tmux -L w resize-window -x 39 -y 12  # then look again
tmux -L w kill-server
```

`WORDL_DEBUG_ANSWER` fixes the practice word; `WORDL_NO_ANIM=1` skips animations. A
mouse click is `tmux send-keys -l $'\e[<0;COL;ROWM'` (1-based).

Traps when scripting tmux: `send-keys Escape` immediately followed by another key
arrives as Alt+key, so pause between them; `gh run list` right after a push can return
the previous run; and never test the copy function against the real clipboard (put a
fake `wl-copy` first on `PATH`).

To try the self-update by hand, copy the binary outside any checkout (a build run from
`target/` never updates) and run it with `WORDL_RELEASE_URL=file://<dir>`, where `<dir>`
is made the way `make_release` in `tests/e2e.sh` makes one.

## Known gaps and ideas

- Never run by a person on a real Mac or in macOS Terminal.app; CI runs the unit and
  end-to-end tests on a macOS runner, which is not the same as someone looking at it.
- The Intel macOS binary is cross-built on an Apple silicon runner and is never
  executed in CI; the other three targets are smoke-tested.
- The README pictures are drawn by `tools/screenshot.py` from tmux's cell data, not
  captured from a terminal window.
- At the smallest sizes (level 1) tiles in a column touch; there is no room for gaps.
- Dialogs taller than the terminal lose their last lines.
- No hover effects, no key-press flash on the on-screen keyboard.
- `answers.txt` was read through by Claude (see "Own word lists"), not by a person.
  The user reports words that feel wrong.
- The definitions were written by Claude in one sitting and checked only by machine
  (coverage, length, format). Some will be wrong or clumsy; the user and the children
  report them.
- A definition is shown only in the statistics dialog after a game. There is no way to
  look up a word that was guessed, and no list of words met so far.
- Not done: other word lengths, other languages, sharing as an image.
