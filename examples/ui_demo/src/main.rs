//! ```text
//! ui_demo render [--out FILE.png] [--scale N]   play a session, write a frame
//! ```

use std::process::ExitCode;

use ui_demo::{Demo, SCREEN};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("render") {
        eprintln!("usage: ui_demo render [--out FILE.png] [--scale N]");
        return ExitCode::from(2);
    }
    let (mut out, mut scale) = ("ui-demo.png".to_string(), 4u32);
    while let Some(a) = args.next() {
        match (a.as_str(), args.next()) {
            ("--out", Some(v)) => out = v,
            ("--scale", Some(v)) => scale = v.parse().unwrap_or(4),
            _ => {
                eprintln!("usage: ui_demo render [--out FILE.png] [--scale N]");
                return ExitCode::from(2);
            }
        }
    }
    let result = (|| -> Result<(), String> {
        let demo = Demo::new()?;
        demo.play_session();
        let size = [SCREEN[0] as u32 * scale, SCREEN[1] as u32 * scale];
        let png = demo.render(size).to_png_bytes()?;
        std::fs::write(&out, png).map_err(|e| format!("{out}: {e}"))?;
        println!(
            "wrote {out} ({}x{}), status: {}",
            size[0],
            size[1],
            demo.status()
        );
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
