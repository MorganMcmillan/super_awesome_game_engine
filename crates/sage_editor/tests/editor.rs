//! The editor core: undo and redo, the inspector, play in editor, live
//! reload.

use sage_core::StateHasher;
use sage_editor::{Mode, Project};

const GAME: &str = r#"
import sage
from sage import component, Entity

@component
class Position:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0

@component
class Velocity:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0

@component
class Player:
    pass

@world.system("move", query="Position, Velocity")
def move(pos: Position, vel: Velocity):
    pos.x += vel.x
    pos.y += vel.y

world.entity("hero", 1).set(Position(1, 2)).set(Velocity(1, 0)).add(Player)
world.entity("rock", 2).set(Position(10, 10))
"#;

fn project() -> Project {
    Project::open(
        "test",
        vec![("game".into(), GAME.into())],
        None,
        Box::new(|_, _| Ok(())),
    )
    .unwrap()
}

fn hash(p: &Project) -> u64 {
    let mut h = StateHasher::new();
    p.ecs().hash_into(&mut h);
    h.finish()
}

fn field(p: &Project, sid: u64, comp: &str, field: &str) -> String {
    p.doc
        .inspect(sid)
        .unwrap()
        .components
        .iter()
        .find(|c| c.name == comp)
        .unwrap()
        .fields
        .iter()
        .find(|f| f.name == field)
        .unwrap()
        .text
        .clone()
}

#[test]
fn the_inspector_shows_every_component_field_from_reflection() {
    let p = project();
    let v = p.doc.inspect(1).unwrap();
    assert_eq!(v.name.as_deref(), Some("hero"));
    let names: Vec<_> = v.components.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Position", "Velocity"]);
    assert_eq!(v.tags.len(), 1);
    assert_eq!(field(&p, 1, "Position", "x"), "1.0");
    assert!(p.doc.addable(2).iter().any(|c| c.ends_with("Velocity")));
    assert!(!p.doc.addable(1).iter().any(|c| c.ends_with("Velocity")));
}

#[test]
fn edits_undo_and_redo_and_reach_the_live_world() {
    let mut p = project();
    let pos = p.doc.inspect(1).unwrap().components[0].path.clone();
    let before = p.doc.scene.write();
    p.doc.set_field(1, &pos, "x", "42").unwrap();
    p.sync().unwrap();
    assert_eq!(field(&p, 1, "Position", "x"), "42.0");
    assert!(p.live_scene().write().contains("x=42.0"));
    assert!(p.doc.dirty());
    p.doc.undo().unwrap();
    p.sync().unwrap();
    assert_eq!(p.doc.scene.write(), before);
    assert!(
        p.live_scene().write().contains("x=1.0 "),
        "{}",
        p.live_scene().write()
    );
    assert!(!p.doc.dirty());
    p.doc.redo().unwrap();
    assert_eq!(field(&p, 1, "Position", "x"), "42.0");
}

#[test]
fn bad_text_is_refused_and_changes_nothing() {
    let mut p = project();
    let pos = p.doc.inspect(1).unwrap().components[0].path.clone();
    let before = p.doc.scene.clone();
    assert!(p.doc.set_field(1, &pos, "x", "banana").is_err());
    assert!(p.doc.set_field(1, &pos, "x", "nan").is_err());
    assert!(p.doc.set_field(1, &pos, "nope", "1").is_err());
    assert!(p.doc.set_field(9, &pos, "x", "1").is_err());
    assert_eq!(p.doc.scene, before);
    assert!(!p.doc.history.can_undo());
}

#[test]
fn a_drag_is_one_undo_step_until_the_gesture_ends() {
    let mut p = project();
    let pos = p.doc.inspect(1).unwrap().components[0].path.clone();
    for v in ["2", "3", "4", "5"] {
        p.doc.set_field(1, &pos, "x", v).unwrap();
    }
    p.doc.end_gesture();
    p.doc.set_field(1, &pos, "x", "9").unwrap();
    assert_eq!(p.doc.history.labels().len(), 2);
    p.doc.undo().unwrap();
    assert_eq!(field(&p, 1, "Position", "x"), "5.0");
    p.doc.undo().unwrap();
    assert_eq!(field(&p, 1, "Position", "x"), "1.0");
    assert!(p.doc.undo().is_none());
}

#[test]
fn editing_after_an_undo_never_merges_and_drops_redo() {
    let mut p = project();
    let pos = p.doc.inspect(1).unwrap().components[0].path.clone();
    p.doc.set_field(1, &pos, "x", "2").unwrap();
    p.doc.undo().unwrap();
    p.doc.set_field(1, &pos, "x", "3").unwrap();
    assert!(!p.doc.history.can_redo());
    assert_eq!(p.doc.history.labels().len(), 1);
}

