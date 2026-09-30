//! PATTERN BLUE: the EMERGENCY banner, then Ramiel's tracking view.

use crate::app::{ALERT_LENGTH, ALERT_BANNER, App};
use crate::term::{BLACK, Canvas, Rgb, Screen, text_width};

/// 5-row block letters for the banner.
fn block_letter(c: char) -> [&'static str; 5] {
    match c {
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        'M' => ["█   █", "██ ██", "█ █ █", "█   █", "█   █"],
        'R' => ["████ ", "█   █", "████ ", "█  █ ", "█   █"],
        'G' => [" ████", "█    ", "█  ██", "█   █", " ████"],
        'N' => ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
        'C' => [" ████", "█    ", "█    ", "█    ", " ████"],
        'Y' => ["█   █", " █ █ ", "  █  ", "  █  ", "  █  "],
        _ => ["     "; 5],
    }
}

impl App {
    /// Diagonal hazard stripes moving along the top and bottom edges. Each
    /// stripe is ▟██▛, shifted a cell per row so the half-cell steps line up
    /// into slanted edges. Returns the rows between the bands.
    fn draw_hazard(&self, screen: &mut Screen, now: f32) -> (i32, i32) {
        let offset = (now * 8.0) as i32;
        let pattern: Vec<char> = "▟██▛  ".chars().collect();
        let n = pattern.len() as i32;
        let thick = if screen.h >= 20 { 2 } else { 1 };
        let bands = [0, screen.h - thick];
        for band in bands {
            for row in 0..thick {
                let y = band + row;
                let level = thick - 1 - row;
                for x in 0..screen.w {
                    let c = pattern[(x - level - offset).rem_euclid(n) as usize];
                    screen.put_char(x, y, c, self.theme.accent, BLACK);
                }
            }
        }
        (thick, screen.h - thick)
    }

    pub fn draw_banner(&self, screen: &mut Screen, now: f32) {
        let theme = &self.theme;
        let red = theme.red;
        let (top, bottom) = (0, screen.h);
        self.draw_hex(screen, now, red.dim(0.8), top, bottom);
        self.draw_hazard(screen, now);

        let word = "EMERGENCY";
        let banner_w = word.len() as i32 * 6 - 1;
        let by = screen.h / 2 - 4;
        let frame = if App::blink(now, 0.5) { red } else { red.dim(0.6) };
        if screen.w >= banner_w + 6 {
            screen.frame((screen.w - banner_w) / 2 - 3, by - 1, banner_w + 6, 7, frame, BLACK);
            let x0 = (screen.w - banner_w) / 2;
            for (i, letter) in word.chars().enumerate() {
                for (row, bits) in block_letter(letter).iter().enumerate() {
                    screen.put(x0 + i as i32 * 6, by + row as i32, bits, red, BLACK);
                }
            }
            screen.put((screen.w - banner_w) / 2 - 3, by - 2, " 警告 WARNING ", theme.accent, BLACK);
        } else {
            screen.frame((screen.w - 16) / 2, by + 1, 16, 3, frame, BLACK);
            screen.center(by + 2, word, red, BLACK);
        }
        screen.center(by + 7, " 緊急事態発生 ", red, BLACK);
        if App::blink(now, 0.8) {
            screen.center(by + 8, " PATTERN BLUE · 使徒 確認 · ANGEL CONFIRMED ", theme.fg, BLACK);
        }
    }

