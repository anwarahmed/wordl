//! Letters as bitmaps, turned into rows of half-block characters.
//!
//! A terminal cell is about twice as tall as it is wide, so `▀` and `▄` give two
//! square pixels per cell. A letter is a 5x5 or 5x7 bitmap scaled by a whole number;
//! two fonts give more usable sizes than one.

/// Rows of a glyph, top to bottom, separated by spaces; `#` is an inked pixel.
const FONT5: [&str; 26] = [
    ".###. #...# ##### #...# #...#",
    "####. #...# ####. #...# ####.",
    ".#### #.... #.... #.... .####",
    "####. #...# #...# #...# ####.",
    "##### #.... ####. #.... #####",
    "##### #.... ####. #.... #....",
    ".#### #.... #..## #...# .####",
    "#...# #...# ##### #...# #...#",
    "##### ..#.. ..#.. ..#.. #####",
    "..### ...#. ...#. #..#. .##..",
    "#...# #..#. ###.. #..#. #...#",
    "#.... #.... #.... #.... #####",
    "#...# ##.## #.#.# #...# #...#",
    "#...# ##..# #.#.# #..## #...#",
    ".###. #...# #...# #...# .###.",
    "####. #...# ####. #.... #....",
    ".###. #...# #.#.# #..#. .##.#",
    "####. #...# ####. #..#. #...#",
    ".#### #.... .###. ....# ####.",
    "##### ..#.. ..#.. ..#.. ..#..",
    "#...# #...# #...# #...# .###.",
    "#...# #...# #...# .#.#. ..#..",
    "#...# #...# #.#.# ##.## #...#",
    "#...# .#.#. ..#.. .#.#. #...#",
    "#...# .#.#. ..#.. ..#.. ..#..",
    "##### ...#. ..#.. .#... #####",
];

const FONT7: [&str; 26] = [
    ".###. #...# #...# ##### #...# #...# #...#",
    "####. #...# #...# ####. #...# #...# ####.",
    ".###. #...# #.... #.... #.... #...# .###.",
    "####. #...# #...# #...# #...# #...# ####.",
    "##### #.... #.... ####. #.... #.... #####",
    "##### #.... #.... ####. #.... #.... #....",
    ".###. #...# #.... #.### #...# #...# .###.",
    "#...# #...# #...# ##### #...# #...# #...#",
    "##### ..#.. ..#.. ..#.. ..#.. ..#.. #####",
    "..### ...#. ...#. ...#. ...#. #..#. .##..",
    "#...# #..#. #.#.. ##... #.#.. #..#. #...#",
    "#.... #.... #.... #.... #.... #.... #####",
    "#...# ##.## #.#.# #.#.# #...# #...# #...#",
    "#...# ##..# ##..# #.#.# #..## #..## #...#",
    ".###. #...# #...# #...# #...# #...# .###.",
    "####. #...# #...# ####. #.... #.... #....",
    ".###. #...# #...# #...# #.#.# #..#. .##.#",
    "####. #...# #...# ####. #.#.. #..#. #...#",
    ".#### #.... #.... .###. ....# ....# ####.",
    "##### ..#.. ..#.. ..#.. ..#.. ..#.. ..#..",
    "#...# #...# #...# #...# #...# #...# .###.",
    "#...# #...# #...# #...# #...# .#.#. ..#..",
    "#...# #...# #...# #.#.# #.#.# ##.## #...#",
    "#...# #...# .#.#. ..#.. .#.#. #...# #...#",
    "#...# #...# .#.#. ..#.. ..#.. ..#.. ..#..",
    "##### ....# ...#. ..#.. .#... #.... #####",
];

const ICON_ENTER: &str = ".....## .....## ..#..## .###### ..#....";
const ICON_BACK: &str = "..#.... .#..... ####### .#..... ..#....";

/// What is drawn on a tile or key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Label {
    None,
    /// An uppercase letter.
    Letter(u8),
    Enter,
    Back,
}

impl Label {
    /// The label as plain text, for blocks too small for bitmaps. `width` is the
    /// block's width in cells.
    pub fn text(self, width: i32) -> String {
        match self {
            Label::None => String::new(),
            Label::Letter(b) => (b as char).to_string(),
            Label::Enter if width >= 7 => "ENTER".into(),
            Label::Enter => "↵".into(),
            Label::Back => "⌫".into(),
        }
    }
}

