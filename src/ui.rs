//! All drawing. Pure functions of `&App`: nothing here changes state.
//!
//! The whole screen is redrawn every frame from the state and the terminal size;
//! ratatui sends only the cells that changed. Where things are clicked is worked out by
//! the same geometry functions that draw them, so the two cannot drift apart.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};

use crate::app::{Action, App, FLASH_STEP, FLIP, KeyId, MessageKind, Modal, SHAKE_STEP};
use crate::font::{self, Label};
use crate::game::{self, Mark, Mode, Status};
use crate::layout::{self, Layout};
use crate::theme::{self, Paint, Shades, Theme};
use crate::words;

const KEY_ROWS: [&str; 3] = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"];
/// Dialog lines are at most this wide, so a dialog fits the narrowest supported
/// terminal (39 columns) with its frame and padding.
const DIALOG_WIDTH: i32 = 35;

fn on(fg: Color, bg: Color) -> Style {
    Style::new().fg(fg).bg(bg)
}

fn width(s: &str) -> i32 {
    s.chars().count() as i32
}

/// `s` in the middle of `w` cells.
fn centered(s: &str, w: i32) -> String {
    let left = ((w - width(s)) / 2).max(0);
    let right = (w - width(s) - left).max(0);
    format!("{}{s}{}", " ".repeat(left as usize), " ".repeat(right as usize))
}

struct Canvas<'a> {
    buf: &'a mut Buffer,
    th: &'a Theme,
}

impl Canvas<'_> {
    /// Writes `s` one cell per character, replacing whatever was there. Anything off
    /// screen is dropped, so callers never have to clip.
    fn put(&mut self, x: i32, y: i32, s: &str, style: Style) {
        if y < 0 {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            let cx = x + i as i32;
            if cx >= 0
                && let Some(cell) = self.buf.cell_mut((cx as u16, y as u16))
            {
                cell.reset();
                cell.set_char(ch).set_style(style);
            }
        }
    }

    /// `s` centered in the `w` cells from `x0`, cut short if it is longer.
    fn center(&mut self, y: i32, x0: i32, w: i32, s: &str, style: Style) {
        let s: String = s.chars().take(w.max(0) as usize).collect();
        self.put(x0 + (w - width(&s)) / 2, y, &s, style);
    }

    /// Draws one tile or key at a size level's height `t`:
    ///   1    one row of text on a colored background
    ///   3    half-block edges around one row of text
    ///   5+   half-block edges around a bitmap glyph
    ///
    /// The first and last rows are half rows (`▄` on top, `▀` at the bottom, in the
    /// block's color on the screen background), which is what puts a gap between
    /// blocks without spending a row on it. `fill` paints the block in `bg`; otherwise
    /// it is a frame in `bg` around the label. `inset` squashes the block vertically,
    /// for the flip animation.
    ///
    /// A filled block is drawn raised, as pixel art, where the theme has shades for it
    /// (see `sprite`); `sunken` presses it in instead.
    #[allow(clippy::too_many_arguments)]
    fn block(&mut self, x: i32, y: i32, w: i32, t: i32, fill: bool, bg: Paint, fg: Paint, label: Label, inset: i32, sunken: bool) {
        let screen = self.th.bg.bg;
        if t < 3 {
            return self.put(x, y, &centered(&label.text(w), w), on(fg.fg, bg.bg).bold());
        }
        let (top, bottom) = (inset, t - 1 - inset);
        let edge = on(bg.fg, screen);
        if bottom < top {
            return;
        }
        if bottom == top {
            return self.put(x, y + top, &"━".repeat(w as usize), edge);
        }
        let shades = self.th.shades(bg).filter(|_| fill);
        if let Some(shades) = shades
            && t >= 5
        {
            return self.sprite(x, y, w, t, bg, fg, label, inset, sunken, shades);
        }
        // With one row of text there is no room for more than a lit top edge and a
        // dark bottom one.
        let (above, below) = match shades {
            Some(s) if sunken => (s.dark, s.light),
            Some(s) => (s.light, s.dark),
            None => (bg.fg, bg.fg),
        };
        self.put(x, y + top, &"▄".repeat(w as usize), on(above, screen));
        self.put(x, y + bottom, &"▀".repeat(w as usize), on(below, screen));
        let rows = if t == 3 { vec![centered(&label.text(w), w)] } else { font::glyph(label, w, t) };
        for i in top + 1..bottom {
            let Some(row) = rows.get((i - 1) as usize) else { continue };
            if fill {
                self.put(x, y + i, row, on(fg.fg, bg.bg).bold());
            } else {
                let inner: String = row.chars().skip(1).take((w - 2).max(0) as usize).collect();
                self.put(x, y + i, "█", edge);
                self.put(x + 1, y + i, &inner, on(fg.fg, screen).bold());
                self.put(x + w - 1, y + i, "█", edge);
            }
        }
    }

    /// A filled block as pixel art: lit top and left edges, dark bottom and right
    /// edges, and a letter that casts a shadow down and to the right.
    ///
    /// Every pixel has its own color. A cell holds two of them, one above the other:
    /// it is drawn as `▀` in the color of the upper pixel on a background in the color
    /// of the lower one. The block's first and last pixel rows are the screen's, as
    /// for flat blocks, which keeps the gap between blocks.
    #[allow(clippy::too_many_arguments)]
    fn sprite(&mut self, x: i32, y: i32, w: i32, t: i32, bg: Paint, fg: Paint, label: Label, inset: i32, sunken: bool, shades: Shades) {
        let screen = self.th.bg.bg;
        let ink = font::pixels(label, w, t);
        let inked = |col: i32, row: i32| row >= 2 && col >= 0 && ink.get((row - 2) as usize).and_then(|r| r.get(col as usize)).copied().unwrap_or(false);
        let (lit, unlit) = if sunken { (shades.dark, shades.light) } else { (shades.light, shades.dark) };
        // Big blocks get a thicker edge and a longer shadow, so the look scales.
        let edge = if t >= 10 { 2 } else { 1 };
        let reach = (font::scale_for(t) + 1) / 2;
        // A light letter throws a dark shadow; a dark letter on a bright tile gets a
        // light one instead, and reads as engraved.
        let cast = if theme::brightness(fg) >= theme::brightness(bg) { shades.shadow } else { shades.light };
        let (first, last) = (1 + 2 * inset, 2 * t - 2 - 2 * inset);
        let color_at = |col: i32, row: i32| {
            if row < first || row > last {
                screen
            } else if inked(col, row) {
                fg.fg
            } else if row - first < edge || col < edge {
                lit
            } else if last - row < edge || w - 1 - col < edge {
                unlit
            } else if !sunken && inked(col - reach, row - reach) {
                cast
            } else {
                bg.bg
            }
        };
        for i in inset..t - inset {
            for col in 0..w {
                let (upper, lower) = (color_at(col, 2 * i), color_at(col, 2 * i + 1));
                self.put(x + col, y + i, if upper == lower { " " } else { "▀" }, on(upper, lower));
            }
        }
    }
}

