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

use sage_app::cli::{self, Args};
use sage_app::{UiHosted, run_ui_windowed};
use sage_core::input::InputRecording;
use sage_platform::WindowOptions;
use ui_playground::{CLEAR, Playground, SCREEN};

fn pixels(scale: u32) -> [u32; 2] {
    [SCREEN[0] as u32 * scale, SCREEN[1] as u32 * scale]
}

fn window(title: &str, scale: u32) -> WindowOptions {
    let [w, h] = pixels(scale);
    WindowOptions::new(title, (w, h))
}

fn run() -> Result<(), String> {
    let a = Args::from_env(
        "play",
        &[
            "--out", "--scale", "--accent", "--frames", "--record", "--click", "--font", "--page",
        ],
        &["--dialog", "--proportional", "--pick-file"],
    )?;
    let scale: u32 = a.get("--scale", 2)?;
    if scale == 0 {
        return Err("--scale must be at least 1".into());
    }
    let page = a.value("--page").map(str::to_uppercase);
    let mut playground = Playground::new()?;
    if let Some(path) = a.value("--font") {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if a.has("--proportional") {
            playground.set_face_proportional(bytes)?;
        } else {
            playground.set_face(bytes)?;
        }
    }
    match a.cmd.as_str() {
        "play" => {
            if a.value("--record").is_some() {
                playground.start_recording(1);
            }
            if a.has("--pick-file") {
                // Ask for the system file dialog as soon as the window is up.
                playground.show_page("PICKERS");
                playground.click("native_pick");
            }
            let run = run_ui_windowed(&mut playground, &window("UI Playground", scale))?;
            println!("{} frames, {} ticks", run.frames, run.ticks);
            if a.has("--pick-file") {
                println!(
                    "file chosen: {:?}",
                    playground.eval::<String>("ui.file_value(native_pick)")
                );
            }
            if let (Some(path), Some(rec)) = (a.value("--record"), playground.take_recording()) {
                cli::write_file(path, rec.to_text())?;
                println!("recorded {} ticks to {path}", rec.len);
            }
        }
        "access" => {
            if let Some(page) = &page {
                playground.show_page(page);
            }
            playground.idle(3);
            print!("{}", playground.access_outline());
        }
        "replay" => {
            let path = a.file.as_deref().ok_or("replay needs a recording file")?;
            let rec =
                InputRecording::from_text(&cli::read_text(path)?).map_err(|e| e.to_string())?;
            let hashes = playground.replay(&rec);
            println!(
                "replayed {} ticks, final state hash {:016x}",
                hashes.len(),
                hashes.last().copied().unwrap_or(0)
            );
            // A picture of where the session ended, when one was asked for.
            if a.value("--out").is_some() || a.value("--scale").is_some() {
                let out = a.value("--out").unwrap_or("ui-playground.png");
                cli::write_file(out, playground.render(pixels(scale)).to_png_bytes()?)?;
            }
        }
        "render" => {
            if let Some(name) = a.value("--accent").map(str::to_uppercase) {
                playground.run(&format!("T.apply(T.accents[{name:?}])"));
            }
            if let Some(page) = &page {
                playground.show_page(page);
            }
            playground.idle(40);
            for expr in a.values("--click") {
                playground.click(expr);
                playground.idle(2);
            }
            playground.hover(playground.center("primary"));
            if a.has("--dialog") {
                playground.click("dialog_button");
                playground.idle(2);
            }
            let out = a.value("--out").unwrap_or("ui-playground.png");
            let [w, h] = pixels(scale);
            cli::write_file(out, playground.render([w, h]).to_png_bytes()?)?;
            println!("wrote {out} ({w}x{h})");
        }
        "smoke" => {
            if let Some(page) = &page {
                playground.show_page(page);
            }
            let mut options = window("UI Playground (smoke)", 2);
            options.hidden = true;
            options.max_frames = Some(a.get("--frames", 5)?);
            let run = run_ui_windowed(&mut playground, &options)?;
            let image = run.last_frame.ok_or("no frame was read back")?;
            // The app background is the clear color's neighbour, the title is
            // drawn in the primary orange: both must be on screen.
            let count = |f: &dyn Fn(&[u8]) -> bool| image.rgba.chunks(4).filter(|p| f(p)).count();
            let orange = count(&|p| p[0] > 200 && (100..180).contains(&p[1]) && p[2] < 80);
            let dark = count(&|p| p[..3] == CLEAR[..3]);
            let magenta = count(&|p| p[0] > 240 && p[1] < 10 && p[2] > 240);
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
    cli::exit(run())
}