#[test]
fn entities_are_added_renamed_duplicated_and_deleted_with_undo() {
    let mut p = project();
    let start = p.doc.scene.write();
    let e = p.doc.spawn("coin", None).unwrap();
    assert_eq!(p.doc.selection, Some(e));
    let vel = p
        .doc
        .addable(e)
        .into_iter()
        .find(|c| c.ends_with("Velocity"))
        .unwrap();
    p.doc.add_component(e, &vel).unwrap();
    assert!(p.doc.add_component(e, &vel).is_err());
    p.doc.rename(e, "gold coin").unwrap();
    let copy = p.doc.duplicate(e).unwrap();
    assert_eq!(
        p.doc.inspect(copy).unwrap().name.as_deref(),
        Some("gold coin copy")
    );
    let child = p.doc.spawn("kid", Some(e)).unwrap();
    let rows = p.doc.outliner();
    let at = |s| rows.iter().find(|r| r.sid == s).unwrap().depth;
    assert_eq!((at(e), at(child)), (0, 1));
    // Deleting a parent takes its children.
    p.doc.despawn(e).unwrap();
    assert!(p.doc.inspect(e).is_none() && p.doc.inspect(child).is_none());
    assert!(p.doc.inspect(copy).is_some());
    // Every step undoes, back to the start.
    while p.doc.undo().is_some() {}
    assert_eq!(p.doc.scene.write(), start);
    while p.doc.redo().is_some() {}
    assert!(p.doc.inspect(copy).is_some() && p.doc.inspect(e).is_none());
    p.doc.remove_component(copy, &vel).unwrap();
    assert!(p.doc.remove_component(copy, &vel).is_err());
}

#[test]
fn playing_runs_the_game_and_stopping_restores_what_was_authored() {
    let mut p = project();
    let authored = p.doc.scene.write();
    let h0 = hash(&p);
    p.play().unwrap();
    p.step(10).unwrap();
    assert!(p.live_scene().write().contains("x=11.0"), "the hero moved");
    assert_eq!(
        p.doc.scene.write(),
        authored,
        "playing does not edit the document"
    );
    assert_ne!(hash(&p), h0);
    p.stop().unwrap();
    assert_eq!(p.mode, Mode::Edit);
    assert_eq!(hash(&p), h0, "stop puts the world back exactly");
    // The same session twice plays the same.
    p.play().unwrap();
    p.step(10).unwrap();
    let a = hash(&p);
    p.stop().unwrap();
    p.play().unwrap();
    p.step(10).unwrap();
    assert_eq!(hash(&p), a);
}

#[test]
fn edits_made_before_play_are_what_plays() {
    let mut p = project();
    let vel = p.doc.inspect(1).unwrap().components[1].path.clone();
    p.doc.set_field(1, &vel, "x", "5").unwrap();
    p.play().unwrap();
    p.step(2).unwrap();
    assert!(
        p.live_scene().write().contains("x=11.0"),
        "{}",
        p.live_scene().write()
    );
}

#[test]
fn live_reload_takes_new_rules_and_keeps_the_scene() {
    let mut p = project();
    let pos = p.doc.inspect(1).unwrap().components[0].path.clone();
    p.doc.set_field(2, &pos, "x", "77").unwrap();
    let faster = GAME.replace("pos.x += vel.x", "pos.x += vel.x * 10");
    p.set_source("game", &faster).unwrap();
    assert!(p.stale);
    p.reload().unwrap();
    assert!(!p.stale);
    assert_eq!(
        field(&p, 2, "Position", "x"),
        "77.0",
        "the edit survived the reload"
    );
    p.play().unwrap();
    p.step(1).unwrap();
    assert!(
        p.live_scene().write().contains("x=11.0"),
        "the new rule ran: {}",
        p.live_scene().write()
    );
}

#[test]
fn a_script_error_keeps_the_old_world_and_says_why() {
    let mut p = project();
    p.set_source("game", "def broken(:\n").unwrap();
    let e = p.reload().unwrap_err();
    assert!(e.contains("game"), "{e}");
    assert!(p.error.is_some());
    assert!(p.stale);
    // Still editable and playable with the old rules.
    p.doc.rename(1, "renamed").unwrap();
    p.sync().unwrap();
    p.set_source("game", GAME).unwrap();
    p.reload().unwrap();
    assert!(p.error.is_none());
    assert_eq!(p.doc.inspect(1).unwrap().name.as_deref(), Some("renamed"));
}

#[test]
fn a_fault_while_playing_stops_the_session() {
    let mut p = project();
    let bad = GAME.replace("pos.y += vel.y", "pos.y += undefined_name");
    p.set_source("game", &bad).unwrap();
    p.play().unwrap();
    let e = p.step(3).unwrap_err();
    assert!(e.contains("undefined_name"), "{e}");
    assert_eq!(p.mode, Mode::Edit);
    assert!(p.error.is_some());
}