/// Breaks text into lines of at most `width` characters, at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

fn mark_paints(th: &Theme, mark: Mark) -> (Paint, Paint) {
    match mark {
        Mark::Green => (th.g, th.gfg),
        Mark::Yellow => (th.y, th.yfg),
        Mark::Gray => (th.x, th.xfg),
    }
}

/// The line under the title: which game this is.
fn info_text(app: &App) -> String {
    let game = &app.game;
    let mut s = match game.mode {
        Mode::Daily => format!("Daily #{}", app.daily_number()),
        Mode::Practice => "Practice".to_string(),
    };
    if game.difficulty != game::Difficulty::Normal {
        s += &format!(" · {}", game.difficulty.name());
    }
    let streak = app.stats.of(game.mode, "streak");
    if streak > 0 {
        s += &format!(" · Streak {streak}");
    }
    s
}

fn draw_title(c: &mut Canvas, app: &App, l: &Layout) {
    let th = c.th;
    let info = info_text(app);
    let dim = on(th.dim.fg, th.bg.bg);
    let mut x = l.hx;
    match l.info_y {
        Some(y) => c.center(y, l.rx, l.rw, &info, dim),
        // On one row: the title, two spaces, the info, centered together when they fit.
        None if 19 + 2 + width(&info) <= l.rw => {
            x = l.rx + (l.rw - 21 - width(&info)) / 2;
            c.put(x + 21, l.hy, &info, dim);
        }
        None => {}
    }
    let colors = [(th.g, th.gfg), (th.y, th.yfg), (th.x, th.xfg), (th.g, th.gfg), (th.y, th.yfg)];
    for (i, (letter, (bg, fg))) in "WORDL".bytes().zip(colors).enumerate() {
        c.block(x + i as i32 * l.title.px, l.hy, l.title.w, l.title.t, true, bg, fg, Label::Letter(letter), 0, false);
    }
}

/// How far the row being typed is pushed sideways by the shake after a refused guess.
fn shake_offset(app: &App, l: &Layout) -> i32 {
    let Some(start) = app.shake else { return 0 };
    let (bw, _) = l.board.grid(5, 6);
    let room = 2.min(l.bx).min(l.cols - l.bx - bw);
    if room < 1 {
        return 0;
    }
    let step = (app.now.saturating_duration_since(start).as_millis() / SHAKE_STEP.as_millis()) as usize;
    [-room, room, -room, room, -1, 1, 0].get(step).copied().unwrap_or(0)
}

