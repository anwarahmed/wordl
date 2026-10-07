//! What is kept on disk: statistics and settings in one file, and the daily puzzle's
//! progress in another. Both are `key=value` lines in `$XDG_STATE_HOME/wordl`, the
//! same files the first, bash, version of the game wrote, so nothing was lost in the
//! move. A game built on this library keeps its own files, in a directory of its own.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::game::{self, Difficulty, Game, Mode, Status};

/// Where the program called `name` keeps its files: `~/.local/state/<name>` unless
/// `XDG_STATE_HOME` says otherwise, on macOS too, so the game behaves the same everywhere.
pub fn state_dir(name: &str) -> PathBuf {
    match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"),
    }
    .join(name)
}

/// The file this program really is, with symlinks resolved. `current_exe` alone resolves
/// them on Linux but on macOS returns the path the program was started by, e.g. Homebrew's
/// `bin/wordl` link instead of the file in its `Cellar`.
pub fn real_exe() -> std::io::Result<PathBuf> {
    std::env::current_exe().and_then(fs::canonicalize)
}

/// The source checkout this binary was built in, when it is being run from that
/// checkout's `target/` directory (directly or through a symlink).
pub fn checkout_root() -> Option<PathBuf> {
    let exe = real_exe().ok()?;
    let target = exe.parent()?.parent()?;
    let root = target.parent()?;
    (target.file_name()? == "target" && root.join("Cargo.toml").exists()).then(|| root.to_path_buf())
}

/// Reads `key=value` lines, ignoring anything that is not plain: keys are lowercase
/// letters, digits and underscores, values letters, digits and commas.
pub fn read_pairs(path: &Path) -> BTreeMap<String, String> {
    let plain_key = |k: &str| !k.is_empty() && k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let plain_value = |v: &str| v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b',');
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, v)| plain_key(k) && plain_value(v))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Writes `key=value` lines that `read_pairs` reads back.
pub fn write_pairs(path: &Path, pairs: &BTreeMap<String, String>) {
    let text: String = pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    // Losing statistics is not worth stopping a game for.
    let _ = path.parent().map(fs::create_dir_all);
    let _ = fs::write(path, text);
}

/// Statistics per mode, and the remembered settings.
#[derive(Default)]
pub struct Stats {
    dir: PathBuf,
    values: BTreeMap<String, String>,
}

