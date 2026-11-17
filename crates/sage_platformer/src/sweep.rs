//! Move-and-collide for boxes against solid boxes: pure functions, no Flecs.
//!
//! The platformer wants "collision queries over our own data" and a custom
//! controller, not a rigid-body solver (design doc 8.1). A body moves one
//! axis at a time, in steps no bigger than [`STEP`] so it cannot tunnel
//! through a thin platform, and stops flush against what blocks it. Touching
//! edges do not overlap ([`EPS`]), so a body resting on the floor is not
//! "inside" it.
//!
//! Everything here is plain `f32` arithmetic over inputs the caller sorts by
//! `StableId`, so it is deterministic.

/// Boxes closer than this are touching, not overlapping.
pub const EPS: f32 = 0.001;
/// The longest move made in one go (units). Thinner than any platform.
pub const STEP: f32 = 4.0;
/// How far from a surface still counts as standing on it or leaning on it.
pub const REACH: f32 = 0.06;
/// The most headroom reported.
pub const MAX_HEADROOM: f32 = 64.0;

/// Something bodies cannot enter. A one-way solid blocks only a body that
/// lands on it from above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolidBox {
    pub id: u64,
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub one_way: bool,
}

impl SolidBox {
    pub fn top(&self) -> f32 {
        self.center[1] + self.half[1]
    }
    pub fn bottom(&self) -> f32 {
        self.center[1] - self.half[1]
    }
}

/// A body before its move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyIn {
    pub center: [f32; 2],
    pub half: [f32; 2],
    /// Units per tick, not per second.
    pub delta: [f32; 2],
    /// Fall through one-way solids this tick.
    pub drop_through: bool,
}

/// What the move found out.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BodyOut {
    pub center: [f32; 2],
    /// The body was stopped moving sideways / up / down.
    pub hit_x: bool,
    pub hit_up: bool,
    pub hit_down: bool,
    pub grounded: bool,
    /// StableId of the solid it stands on (0 for none).
    pub floor: u64,
    /// -1 a wall touches its left side, 1 its right side, 0 neither or both.
    pub wall: i32,
    pub ceiling: bool,
    /// Free space above its head, up to [`MAX_HEADROOM`].
    pub headroom: f32,
    /// Grounded with nothing underfoot just past this side.
    pub drop_left: bool,
    pub drop_right: bool,
}

fn overlaps(c: [f32; 2], h: [f32; 2], s: &SolidBox) -> bool {
    (c[0] - s.center[0]).abs() < h[0] + s.half[0] - EPS
        && (c[1] - s.center[1]).abs() < h[1] + s.half[1] - EPS
}

/// Whether `s` blocks a body for a horizontal move.
fn blocks_x(s: &SolidBox) -> bool {
    !s.one_way
}

/// Moves along x by `dx`, stopping flush against the first blocker.
/// Returns the new x and whether it was blocked.
fn move_x(solids: &[SolidBox], mut c: [f32; 2], h: [f32; 2], dx: f32) -> (f32, bool) {
    let mut left = dx;
    while left.abs() > 0.0 {
        let step = left.clamp(-STEP, STEP);
        left -= step;
        let to = [c[0] + step, c[1]];
        let mut stop: Option<f32> = None;
        for s in solids.iter().filter(|s| blocks_x(s)) {
            if overlaps(to, h, s) && !overlaps(c, h, s) {
                let edge = if step > 0.0 {
                    s.center[0] - s.half[0] - h[0]
                } else {
                    s.center[0] + s.half[0] + h[0]
                };
                stop = Some(match stop {
                    None => edge,
                    Some(e) if step > 0.0 => e.min(edge),
                    Some(e) => e.max(edge),
                });
            }
        }
        match stop {
            Some(edge) => return (edge, true),
            None => c[0] = to[0],
        }
    }
    (c[0], false)
}

/// Moves along y by `dy`. Returns the new y, whether a ceiling stopped it and
/// whether a floor did.
fn move_y(
    solids: &[SolidBox],
    mut c: [f32; 2],
    h: [f32; 2],
    dy: f32,
    drop_through: bool,
) -> (f32, bool, bool) {
    let mut left = dy;
    while left.abs() > 0.0 {
        let step = left.clamp(-STEP, STEP);
        left -= step;
        let to = [c[0], c[1] + step];
        let bottom_before = c[1] - h[1];
        let mut stop: Option<f32> = None;
        for s in solids {
            if s.one_way && (step > 0.0 || drop_through || bottom_before < s.top() - EPS) {
                continue;
            }
            if overlaps(to, h, s) && !overlaps(c, h, s) {
                let edge = if step > 0.0 {
                    s.center[1] - s.half[1] - h[1]
                } else {
                    s.center[1] + s.half[1] + h[1]
                };
                stop = Some(match stop {
                    None => edge,
                    Some(e) if step > 0.0 => e.min(edge),
                    Some(e) => e.max(edge),
                });
            }
        }
        match stop {
            Some(edge) => return (edge, step > 0.0, step < 0.0),
            None => c[1] = to[1],
        }
    }
    (c[1], false, false)
}

