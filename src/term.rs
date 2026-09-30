//! A cell buffer that only redraws what changed since the last frame, and a
//! braille canvas that draws at 2x4 dots per cell on top of it.

use std::fmt::Write as _;
use std::io::{self, Write};

use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const BLACK: Rgb = Rgb(0, 0, 0);

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let hex = hex.strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Rgb(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Blend toward `other` by `amount` (0 keeps self, 1 gives other).
    pub fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let t = amount.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(lerp(self.0, other.0), lerp(self.1, other.1), lerp(self.2, other.2))
    }

    /// Darken toward black by `amount`.
    pub fn dim(self, amount: f32) -> Rgb {
        self.mix(BLACK, amount)
    }

    fn luma(self) -> u32 {
        self.0 as u32 * 3 + self.1 as u32 * 6 + self.2 as u32
    }
}

/// Continuation marker for the right half of a wide (CJK) character.
const WIDE_TAIL: char = '\0';

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Rgb,
    bg: Rgb,
}

const BLANK: Cell = Cell { ch: ' ', fg: BLACK, bg: BLACK };

pub fn char_width(c: char) -> i32 {
    c.width().unwrap_or(0).max(1) as i32
}

pub fn text_width(text: &str) -> i32 {
    text.chars().map(char_width).sum()
}

pub struct Screen {
    pub w: i32,
    pub h: i32,
    cells: Vec<Cell>,
    prev: Option<Vec<Cell>>,
    out: String,
}

impl Screen {
    pub fn new(w: u16, h: u16) -> Screen {
        let mut screen = Screen { w: 0, h: 0, cells: Vec::new(), prev: None, out: String::new() };
        screen.resize(w, h);
        screen
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.w = w as i32;
        self.h = h as i32;
        self.cells = vec![BLANK; (self.w * self.h) as usize];
        self.prev = None;
    }

    pub fn clear(&mut self) {
        self.cells.fill(BLANK);
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    /// Write text at x, y and return the column after it. Wide characters
    /// take two cells; anything off screen is clipped.
    pub fn put(&mut self, mut x: i32, y: i32, text: &str, fg: Rgb, bg: Rgb) -> i32 {
        if y < 0 || y >= self.h {
            return x + text_width(text);
        }
        for c in text.chars() {
            let cw = char_width(c);
            if x >= 0 && x + cw <= self.w {
                let i = self.index(x, y);
                // Don't leave half of a wide character behind
                if self.cells[i].ch == WIDE_TAIL && x > 0 {
                    self.cells[i - 1].ch = ' ';
                }
                let end = i + cw as usize;
                if x + cw < self.w && self.cells[end].ch == WIDE_TAIL {
                    self.cells[end].ch = ' ';
                }
                self.cells[i] = Cell { ch: c, fg, bg };
                if cw == 2 {
                    self.cells[i + 1] = Cell { ch: WIDE_TAIL, fg, bg };
                }
            }
            x += cw;
        }
        x
    }

    pub fn put_char(&mut self, x: i32, y: i32, c: char, fg: Rgb, bg: Rgb) {
        let mut buf = [0u8; 4];
        self.put(x, y, c.encode_utf8(&mut buf), fg, bg);
    }

    pub fn fill_row(&mut self, y: i32, bg: Rgb) {
        self.fill(0, y, self.w, 1, bg);
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, bg: Rgb) {
        for row in y.max(0)..(y + h).min(self.h) {
            for col in x.max(0)..(x + w).min(self.w) {
                let i = self.index(col, row);
                self.cells[i] = Cell { ch: ' ', fg: BLACK, bg };
            }
        }
    }

    pub fn center(&mut self, y: i32, text: &str, fg: Rgb, bg: Rgb) {
        let x = (self.w - text_width(text)) / 2;
        self.put(x, y, text, fg, bg);
    }

    /// A double-line box. Only the inside gets the fill color, since a
    /// border cell's background shows around the line.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb, fill: Rgb) {
        let rule = "═".repeat((w - 2).max(0) as usize);
        self.put(x, y, &format!("╔{rule}╗"), color, BLACK);
        for row in y + 1..y + h - 1 {
            self.put(x, row, "║", color, BLACK);
            self.fill(x + 1, row, w - 2, 1, fill);
            self.put(x + w - 1, row, "║", color, BLACK);
        }
        self.put(x, y + h - 1, &format!("╚{rule}╝"), color, BLACK);
    }

