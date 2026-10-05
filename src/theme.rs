//! Color themes. Code asks for roles ("the green tile", "an unused key"), never for
//! colors; only this file knows RGB values.
//!
//! "terminal" uses only the terminal's own 16-color palette and no background, so it
//! follows whatever theme the terminal has. The others are truecolor, mapped to the
//! nearest of 256 colors on terminals that do not announce truecolor.

use ratatui::style::Color;

/// One role's color, as a foreground and as a background. They are the same color in
/// the truecolor themes; in "terminal" they can differ (an absent key is gray text on
/// the terminal's own background).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paint {
    pub fg: Color,
    pub bg: Color,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    /// The screen.
    pub bg: Paint,
    /// Ordinary text.
    pub fg: Paint,
    /// Hints and labels.
    pub dim: Paint,
    /// The frame of an empty tile.
    pub empty: Paint,
    /// The frame of a tile with a typed letter.
    pub typed: Paint,
    /// Right letter, right spot.
    pub g: Paint,
    /// Right letter, wrong spot.
    pub y: Paint,
    /// Letter not in the word.
    pub x: Paint,
    /// The flash that runs along a winning row.
    pub win: Paint,
    /// Letters on green, yellow and gray tiles.
    pub gfg: Paint,
    pub yfg: Paint,
    pub xfg: Paint,
    /// A key not tried yet, and its letter.
    pub key: Paint,
    pub keyfg: Paint,
    /// A key known not to be in the word, and its letter.
    pub keyx: Paint,
    pub keyxfg: Paint,
    /// Key names, dialog titles.
    pub accent: Paint,
    /// Dialog background and border.
    pub panel: Paint,
    pub panelb: Paint,
    /// The message chip and its text.
    pub toast: Paint,
    pub toastfg: Paint,
    /// Dialog buttons and their text.
    pub btn: Paint,
    pub btnfg: Paint,
}

pub const NAMES: [&str; 5] = ["midnight", "daylight", "neon", "contrast", "terminal"];

/// The nearest of the 6x6x6 color cube and the 24-step gray ramp.
fn nearest_256(r: u8, g: u8, b: u8) -> Color {
    let rgb = [r as i32, g as i32, b as i32];
    let cube_index = |v: i32| {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (v - 35) / 40
        }
    };
    let cube_value = |i: i32| if i == 0 { 0 } else { 55 + 40 * i };
    let ci = rgb.map(cube_index);
    let gray = (rgb[0] + rgb[1] + rgb[2]) / 3;
    let gi = if gray < 8 {
        0
    } else if gray > 238 {
        23
    } else {
        (gray - 3) / 10
    };
    let gv = 8 + 10 * gi;
    let cube_distance: i32 = (0..3).map(|i| (rgb[i] - cube_value(ci[i])).pow(2)).sum();
    let gray_distance: i32 = rgb.iter().map(|v| (v - gv).pow(2)).sum();
    Color::Indexed(if gray_distance < cube_distance { 232 + gi } else { 16 + 36 * ci[0] + 6 * ci[1] + ci[2] } as u8)
}

fn from_rgb(name: &'static str, truecolor: bool, c: [(u8, u8, u8); 23]) -> Theme {
    let p = |i: usize| {
        let (r, g, b) = c[i];
        let color = if truecolor { Color::Rgb(r, g, b) } else { nearest_256(r, g, b) };
        Paint { fg: color, bg: color }
    };
    Theme {
        name,
        bg: p(0),
        fg: p(1),
        dim: p(2),
        empty: p(3),
        typed: p(4),
        g: p(5),
        y: p(6),
        x: p(7),
        win: p(8),
        gfg: p(9),
        yfg: p(10),
        xfg: p(11),
        key: p(12),
        keyfg: p(13),
        keyx: p(14),
        keyxfg: p(15),
        accent: p(16),
        panel: p(17),
        panelb: p(18),
        toast: p(19),
        toastfg: p(20),
        btn: p(21),
        btnfg: p(22),
    }
}

/// The terminal's own colors. `Reset` is its default foreground or background.
fn terminal() -> Theme {
    let same = |c: Color| Paint { fg: c, bg: c };
    let default = same(Color::Reset);
    Theme {
        name: "terminal",
        bg: Paint { fg: Color::Black, bg: Color::Reset },
        fg: default,
        dim: same(Color::DarkGray),
        empty: same(Color::DarkGray),
        typed: same(Color::Gray),
        g: same(Color::Green),
        y: same(Color::Yellow),
        x: same(Color::DarkGray),
        win: same(Color::LightGreen),
        gfg: same(Color::Black),
        yfg: same(Color::Black),
        xfg: same(Color::White),
        key: same(Color::Gray),
        keyfg: same(Color::Black),
        keyx: Paint { fg: Color::DarkGray, bg: Color::Reset },
        keyxfg: same(Color::DarkGray),
        accent: same(Color::Cyan),
        panel: default,
        panelb: same(Color::Cyan),
        toast: same(Color::Gray),
        toastfg: same(Color::Black),
        btn: same(Color::Cyan),
        btnfg: same(Color::Black),
    }
}

