//! Screensaver state: the MAGI's deliberations, A.T. Field ripples, the log,
//! and the timeline that moves between boot, dashboard and alert.

use crate::sys::{self, Rng, Theme};
use crate::term::{Rgb, Screen};

/// Seconds from an alert's start until Ramiel's tracking view replaces the
/// EMERGENCY banner, and until the alert ends.
pub const ALERT_BANNER: f32 = 4.5;
pub const ALERT_LENGTH: f32 = 18.0;

pub const TOPICS: &[(&str, &str, bool)] = &[
    ("第3新東京市 迎撃システム起動", "ACTIVATE INTERCEPTION SYSTEM", false),
    ("EVA初号機 凍結解除", "UNFREEZE EVA UNIT-01", false),
    ("本部 自爆決議", "HQ SELF-DESTRUCT", true),
    ("ジオフロント 隔壁閉鎖", "SEAL GEOFRONT BULKHEADS", false),
    ("N2兵器 使用許可", "AUTHORIZE N2 WEAPONS", false),
    ("LCL 濃度調整", "ADJUST LCL CONCENTRATION", false),
    ("ヤシマ作戦 発動", "INITIATE OPERATION YASHIMA", false),
    ("初号機 起動試験", "UNIT-01 ACTIVATION TEST", false),
    ("零号機 再起動実験", "UNIT-00 REACTIVATION TEST", false),
    ("第1種戦闘配置", "LEVEL 1 BATTLE STATIONS", false),
];

const FLAVOR: &[&str] = &[
    "CENTRAL DOGMA: ALL SECTORS NOMINAL",
    "A.T. FIELD: NOT DETECTED",
    "LCL PURITY 99.89%",
    "EVA-01: CAGE 7 · COOLANT NOMINAL",
    "UMBILICAL CABLE: CONNECTED",
    "GEOFRONT BULKHEADS: OPEN",
    "PATTERN ANALYSIS: NO MATCH",
    "TERMINAL DOGMA: SEALED",
];

pub const UNITS: [&str; 3] = ["BALTHASAR·2", "CASPER·3", "MELCHIOR·1"];

pub struct Magi {
    pub number: u32,
    pub topic: usize,
    /// When each unit decides, and how it votes.
    pub votes: [(f32, bool); 3],
    pub decided_at: f32,
}

impl Magi {
    fn new(rng: &mut Rng, now: f32) -> Magi {
        let mut magi = Magi { number: 100 + rng.below(800) as u32, topic: 0, votes: [(0.0, false); 3], decided_at: 0.0 };
        magi.start(rng, now);
        magi
    }

    fn start(&mut self, rng: &mut Rng, now: f32) {
        self.number += 1;
        self.topic = rng.below(TOPICS.len());
        let always_reject = TOPICS[self.topic].2;
        for vote in &mut self.votes {
            *vote = (now + rng.range(2.0, 7.0), !always_reject && rng.chance(0.8));
        }
        self.decided_at = self.votes.iter().map(|v| v.0).fold(0.0, f32::max);
    }

    /// None while deliberating, else how the unit voted.
    pub fn state(&self, unit: usize, now: f32) -> Option<bool> {
        let (at, vote) = self.votes[unit];
        (now >= at).then_some(vote)
    }

    /// Seconds since the unit voted, if it has.
    pub fn since_vote(&self, unit: usize, now: f32) -> Option<f32> {
        let at = self.votes[unit].0;
        (now >= at).then_some(now - at)
    }

    pub fn verdict(&self, now: f32) -> Option<(&'static str, bool)> {
        if now < self.decided_at {
            return None;
        }
        Some(match self.votes.iter().filter(|v| v.1).count() {
            3 => ("全会一致 承認 · UNANIMOUS APPROVAL", true),
            2 => ("多数決 承認 2:1 · APPROVED BY MAJORITY", true),
            _ => ("否決 · PROPOSAL REJECTED", false),
        })
    }
}

/// A hexagonal shockwave spreading across the hex grid.
pub struct Ripple {
    pub x: f32,
    pub y: f32,
    pub born: f32,
    pub speed: f32,
    pub reach: f32,
    pub color: Rgb,
}

impl Ripple {
    /// How brightly this ripple lights the cell at x, y right now (0..1).
    pub fn intensity(&self, x: i32, y: i32, now: f32) -> f32 {
        let radius = (now - self.born) * self.speed;
        if radius > self.reach {
            return 0.0;
        }
        // Cells are about twice as tall as wide, so scale y to match x. The
        // norm makes the rings flat-topped hexagons rather than circles
        let dx = (x as f32 - self.x).abs();
        let dy = ((y as f32 - self.y) * 2.0).abs();
        let d = (dy * 1.155).max(dx + dy * 0.577);
        let ring = (1.0 - (d - radius).abs() / 2.2).max(0.0) * (1.0 - radius / self.reach);
        // Round to a few steps so cells only change when their step does:
        // the terminal pays for every cell that changes
        (ring * 6.0).round() / 6.0
    }
}

pub struct LogLine {
    pub text: String,
    pub at: f32,
}

pub struct App {
    pub theme: Theme,
    pub rng: Rng,
    pub host: String,
    pub cpu: String,
    pub memory: f32,
    /// When the boot sequence ends and the dashboard is fully revealed, or
    /// None to skip it.
    pub boot_until: Option<f32>,
    pub magi: Magi,
    pub ripples: Vec<Ripple>,
    next_ripple: f32,
    pub log: Vec<LogLine>,
    next_log: f32,
    stats: Vec<String>,
    pub alert_start: Option<f32>,
    next_alert: f32,
}

