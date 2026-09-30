//! MAGI: a NERV-style terminal screensaver for Omarchy.
//!
//! Omarchy's screensaver launcher (one fullscreen window per monitor, tracked
//! by the idle timer) runs this inside its windows in place of
//! omarchy-screensaver; see the README for setting that up.
//!
//! Run it directly to preview it; any key exits. `--alert` starts on a
//! PATTERN BLUE alert and `--no-boot` skips the boot sequence. MAGI_HOST
//! replaces the hostname shown in the footer.

mod alert;
mod app;
mod boot;
mod dashboard;
mod sys;
mod term;

use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::{cursor, execute, terminal};

use app::App;
use term::Screen;

/// Frames per second, unless MAGI_FPS says otherwise. The terminal does most
/// of the work (each frame with changes costs it a full redraw), so this is
/// the main lever on CPU use.
const DEFAULT_FPS: f32 = 15.0;
const SCREENSAVER_CLASS: &str = "org.omarchy.screensaver";

/// Omarchy's launcher runs us in a terminal with the screensaver class.
fn in_screensaver_window() -> bool {
    let parent = std::os::unix::process::parent_id();
    std::fs::read(format!("/proc/{parent}/cmdline"))
        .map(|cmdline| String::from_utf8_lossy(&cmdline).contains(SCREENSAVER_CLASS))
        .unwrap_or(false)
}

fn screensaver_in_focus() -> bool {
    Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(&format!("\"class\": \"{SCREENSAVER_CLASS}\"")))
        .unwrap_or(false)
}

fn hyprctl_quiet(args: &[&str]) -> bool {
    Command::new("hyprctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn set_cursor_invisible(invisible: bool) {
    let value = if invisible { "true" } else { "false" };
    let lua = format!("hl.config({{ cursor = {{ invisible = {value} }} }})");
    if !hyprctl_quiet(&["eval", &lua]) {
        hyprctl_quiet(&["keyword", "cursor:invisible", value]);
    }
}

/// Terminals start at 80x24 and resize once the compositor sizes the
/// window, so wait for that before laying anything out.
fn settled_size() -> (u16, u16) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let size = terminal::size().unwrap_or((80, 24));
        if size != (80, 24) || Instant::now() >= deadline {
            return size;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn close_screensavers() {
    // Close the screensaver on every monitor, as Omarchy's does
    let pattern = format!("[{}]{}", &SCREENSAVER_CLASS[..1], &SCREENSAVER_CLASS[1..]);
    let _ = Command::new("pkill").args(["-f", &pattern]).status();
}

/// Exit on SIGTERM, SIGHUP or SIGINT from a separate thread. When the
/// terminal closes it stops reading, so the main thread can be stuck in a
/// write that never finishes; this restores the mouse cursor regardless.
fn exit_on_signals(screensaver: bool) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGTERM, SIGHUP, SIGINT]) else { return };
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            if screensaver {
                set_cursor_invisible(false);
                close_screensavers();
            }
            std::process::exit(0);
        }
    });
}

fn run(screensaver: bool, boot: bool, alert: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    let (w, h) = settled_size();
    let mut screen = Screen::new(w, h);
    let started = Instant::now();
    let seconds = || started.elapsed().as_secs_f32();
    let mut app = App::new(seconds(), boot, alert);
    let mut next_focus_check = 2.0;
    let fps = std::env::var("MAGI_FPS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(DEFAULT_FPS);
    let frame = Duration::from_secs_f32(1.0 / fps.clamp(1.0, 60.0));

    loop {
        let frame_start = Instant::now();
        let now = seconds();
        // Check the size every frame: the window can go fullscreen before
        // resize events are being listened for
        let (w, h) = terminal::size()?;
        if (w as i32, h as i32) != (screen.w, screen.h) {
            screen.resize(w, h);
            execute!(out, terminal::Clear(terminal::ClearType::All))?;
        }
        app.update(&screen, now);
        app.draw(&mut screen, now);
        screen.flush(&mut out)?;

        if screensaver && now >= next_focus_check {
            if !screensaver_in_focus() {
                return Ok(());
            }
            next_focus_check = now + 1.0;
        }

        let deadline = frame_start + frame;
        while let Some(wait) = deadline.checked_duration_since(Instant::now()) {
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => return Ok(()),
                _ => {}
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let alert = args.iter().any(|a| a == "--alert");
    let boot = !alert && !args.iter().any(|a| a == "--no-boot");
    let screensaver = in_screensaver_window();

    exit_on_signals(screensaver);

    if screensaver {
        set_cursor_invisible(true);
    }
    let setup = (|| -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
        // Black background, like Omarchy's own screensaver
        write!(out, "\x1b]11;rgb:00/00/00\x07\x1b[2J")?;
        out.flush()
    })();

    // The terminal can vanish under us (closed along with another monitor's
    // screensaver), so errors from here on just mean it's time to stop
    let result = setup.and_then(|()| run(screensaver, boot, alert));

    // Restore the mouse cursor first: when the window is closed from
    // outside, the terminal is already gone and resetting it fails
    if screensaver {
        set_cursor_invisible(false);
    }
    let mut out = io::stdout();
    let _ = write!(out, "\x1b[0m");
    let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    if screensaver {
        close_screensavers();
    }
    if let Err(err) = result
        && !screensaver
    {
        eprintln!("magi: {err}");
    }
}
