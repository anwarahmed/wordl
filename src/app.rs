//! The state of the running game and everything keys and clicks do to it.
//! `ui.rs` only reads this; nothing here draws.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::game::{self, DAILY_BASE, Difficulty, Game, Mode, Status, Word};
use crate::layout;
use crate::store::Stats;
use crate::theme::{self, Theme};
use crate::{ui, words};

/// How long one tile takes to flip over when a guess is revealed.
pub const FLIP: Duration = Duration::from_millis(190);
/// One step of the row's shake after a refused guess, and how many steps there are.
pub const SHAKE_STEP: Duration = Duration::from_millis(35);
pub const SHAKE_STEPS: u32 = 7;
/// One step of the flash that runs along a winning row.
pub const FLASH_STEP: Duration = Duration::from_millis(70);
/// The pause between the end of a game and the statistics opening by themselves.
const STATS_DELAY: Duration = Duration::from_millis(1300);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modal {
    None,
    Help,
    Stats,
    /// "Give up?", asked before a game is ended on purpose.
    GiveUp,
}

/// A key of the on-screen keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyId {
    Letter(u8),
    Enter,
    Back,
}

/// Something a key or a click asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Key(KeyId),
    Help,
    New,
    Daily,
    Stats,
    Theme,
    Difficulty,
    /// Ask whether to give up.
    GiveUp,
    /// Give up, confirmed.
    Surrender,
    Copy,
    Close,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageKind {
    /// Something was refused; shown as a chip.
    Error,
    /// The game was won.
    Win,
    /// A plain note.
    Plain,
}

pub struct Message {
    pub text: String,
    pub kind: MessageKind,
    /// When it disappears; `None` stays.
    until: Option<Instant>,
}

/// How the game was started.
pub struct Options {
    pub mode: Mode,
    pub theme: Option<String>,
    pub difficulty: Option<Difficulty>,
    pub animate: bool,
    pub truecolor: bool,
    /// Fixes the practice word, for tests (`WORDL_DEBUG_ANSWER`).
    pub debug_answer: Option<Word>,
}

pub struct App {
    pub game: Game,
    pub stats: Stats,
    pub theme: Theme,
    truecolor: bool,
    /// The difficulty new games start with. A game under way keeps its own.
    pub chosen: Difficulty,
    pub modal: Modal,
    pub message: Option<Message>,
    /// The end-of-game message returns once a passing note ("Result copied") is gone.
    restore_end: bool,
    pending_stats: Option<Instant>,
    /// Row being revealed and when it started.
    pub reveal: Option<(usize, Instant)>,
    pub shake: Option<Instant>,
    pub celebrate: Option<(usize, Instant)>,
    animate: bool,
    /// The time of the frame being drawn; animations are a function of it.
    pub now: Instant,
    pub quit: bool,
    /// Terminal size in columns and rows, kept current by the event loop.
    pub size: (i32, i32),
    /// Set to have the whole screen repainted (Ctrl-L).
    pub repaint: bool,
    debug_answer: Option<Word>,
    seed: u64,
}

/// Local days since 1970-01-01.
pub fn today() -> i64 {
    chrono::Local::now().date_naive().signed_duration_since(chrono::NaiveDate::default()).num_days()
}

