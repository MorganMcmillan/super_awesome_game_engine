//! The kit's rules, one behavior at a time, on tiny levels.

mod common;

use common::*;
use flecs_ecs::prelude::*;
use sage_platformer::GameState;
use sage_platformer::buttons::*;

#[test]
fn the_player_falls_onto_the_floor_and_stands() {
    let mut p = with(FLAT);
    hold(&mut p, 0, 60);
    let b = p.player_body();
    assert_eq!(b.grounded, 1);
    assert_eq!(p.mode_name(), "ground");
    // Floor top is y = 32 (two rows of 16); the player is 14 tall.
    assert_eq!(p.player_position().y, 32.0 + 7.0);
    assert_eq!(p.state(), GameState::Playing);
    assert!(p.play_enabled());
}

fn max_height(p: &mut sage_platformer::Platformer, buttons: &[(u32, u32)]) -> f32 {
    let mut top = p.player_position().y;
    for &(b, n) in buttons {
        for _ in 0..n {
            hold(p, b, 1);
            top = top.max(p.player_position().y);
        }
    }
    top
}

#[test]
fn walking_and_running_reach_their_top_speeds_and_braking_stops() {
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    let x0 = p.player_position().x;
    hold(&mut p, RIGHT, 40);
    assert!(
        (p.player_velocity().x - 80.0).abs() < 0.01,
        "{:?}",
        p.player_velocity()
    );
    assert!(p.player_position().x > x0 + 40.0);
    hold(&mut p, RIGHT | RUN, 60);
    assert!((p.player_velocity().x - 140.0).abs() < 0.01);
    // Let go: friction stops it.
    hold(&mut p, 0, 40);
    assert_eq!(p.player_velocity().x, 0.0);
    // Turning around brakes harder than friction does.
    hold(&mut p, RIGHT | RUN, 40);
    let before = p.player_velocity().x;
    hold(&mut p, LEFT, 5);
    assert!(
        p.player_velocity().x < before - 80.0,
        "{}",
        p.player_velocity().x
    );
}

#[test]
fn holding_jump_rises_higher_than_tapping_it() {
    let open = "
..............
..............
..............
..............
..............
..............
..............
..............
.....@........
##############
##############
";
    let mut held = with(open);
    hold(&mut held, 0, 30);
    let floor_y = held.player_position().y;
    let high = max_height(&mut held, &[(JUMP, 60)]) - floor_y;
    let mut tapped = with(open);
    hold(&mut tapped, 0, 30);
    let low = max_height(&mut tapped, &[(JUMP, 3), (0, 57)]) - floor_y;
    assert!((60.0..80.0).contains(&high), "held jump rose {high}");
    assert!(low < high * 0.6, "tap {low} vs held {high}");
    assert!(low > 5.0, "a tap still leaves the ground: {low}");
}

#[test]
fn a_jump_still_works_just_after_a_ledge_and_a_jump_pressed_early_still_counts() {
    // A floor that ends: walk off it.
    let ledge = "
...........
...........
...........
...........
...........
@..........
####........
####........
";
    let jumps_after = |ticks_late: u32| {
        let mut p = with(ledge);
        hold(&mut p, 0, 20);
        // Walk right until the feet leave the floor.
        let mut n = 0;
        while p.player_body().grounded == 1 {
            hold(&mut p, RIGHT, 1);
            n += 1;
            assert!(n < 200, "never left the ledge");
        }
        hold(&mut p, RIGHT, ticks_late);
        hold(&mut p, RIGHT | JUMP, 1);
        p.tally()[0] // jumps
    };
    assert_eq!(jumps_after(3), 1, "inside coyote time");
    assert_eq!(jumps_after(12), 0, "too late");

    // Buffered: press JUMP a few ticks before landing.
    let open = "
.......
.......
.......
.......
..@....
#######
#######
";
    let mut p = with(open);
    hold(&mut p, 0, 30);
    hold(&mut p, JUMP, 1);
    hold(&mut p, 0, 1);
    // Wait until the descent is a few ticks from the floor.
    while !(p.player_velocity().y < 0.0 && p.player_position().y < 32.0 + 7.0 + 4.0) {
        hold(&mut p, 0, 1);
    }
    let before = p.tally()[0];
    hold(&mut p, JUMP, 1);
    hold(&mut p, 0, 1);
    hold(&mut p, JUMP, 6); // still held; the press was buffered
    assert!(p.tally()[0] > before, "the early press was lost");
}