    /// Only the corners of a box, like a targeting reticle.
    pub fn corners(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
        let right = x + w - 1;
        let bottom = y + h - 1;
        self.put(x, y, "┏━", color, BLACK);
        self.put(right - 1, y, "━┓", color, BLACK);
        self.put(x, bottom, "┗━", color, BLACK);
        self.put(right - 1, bottom, "━┛", color, BLACK);
    }

    pub fn bg_at(&self, x: i32, y: i32) -> Rgb {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return BLACK;
        }
        self.cells[self.index(x, y)].bg
    }

    /// Write the changed cells to the terminal.
    pub fn flush(&mut self, out: &mut impl Write) -> io::Result<()> {
        self.out.clear();
        let mut cur_fg = None;
        let mut cur_bg = None;
        let mut next_i = usize::MAX;
        for (i, cell) in self.cells.iter().enumerate() {
            if cell.ch == WIDE_TAIL {
                continue;
            }
            if let Some(prev) = &self.prev {
                // A wide character also changes if its right half does
                let wide = char_width(cell.ch) == 2;
                if prev[i] == *cell && (!wide || prev[i + 1] == self.cells[i + 1]) {
                    continue;
                }
            }
            let (x, y) = (i as i32 % self.w, i as i32 / self.w);
            if i != next_i || x == 0 {
                let _ = write!(self.out, "\x1b[{};{}H", y + 1, x + 1);
            }
            if cur_fg != Some(cell.fg) {
                let Rgb(r, g, b) = cell.fg;
                let _ = write!(self.out, "\x1b[38;2;{r};{g};{b}m");
                cur_fg = Some(cell.fg);
            }
            if cur_bg != Some(cell.bg) {
                let Rgb(r, g, b) = cell.bg;
                let _ = write!(self.out, "\x1b[48;2;{r};{g};{b}m");
                cur_bg = Some(cell.bg);
            }
            self.out.push(cell.ch);
            next_i = i + char_width(cell.ch) as usize;
        }
        self.prev = Some(self.cells.clone());
        if !self.out.is_empty() {
            out.write_all(self.out.as_bytes())?;
            out.flush()?;
        }
        Ok(())
    }
}

/// Draws in braille dots, 2 wide by 4 tall per cell, over a region of the
/// screen. Each cell takes the brightest color plotted into it.
pub struct Canvas {
    pub cols: i32,
    pub rows: i32,
    dots: Vec<u8>,
    colors: Vec<Rgb>,
}

impl Canvas {
    pub fn new(cols: i32, rows: i32) -> Canvas {
        let n = (cols.max(0) * rows.max(0)) as usize;
        Canvas { cols, rows, dots: vec![0; n], colors: vec![BLACK; n] }
    }

    pub fn width(&self) -> i32 {
        self.cols * 2
    }

    pub fn height(&self) -> i32 {
        self.rows * 4
    }

    pub fn plot(&mut self, px: i32, py: i32, color: Rgb) {
        if px < 0 || py < 0 || px >= self.width() || py >= self.height() {
            return;
        }
        const BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
        let i = ((py / 4) * self.cols + px / 2) as usize;
        self.dots[i] |= BITS[(px % 2) as usize][(py % 4) as usize];
        if color.luma() >= self.colors[i].luma() {
            self.colors[i] = color;
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            self.plot((x0 + (x1 - x0) * t).round() as i32, (y0 + (y1 - y0) * t).round() as i32, color);
        }
    }

    /// Draw the canvas onto the screen with its top left at x, y, keeping the
    /// screen's background under each dot.
    pub fn blit(&self, screen: &mut Screen, x: i32, y: i32) {
        for row in 0..self.rows {
            for col in 0..self.cols {
                let i = (row * self.cols + col) as usize;
                if self.dots[i] != 0 {
                    let c = char::from_u32(0x2800 + self.dots[i] as u32).unwrap_or(' ');
                    let bg = screen.bg_at(x + col, y + row);
                    screen.put_char(x + col, y + row, c, self.colors[i], bg);
                }
            }
        }
    }
}
