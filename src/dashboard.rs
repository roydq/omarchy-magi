//! The standby dashboard: the MAGI deliberating, sync waveforms and the log.

use crate::app::{App, TOPICS, UNITS};
use crate::sys::{self, Rng};
use crate::term::{BLACK, Canvas, Rgb, Screen, text_width};

/// Characters per second the newest log line types out at.
const LOG_TYPE_SPEED: f32 = 40.0;

impl App {
    pub fn draw_dashboard(&mut self, screen: &mut Screen, now: f32) {
        let box_h = if screen.h >= 34 { 7 } else { 5 };
        let magi_rows = 2 * box_h + 4;
        let panel_rows = (screen.h - 2 - magi_rows - 2).max(0);

        let hex = self.theme.muted.dim(0.55);
        self.draw_hex(screen, now, hex, 2, 2 + magi_rows);
        self.draw_header(screen);
        self.draw_magi(screen, now, 2, box_h);
        if panel_rows >= 1 {
            // The panels' titles sit in the divider line above them
            let rule_y = 2 + magi_rows;
            let rule = "─".repeat(screen.w.max(0) as usize);
            screen.put(0, rule_y, &rule, self.theme.accent.dim(0.6), BLACK);
            self.draw_waveforms(screen, now, rule_y, screen.h - 1);
            self.draw_log(screen, now, rule_y, screen.h - 1);
        }
        self.draw_footer(screen, now);
    }

    pub fn draw_header(&self, screen: &mut Screen) {
        let theme = &self.theme;
        screen.fill_row(0, BLACK);
        let x = screen.put(1, 0, "▌NERV ", theme.accent, BLACK);
        let x = screen.put(x, 0, "特務機関ネルフ", theme.fg, BLACK);
        if screen.w >= 70 {
            screen.put(x + 2, 0, "· MAGI SYSTEM", theme.dim, BLACK);
        }
        let clock = sys::clock("%Y.%m.%d %a %H:%M:%S").to_uppercase();
        screen.put(screen.w - text_width(&clock) - 1, 0, &clock, theme.fg, BLACK);
        let rule = "━".repeat(screen.w.max(0) as usize);
        screen.put(0, 1, &rule, theme.accent.dim(0.6), BLACK);
    }

    fn draw_magi(&mut self, screen: &mut Screen, now: f32, top: i32, box_h: i32) {
        let theme = &self.theme;
        let box_w = (screen.w / 4).clamp(20, 34);
        let cx = screen.w / 2;

        screen.fill_row(top, BLACK);
        let (jp, en, _) = TOPICS[self.magi.topic];
        screen.center(top, &format!("提訴 #{:04} · {jp} · {en}", self.magi.number), theme.fg, BLACK);

        let top_y = top + 2;
        let side_y = top_y + box_h + 1;
        let gap = (box_w / 3).max(7);
        let boxes = [(cx - box_w / 2, top_y), (cx - gap - box_w, side_y), (cx + gap, side_y)];

        // Wiring to the MAGI label in the middle
        let label_y = side_y + box_h / 2;
        for y in top_y + box_h..label_y {
            screen.put(cx, y, "┃", theme.accent, BLACK);
        }
        screen.put(cx - gap, label_y, &"━".repeat((gap - 3).max(0) as usize), theme.accent, BLACK);
        screen.put(cx + 4, label_y, &"━".repeat((gap - 4).max(0) as usize), theme.accent, BLACK);
        screen.put(cx - 2, label_y, "MAGI", theme.accent, BLACK);

        for (unit, &(x, y)) in boxes.iter().enumerate() {
            let (color, label, fill) = match self.magi.state(unit, now) {
                None => {
                    let color = if App::blink(now, 0.5) { theme.yellow } else { theme.dim };
                    (color, "審議中", BLACK)
                }
                Some(vote) => {
                    let color = if vote { theme.green } else { theme.red };
                    // Flash the fill as the vote lands, then settle
                    let since = self.magi.since_vote(unit, now).unwrap_or(1.0);
                    let flash = (1.0 - since / 0.4).max(0.0);
                    let fill = color.dim(0.88 - flash * 0.5);
                    (color, if vote { "承認" } else { "否決" }, fill)
                }
            };
            screen.frame(x, y, box_w, box_h, color, fill);
            let name = UNITS[unit];
            screen.put(x + (box_w - text_width(name)) / 2, y + 1, name, color, fill);
            screen.put(x + (box_w - text_width(label)) / 2, y + box_h / 2 + 1, label, color, fill);

            if self.magi.state(unit, now).is_none() {
                // Flickering braille noise while the unit thinks
                let mut noise = Canvas::new(box_w - 6, 1);
                let mut rng = Rng::seeded((now * 8.0) as u64 * 3 + unit as u64);
                for px in 0..noise.width() {
                    for py in 0..4 {
                        if rng.chance(0.18) {
                            noise.plot(px, py, theme.yellow.dim(0.45));
                        }
                    }
                }
                noise.blit(screen, x + 3, y + box_h - 2);
            }
        }

        let verdict_y = side_y + box_h;
        screen.fill_row(verdict_y, BLACK);
        match self.magi.verdict(now) {
            Some((text, passed)) => {
                let color = if passed { theme.green } else { theme.red };
                screen.center(verdict_y, &format!("決議: {text}"), color, BLACK);
            }
            None => screen.center(verdict_y, "審議中 · DELIBERATING", theme.dim, BLACK),
        }
    }

