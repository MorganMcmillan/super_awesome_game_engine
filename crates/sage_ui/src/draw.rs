//! Drawing: a laid-out, interacted tree becomes a [`UiFrame`], the plain
//! data presentation consumes (`sage_core::host`). The UI itself never
//! touches a renderer.

use sage_core::host::{UI_ADVANCE, UI_GLYPH, UiDraw, UiFrame};

use crate::layout::{Direction, Fit, ImageNode, Node, Rect, Tree, Widget, rgba};
use crate::style::{faded, resolve};

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
        let colors = if g.vertical {
            [fill, fill, to, to]
        } else {
            [fill, to, to, fill]
        };
        if fill[3] > 0 || to[3] > 0 {
            out.push(UiDraw::Gradient {
                pos: [r.x, r.y],
                size: [r.w, r.h],
                colors,
                clip: clip4(clip),
            });
        }
    } else if rounded {
        let bw = n.border_width.min(r.w / 2.0).min(r.h / 2.0);
        if bw > 0.0 && border[3] > 0 {
            out.push(UiDraw::Round {
                pos: [r.x, r.y],
                size: [r.w, r.h],
                radius: n.radius,
                color: border,
                clip: clip4(clip),
            });
            if fill[3] > 0 {
                out.push(UiDraw::Round {
                    pos: [r.x + bw, r.y + bw],
                    size: [r.w - 2.0 * bw, r.h - 2.0 * bw],
                    radius: (n.radius - bw).max(0.0),
                    color: fill,
                    clip: clip4(clip),
                });
            }
        } else if fill[3] > 0 {
            out.push(UiDraw::Round {
                pos: [r.x, r.y],
                size: [r.w, r.h],
                radius: n.radius,
                color: fill,
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
    let accent = if n.enabled {
        rgba(n.accent)
    } else {
        faded(rgba(n.accent), 0.5)
    };
    let accent = faded(accent, fade);
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
    if n.focused && n.enabled {
        outline(out, r, 1.0, accent, clip);
    }
    // A text input shows its text inside its own box, never outside it.
    let text_clip = if n.widget == Widget::TextInput {
        Some(n.clip.map_or(r, |c| c.intersect(&r)))
    } else {
        n.clip
    };
    if !n.lines.is_empty() {
        let color = c4(col.text);
        let visible = text_clip.is_none_or(|c| c.w > 0.0 && c.h > 0.0);
        if visible {
            out.push(UiDraw::Text {
                pos: [r.x + n.padding[0], r.y + n.padding[1]],
                text: n.lines.clone(),
                scale: n.text_scale,
                color,
                clip: clip4(text_clip),
            });
        }
    }
    if n.widget == Widget::TextInput && n.focused && n.enabled {
        let caret = n.caret.min(n.text.chars().count()) as f32;
        let s = n.text_scale;
        let bar = Rect {
            x: r.x + n.padding[0] + caret * UI_ADVANCE * s - (s / 2.0).floor(),
            y: r.y + n.padding[1],
            w: s.max(1.0),
            h: UI_GLYPH[1] * s,
        };
        rect(out, bar, c4(col.text), text_clip);
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
        assert_eq!(
            d,
            vec![UiDraw::Gradient {
                pos: [0.0, 0.0],
                size: [40.0, 20.0],
                colors: [top, top, bottom, bottom],
                clip: None
            }]
        );
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
}
