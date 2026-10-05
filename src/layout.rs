//! Where everything goes for a given terminal size.
//!
//! Sizes are levels: a level is the height of a tile or key in rows. Levels 1 to 3 are
//! text (a character on a colored cell); 5 and up are pixel art. There is no level 4:
//! it has too few rows for the 5-pixel font and one too many for text.
//!
//! The layout is recomputed from the size every frame, never patched.

use crate::font::font_for;

/// Geometry of a grid of tiles or keys at one level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dims {
    /// Rows of one block.
    pub t: i32,
    /// Columns of one block.
    pub w: i32,
    /// Distance between neighbours, left edge to left edge.
    pub px: i32,
    /// Distance between rows, top to top.
    pub py: i32,
}

impl Dims {
    pub fn of(level: i32) -> Self {
        match level {
            1 => Self { t: 1, w: 3, px: 4, py: 1 },
            2 => Self { t: 1, w: 5, px: 6, py: 2 },
            3 => Self { t: 3, w: 5, px: 7, py: 3 },
            l => {
                // An odd scale makes letters an odd number of pixels wide; one more
                // column lets them sit in the middle.
                let odd = font_for(l).1 % 2;
                Self { t: l, w: 2 * l - 2 + odd, px: 2 * l + odd, py: l }
            }
        }
    }

    /// Width and height of a grid of `cols` by `rows` blocks.
    pub fn grid(self, cols: i32, rows: i32) -> (i32, i32) {
        ((cols - 1) * self.px + self.w, (rows - 1) * self.py + self.t)
    }
}

const LEVELS: [i32; 15] = [16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 3, 2, 1];

/// The smallest terminal the game fits in with the keyboard under the board. A wide
/// terminal can be shorter, with the keyboard beside it.
pub const MIN_COLS: i32 = 39;
pub const MIN_ROWS: i32 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Keyboard under the board.
    Stacked,
    /// Keyboard beside the board: height is the scarce dimension on a wide terminal.
    Side,
}

/// Positions are 0-based cells from the top left of the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    pub kind: Kind,
    pub cols: i32,
    pub rows: i32,
    pub board: Dims,
    pub bx: i32,
    pub by: i32,
    pub keys: Dims,
    pub kx: i32,
    pub ky: i32,
    /// Widths of the two wide keys on the bottom row, left and right.
    pub wide_left: i32,
    pub wide_right: i32,
    /// The title's level (1, 3 or 5) and position.
    pub title_level: i32,
    pub title: Dims,
    pub hx: i32,
    pub hy: i32,
    /// Row of the line under the title; `None` when it shares the title's row.
    pub info_y: Option<i32>,
    pub msg_y: i32,
    /// The region the title, info line and message are centered in.
    pub rx: i32,
    pub rw: i32,
    pub footer_y: i32,
}

/// The smallest and largest keyboard level that suit a board level: keys never
/// outgrow the tiles, and big tiles don't get a tiny keyboard.
fn key_levels(board: i32) -> (i32, i32) {
    let min = if board >= 8 {
        3
    } else if board >= 5 {
        2
    } else {
        1
    };
    let max = match board * 4 / 5 {
        4 => 3,
        m => m,
    };
    (min, max.max(min))
}

fn title_height(level: i32) -> i32 {
    if level == 1 { 1 } else { level + 1 }
}

/// The biggest board that fits, as (board, keyboard, title) levels.
fn fit(kind: Kind, cols: i32, rows: i32) -> Option<(i32, i32, i32)> {
    for l in LEVELS {
        let (bw, bh) = Dims::of(l).grid(5, 6);
        if bw > cols || (kind == Kind::Side && bh > rows - 1) {
            continue;
        }
        let (kmin, kmax) = key_levels(l);
        for k in (kmin..=kmax).rev().filter(|&k| k != 4) {
            let (kw, kh) = Dims::of(k).grid(10, 3);
            for hv in [5, 3, 1] {
                if hv > 1 && hv > l {
                    continue;
                }
                let (tw, _) = Dims::of(hv).grid(5, 1);
                let hh = title_height(hv);
                let fits = match kind {
                    Kind::Stacked => kw <= cols && tw <= cols && hh + bh + 1 + kh < rows,
                    Kind::Side => bw + 4 + kw <= cols && bw + 4 + tw <= cols && hh + 1 + kh < rows,
                };
                if fits {
                    return Some((l, k, hv));
                }
            }
        }
    }
    None
}