impl App {
    pub fn new(mut stats: Stats, options: Options) -> Self {
        stats.expire_daily_streak(today());
        let chosen = options.difficulty.unwrap_or_else(|| stats.difficulty());
        let theme = theme::theme(options.theme.as_deref().unwrap_or_else(|| stats.theme()), options.truecolor);
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64) ^ ((std::process::id() as u64) << 32) | 1;
        let mut app = Self {
            game: Game::new(Mode::Practice, 0, *b"WORDL", chosen),
            stats,
            theme,
            truecolor: options.truecolor,
            chosen,
            modal: Modal::None,
            message: None,
            restore_end: false,
            pending_stats: None,
            reveal: None,
            shake: None,
            celebrate: None,
            animate: options.animate,
            now: Instant::now(),
            quit: false,
            size: (80, 24),
            repaint: false,
            debug_answer: options.debug_answer,
            seed,
        };
        app.new_game(options.mode);
        app
    }

    /// Starts a game. Practice is always a new random word; the daily puzzle picks up
    /// where it was left, since there is only one word a day.
    pub fn new_game(&mut self, mode: Mode) {
        let day = today();
        self.game = match mode {
            Mode::Daily => self.stats.load_daily(day).unwrap_or_else(|| Game::new(mode, day, game::daily_answer(day), self.chosen)),
            Mode::Practice => Game::new(mode, day, self.debug_answer.unwrap_or_else(|| self.random_answer()), self.chosen),
        };
        self.modal = Modal::None;
        self.pending_stats = None;
        (self.reveal, self.shake, self.celebrate) = (None, None, None);
        self.end_message();
    }

    fn random_answer(&mut self) -> Word {
        // xorshift64: plenty for picking a word.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let list = words::answers();
        game::word(list[(self.seed % list.len() as u64) as usize]).expect("answers are five letters")
    }

    /// The puzzle's number, for the daily mode.
    pub fn daily_number(&self) -> i64 {
        self.game.day - DAILY_BASE
    }

    fn small(&self) -> bool {
        layout::layout(self.size.0, self.size.1).is_none()
    }

    /// Whether an animation is running. While one is, the event loop draws frames and
    /// leaves keys in the queue, so typing ahead is neither lost nor acted on early.
    pub fn animating(&self) -> bool {
        self.reveal.is_some() || self.shake.is_some() || self.celebrate.is_some()
    }

    /// When the loop has to wake up without a key: a message expiring, or the
    /// statistics opening.
    pub fn next_deadline(&self) -> Option<Instant> {
        [self.message.as_ref().and_then(|m| m.until), self.pending_stats].into_iter().flatten().min()
    }

    /// Moves time forward: ends animations, expires the message, opens the statistics.
    pub fn tick(&mut self) {
        self.now = Instant::now();
        if self.reveal.is_some_and(|(_, start)| self.now >= start + FLIP * 5) {
            let (row, _) = self.reveal.take().unwrap();
            if self.game.status == Status::Won {
                self.celebrate = Some((row, self.now));
            } else {
                self.after_guess();
            }
        }
        if self.celebrate.is_some_and(|(_, start)| self.now >= start + FLASH_STEP * 6) {
            self.celebrate = None;
            self.after_guess();
        }
        if self.shake.is_some_and(|start| self.now >= start + SHAKE_STEP * SHAKE_STEPS) {
            self.shake = None;
        }
        if self.message.as_ref().is_some_and(|m| m.until.is_some_and(|t| self.now >= t)) {
            self.message = None;
            if std::mem::take(&mut self.restore_end) {
                self.end_message();
            }
        }
        if self.pending_stats.is_some_and(|t| self.now >= t) {
            self.pending_stats = None;
            if !self.small() {
                self.modal = Modal::Stats;
            }
        }
    }

    fn toast(&mut self, text: impl Into<String>, kind: MessageKind, seconds: u64) {
        self.message = Some(Message { text: text.into(), kind, until: Some(Instant::now() + Duration::from_secs(seconds)) });
    }

    /// A passing message goes when the player types again; a lasting one stays.
    fn clear_toast(&mut self) {
        if self.message.as_ref().is_some_and(|m| m.until.is_some()) {
            self.message = None;
        }
    }

    /// The lasting message of a finished game, or none while it is being played.
    fn end_message(&mut self) {
        const PRAISE: [&str; 6] = ["Genius", "Magnificent", "Impressive", "Splendid", "Great", "Phew"];
        let tries = self.game.guesses.len();
        self.message = match self.game.status {
            Status::Won => Some((format!("{}! Solved in {tries}/6", PRAISE[tries.clamp(1, 6) - 1]), MessageKind::Win)),
            Status::Lost => Some((format!("The word was {}", game::text(&self.game.answer)), MessageKind::Error)),
            Status::Playing => None,
        }
        .map(|(text, kind)| Message { text, kind, until: None });
    }

    /// Once a guess has been revealed: if that ended the game, say so and line up the
    /// statistics.
    fn after_guess(&mut self) {
        if !self.game.playing() {
            self.end_message();
            self.pending_stats = Some(Instant::now() + STATS_DELAY);
        }
    }

    fn type_letter(&mut self, letter: u8) {
        if self.game.playing() && self.game.cur.len() < 5 {
            self.clear_toast();
            self.game.cur.push(letter.to_ascii_uppercase());
        }
    }

    fn backspace(&mut self) {
        if self.game.playing() && !self.game.cur.is_empty() {
            self.clear_toast();
            self.game.cur.pop();
        }
    }

    fn refuse(&mut self, why: impl Into<String>, seconds: u64) {
        self.toast(why, MessageKind::Error, seconds);
        if self.animate {
            self.shake = Some(Instant::now());
        }
    }

    fn submit(&mut self) {
        if !self.game.playing() {
            return self.act(Action::Stats);
        }
        let Some(guess) = game::word(&String::from_utf8_lossy(&self.game.cur)) else {
            return self.refuse("Not enough letters", 2);
        };
        if !words::is_word(&game::text(&guess)) {
            return self.refuse("Not in word list", 2);
        }
        if let Err(why) = game::check_clues(self.game.difficulty, &self.game.guesses, &self.game.marks, &guess) {
            return self.refuse(why, 3);
        }
        self.clear_toast();
        self.game.cur.clear();
        self.game.add_guess(guess);
        self.stats.save_daily(&self.game);
        if !self.game.playing() {
            self.stats.record(&self.game);
        }
        if self.animate {
            self.reveal = Some((self.game.guesses.len() - 1, Instant::now()));
        } else {
            self.after_guess();
        }
    }

    /// The result as text to share: the score and one row of squares per guess.
    pub fn share_text(&self) -> String {
        let game = &self.game;
        let (green, yellow) = if self.theme.name == "contrast" { ("🟧", "🟦") } else { ("🟩", "🟨") };
        let score = if game.status == Status::Won { game.guesses.len().to_string() } else { "X".to_string() };
        let mut text = match game.mode {
            Mode::Daily => format!("Wordl #{} {score}/6", self.daily_number()),
            Mode::Practice => format!("Wordl practice {score}/6"),
        };
        text.push_str(&"*".repeat(game.difficulty.index()));
        text.push('\n');
        for marks in &game.marks {
            text.push('\n');
            for mark in marks {
                text.push_str(match mark {
                    game::Mark::Green => green,
                    game::Mark::Yellow => yellow,
                    game::Mark::Gray => "⬛",
                });
            }
        }
        text.push('\n');
        text
    }

    /// Runs one action. Keys and clicks both end up here.
    pub fn act(&mut self, action: Action) {
        match action {
            Action::Key(KeyId::Letter(letter)) => self.type_letter(letter),
            Action::Key(KeyId::Enter) => self.submit(),
            Action::Key(KeyId::Back) => self.backspace(),
            Action::Help => (self.modal, self.pending_stats) = (Modal::Help, None),
            Action::Stats => (self.modal, self.pending_stats) = (Modal::Stats, None),
            Action::Close => self.modal = Modal::None,
            Action::New => self.new_game(Mode::Practice),
            Action::Daily => self.new_game(Mode::Daily),
            Action::Theme => {
                self.theme = theme::theme(theme::next_name(self.theme.name), self.truecolor);
                self.stats.set("theme", self.theme.name);
                self.stats.save();
                if self.modal == Modal::None && self.game.playing() && !self.small() {
                    self.toast(format!("Theme: {}", self.theme.name), MessageKind::Plain, 2);
                }
            }
            Action::Difficulty => {
                self.chosen = self.chosen.next();
                self.stats.set("hard", self.chosen.index());
                self.stats.save();
                if self.small() || !self.game.playing() {
                    return;
                }
                // A game under way can be made easier, but not harder: its earlier
                // guesses were not held to the stricter rules.
                if self.chosen <= self.game.difficulty || self.game.guesses.is_empty() {
                    self.game.difficulty = self.chosen;
                    self.stats.save_daily(&self.game);
                    self.toast(format!("Difficulty: {}", self.chosen.name()), MessageKind::Plain, 2);
                } else {
                    self.toast(format!("{} starts next game", self.chosen.name()), MessageKind::Plain, 3);
                }
            }
            // Asks first: one stray key must not end a game.
            Action::GiveUp => {
                if self.game.playing() && !self.small() {
                    (self.modal, self.pending_stats) = (Modal::GiveUp, None);
                }
            }
            // A loss, with the answer shown.
            Action::Surrender => {
                if self.game.playing() {
                    self.modal = Modal::None;
                    self.game.give_up();
                    self.stats.save_daily(&self.game);
                    self.stats.record(&self.game);
                    self.after_guess();
                }
            }
            Action::Copy => {
                if self.game.playing() {
                    return;
                }
                self.modal = Modal::None;
                let copied = copy_to_clipboard(&self.share_text());
                self.toast(if copied { "Result copied" } else { "No clipboard tool found" }, if copied { MessageKind::Plain } else { MessageKind::Error }, 3);
                self.restore_end = true;
            }
            Action::Quit => self.quit = true,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.modifiers.contains(KeyModifiers::ALT) {
            return;
        }
        self.pending_stats = None;
        match key.code {
            KeyCode::Char('q' | 'c') if ctrl => return self.act(Action::Quit),
            KeyCode::Char('l') if ctrl => return self.repaint = true,
            KeyCode::Char('t') if ctrl => return self.act(Action::Theme),
            _ => {}
        }
        if self.small() {
            return;
        }
        let playing = self.game.playing();
        match (self.modal, key.code) {
            (Modal::None, _) => {}
            (Modal::Stats, KeyCode::Char('n' | 'N')) if !playing => return self.act(Action::New),
            // Enter submits guesses and is easily pressed once too often: here it must
            // neither start the next word nor close the result before it has been read.
            (Modal::Stats, KeyCode::Enter) if !playing => return,
            (Modal::Stats, KeyCode::Char('c' | 'C')) if !playing => return self.act(Action::Copy),
            (Modal::GiveUp, KeyCode::Enter | KeyCode::Char('y' | 'Y')) => return self.act(Action::Surrender),
            _ => return self.act(Action::Close),
        }
        // Every letter types, so commands are Ctrl keys or `?`.
        match key.code {
            KeyCode::Char('n') if ctrl => self.act(Action::New),
            KeyCode::Char('d') if ctrl => self.act(Action::Daily),
            KeyCode::Char('s') if ctrl => self.act(Action::Stats),
            KeyCode::Char('x') if ctrl => self.act(Action::Difficulty),
            KeyCode::Char('g') if ctrl => self.act(Action::GiveUp),
            KeyCode::Char(_) if ctrl => {}
            KeyCode::Char(c) if c.is_ascii_alphabetic() => self.type_letter(c as u8),
            KeyCode::Char('?') | KeyCode::F(1) => self.act(Action::Help),
            KeyCode::Enter => self.submit(),
            KeyCode::Backspace | KeyCode::Delete => self.backspace(),
            _ => {}
        }
    }

    /// A left click presses whatever is under it; with a dialog open, a click anywhere
    /// else closes the dialog.
    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        self.pending_stats = None;
        match ui::action_at(self, mouse.column as i32, mouse.row as i32) {
            Some(action) => self.act(action),
            None if self.modal != Modal::None => self.act(Action::Close),
            None => {}
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= chunk.len() { TABLE[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// Copies with the first clipboard tool found; failing that, asks the terminal to
/// (OSC 52), which cannot report back, so that counts as done.
fn copy_to_clipboard(text: &str) -> bool {
    let tools: [&[&str]; 5] = [&["wl-copy"], &["pbcopy"], &["xclip", "-selection", "clipboard"], &["xsel", "-ib"], &["clip.exe"]];
    for tool in tools {
        let child = Command::new(tool[0]).args(&tool[1..]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            let written = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
            if child.wait().is_ok_and(|status| status.success()) && written {
                return true;
            }
        }
    }
    let mut out = std::io::stdout().lock();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes())).and_then(|()| out.flush()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn app(name: &str) -> (App, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wordl-test-{}-app-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = Options { mode: Mode::Practice, theme: None, difficulty: None, animate: false, truecolor: true, debug_answer: game::word("CRANE") };
        (App::new(Stats::load(dir.clone()), options), dir)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn type_word(app: &mut App, word: &str) {
        for c in word.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
    }

    fn message(app: &App) -> &str {
        app.message.as_ref().map_or("", |m| m.text.as_str())
    }

    #[test]
    fn a_game_is_typed_refused_and_won() {
        let (mut app, dir) = app("win");
        type_word(&mut app, "sla");
        assert_eq!(message(&app), "Not enough letters");
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(app.game.cur, b"SL");
        type_word(&mut app, "qqq");
        assert_eq!(message(&app), "Not in word list");
        for _ in 0..5 {
            app.on_key(key(KeyCode::Backspace));
        }
        type_word(&mut app, "slate");
        assert_eq!((app.game.guesses.len(), message(&app)), (1, ""));
        type_word(&mut app, "CRANE");
        assert_eq!(app.game.status, Status::Won);
        assert_eq!(message(&app), "Magnificent! Solved in 2/6");
        assert_eq!((app.stats.of(Mode::Practice, "wins"), app.stats.of(Mode::Practice, "d2")), (1, 1));
        // Enter on a finished game opens the statistics, and more of it changes nothing:
        // the result stays up until it is answered with N, C or Esc.
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.modal, Modal::Stats);
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.modal, app.game.status), (Modal::Stats, Status::Won));
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!((app.modal, app.game.guesses.len(), app.game.status), (Modal::None, 0, Status::Playing));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn six_wrong_guesses_lose() {
        let (mut app, dir) = app("lose");
        for word in ["slate", "count", "world", "house", "mound", "plant"] {
            type_word(&mut app, word);
        }
        assert_eq!((app.game.status, message(&app)), (Status::Lost, "The word was CRANE"));
        assert_eq!((app.stats.of(Mode::Practice, "played"), app.stats.of(Mode::Practice, "wins")), (1, 0));
        assert_eq!(app.share_text(), "Wordl practice X/6\n\n⬛⬛🟩⬛🟩\n🟩⬛⬛🟩⬛\n⬛⬛🟨⬛⬛\n⬛⬛⬛⬛🟩\n⬛⬛⬛🟩⬛\n⬛⬛🟩🟩⬛\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn difficulty_can_ease_mid_game_but_not_tighten() {
        let (mut app, dir) = app("difficulty");
        app.on_key(ctrl('x'));
        assert_eq!((app.chosen, app.game.difficulty, message(&app)), (Difficulty::Hard, Difficulty::Hard, "Difficulty: Hard"));
        type_word(&mut app, "slate");
        type_word(&mut app, "count");
        assert_eq!(message(&app), "3rd letter must be A");
        app.on_key(ctrl('x'));
        assert_eq!((app.chosen, app.game.difficulty, message(&app)), (Difficulty::Ultra, Difficulty::Hard, "Ultra Hard starts next game"));
        app.on_key(ctrl('x'));
        assert_eq!((app.chosen, app.game.difficulty), (Difficulty::Normal, Difficulty::Normal));
        // The choice is remembered.
        assert_eq!(Stats::load(dir.clone()).difficulty(), Difficulty::Normal);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn giving_up_asks_first_and_counts_as_a_loss() {
        let (mut app, dir) = app("giveup");
        type_word(&mut app, "slate");
        app.on_key(ctrl('g'));
        assert_eq!((app.modal, app.game.status), (Modal::GiveUp, Status::Playing));
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.modal, app.game.status), (Modal::None, Status::Playing));
        app.on_key(ctrl('g'));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.modal, app.game.status, app.game.gave_up), (Modal::None, Status::Lost, true));
        assert_eq!(message(&app), "The word was CRANE");
        assert_eq!((app.stats.of(Mode::Practice, "played"), app.stats.of(Mode::Practice, "wins")), (1, 0));
        // Not twice.
        app.act(Action::Surrender);
        assert_eq!(app.stats.of(Mode::Practice, "played"), 1);
        app.act(Action::New);
        assert_eq!((app.game.status, app.game.gave_up), (Status::Playing, false));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_given_up_daily_puzzle_stays_given_up() {
        let (mut app, dir) = app("daily");
        app.on_key(ctrl('d'));
        assert_eq!(app.game.mode, Mode::Daily);
        // Whatever today's word is, this is a valid first guess or the answer itself.
        type_word(&mut app, "slate");
        if app.game.playing() {
            app.act(Action::Surrender);
            app.on_key(ctrl('n'));
            app.on_key(ctrl('d'));
            assert_eq!((app.game.status, app.game.gave_up, app.game.guesses.len()), (Status::Lost, true, 1));
            assert_eq!(app.stats.of(Mode::Daily, "played"), 1);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn themes_cycle_and_are_remembered() {
        let (mut app, dir) = app("theme");
        app.on_key(ctrl('t'));
        assert_eq!((app.theme.name, message(&app)), ("daylight", "Theme: daylight"));
        assert_eq!(Stats::load(dir.clone()).theme(), "daylight");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
