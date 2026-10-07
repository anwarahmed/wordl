//! The rules: scoring a guess, the three difficulties, and the state of one game.
//! Pure, with no I/O and no clock, so it is unit-tested directly.

use crate::words;

/// A five-letter word in uppercase ASCII.
pub type Word = [u8; 5];

pub fn word(s: &str) -> Option<Word> {
    let bytes = s.as_bytes();
    (bytes.len() == 5 && bytes.iter().all(u8::is_ascii_alphabetic)).then(|| std::array::from_fn(|i| bytes[i].to_ascii_uppercase()))
}

pub fn text(w: &Word) -> String {
    w.iter().map(|&b| b as char).collect()
}

/// What a guess revealed about one of its letters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    /// In the word, in this spot.
    Green,
    /// In the word, elsewhere.
    Yellow,
    /// Not in the word (or no more copies of it).
    Gray,
}

pub type Marks = [Mark; 5];

/// Scores a guess. Greens are settled first; each remaining letter of the answer can
/// then make one guessed copy of itself yellow.
pub fn evaluate(guess: &Word, answer: &Word) -> Marks {
    let mut marks = [Mark::Gray; 5];
    let mut rest = Vec::with_capacity(5);
    for i in 0..5 {
        if guess[i] == answer[i] {
            marks[i] = Mark::Green;
        } else {
            rest.push(answer[i]);
        }
    }
    for i in 0..5 {
        if marks[i] != Mark::Green
            && let Some(at) = rest.iter().position(|&b| b == guess[i])
        {
            marks[i] = Mark::Yellow;
            rest.swap_remove(at);
        }
    }
    marks
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Difficulty {
    /// Any dictionary word is a valid guess.
    Normal,
    /// Green letters stay where they are, yellow letters are reused.
    Hard,
    /// Also: a yellow letter must move to another spot, and gray clues are obeyed.
    Ultra,
}

impl Difficulty {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Self {
        [Self::Normal, Self::Hard, Self::Ultra][i % 3]
    }

    pub fn next(self) -> Self {
        Self::from_index(self.index() + 1)
    }

    pub fn name(self) -> &'static str {
        ["Normal", "Hard", "Ultra Hard"][self.index()]
    }
}

const ORDINALS: [&str; 5] = ["1st", "2nd", "3rd", "4th", "5th"];