/// Lays the game out for a terminal of `cols` by `rows`, or `None` when it is too
/// small. Whichever of the two arrangements allows bigger tiles wins.
pub fn layout(cols: i32, rows: i32) -> Option<Layout> {
    let stacked = fit(Kind::Stacked, cols, rows);
    let side = fit(Kind::Side, cols, rows);
    let (kind, (l, k, hv)) = match (stacked, side) {
        (Some(s), Some(p)) if s.0 > p.0 || (s.0 == p.0 && s.1 >= p.1) => (Kind::Stacked, s),
        (_, Some(p)) => (Kind::Side, p),
        (Some(s), None) => (Kind::Stacked, s),
        (None, None) => return None,
    };

    let (board, keys, title) = (Dims::of(l), Dims::of(k), Dims::of(hv));
    let (bw, bh) = board.grid(5, 6);
    let (kw, kh) = keys.grid(10, 3);
    let (tw, _) = title.grid(5, 1);
    let hh = title_height(hv);
    // The bottom row is a wide key, seven letters and a wide key; the two wide keys
    // share what ten keys' width leaves.
    let wide = 3 * keys.w + keys.px - keys.w;
    let (wide_left, wide_right) = ((wide + 1) / 2, wide / 2);

    // Worked out in 1-based rows and columns, as the terminal counts them.
    let (bx, by, rx, rw, hy, msg_y, ky);
    match kind {
        Kind::Stacked => {
            let mut spare = rows - (hh + bh + 1 + kh + 1);
            let gap = |spare: &mut i32| {
                let g = (*spare > 0) as i32;
                *spare -= g;
                g
            };
            let after_title = gap(&mut spare);
            let before_keys = gap(&mut spare);
            let after_board = gap(&mut spare);
            (rx, rw) = (1, cols);
            hy = 1 + spare / 2;
            by = hy + hh + after_title;
            msg_y = by + bh + after_board;
            ky = msg_y + 1 + before_keys;
            bx = (cols - bw) / 2 + 1;
        }
        Kind::Side => {
            let pw = kw.max(tw);
            let gap = ((cols - bw - pw) / 3).clamp(4, 16);
            bx = (cols - bw - gap - pw) / 2 + 1;
            by = 1 + (rows - 1 - bh) / 2;
            (rx, rw) = (bx + bw + gap, pw);
            let mut spare = rows - 1 - (hh + 1 + kh);
            let (mut after_title, mut before_keys) = (0, 0);
            if spare > 0 {
                after_title = 1;
                spare -= 1;
            }
            if spare > 0 {
                before_keys = 1;
                spare -= 1;
            }
            if spare > 5 {
                (after_title, before_keys) = (2, 2);
                spare -= 2;
            }
            hy = 1 + spare / 2;
            msg_y = hy + hh + after_title;
            ky = msg_y + 1 + before_keys;
        }
    }
    Some(Layout {
        kind,
        cols,
        rows,
        board,
        bx: bx - 1,
        by: by - 1,
        keys,
        kx: rx + (rw - kw) / 2 - 1,
        ky: ky - 1,
        wide_left,
        wide_right,
        title_level: hv,
        title,
        hx: rx + (rw - tw) / 2 - 1,
        hy: hy - 1,
        info_y: (hv > 1).then_some(hy + hv - 1),
        msg_y: msg_y - 1,
        rx: rx - 1,
        rw,
        footer_y: rows - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_sizes() {
        let l = layout(80, 24).unwrap();
        assert_eq!((l.kind, l.board.t, l.keys.t, l.title_level), (Kind::Stacked, 3, 1, 1));
        assert_eq!((l.bx, l.by, l.kx, l.ky, l.msg_y, l.footer_y), (23, 1, 20, 20, 19, 23));
        // A wide terminal puts the keyboard beside the board and gets bigger tiles.
        let l = layout(190, 50).unwrap();
        assert_eq!((l.kind, l.board.t, l.keys.t), (Kind::Side, 8, 5));
        assert_eq!(layout(39, 12).unwrap().board.w, 3);
        assert_eq!(layout(38, 12), None);
        assert_eq!(layout(39, 11), None);
        assert_eq!(layout(0, 0), None);
    }

    /// Every size from tiny to huge: the game must fit whenever the terminal is at
    /// least 39x12, and nothing may be off screen or on top of something else.
    #[test]
    fn nothing_overlaps_or_leaves_the_screen_at_any_size() {
        for rows in 0..=100 {
            for cols in 0..=420 {
                let Some(l) = layout(cols, rows) else {
                    assert!(cols < MIN_COLS || rows < MIN_ROWS, "{cols}x{rows} does not fit");
                    continue;
                };
                let at = format!("{cols}x{rows} {:?}", l.kind);
                let (bw, bh) = l.board.grid(5, 6);
                let (kw, kh) = l.keys.grid(10, 3);
                let (tw, _) = l.title.grid(5, 1);
                let hh = title_height(l.title_level);
                assert!(l.bx >= 0 && l.bx + bw <= cols, "{at}: board sideways");
                assert!(l.by >= 0 && l.by + bh < rows, "{at}: board over the footer");
                assert!(l.kx >= 0 && l.kx + kw <= cols, "{at}: keyboard sideways");
                assert!(l.ky + kh < rows, "{at}: keyboard over the footer");
                assert!(l.hy >= 0 && l.hx >= 0 && l.hx + tw <= cols, "{at}: title");
                assert!(l.msg_y >= l.hy + hh && l.ky > l.msg_y, "{at}: title, message, keyboard");
                assert_eq!(l.wide_left + l.wide_right + 7 * l.keys.w + 8 * (l.keys.px - l.keys.w), kw, "{at}: bottom row");
                match l.kind {
                    Kind::Stacked => assert!(l.by >= l.hy + hh && l.msg_y >= l.by + bh, "{at}: board, title, message"),
                    Kind::Side => assert!(l.rx > l.bx + bw && l.hx >= l.rx && l.kx >= l.rx, "{at}: board and side panel"),
                }
            }
        }
    }
}
