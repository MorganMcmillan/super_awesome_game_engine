//! Drawing: a laid-out, interacted tree becomes a [`UiFrame`], the plain
//! data presentation consumes (`sage_core::host`). The UI itself never
//! touches a renderer.

use sage_core::host::{
    STYLE_BOLD, STYLE_ITALIC, UI_ADVANCE, UI_GLYPH, UI_LINE_HEIGHT, UiDraw, UiFrame,
};

use crate::layout::{
    Direction, Fit, ImageNode, Node, Rect, ShapeNode, Tree, ViewportNode, Widget, chars_per_line,
    rgba,
};
use crate::style::{faded, resolve};
use crate::text::{locate, mask, rows};

/// A rectangle cut off at `clip`.
fn rect(out: &mut Vec<UiDraw>, r: Rect, color: [u8; 4], clip: Option<Rect>) {
    let r = clip.map_or(r, |c| r.intersect(&c));
    if color[3] > 0 && r.w > 0.0 && r.h > 0.0 {
        out.push(UiDraw::Rect {
            pos: [r.x, r.y],
            size: [r.w, r.h],
            color,
        });
    }
}

/// Cuts a convex polygon to the rectangle `r` (left, top, right, bottom).
pub fn clip_polygon(poly: &[[f32; 2]], r: [f32; 4]) -> Vec<[f32; 2]> {
    // Sutherland-Hodgman, one edge of the rectangle at a time.
    let mut pts = poly.to_vec();
    for edge in 0..4 {
        let inside = |p: [f32; 2]| match edge {
            0 => p[0] >= r[0],
            1 => p[1] >= r[1],
            2 => p[0] <= r[2],
            _ => p[1] <= r[3],
        };
        let cut = |a: [f32; 2], b: [f32; 2]| {
            let (axis, line) = match edge {
                0 => (0, r[0]),
                1 => (1, r[1]),
                2 => (0, r[2]),
                _ => (1, r[3]),
            };
            let t = (line - a[axis]) / (b[axis] - a[axis]);
            let mut p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            p[axis] = line;
            p
        };
        let mut next = vec![];
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            match (inside(a), inside(b)) {
                (true, true) => next.push(b),
                (true, false) => next.push(cut(a, b)),
                (false, true) => {
                    next.push(cut(a, b));
                    next.push(b);
                }
                (false, false) => {}
            }
        }
        pts = next;
        if pts.is_empty() {
            break;
        }
    }
    pts
}

/// A convex polygon as a fan of triangles, cut to `clip` first.
fn fan(out: &mut Vec<UiDraw>, poly: &[[f32; 2]], color: [u8; 4], clip: [f32; 4]) {
    if color[3] == 0 {
        return;
    }
    let pts = clip_polygon(poly, clip);
    for k in 1..pts.len().saturating_sub(1) {
        out.push(UiDraw::Triangle {
            pts: [pts[0], pts[k], pts[k + 1]],
            colors: [color; 3],
        });
    }
}

/// Draws a canvas shape: its points are relative to the node's top-left, and
/// it is cut off at the node's rectangle and any clip.
fn shape(out: &mut Vec<UiDraw>, n: &Node, s: &ShapeNode, fade: f32) {
    let r = n.rect;
    let bounds = n.clip.map_or(r, |c| r.intersect(&c));
    let clip = [bounds.x, bounds.y, bounds.x + bounds.w, bounds.y + bounds.h];
    let at = |p: [f32; 2]| [r.x + p[0], r.y + p[1]];
    let color = faded(rgba(s.color), fade);
    match s.kind {
        1 => {
            let (a, b) = (at(s.pts[0]), at(s.pts[1]));
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            let half = s.width.max(0.0) / 2.0;
            let (nx, ny) = if len > 0.0 {
                (-dy / len * half, dx / len * half)
            } else {
                (0.0, half)
            };
            // A point is a square of the line's width.
            let (tx, ty) = if len > 0.0 { (0.0, 0.0) } else { (half, 0.0) };
            fan(
                out,
                &[
                    [a[0] + nx - tx, a[1] + ny - ty],
                    [b[0] + nx + tx, b[1] + ny + ty],
                    [b[0] - nx + tx, b[1] - ny + ty],
                    [a[0] - nx - tx, a[1] - ny - ty],
                ],
                color,
                clip,
            );
        }
        2 => {
            fan(
                out,
                &[at(s.pts[0]), at(s.pts[1]), at(s.pts[2])],
                color,
                clip,
            );
        }
        3 => {
            let (c, radii) = (at(s.pts[0]), s.pts[1]);
            let segments = ((radii[0].max(radii[1]) * 2.0) as usize).clamp(12, 64);
            let poly: Vec<[f32; 2]> = (0..segments)
                .map(|k| {
                    let a = k as f32 / segments as f32 * std::f32::consts::TAU;
                    [c[0] + radii[0] * a.cos(), c[1] + radii[1] * a.sin()]
                })
                .collect();
            fan(out, &poly, color, clip);
        }
        _ => {}
    }
}

/// How a world of `world` units is fitted into `r`: its scale and the
/// top-left of its bottom-left-origin box, letterboxed and centered.
pub fn viewport_fit(r: Rect, world: [f32; 2]) -> (f32, [f32; 2]) {
    if world[0] <= 0.0 || world[1] <= 0.0 {
        return (1.0, [r.x, r.y]);
    }
    let scale = (r.w / world[0]).min(r.h / world[1]);
    (
        scale,
        [
            r.x + (r.w - world[0] * scale) / 2.0,
            r.y + (r.h - world[1] * scale) / 2.0,
        ],
    )
}

/// The color a simulation's `DrawKind` is drawn in inside a viewport.
fn kind_rgba(kind: sage_core::host::DrawKind) -> [u8; 4] {
    use sage_core::host::DrawKind;
    match kind {
        DrawKind::Court => [16, 20, 28, 255],
        DrawKind::Wall => [120, 130, 150, 255],
        DrawKind::Goal => [60, 30, 40, 255],
        DrawKind::Paddle => [235, 235, 240, 255],
        DrawKind::Ball => [255, 200, 60, 255],
    }
}

/// An embedded simulation's frame, fitted into the node's rectangle.
fn viewport(out: &mut Vec<UiDraw>, n: &Node, v: &ViewportNode, fade: f32) {
    let r = n.rect;
    let clip = n.clip;
    let Some(frame) = &v.frame else {
        return;
    };
    let (scale, origin) = viewport_fit(r, frame.world_size);
    let height = frame.world_size[1];
    for d in &frame.rects {
        let top_left = Rect {
            x: origin[0] + (d.center[0] - d.half[0]) * scale,
            y: origin[1] + (height - d.center[1] - d.half[1]) * scale,
            w: d.half[0] * 2.0 * scale,
            h: d.half[1] * 2.0 * scale,
        };
        rect(
            out,
            top_left.intersect(&r),
            faded(kind_rgba(d.kind), fade),
            clip,
        );
    }
    let text = Rect {
        x: r.x + 3.0,
        y: r.y + 2.0,
        w: (r.w - 6.0).max(0.0),
        h: (r.h - 4.0).max(0.0),
    };
    for (k, line) in frame.hud.iter().enumerate() {
        out.push(UiDraw::Text {
            style: 0,
            pos: [text.x, text.y + k as f32 * UI_LINE_HEIGHT],
            text: line.clone(),
            scale: 1.0,
            color: faded([235, 235, 240, 255], fade),
            clip: clip4(Some(clip.map_or(text, |c| c.intersect(&text)))),
        });
    }
}