    /// Braille oscilloscope traces of the three Evas' sync ratios in the
    /// left half, titled in the divider row `title_y` and drawn below it.
    pub fn draw_waveforms(&self, screen: &mut Screen, now: f32, title_y: i32, bottom: i32) {
        let theme = &self.theme;
        let half = screen.w / 2;
        let top = title_y + 1;
        screen.fill(0, top, half, bottom - top, BLACK);
        // Scroll at 12 fps; smoother isn't worth the terminal's redraws
        let now = (now * 12.0).floor() / 12.0;

        // Drawn back to front; where traces cross, the brighter color wins
        let units: [(&str, Rgb, f32, f32); 3] =
            [("00", theme.blue, 0.38, 0.55), ("02", theme.red, 0.44, 0.75), ("01", theme.green, 0.41, 0.95)];
        let mut canvas = Canvas::new(half - 2, bottom - top);
        let (w, h) = (canvas.width(), canvas.height() as f32);
        const AMP: f32 = 0.02;
        // Wobble around each unit's base ratio, -1..1 in units of AMP
        let wave = |i: usize, x: f32| {
            let fi = i as f32;
            let xw = x + now * 14.0;
            0.55 * (xw * 0.16 * (1.0 + 0.35 * fi) + now * 1.3 + fi * 2.1).sin()
                + 0.3 * (xw * 0.061 - now * 0.8 + fi).sin()
                + 0.15 * (xw * 0.41 + now * 2.2 + fi * 4.0).sin()
        };

        for (i, &(_, color, _, reach)) in units.iter().enumerate() {
            let to_y = |v: f32| (h - 1.0) / 2.0 - v * reach * (h - 1.0) / 2.0;
            let mut prev = to_y(wave(i, 0.0));
            for x in 1..w {
                let y = to_y(wave(i, x as f32));
                canvas.line((x - 1) as f32, prev, x as f32, y, color);
                prev = y;
            }
        }
        canvas.blit(screen, 1, top);

        // Title and live values in the divider row, in unit order
        let mut x = screen.put(1, title_y, " SYNC ", theme.dim, BLACK);
        let mut order: Vec<usize> = (0..units.len()).collect();
        order.sort_by_key(|&i| units[i].0);
        for i in order {
            let (name, color, base, _) = units[i];
            let value = (base + AMP * wave(i, (w - 1) as f32)) * 100.0;
            if x + 11 < half {
                x = screen.put(x, title_y, &format!("{name} "), theme.dim, BLACK);
                x = screen.put(x, title_y, &format!("{value:4.1}% "), color, BLACK);
            }
        }
    }

    fn draw_log(&self, screen: &mut Screen, now: f32, top: i32, bottom: i32) {
        let theme = &self.theme;
        let half = screen.w / 2;
        let rows = (bottom - top - 1).max(0) as usize;
        screen.put(half + 1, top, " MAGI LOG ", theme.dim, BLACK);
        let lines = &self.log[self.log.len().saturating_sub(rows)..];
        let max = (screen.w - half - 3).max(0) as usize;
        for (i, line) in lines.iter().enumerate() {
            let age = lines.len() - 1 - i;
            let fade = age as f32 / rows.max(1) as f32;
            let color = theme.fg.dim(0.15 + fade * 0.6);
            let y = top + 1 + i as i32;
            let full = format!("> {}", line.text);
            if age == 0 {
                // The newest line types itself out
                let shown = ((now - line.at) * LOG_TYPE_SPEED) as usize + 2;
                let text: String = full.chars().take(shown.min(max)).collect();
                let end = screen.put(half + 1, y, &text, theme.fg, BLACK);
                if shown < full.chars().count() && App::blink(now, 0.15) {
                    screen.put(end, y, "▌", theme.accent, BLACK);
                }
            } else {
                let text: String = full.chars().take(max).collect();
                screen.put(half + 1, y, &text, color, BLACK);
            }
        }
    }

    pub fn draw_footer(&self, screen: &mut Screen, now: f32) {
        let theme = &self.theme;
        let y = screen.h - 1;
        screen.fill_row(y, BLACK);
        screen.put(1, y, "SELF-DESTRUCT: DISABLED", theme.dim, BLACK);
        let status = if self.alert_start.is_some() { "ALERT" } else { "STANDBY" };
        let color = if self.alert_start.is_some() {
            theme.red
        } else if App::blink(now, 1.0) {
            theme.accent
        } else {
            theme.dim
        };
        screen.put(screen.w - text_width(status) - 1, y, status, color, BLACK);
        if screen.w >= 70 {
            screen.center(y, &format!("TERMINAL DOGMA · {}", self.host), theme.dim, BLACK);
        }
    }
}