fn draw_tile(c: &mut Canvas, app: &App, l: &Layout, row: usize, col: usize) {
    let (th, game) = (c.th, &app.game);
    let guessed = game.guesses.len();
    let d = l.board;
    let mut x = l.bx + col as i32 * d.px;
    let y = l.by + row as i32 * d.py;
    if row == guessed {
        x += shake_offset(app, l);
    }

    // A guess is revealed one tile at a time: each squashes flat in its typed look,
    // then grows back in its color.
    let (mut revealed, mut inset) = (row < guessed, 0);
    if let Some((reveal_row, start)) = app.reveal
        && reveal_row == row
    {
        let elapsed = app.now.saturating_duration_since(start).as_millis();
        let turning = (elapsed / FLIP.as_millis()) as usize;
        let phase = (elapsed % FLIP.as_millis()) as f32 / FLIP.as_millis() as f32;
        let half = (d.t / 2) as f32;
        if col > turning {
            revealed = false;
        } else if col == turning {
            revealed = phase >= 0.5;
            inset = (half * 2.0 * if revealed { 1.0 - phase } else { phase }).round() as i32;
        }
    }

    if revealed {
        let (mut bg, mut fg) = mark_paints(th, game.marks[row][col]);
        // The flash that runs along a winning row.
        if let Some((flash_row, start)) = app.celebrate
            && flash_row == row
            && (app.now.saturating_duration_since(start).as_millis() / FLASH_STEP.as_millis()) as usize == col
        {
            (bg, fg) = (th.win, th.gfg);
        }
        return c.block(x, y, d.w, d.t, true, bg, fg, Label::Letter(game.guesses[row][col]), inset, false);
    }
    let letter = if row < guessed {
        Some(game.guesses[row][col])
    } else if row == guessed && game.gave_up {
        // After giving up, the answer is shown in the row the next guess would have used.
        Some(game.answer[col])
    } else if row == guessed {
        game.cur.get(col).copied()
    } else {
        None
    };
    let label = letter.map_or(Label::None, Label::Letter);
    if d.t == 1 {
        c.block(x, y, d.w, 1, true, th.empty, th.fg, label, 0, false);
    } else {
        c.block(x, y, d.w, d.t, false, if letter.is_some() { th.typed } else { th.empty }, th.fg, label, inset, false);
    }
}

/// Every key of the on-screen keyboard as (x, y, width, key). Backspace is bottom left
/// and Enter bottom right.
pub fn keyboard_keys(l: &Layout) -> Vec<(i32, i32, i32, KeyId)> {
    let d = l.keys;
    let gap = d.px - d.w;
    let mut keys = Vec::with_capacity(28);
    for (r, letters) in KEY_ROWS.iter().enumerate() {
        let y = l.ky + r as i32 * d.py;
        let mut x = l.kx + if r == 1 { d.px / 2 } else { 0 };
        if r == 2 {
            keys.push((x, y, l.wide_left, KeyId::Back));
            x += l.wide_left + gap;
        }
        for letter in letters.bytes() {
            keys.push((x, y, d.w, KeyId::Letter(letter)));
            x += d.px;
        }
        if r == 2 {
            keys.push((x, y, l.wide_right, KeyId::Enter));
        }
    }
    keys
}

fn draw_keyboard(c: &mut Canvas, app: &App, l: &Layout) {
    let th = c.th;
    // A key takes its color once the guess that earned it has been revealed.
    let shown = app.game.guesses.len() - app.reveal.is_some() as usize;
    for (x, y, w, key) in keyboard_keys(l) {
        let (label, mark) = match key {
            KeyId::Letter(letter) => (Label::Letter(letter), app.game.key_mark(letter, shown)),
            KeyId::Enter => (Label::Enter, None),
            KeyId::Back => (Label::Back, None),
        };
        let (bg, fg) = match mark {
            Some(Mark::Gray) => (th.keyx, th.keyxfg),
            Some(mark) => mark_paints(th, mark),
            None => (th.key, th.keyfg),
        };
        // A key known not to be in the word looks pressed in.
        c.block(x, y, w, l.keys.t, true, bg, fg, label, 0, mark == Some(Mark::Gray));
    }
}

fn draw_message(c: &mut Canvas, app: &App, l: &Layout) {
    let th = c.th;
    let Some(message) = &app.message else { return };
    let (text, style) = match message.kind {
        MessageKind::Win => (format!(" {} ", message.text), on(th.gfg.fg, th.g.bg)),
        MessageKind::Error => (format!(" {} ", message.text), on(th.toastfg.fg, th.toast.bg)),
        MessageKind::Plain => (message.text.clone(), on(th.fg.fg, th.bg.bg)),
    };
    c.center(l.msg_y, l.rx, l.rw, &text, style.bold());
}