/// Is any solid (a one-way one too) under the point `(x, y)`?
fn supported(solids: &[SolidBox], x: f32, y: f32) -> bool {
    solids
        .iter()
        .any(|s| (x - s.center[0]).abs() < s.half[0] && (y - s.center[1]).abs() < s.half[1])
}

/// Moves a body and reports what surrounds it afterwards. `solids` must be in
/// `StableId` order (ties between equally high floors go to the lower id).
pub fn step_body(solids: &[SolidBox], b: BodyIn) -> BodyOut {
    let h = b.half;
    let (x, hit_x) = move_x(solids, b.center, h, b.delta[0]);
    let (y, hit_up, hit_down) = move_y(solids, [x, b.center[1]], h, b.delta[1], b.drop_through);
    let c = [x, y];
    survey(
        solids,
        c,
        h,
        b.drop_through,
        BodyOut {
            center: c,
            hit_x,
            hit_up,
            hit_down,
            ..Default::default()
        },
    )
}

/// Fills in what is around a body that did not move (or just did): the
/// floor, walls, ceiling, headroom and ledges.
pub fn survey(
    solids: &[SolidBox],
    c: [f32; 2],
    h: [f32; 2],
    drop_through: bool,
    mut out: BodyOut,
) -> BodyOut {
    let bottom = c[1] - h[1];
    let top = c[1] + h[1];
    // Floor: the highest top within reach of the feet.
    let mut best: Option<(f32, u64)> = None;
    for s in solids {
        if (c[0] - s.center[0]).abs() >= h[0] + s.half[0] - EPS {
            continue;
        }
        let gap = bottom - s.top();
        if gap.abs() > REACH {
            // A one-way solid only holds a body that is above it.
            continue;
        }
        if s.one_way && drop_through {
            continue;
        }
        if best.is_none_or(|(t, _)| s.top() > t) {
            best = Some((s.top(), s.id));
        }
    }
    out.grounded = best.is_some();
    out.floor = best.map_or(0, |b| b.1);
    // Walls and ceiling and headroom.
    let (mut left, mut right) = (false, false);
    let mut headroom = MAX_HEADROOM;
    let mut ceiling = false;
    for s in solids.iter().filter(|s| !s.one_way) {
        if (c[1] - s.center[1]).abs() < h[1] + s.half[1] - EPS {
            let gap = (c[0] - s.center[0]).abs() - (h[0] + s.half[0]);
            if gap.abs() <= REACH {
                if s.center[0] < c[0] {
                    left = true;
                } else {
                    right = true;
                }
            }
        }
        if (c[0] - s.center[0]).abs() < h[0] + s.half[0] - EPS {
            let gap = s.bottom() - top;
            if gap >= -EPS {
                headroom = headroom.min(gap.max(0.0));
                if gap <= REACH {
                    ceiling = true;
                }
            }
        }
    }
    out.wall = match (left, right) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    out.ceiling = ceiling;
    out.headroom = headroom;
    if out.grounded {
        let probe = bottom - 1.0;
        out.drop_left = !supported(solids, c[0] - h[0] - 1.0, probe);
        out.drop_right = !supported(solids, c[0] + h[0] + 1.0, probe);
    }
    out
}