impl Stats {
    /// The directory the files are in, for a game that keeps more of them there.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn load(dir: PathBuf) -> Self {
        let values = read_pairs(&dir.join("stats"));
        Self { dir, values }
    }

    pub fn save(&self) {
        write_pairs(&self.dir.join("stats"), &self.values);
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn num(&self, key: &str) -> i64 {
        self.text(key).and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    pub fn set(&mut self, key: &str, value: impl ToString) {
        self.values.insert(key.to_string(), value.to_string());
    }

    /// The count for one mode, e.g. `of(Mode::Daily, "streak")`.
    pub fn of(&self, mode: Mode, what: &str) -> i64 {
        self.num(&format!("{}_{what}", mode.key()))
    }

    fn set_of(&mut self, mode: Mode, what: &str, value: i64) {
        self.set(&format!("{}_{what}", mode.key()), value);
    }

    pub fn theme(&self) -> &str {
        self.text("theme").unwrap_or("midnight")
    }

    pub fn difficulty(&self) -> Difficulty {
        Difficulty::from_index(self.num("hard").clamp(0, 2) as usize)
    }

    /// Whether the update check at startup is on. It is unless switched off.
    pub fn auto_update(&self) -> bool {
        self.text("update") != Some("0")
    }

    /// A daily streak only lasts while a day is not missed.
    pub fn expire_daily_streak(&mut self, today: i64) {
        if self.num("daily_lastwin") < today - 1 {
            self.set_of(Mode::Daily, "streak", 0);
        }
    }

    /// Counts a finished game and saves.
    pub fn record(&mut self, game: &Game) {
        let mode = game.mode;
        self.set_of(mode, "played", self.of(mode, "played") + 1);
        if game.status == Status::Won {
            self.set_of(mode, "wins", self.of(mode, "wins") + 1);
            let tries = format!("d{}", game.guesses.len());
            self.set_of(mode, &tries, self.of(mode, &tries) + 1);
            // The daily streak counts consecutive days, the practice one consecutive wins.
            let broken = mode == Mode::Daily && self.num("daily_lastwin") != game.day - 1;
            self.set_of(mode, "streak", if broken { 1 } else { self.of(mode, "streak") + 1 });
            if mode == Mode::Daily {
                self.set("daily_lastwin", game.day);
            }
            self.set_of(mode, "best", self.of(mode, "best").max(self.of(mode, "streak")));
        } else {
            self.set_of(mode, "streak", 0);
        }
        self.save();
    }

    /// Saves the daily puzzle so it can be picked up later in the day. Practice games
    /// are never saved: every launch starts a new word.
    pub fn save_daily(&self, game: &Game) {
        if game.mode != Mode::Daily {
            return;
        }
        let guesses: Vec<String> = game.guesses.iter().map(game::text).collect();
        let pairs = BTreeMap::from([
            ("day".to_string(), game.day.to_string()),
            ("answer".to_string(), game::text(&game.answer)),
            ("hard".to_string(), game.difficulty.index().to_string()),
            ("gaveup".to_string(), (game.gave_up as u8).to_string()),
            ("tries".to_string(), game.tries.to_string()),
            ("guesses".to_string(), guesses.join(",")),
        ]);
        write_pairs(&self.dir.join("daily"), &pairs);
    }

    /// Today's daily puzzle as it was left, if it was started today.
    pub fn load_daily(&self, today: i64) -> Option<Game> {
        let saved = read_pairs(&self.dir.join("daily"));
        if saved.get("day")?.parse::<i64>().ok()? != today {
            return None;
        }
        let answer = game::word(saved.get("answer")?).filter(|_| saved["answer"].bytes().all(|b| b.is_ascii_uppercase()))?;
        let difficulty = Difficulty::from_index(saved.get("hard").and_then(|v| v.parse::<usize>().ok()).filter(|&v| v <= 2).unwrap_or(0));
        // Absent in files written before a game could allow anything but six.
        let tries = saved.get("tries").and_then(|v| v.parse::<usize>().ok()).filter(|v| (1..=game::MAX_TRIES).contains(v)).unwrap_or(game::TRIES);
        let mut game = Game::new(Mode::Daily, today, answer, difficulty).with_tries(tries);
        for guess in saved.get("guesses").map(String::as_str).unwrap_or("").split(',').filter_map(game::word) {
            if game.playing() {
                game.add_guess(guess);
            }
        }
        // A daily puzzle that was given up stays given up.
        if saved.get("gaveup").map(String::as_str) == Some("1") && game.playing() {
            game.give_up();
        }
        Some(game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wordl-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn finished(mode: Mode, day: i64, guesses: &[&str]) -> Game {
        let mut game = Game::new(mode, day, game::word("CRANE").unwrap(), Difficulty::Normal);
        for g in guesses {
            game.add_guess(game::word(g).unwrap());
        }
        game
    }

    #[test]
    fn reads_only_plain_pairs() {
        let dir = temp_dir("pairs");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("stats"), "theme=neon\nhard=2\nBAD KEY=1\nevil=$(rm -rf)\nno equals\npractice_wins=7\n").unwrap();
        let stats = Stats::load(dir.clone());
        assert_eq!(stats.theme(), "neon");
        assert_eq!(stats.difficulty(), Difficulty::Ultra);
        assert_eq!(stats.of(Mode::Practice, "wins"), 7);
        assert_eq!(stats.text("evil"), None);
        assert!(stats.auto_update());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn counts_wins_losses_and_streaks() {
        let dir = temp_dir("record");
        let mut stats = Stats::load(dir.clone());
        stats.record(&finished(Mode::Practice, 0, &["SLATE", "CRANE"]));
        stats.record(&finished(Mode::Practice, 0, &["CRANE"]));
        assert_eq!((stats.of(Mode::Practice, "played"), stats.of(Mode::Practice, "wins")), (2, 2));
        assert_eq!((stats.of(Mode::Practice, "streak"), stats.of(Mode::Practice, "best")), (2, 2));
        assert_eq!((stats.of(Mode::Practice, "d1"), stats.of(Mode::Practice, "d2")), (1, 1));
        let mut lost = finished(Mode::Practice, 0, &["SLATE"]);
        lost.give_up();
        stats.record(&lost);
        assert_eq!((stats.of(Mode::Practice, "played"), stats.of(Mode::Practice, "streak"), stats.of(Mode::Practice, "best")), (3, 0, 2));
        // Saved, and read back by a fresh load.
        assert_eq!(Stats::load(dir.clone()).of(Mode::Practice, "played"), 3);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_daily_streak_needs_consecutive_days() {
        let dir = temp_dir("streak");
        let mut stats = Stats::load(dir.clone());
        stats.record(&finished(Mode::Daily, 100, &["CRANE"]));
        stats.record(&finished(Mode::Daily, 101, &["CRANE"]));
        assert_eq!(stats.of(Mode::Daily, "streak"), 2);
        stats.record(&finished(Mode::Daily, 103, &["CRANE"]));
        assert_eq!((stats.of(Mode::Daily, "streak"), stats.of(Mode::Daily, "best")), (1, 2));
        stats.expire_daily_streak(104);
        assert_eq!(stats.of(Mode::Daily, "streak"), 1);
        stats.expire_daily_streak(105);
        assert_eq!(stats.of(Mode::Daily, "streak"), 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_daily_puzzle_is_resumed_the_same_day_only() {
        let dir = temp_dir("daily");
        let stats = Stats::load(dir.clone());
        let mut game = finished(Mode::Daily, 200, &["SLATE", "TRACK"]);
        game.difficulty = Difficulty::Hard;
        stats.save_daily(&game);
        let back = stats.load_daily(200).unwrap();
        assert_eq!((back.guesses.len(), back.status, back.difficulty, back.gave_up), (2, Status::Playing, Difficulty::Hard, false));
        assert_eq!(back.marks, game.marks);
        assert!(stats.load_daily(201).is_none());

        game.give_up();
        stats.save_daily(&game);
        let back = stats.load_daily(200).unwrap();
        assert_eq!((back.guesses.len(), back.status, back.gave_up), (2, Status::Lost, true));

        // A game that allows more guesses is still under way after its sixth.
        let long = ["SLATE"; 6];
        stats.save_daily(&finished(Mode::Daily, 200, &long));
        assert_eq!(stats.load_daily(200).unwrap().status, Status::Lost);
        let mut game = Game::new(Mode::Daily, 200, game::word("CRANE").unwrap(), Difficulty::Normal).with_tries(8);
        long.iter().for_each(|g| game.add_guess(game::word(g).unwrap()));
        stats.save_daily(&game);
        let back = stats.load_daily(200).unwrap();
        assert_eq!((back.guesses.len(), back.tries, back.status), (6, 8, Status::Playing));
        // A file from before the count was saved means six.
        let mut old = read_pairs(&dir.join("daily"));
        old.remove("tries");
        write_pairs(&dir.join("daily"), &old);
        assert_eq!(stats.load_daily(200).unwrap().status, Status::Lost);

        // Practice games are never saved.
        let _ = fs::remove_file(dir.join("daily"));
        stats.save_daily(&finished(Mode::Practice, 200, &["SLATE"]));
        assert!(stats.load_daily(200).is_none());
        let _ = fs::remove_dir_all(dir);
    }
}