/// The font height (5 or 7 pixels) and whole-number scale that fill a block of `t`
/// rows best. A block has `2t - 4` pixel rows for its glyph.
pub fn font_for(t: i32) -> (i32, i32) {
    let a = 2 * t - 4;
    let limit = a - a / 8;
    let (mut font, mut scale, mut best) = (5, 1, 0);
    for f in [7, 5] {
        let s = limit / f;
        if s >= 1 && f * s > best {
            (font, scale, best) = (f, s, f * s);
        }
    }
    (font, scale)
}

/// Renders a label for a block `w` cells wide and `t` rows tall (`t >= 5`): one string
/// per inner row (`t - 2` of them), each `w` characters of space, `▀`, `▄` or `█`.
/// Colorless; the caller sets the colors around it.
pub fn glyph(label: Label, w: i32, t: i32) -> Vec<String> {
    let a = (2 * t - 4).max(0);
    let blank = || vec![" ".repeat(w.max(0) as usize); (t - 2).max(0) as usize];
    let (bitmap, bw, f, s) = match label {
        Label::None => return blank(),
        Label::Letter(b) if b.is_ascii_uppercase() => {
            let (f, s) = font_for(t);
            let font = if f == 7 { &FONT7 } else { &FONT5 };
            (font[(b - b'A') as usize], 5, f, s)
        }
        Label::Letter(_) => return blank(),
        Label::Enter | Label::Back => {
            let s = ((a - a / 8) / 5).min((w - 2) / 7).max(1);
            (if label == Label::Enter { ICON_ENTER } else { ICON_BACK }, 7, 5, s)
        }
    };
    let (gw, gh) = (bw * s, f * s);
    let (xo, yo) = (((w - gw) / 2).max(0), ((a - gh) / 2).max(0));
    // Pixel rows, each `w` wide; true is inked.
    let mut px = vec![vec![false; w.max(0) as usize]; a as usize];
    for (r, line) in bitmap.split(' ').enumerate() {
        for (c, ch) in line.bytes().enumerate() {
            if ch != b'#' {
                continue;
            }
            for dy in 0..s {
                for dx in 0..s {
                    let (y, x) = (yo + r as i32 * s + dy, xo + c as i32 * s + dx);
                    if y < a && x < w {
                        px[y as usize][x as usize] = true;
                    }
                }
            }
        }
    }
    px.chunks(2)
        .map(|pair| {
            (0..w.max(0) as usize)
                .map(|x| match (pair[0][x], pair.get(1).is_some_and(|row| row[x])) {
                    (false, false) => ' ',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (true, true) => '█',
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_are_complete_and_rectangular() {
        for (font, height) in [(&FONT5, 5), (&FONT7, 7)] {
            for glyph in font {
                let rows: Vec<&str> = glyph.split(' ').collect();
                assert_eq!(rows.len(), height, "{glyph}");
                assert!(rows.iter().all(|r| r.len() == 5 && r.bytes().all(|b| b == b'#' || b == b'.')), "{glyph}");
            }
        }
        for icon in [ICON_ENTER, ICON_BACK] {
            assert!(icon.split(' ').count() == 5 && icon.split(' ').all(|r| r.len() == 7));
        }
    }

    #[test]
    fn picks_the_font_that_fills_the_block() {
        assert_eq!(font_for(5), (5, 1));
        assert_eq!(font_for(6), (7, 1));
        assert_eq!(font_for(8), (5, 2));
        assert_eq!(font_for(10), (7, 2));
        assert_eq!(font_for(14), (7, 3));
    }

    #[test]
    fn glyphs_fit_their_block_at_every_size() {
        for t in 5..=16 {
            let (_, s) = font_for(t);
            let w = 2 * t - 2 + s % 2;
            for label in [Label::Letter(b'W'), Label::Letter(b'Q'), Label::Enter, Label::Back, Label::None] {
                let rows = glyph(label, if label == Label::Enter { w * 3 / 2 } else { w }, t);
                assert_eq!(rows.len() as i32, t - 2);
                let width = rows[0].chars().count();
                assert!(rows.iter().all(|r| r.chars().count() == width));
                // A margin on both sides, so an outlined tile's frame never cuts a letter.
                assert!(rows.iter().all(|r| r.starts_with(' ') && r.ends_with(' ')), "{label:?} at {t}");
            }
        }
    }

    #[test]
    fn a_letter_is_drawn_as_half_blocks() {
        // 5x5 font, 9 wide and 5 tall: three inner rows, the L's foot in the top half of the last.
        let rows = glyph(Label::Letter(b'L'), 9, 5);
        assert_eq!(rows, ["  █      ", "  █      ", "  ▀▀▀▀▀  "]);
    }
}