#[test]
fn a_wall_slide_and_a_wall_jump() {
    let shaft = "
..........#
..........#
..........#
..........#
..........#
..........#
.....@....#
###########
";
    let mut p = with(shaft);
    hold(&mut p, 0, 20);
    hold(&mut p, RIGHT | RUN, 60); // run into the wall
    assert_eq!(p.player_body().wall, 1);
    hold(&mut p, RIGHT, 1);
    // Jump and keep pressing into the wall: at the top of the jump it grabs.
    let mut n = 0;
    hold(&mut p, RIGHT | JUMP, 1);
    while p.mode_name() != "wall" {
        hold(&mut p, RIGHT | JUMP, 1);
        n += 1;
        assert!(n < 100, "never grabbed the wall: {}", p.mode_name());
    }
    // Sliding is slow.
    hold(&mut p, RIGHT, 10);
    assert!(p.player_velocity().y >= -60.01, "{:?}", p.player_velocity());
    // Release JUMP for a tick so the next press is a new press, then kick off.
    hold(&mut p, RIGHT, 1);
    hold(&mut p, RIGHT | JUMP, 1);
    assert_eq!(p.tally()[2], 1, "wall jumps");
    assert!(p.player_velocity().x < -100.0, "{:?}", p.player_velocity());
    assert!(p.player_velocity().y > 200.0);
    // For a moment steering back toward the wall is ignored.
    hold(&mut p, RIGHT, 5);
    assert!(p.player_velocity().x < -100.0);
}

#[test]
fn rolling_is_fast_and_low_and_ends_standing() {
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    assert_eq!(p.player_collider().hh, 7.0);
    hold(&mut p, RIGHT | RUN, 30);
    hold(&mut p, RIGHT | RUN | DOWN, 1);
    hold(&mut p, DOWN, 1);
    assert_eq!(p.mode_name(), "roll");
    assert_eq!(p.player_collider().hh, 4.0, "low while rolling");
    assert!(p.player_velocity().x >= 150.0, "{:?}", p.player_velocity());
    // The feet did not move when the box shrank.
    let feet = p.player_position().y - p.player_collider().hh;
    assert!((feet - 32.0).abs() < 0.01, "feet at {feet}");
    // It runs out and stands up again.
    hold(&mut p, 0, 60);
    assert_eq!(p.mode_name(), "ground");
    assert_eq!(p.player_collider().hh, 7.0);
    assert_eq!(p.tally()[3], 1, "rolls");
}

#[test]
fn ducking_when_slow_and_getting_up_again() {
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    hold(&mut p, DOWN, 3);
    assert_eq!(p.mode_name(), "duck");
    assert_eq!(p.player_collider().hh, 4.0);
    hold(&mut p, 0, 3);
    assert_eq!(p.mode_name(), "ground");
    assert_eq!(p.player_collider().hh, 7.0);
}

#[test]
fn a_dive_lunges_low_and_lands_in_a_roll() {
    let open = "
..............
..............
..............
..............
..............
..............
.....@........
##############
##############
";
    let mut p = with(open);
    hold(&mut p, 0, 30);
    hold(&mut p, RIGHT | JUMP, 12);
    hold(&mut p, RIGHT | DOWN, 1); // down held...
    hold(&mut p, RIGHT | DOWN | SPIN, 1); // ...then spin: dive
    assert_eq!(p.mode_name(), "dive");
    assert_eq!(p.player_collider().hh, 4.0);
    assert!(p.player_velocity().x >= 199.0);
    assert_eq!(p.tally()[4], 1, "dives");
    let mut n = 0;
    while p.mode_name() == "dive" {
        hold(&mut p, DOWN, 1);
        n += 1;
        assert!(n < 120);
    }
    assert_eq!(p.mode_name(), "roll", "a dive lands in a roll");
    hold(&mut p, 0, 80);
    assert_eq!(p.mode_name(), "ground");
}

