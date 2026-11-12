//! ```text
//! ui_playground play   [--scale N]                      open the playground in a window
//! ui_playground render [--out FILE.png] [--scale N]     write a screenshot (CPU render)
//!                      [--accent ORANGE|TEAL|VIOLET] [--dialog] [--page NAME]
//! ui_playground smoke  [--frames N] [--page NAME]       open a hidden window, draw, check
//! ```
//! In the window: mouse, wheel and typing work; Tab and Shift+Tab move focus,
//! Enter presses, arrows adjust sliders and move the caret, PageUp and
//! PageDown scroll, Esc quits. Exit code is non-zero on any failure.

use std::process::ExitCode;

use sage_platform::WindowOptions;
use sage_platform::ui_window::run_ui_windowed;
use ui_playground::{CLEAR, Playground, SCREEN};

struct Args {
    cmd: String,
    out: String,
    scale: u32,
    accent: Option<String>,
    dialog: bool,
    page: Option<String>,
    frames: u64,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        cmd: it.next().unwrap_or_else(|| "play".into()),
        out: "ui-playground.png".into(),
        scale: 2,
        accent: None,
        dialog: false,
        page: None,
        frames: 5,
    };
    while let Some(flag) = it.next() {
        let mut value = |what: &str| it.next().ok_or(format!("{flag} needs {what}"));
        match flag.as_str() {
            "--out" => a.out = value("a file")?,
            "--scale" => a.scale = value("a number")?.parse().map_err(|_| "bad --scale")?,
            "--accent" => a.accent = Some(value("a name")?.to_uppercase()),
            "--frames" => a.frames = value("a number")?.parse().map_err(|_| "bad --frames")?,
            "--dialog" => a.dialog = true,
            "--page" => a.page = Some(value("a name")?.to_uppercase()),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if a.scale == 0 {
        return Err("--scale must be at least 1".into());
    }
    Ok(a)
}

fn pixels(scale: u32) -> (u32, u32) {
    (SCREEN[0] as u32 * scale, SCREEN[1] as u32 * scale)
}

fn run() -> Result<(), String> {
    let a = parse()?;
    let mut playground = Playground::new()?;
    match a.cmd.as_str() {
        "play" => {
            let options = WindowOptions::new("UI Playground", pixels(a.scale));
            let run = run_ui_windowed(&mut playground, &options)?;
            println!("{} frames, {} ticks", run.frames, run.ticks);
        }
        "render" => {
            if let Some(name) = &a.accent {
                playground.run(&format!("T.apply(T.accents[{name:?}])"));
            }
            if let Some(page) = &a.page {
                playground.show_page(page);
            }
            playground.idle(40);
            playground.hover(playground.center("primary"));
            if a.dialog {
                playground.click("dialog_button");
                playground.idle(2);
            }
            let (w, h) = pixels(a.scale);
            let png = playground.render([w, h]).to_png_bytes()?;
            std::fs::write(&a.out, png).map_err(|e| format!("{}: {e}", a.out))?;
            println!("wrote {} ({w}x{h})", a.out);
        }
        "smoke" => {
            if let Some(page) = &a.page {
                playground.show_page(page);
            }
            let mut options = WindowOptions::new("UI Playground (smoke)", pixels(2));
            options.hidden = true;
            options.max_frames = Some(a.frames);
            let run = run_ui_windowed(&mut playground, &options)?;
            let image = run.last_frame.ok_or("no frame was read back")?;
            // The app background is the clear color's neighbour, the title is
            // drawn in the primary orange: both must be on screen.
            let mut orange = 0usize;
            let mut dark = 0usize;
            let mut magenta = 0usize;
            for y in 0..image.height {
                for x in 0..image.width {
                    let p = image.pixel(x, y);
                    if p[0] > 200 && (100..180).contains(&p[1]) && p[2] < 80 {
                        orange += 1;
                    }
                    if p[..3] == CLEAR[..3] {
                        dark += 1;
                    }
                    if p[0] > 240 && p[1] < 10 && p[2] > 240 {
                        magenta += 1;
                    }
                }
            }
            println!(
                "{} frames, {} ticks, {}x{}: {orange} orange pixels, {dark} background pixels",
                run.frames, run.ticks, image.width, image.height
            );
            if magenta > 0 {
                return Err(format!(
                    "{magenta} magenta pixels: an image asset is missing"
                ));
            }
            if orange == 0 || dark == 0 {
                return Err("the window did not draw the themed UI".into());
            }
        }
        other => {
            return Err(format!(
                "unknown command {other:?}; see the usage at the top of main.rs"
            ));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
