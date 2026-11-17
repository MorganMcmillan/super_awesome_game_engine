//! The platformer, made from the platformer kit.
//!
//! ```text
//! platformer play     [--seed N] [--record FILE]   windowed; arrows or WASD, Space/Z jump,
//!                                                  Shift/X run, C spin, P pause, Esc quits
//! platformer headless [--seed N] [--record FILE]   the bot plays the level to the end
//! platformer replay   FILE [--hashes OUT]          replay a recording headless
//! platformer smoke    [--frames N]                 open a window, draw, check the picture
//! platformer render   --out FILE [--ticks N]       a PNG of the game after N bot ticks (CPU)
//! platformer gen-assets                            rewrite examples/platformer/assets/
//! ```
//! `play` and `smoke` take `--mute`. Exit code is non-zero on any failure, so
//! every command is script-checkable.

use std::process::ExitCode;

use platformer::{Bot, Game, assets, input_map, level_picture};
use sage_app::cli::{self, Args};
use sage_assets::{AssetStore, generated};
use sage_core::{InputRecording, Simulation};
use sage_platform::WindowOptions;
use sage_platform::game_window::{GameApp, render_frame, run_game_windowed};
use sage_platformer::Platformer;

fn summary(p: &Platformer) -> String {
    let (hp, max) = p.health();
    let pos = p.player_position();
    format!(
        "tick={} x={:.0} y={:.0} mode={} hp={hp}/{max} coins={} stomps={} deaths={} hash={:#018x}",
        p.tick(),
        pos.x,
        pos.y,
        p.mode_name(),
        p.coins(),
        p.stomps(),
        p.deaths(),
        p.state_hash()
    )
}

/// The bot plays until it reaches the right end of the level or `limit` ticks.
fn bot_game(seed: u64, limit: u64) -> Result<(Platformer, InputRecording, bool), String> {
    let mut p = Platformer::new(platformer::config(seed))?;
    let mut bot = Bot::new(level_picture());
    let goal = bot.size()[0] - 48.0;
    let mut rec = InputRecording::new(seed);
    let mut done = false;
    while p.tick() < limit {
        let input = bot.input(&p);
        rec.record(p.tick(), input);
        p.step(input);
        if p.player_position().x >= goal {
            done = true;
            break;
        }
    }
    Ok((p, rec, done))
}

fn run() -> Result<(), String> {
    let a = Args::from_env(
        "play",
        &["--seed", "--record", "--hashes", "--out", "--frames", "--ticks"],
        &["--mute"],
    )?;
    let seed: u64 = a.get("--seed", 1)?;
    match a.cmd.as_str() {
        "play" | "smoke" => {
            let smoke = a.cmd == "smoke";
            let mut game = Game::new(seed)?;
            let mut options =
                WindowOptions::new(format!("{} grassland", sage_core::CODENAME), (1280, 720));
            options.max_frames = smoke.then_some(a.get("--frames", 120)?);
            // Every WAV in the assets directory is a cue named after its file.
            options.sounds = AssetStore::new(assets::DIR).load_cues()?;
            options.audio = !a.has("--mute");
            let run = run_game_windowed(&mut game, seed, &input_map(), &options)?;
            if let Some(path) = a.value("--record") {
                cli::write_file(path, run.recording.to_text())?;
            }
            println!(
                "frames={} audio={} cues={}/{} {}",
                run.frames,
                run.audio,
                run.cues_played,
                run.cues_emitted,
                summary(&game.sim)
            );
            if smoke {
                let img = run.last_frame.ok_or("smoke: no frame was read back")?;
                let drawn = |c: [u8; 3]| img.rgba.chunks(4).any(|p| p[..3] == c && p[3] == 255);
                for (what, color) in [("the player", [220, 64, 56]), ("grass", [84, 172, 72])] {
                    if !drawn(color) {
                        return Err(format!("smoke: nothing drawn for {what}"));
                    }
                }
                println!("smoke: ok");
            }
        }
        "headless" => {
            let (p, rec, done) = bot_game(seed, 60 * 60 * 3)?;
            if let Some(path) = a.value("--record") {
                cli::write_file(path, rec.to_text())?;
            }
            println!("{}", summary(&p));
            if !done {
                return Err("the bot did not reach the end of the level".into());
            }
        }
        "replay" => {
            let p = cli::replay(a.file.as_deref(), a.value("--hashes"), |seed| {
                Platformer::new(platformer::config(seed))
            })?;
            println!("{}", summary(&p));
        }
        "render" => {
            let out = a.value("--out").ok_or("render needs --out FILE")?;
            let mut p = Platformer::new(platformer::config(seed))?;
            let mut bot = Bot::new(level_picture());
            for _ in 0..a.get("--ticks", 600u64)? {
                let input = bot.input(&p);
                p.step(input);
            }
            let game = Game::around(p);
            let img = render_frame(&game.frame(), &game.images(), [1280, 720]);
            cli::write_file(out, img.to_png_bytes()?)?;
            println!("{out}: {}", summary(&game.sim));
        }
        "gen-assets" => {
            for line in generated::write_all(assets::DIR, &assets::generate())? {
                println!("{line}");
            }
        }
        other => {
            return Err(format!(
                "unknown command '{other}' (play, headless, replay, smoke, render, gen-assets)"
            ));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    cli::exit(run())
}
