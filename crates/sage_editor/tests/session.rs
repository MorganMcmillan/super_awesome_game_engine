//! A session without a screen: requests, the host's part of a tick, the
//! scene viewport's pointer, and a project's files.

use sage_editor::viewport::{pick, scene_frame};
use sage_editor::{Mode, Project, Request, ScenePointer, Session};

const GAME: &str = r#"
import sage
from sage import component

@component
class Position:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0

@component
class Size:
    hx: sage.f32 = 4.0
    hy: sage.f32 = 4.0

@component
class Velocity:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0

@world.system("move", query="Position, Velocity")
def move(pos: Position, vel: Velocity):
    pos.x += vel.x

world.entity("hero", 1).set(Position(20, 30)).set(Size(5, 5)).set(Velocity(1, 0))
world.entity("rock", 2).set(Position(90, 40)).set(Size(8, 6))
world.entity("ghost", 3).set(Size(1, 1))
"#;

fn session() -> Session {
    let project = Project::open(
        "test",
        vec![("game".into(), GAME.into())],
        None,
        Box::new(|_, _| Ok(())),
    )
    .unwrap();
    Session::new(project, None)
}

fn x_of(s: &Session, sid: u64) -> String {
    let e = s.project.doc.inspect(sid).unwrap();
    let c = e.components.iter().find(|c| c.name == "Position").unwrap();
    c.fields.iter().find(|f| f.name == "x").unwrap().text.clone()
}

fn pointer(at: [f32; 2], down: bool) -> Option<ScenePointer> {
    Some(ScenePointer {
        at,
        over: true,
        down,
    })
}

#[test]
fn requests_have_names_scripts_use_and_unknown_ones_are_refused() {
    for r in Request::ALL {
        assert_eq!(Request::parse(r.name()), Some(r));
    }
    assert_eq!(Request::parse("explode"), None);
}

#[test]
fn play_ticks_the_game_pause_holds_it_step_advances_it_and_stop_puts_it_back() {
    let mut s = session();
    let authored = s.project.doc.scene.write();
    let seen = s.revision();
    s.request(Request::Play);
    s.tick(None);
    assert_eq!(s.project.mode, Mode::Play);
    assert_eq!(s.log, "playing");
    assert!(s.revision() > seen, "the views are told play started");
    let after_play = s.project.tick;
    s.tick(None);
    assert_eq!(s.project.tick, after_play + 1, "a playing game ticks");

    s.request(Request::Pause);
    s.tick(None);
    assert!(s.paused);
    let held = s.project.tick;
    s.tick(None);
    assert_eq!(s.project.tick, held, "paused: no ticks");
    s.request(Request::Step);
    s.tick(None);
    assert_eq!(s.project.tick, held + 1, "step advances a held game by one");

    s.request(Request::Stop);
    s.tick(None);
    assert_eq!(s.project.mode, Mode::Edit);
    assert!(!s.paused, "stopping also resumes");
    assert_eq!(s.project.doc.scene.write(), authored);
    assert_eq!(s.project.live_scene().write(), authored, "the world is back");
}

#[test]
fn step_from_edit_mode_starts_the_game_held_one_tick_in() {
    let mut s = session();
    s.request(Request::Step);
    s.tick(None);
    assert_eq!(s.project.mode, Mode::Play);
    assert_eq!(s.log, "stepped");
    assert!(s.paused, "a stepped game is held, it does not run on");
    assert_eq!(s.project.tick, 1);
    s.tick(None);
    s.tick(None);
    assert_eq!(s.project.tick, 1);
    assert!(s.project.live_scene().write().contains("x=21.0 y=30.0"));
    // Pause resumes it.
    s.request(Request::Pause);
    s.tick(None);
    assert!(!s.paused);
    assert_eq!(s.project.tick, 2);
}

#[test]
fn a_changed_script_reloads_on_the_next_tick_and_an_error_is_kept_for_the_views() {
    let mut s = session();
    s.project
        .set_source("game", &GAME.replace("pos.x += vel.x", "pos.x += 2 * vel.x"))
        .unwrap();
    assert!(s.project.stale);
    s.tick(None);
    assert!(!s.project.stale, "live reload");
    assert_eq!(s.project.error, None);

    s.project.set_source("game", "this is not python").unwrap();
    s.tick(None);
    assert!(s.project.error.is_some());
    assert!(s.project.stale, "the old world stays until the script works");
    // A broken script is not retried every tick.
    let seen = s.revision();
    s.tick(None);
    assert_eq!(s.revision(), seen);
}

#[test]
fn the_viewport_draws_positioned_entities_and_marks_the_selected_one() {
    use sage_core::host::DrawKind;
    let mut s = session();
    s.project.doc.select(Some(2));
    let frame = s.frame();
    // The court, then the two entities that have a position (not the ghost).
    let kinds: Vec<_> = frame.rects.iter().map(|r| (r.id.0, r.kind)).collect();
    assert_eq!(
        kinds,
        [(0, DrawKind::Court), (1, DrawKind::Wall), (2, DrawKind::Paddle)]
    );
    assert_eq!(frame.rects[2].half, [8.0, 6.0]);
    // The world grows to hold everything, with room around it.
    assert!(frame.world_size[0] >= 160.0 && frame.world_size[1] >= 90.0);
    let scene = &s.project.doc.scene;
    assert_eq!(scene_frame(scene, None, 0).rects[2].kind, DrawKind::Wall);
    assert_eq!(pick(scene, [96.0, 44.0]).map(|h| h.0), Some(2));
    assert_eq!(pick(scene, [150.0, 80.0]), None);
}

