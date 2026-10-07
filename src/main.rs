mod app;
mod font;
mod layout;
mod theme;
mod ui;

use std::cell::RefCell;
use std::io::{self, IsTerminal, Write, stdout};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;

// The rules, the words, the saved files and the updater are the library half of this
// crate (`lib.rs`). Imported here so the modules above reach them as `crate::game` etc.
use wordl::update::Program;
use wordl::{game, store, words};

use app::{App, Options};
use game::{Difficulty, Mode};
use store::Stats;

/// Who the updater works for: this binary, not the library, knows its name and version.
const PROGRAM: Program = Program { name: "wordl", version: env!("CARGO_PKG_VERSION"), repo: "anwarahmed/wordl" };
/// The commit this binary was built from, for `--version`; empty if unknown, `-dirty`
/// if the tree had local changes.
const COMMIT: &str = env!("WORDL_COMMIT");

const USAGE: &str = "\
wordl - a Wordle-style word game for the terminal

Usage:
  wordl [options]           play
  wordl update              check for a newer release now and install it
  wordl update off | on     stop, or resume, checking when the game starts
  wordl --help | --version | --licenses

Options:
  -p, --practice            start with a new random word (default)
  -d, --daily               start with today's puzzle
  -t, --theme NAME          midnight, daylight, neon, contrast, ocean, ember, paper, sky, candy
                            or terminal
      --normal              any dictionary word is a valid guess
      --hard                green letters stay fixed, yellow letters must be reused
      --ultra               ultra hard: also, yellow letters must move to another
                            spot and gray letters may not be played again
      --no-animation        skip the tile animations

In the game: type letters, Enter to submit, Backspace to delete.
  ?  help        ^N new word      ^D daily puzzle    ^S statistics
  ^T theme       ^X difficulty    ^G give up         ^Q quit
The on-screen keyboard and the buttons can be clicked with the mouse.

Environment:
  WORDL_NO_UPDATE           set to skip the update check for one run

Statistics are kept in $XDG_STATE_HOME/wordl (~/.local/state/wordl).
";

fn fail(msg: &str) -> ExitCode {
    eprintln!("wordl: {msg}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stats = Stats::load(store::state_dir(PROGRAM.name));
    let mut options = Options {
        mode: Mode::Practice,
        theme: None,
        difficulty: None,
        animate: std::env::var_os("WORDL_NO_ANIM").is_none_or(|v| v.is_empty()),
        truecolor: matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit")),
        debug_answer: std::env::var("WORDL_DEBUG_ANSWER").ok().and_then(|w| game::word(&w)),
    };

    let mut rest = args.iter().map(String::as_str);
    while let Some(arg) = rest.next() {
        match arg {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-v" | "-V" | "--version" => {
                println!("wordl {} ({})", PROGRAM.version, if COMMIT.is_empty() { "unknown commit" } else { COMMIT });
                return ExitCode::SUCCESS;
            }
            "--licenses" => {
                print!("wordl is MIT licensed:\n\n{}\n", include_str!("../LICENSE"));
                print!("Its word lists are derived from SCOWL, which asks for this notice:\n\n{}", words::SCOWL_NOTICE);
                return ExitCode::SUCCESS;
            }
            "update" => {
                return match rest.next() {
                    None => PROGRAM.command().map_or_else(|e| fail(&e), |()| ExitCode::SUCCESS),
                    Some(switch @ ("on" | "off")) => {
                        stats.set("update", (switch == "on") as u8);
                        stats.save();
                        println!("The update check at startup is {switch}.");
                        ExitCode::SUCCESS
                    }
                    Some(_) => fail("'update' takes on, off or nothing"),
                };
            }
            "-p" | "--practice" => options.mode = Mode::Practice,
            "-d" | "--daily" => options.mode = Mode::Daily,
            "-t" | "--theme" => match rest.next() {
                Some(name) if theme::NAMES.contains(&name) => options.theme = Some(name.to_string()),
                name => return fail(&format!("unknown theme '{}' (choose from: {})", name.unwrap_or_default(), theme::NAMES.join(" "))),
            },
            "--normal" => options.difficulty = Some(Difficulty::Normal),
            "--hard" => options.difficulty = Some(Difficulty::Hard),
            "--ultra" => options.difficulty = Some(Difficulty::Ultra),
            "--no-animation" => options.animate = false,
            other => return fail(&format!("unknown option '{other}' (try --help)")),
        }
    }

    if !io::stdin().is_terminal() || !stdout().is_terminal() {
        return fail("needs an interactive terminal.");
    }
    PROGRAM.before_start(stats.auto_update());

    // Installed before ratatui's hook, which restores the terminal and then calls this one.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // ratatui's hook does not know the mouse was captured.
        let _ = execute!(stdout(), DisableMouseCapture);
        default_hook(info)
    }));

    let mut app = App::new(stats, options);
    match run(&mut app) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e.to_string()),
    }
}