/// Pushes a body out of `mover` (which has just moved by `delta`) along the
/// axis the mover travelled most. Returns the pushed position and whether
/// the body is still inside the mover (squeezed against something else).
pub fn push_out(
    solids: &[SolidBox],
    mover: &SolidBox,
    c: [f32; 2],
    h: [f32; 2],
    delta: [f32; 2],
) -> ([f32; 2], bool) {
    if !overlaps(c, h, mover) {
        return (c, false);
    }
    let others: Vec<SolidBox> = solids
        .iter()
        .filter(|s| s.id != mover.id)
        .copied()
        .collect();
    let mut p = c;
    if delta[0].abs() >= delta[1].abs() {
        let want = if delta[0] > 0.0 {
            mover.center[0] + mover.half[0] + h[0] - c[0]
        } else {
            mover.center[0] - mover.half[0] - h[0] - c[0]
        };
        p[0] = move_x(&others, c, h, want).0;
    } else {
        let want = if delta[1] > 0.0 {
            mover.center[1] + mover.half[1] + h[1] - c[1]
        } else {
            mover.center[1] - mover.half[1] - h[1] - c[1]
        };
        p[1] = move_y(&others, c, h, want, true).0;
    }
    (p, overlaps(p, h, mover))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: u64, x: f32, y: f32, hw: f32, hh: f32) -> SolidBox {
        SolidBox {
            id,
            center: [x, y],
            half: [hw, hh],
            one_way: false,
        }
    }

    fn body(x: f32, y: f32, dx: f32, dy: f32) -> BodyIn {
        BodyIn {
            center: [x, y],
            half: [6.0, 8.0],
            delta: [dx, dy],
            drop_through: false,
        }
    }

    /// A floor whose top is y = 0, from x = -100 to 100.
    fn floor() -> SolidBox {
        block(1, 0.0, -8.0, 100.0, 8.0)
    }

    #[test]
    fn a_falling_body_lands_flush_and_is_grounded() {
        let out = step_body(&[floor()], body(0.0, 10.0, 0.0, -5.0));
        assert_eq!(out.center, [0.0, 8.0]);
        assert!(out.hit_down && out.grounded);
        assert_eq!(out.floor, 1);
        // Standing still on it keeps it grounded and not "hitting".
        let again = step_body(&[floor()], body(0.0, 8.0, 0.0, 0.0));
        assert!(again.grounded && !again.hit_down);
    }

    #[test]
    fn a_fast_body_does_not_tunnel_through_a_thin_floor() {
        let thin = block(1, 0.0, -1.0, 100.0, 1.0);
        let out = step_body(&[thin], body(0.0, 30.0, 0.0, -60.0));
        assert_eq!(out.center[1], 8.0);
        assert!(out.hit_down);
    }

    #[test]
    fn walls_stop_sideways_motion_and_report_their_side() {
        let wall = block(2, 20.0, 8.0, 4.0, 40.0);
        let out = step_body(&[floor(), wall], body(0.0, 8.0, 30.0, 0.0));
        // Wall's left edge is at 16; the body's half width is 6.
        assert_eq!(out.center[0], 10.0);
        assert!(out.hit_x);
        assert_eq!(out.wall, 1);
        let left = block(2, -20.0, 8.0, 4.0, 40.0);
        let out = step_body(&[floor(), left], body(0.0, 8.0, -30.0, 0.0));
        assert_eq!((out.center[0], out.wall), (-10.0, -1));
    }

    #[test]
    fn ceilings_stop_a_jump_and_headroom_counts_the_gap() {
        let roof = block(2, 0.0, 40.0, 50.0, 4.0); // bottom at 36
        let out = step_body(&[floor(), roof], body(0.0, 8.0, 0.0, 40.0));
        assert_eq!(out.center[1], 28.0); // top of body (36) touches the roof
        assert!(out.hit_up && out.ceiling);
        let rest = step_body(&[floor(), roof], body(0.0, 8.0, 0.0, 0.0));
        assert_eq!(rest.headroom, 20.0);
        let open = step_body(&[floor()], body(0.0, 8.0, 0.0, 0.0));
        assert_eq!(open.headroom, MAX_HEADROOM);
    }

    #[test]
    fn a_one_way_platform_holds_from_above_and_lets_a_jump_through() {
        let mut p = block(2, 0.0, 30.0, 20.0, 2.0); // top at 32
        p.one_way = true;
        // Rising through it from below is allowed.
        let up = step_body(&[p], body(0.0, 20.0, 0.0, 30.0));
        assert_eq!(up.center[1], 50.0);
        // Falling onto it from above lands.
        let down = step_body(&[p], body(0.0, 60.0, 0.0, -40.0));
        assert_eq!(down.center[1], 40.0);
        assert!(down.grounded);
        // Holding down (drop through) falls past it.
        let mut b = body(0.0, 40.0, 0.0, -20.0);
        b.drop_through = true;
        let through = step_body(&[p], b);
        assert_eq!(through.center[1], 20.0);
        assert!(!through.grounded);
        // A body that starts below the top does not get snapped up onto it.
        let inside = step_body(&[p], body(0.0, 30.0, 0.0, -2.0));
        assert_eq!(inside.center[1], 28.0);
    }

    #[test]
    fn ledges_are_reported_on_the_side_that_drops() {
        // A floor that ends at x = 10.
        let edge = block(1, 0.0, -8.0, 10.0, 8.0);
        let out = step_body(&[edge], body(5.0, 8.0, 0.0, 0.0));
        assert!(out.grounded);
        assert!(out.drop_right, "nothing under x = 12");
        assert!(!out.drop_left);
    }

    #[test]
    fn the_higher_floor_wins_and_ties_go_to_the_lower_id() {
        let a = block(3, 0.0, -8.0, 50.0, 8.0);
        let b = block(2, 0.0, -8.0, 50.0, 8.0);
        // Callers sort by id, so b comes first.
        let out = step_body(&[b, a], body(0.0, 8.0, 0.0, 0.0));
        assert_eq!(out.floor, 2);
    }

    #[test]
    fn pushing_out_of_a_mover_follows_its_motion() {
        // The mover spans x 2..10 and has just moved right by 2; the body
        // ahead of it (left edge 8) is pushed to the mover's far edge.
        let mover = block(5, 6.0, 8.0, 4.0, 8.0);
        let (p, squeezed) = push_out(&[mover], &mover, [14.0, 8.0], [6.0, 8.0], [2.0, 0.0]);
        assert_eq!(p, [16.0, 8.0]);
        assert!(!squeezed);
        // With a wall right behind it, the body is squeezed instead.
        let wall = block(6, 24.0, 8.0, 4.0, 40.0); // left edge at 20
        let (p, squeezed) = push_out(&[mover, wall], &mover, [14.0, 8.0], [6.0, 8.0], [2.0, 0.0]);
        assert_eq!(p, [14.0, 8.0]);
        assert!(squeezed);
    }
}