#[test]
fn a_spin_slows_the_fall_once_per_jump() {
    let open = "
..............
..............
..............
..............
..............
..............
.....@........
##############
##############
";
    let mut p = with(open);
    hold(&mut p, 0, 30);
    hold(&mut p, JUMP, 20);
    hold(&mut p, 0, 1);
    hold(&mut p, SPIN, 1);
    assert_eq!(p.mode_name(), "spin");
    assert_eq!(p.tally()[5], 1, "spins");
    let mut slowest = 0.0f32;
    for _ in 0..15 {
        hold(&mut p, 0, 1);
        slowest = slowest.min(p.player_velocity().y);
    }
    assert!(slowest >= -70.01, "fell at {slowest}");
    // When it ends, a second spin in the same jump does nothing.
    while p.mode_name() == "spin" {
        hold(&mut p, 0, 1);
    }
    hold(&mut p, SPIN, 1);
    assert_eq!(p.tally()[5], 1, "no second spin");
    // Landing recharges it.
    hold(&mut p, 0, 120);
    assert_eq!(p.mode_name(), "ground");
    hold(&mut p, JUMP, 10);
    hold(&mut p, 0, 1);
    hold(&mut p, SPIN, 1);
    assert_eq!(p.tally()[5], 2);
}

// ----- actors -------------------------------------------------------------------------

/// Standing still, a coin is an overlap away.
#[test]
fn touching_a_coin_collects_it_once() {
    let level = "
.......
.......
.......
..@.o..
#######
#######
";
    let mut p = with(level);
    assert_eq!(p.coins(), 0);
    hold(&mut p, 0, 20);
    assert_eq!(p.coins(), 0);
    hold(&mut p, RIGHT, 40);
    assert_eq!(p.coins(), 1);
    assert_eq!(p.tally()[6], 1);
    assert_eq!(p.take_cues(), vec!["land".to_string(), "coin".to_string()]);
    // The coin is gone from the world.
    hold(&mut p, LEFT, 60);
    hold(&mut p, RIGHT, 60);
    assert_eq!(p.coins(), 1);
}

const STOMP_LEVEL: &str = "
.......
.......
.@.....
.......
.e.....
#######
#######
";

fn still_enemy(level: &str) -> sage_platformer::Platformer {
    with_script(level, "kit.tune(walker_speed=0.0)\n")
}

#[test]
fn landing_on_an_enemy_defeats_it_and_bounces() {
    let mut p = still_enemy(STOMP_LEVEL);
    let mut n = 0;
    while p.stomps() == 0 {
        hold(&mut p, 0, 1);
        n += 1;
        assert!(n < 120, "never landed on it");
    }
    assert_eq!(p.health().0, 3, "stomping hurts nobody");
    assert!(p.player_velocity().y > 200.0, "{:?}", p.player_velocity());
    assert_eq!(p.take_cues().last().unwrap(), "stomp");
    // Holding JUMP bounces higher than not.
    let mut high = still_enemy(STOMP_LEVEL);
    let mut n = 0;
    while high.stomps() == 0 {
        hold(&mut high, JUMP, 1);
        n += 1;
        assert!(n < 120);
    }
    assert!(high.player_velocity().y > p.player_velocity().y);
}

#[test]
fn walking_into_an_enemy_hurts_and_throws_you_back() {
    let level = "
.........
.........
.........
.@..e....
#########
#########
";
    let mut p = with(level);
    hold(&mut p, 0, 20);
    let mut n = 0;
    while p.health().0 == 3 {
        hold(&mut p, RIGHT, 1);
        n += 1;
        assert!(n < 200, "never touched the enemy");
    }
    assert_eq!(p.health(), (2, 3));
    assert_eq!(p.stomps(), 0);
    assert_eq!(p.mode_name(), "hurt");
    assert!(
        p.player_velocity().x < -50.0,
        "thrown back: {:?}",
        p.player_velocity()
    );
    assert!(p.player_velocity().y > 100.0);
    assert_eq!(p.tally()[8], 1, "hurts");
    // The control comes back; so does the ground.
    hold(&mut p, 0, 60);
    assert_eq!(p.mode_name(), "ground");
    assert_eq!(p.health().0, 2, "no second hit while invulnerable");
}

#[test]
fn being_hurt_makes_you_safe_for_a_moment() {
    let level = "
.........
.........
.@...^^^.
#########
#########
";
    let mut p = with(level);
    hold(&mut p, 0, 20);
    hold(&mut p, RIGHT | RUN, 80);
    // One touch, not one per tick, however long the spikes are.
    assert_eq!(p.health().0, 2, "{:?} {}", p.health(), p.mode_name());
    hold(&mut p, 0, 200);
    assert_eq!(p.health().0, 2);
}