/// Collects what ratatui writes during one frame, so `commit` can send it at once and
/// as a synchronized update: a frame is then never seen half drawn. Clones share one
/// buffer: the backend owns one and `run` commits through the other.
#[derive(Clone, Default)]
struct FrameWriter(Rc<RefCell<Frames>>);

#[derive(Default)]
struct Frames {
    frame: Vec<u8>,
    last: Vec<u8>,
}

impl Write for FrameWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().frame.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl FrameWriter {
    /// A frame identical to the one before it changes nothing on screen, so it is not
    /// sent at all: an idle game writes nothing.
    fn commit(&self, out: &mut impl Write) -> io::Result<()> {
        let Frames { frame, last } = &mut *self.0.borrow_mut();
        if frame != last {
            out.write_all(b"\x1b[?2026h")?;
            out.write_all(frame)?;
            out.write_all(b"\x1b[?2026l")?;
            out.flush()?;
        }
        std::mem::swap(frame, last);
        frame.clear();
        Ok(())
    }
}

/// How long the loop sleeps between frames of an animation.
const FRAME: Duration = Duration::from_millis(16);
/// The longest the loop waits for a key before looking at the terminal again.
const IDLE: Duration = Duration::from_millis(250);

fn run(app: &mut App) -> io::Result<()> {
    // Raw mode, alternate screen and the panic hook; drawing goes through `FrameWriter` instead.
    drop(ratatui::init());
    // For clicks on the keyboard and the buttons. While captured, selecting text in the
    // terminal needs shift (option on macOS).
    execute!(stdout(), EnableMouseCapture)?;
    let frames = FrameWriter::default();
    let mut terminal = Terminal::new(CrosstermBackend::new(frames.clone()))?;
    let result = loop {
        app.tick();
        if std::mem::take(&mut app.repaint)
            && let Err(e) = terminal.clear()
        {
            break Err(e);
        }
        // The layout is worked out from the size on every frame, so a resize needs no
        // handling of its own; this copy is for finding what a click landed on.
        if let Ok(size) = terminal.size() {
            app.size = (size.width as i32, size.height as i32);
        }
        if let Err(e) = terminal.draw(|f| ui::draw(f, app)).and_then(|_| frames.commit(&mut stdout().lock())) {
            break Err(e);
        }
        if app.quit {
            break Ok(());
        }
        // While something is moving, keys wait in the queue: nothing typed during an
        // animation is lost, and nothing acts before the player has seen the result.
        if app.animating() {
            std::thread::sleep(FRAME);
            continue;
        }
        let wait = app.next_deadline().map_or(IDLE, |at| at.saturating_duration_since(Instant::now()).min(IDLE));
        match event::poll(wait) {
            Ok(true) => loop {
                match event::read() {
                    Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => app.on_key(key),
                    Ok(Event::Mouse(mouse)) => app.on_mouse(mouse),
                    Ok(_) => {}
                    Err(e) => {
                        let _ = execute!(stdout(), DisableMouseCapture);
                        ratatui::restore();
                        return Err(e);
                    }
                }
                // Drain queued keys before redrawing so fast typing never lags, but stop
                // at a key that starts an animation, to let it play.
                if app.quit || app.animating() || !event::poll(Duration::ZERO).unwrap_or(false) {
                    break;
                }
            },
            Ok(false) => {}
            Err(e) => break Err(e),
        }
    };
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