/// The width of one strip of a gradient, in UI units.
const GRADIENT_STRIP: f32 = 8.0;

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round() as u8
}

/// sRGB bytes to OKLAB (Björn Ottosson's matrices).
fn to_oklab(c: [u8; 4]) -> [f32; 3] {
    let (r, g, b) = (
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
    );
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab(lab: [f32; 3], alpha: u8) -> [u8; 4] {
    let l = lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2];
    let m = lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2];
    let s = lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2];
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        linear_to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        linear_to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s),
        linear_to_srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        alpha,
    ]
}

/// `a` to `b` at `t`, mixed in OKLAB (alpha linearly). The ends are exact.
fn oklab_mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let (la, lb) = (to_oklab(a), to_oklab(b));
    let lab = std::array::from_fn(|i| la[i] + (lb[i] - la[i]) * t);
    let alpha = (a[3] as f32 + (b[3] as f32 - a[3] as f32) * t).round() as u8;
    from_oklab(lab, alpha)
}

fn outline(out: &mut Vec<UiDraw>, r: Rect, t: f32, color: [u8; 4], clip: Option<Rect>) {
    let edge = |x, y, w, h| Rect { x, y, w, h };
    rect(out, edge(r.x, r.y, r.w, t), color, clip);
    rect(out, edge(r.x, r.y + r.h - t, r.w, t), color, clip);
    rect(out, edge(r.x, r.y + t, t, r.h - 2.0 * t), color, clip);
    rect(
        out,
        edge(r.x + r.w - t, r.y + t, t, r.h - 2.0 * t),
        color,
        clip,
    );
}

/// The bar across the flow of a container that scrolls both ways.
fn scrollbar_cross(out: &mut Vec<UiDraw>, n: &Node, color: [u8; 4]) {
    let row = n.direction == Direction::Row;
    // Across a column is horizontal, across a row is vertical.
    let (start, len) = if row {
        (n.rect.y, n.rect.h)
    } else {
        (n.rect.x, n.rect.w)
    };
    let pad = if row {
        n.padding[1] + n.padding[3]
    } else {
        n.padding[0] + n.padding[2]
    };
    let view = len - pad;
    if n.content_cross <= view + 0.5 || view <= 0.0 {
        return;
    }
    let thumb = (view * view / n.content_cross).clamp(6.0, view);
    let t = (n.scroll_offset_cross / (n.content_cross - view)).clamp(0.0, 1.0);
    let at = start + t * (len - thumb);
    let r = if row {
        Rect {
            x: n.rect.x + n.rect.w - 3.0,
            y: at,
            w: 2.0,
            h: thumb,
        }
    } else {
        Rect {
            x: at,
            y: n.rect.y + n.rect.h - 3.0,
            w: thumb,
            h: 2.0,
        }
    };
    rect(out, r, color, n.clip);
}

fn scrollbar(out: &mut Vec<UiDraw>, n: &Node) {
    let row = n.direction == Direction::Row;
    let pad = if row {
        n.padding[0] + n.padding[2]
    } else {
        n.padding[1] + n.padding[3]
    };
    let (start, len) = if row {
        (n.rect.x, n.rect.w)
    } else {
        (n.rect.y, n.rect.h)
    };
    let view = len - pad;
    if n.content <= view + 0.5 || view <= 0.0 {
        if n.scroll_cross {
            scrollbar_cross(out, n, [200, 205, 215, 160]);
        }
        return;
    }
    let thumb = (view * view / n.content).clamp(6.0, view);
    let t = (n.scroll_offset / (n.content - view)).clamp(0.0, 1.0);
    let at = start + t * (len - thumb);
    // The accent colors the thumb; a node with no accent still gets a visible
    // bar, in a light neutral.
    let color = {
        let mut c = rgba(n.accent);
        if c[3] == 0 {
            c = [200, 205, 215, 255];
        }
        c[3] = c[3] / 2 + c[3] / 4;
        c
    };
    let r = if row {
        Rect {
            x: at,
            y: n.rect.y + n.rect.h - 3.0,
            w: thumb,
            h: 2.0,
        }
    } else {
        Rect {
            x: n.rect.x + n.rect.w - 3.0,
            y: at,
            w: 2.0,
            h: thumb,
        }
    };
    rect(out, r, color, n.clip);
    if n.scroll_cross {
        scrollbar_cross(out, n, color);
    }
}

fn clip4(c: Option<Rect>) -> Option<[f32; 4]> {
    c.map(|c| [c.x, c.y, c.x + c.w, c.y + c.h])
}

/// The draw command for an image inside `r` (the node's content box).
fn image(im: &ImageNode, r: Rect, fade: f32, clip: Option<Rect>) -> Option<UiDraw> {
    let fs = im.frame_size();
    let (cols, rows) = (im.cols.max(1), im.rows.max(1));
    let (col, row) = (im.frame % cols, (im.frame / cols) % rows);
    let uv = [
        col as f32 / cols as f32,
        row as f32 / rows as f32,
        (col + 1) as f32 / cols as f32,
        (row + 1) as f32 / rows as f32,
    ];
    let (mut pos, mut size) = ([r.x, r.y], [r.w, r.h]);
    let natural = fs[0] > 0.0 && fs[1] > 0.0;
    if natural {
        let contain = (r.w / fs[0]).min(r.h / fs[1]);
        let cover = (r.w / fs[0]).max(r.h / fs[1]);
        let scale = match im.fit {
            Fit::Stretch => None,
            Fit::Contain => Some(contain),
            Fit::Cover => Some(cover),
            Fit::None => Some(1.0),
            Fit::ScaleDown => Some(contain.min(1.0)),
        };
        if let Some(k) = scale {
            size = [fs[0] * k, fs[1] * k];
            pos = [r.x + (r.w - size[0]) / 2.0, r.y + (r.h - size[1]) / 2.0];
        }
    }
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    // Whatever sticks out of the box (cover, none) is cut off at the box.
    let clip = Some(clip.map_or(r, |c| c.intersect(&r)));
    Some(UiDraw::Image {
        asset: im.asset,
        pos,
        size,
        uv,
        tint: faded(rgba(im.tint), fade),
        clip: clip4(clip),
    })
}

