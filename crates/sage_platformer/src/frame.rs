//! Presentation of a platformer world: a read-only [`UiFrame`] draw list.
//!
//! The view is a fixed number of world units (the `Camera`'s `w` by `h`), the
//! window shows it at a whole-number scale (`sage_platform::sprite::ui_view`),
//! and everything is snapped to whole units, so it stays crisp. Draw commands
//! are in UI units (y down); the world is y up, so the conversion lives here.

use flecs_ecs::prelude::*;
use sage_collision::ecs::{Collider, Position};
use sage_core::host::{UI_ADVANCE, UiDraw, UiFrame};
use sage_ecs_py::StableIdC;

use crate::vocab::{Backdrop, Camera, Look};

/// What the heads-up display shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hud {
    pub hp: u32,
    pub max_hp: u32,
    pub coins: u32,
    /// 0 playing, 1 paused, 2 dead.
    pub state: u32,
}

/// `0xRRGGBBAA` to bytes.
pub fn rgba(c: u32) -> [u8; 4] {
    [(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8, c as u8]
}

/// A deterministic 0..1 value for hill `k` of a layer.
fn hill_height(k: i64, layer: i64) -> f32 {
    let h = (k.wrapping_mul(2_654_435_761).wrapping_add(layer * 40_503)) >> 7;
    (h & 0xff) as f32 / 255.0
}

/// One layer of scenery hills: triangles a period apart that scroll at
/// `factor` times the camera's speed.
struct Hills {
    factor: f32,
    period: f32,
    height: f32,
    color: u32,
    layer: i64,
}

impl Hills {
    fn draw(&self, draws: &mut Vec<UiDraw>, view: [f32; 2], camera_origin: [f32; 2]) {
        if self.color & 0xff == 0 {
            return;
        }
        let period = self.period;
        let shift = camera_origin[0] * self.factor;
        let base = view[1] + camera_origin[1] * self.factor;
        let first = ((shift - period) / period).floor() as i64;
        let last = ((shift + view[0] + period) / period).ceil() as i64;
        let c = rgba(self.color);
        for k in first..=last {
            let peak_x = (k as f32 + 0.5) * period - shift;
            let h = self.height * (0.55 + 0.45 * hill_height(k, self.layer));
            draws.push(UiDraw::Triangle {
                pts: [
                    [(peak_x - period * 0.8).round(), base],
                    [peak_x.round(), (base - h).round()],
                    [(peak_x + period * 0.8).round(), base],
                ],
                colors: [c; 3],
            });
        }
    }
}

/// The text is the built-in pixel face, upper case.
fn text(draws: &mut Vec<UiDraw>, pos: [f32; 2], s: &str, color: [u8; 4]) {
    draws.push(UiDraw::Text {
        pos,
        text: s.to_string(),
        scale: 1.0,
        color,
        clip: None,
        style: 0,
    });
}

fn centered(draws: &mut Vec<UiDraw>, view: [f32; 2], y: f32, s: &str, color: [u8; 4]) {
    let w = s.chars().count() as f32 * UI_ADVANCE;
    text(draws, [((view[0] - w) / 2.0).floor(), y], s, color);
}

pub fn compose(world: &World, hud: &Hud) -> UiFrame {
    let cam = world.get::<&Camera>(|c| *c);
    let back = world.get::<&Backdrop>(|b| *b);
    let view = [cam.w.max(1.0), cam.h.max(1.0)];
    let left = (cam.x - view[0] / 2.0).round();
    let bottom = (cam.y - view[1] / 2.0).round();
    let mut draws = vec![UiDraw::Gradient {
        pos: [0.0, 0.0],
        size: view,
        colors: [
            rgba(back.sky_top),
            rgba(back.sky_top),
            rgba(back.sky_bottom),
            rgba(back.sky_bottom),
        ],
        clip: None,
    }];
    let origin = [left, bottom];
    Hills {
        factor: 0.25,
        period: 96.0,
        height: 90.0,
        color: back.far,
        layer: 1,
    }
    .draw(&mut draws, view, origin);
    Hills {
        factor: 0.5,
        period: 64.0,
        height: 56.0,
        color: back.near,
        layer: 2,
    }
    .draw(&mut draws, view, origin);

    struct Row {
        layer: i32,
        sid: u64,
        center: [f32; 2],
        half: [f32; 2],
        look: Look,
    }
    let mut rows: Vec<Row> = vec![];
    world
        .new_query::<(&StableIdC, &Position, &Collider, &Look)>()
        .each(|(sid, p, c, look)| {
            rows.push(Row {
                layer: look.layer,
                sid: sid.value,
                center: [p.x, p.y],
                half: [c.hw, c.hh],
                look: *look,
            });
        });
    rows.sort_by_key(|r| (r.layer, r.sid));
    for r in rows {
        let l = (r.center[0] - r.half[0] - left).round();
        let rt = (r.center[0] + r.half[0] - left).round();
        let t = (view[1] - (r.center[1] + r.half[1] - bottom)).round();
        let b = (view[1] - (r.center[1] - r.half[1] - bottom)).round();
        if rt <= 0.0 || l >= view[0] || b <= 0.0 || t >= view[1] || rt <= l || b <= t {
            continue;
        }
        let (pos, size) = ([l, t], [rt - l, b - t]);
        if r.look.image != 0 {
            let (cols, rows_) = (r.look.cols.max(1), r.look.rows.max(1));
            let (cx, cy) = (r.look.frame % cols, (r.look.frame / cols) % rows_);
            let (u0, u1) = (cx as f32 / cols as f32, (cx + 1) as f32 / cols as f32);
            let (v0, v1) = (cy as f32 / rows_ as f32, (cy + 1) as f32 / rows_ as f32);
            let (u0, u1) = if r.look.flip != 0 { (u1, u0) } else { (u0, u1) };
            draws.push(UiDraw::Image {
                asset: r.look.image,
                pos,
                size,
                uv: [u0, v0, u1, v1],
                tint: if r.look.color == 0 {
                    [255; 4]
                } else {
                    rgba(r.look.color)
                },
                clip: None,
            });
        } else if r.look.color & 0xff != 0 {
            draws.push(UiDraw::Rect {
                pos,
                size,
                color: rgba(r.look.color),
            });
        }
    }

    // The heads-up display: hearts, then coins.
    for i in 0..hud.max_hp {
        let full = i < hud.hp;
        draws.push(UiDraw::Rect {
            pos: [6.0 + i as f32 * 10.0, 6.0],
            size: [8.0, 7.0],
            color: if full {
                [226, 60, 60, 255]
            } else {
                [60, 40, 48, 200]
            },
        });
    }
    text(
        &mut draws,
        [6.0, 17.0],
        &format!("COINS {}", hud.coins),
        [255, 240, 160, 255],
    );
    if hud.state != 0 {
        draws.push(UiDraw::Rect {
            pos: [0.0, 0.0],
            size: view,
            color: [0, 0, 0, 120],
        });
        let (word, color) = if hud.state == 1 {
            ("PAUSED", [255, 255, 255, 255])
        } else {
            ("OOPS", [255, 150, 130, 255])
        };
        centered(&mut draws, view, view[1] / 2.0 - 4.0, word, color);
    }
    UiFrame { size: view, draws }
}