pub struct FooterItem {
    pub x: i32,
    pub key: &'static str,
    pub label: String,
    pub action: Action,
}

/// The footer's buttons, positioned for a terminal `cols` wide, and whether there is
/// room for their labels (otherwise only the keys are shown).
pub fn footer_items(app: &App, cols: i32) -> (Vec<FooterItem>, bool) {
    let level = app.chosen.name().split(' ').next().unwrap_or_default();
    let items = [
        ("?", "Help", Action::Help),
        ("^N", "New", Action::New),
        ("^D", "Daily", Action::Daily),
        ("^S", "Stats", Action::Stats),
        ("^T", "Theme", Action::Theme),
        ("^X", level, Action::Difficulty),
        ("^G", "Give up", Action::GiveUp),
        ("^Q", "Quit", Action::Quit),
    ];
    let count = items.len() as i32;
    let with_labels: i32 = items.iter().map(|(key, label, _)| width(key) + 1 + width(label)).sum();
    let keys_only: i32 = items.iter().map(|(key, ..)| width(key)).sum();
    let (labels, sep, total) = [3, 2, 1]
        .into_iter()
        .map(|sep| (true, sep, with_labels + sep * (count - 1)))
        .find(|&(_, _, total)| total <= cols - 2)
        .unwrap_or((false, 2, keys_only + 2 * (count - 1)));
    let mut x = (cols - total) / 2;
    let placed = items
        .into_iter()
        .map(|(key, label, action)| {
            let item = FooterItem { x, key, label: label.to_string(), action };
            x += width(key) + if labels { 1 + width(label) } else { 0 } + sep;
            item
        })
        .collect();
    (placed, labels)
}

fn draw_footer(c: &mut Canvas, app: &App, l: &Layout) {
    let th = c.th;
    let (items, labels) = footer_items(app, l.cols);
    for item in items {
        c.put(item.x, l.footer_y, item.key, on(th.accent.fg, th.bg.bg).bold());
        if !labels {
            continue;
        }
        // The difficulty's name is colored when it is not Normal.
        let style = match (item.action, app.chosen) {
            (Action::Difficulty, game::Difficulty::Hard) => on(th.g.fg, th.bg.bg).bold(),
            (Action::Difficulty, game::Difficulty::Ultra) => on(th.y.fg, th.bg.bg).bold(),
            _ => on(th.dim.fg, th.bg.bg),
        };
        c.put(item.x + width(item.key) + 1, l.footer_y, &item.label, style);
    }
}

/// One line of a dialog: styled pieces, centered or from the left.
struct DialogLine {
    spans: Vec<(String, Style)>,
    center: bool,
}

impl DialogLine {
    fn width(&self) -> i32 {
        self.spans.iter().map(|(s, _)| width(s)).sum()
    }
}