fn node(out: &mut Vec<UiDraw>, n: &Node, fade: f32) {
    let r = n.rect;
    if n.clip
        .is_some_and(|c| r.intersect(&c).w <= 0.0 || r.intersect(&c).h <= 0.0)
    {
        return;
    }
    let col = n.shown.unwrap_or_else(|| resolve(n));
    let c4 = |c: u32| faded(rgba(c), fade);
    let fill = c4(col.fill);
    let border = c4(col.border);
    let clip = n.clip;
    let rounded = n.radius > 0.0 && n.gradient.is_none();
    let accent = if n.enabled {
        rgba(n.accent)
    } else {
        faded(rgba(n.accent), 0.5)
    };
    let accent = faded(accent, fade);
    // A focused rounded node rings itself with rounded layers; a square one
    // (or one with no fill to cover the ring's inside) gets the outline.
    let round_ring = rounded && n.focused && n.enabled && fill[3] > 0;
    // Shadow first, under everything.
    if let Some(sh) = n.shadow {
        out.push(UiDraw::Shadow {
            pos: [r.x, r.y + sh.dy],
            size: [r.w, r.h],
            radius: n.radius,
            blur: sh.blur,
            color: c4(sh.color),
            clip: clip4(clip),
        });
    }
    // Fill and border. A rounded border is the border color's rounded
    // rectangle with the fill's rounded rectangle inset inside it.
    if let Some(g) = n.gradient {
        let to = c4(g.to);
        if fill[3] > 0 || to[3] > 0 {
            // Presentation blends linearly between corners, which muddies the
            // middle of a gradient; so the gradient is cut into strips whose
            // end colors are mixed in OKLAB, and each strip blends linearly.
            let extent = if g.vertical { r.h } else { r.w };
            let steps = if fill == to {
                1
            } else {
                (extent / GRADIENT_STRIP).ceil().clamp(1.0, 32.0) as usize
            };
            for k in 0..steps {
                let (t0, t1) = (k as f32 / steps as f32, (k + 1) as f32 / steps as f32);
                let (a, b) = (oklab_mix(fill, to, t0), oklab_mix(fill, to, t1));
                let (pos, size, colors) = if g.vertical {
                    ([r.x, r.y + r.h * t0], [r.w, r.h * (t1 - t0)], [a, a, b, b])
                } else {
                    ([r.x + r.w * t0, r.y], [r.w * (t1 - t0), r.h], [a, b, b, a])
                };
                out.push(UiDraw::Gradient {
                    pos,
                    size,
                    colors,
                    clip: clip4(clip),
                });
            }
        }
    } else if rounded {
        // Layers, outermost first: the focus ring (1 unit), the border, the
        // fill. Each is a rounded rectangle inset by what lies outside it.
        let rw = if round_ring { 1.0 } else { 0.0 };
        let bw = if border[3] > 0 {
            n.border_width.min(r.w / 2.0).min(r.h / 2.0)
        } else {
            0.0
        };
        let mut layers = vec![];
        if round_ring {
            layers.push((0.0, accent));
        }
        if bw > rw {
            layers.push((rw, border));
        }
        if fill[3] > 0 || !layers.is_empty() {
            layers.push((bw.max(rw), fill));
        }
        for (inset, color) in layers {
            if color[3] == 0 {
                continue;
            }
            out.push(UiDraw::Round {
                pos: [r.x + inset, r.y + inset],
                size: [r.w - 2.0 * inset, r.h - 2.0 * inset],
                radius: (n.radius - inset).max(0.0),
                color,
                clip: clip4(clip),
            });
        }
    } else {
        rect(out, r, fill, clip);
    }
    if let Some(sl) = n.slice {
        out.push(UiDraw::Slice {
            asset: sl.asset,
            pos: [r.x, r.y],
            size: [r.w, r.h],
            insets: sl.insets,
            tint: c4(sl.tint),
            clip: clip4(clip),
        });
    }
    match n.widget {
        Widget::Slider | Widget::Meter if n.max != n.min => {
            let t = ((n.value - n.min) / (n.max - n.min)).clamp(0.0, 1.0);
            let fill = Rect {
                w: (r.w * t).round(),
                ..r
            };
            rect(out, fill, accent, clip);
        }
        Widget::Toggle if n.on => {
            let inset = (r.h.min(r.w) / 4.0).round().max(1.0);
            let mark = Rect {
                x: r.x + inset,
                y: r.y + inset,
                w: r.w - 2.0 * inset,
                h: r.h - 2.0 * inset,
            };
            rect(out, mark, accent, clip);
        }
        _ => {}
    }
    if let Some(v) = &n.viewport {
        viewport(out, n, v, fade);
    }
    if n.svg_error {
        rect(out, r, [255, 0, 255, 255], clip);
    }
    if let Some(svg) = &n.svg {
        let content = Rect {
            x: r.x + n.padding[0],
            y: r.y + n.padding[1],
            w: (r.w - n.padding[0] - n.padding[2]).max(0.0),
            h: (r.h - n.padding[1] - n.padding[3]).max(0.0),
        };
        let bounds = clip.map_or(r, |c| r.intersect(&c));
        for d in crate::svg::draw(
            svg,
            [content.x, content.y],
            [content.w, content.h],
            clip4(Some(bounds)),
        ) {
            out.push(match d {
                UiDraw::Triangle { pts, colors } => UiDraw::Triangle {
                    pts,
                    colors: colors.map(|c| faded(c, fade)),
                },
                other => other,
            });
        }
    }
    if let Some(sh) = &n.shape {
        shape(out, n, sh, fade);
    }
    if let Some(im) = &n.image {
        let content = Rect {
            x: r.x + n.padding[0],
            y: r.y + n.padding[1],
            w: (r.w - n.padding[0] - n.padding[2]).max(0.0),
            h: (r.h - n.padding[1] - n.padding[3]).max(0.0),
        };
        if let Some(d) = image(im, content, fade, clip) {
            out.push(d);
        }
    }
    if n.border_width > 0.0 && !(rounded && border[3] > 0) {
        outline(out, r, n.border_width, border, clip);
    }
    if n.focused && n.enabled && !round_ring {
        outline(out, r, 1.0, accent, clip);
    }
    // A text input shows its text inside its own box, never outside it.
    let text_clip = if n.widget == Widget::TextInput {
        Some(n.clip.map_or(r, |c| c.intersect(&r)))
    } else {
        n.clip
    };
    if n.widget == Widget::TextInput {
        text_field(out, n, c4(col.text), c4(n.accent), text_clip);
    } else if n.rich.is_some() {
        selection_tint(out, n, accent, text_clip);
        rich_text(out, n, &c4, text_clip);
    } else if !n.lines.is_empty() {
        selection_tint(out, n, accent, text_clip);
        let color = c4(col.text);
        let visible = text_clip.is_none_or(|c| c.w > 0.0 && c.h > 0.0);
        if visible {
            out.push(UiDraw::Text {
                style: 0,
                pos: [r.x + n.padding[0], r.y + n.padding[1]],
                text: n.lines.clone(),
                scale: n.text_scale,
                color,
                clip: clip4(text_clip),
            });
        }
    }
}

