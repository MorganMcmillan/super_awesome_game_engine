//! ```text
//! editor play   [--project DIR]            open the editor in a window
//! editor render [--project DIR] [--out FILE.png] [--tab Scene|Code|Blocks|Timing|Prefabs]
//!               [--select SID] [--click EXPR]...   draw a screenshot (CPU render)
//! editor smoke  [--frames N] [--tab NAME] [--out FILE.png]   open a hidden window, draw, check
//! ```
//! `--project DIR` opens every `.py` file in DIR and its `scene.sage`
//! (without it, a small sample game). Ctrl+S saves; Ctrl+Q quits.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use editor::{CLEAR, Editor, SCREEN, no_prepare, sample_project};
use sage_app::cli::{self, Args};
use sage_app::{UiHosted, run_ui_windowed};
use sage_editor::Project;
use sage_platform::WindowOptions;

const TABS: [&str; 5] = ["Scene", "Code", "Blocks", "Timing", "Prefabs"];

/// The window's size in pixels: the screen at scale 2.
const PIXELS: [u32; 2] = [SCREEN[0] as u32 * 2, SCREEN[1] as u32 * 2];

fn main() -> ExitCode {
    cli::exit(run())
}

/// Clicks the tab called `name` (in any case).
fn open_tab(ed: &Editor, name: &str) -> Result<(), String> {
    let i = TABS
        .iter()
        .position(|t| t.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("--tab is one of {}", TABS.join(", ")))?;
    ed.click(&format!("tab_buttons[{i}]"));
    Ok(())
}

fn run() -> Result<(), String> {
    let a = Args::from_env(
        "play",
        &["--project", "--out", "--tab", "--select", "--click", "--frames"],
        &[],
    )?;
    let dir = a.value("--project").map(PathBuf::from);
    let project = match &dir {
        Some(d) => Project::open_dir(d, no_prepare())?,
        None => sample_project()?,
    };
    let mut ed = Editor::new(project, dir)?;
    let out = a.value("--out").unwrap_or("editor.png");
    match a.cmd.as_str() {
        "play" => {
            let options = WindowOptions::new("Editor", (PIXELS[0], PIXELS[1]));
            let started = Instant::now();
            let run = run_ui_windowed(&mut ed, &options)?;
            let secs = started.elapsed().as_secs_f64().max(1e-9);
            println!(
                "{} frames, {} ticks in {secs:.1}s: average {:.1} fps, {:.1} ticks/s",
                run.frames,
                run.ticks,
                run.frames as f64 / secs,
                run.ticks as f64 / secs
            );
        }
        "smoke" => {
            // A real (hidden) GPU window: draw the editor, read the frame back
            // and check what a person would see is on it.
            if let Some(tab) = a.value("--tab") {
                open_tab(&ed, tab)?;
            }
            let mut options = WindowOptions::new("Editor (smoke)", (PIXELS[0], PIXELS[1]));
            options.hidden = true;
            options.max_frames = Some(a.get("--frames", 5)?);
            let started = Instant::now();
            let run = run_ui_windowed(&mut ed, &options)?;
            let secs = started.elapsed().as_secs_f64().max(1e-9);
            println!(
                "average {:.1} fps over {} frames",
                run.frames as f64 / secs,
                run.frames
            );
            let image = run.last_frame.ok_or("no frame was read back")?;
            let count = |f: &dyn Fn(&[u8]) -> bool| image.rgba.chunks(4).filter(|p| f(p)).count();
            let orange = count(&|p| p[0] > 200 && (100..180).contains(&p[1]) && p[2] < 80);
            let yellow = count(&|p| p[0] > 220 && p[1] > 200 && p[2] < 160);
            let dark = count(&|p| p[..3] == CLEAR[..3]);
            cli::write_file(out, image.to_png_bytes()?)?;
            println!(
                "{} frames, {} ticks, {}x{}: {orange} orange, {yellow} sticky-note yellow, {dark} background; wrote {out}",
                run.frames, run.ticks, image.width, image.height
            );
            if orange == 0 || dark == 0 {
                return Err("the editor's colors are not on the frame".into());
            }
        }
        "render" => {
            if let Some(sid) = a.value("--select") {
                let sid: u64 = sid.parse().map_err(|_| format!("bad --select '{sid}'"))?;
                ed.project(|p| p.doc.select(Some(sid)));
            }
            ed.idle(2);
            if let Some(tab) = a.value("--tab") {
                open_tab(&ed, tab)?;
            }
            for expr in a.values("--click") {
                ed.click(expr);
            }
            ed.idle(8);
            cli::write_file(out, ed.render(PIXELS).to_png_bytes()?)?;
            println!("wrote {out}");
        }
        other => return Err(format!("unknown command {other}")),
    }
    Ok(())
}
