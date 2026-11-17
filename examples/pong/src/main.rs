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
use sage_app::cli::{self, Args};
use sage_assets::{AssetStore, generated};
use sage_core::host::DrawKind;
use sage_core::{InputRecording, Simulation};
use sage_platform::{
    Binding, InputMap, Key, PadAxis, PadButton, RendererChoice, WindowOptions, run_windowed,
};

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

fn renderer(name: &str) -> Result<RendererChoice, String> {
    match name {
        "gpu" => Ok(RendererChoice::Gpu),
        "sdl" => Ok(RendererChoice::Sdl),
        "auto" => Ok(RendererChoice::Auto),
        other => Err(format!("unknown renderer '{other}' (gpu, sdl, auto)")),
    }
}

fn run() -> Result<(), String> {
    let a = Args::from_env(
        "play",
        &["--seed", "--record", "--hashes", "--frames", "--renderer"],
        &["--mute"],
    )?;
    let seed: u64 = a.get("--seed", 1)?;
    match a.cmd.as_str() {
        "play" | "smoke" => {
            let smoke = a.cmd == "smoke";
            let mut sim = Shown::new(Pong::new(PongConfig::new(seed))?);
            // The committed assets, through the asset store: the ball's
            // sprite, and every sound as a cue named after its file.
            let mut store = AssetStore::new(pong::assets::DIR);
            let (_, ball) = store.load_image("ball.png")?;
            let mut options =
                WindowOptions::new(format!("{} pong (python)", sage_core::CODENAME), (960, 600));
            options.max_frames = smoke.then_some(a.get("--frames", 120)?);
            options.renderer = renderer(a.value("--renderer").unwrap_or("auto"))?;
            options.sprites = vec![(DrawKind::Ball, (*ball).clone())];
            options.sounds = store.load_cues()?;
            options.audio = !a.has("--mute");
            let run = run_windowed(&mut sim, seed, &input_map(), &options)?;
            if let Some(path) = a.value("--record") {
                cli::write_file(path, run.recording.to_text())?;
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
                    if !px.iter().any(|(k, n)| *k == kind && *n > 0) {
                        return Err(format!("smoke: nothing drawn for {kind:?}"));
                    }
                }
                println!("smoke: ok");
            }
        }
        "headless" => {
            let mut pong = Pong::new(PongConfig::new(seed))?;
            let mut rec = InputRecording::new(seed);
            while pong.state() != GameState::GameOver {
                if pong.tick() > 60 * 60 * 10 {
                    return Err(format!("game did not finish: {}", summary(&pong)));
                }
                let input = pong.bot(Side::Left);
                rec.record(pong.tick(), input);
                pong.step(input);
            }
            if let Some(path) = a.value("--record") {
                cli::write_file(path, rec.to_text())?;
            }
            println!("{}", summary(&pong));
        }
        "replay" => {
            let pong = cli::replay(a.file.as_deref(), a.value("--hashes"), |seed| {
                Pong::new(PongConfig::new(seed))
            })?;
            println!("{}", summary(&pong));
        }
        "gen-assets" => {
            for line in generated::write_all(pong::assets::DIR, &pong::assets::generate()?)? {
                println!("{line}");
            }
        }
        other => {
            return Err(format!(
                "unknown command '{other}' (play, headless, replay, smoke, gen-assets)"
            ));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    cli::exit(run())
}