/// The tint behind the selected text of a selectable node.
fn selection_tint(out: &mut Vec<UiDraw>, n: &Node, accent: [u8; 4], clip: Option<Rect>) {
    if !n.selectable || n.field.anchor == n.caret {
        return;
    }
    let len = n.text.chars().count();
    let (a, c) = (n.field.anchor.min(len), n.caret.min(len));
    let (lo, hi) = (a.min(c), a.max(c));
    let s = n.text_scale;
    let (adv, lh) = (UI_ADVANCE * s, UI_LINE_HEIGHT * s);
    let (x0, y0) = (n.rect.x + n.padding[0], n.rect.y + n.padding[1]);
    let cols = chars_per_line(n.rect.w - n.padding[0] - n.padding[2], s);
    let mut tint = accent;
    tint[3] = 0x66;
    for (row, rw) in rows(&n.text, cols, n.wrap).iter().enumerate() {
        let (from, to) = (lo.max(rw.start), hi.min(rw.end));
        if from < to {
            let r = Rect {
                x: x0 + (from - rw.start) as f32 * adv,
                y: y0 + row as f32 * lh,
                w: (to - from) as f32 * adv,
                h: lh,
            };
            rect(out, r, tint, clip);
        }
    }
}

/// Rich text: the wrapped rows drawn as runs of equal style. Code gets a
/// tinted box, a link its accent color and an underline, bold is the glyphs
/// drawn twice one unit apart (the only face is a bitmap).
fn rich_text(out: &mut Vec<UiDraw>, n: &Node, c4: &dyn Fn(u32) -> [u8; 4], clip: Option<Rect>) {
    let Some(rich) = &n.rich else { return };
    let r = n.rect;
    let s = n.text_scale;
    let (adv, lh) = (UI_ADVANCE * s, UI_LINE_HEIGHT * s);
    let (x0, y0) = (r.x + n.padding[0], r.y + n.padding[1]);
    let base = n.shown.unwrap_or_else(|| resolve(n)).text;
    let chars: Vec<char> = n.text.chars().collect();
    let rs = rows(
        &n.text,
        chars_per_line(r.w - n.padding[0] - n.padding[2], s),
        n.wrap,
    );
    for (row, rw) in rs.iter().enumerate() {
        let y = y0 + row as f32 * lh;
        // Runs of equal style within the row, ignoring the trailing break.
        let mut k = rw.start;
        while k < rw.end {
            let st = rich.styles.get(k).copied().unwrap_or_default();
            let mut e = k;
            while e < rw.end && rich.styles.get(e).copied().unwrap_or_default() == st {
                e += 1;
            }
            let text: String = chars[k..e].iter().collect();
            let x = x0 + (k - rw.start) as f32 * adv;
            let color = if st.color != 0 {
                c4(st.color)
            } else if st.link != 0 {
                c4(n.accent)
            } else {
                c4(base)
            };
            if st.code {
                let w = (e - k) as f32 * adv;
                rect(
                    out,
                    Rect {
                        x,
                        y: y - s,
                        w,
                        h: UI_GLYPH[1] * s + 2.0 * s,
                    },
                    [255, 255, 255, 0x1E],
                    clip,
                );
            }
            let shown = text.trim_end().to_string();
            if !shown.is_empty() {
                // Bold and italic are a style: a face with styled glyphs draws
                // them, and the built-in face strikes bold twice.
                out.push(UiDraw::Text {
                    pos: [x, y],
                    text: shown.clone(),
                    scale: s,
                    color,
                    clip: clip4(clip),
                    style: (u8::from(st.bold) * STYLE_BOLD) | (u8::from(st.italic) * STYLE_ITALIC),
                });
            }
            if st.link != 0 {
                let w = shown.chars().count() as f32 * adv;
                rect(
                    out,
                    Rect {
                        x,
                        y: y + UI_GLYPH[1] * s + s,
                        w,
                        h: s.max(1.0),
                    },
                    color,
                    clip,
                );
            }
            k = e;
        }
    }
}

/// A text input: its text (masked for a password), a placeholder while it is
/// empty, the selection, and the caret. One line scrolls sideways by its
/// `view` columns, several lines wrap and scroll by rows.
fn text_field(
    out: &mut Vec<UiDraw>,
    n: &Node,
    color: [u8; 4],
    accent: [u8; 4],
    clip: Option<Rect>,
) {
    let r = n.rect;
    let s = n.text_scale;
    let (adv, lh) = (UI_ADVANCE * s, UI_LINE_HEIGHT * s);
    let (x0, y0) = (r.x + n.padding[0], r.y + n.padding[1]);
    let multi = n.field.multiline;
    let shown = if n.field.masked {
        mask(&n.text)
    } else {
        n.text.clone()
    };
    let cols = chars_per_line(r.w - n.padding[0] - n.padding[2], s);
    let rs = rows(&shown, cols, multi);
    let vis = (((r.h - n.padding[1] - n.padding[3]) / lh).floor() as usize).max(1);
    // Where a character column of a row is drawn.
    let (first_row, xview) = if multi {
        (n.field.view, 0)
    } else {
        (0, n.field.view)
    };
    let at = |row: usize, col: usize| {
        [
            x0 + (col as f32 - xview as f32) * adv,
            y0 + (row as f32 - first_row as f32) * lh,
        ]
    };
    if n.text.is_empty() {
        if !n.field.placeholder.is_empty() {
            let mut dim = color;
            dim[3] /= 2;
            out.push(UiDraw::Text {
                style: 0,
                pos: [x0, y0],
                text: n.field.placeholder.clone(),
                scale: s,
                color: dim,
                clip: clip4(clip),
            });
        }
    } else {
        let last = if multi {
            (first_row + vis).min(rs.len())
        } else {
            rs.len()
        };
        for (row, rw) in rs.iter().enumerate().take(last).skip(first_row) {
            let line: String = shown
                .chars()
                .skip(rw.start)
                .take(rw.end - rw.start)
                .collect();
            out.push(UiDraw::Text {
                style: 0,
                pos: at(row, 0),
                text: line.trim_end_matches(['\n']).to_string(),
                scale: s,
                color,
                clip: clip4(clip),
            });
        }
    }
    if !(n.focused && n.enabled) {
        return;
    }
    let (lo, hi) = {
        let len = n.text.chars().count();
        let (a, c) = (n.field.anchor.min(len), n.caret.min(len));
        (a.min(c), a.max(c))
    };
    if lo != hi {
        let mut tint = accent;
        tint[3] = 0x66;
        for (row, rw) in rs.iter().enumerate() {
            let (a, b) = (lo.max(rw.start), hi.min(rw.end));
            if a >= b || (multi && (row < first_row || row >= first_row + vis)) {
                continue;
            }
            let [x, y] = at(row, a - rw.start);
            let w = (b - a) as f32 * adv;
            rect(out, Rect { x, y, w, h: lh }, tint, clip);
        }
    }
    let (row, col) = locate(&rs, n.caret.min(n.text.chars().count()));
    let [x, y] = at(row, col);
    let bar = Rect {
        x: x - (s / 2.0).floor(),
        y,
        w: s.max(1.0),
        h: UI_GLYPH[1] * s,
    };
    rect(out, bar, color, clip);
    // An input method's pre-edit text sits at the caret, underlined, over
    // what follows until it is committed.
    if !n.field.preedit.is_empty() {
        let width = n.field.preedit.chars().count() as f32 * adv;
        let mut back = accent;
        back[3] = 0x33;
        rect(
            out,
            Rect {
                x,
                y,
                w: width,
                h: lh,
            },
            back,
            clip,
        );
        out.push(UiDraw::Text {
            style: 0,
            pos: [x, y],
            text: n.field.preedit.clone(),
            scale: s,
            color,
            clip: clip4(clip),
        });
        rect(
            out,
            Rect {
                x,
                y: y + UI_GLYPH[1] * s + (s / 2.0).floor(),
                w: width,
                h: s.max(1.0),
            },
            color,
            clip,
        );
    }
}

