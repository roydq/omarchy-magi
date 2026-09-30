//! The MAGI system boot that plays when the screensaver starts.

use crate::app::App;
use crate::term::{BLACK, Screen};

/// How long the boot sequence runs, including the final reveal.
pub const LENGTH: f32 = 7.6;
/// How long the dashboard takes to be revealed at the end.
pub const REVEAL: f32 = 0.7;

/// Characters typed per second.
const TYPE_SPEED: f32 = 90.0;
/// How long the dot leader takes to run out to the status.
const LEADER_TIME: f32 = 0.18;

enum Status {
    Ok,
    Online,
}

struct Step {
    label: String,
    status: Status,
    /// Extra time the step holds before its status appears (the memory test).
    work: f32,
}

impl App {
    fn boot_steps(&self) -> Vec<Step> {
        let step = |label: String, status, work| Step { label, status, work };
        vec![
            step("BIOS CHECK".into(), Status::Ok, 0.0),
            step(format!("CPU  {}", self.cpu), Status::Ok, 0.0),
            step("MEMORY TEST".into(), Status::Ok, 1.1),
            step("PERSONALITY IMPRINT · DR. NAOKO AKAGI".into(), Status::Ok, 0.3),
            step("MELCHIOR·1".into(), Status::Online, 0.25),
            step("BALTHASAR·2".into(), Status::Online, 0.25),
            step("CASPER·3".into(), Status::Online, 0.25),
            step("LINK CENTRAL DOGMA".into(), Status::Ok, 0.2),
        ]
    }

    /// Draw the boot `t` seconds in.
    pub fn draw_boot(&mut self, screen: &mut Screen, t: f32) {
        let theme = &self.theme;
        let steps = self.boot_steps();
        let x = ((screen.w - 64) / 2).max(1);
        let status_col = (x + 46).min(screen.w - 9);

        let title = "NERV · 特務機関ネルフ";
        screen.center(1, title, theme.accent, BLACK);
        screen.center(2, "MAGI SYSTEM BOOT SEQUENCE · Ver. 2.01", theme.dim, BLACK);

        let mut start = 0.35;
        let top = 4;
        let mut y = top;
        for step in &steps {
            if t < start || y >= screen.h - 2 {
                break;
            }
            let typed = ((t - start) * TYPE_SPEED) as usize;
            let label: String = step.label.chars().take(typed).collect();
            let end = screen.put(x, y, &label, theme.fg, BLACK);
            let typed_all = typed >= step.label.chars().count();
            let type_time = step.label.chars().count() as f32 / TYPE_SPEED;
            let leader_start = start + type_time;

            if !typed_all {
                screen.put(end, y, "▌", theme.accent, BLACK);
            } else {
                // Dot leader out to the status column
                let run = ((t - leader_start) / LEADER_TIME).clamp(0.0, 1.0);
                let dots = ((status_col - 1 - end - 1).max(0) as f32 * run) as usize;
                screen.put(end + 1, y, &".".repeat(dots), theme.dim, BLACK);

                let status_at = leader_start + LEADER_TIME + step.work;
                if step.work > 0.9 {
                    // The memory test counts up with a progress bar
                    let p = ((t - leader_start - LEADER_TIME) / step.work).clamp(0.0, 1.0);
                    let bar_y = y + 1;
                    let bar_w = (status_col - x - 12).clamp(8, 40);
                    let filled = (bar_w as f32 * p) as usize;
                    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(bar_w as usize - filled));
                    screen.put(x + 2, bar_y, &bar, theme.accent, BLACK);
                    screen.put(x + 3 + bar_w, bar_y, &format!("{:4.1} GB", self.memory * p), theme.fg, BLACK);
                }
                if t >= status_at {
                    let (text, color) = match step.status {
                        Status::Ok => ("OK", theme.green),
                        Status::Online => ("ONLINE", theme.green),
                    };
                    screen.put(status_col, y, text, color, BLACK);
                }
            }
            start = leader_start + LEADER_TIME + step.work + 0.05;
            y += if step.work > 0.9 { 2 } else { 1 };
        }

        if t >= start + 0.2 {
            let ready = "ALL SYSTEMS NOMINAL · MAGI ONLINE";
            let color = if App::blink(t, 0.25) { theme.accent } else { theme.fg };
            screen.center((y + 1).min(screen.h - 1), ready, color, BLACK);
        }
    }
}
