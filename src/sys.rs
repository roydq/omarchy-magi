//! Theme colors, system stats, the clock and a small random number generator.

use std::fs;
use std::path::PathBuf;

use crate::term::Rgb;

pub struct Theme {
    pub accent: Rgb,
    pub fg: Rgb,
    pub dim: Rgb,
    pub muted: Rgb,
    pub red: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub blue: Rgb,
    pub cyan: Rgb,
    pub magenta: Rgb,
}

impl Theme {
    /// The current Omarchy theme's colors, with NERV colors for anything it
    /// doesn't define.
    pub fn load() -> Theme {
        let mut theme = Theme {
            accent: Rgb(0xff, 0x6a, 0x00),
            fg: Rgb(0xff, 0x9d, 0x45),
            dim: Rgb(0x8f, 0x4e, 0x1c),
            muted: Rgb(0x7a, 0x3f, 0x14),
            red: Rgb(0xff, 0x2a, 0x2a),
            green: Rgb(0x3d, 0xff, 0x7a),
            yellow: Rgb(0xff, 0xcc, 0x00),
            blue: Rgb(0x5a, 0x86, 0xff),
            cyan: Rgb(0x2f, 0xe6, 0xe6),
            magenta: Rgb(0xa6, 0x6b, 0xff),
        };
        let path = home().join(".local/state/omarchy/current/theme/colors.toml");
        let Ok(text) = fs::read_to_string(path) else { return theme };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Some(color) = Rgb::parse(value.trim().trim_matches('"')) else { continue };
            match key.trim() {
                "accent" => theme.accent = color,
                "foreground" => theme.fg = color,
                "dark_foreground" => theme.dim = color,
                "muted" => theme.muted = color,
                "red" => theme.red = color,
                "green" => theme.green = color,
                "yellow" => theme.yellow = color,
                "blue" => theme.blue = color,
                "cyan" => theme.cyan = color,
                "magenta" => theme.magenta = color,
                _ => {}
            }
        }
        theme
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn first_line(path: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    Some(text.lines().next()?.trim().to_string())
}

pub fn hostname() -> String {
    std::env::var("MAGI_HOST")
        .ok()
        .or_else(|| first_line("/proc/sys/kernel/hostname"))
        .unwrap_or_else(|| "MAGI".into())
        .to_uppercase()
}

/// Total memory in GB.
pub fn memory_total() -> f32 {
    meminfo("MemTotal").unwrap_or(0) as f32 / 1048576.0
}

fn meminfo(key: &str) -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with(key))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The CPU model and core count, for the boot sequence.
pub fn cpu() -> String {
    let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = text
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, m)| m.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "UNKNOWN".into());
    let cores = text.lines().filter(|l| l.starts_with("processor")).count();
    format!("{} ×{}", model.replace("(R)", "").replace("(TM)", "").to_uppercase(), cores.max(1))
}

/// A rotating set of real stats for the MAGI log.
pub fn stats() -> Vec<String> {
    let mut stats = Vec::new();
    if let Some(load) = first_line("/proc/loadavg") {
        let parts: Vec<_> = load.split_whitespace().take(3).collect();
        stats.push(format!("MAGI LOAD {}", parts.join(" / ")));
    }
    if let (Some(total), Some(avail)) = (meminfo("MemTotal"), meminfo("MemAvailable")) {
        let gb = |kb: u64| kb as f32 / 1048576.0;
        stats.push(format!("MEMORY {:.1} / {:.1} GB", gb(total - avail), gb(total)));
    }
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        let mut batteries: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("BAT")))
            .collect();
        batteries.sort();
        if let Some(bat) = batteries.first() {
            let read = |f: &str| first_line(&bat.join(f).to_string_lossy());
            if let (Some(capacity), Some(status)) = (read("capacity"), read("status")) {
                stats.push(format!("POWER {capacity}% · {}", status.to_uppercase()));
            }
        }
    }
    if let Some(temp) = first_line("/sys/class/thermal/thermal_zone0/temp").and_then(|t| t.parse::<f32>().ok()) {
        stats.push(format!("CORE TEMP {:.0}°C", temp / 1000.0));
    }
    if let Some(uptime) = first_line("/proc/uptime").and_then(|u| u.split_whitespace().next()?.parse::<f64>().ok()) {
        let s = uptime as u64;
        stats.push(format!("UPTIME {}d {}h {}m", s / 86400, s % 86400 / 3600, s % 3600 / 60));
    }
    stats
}

/// The local time formatted with strftime.
pub fn clock(format: &str) -> String {
    let fmt = std::ffi::CString::new(format).unwrap_or_default();
    let mut buf = [0u8; 64];
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        let n = libc::strftime(buf.as_mut_ptr().cast(), buf.len(), fmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// xorshift64*, seeded from the clock. Plenty for animation.
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Rng {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545f4914f6cdd1d);
        Rng(seed | 1)
    }

    /// A generator that gives the same numbers for the same seed, for
    /// effects that should only change a few times a second.
    pub fn seeded(seed: u64) -> Rng {
        let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
        rng.next();
        rng
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545f4914f6cdd1d)
    }

    /// A float in [0, 1).
    pub fn float(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.float()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.float() < p
    }
}