#[test]
fn a_click_selects_and_a_drag_moves_in_whole_units_as_one_undo_step() {
    let mut s = session();
    // Press on the hero, a little off its center, and drag.
    s.tick(pointer([21.0, 31.0], true));
    assert_eq!(s.project.doc.selection, Some(1));
    s.tick(pointer([40.3, 31.0], true));
    s.tick(pointer([70.6, 31.0], true));
    s.tick(pointer([70.6, 31.0], false));
    // It kept the offset it was picked up with, and landed on a whole unit.
    assert_eq!(x_of(&s, 1), "70.0");
    assert_eq!(s.project.doc.history.labels().len(), 1, "one step");
    s.project.undo();
    assert_eq!(x_of(&s, 1), "20.0");

    // Without snapping it lands where the pointer put it, to a tenth.
    s.scene_view.snap = 0.0;
    s.tick(pointer([21.0, 31.0], true));
    s.tick(pointer([40.34, 31.0], true));
    s.tick(pointer([40.34, 31.0], false));
    assert_eq!(x_of(&s, 1), "39.3");

    // A press on nothing clears the selection.
    s.tick(pointer([150.0, 80.0], true));
    s.tick(pointer([150.0, 80.0], false));
    assert_eq!(s.project.doc.selection, None);
}

#[test]
fn while_playing_the_viewport_looks_but_does_not_edit() {
    let mut s = session();
    s.request(Request::Play);
    s.tick(None);
    let before = s.project.doc.scene.write();
    s.tick(pointer([21.0, 31.0], true));
    s.tick(pointer([60.0, 31.0], true));
    s.tick(pointer([60.0, 31.0], false));
    assert_eq!(s.project.doc.scene.write(), before);
    assert_eq!(s.project.doc.selection, None);
}

#[test]
fn while_playing_the_session_shows_the_running_world_and_a_value_can_be_kept() {
    let mut s = session();
    assert!(s.live().is_none());
    assert!(s.keep(1, "game.Position", "x").is_err(), "nothing runs yet");
    s.request(Request::Play);
    s.tick(None);
    s.tick(None);
    // The hero moves one unit a tick; the authored scene does not.
    let live_x = |s: &Session| {
        let e = sage_editor::document::inspect(s.live().unwrap(), 1).unwrap();
        e.components.iter().find(|c| c.name == "Position").unwrap().fields[0].text.clone()
    };
    assert_eq!(live_x(&s), "22.0");
    assert_eq!(x_of(&s, 1), "20.0");
    assert_eq!(s.frame().rects[1].center[0], 22.0, "the viewport shows the game");

    // Keep it: the authored value becomes the runtime one, as one undo step.
    s.keep(1, "game.Position", "x").unwrap();
    assert_eq!(x_of(&s, 1), "22.0");
    assert!(s.keep(1, "game.Position", "nope").is_err());
    assert!(s.keep(9, "game.Position", "x").is_err());

    s.request(Request::Stop);
    s.tick(None);
    s.tick(None);
    assert!(s.live().is_none());
    assert_eq!(x_of(&s, 1), "22.0", "what was kept survives stopping");
    assert!(s.project.live_scene().write().contains("x=22.0 y=30.0"));
    assert_eq!(s.project.doc.history.labels(), ["Set Position.x"]);
    s.project.undo();
    assert_eq!(x_of(&s, 1), "20.0");
}

#[test]
fn entities_the_game_makes_are_listed_as_runtime_only() {
    let spawning = format!(
        "{GAME}\n@world.system(\"spawn\", query=\"Velocity\")\ndef spawn(vel: Velocity):\n    if world.lookup(50) is None:\n        world.entity(\"spark\", 50).set(Position(1, 1))\n"
    );
    let project = Project::open(
        "test",
        vec![("game".into(), spawning)],
        None,
        Box::new(|_, _| Ok(())),
    )
    .unwrap();
    let mut s = Session::new(project, None);
    assert!(s.runtime_only().is_empty());
    s.request(Request::Play);
    s.tick(None);
    s.tick(None);
    let rows = s.runtime_only();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!((rows[0].sid, rows[0].label.as_str()), (50, "spark #50"));
    s.request(Request::Stop);
    s.tick(None);
    assert!(s.runtime_only().is_empty());
    assert!(!s.project.live_scene().write().contains("spark"));
}

#[test]
fn a_project_saves_to_a_directory_and_opens_from_it_again() {
    let dir = std::env::temp_dir().join(format!("sage-session-{}", std::process::id()));
    let project = Project::open(
        "test",
        vec![("game".into(), GAME.into())],
        None,
        Box::new(|_, _| Ok(())),
    )
    .unwrap();
    let mut s = Session::new(project, Some(dir.clone()));
    s.project.doc.set_field(1, "game.Position", "x", "55").unwrap();
    s.project.settings.insert("suggest_by_type".into(), "off".into());
    assert!(s.project.doc.dirty());
    s.request(Request::Save);
    s.tick(None);
    assert_eq!(s.log, format!("saved to {}", dir.display()));
    assert!(!s.project.doc.dirty());

    let again = Project::open_dir(&dir, Box::new(|_, _| Ok(()))).unwrap();
    assert_eq!(again.source("game"), Some(GAME));
    assert_eq!(again.doc.scene.write(), s.project.doc.scene.write());
    assert_eq!(again.settings.get("suggest_by_type").map(String::as_str), Some("off"));
    assert_eq!(again.name, dir.file_name().unwrap().to_string_lossy());

    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let why = Project::open_dir(&empty, Box::new(|_, _| Ok(()))).err().unwrap();
    assert!(why.contains("has no .py files"), "{why}");
    std::fs::remove_dir_all(&dir).unwrap();
}
