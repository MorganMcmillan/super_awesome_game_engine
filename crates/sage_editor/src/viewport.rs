//! The scene viewport's model: what it draws and what the pointer does in it.
//!
//! Nothing here knows about a window or the UI. The host gives it the pointer
//! in world units each tick ([`ScenePointer`]) and shows the [`Frame`] it
//! returns, so picking and dragging are tested without either.

use crate::project::{Mode, Project};
use sage_core::StableId;
use sage_core::host::{DrawKind, DrawRect, Frame};
use sage_scene::{Scene, SceneComponent, SceneEntity, Value};

/// A float field of a component, by name.
fn float(c: &SceneComponent, name: &str) -> Option<f32> {
    c.fields
        .iter()
        .find(|(f, _)| f == name)
        .and_then(|(_, v)| match v {
            Value::F32(x) => Some(*x),
            Value::F64(x) => Some(*x as f32),
            _ => None,
        })
}

/// The position component of an entity: the one named Position, else the
/// first with float `x` and `y` fields.
pub fn position_of(e: &SceneEntity) -> Option<(&SceneComponent, [f32; 2])> {
    let mut first = None;
    for c in &e.components {
        if let (Some(x), Some(y)) = (float(c, "x"), float(c, "y")) {
            if c.path.ends_with("Position") {
                return Some((c, [x, y]));
            }
            first.get_or_insert((c, [x, y]));
        }
    }
    first
}

/// Half extents from a component with `hx` and `hy` (or `half_x` and
/// `half_y`), else a small default so everything can be seen and picked.
pub fn half_of(e: &SceneEntity) -> [f32; 2] {
    e.components
        .iter()
        .find_map(|c| {
            [("hx", "hy"), ("half_x", "half_y")]
                .into_iter()
                .find_map(|(a, b)| Some([float(c, a)?, float(c, b)?]))
        })
        .map_or([3.0, 3.0], |[x, y]| [x.abs().max(0.5), y.abs().max(0.5)])
}

/// What the scene viewport shows: every entity with a position, as a box.
/// The selected one is drawn as `DrawKind::Paddle`, the rest as walls.
pub fn scene_frame(scene: &Scene, selected: Option<u64>, tick: u64) -> Frame {
    let mut size = [160.0f32, 90.0f32];
    let mut rects = vec![];
    for e in &scene.entities {
        let Some((_, p)) = position_of(e) else {
            continue;
        };
        let half = half_of(e);
        size[0] = size[0].max(p[0] + half[0] + 10.0);
        size[1] = size[1].max(p[1] + half[1] + 10.0);
        rects.push(DrawRect {
            id: StableId(e.sid),
            kind: if Some(e.sid) == selected {
                DrawKind::Paddle
            } else {
                DrawKind::Wall
            },
            center: p,
            half,
        });
    }
    rects.insert(
        0,
        DrawRect {
            id: StableId(0),
            kind: DrawKind::Court,
            center: [size[0] / 2.0, size[1] / 2.0],
            half: [size[0] / 2.0, size[1] / 2.0],
        },
    );
    Frame {
        tick,
        world_size: size,
        rects,
        hud: vec![],
    }
}

/// The topmost entity under a point: its StableId, position component and
/// position.
pub fn pick(scene: &Scene, at: [f32; 2]) -> Option<(u64, String, [f32; 2])> {
    scene.entities.iter().rev().find_map(|e| {
        let (c, pos) = position_of(e)?;
        let h = half_of(e);
        ((at[0] - pos[0]).abs() <= h[0] && (at[1] - pos[1]).abs() <= h[1])
            .then(|| (e.sid, c.path.clone(), pos))
    })
}

/// The pointer over the scene viewport for one tick, in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScenePointer {
    pub at: [f32; 2],
    /// The pointer is inside the viewport.
    pub over: bool,
    /// The primary button is held.
    pub down: bool,
}

#[derive(Clone, Debug)]
struct Grab {
    sid: u64,
    component: String,
    /// Where on the entity it was picked up, from its center.
    offset: [f32; 2],
}

/// Click to select, drag to move. A drag is one undo step.
#[derive(Clone, Debug)]
pub struct SceneView {
    /// Dragged entities land on multiples of this many units: whole units by
    /// default, which is what a pixel-perfect 2D game wants (design doc
    /// 10.1). 0 leaves them where the pointer put them, to a tenth of a unit.
    pub snap: f32,
    grab: Option<Grab>,
    was_down: bool,
}

impl Default for SceneView {
    fn default() -> SceneView {
        SceneView {
            snap: 1.0,
            grab: None,
            was_down: false,
        }
    }
}

impl SceneView {
    fn snapped(&self, v: f32) -> String {
        if self.snap > 0.0 {
            format!("{:.1}", (v / self.snap).round() * self.snap)
        } else {
            format!("{v:.1}")
        }
    }

    /// One tick of the pointer. Returns whether the selection changed (the
    /// views need rebuilding even if the document did not change). The
    /// viewport only edits in edit mode: while playing, the pointer looks.
    pub fn pointer(&mut self, project: &mut Project, p: ScenePointer) -> bool {
        let mut picked = false;
        if p.over && p.down && !self.was_down && project.mode == Mode::Edit {
            let hit = pick(&project.doc.scene, p.at);
            project.doc.select(hit.as_ref().map(|h| h.0));
            self.grab = hit.map(|(sid, component, pos)| Grab {
                sid,
                component,
                offset: [p.at[0] - pos[0], p.at[1] - pos[1]],
            });
            picked = true;
        }
        if p.down {
            if let Some(g) = &self.grab {
                let x = self.snapped(p.at[0] - g.offset[0]);
                let y = self.snapped(p.at[1] - g.offset[1]);
                // A refused move (the entity was deleted under the pointer)
                // simply does not move it.
                let _ = project
                    .doc
                    .set_fields(g.sid, &g.component, &[("x", &x), ("y", &y)]);
            }
        } else if self.was_down {
            project.doc.end_gesture();
            self.grab = None;
        }
        self.was_down = p.down;
        picked
    }
}
