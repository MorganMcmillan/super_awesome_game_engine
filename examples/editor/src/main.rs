//! ```text
//! editor play   [--project DIR]            open the editor in a window
//! editor render [--project DIR] [--out FILE.png] [--tab SCENE|CODE|BLOCKS]
//!               [--select SID] [--click EXPR]...   draw a screenshot (CPU render)
//! editor smoke  [--frames N]               open a hidden window, draw, check
//! ```
//! `--project DIR` opens every `.py` file in DIR and its `scene.sage`
//! (without it, a small sample game). Ctrl+S saves; Ctrl+Q quits.

use editor::{Editor, SCREEN, open_dir, sample_project};
use sage_platform::WindowOptions;
use sage_platform::ui_window::run_ui_windowed;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("editor: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "play".into());
    let (mut project, mut out, mut tab, mut select, mut frames) =
        (None, "editor.png".to_string(), None, None, 5u64);
    let mut clicks = vec![];
    while let Some(flag) = args.next() {
        let mut value = |what: &str| args.next().ok_or(format!("{flag} needs {what}"));
        match flag.as_str() {
            "--project" => project = Some(std::path::PathBuf::from(value("a directory")?)),
            "--out" => out = value("a file")?,
            "--tab" => tab = Some(value("a tab name")?),
            "--select" => {
                select = Some(
                    value("a StableId")?
                        .parse::<u64>()
                        .map_err(|e| e.to_string())?,
                )
            }
            "--click" => clicks.push(value("an expression")?),
            "--frames" => {
                frames = value("a number")?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    let proj = match &project {
        Some(d) => open_dir(d)?,
        None => sample_project()?,
    };
    let mut ed = Editor::new(proj, project)?;
    match cmd.as_str() {
        "play" => {
            let options =
                WindowOptions::new("Editor", (SCREEN[0] as u32 * 2, SCREEN[1] as u32 * 2));
            let started = std::time::Instant::now();
            let run = run_ui_windowed(&mut ed, &options)?;
            let secs = started.elapsed().as_secs_f64().max(1e-9);
            println!(
                "{} frames, {} ticks in {secs:.1}s: average {:.1} fps, {:.1} ticks/s",
                run.frames,
                run.ticks,
                run.frames as f64 / secs,
                run.ticks as f64 / secs
            );
            Ok(())
        }
        "smoke" => {
            // A real (hidden) GPU window: draw the editor, read the frame back
            // and check what a person would see is on it.
            if let Some(t) = &tab {
                let i = ["SCENE", "CODE", "BLOCKS", "TIMING", "PREFABS"]
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(t))
                    .ok_or("tab is SCENE, CODE, BLOCKS, TIMING or PREFABS")?;
                ed.click(&format!("tab_buttons[{i}]"));
            }
            let mut options = WindowOptions::new(
                "Editor (smoke)",
                (SCREEN[0] as u32 * 2, SCREEN[1] as u32 * 2),
            );
            options.hidden = true;
            options.max_frames = Some(frames);
            let started = std::time::Instant::now();
            let run = run_ui_windowed(&mut ed, &options)?;
            let secs = started.elapsed().as_secs_f64().max(1e-9);
            println!(
                "average {:.1} fps over {} frames",
                run.frames as f64 / secs,
                run.frames
            );
            let image = run.last_frame.ok_or("no frame was read back")?;
            let (mut orange, mut dark, mut yellow, mut other) = (0usize, 0usize, 0usize, 0usize);
            for y in 0..image.height {
                for x in 0..image.width {
                    let p = image.pixel(x, y);
                    if p[0] > 200 && (100..180).contains(&p[1]) && p[2] < 80 {
                        orange += 1;
                    } else if p[0] > 220 && p[1] > 200 && p[2] < 160 {
                        yellow += 1;
                    } else if p[..3] == [15, 13, 11] {
                        dark += 1;
                    } else {
                        other += 1;
                    }
                }
            }
            std::fs::write(&out, image.to_png_bytes()?).map_err(|e| e.to_string())?;
            println!(
                "{} frames, {} ticks, {}x{}: {orange} orange, {yellow} sticky-note yellow, {dark} background, {other} other; wrote {out}",
                run.frames, run.ticks, image.width, image.height
            );
            if orange == 0 || dark == 0 {
                return Err("the editor's colors are not on the frame".into());
            }
            Ok(())
        }
        "render" => {
            if let Some(s) = select {
                ed.project(|p| p.doc.select(Some(s)));
            }
            ed.idle(2);
            if let Some(t) = tab {
                let i = ["SCENE", "CODE", "BLOCKS"]
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(&t))
                    .ok_or("tab is SCENE, CODE or BLOCKS")?;
                ed.click(&format!("tab_buttons[{i}]"));
            }
            for c in &clicks {
                ed.click(c);
            }
            ed.idle(3);
            let size = [SCREEN[0] as u32 * 2, SCREEN[1] as u32 * 2];
            let img = ed.render(size);
            std::fs::write(&out, img.to_png_bytes()?).map_err(|e| e.to_string())?;
            println!("wrote {out}");
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}
