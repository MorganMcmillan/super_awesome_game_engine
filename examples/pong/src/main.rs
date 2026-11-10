//! Pong written in Python (scripts/pong.py).
//!
//! ```text
//! pong play     [--seed N] [--record FILE]   windowed; W/S and Up/Down; Esc quits
//! pong headless [--seed N] [--record FILE]   scripted game: left bot vs idle right
//! pong replay   FILE [--hashes OUT]          replay a recording headless
//! pong smoke    [--frames N]                 open a window, draw, check the court
//! pong gen-assets                            rewrite examples/pong/assets/
//! ```
//! `play` and `smoke` take `--renderer gpu|sdl|auto` (default auto) and `--mute`.
//! Exit code is non-zero on any failure, so every command is script-checkable.

use std::process::ExitCode;

use pong::{GameState, LEFT_DOWN, LEFT_UP, Pong, PongConfig, RIGHT_DOWN, RIGHT_UP, Shown, Side};
use sage_assets::{AssetKind, AssetStore, Image, Sound};
use sage_core::host::DrawKind;
use sage_core::{InputRecording, Simulation, run_recording};
use sage_platform::{
    Binding, InputMap, Key, PadAxis, PadButton, RendererChoice, WindowOptions, run_windowed,
};

struct Args {
    cmd: String,
    file: Option<String>,
    seed: u64,
    record: Option<String>,
    hashes: Option<String>,
    frames: u64,
    renderer: RendererChoice,
    mute: bool,
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
        renderer: RendererChoice::Auto,
        mute: false,
    };
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--seed" => a.seed = value("--seed")?.parse().map_err(|_| "bad --seed")?,
            "--record" => a.record = Some(value("--record")?),
            "--hashes" => a.hashes = Some(value("--hashes")?),
            "--frames" => a.frames = value("--frames")?.parse().map_err(|_| "bad --frames")?,
            "--renderer" => {
                a.renderer = match value("--renderer")?.as_str() {
                    "gpu" => RendererChoice::Gpu,
                    "sdl" => RendererChoice::Sdl,
                    "auto" => RendererChoice::Auto,
                    other => return Err(format!("unknown renderer '{other}' (gpu, sdl, auto)")),
                }
            }
            "--mute" => a.mute = true,
            s if !s.starts_with("--") && a.file.is_none() => a.file = Some(s.to_string()),
            s => return Err(format!("unknown argument '{s}'")),
        }
    }
    Ok(a)
}

fn input_map() -> InputMap {
    InputMap::default()
        .bind(Key::W, LEFT_UP)
        .bind(Key::S, LEFT_DOWN)
        .bind(Key::Up, RIGHT_UP)
        .bind(Key::Down, RIGHT_DOWN)
        // The left stick and d-pad of any gamepad drive the left paddle.
        .bind_input(Binding::PadButton(PadButton::DPadUp), LEFT_UP)
        .bind_input(Binding::PadButton(PadButton::DPadDown), LEFT_DOWN)
        .bind_input(
            Binding::PadAxis {
                axis: PadAxis::LeftY,
                positive: false,
            },
            LEFT_UP,
        )
        .bind_input(
            Binding::PadAxis {
                axis: PadAxis::LeftY,
                positive: true,
            },
            LEFT_DOWN,
        )
}

type LoadedAssets = (Vec<(DrawKind, Image)>, Vec<(String, std::rc::Rc<Sound>)>);

/// Loads the committed assets through the asset store: the ball sprite, and
/// every WAV as a cue named after its file.
fn load_assets() -> Result<LoadedAssets, String> {
    let mut store = AssetStore::new(pong::assets::DIR);
    let mut sounds = vec![];
    for entry in store.catalog()? {
        if entry.kind == AssetKind::Sound {
            let (_, sound) = store.load_sound(&entry.path)?;
            let cue = entry.path.trim_end_matches(".wav").to_string();
            sounds.push((cue, sound));
        }
    }
    let (_, ball) = store.load_image("ball.png")?;
    Ok((vec![(DrawKind::Ball, (*ball).clone())], sounds))
}

fn summary(p: &Pong) -> String {
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
            let mut sim = Shown::new(Pong::new(PongConfig::new(a.seed))?);
            let (sprites, sounds) = load_assets()?;
            let mut options =
                WindowOptions::new(format!("{} pong (python)", sage_core::CODENAME), (960, 600));
            options.max_frames = smoke.then_some(a.frames);
            options.renderer = a.renderer;
            options.sprites = sprites;
            options.sounds = sounds;
            options.audio = !a.mute;
            let run = run_windowed(&mut sim, a.seed, &input_map(), &options)?;
            if let Some(path) = &a.record {
                write(path, &run.recording.to_text())?;
            }
            println!(
                "frames={} renderer={} audio={} cues={}/{} {}",
                run.frames,
                run.renderer,
                run.audio,
                run.cues_played,
                run.cues_emitted,
                summary(&sim.pong)
            );
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
            let mut pong = Pong::new(PongConfig::new(a.seed))?;
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
            let mut pong = Pong::new(PongConfig::new(rec.seed))?;
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
        "gen-assets" => {
            std::fs::create_dir_all(pong::assets::DIR).map_err(|e| e.to_string())?;
            for (name, bytes) in pong::assets::generate()? {
                let path = format!("{}/{name}", pong::assets::DIR);
                std::fs::write(&path, &bytes).map_err(|e| format!("{path}: {e}"))?;
                println!("{name}: {} bytes", bytes.len());
            }
            Ok(())
        }
        other => Err(format!(
            "unknown command '{other}' (play, headless, replay, smoke, gen-assets)"
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