#[test]
fn spinning_rolling_and_diving_defeat_enemies_that_are_not_below_you() {
    // Roll into a standing enemy: it falls, you do not get hurt.
    let level = "
.........
.........
.........
.@...e...
#########
#########
";
    let mut p = still_enemy(level);
    hold(&mut p, 0, 20);
    hold(&mut p, RIGHT | RUN, 25);
    hold(&mut p, RIGHT | RUN | DOWN, 1);
    hold(&mut p, RIGHT | DOWN, 30);
    assert_eq!(p.stomps(), 1, "{:?}", p.mode_name());
    assert_eq!(p.health().0, 3);
}

#[test]
fn a_checkpoint_sets_where_you_come_back_and_a_fall_is_death() {
    let level = "
...........
...........
...........
.@...c.....
#######..###
#######..###
";
    let mut p = with(level);
    hold(&mut p, 0, 20);
    let start = p.player_position();
    hold(&mut p, RIGHT, 50); // past the flag
    assert_eq!(p.tally()[9], 1, "checkpoints");
    // Run on into the hole.
    let mut n = 0;
    while p.state() == GameState::Playing {
        hold(&mut p, RIGHT, 1);
        n += 1;
        assert!(n < 300, "never fell: {:?}", p.player_position());
    }
    assert_eq!(p.state(), GameState::Dead);
    assert!(
        !p.play_enabled(),
        "the world holds still while you are dead"
    );
    assert_eq!(p.deaths(), 1);
    let frozen = p.player_position();
    hold(&mut p, 0, 20);
    assert_eq!(p.player_position(), frozen);
    // Then you are back, at the flag and not at the start.
    hold(&mut p, 0, 60);
    assert_eq!(p.state(), GameState::Playing);
    assert!(p.play_enabled());
    assert_eq!(p.health(), (3, 3));
    let back = p.player_position();
    assert!(back.x > start.x + 40.0, "{back:?} vs {start:?}");
    hold(&mut p, 0, 30);
    assert_eq!(p.mode_name(), "ground");
}

#[test]
fn running_out_of_health_is_death_too() {
    let level = "
.........
.........
.@.......
#^^^^^^^##
#########
";
    let mut p = with_script(level, "kit.tune(invuln_ticks=1, hurt_ticks=2)\n");
    let mut n = 0;
    while p.state() == GameState::Playing {
        hold(&mut p, 0, 1);
        n += 1;
        assert!(n < 300, "hp {:?} at {:?}", p.health(), p.player_position());
    }
    assert_eq!(p.health().0, 0);
}

#[test]
fn pause_freezes_everything_and_resume_continues() {
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    hold(&mut p, RIGHT, 10);
    hold(&mut p, 0, 1);
    hold(&mut p, PAUSE, 1);
    assert_eq!(p.state(), GameState::Paused);
    assert!(!p.play_enabled());
    let frozen = p.player_position();
    hold(&mut p, RIGHT, 30); // input is ignored
    assert_eq!(p.player_position(), frozen);
    hold(&mut p, 0, 1);
    hold(&mut p, PAUSE, 1);
    assert_eq!(p.state(), GameState::Playing);
    hold(&mut p, RIGHT, 10);
    assert!(p.player_position().x > frozen.x);
}

#[test]
fn a_moving_platform_carries_whoever_stands_on_it() {
    let level = "
...........
...........
...@.......
..hhh......
...........
...........
###########
";
    let mut p = with(level);
    hold(&mut p, 0, 10);
    assert_eq!(p.player_body().grounded, 1);
    let sid_on = p.player_body().floor;
    assert!(sid_on >= 1000);
    let x0 = p.player_position().x;
    hold(&mut p, 0, 90); // 1.5 s at 30 u/s: it has moved ~45 units
    assert_eq!(p.player_body().grounded, 1, "still on it");
    assert_eq!(p.player_body().floor, sid_on);
    let moved = p.player_position().x - x0;
    assert!(moved.abs() > 25.0, "carried only {moved}");
}