/// Checks a guess against the clues so far, for Hard and Ultra Hard. The error is the
/// reason shown to the player.
///
/// For a repeated letter, gray means "no more copies than this guess showed as green or
/// yellow", so a later guess may hold that many and no more.
pub fn check_clues(difficulty: Difficulty, guesses: &[Word], marks: &[Marks], guess: &Word) -> Result<(), String> {
    if difficulty == Difficulty::Normal {
        return Ok(());
    }
    for (prev, marks) in guesses.iter().zip(marks) {
        for i in 0..5 {
            let ch = prev[i] as char;
            match marks[i] {
                Mark::Green if guess[i] != prev[i] => return Err(format!("{} letter must be {ch}", ORDINALS[i])),
                Mark::Yellow if !guess.contains(&prev[i]) => return Err(format!("Guess must contain {ch}")),
                _ => {}
            }
        }
        if difficulty != Difficulty::Ultra {
            continue;
        }
        for i in 0..5 {
            let ch = prev[i] as char;
            match marks[i] {
                Mark::Green => {}
                Mark::Yellow => {
                    if guess[i] == prev[i] {
                        return Err(format!("{} letter can't be {ch}", ORDINALS[i]));
                    }
                }
                Mark::Gray => {
                    let known = (0..5).filter(|&j| prev[j] == prev[i] && marks[j] != Mark::Gray).count();
                    let played = guess.iter().filter(|&&b| b == prev[i]).count();
                    if played > known {
                        return Err(if known == 0 { format!("{ch} is not in the word") } else { format!("Only {known} {ch} in the word") });
                    }
                    if guess[i] == prev[i] {
                        return Err(format!("{} letter can't be {ch}", ORDINALS[i]));
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// One word a day, the same for everyone.
    Daily,
    /// A random word.
    Practice,
}

impl Mode {
    /// The prefix of this mode's keys in the statistics file.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Daily => "daily",
            Mode::Practice => "practice",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Playing,
    Won,
    Lost,
}

/// How many guesses a game allows unless told otherwise.
pub const TRIES: usize = 6;
/// The most a game can be told to allow; a saved game claiming more is not believed.
pub const MAX_TRIES: usize = 12;

/// The day number (days since 1970-01-01, local time) of the day before puzzle #1.
pub const DAILY_BASE: i64 = 20726;

/// The daily word. Changing `words/answers.txt` changes it.
pub fn daily_answer(day: i64) -> Word {
    let list = words::answers();
    let index = (day * 7919 + 104_729).rem_euclid(list.len() as i64) as usize;
    word(list[index]).expect("answers are five letters")
}

pub struct Game {
    pub mode: Mode,
    /// Local day number the game was started on; names the daily puzzle.
    pub day: i64,
    pub answer: Word,
    pub guesses: Vec<Word>,
    pub marks: Vec<Marks>,
    /// Letters typed for the next guess.
    pub cur: Vec<u8>,
    pub status: Status,
    /// The difficulty this game is held to, which can differ from the chosen one: a
    /// game under way can be made easier but not harder.
    pub difficulty: Difficulty,
    /// Lost by giving up rather than by running out of guesses.
    pub gave_up: bool,
    /// How many guesses the game allows: six, unless `with_tries` said otherwise.
    pub tries: usize,
}

impl Game {
    pub fn new(mode: Mode, day: i64, answer: Word, difficulty: Difficulty) -> Self {
        Self { mode, day, answer, guesses: Vec::new(), marks: Vec::new(), cur: Vec::new(), status: Status::Playing, difficulty, gave_up: false, tries: TRIES }
    }

    /// The same game allowing `tries` guesses (at least one, at most `MAX_TRIES`), for
    /// an easier or a harder game than the usual six.
    pub fn with_tries(mut self, tries: usize) -> Self {
        self.tries = tries.clamp(1, MAX_TRIES);
        self
    }

    pub fn playing(&self) -> bool {
        self.status == Status::Playing
    }

    /// Records a guess, already checked, and settles whether the game is over.
    pub fn add_guess(&mut self, guess: Word) {
        self.marks.push(evaluate(&guess, &self.answer));
        self.guesses.push(guess);
        if guess == self.answer {
            self.status = Status::Won;
        } else if self.guesses.len() >= self.tries {
            self.status = Status::Lost;
        }
    }

    pub fn give_up(&mut self) {
        self.cur.clear();
        self.gave_up = true;
        self.status = Status::Lost;
    }

    /// The best that the first `rows` guesses say about a letter: green beats yellow
    /// beats gray. Colors the on-screen keyboard.
    pub fn key_mark(&self, letter: u8, rows: usize) -> Option<Mark> {
        let mut best = None;
        for (guess, marks) in self.guesses.iter().zip(&self.marks).take(rows) {
            for i in 0..5 {
                if guess[i] == letter {
                    best = match (best, marks[i]) {
                        (Some(Mark::Green), _) | (_, Mark::Green) => Some(Mark::Green),
                        (Some(Mark::Yellow), _) | (_, Mark::Yellow) => Some(Mark::Yellow),
                        _ => Some(Mark::Gray),
                    };
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Word {
        word(s).unwrap()
    }

    fn score(guess: &str, answer: &str) -> String {
        evaluate(&w(guess), &w(answer))
            .iter()
            .map(|m| match m {
                Mark::Green => 'g',
                Mark::Yellow => 'y',
                Mark::Gray => 'x',
            })
            .collect()
    }

    #[test]
    fn scores_guesses() {
        assert_eq!(score("CRANE", "CRANE"), "ggggg");
        assert_eq!(score("SLATE", "CRANE"), "xxgxg");
        // A repeated letter is yellow or green only as often as the answer has it.
        assert_eq!(score("EERIE", "CRANE"), "xxyxg");
        assert_eq!(score("BOBBY", "ABBEY"), "yxgxg");
        assert_eq!(score("LLAMA", "ALLOW"), "ygyxx");
        assert_eq!(score("NANNY", "CRANE"), "xyxgx");
    }

    /// Why `guess` is refused after `earlier` guesses at `answer`, or "ok".
    fn verdict(difficulty: Difficulty, answer: &str, earlier: &str, guess: &str) -> String {
        let mut game = Game::new(Mode::Practice, 0, w(answer), difficulty);
        for g in earlier.split_whitespace() {
            game.add_guess(w(g));
        }
        check_clues(difficulty, &game.guesses, &game.marks, &w(guess)).err().unwrap_or_else(|| "ok".into())
    }

    #[test]
    fn normal_accepts_anything() {
        assert_eq!(verdict(Difficulty::Normal, "CRANE", "SLATE", "COUNT"), "ok");
    }

    #[test]
    fn hard_keeps_green_and_reuses_yellow() {
        let hard = Difficulty::Hard;
        assert_eq!(verdict(hard, "CRANE", "SLATE", "BLAZE"), "ok");
        assert_eq!(verdict(hard, "CRANE", "SLATE", "COUNT"), "3rd letter must be A");
        assert_eq!(verdict(hard, "CRANE", "TRACK", "BRAVE"), "Guess must contain C");
        // Hard lets a yellow letter stay where it was.
        assert_eq!(verdict(hard, "CRANE", "TRACK", "CRACK"), "ok");
    }

    #[test]
    fn ultra_moves_yellow_and_obeys_gray() {
        let ultra = Difficulty::Ultra;
        assert_eq!(verdict(ultra, "CRANE", "SLATE", "BLAZE"), "L is not in the word");
        assert_eq!(verdict(ultra, "CRANE", "TRACK", "CRACK"), "4th letter can't be C");
        assert_eq!(verdict(ultra, "CRANE", "SLATE TRACK", "CRANE"), "ok");
        assert_eq!(verdict(ultra, "CRANE", "EERIE", "CREPE"), "Only 1 E in the word");
        assert_eq!(verdict(ultra, "ABBEY", "BOBBY", "ABBEY"), "ok");
        assert_eq!(verdict(ultra, "ABBEY", "BOBBY", "ALBBY"), "4th letter can't be B");
    }

    #[test]
    fn games_end_on_the_answer_or_the_sixth_guess() {
        let mut game = Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal);
        game.add_guess(w("SLATE"));
        assert_eq!(game.status, Status::Playing);
        game.add_guess(w("CRANE"));
        assert_eq!(game.status, Status::Won);

        let mut game = Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal);
        for _ in 0..6 {
            game.add_guess(w("SLATE"));
        }
        assert_eq!(game.status, Status::Lost);
        assert!(!game.gave_up);
    }

    #[test]
    fn a_game_can_allow_more_or_fewer_guesses() {
        let mut game = Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal).with_tries(8);
        for _ in 0..7 {
            game.add_guess(w("SLATE"));
        }
        assert_eq!(game.status, Status::Playing);
        game.add_guess(w("SLATE"));
        assert_eq!(game.status, Status::Lost);

        let mut game = Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal).with_tries(1);
        game.add_guess(w("SLATE"));
        assert_eq!(game.status, Status::Lost);
        assert_eq!(Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal).with_tries(99).tries, MAX_TRIES);
    }

    #[test]
    fn keyboard_shows_the_best_clue_for_a_letter() {
        let mut game = Game::new(Mode::Practice, 0, w("CRANE"), Difficulty::Normal);
        game.add_guess(w("TRACK")); // C is yellow
        game.add_guess(w("CLASS")); // C is green
        assert_eq!(game.key_mark(b'C', 1), Some(Mark::Yellow));
        assert_eq!(game.key_mark(b'C', 2), Some(Mark::Green));
        assert_eq!(game.key_mark(b'T', 2), Some(Mark::Gray));
        assert_eq!(game.key_mark(b'Z', 2), None);
    }

    #[test]
    fn the_daily_word_is_fixed_for_a_day() {
        assert_eq!(daily_answer(20731), daily_answer(20731));
        assert!(words::is_word(&text(&daily_answer(20731))));
        assert!(words::is_word(&text(&daily_answer(-5))));
    }
}