impl App {
    pub fn new(now: f32, boot: bool, alert: bool) -> App {
        let mut rng = Rng::new();
        let magi = Magi::new(&mut rng, now);
        let next_alert = now + rng.range(150.0, 360.0);
        App {
            theme: Theme::load(),
            rng,
            host: sys::hostname(),
            cpu: sys::cpu(),
            memory: sys::memory_total(),
            boot_until: boot.then_some(now + crate::boot::LENGTH),
            magi,
            ripples: Vec::new(),
            next_ripple: now,
            log: Vec::new(),
            next_log: now,
            stats: Vec::new(),
            alert_start: alert.then_some(now),
            next_alert,
        }
    }

    pub fn booting(&self, now: f32) -> bool {
        self.boot_until.is_some_and(|until| now < until)
    }

    /// Seconds into the current alert, if there is one.
    pub fn alert(&self, now: f32) -> Option<f32> {
        self.alert_start.map(|start| now - start)
    }

    pub fn update(&mut self, screen: &Screen, now: f32) {
        if self.booting(now) {
            // Keep the MAGI from deciding before anyone can see them
            if now > self.magi.decided_at - 3.0 {
                self.magi.start(&mut self.rng, now);
            }
            return;
        }
        if now > self.magi.decided_at + 6.0 {
            self.magi.start(&mut self.rng, now);
        }

        let alert = self.alert(now);
        if now >= self.next_ripple {
            let reach = screen.w as f32 * if alert.is_some() { 0.7 } else { 0.5 };
            let color = if alert.is_some() { self.theme.red } else { self.theme.accent };
            self.ripples.push(Ripple {
                x: self.rng.range(0.0, screen.w as f32),
                y: self.rng.range(2.0, screen.h as f32 * 0.65),
                born: now,
                speed: self.rng.range(14.0, 22.0),
                reach,
                color,
            });
            self.next_ripple = now + if alert.is_some() { self.rng.range(0.5, 1.1) } else { self.rng.range(2.5, 5.0) };
        }
        self.ripples.retain(|r| (now - r.born) * r.speed < r.reach);

        if now >= self.next_log {
            if self.stats.is_empty() {
                self.stats = sys::stats();
                // Shuffle so the stats come out in a different order each round
                for i in (1..self.stats.len()).rev() {
                    let j = self.rng.below(i + 1);
                    self.stats.swap(i, j);
                }
            }
            let text = if self.rng.chance(0.6) && !self.stats.is_empty() {
                self.stats.pop().unwrap_or_default()
            } else {
                FLAVOR[self.rng.below(FLAVOR.len())].to_string()
            };
            self.push_log(text, now);
            self.next_log = now + self.rng.range(1.8, 3.2);
        }

        match alert {
            None if now >= self.next_alert => {
                self.alert_start = Some(now);
                self.push_log("!! PATTERN BLUE DETECTED !!".into(), now);
            }
            Some(elapsed) if elapsed >= ALERT_LENGTH => {
                self.alert_start = None;
                self.next_alert = now + self.rng.range(150.0, 360.0);
                self.push_log("TARGET LOST · RETURNING TO STANDBY".into(), now);
            }
            _ => {}
        }
    }

    fn push_log(&mut self, text: String, now: f32) {
        self.log.push(LogLine { text, at: now });
        if self.log.len() > 40 {
            self.log.remove(0);
        }
    }

    pub fn blink(now: f32, period: f32) -> bool {
        (now / period) as i64 % 2 == 0
    }

    pub fn draw(&mut self, screen: &mut Screen, now: f32) {
        screen.clear();
        if self.booting(now) {
            let boot_until = self.boot_until.unwrap_or(now);
            let reveal_from = boot_until - crate::boot::REVEAL;
            if now < reveal_from {
                self.draw_boot(screen, now - (boot_until - crate::boot::LENGTH));
                return;
            }
            // Reveal the dashboard top to bottom behind a bright scanline
            self.draw_dashboard(screen, now);
            let progress = (now - reveal_from) / crate::boot::REVEAL;
            let edge = (progress * (screen.h + 1) as f32) as i32;
            screen.fill(0, edge + 1, screen.w, screen.h, crate::term::BLACK);
            let line = "▀".repeat(screen.w.max(0) as usize);
            screen.put(0, edge, &line, self.theme.accent, crate::term::BLACK);
            return;
        }
        match self.alert(now) {
            Some(elapsed) if elapsed < ALERT_BANNER => self.draw_banner(screen, now),
            Some(elapsed) => self.draw_ramiel(screen, now, elapsed - ALERT_BANNER),
            None => self.draw_dashboard(screen, now),
        }
    }

    /// The hex grid backdrop, lit by any passing ripples.
    pub fn draw_hex(&self, screen: &mut Screen, now: f32, base: Rgb, top: i32, bottom: i32) {
        const TILES: [&[u8; 6]; 2] = [b"__/  \\", b"  \\__/"];
        for y in top..bottom {
            let tile = TILES[(y % 2) as usize];
            for x in 0..screen.w {
                let ch = tile[(x % 6) as usize];
                if ch == b' ' {
                    continue;
                }
                let mut color = base;
                for ripple in &self.ripples {
                    let v = ripple.intensity(x, y, now);
                    if v > 0.01 {
                        color = color.mix(ripple.color, v);
                    }
                }
                screen.put_char(x, y, ch as char, color, crate::term::BLACK);
            }
        }
    }
}
