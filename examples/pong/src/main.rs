//! Pong written in Python (scripts/pong.py).
//!
//! ```text
//! pong_py play     [--seed N] [--record FILE]   windowed; W/S and Up/Down; Esc quits
//! pong_py headless [--seed N] [--record FILE]   scripted game: left bot vs idle right
//! pong_py replay   FILE [--hashes OUT]          replay a recording headless
//! pong_py smoke    [--frames N]                 open a window, draw, check the court
//! ```
//! Exit code is non-zero on any failure, so every command is script-checkable.

use std::process::ExitCode;

use pong_py::{GameState, LEFT_DOWN, LEFT_UP, PongPy, PongPyConfig, RIGHT_DOWN, RIGHT_UP, Side};
use sage_core::host::DrawKind;
use sage_core::{InputRecording, Simulation, run_recording};
use sage_platform::{InputMap, Key, Presentable, WindowOptions, run_windowed};

struct Args {
    cmd: String,
    file: Option<String>,
    seed: u64,
    record: Option<String>,
    hashes: Option<String>,
    frames: u64,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        cmd: it.next().unwrap_or_else(|| "play".into()),
        file: None,
        seed: 1,
        record: None,
        hashes: None,
        frames: 120,
    };
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--seed" => a.seed = value("--seed")?.parse().map_err(|_| "bad --seed")?,
            "--record" => a.record = Some(value("--record")?),
            "--hashes" => a.hashes = Some(value("--hashes")?),
            "--frames" => a.frames = value("--frames")?.parse().map_err(|_| "bad --frames")?,
            s if !s.starts_with("--") && a.file.is_none() => a.file = Some(s.to_string()),
            s => return Err(format!("unknown argument '{s}'")),
        }
    }
    Ok(a)
}

struct Shown(PongPy);

impl Simulation for Shown {
    fn step(&mut self, input: sage_core::TickInput) {
        self.0.step(input)
    }
    fn tick(&self) -> u64 {
        self.0.tick()
    }
    fn state_hash(&self) -> u64 {
        self.0.state_hash()
    }
}

impl Presentable for Shown {
    fn frame(&self) -> sage_core::host::Frame {
        self.0.frame()
    }
}

fn input_map() -> InputMap {
    InputMap::default()
        .bind(Key::W, LEFT_UP)
        .bind(Key::S, LEFT_DOWN)
        .bind(Key::Up, RIGHT_UP)
        .bind(Key::Down, RIGHT_DOWN)
}

fn summary(p: &PongPy) -> String {
    let [l, r] = p.points();
    format!(
        "tick={} state={} score={l}:{r} winner={} hash={:#018x}",
        p.tick(),
        p.state().name(),
        p.winner().map_or("none", Side::name),
        p.state_hash()
    )
}

fn write(path: &str, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
}

fn run(a: Args) -> Result<(), String> {
    match a.cmd.as_str() {
        "play" | "smoke" => {
            let smoke = a.cmd == "smoke";
            let mut sim = Shown(PongPy::new(PongPyConfig::new(a.seed))?);
            let run = run_windowed(
                &mut sim,
                a.seed,
                &input_map(),
                &WindowOptions {
                    title: format!("{} pong (python)", sage_core::CODENAME),
                    size: (960, 600),
                    max_frames: smoke.then_some(a.frames),
                    hidden: false,
                },
            )?;
            if let Some(path) = &a.record {
                write(path, &run.recording.to_text())?;
            }
            println!("frames={} {}", run.frames, summary(&sim.0));
            if smoke {
                let px = run.last_frame_pixels.unwrap_or_default();
                println!("pixels: {px:?}");
                for kind in [
                    DrawKind::Court,
                    DrawKind::Wall,
                    DrawKind::Paddle,
                    DrawKind::Ball,
                ] {
                    let n = px.iter().find(|(k, _)| *k == kind).map_or(0, |(_, n)| *n);
                    if n == 0 {
                        return Err(format!("smoke: nothing drawn for {kind:?}"));
                    }
                }
                println!("smoke: ok");
            }
            Ok(())
        }
        "headless" => {
            let mut pong = PongPy::new(PongPyConfig::new(a.seed))?;
            let mut rec = InputRecording::new(a.seed);
            while pong.state() != GameState::GameOver {
                if pong.tick() > 60 * 60 * 10 {
                    return Err(format!("game did not finish: {}", summary(&pong)));
                }
                let input = pong.bot(Side::Left);
                rec.record(pong.tick(), input);
                pong.step(input);
            }
            if let Some(path) = &a.record {
                write(path, &rec.to_text())?;
            }
            println!("{}", summary(&pong));
            Ok(())
        }
        "replay" => {
            let path = a.file.ok_or("replay needs a recording file")?;
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            let rec = InputRecording::from_text(&text).map_err(|e| e.to_string())?;
            let mut pong = PongPy::new(PongPyConfig::new(rec.seed))?;
            let trace = run_recording(&mut pong, &rec);
            if let Some(out) = &a.hashes {
                let mut s = format!("initial {:#018x}\n", trace.initial);
                for (i, h) in trace.hashes.iter().enumerate() {
                    s.push_str(&format!("{} {h:#018x}\n", i + 1));
                }
                write(out, &s)?;
            }
            println!("{}", summary(&pong));
            Ok(())
        }
        other => Err(format!(
            "unknown command '{other}' (play, headless, replay, smoke)"
        )),
    }
}

fn main() -> ExitCode {
    match parse().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