/// Draw commands back to front for the whole tree. A scroll container's
/// scrollbar is drawn right after the last of its descendants.
pub fn draw(tree: &Tree, size: [f32; 2]) -> UiFrame {
    let order = tree.draw_order();
    let fades = tree.effective_fade();
    let mut pos = vec![usize::MAX; tree.nodes.len()];
    for (k, &i) in order.iter().enumerate() {
        pos[i] = k;
    }
    let mut last = vec![0usize; tree.nodes.len()];
    for &j in &order {
        let mut at = Some(j);
        for _ in 0..64 {
            let Some(a) = at else { break };
            if tree.nodes[a].scroll {
                last[a] = last[a].max(pos[j]);
            }
            at = tree.nodes[a]
                .parent
                .filter(|&p| p < tree.nodes.len() && p != a);
        }
    }
    let mut bars_after: Vec<Vec<usize>> = vec![vec![]; order.len()];
    for &i in &order {
        if tree.nodes[i].scroll {
            bars_after[last[i]].push(i);
        }
    }
    let mut out = vec![];
    for (k, &i) in order.iter().enumerate() {
        node(&mut out, &tree.nodes[i], fades[i]);
        for &bar in &bars_after[k] {
            scrollbar(&mut out, &tree.nodes[bar]);
        }
    }
    UiFrame { size, draws: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Size, layout};

    fn texts(f: &UiFrame) -> Vec<&str> {
        f.draws
            .iter()
            .filter_map(|d| match d {
                UiDraw::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_tree_draws_back_to_front_with_state_tints() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    id: 1,
                    width: Size::Fixed(50.0),
                    height: Size::Fixed(20.0),
                    fill: 0x1020_30ff,
                    padding: [2.0; 4],
                    ..Node::default()
                },
                Node {
                    id: 2,
                    parent: Some(0),
                    width: Size::Fixed(30.0),
                    height: Size::Fixed(10.0),
                    widget: Widget::Button,
                    fill: 0x4040_40ff,
                    text: "Go".into(),
                    ..Node::default()
                },
            ],
        };
        layout(&mut t, [50.0, 20.0]);
        let f = draw(&t, [50.0, 20.0]);
        assert_eq!(
            f.draws,
            vec![
                UiDraw::Rect {
                    pos: [0.0, 0.0],
                    size: [50.0, 20.0],
                    color: [0x10, 0x20, 0x30, 0xff]
                },
                UiDraw::Rect {
                    pos: [2.0, 2.0],
                    size: [30.0, 10.0],
                    color: [0x40, 0x40, 0x40, 0xff]
                },
                UiDraw::Text {
                    style: 0,
                    pos: [2.0, 2.0],
                    text: "Go".into(),
                    scale: 1.0,
                    color: [255; 4],
                    clip: None,
                },
            ]
        );
        t.nodes[1].hovered = true;
        let hovered = draw(&t, [50.0, 20.0]);
        assert_eq!(
            hovered.draws[1],
            UiDraw::Rect {
                pos: [2.0, 2.0],
                size: [30.0, 10.0],
                color: [0x50, 0x50, 0x50, 0xff]
            }
        );
        t.nodes[1].pressed = true;
        t.nodes[1].enabled = false;
        let disabled = draw(&t, [50.0, 20.0]);
        let UiDraw::Rect { color, .. } = disabled.draws[1] else {
            panic!()
        };
        assert_eq!(color[3], 0x7f, "disabled widgets are dimmed, not tinted");
    }

    #[test]
    fn sliders_toggles_and_focus_rings() {
        let mut t = Tree {
            nodes: vec![Node {
                id: 1,
                width: Size::Fixed(40.0),
                height: Size::Fixed(8.0),
                widget: Widget::Slider,
                fill: 0x0000_00ff,
                accent: 0xff00_00ff,
                min: 0.0,
                max: 10.0,
                value: 5.0,
                focused: true,
                ..Node::default()
            }],
        };
        layout(&mut t, [40.0, 8.0]);
        let f = draw(&t, [40.0, 8.0]);
        // Track, filled half, then four focus-ring edges.
        assert_eq!(f.draws.len(), 2 + 4);
        assert_eq!(
            f.draws[1],
            UiDraw::Rect {
                pos: [0.0, 0.0],
                size: [20.0, 8.0],
                color: [255, 0, 0, 255]
            }
        );
    }

    #[test]
    fn content_outside_a_scroll_container_is_cut_and_a_scrollbar_is_drawn() {
        let mut nodes = vec![Node {
            id: 1,
            width: Size::Fixed(60.0),
            height: Size::Fixed(30.0),
            scroll: true,
            scroll_offset: 10.0,
            accent: 0xff00_00ff,
            ..Node::default()
        }];
        for k in 0..3u64 {
            nodes.push(Node {
                id: 10 + k,
                parent: Some(0),
                width: Size::Fixed(60.0),
                height: Size::Fixed(20.0),
                fill: 0x0000_ffff,
                text: format!("row {k}"),
                ..Node::default()
            });
        }
        let mut t = Tree { nodes };
        layout(&mut t, [100.0, 100.0]);
        let f = draw(&t, [100.0, 100.0]);
        // Rows are at y -10..10, 10..30, 30..50; the view is 0..30.
        let rects: Vec<([f32; 2], [f32; 2])> = f
            .draws
            .iter()
            .filter_map(|d| match d {
                UiDraw::Rect { pos, size, color } if color[2] == 255 => Some((*pos, *size)),
                _ => None,
            })
            .collect();
        assert_eq!(
            rects,
            vec![([0.0, 0.0], [60.0, 10.0]), ([0.0, 10.0], [60.0, 20.0])],
            "the first row is cut, the third is gone"
        );
        // The text of the cut-off row carries the clip; the hidden row has none.
        let clips: Vec<(&str, Option<[f32; 4]>)> = f
            .draws
            .iter()
            .filter_map(|d| match d {
                UiDraw::Text { text, clip, .. } => Some((text.as_str(), *clip)),
                _ => None,
            })
            .collect();
        assert_eq!(clips.len(), 2, "{clips:?}");
        assert_eq!(clips[0].1, Some([0.0, 0.0, 60.0, 30.0]));
        // The scrollbar is the last thing drawn: 30 / 60 of the track, a
        // sixth of the way along it.
        let Some(UiDraw::Rect { pos, size, color }) = f.draws.last() else {
            panic!("no scrollbar")
        };
        assert_eq!(color[0], 255);
        assert_eq!(size, &[2.0, 15.0]);
        assert_eq!(
            pos,
            &[57.0, 5.0],
            "offset 10 of 30 is a third of the free track"
        );
    }

    #[test]
    fn a_text_input_draws_its_caret_inside_its_box() {
        let mut t = Tree {
            nodes: vec![Node {
                id: 1,
                width: Size::Fixed(40.0),
                height: Size::Fixed(12.0),
                padding: [3.0, 2.0, 3.0, 2.0],
                widget: Widget::TextInput,
                text: "abcdefghijklmnopq".into(),
                caret: 2,
                focused: true,
                ..Node::default()
            }],
        };
        layout(&mut t, [40.0, 12.0]);
        let f = draw(&t, [40.0, 12.0]);
        let UiDraw::Text { clip, .. } = f
            .draws
            .iter()
            .find(|d| matches!(d, UiDraw::Text { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            *clip,
            Some([0.0, 0.0, 40.0, 12.0]),
            "long text is cut at the box"
        );
        // The caret bar: after two glyph cells, one unit wide, a glyph tall.
        assert!(
            f.draws.contains(&UiDraw::Rect {
                pos: [3.0 + 12.0, 2.0],
                size: [1.0, 7.0],
                color: [255; 4]
            }),
            "{:?}",
            f.draws
        );
        t.nodes[0].focused = false;
        let f = draw(&t, [40.0, 12.0]);
        assert!(
            !f.draws
                .iter()
                .any(|d| matches!(d, UiDraw::Rect { size, .. } if size[0] == 1.0))
        );
    }

    #[test]
    fn a_meter_fills_like_a_slider_and_wrapped_lines_are_drawn() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    id: 1,
                    width: Size::Fixed(40.0),
                    height: Size::Fixed(6.0),
                    widget: Widget::Meter,
                    fill: 0x1111_11ff,
                    accent: 0xff80_00ff,
                    min: 0.0,
                    max: 10.0,
                    value: 2.5,
                    ..Node::default()
                },
                Node {
                    id: 2,
                    width: Size::Fixed(65.0),
                    height: Size::Auto,
                    text: "the quick brown fox".into(),
                    wrap: true,
                    ..Node::default()
                },
            ],
        };
        layout(&mut t, [100.0, 100.0]);
        let f = draw(&t, [100.0, 100.0]);
        assert!(f.draws.contains(&UiDraw::Rect {
            pos: [0.0, 0.0],
            size: [10.0, 6.0],
            color: [255, 128, 0, 255]
        }));
        assert_eq!(texts(&f), vec!["the quick\nbrown fox"]);
    }

    #[test]
    fn a_scrollbar_is_visible_even_without_an_accent() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    id: 1,
                    width: Size::Fixed(30.0),
                    height: Size::Fixed(20.0),
                    scroll: true,
                    ..Node::default()
                },
                Node {
                    id: 2,
                    parent: Some(0),
                    width: Size::Fixed(30.0),
                    height: Size::Fixed(60.0),
                    ..Node::default()
                },
            ],
        };
        t.nodes[0].accent = 0;
        layout(&mut t, [50.0, 50.0]);
        let f = draw(&t, [50.0, 50.0]);
        let Some(UiDraw::Rect { color, .. }) = f.draws.last() else {
            panic!("no scrollbar drawn: {:?}", f.draws)
        };
        assert!(color[3] > 0, "{color:?}");
    }

    fn one(n: Node, w: f32, h: f32) -> Vec<UiDraw> {
        let mut t = Tree { nodes: vec![n] };
        layout(&mut t, [w, h]);
        draw(&t, [w, h]).draws
    }

    fn img(fit: Fit) -> ImageNode {
        ImageNode {
            asset: 5,
            fit,
            tint: 0xffff_ffff,
            size: [40.0, 20.0],
            frame: 0,
            cols: 1,
            rows: 1,
        }
    }

    type ImageParts<'a> = (
        &'a [f32; 2],
        &'a [f32; 2],
        &'a [f32; 4],
        &'a Option<[f32; 4]>,
    );

    fn the_image(draws: &[UiDraw]) -> ImageParts<'_> {
        draws
            .iter()
            .find_map(|d| match d {
                UiDraw::Image {
                    pos,
                    size,
                    uv,
                    clip,
                    ..
                } => Some((pos, size, uv, clip)),
                _ => None,
            })
            .expect("an image draw")
    }

    #[test]
    fn image_fit_modes_place_the_image_in_its_box() {
        // A 100 x 100 box holding a 40 x 20 image.
        let make = |fit| Node {
            width: Size::Fixed(100.0),
            height: Size::Fixed(100.0),
            image: Some(img(fit)),
            ..Node::default()
        };
        let d = one(make(Fit::Stretch), 200.0, 200.0);
        let (pos, size, _, _) = the_image(&d);
        assert_eq!((*pos, *size), ([0.0, 0.0], [100.0, 100.0]));
        let d = one(make(Fit::Contain), 200.0, 200.0);
        let (pos, size, _, _) = the_image(&d);
        assert_eq!(
            (*pos, *size),
            ([0.0, 25.0], [100.0, 50.0]),
            "scaled to the width, centered"
        );
        let d = one(make(Fit::Cover), 200.0, 200.0);
        let (pos, size, _, clip) = the_image(&d);
        assert_eq!(
            (*pos, *size),
            ([-50.0, 0.0], [200.0, 100.0]),
            "scaled to the height, centered"
        );
        assert_eq!(
            *clip,
            Some([0.0, 0.0, 100.0, 100.0]),
            "what sticks out is cut at the box"
        );
        let d = one(make(Fit::None), 200.0, 200.0);
        let (pos, size, _, _) = the_image(&d);
        assert_eq!((*pos, *size), ([30.0, 40.0], [40.0, 20.0]));
        // ScaleDown never enlarges.
        let d = one(make(Fit::ScaleDown), 200.0, 200.0);
        assert_eq!(*the_image(&d).1, [40.0, 20.0]);
        let mut small = make(Fit::ScaleDown);
        small.width = Size::Fixed(20.0);
        small.height = Size::Fixed(20.0);
        let d = one(small, 200.0, 200.0);
        assert_eq!(*the_image(&d).1, [20.0, 10.0], "shrunk to fit");
    }

    #[test]
    fn a_sprite_sheet_frame_selects_its_part_of_the_image() {
        let mut im = img(Fit::Stretch);
        im.cols = 4;
        im.rows = 2;
        im.frame = 5; // column 1, row 1
        let d = one(
            Node {
                image: Some(im),
                ..Node::default()
            },
            100.0,
            100.0,
        );
        assert_eq!(*the_image(&d).2, [0.25, 0.5, 0.5, 1.0]);
        im.frame = 9; // wraps: 9 % 4 = column 1, (9 / 4) % 2 = row 0
        let d = one(
            Node {
                image: Some(im),
                ..Node::default()
            },
            100.0,
            100.0,
        );
        assert_eq!(*the_image(&d).2, [0.25, 0.0, 0.5, 0.5]);
    }

    #[test]
    fn oklab_mixes_are_perceptual_not_naive() {
        let (black, white) = ([0, 0, 0, 255], [255, 255, 255, 255]);
        // Half way in lightness is about 99, not the naive 128.
        let mid = oklab_mix(black, white, 0.5);
        assert!(
            (97..=101).contains(&mid[0]) && mid[0] == mid[1] && mid[1] == mid[2],
            "{mid:?}"
        );
        assert_eq!(oklab_mix(black, white, 0.0), black);
        assert_eq!(oklab_mix(black, white, 1.0), white);
        // Colors round trip, and alpha blends linearly.
        let c = [200, 30, 90, 255];
        assert_eq!(from_oklab(to_oklab(c), 255), c);
        assert_eq!(oklab_mix([9, 9, 9, 0], [9, 9, 9, 100], 0.5)[3], 50);
    }

    #[test]
    fn a_focused_rounded_node_has_a_rounded_ring_not_a_square_one() {
        let d = one(
            Node {
                width: Size::Fixed(40.0),
                height: Size::Fixed(20.0),
                fill: 0x1020_30ff,
                accent: 0x00ff_00ff,
                radius: 4.0,
                focused: true,
                ..Node::default()
            },
            100.0,
            100.0,
        );
        // Accent Round, then the fill inset by the ring: no square Rect.
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(matches!(&d[0], UiDraw::Round { size, radius, color, .. }
            if *size == [40.0, 20.0] && *radius == 4.0 && *color == [0, 255, 0, 255]));
        assert!(
            matches!(&d[1], UiDraw::Round { pos, size, radius, color, .. }
            if *pos == [1.0, 1.0] && *size == [38.0, 18.0] && *radius == 3.0 && *color == [0x10, 0x20, 0x30, 255])
        );
    }

    #[test]
    fn rounded_fills_borders_shadows_gradients_and_slices_are_their_own_commands() {
        let base = Node {
            width: Size::Fixed(40.0),
            height: Size::Fixed(20.0),
            fill: 0x1020_30ff,
            ..Node::default()
        };
        // A rounded fill is one Round.
        let d = one(
            Node {
                radius: 4.0,
                ..base.clone()
            },
            100.0,
            100.0,
        );
        assert_eq!(
            d,
            vec![UiDraw::Round {
                pos: [0.0, 0.0],
                size: [40.0, 20.0],
                radius: 4.0,
                color: [0x10, 0x20, 0x30, 0xff],
                clip: None
            }]
        );
        // With a border: the border color's Round, then the fill's inset by the width.
        let d = one(
            Node {
                radius: 4.0,
                border: 0xff00_00ff,
                border_width: 2.0,
                ..base.clone()
            },
            100.0,
            100.0,
        );
        assert_eq!(d.len(), 2);
        assert!(
            matches!(&d[0], UiDraw::Round { color, radius, .. } if *color == [255, 0, 0, 255] && *radius == 4.0)
        );
        assert!(
            matches!(&d[1], UiDraw::Round { pos, size, radius, color, .. }
            if *pos == [2.0, 2.0] && *size == [36.0, 16.0] && *radius == 2.0 && *color == [0x10, 0x20, 0x30, 0xff])
        );
        // A shadow comes first, offset down by dy.
        let d = one(
            Node {
                radius: 3.0,
                shadow: Some(crate::ShadowNode {
                    blur: 5.0,
                    dy: 2.0,
                    color: 0x0000_00aa,
                }),
                ..base.clone()
            },
            100.0,
            100.0,
        );
        assert!(matches!(&d[0], UiDraw::Shadow { pos, blur, color, .. }
            if *pos == [0.0, 2.0] && *blur == 5.0 && *color == [0, 0, 0, 0xaa]));
        assert!(matches!(&d[1], UiDraw::Round { .. }));
        // A vertical gradient: fill on top, `to` at the bottom.
        let d = one(
            Node {
                gradient: Some(crate::GradientNode {
                    to: 0xff00_00ff,
                    vertical: true,
                }),
                ..base.clone()
            },
            100.0,
            100.0,
        );
        let top = [0x10, 0x20, 0x30, 0xff];
        let bottom = [255, 0, 0, 255];
        // Strips from top to bottom, each blending linearly between OKLAB
        // mixes, so the ends are exact and the strips meet.
        type Strip = ([f32; 2], [f32; 2], [[u8; 4]; 4]);
        let strips: Vec<Strip> = d
            .iter()
            .map(|c| match c {
                UiDraw::Gradient {
                    pos, size, colors, ..
                } => (*pos, *size, *colors),
                other => panic!("{other:?}"),
            })
            .collect();
        assert!(strips.len() > 1);
        assert_eq!(strips[0].2[0], top);
        assert_eq!(strips.last().unwrap().2[3], bottom);
        for w in strips.windows(2) {
            assert_eq!(w[0].2[3], w[1].2[0], "strips share an edge color");
            assert!((w[0].0[1] + w[0].1[1] - w[1].0[1]).abs() < 1e-3);
        }
        // A 9-slice background is drawn over the fill.
        let d = one(
            Node {
                slice: Some(crate::SliceNode {
                    asset: 9,
                    insets: [3.0; 4],
                    tint: 0xffff_ffff,
                }),
                ..base
            },
            100.0,
            100.0,
        );
        assert!(matches!(&d[0], UiDraw::Rect { .. }));
        assert!(matches!(&d[1], UiDraw::Slice { asset: 9, insets, .. } if *insets == [3.0; 4]));
    }

    #[test]
    fn fade_scales_every_color_of_a_node_and_its_children() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    id: 1,
                    width: Size::Fixed(20.0),
                    height: Size::Fixed(20.0),
                    fill: 0xff00_00ff,
                    fade: 0.5,
                    ..Node::default()
                },
                Node {
                    id: 2,
                    parent: Some(0),
                    width: Size::Fixed(10.0),
                    height: Size::Fixed(10.0),
                    fill: 0x00ff_00ff,
                    text: "A".into(),
                    ..Node::default()
                },
            ],
        };
        layout(&mut t, [50.0, 50.0]);
        let f = draw(&t, [50.0, 50.0]);
        let alphas: Vec<u8> = f
            .draws
            .iter()
            .map(|d| match d {
                UiDraw::Rect { color, .. } | UiDraw::Text { color, .. } => color[3],
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            alphas,
            vec![128, 128, 128],
            "parent fill, child fill and child text"
        );
    }

    #[test]
    fn a_hidden_node_draws_nothing() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    width: Size::Fixed(10.0),
                    height: Size::Fixed(10.0),
                    fill: 0xffff_ffff,
                    ..Node::default()
                },
                Node {
                    parent: Some(0),
                    hidden: true,
                    width: Size::Fixed(5.0),
                    height: Size::Fixed(5.0),
                    fill: 0xff00_00ff,
                    ..Node::default()
                },
            ],
        };
        layout(&mut t, [50.0, 50.0]);
        assert_eq!(draw(&t, [50.0, 50.0]).draws.len(), 1);
    }

    #[test]
    fn a_shown_transition_state_overrides_the_resolved_colors() {
        let mut n = Node {
            width: Size::Fixed(10.0),
            height: Size::Fixed(10.0),
            fill: 0x0000_00ff,
            ..Node::default()
        };
        n.shown = Some(crate::Resolved {
            fill: 0x8080_80ff,
            text: 0xffff_ffff,
            border: 0,
            fade: 0.0,
        });
        let d = one(n, 20.0, 20.0);
        assert!(matches!(&d[0], UiDraw::Rect { color, .. } if *color == [0x80, 0x80, 0x80, 0xff]));
    }

    fn corners(f: &UiFrame) -> Vec<[f32; 2]> {
        f.draws
            .iter()
            .flat_map(|d| match d {
                UiDraw::Triangle { pts, .. } => pts.to_vec(),
                _ => vec![],
            })
            .collect()
    }

    #[test]
    fn a_polygon_is_cut_to_a_rectangle_and_a_fan_covers_what_is_left() {
        let r = [0.0, 0.0, 10.0, 10.0];
        // Entirely inside: unchanged. Entirely outside: gone.
        let tri = [[1.0, 1.0], [9.0, 1.0], [1.0, 9.0]];
        let mut same = clip_polygon(&tri, r);
        same.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(same, [[1.0, 1.0], [1.0, 9.0], [9.0, 1.0]]);
        assert!(clip_polygon(&[[20.0, 20.0], [30.0, 20.0], [20.0, 30.0]], r).is_empty());
        // Across the right edge: every point lands inside, and the area is
        // the part that was inside (a 10x10 square less a corner).
        let big = [[5.0, -5.0], [25.0, -5.0], [25.0, 15.0], [5.0, 15.0]];
        let cut = clip_polygon(&big, r);
        assert!(
            cut.iter()
                .all(|p| (0.0..=10.0).contains(&p[0]) && (0.0..=10.0).contains(&p[1]))
        );
        let area: f32 = (0..cut.len())
            .map(|k| {
                let (a, b) = (cut[k], cut[(k + 1) % cut.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f32>()
            / 2.0;
        assert!((area.abs() - 50.0).abs() < 1e-3, "{area}");
    }

    #[test]
    fn a_line_is_a_thick_quad_and_a_shape_never_leaves_its_node() {
        let canvas = |shape: ShapeNode| {
            let mut t = Tree {
                nodes: vec![Node {
                    id: 1,
                    width: Size::Fixed(20.0),
                    height: Size::Fixed(10.0),
                    shape: Some(shape),
                    ..Node::default()
                }],
            };
            layout(&mut t, [20.0, 10.0]);
            draw(&t, [20.0, 10.0])
        };
        let line = canvas(ShapeNode {
            kind: 1,
            pts: [[2.0, 5.0], [12.0, 5.0], [0.0; 2]],
            width: 4.0,
            color: 0xff00_00ff,
        });
        let c = corners(&line);
        assert_eq!(c.len(), 6, "a quad is two triangles");
        let (lo, hi) = (
            c.iter().map(|p| p[1]).fold(f32::MAX, f32::min),
            c.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
        );
        assert_eq!((lo, hi), (3.0, 7.0), "four units thick, centered on y 5");
        // A line that runs far outside is cut at the box.
        let out = canvas(ShapeNode {
            kind: 1,
            pts: [[0.0, 5.0], [500.0, 5.0], [0.0; 2]],
            width: 20.0,
            color: 0xff00_00ff,
        });
        assert!(
            corners(&out)
                .iter()
                .all(|p| p[0] <= 20.0 && (0.0..=10.0).contains(&p[1]))
        );
        // An ellipse is a fan; a transparent shape draws nothing.
        let ellipse = canvas(ShapeNode {
            kind: 3,
            pts: [[10.0, 5.0], [4.0, 4.0], [0.0; 2]],
            width: 0.0,
            color: 0x00ff_00ff,
        });
        assert!(corners(&ellipse).len() >= 30);
        let clear = canvas(ShapeNode {
            kind: 2,
            pts: [[0.0; 2], [5.0, 0.0], [0.0, 5.0]],
            width: 0.0,
            color: 0xff00_0000,
        });
        assert!(clear.draws.is_empty());
    }

    #[test]
    fn a_viewport_letterboxes_the_world_and_draws_it_y_up() {
        use sage_core::StableId;
        use sage_core::host::{DrawKind, DrawRect, Frame};
        let r = Rect {
            x: 10.0,
            y: 20.0,
            w: 200.0,
            h: 100.0,
        };
        // A 100x100 world in a 200x100 box: scale 1, centered sideways.
        assert_eq!(viewport_fit(r, [100.0, 100.0]), (1.0, [60.0, 20.0]));
        assert_eq!(viewport_fit(r, [400.0, 100.0]), (0.5, [10.0, 45.0]));
        let frame = Frame {
            tick: 0,
            world_size: [100.0, 100.0],
            rects: vec![DrawRect {
                id: StableId(1),
                kind: DrawKind::Ball,
                center: [10.0, 10.0],
                half: [5.0, 5.0],
            }],
            hud: vec!["HI".into()],
        };
        let mut t = Tree {
            nodes: vec![Node {
                id: 1,
                width: Size::Fixed(200.0),
                height: Size::Fixed(100.0),
                viewport: Some(ViewportNode {
                    frame: Some(frame),
                    ..ViewportNode::default()
                }),
                ..Node::default()
            }],
        };
        layout(&mut t, [200.0, 100.0]);
        let f = draw(&t, [200.0, 100.0]);
        // The world is centered (50 units in from each side) and y points up:
        // the ball spans world y 5..15, so its top edge is 85 from the top.
        assert!(
            f.draws.contains(&UiDraw::Rect {
                pos: [55.0, 85.0],
                size: [10.0, 10.0],
                color: [255, 200, 60, 255],
            }),
            "{:?}",
            f.draws
        );
        assert_eq!(texts(&f), vec!["HI"]);
    }
}