/// The theme of that name; an unknown name gives the default, "midnight".
#[rustfmt::skip]
pub fn theme(name: &str, truecolor: bool) -> Theme {
    match name {
        "daylight" => from_rgb("daylight", truecolor, [
        (248, 248, 244), // bg
        (28, 31, 40), // fg
        (128, 134, 148), // dim
        (205, 210, 221), // empty
        (112, 119, 136), // typed
        (58, 158, 84), // g
        (219, 166, 35), // y
        (118, 125, 141), // x
        (104, 204, 130), // win
        (255, 255, 255), // gfg
        (255, 255, 255), // yfg
        (255, 255, 255), // xfg
        (212, 217, 228), // key
        (28, 31, 40), // keyfg
        (118, 125, 141), // keyx
        (240, 241, 245), // keyxfg
        (37, 99, 220), // accent
        (255, 255, 255), // panel
        (37, 99, 220), // panelb
        (28, 31, 40), // toast
        (248, 248, 244), // toastfg
        (37, 99, 220), // btn
        (255, 255, 255), // btnfg
        ]),
        "neon" => from_rgb("neon", truecolor, [
        (14, 9, 30), // bg
        (240, 234, 255), // fg
        (138, 120, 184), // dim
        (62, 46, 106), // empty
        (158, 128, 230), // typed
        (0, 200, 120), // g
        (255, 190, 0), // y
        (58, 44, 96), // x
        (120, 255, 190), // win
        (6, 30, 20), // gfg
        (40, 26, 0), // yfg
        (240, 234, 255), // xfg
        (98, 74, 156), // key
        (255, 255, 255), // keyfg
        (30, 22, 54), // keyx
        (98, 82, 140), // keyxfg
        (255, 92, 205), // accent
        (26, 18, 52), // panel
        (255, 92, 205), // panelb
        (255, 92, 205), // toast
        (14, 9, 30), // toastfg
        (255, 92, 205), // btn
        (14, 9, 30), // btnfg
        ]),
        // Orange and blue instead of green and yellow, for color-blind players.
        "contrast" => from_rgb("contrast", truecolor, [
        (17, 19, 26), // bg
        (236, 238, 244), // fg
        (125, 131, 150), // dim
        (52, 57, 74), // empty
        (122, 130, 156), // typed
        (245, 121, 58), // g
        (133, 192, 249), // y
        (62, 67, 84), // x
        (255, 170, 120), // win
        (20, 12, 6), // gfg
        (8, 20, 34), // yfg
        (255, 255, 255), // xfg
        (86, 93, 117), // key
        (255, 255, 255), // keyfg
        (32, 35, 46), // keyx
        (96, 102, 122), // keyxfg
        (133, 192, 249), // accent
        (27, 30, 41), // panel
        (133, 192, 249), // panelb
        (236, 238, 244), // toast
        (17, 19, 26), // toastfg
        (133, 192, 249), // btn
        (8, 20, 34), // btnfg
        ]),
        "terminal" => terminal(),
        _ => from_rgb("midnight", truecolor, [
        (17, 19, 26), // bg
        (236, 238, 244), // fg
        (125, 131, 150), // dim
        (52, 57, 74), // empty
        (122, 130, 156), // typed
        (40, 167, 88), // g
        (222, 168, 36), // y
        (62, 67, 84), // x
        (110, 226, 150), // win
        (255, 255, 255), // gfg
        (255, 255, 255), // yfg
        (255, 255, 255), // xfg
        (86, 93, 117), // key
        (255, 255, 255), // keyfg
        (32, 35, 46), // keyx
        (96, 102, 122), // keyxfg
        (108, 168, 255), // accent
        (27, 30, 41), // panel
        (108, 168, 255), // panelb
        (236, 238, 244), // toast
        (17, 19, 26), // toastfg
        (108, 168, 255), // btn
        (12, 18, 32), // btnfg
        ]),
    }
}

/// The theme after this one in `NAMES`, wrapping around.
pub fn next_name(name: &str) -> &'static str {
    let at = NAMES.iter().position(|n| *n == name).unwrap_or(0);
    NAMES[(at + 1) % NAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_has_a_theme_and_the_cycle_visits_all() {
        let mut name = NAMES[0];
        for expected in NAMES {
            assert_eq!(name, expected);
            assert_eq!(theme(name, true).name, name);
            name = next_name(name);
        }
        assert_eq!(name, NAMES[0]);
        assert_eq!(theme("no such theme", true).name, "midnight");
    }

    #[test]
    fn falls_back_to_256_colors() {
        // A dark blue-gray background must not turn into the cube's dark blue.
        assert_eq!(nearest_256(17, 19, 26), Color::Indexed(233));
        assert_eq!(nearest_256(255, 255, 255), Color::Indexed(231));
        assert_eq!(nearest_256(0, 0, 0), Color::Indexed(16));
        assert_eq!(nearest_256(40, 167, 88), Color::Indexed(35));
        assert!(matches!(theme("midnight", false).bg.bg, Color::Indexed(_)));
        assert!(matches!(theme("midnight", true).bg.bg, Color::Rgb(17, 19, 26)));
    }
}
