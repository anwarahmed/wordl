# wordl

A Wordle-style word game for the terminal, written in bash. Guess the hidden
five-letter word in six tries.

![wordl in a large terminal: block-letter tiles on the left, keyboard on the right](assets/screenshot.png)

- **Fills the terminal and follows its size.** On a large terminal the tiles and keys
  are big block letters; on a small one they shrink to single characters. Wide
  terminals put the keyboard beside the board, tall ones underneath.
- **Keyboard first, mouse welcome.** Everything has a key; the on-screen keyboard and
  every button can also be clicked.
- **A new word every time**, plus one daily puzzle that is the same for everyone.
- **Three difficulty levels**, five color themes, statistics with streaks, and a
  result you can copy and share.

<img src="assets/screenshot-small.png" width="480" alt="wordl in an 80x24 terminal">

## Install

wordl runs on macOS and Linux. It needs **bash 4.4 or newer** and a UTF-8 terminal.

### Homebrew (macOS and Linux)

```sh
brew install anwarahmed/tap/wordl
```

Homebrew brings its own current bash, so nothing else is needed. Update with
`brew upgrade wordl`, remove with `brew uninstall wordl`.

### Install script

```sh
curl -fsSL https://raw.githubusercontent.com/anwarahmed/wordl/main/install.sh | sh
```

This downloads the latest release, checks its checksum, puts the game in
`~/.local/share/wordl` and links it as `~/.local/bin/wordl`. A copy installed this way
keeps itself up to date (see [Updates](#updates)). The script needs `curl` and `tar`.
It does not install bash: macOS ships bash 3.2, so on a Mac run `brew install bash`
first (or use the Homebrew install above). To remove the game:

```sh
curl -fsSL https://raw.githubusercontent.com/anwarahmed/wordl/main/install.sh | sh -s -- --uninstall
```

### Arch Linux

Each [release](https://github.com/anwarahmed/wordl/releases/latest) carries a
`PKGBUILD`. Download it into an empty directory and build the package:

```sh
curl -fsSLO https://github.com/anwarahmed/wordl/releases/latest/download/PKGBUILD
makepkg -si
```

Remove it with `sudo pacman -R wordl`. (The package is not in the AUR yet.)

### From a clone

```sh
git clone https://github.com/anwarahmed/wordl
./wordl/wordl
```

## Updates

| Installed with | How it updates |
| -------------- | -------------- |
| Install script | By itself: each time it starts it checks for a newer release, installs it and restarts |
| Homebrew       | `brew upgrade wordl` |
| Arch package   | Build the newer `PKGBUILD` the same way |
| A clone        | `git pull` |

Only the install script's copy updates itself. A copy that Homebrew or pacman owns is
marked as theirs when it is installed and never touches its own files, and neither does
a clone.

For a copy that updates itself:

```sh
wordl update        # check now and install a newer release
wordl update off    # stop checking at startup ("on" turns it back on)
```

`WORDL_NO_UPDATE=1` skips the check for one run. The check waits at most three seconds
and says nothing when you are offline. An update is verified against the release's
SHA-256 checksum, never moves to an older version, and replaces only the game's own
files; if anything fails, the version you have starts as usual.

## Play

Type a five-letter word and press Enter. Each tile then tells you how close you were:

| Tile   | Meaning                                |
| ------ | -------------------------------------- |
| Green  | the letter is in the word, in this spot |
| Yellow | the letter is in the word, elsewhere   |
| Gray   | the letter is not in the word          |

The on-screen keyboard keeps track of what you know about each letter.

| Key         | Action                                   |
| ----------- | ---------------------------------------- |
| `A`-`Z`     | type a letter                            |
| `Enter`     | submit the guess                         |
| `Backspace` | delete a letter                          |
| `?` or `F1` | help                                     |
| `Ctrl-N`    | new game with a random word              |
| `Ctrl-D`    | today's daily puzzle                     |
| `Ctrl-S`    | statistics                               |
| `Ctrl-T`    | next color theme                         |
| `Ctrl-X`    | next difficulty                          |
| `Ctrl-L`    | redraw the screen                        |
| `Ctrl-Q`    | quit                                     |

### Practice and the daily puzzle

Every launch, and every `Ctrl-N`, starts a practice game with a new random word. The
daily puzzle (`Ctrl-D`, or `wordl --daily`) is one word a day, the same for everyone on
the same date; it is the one game that is picked up where you left it. Statistics are
kept separately for the two.

### Difficulty

`Ctrl-X` cycles through three levels. A game that is under way can be made easier at
any time; a harder level applies from the next game.

- **Normal** - guesses must be valid dictionary words.
- **Hard** - Wordle's hard mode: green letters must stay fixed and yellow letters must
  be reused.
- **Ultra Hard** - stricter still: yellow letters must also move away from the spot
  where they were clued, and gray clues must be obeyed (a gray letter can't be played
  again, beyond the copies of it already shown as green or yellow).

A refused guess says which clue it breaks. Shared results mark Hard with `*` and Ultra
Hard with `**`.

### Themes

`midnight` (the default), `daylight`, `neon`, `contrast` and `terminal`. `contrast`
uses orange and blue instead of green and yellow, for color-blind players. `terminal`
uses only your terminal's own 16 colors, so it follows your terminal theme.

### Terminal size

The game redraws itself when the window is resized and picks the largest board that
fits. The smallest usable size is 39 columns by 12 rows (a wide terminal can be as
short as 7 rows). Truecolor is used when the terminal announces it (`COLORTERM`),
256 colors otherwise.

## Options

```
wordl [options]
wordl update [on|off]

  -p, --practice     start with a new random word (default)
  -d, --daily        start with today's puzzle
  -t, --theme NAME   midnight daylight neon contrast terminal
      --normal       any dictionary word is a valid guess
      --hard         green letters stay fixed, yellow letters must be reused
      --ultra        ultra hard: also, yellow letters must move to another
                     spot and gray letters may not be played again
      --no-animation skip the tile animations
  -v, --version      print the version
  -h, --help         show this help

  wordl update       check for a newer release now and install it
  wordl update off   stop checking at startup (on: check again)
  WORDL_NO_UPDATE    set this variable to skip the check for one run
```

The theme and difficulty you choose in the game are remembered.

## Files

Statistics, the daily puzzle's progress and your settings are plain text files in
`$XDG_STATE_HOME/wordl` (`~/.local/state/wordl` by default). Delete the folder to
start over.

## Word lists

The words come from [SCOWL](http://wordlist.aspell.net/) by Kevin Atkinson, not from
any other game:

- `words/allowed.txt` - about 11,400 five-letter words accepted as guesses.
- `words/answers.txt` - about 2,000 common ones a puzzle can be, with plurals and
  simple inflections filtered out.

`tools/build-words.py` regenerates both from a SCOWL download.

## Development

```sh
tests/run.sh          # rules, word lists, layout at every size, and the real game in tmux
shellcheck wordl
```

See [CLAUDE.md](CLAUDE.md) for how the script is put together and
[RELEASING.md](RELEASING.md) for how releases are made.

## License

The game is [MIT licensed](LICENSE). The word lists are derived from SCOWL and carry
its notice in [`words/SCOWL-COPYRIGHT`](words/SCOWL-COPYRIGHT).

wordl is an independent project and is not affiliated with Wordle or The New York
Times.
