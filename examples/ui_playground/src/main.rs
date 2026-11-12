//! ```text
//! ui_playground play   [--scale N]                      open the playground in a window
//! ui_playground render [--out FILE.png] [--scale N]     write a screenshot (CPU render)
//!                      [--accent ORANGE|TEAL|VIOLET] [--dialog] [--page NAME]
//! ui_playground smoke  [--frames N] [--page NAME]       open a hidden window, draw, check
//! ui_playground ... --font FILE.ttf [--proportional]    draw text with a TrueType face (monospaced, or shaped
//!                                                       and proportional for labels and paragraphs)
//! ui_playground render --click EXPR ...                 click entities (Python expressions) first
//! ui_playground play   --pick-file                      open the system file dialog on start
//! ui_playground play   --record FILE                    also write the session as `sage-input v2`
//! ui_playground access [--page NAME]                    print what a screen reader is told
//! ui_playground replay FILE [--out FILE.png]            replay a recorded session (no window)
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
    record: Option<String>,
    file: Option<String>,
    /// Python expressions for entities to click before the picture (`render`).
    clicks: Vec<String>,
    /// A TrueType file to draw text with instead of the built-in face.
    font: Option<String>,
    proportional: bool,
    pick_file: bool,
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
        record: None,
        file: None,
        clicks: vec![],
        font: None,
        proportional: false,
        pick_file: false,
    };
    while let Some(flag) = it.next() {
        let mut value = |what: &str| it.next().ok_or(format!("{flag} needs {what}"));
        match flag.as_str() {
            "--out" => a.out = value("a file")?,
            "--scale" => a.scale = value("a number")?.parse().map_err(|_| "bad --scale")?,
            "--accent" => a.accent = Some(value("a name")?.to_uppercase()),
            "--frames" => a.frames = value("a number")?.parse().map_err(|_| "bad --frames")?,
            "--dialog" => a.dialog = true,
            "--record" => a.record = Some(value("a file")?),
            "--click" => a.clicks.push(value("an expression")?),
            "--proportional" => a.proportional = true,
            "--pick-file" => a.pick_file = true,
            "--font" => a.font = Some(value("a .ttf file")?),
            "--page" => a.page = Some(value("a name")?.to_uppercase()),
            other if !other.starts_with("--") && a.file.is_none() => a.file = Some(other.into()),
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
    if let Some(path) = &a.font {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if a.proportional {
            playground.set_face_proportional(bytes)?;
        } else {
            playground.set_face(bytes)?;
        }
    }
    match a.cmd.as_str() {
        "play" => {
            let options = WindowOptions::new("UI Playground", pixels(a.scale));
            if a.record.is_some() {
                playground.start_recording(1);
            }
            if a.pick_file {
                // Ask for the system file dialog as soon as the window is up.
                playground.show_page("PICKERS");
                playground.click("native_pick");
            }
            let run = run_ui_windowed(&mut playground, &options)?;
            println!("{} frames, {} ticks", run.frames, run.ticks);
            if a.pick_file {
                println!(
                    "file chosen: {:?}",
                    playground.eval::<String>("ui.file_value(native_pick)")
                );
            }
            if let (Some(path), Some(rec)) = (&a.record, playground.take_recording()) {
                std::fs::write(path, rec.to_text()).map_err(|e| format!("{path}: {e}"))?;
                println!("recorded {} ticks to {path}", rec.len);
            }
        }
        "access" => {
            if let Some(page) = &a.page {
                playground.show_page(page);
            }
            playground.idle(3);
            print!("{}", playground.access_outline());
        }
        "replay" => {
            let path = a.file.as_deref().ok_or("replay needs a recording file")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let rec =
                sage_core::input::InputRecording::from_text(&text).map_err(|e| e.to_string())?;
            let hashes = playground.replay(&rec);
            println!(
                "replayed {} ticks, final state hash {:016x}",
                hashes.len(),
                hashes.last().copied().unwrap_or(0)
            );
            if a.out != "ui-playground.png" || a.scale != 2 {
                let image =
                    playground.render([SCREEN[0] as u32 * a.scale, SCREEN[1] as u32 * a.scale]);
                std::fs::write(&a.out, image.to_png_bytes()?).map_err(|e| e.to_string())?;
            }
        }
        "render" => {
            if let Some(name) = &a.accent {
                playground.run(&format!("T.apply(T.accents[{name:?}])"));
            }
            if let Some(page) = &a.page {
                playground.show_page(page);
            }
            playground.idle(40);
            for expr in &a.clicks {
                playground.click(expr);
                playground.idle(2);
            }
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