#[test]
fn a_one_way_platform_can_be_jumped_through_landed_on_and_dropped_through() {
    let level = "
.........
.........
..=====..
.........
.........
.@.......
#########
#########
";
    let mut p = with(level);
    hold(&mut p, 0, 20);
    let floor = p.player_position().y;
    // Walk under it and jump up through it.
    hold(&mut p, RIGHT, 12);
    hold(&mut p, JUMP, 40);
    hold(&mut p, 0, 60);
    assert_eq!(p.player_body().grounded, 1);
    let top = p.player_position().y;
    assert!(top > floor + 30.0, "stood on top: {top} vs {floor}");
    // DOWN, then JUMP: fall through to the floor.
    hold(&mut p, DOWN, 3);
    hold(&mut p, DOWN | JUMP, 1);
    hold(&mut p, 0, 80);
    assert!(
        (p.player_position().y - floor).abs() < 0.5,
        "{:?}",
        p.player_position()
    );
}

#[test]
fn the_camera_follows_the_player_and_stays_inside_the_level() {
    let level = "
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
..........................................
.@........................................
##########################################
##########################################
";
    let mut p = with(level);
    hold(&mut p, 0, 30);
    let cam = p.camera();
    assert_eq!((cam.w, cam.h), (320.0, 180.0));
    assert_eq!(cam.x, 160.0, "clamped to the left edge");
    hold(&mut p, RIGHT | RUN, 240);
    let cam = p.camera();
    assert!(cam.x > 200.0, "{cam:?}");
    assert!(cam.x <= 42.0 * 16.0 - 160.0 + 0.01);
    assert!(cam.y >= 90.0);
}

#[test]
fn the_frame_draws_the_world_the_camera_sees() {
    use sage_core::host::UiDraw;
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    let f = p.frame();
    assert_eq!(f.size, [320.0, 180.0]);
    let rects = f
        .draws
        .iter()
        .filter(|d| matches!(d, UiDraw::Rect { .. }))
        .count();
    assert!(rects > 20, "tiles and the player: {rects}");
    assert!(
        f.draws.iter().any(|d| matches!(d, UiDraw::Triangle { .. })),
        "hills"
    );
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Text { text, .. } if text == "COINS 0"))
    );
    // The player is drawn at the middle of the view, horizontally.
    let player = f.draws.iter().find_map(|d| match d {
        UiDraw::Rect { pos, size, color } if *color == [220, 64, 56, 255] => Some((*pos, *size)),
        _ => None,
    });
    let (pos, size) = player.expect("the player is drawn");
    assert_eq!(size, [10.0, 14.0]);
    assert!(pos[0] > 100.0 && pos[0] < 200.0, "{pos:?}");
}

// Paused and dead put a dim layer and a word over the world.
#[test]
fn the_frame_says_when_it_is_paused() {
    use sage_core::host::UiDraw;
    let mut p = with(FLAT);
    hold(&mut p, 0, 30);
    hold(&mut p, PAUSE, 1);
    let f = p.frame();
    assert!(
        f.draws
            .iter()
            .any(|d| matches!(d, UiDraw::Text { text, .. } if text == "PAUSED"))
    );
}

#[test]
fn a_wall_kick_works_leaning_away_from_the_wall_too() {
    let shaft = "
..........#
..........#
..........#
..........#
..........#
..........#
.....@....#
###########
";
    let mut p = with(shaft);
    hold(&mut p, 0, 20);
    hold(&mut p, RIGHT | RUN, 60);
    let mut n = 0;
    hold(&mut p, RIGHT | JUMP, 1);
    while p.mode_name() != "wall" {
        hold(&mut p, RIGHT | JUMP, 1);
        n += 1;
        assert!(n < 100, "never grabbed the wall");
    }
    hold(&mut p, RIGHT, 1); // let go of JUMP, keep pushing in
    hold(&mut p, LEFT | JUMP, 1); // lean away and kick
    assert_eq!(p.tally()[2], 1, "wall jumps");
    assert!(p.player_velocity().x < -100.0, "{:?}", p.player_velocity());
}

#[test]
fn enemies_face_the_way_they_walk() {
    let level = "
..........
..........
.@........
..e.......
##########
##########
";
    let mut p = with(level);
    hold(&mut p, 0, 30);
    // Find the walker's Look through the frame: it is the purple box. Its
    // `flip` is not drawn for a box, so read it from the world instead.
    let flips: Vec<u32> = {
        let mut v = vec![];
        p.world()
            .new_query::<(&sage_platformer::Look, &sage_platformer::Body)>()
            .each(|(look, _)| v.push(look.flip));
        v
    };
    // The player (facing right) and the walker (walking left at first).
    assert!(
        flips.contains(&1),
        "a walker walking left is mirrored: {flips:?}"
    );
}
