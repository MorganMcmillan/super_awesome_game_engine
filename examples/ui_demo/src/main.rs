//! ```text
//! ui_demo render [--out FILE.png] [--scale N]   play a session, write a frame
//! ```

use std::process::ExitCode;

use sage_app::UiHosted;
use sage_app::cli::{self, Args};
use ui_demo::{Demo, SCREEN};

fn run() -> Result<(), String> {
    let a = Args::from_env("render", &["--out", "--scale"], &[])?;
    if a.cmd != "render" {
        return Err("usage: ui_demo render [--out FILE.png] [--scale N]".into());
    }
    let out = a.value("--out").unwrap_or("ui-demo.png");
    let scale: u32 = a.get("--scale", 4)?;
    let demo = Demo::new()?;
    demo.play_session();
    let size = [SCREEN[0] as u32 * scale, SCREEN[1] as u32 * scale];
    cli::write_file(out, demo.render(size).to_png_bytes()?)?;
    println!(
        "wrote {out} ({}x{}), status: {}",
        size[0],
        size[1],
        demo.status()
    );
    Ok(())
}

fn main() -> ExitCode {
    cli::exit(run())
}