    /// Ramiel, the fifth Angel, tracked as a tumbling octahedron. `t` is
    /// seconds since the tracking view took over.
    pub fn draw_ramiel(&self, screen: &mut Screen, now: f32, t: f32) {
        let theme = &self.theme;
        self.draw_hex(screen, now, theme.red.dim(0.82), 0, screen.h);
        let (top, bottom) = self.draw_hazard(screen, now);
        let progress = (t / (ALERT_LENGTH - ALERT_BANNER)).clamp(0.0, 1.0);

        // Targeting frame on the left
        let view_w = (screen.w / 2).max(24).min(screen.w - 2);
        let (vx, vy) = (1, top + 1);
        let view_h = (bottom - top - 2).max(6);
        screen.fill(vx, vy, view_w, view_h, BLACK);
        let lock = if App::blink(now, 0.3) { theme.red } else { theme.red.dim(0.5) };
        screen.corners(vx, vy, view_w, view_h, lock);
        screen.put(vx + 3, vy, " TARGET LOCK ", lock, BLACK);

        let mut canvas = Canvas::new(view_w - 4, view_h - 2);
        let (cw, ch) = (canvas.width() as f32, canvas.height() as f32);
        let (cx, cy) = (cw / 2.0, ch / 2.0);

        // Crosshair and range rings
        for x in (0..canvas.width()).step_by(3) {
            canvas.plot(x, cy as i32, theme.red.dim(0.6));
        }
        for y in (0..canvas.height()).step_by(3) {
            canvas.plot(cx as i32, y, theme.red.dim(0.6));
        }
        let ring_r = cw.min(ch) * 0.46;
        for k in 0..120 {
            let a = k as f32 / 120.0 * std::f32::consts::TAU;
            canvas.plot((cx + ring_r * a.cos()) as i32, (cy + ring_r * a.sin()) as i32, theme.red.dim(0.45));
        }

        // The octahedron grows as it closes in, and pulses
        let scale = cw.min(ch) * (0.27 + 0.1 * progress) * (1.0 + 0.04 * (now * 6.0).sin());
        let (ay, ax) = (now * 0.9, 0.5 + 0.25 * (now * 0.4).sin());
        let rotate = |[x, y, z]: [f32; 3]| {
            let (x, z) = (x * ay.cos() + z * ay.sin(), -x * ay.sin() + z * ay.cos());
            let (y, z) = (y * ax.cos() - z * ax.sin(), y * ax.sin() + z * ax.cos());
            [x, y, z]
        };
        let verts: Vec<[f32; 3]> =
            [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.35, 0.0], [0.0, -1.35, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]]
                .into_iter()
                .map(rotate)
                .collect();
        let project = |[x, y, z]: [f32; 3]| {
            let persp = 3.5 / (3.5 + z);
            (cx + x * scale * persp, cy - y * scale * persp)
        };
        const EDGES: [(usize, usize); 12] =
            [(0, 2), (0, 3), (0, 4), (0, 5), (1, 2), (1, 3), (1, 4), (1, 5), (2, 4), (2, 5), (3, 4), (3, 5)];
        let blue = theme.blue;
        let bright = blue.mix(Rgb(255, 255, 255), 0.35);
        // Back edges first, dimmer, so front edges win where they cross
        let mut edges: Vec<_> = EDGES.iter().map(|&(a, b)| (verts[a][2] + verts[b][2], a, b)).collect();
        edges.sort_by(|p, q| q.0.total_cmp(&p.0));
        for (depth, a, b) in edges {
            let color = if depth > 0.0 { blue.dim(0.45) } else { bright };
            let (x0, y0) = project(verts[a]);
            let (x1, y1) = project(verts[b]);
            canvas.line(x0, y0, x1, y1, color);
        }
        canvas.blit(screen, vx + 2, vy + 1);

        // Readout on the right
        let rx = vx + view_w + 2;
        let rw = screen.w - rx - 1;
        screen.fill(rx - 1, vy, rw + 2, view_h, BLACK);
        let range = 30.0 * (1.0 - progress).powf(1.6) + 0.4;
        let field = 0.55 + 0.45 * (now * 3.0).sin().abs();
        let bar_w = (rw - 12).clamp(4, 16) as usize;
        let filled = (bar_w as f32 * field) as usize;
        let rows: [(&str, String, Rgb); 7] = [
            ("目標", "第5使徒 ラミエル".into(), theme.fg),
            ("TARGET", "5TH ANGEL · RAMIEL".into(), theme.fg),
            ("PATTERN", "BLUE".into(), theme.blue),
            ("RANGE", format!("{range:6.2} km"), theme.yellow),
            ("BEARING", format!("N {:03.0}°", 23.0 + 4.0 * (now * 0.7).sin()), theme.fg),
            ("A.T. FIELD", format!("{}{}", "▰".repeat(filled), "▱".repeat(bar_w - filled)), theme.accent),
            ("ANALYSIS", "解析不能".into(), if App::blink(now, 0.4) { theme.red } else { theme.red.dim(0.5) }),
        ];
        let mut y = vy + 1;
        for (label, value, color) in rows {
            if y >= vy + view_h {
                break;
            }
            screen.put(rx, y, label, theme.dim, BLACK);
            let vxpos = rx + 11;
            if vxpos + text_width(&value) <= screen.w {
                screen.put(vxpos, y, &value, color, BLACK);
            }
            y += if label == "TARGET" { 2 } else { 1 };
        }
        if y + 1 < vy + view_h && App::blink(now, 0.8) {
            screen.put(rx, y + 1, "第1種戦闘配置 · BATTLE STATIONS", theme.red, BLACK);
        }
    }
}