struct Dialog {
    lines: Vec<DialogLine>,
    buttons: Vec<(&'static str, Action)>,
}

fn dialog(app: &App) -> Option<Dialog> {
    let th = &app.theme;
    let panel = th.panel.bg;
    let text = on(th.fg.fg, panel);
    let dim = on(th.dim.fg, panel);
    let accent = on(th.accent.fg, panel).bold();
    let line = |spans: Vec<(String, Style)>| DialogLine { spans, center: false };
    let middle = |s: &str, style: Style| DialogLine { spans: vec![(s.to_string(), style)], center: true };
    let blank = || DialogLine { spans: Vec::new(), center: false };
    let game = &app.game;

    Some(match app.modal {
        Modal::None => return None,
        Modal::Help => {
            let mut lines = vec![middle("HOW TO PLAY", accent), blank()];
            for s in ["Guess the hidden word in 6 tries.", "Each guess must be a real 5-letter", "word. The tiles then show:"] {
                lines.push(line(vec![(s.to_string(), text)]));
            }
            lines.push(blank());
            for (mark, meaning) in
                [(Mark::Green, "right letter, right spot"), (Mark::Yellow, "right letter, wrong spot"), (Mark::Gray, "letter is not in the word")]
            {
                let (bg, fg) = mark_paints(th, mark);
                lines.push(line(vec![(" A ".to_string(), on(fg.fg, bg.bg).bold()), (format!("  {meaning}"), text)]));
            }
            lines.push(blank());
            for (k1, d1, k2, d2) in [
                ("A-Z", "type", "Enter", "submit"),
                ("Bksp", "delete", "?", "this help"),
                ("^N", "new game", "^D", "daily puzzle"),
                ("^S", "statistics", "^T", "change theme"),
                ("^X", "difficulty", "^G", "give up"),
                ("^Q", "quit", "", ""),
            ] {
                let pad = " ".repeat((17 - width(k1) - width(d1)).max(0) as usize);
                lines.push(line(vec![(k1.to_string(), accent), (format!(" {d1}{pad}"), text), (k2.to_string(), accent), (format!(" {d2}"), text)]));
            }
            lines.push(blank());
            for (level, rule) in [("Normal", "any real word"), ("Hard", "reuse green and yellow"), ("Ultra", "obey every clue")] {
                lines.push(line(vec![(format!("{level:<8}"), text.bold()), (rule.to_string(), dim)]));
            }
            Dialog { lines, buttons: vec![("Esc Close", Action::Close)] }
        }
        Modal::Stats => {
            let mode = game.mode;
            let played = app.stats.of(mode, "played");
            let percent = if played > 0 { app.stats.of(mode, "wins") * 100 / played } else { 0 };
            let mut lines = vec![middle(&format!("STATISTICS · {}", mode.key().to_uppercase()), accent), blank()];
            // Once a game is over the word is given with what it means: the players are
            // children, and this is where a new word gets learned.
            let answer = game::text(&game.answer);
            let meaning = words::definition(&answer).unwrap_or_default();
            let word_style = on(th.y.fg, panel).bold();
            match game.status {
                Status::Won => {
                    lines.push(middle(&format!("Solved in {}/6", game.guesses.len()), on(th.g.fg, panel).bold()));
                    // "CRANE: a tall bird..." with the word picked out on the first line.
                    for (i, row) in wrap(&format!("{answer}: {meaning}"), DIALOG_WIDTH as usize).into_iter().enumerate() {
                        match row.strip_prefix(answer.as_str()).filter(|_| i == 0 && !meaning.is_empty()) {
                            Some(rest) => lines.push(DialogLine { spans: vec![(answer.clone(), word_style), (rest.to_string(), text)], center: true }),
                            None if !meaning.is_empty() => lines.push(middle(&row, text)),
                            None => {}
                        }
                    }
                    lines.push(blank());
                }
                Status::Lost => {
                    if game.gave_up {
                        lines.push(middle("You gave up", dim));
                    }
                    lines.push(DialogLine { spans: vec![("The word was ".to_string(), text), (answer, word_style)], center: true });
                    lines.extend(wrap(meaning, DIALOG_WIDTH as usize).iter().map(|row| middle(row, text)));
                    lines.push(blank());
                }
                Status::Playing => {}
            }
            lines.push(middle(&format!("{:>6}  {:>6}  {:>6}  {:>6}", "Played", "Win %", "Streak", "Best"), dim));
            let row = format!("{played:>6}  {percent:>6}  {:>6}  {:>6}", app.stats.of(mode, "streak"), app.stats.of(mode, "best"));
            lines.extend([middle(&row, text.bold()), blank(), line(vec![("GUESS DISTRIBUTION".to_string(), dim)])]);
            let counts: Vec<i64> = (1..=6).map(|i| app.stats.of(mode, &format!("d{i}"))).collect();
            let most = counts.iter().copied().max().unwrap_or(0).max(1);
            for (i, &count) in counts.iter().enumerate() {
                // The count sits at the right end of its bar, so a bar is never
                // narrower than its number.
                let len = ((count * 26 / most) as usize).max(count.to_string().len() + 1);
                let this_game = game.status == Status::Won && game.guesses.len() == i + 1;
                let (bg, fg) = if this_game { (th.g, th.gfg) } else { (th.key, th.keyfg) };
                lines.push(line(vec![(format!("{} ", i + 1), dim), (format!("{count:>len$} "), on(fg.fg, bg.bg).bold())]));
            }
            let buttons = match game.status {
                Status::Playing => vec![("Esc Close", Action::Close)],
                _ => vec![("Enter New", Action::New), ("C Copy", Action::Copy), ("Esc Close", Action::Close)],
            };
            Dialog { lines, buttons }
        }
        Modal::GiveUp => Dialog {
            lines: vec![middle("GIVE UP?", accent), blank(), middle("The word will be shown and the", text), middle("game counts as a loss.", text)],
            buttons: vec![("Enter Give up", Action::Surrender), ("Esc Keep playing", Action::Close)],
        },
    })
}

/// Where a dialog and its buttons are for a terminal of `cols` by `rows`.
struct DialogBox {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    inner: i32,
    /// Lines that fit; a dialog taller than the terminal loses its last ones.
    shown: usize,
    /// Each button as (x, y, label, action); it is drawn as the label with a space on
    /// either side.
    buttons: Vec<(i32, i32, &'static str, Action)>,
}

fn dialog_box(dialog: &Dialog, cols: i32, rows: i32) -> DialogBox {
    let inner = dialog.lines.iter().map(DialogLine::width).max().unwrap_or(0).max(DIALOG_WIDTH);
    let w = inner + 4;
    let shown = (dialog.lines.len() as i32).min(rows - 4).max(0);
    let h = shown + 4;
    let (x, y) = (((cols - w) / 2).max(0), ((rows - h) / 2).max(0));
    let buttons_width: i32 = dialog.buttons.iter().map(|(label, _)| width(label) + 4).sum::<i32>() - 2;
    let mut bx = x + 2 + (inner - buttons_width) / 2;
    let buttons = dialog
        .buttons
        .iter()
        .map(|&(label, action)| {
            let button = (bx, y + h - 2, label, action);
            bx += width(label) + 4;
            button
        })
        .collect();
    DialogBox { x, y, w, h, inner, shown: shown as usize, buttons }
}

fn draw_dialog(c: &mut Canvas, app: &App, l: &Layout) {
    let Some(dialog) = dialog(app) else { return };
    let th = c.th;
    let b = dialog_box(&dialog, l.cols, l.rows);
    let panel = on(th.fg.fg, th.panel.bg);
    let border = on(th.panelb.fg, th.panel.bg);
    let bar = "─".repeat((b.w - 2) as usize);
    c.put(b.x, b.y, &format!("╭{bar}╮"), border);
    c.put(b.x, b.y + b.h - 1, &format!("╰{bar}╯"), border);
    for i in 1..b.h - 1 {
        c.put(b.x, b.y + i, "│", border);
        c.put(b.x + 1, b.y + i, &" ".repeat((b.w - 2) as usize), panel);
        c.put(b.x + b.w - 1, b.y + i, "│", border);
    }
    for (i, line) in dialog.lines.iter().take(b.shown).enumerate() {
        let mut x = b.x + 2 + if line.center { (b.inner - line.width()) / 2 } else { 0 };
        for (text, style) in &line.spans {
            c.put(x, b.y + 1 + i as i32, text, *style);
            x += width(text);
        }
    }
    for (x, y, label, _) in b.buttons {
        c.put(x, y, &format!(" {label} "), on(th.btnfg.fg, th.btn.bg).bold());
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let (cols, rows) = (area.width as i32, area.height as i32);
    let th = &app.theme;
    let mut c = Canvas { buf: frame.buffer_mut(), th };
    let screen = on(th.fg.fg, th.bg.bg);
    for y in 0..rows {
        c.put(0, y, &" ".repeat(cols as usize), screen);
    }
    let Some(l) = layout::layout(cols, rows) else {
        c.center((rows - 1) / 2, 0, cols, &format!("wordl needs {}x{}", layout::MIN_COLS, layout::MIN_ROWS), screen.bold());
        if rows > 2 {
            c.center((rows - 1) / 2 + 1, 0, cols, &format!("this is {cols}x{rows}"), on(th.dim.fg, th.bg.bg));
        }
        return;
    };
    draw_title(&mut c, app, &l);
    for row in 0..6 {
        for col in 0..5 {
            draw_tile(&mut c, app, &l, row, col);
        }
    }
    draw_message(&mut c, app, &l);
    draw_keyboard(&mut c, app, &l);
    draw_footer(&mut c, app, &l);
    draw_dialog(&mut c, app, &l);
}

/// What a click at (x, y) presses, if anything. With a dialog open only its buttons
/// can be pressed.
pub fn action_at(app: &App, x: i32, y: i32) -> Option<Action> {
    let l = layout::layout(app.size.0, app.size.1)?;
    if let Some(dialog) = dialog(app) {
        let b = dialog_box(&dialog, l.cols, l.rows);
        return b.buttons.into_iter().find(|&(bx, by, label, _)| y == by && x >= bx && x < bx + width(label) + 2).map(|(.., action)| action);
    }
    if let Some(&(.., key)) = keyboard_keys(&l).iter().find(|&&(kx, ky, w, _)| x >= kx && x < kx + w && y >= ky && y < ky + l.keys.py) {
        return Some(Action::Key(key));
    }
    let (items, labels) = footer_items(app, l.cols);
    items
        .into_iter()
        .find(|item| {
            // A column of slack on either side makes the small targets easier to hit.
            let w = width(item.key) + if labels { 1 + width(&item.label) } else { 0 };
            y == l.footer_y && x >= item.x - 1 && x <= item.x + w
        })
        .map(|item| item.action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Options;
    use crate::store::Stats;
    use crate::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    fn app(name: &str) -> (App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("wordl-test-{}-ui-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = Options { mode: Mode::Practice, theme: None, difficulty: None, animate: true, truecolor: true, debug_answer: game::word("CRANE") };
        (App::new(Stats::load(dir.clone()), options), dir)
    }

    /// The screen as text, one string per row.
    fn screen(app: &mut App, cols: u16, rows: u16) -> Vec<String> {
        app.size = (cols as i32, rows as i32);
        let mut terminal = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buf = terminal.backend().buffer();
        (0..rows).map(|y| (0..cols).map(|x| buf[(x, y)].symbol()).collect()).collect()
    }

    fn has(screen: &[String], text: &str) -> bool {
        screen.iter().any(|row| row.contains(text))
    }

    #[test]
    fn draws_the_game_at_80x24() {
        let (mut app, dir) = app("80x24");
        app.game.add_guess(game::word("SLATE").unwrap());
        app.game.cur = b"CR".to_vec();
        let s = screen(&mut app, 80, 24);
        assert!(s[0].contains(" W   O   R   D   L   Practice"), "{}", s[0]);
        // A revealed guess, then the one being typed in framed tiles.
        assert!(s[2].contains("  S      L      A      T      E  "), "{}", s[2]);
        assert!(s[5].contains("█ C █  █ R █  █   █"), "{}", s[5]);
        assert!(s[20].contains("Q   W   E   R   T   Y   U   I   O   P"));
        assert!(s[22].contains("⌫    Z   X   C   V   B   N   M    ↵"));
        assert!(s[23].contains("? Help") && s[23].contains("^G Give up") && s[23].contains("^X Normal"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn draws_big_letters_on_a_big_terminal() {
        let (mut app, dir) = app("big");
        app.game.cur = b"L".to_vec();
        let s = screen(&mut app, 190, 50);
        // The typed L as a bitmap inside its frame: a vertical stroke, then its foot.
        assert!(has(&s, "█ ██         █  █            █"), "no block letter");
        assert!(has(&s, "█ ██▄▄▄▄▄▄▄▄ █") || has(&s, "█ ██████████ █"), "no foot of the L");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A revealed tile on a big terminal is pixel art: every cell carries two pixels,
    /// the upper as the character's color and the lower as its background.
    #[test]
    fn revealed_tiles_are_shaded_pixel_art() {
        let (mut app, dir) = app("sprite");
        app.game.add_guess(game::word("CRANE").unwrap());
        app.size = (190, 50);
        let l = layout::layout(190, 50).unwrap();
        let cells = |app: &App| {
            let mut terminal = Terminal::new(TestBackend::new(190, 50)).unwrap();
            terminal.draw(|f| draw(f, app)).unwrap();
            let buf = terminal.backend().buffer().clone();
            move |x: i32, y: i32| buf[((l.bx + x) as u16, (l.by + y) as u16)].clone()
        };
        let th = app.theme;
        let shades = th.shades(th.g).unwrap();
        let at = cells(&app);
        // Top-left cell of the first tile: the screen above, the lit edge below.
        assert_eq!((at(0, 0).symbol(), at(0, 0).fg, at(0, 0).bg), ("▀", th.bg.bg, shades.light));
        // Further down the left edge both pixels are lit, so the cell is one color.
        assert_eq!((at(0, 3).symbol(), at(0, 3).bg), (" ", shades.light));
        // The right edge and the bottom edge are dark.
        assert_eq!(at(l.board.w - 1, 3).bg, shades.dark);
        assert_eq!((at(3, l.board.t - 1).fg, at(3, l.board.t - 1).bg), (shades.dark, th.bg.bg));
        // Somewhere on the tile there is the letter, its shadow, and the plain green.
        let seen: Vec<_> = (0..l.board.w).flat_map(|x| (0..l.board.t).map(move |y| (x, y))).flat_map(|(x, y)| [at(x, y).fg, at(x, y).bg]).collect();
        for color in [th.gfg.fg, shades.shadow, th.g.bg] {
            assert!(seen.contains(&color), "{color:?} is missing from the tile");
        }

        // The terminal theme has no shades to work with: its tiles stay flat.
        app.theme = theme::theme("terminal", true);
        let at = cells(&app);
        assert_eq!((at(0, 0).symbol(), at(0, 0).fg), ("▄", app.theme.g.fg));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn says_when_the_terminal_is_too_small() {
        let (mut app, dir) = app("small");
        let s = screen(&mut app, 30, 8);
        assert!(has(&s, "wordl needs 39x12") && has(&s, "this is 30x8"));
        // Nothing to draw into at all must not panic either.
        screen(&mut app, 1, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Every size, theme and dialog, mid-animation: drawing must never panic, and a
    /// dialog must keep its frame.
    #[test]
    fn draws_everything_at_any_size() {
        let (mut app, dir) = app("sweep");
        app.game.add_guess(game::word("SLATE").unwrap());
        app.game.add_guess(game::word("CRANE").unwrap());
        app.reveal = Some((1, Instant::now()));
        app.shake = Some(Instant::now());
        app.celebrate = Some((1, Instant::now()));
        for (cols, rows) in [(39, 12), (40, 13), (60, 20), (68, 7), (80, 24), (100, 30), (120, 40), (190, 50), (250, 70), (400, 100), (39, 100), (400, 12)] {
            for name in theme::NAMES {
                for truecolor in [true, false] {
                    app.theme = theme::theme(name, truecolor);
                    for modal in [Modal::None, Modal::Help, Modal::Stats, Modal::GiveUp] {
                        app.modal = modal;
                        let s = screen(&mut app, cols, rows);
                        if modal == Modal::None {
                            assert!(has(&s, "^Q"), "{cols}x{rows}");
                        } else {
                            assert!(has(&s, "╭") && has(&s, "╯"), "{cols}x{rows} {modal:?}");
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dialogs_fit_the_narrowest_terminal() {
        let (mut app, dir) = app("dialogs");
        for modal in [Modal::Help, Modal::Stats, Modal::GiveUp] {
            app.modal = modal;
            let d = dialog(&app).unwrap();
            assert!(d.lines.iter().all(|l| l.width() <= DIALOG_WIDTH), "{modal:?}");
            let buttons: i32 = d.buttons.iter().map(|(label, _)| width(label) + 4).sum::<i32>() - 2;
            assert!(buttons <= DIALOG_WIDTH, "{modal:?} buttons");
        }
        app.game.give_up();
        app.modal = Modal::Stats;
        let s = screen(&mut app, 39, 24);
        assert!(has(&s, "You gave up") && has(&s, "The word was CRANE") && has(&s, " Enter New "));
        // The meaning of the word comes with it, wrapped to the dialog.
        assert!(has(&s, "a tall bird with long legs; a") && has(&s, "machine that lifts"), "{s:#?}");

        // After a win the word is named with its meaning too.
        app.act(Action::New);
        app.game.answer = game::word("SKEIN").unwrap();
        app.game.add_guess(game::word("SKEIN").unwrap());
        app.modal = Modal::Stats;
        let s = screen(&mut app, 80, 24);
        assert!(has(&s, "Solved in 1/6") && has(&s, "SKEIN: a loose bundle of yarn or"), "{s:#?}");
        // The tallest the dialog gets still fits 24 rows, buttons and all.
        assert!(has(&s, "╭") && has(&s, "╯") && has(&s, " Esc Close "));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wraps_text_at_spaces() {
        assert_eq!(wrap("a tall bird with long legs; a machine that lifts", 35), ["a tall bird with long legs; a", "machine that lifts"]);
        assert_eq!(wrap("short", 35), ["short"]);
        assert!(wrap("", 35).is_empty());
        // Every definition fits the dialog in three lines, with the word in front.
        for word in crate::words::answers() {
            let line = format!("{}: {}", word.to_uppercase(), words::definition(word).unwrap());
            let rows = wrap(&line, DIALOG_WIDTH as usize);
            assert!(rows.len() <= 3 && rows.iter().all(|r| r.chars().count() <= DIALOG_WIDTH as usize), "{word}: {rows:?}");
        }
    }

    #[test]
    fn clicks_land_on_what_is_drawn_there() {
        let (mut app, dir) = app("clicks");
        let s = screen(&mut app, 80, 24);
        let find = |text: &str| {
            let (y, row) = s.iter().enumerate().find(|(_, row)| row.contains(text)).unwrap();
            (row[..row.find(text).unwrap()].chars().count() as i32, y as i32)
        };
        let (x, y) = find("Q   W");
        assert_eq!(action_at(&app, x, y), Some(Action::Key(KeyId::Letter(b'Q'))));
        assert_eq!(action_at(&app, x + 4, y), Some(Action::Key(KeyId::Letter(b'W'))));
        let (x, y) = find("⌫");
        assert_eq!(action_at(&app, x, y), Some(Action::Key(KeyId::Back)));
        let (x, y) = find("↵");
        assert_eq!(action_at(&app, x, y), Some(Action::Key(KeyId::Enter)));
        let (x, y) = find("^G Give up");
        assert_eq!(action_at(&app, x + 5, y), Some(Action::GiveUp));
        assert_eq!(action_at(&app, 0, 0), None);

        // With a dialog open, only its buttons can be pressed.
        app.modal = Modal::GiveUp;
        let s = screen(&mut app, 80, 24);
        let find = |text: &str| {
            let (y, row) = s.iter().enumerate().find(|(_, row)| row.contains(text)).unwrap();
            (row[..row.find(text).unwrap()].chars().count() as i32, y as i32)
        };
        let (x, y) = find(" Enter Give up ");
        assert_eq!(action_at(&app, x + 1, y), Some(Action::Surrender));
        let (x, y) = find(" Esc Keep playing ");
        assert_eq!(action_at(&app, x + 3, y), Some(Action::Close));
        assert_eq!(action_at(&app, 0, 23), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
