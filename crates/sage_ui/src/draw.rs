//! Drawing: a laid-out, interacted tree becomes a [`UiFrame`], the plain
//! data presentation consumes (`sage_core::host`). The UI itself never
//! touches a renderer.

use sage_core::host::{UI_ADVANCE, UI_GLYPH, UiDraw, UiFrame};

use crate::layout::{Direction, Node, Rect, Tree, Widget, rgba};

fn lighten(c: [u8; 4], by: u8) -> [u8; 4] {
    [
        c[0].saturating_add(by),
        c[1].saturating_add(by),
        c[2].saturating_add(by),
        c[3],
    ]
}

fn darken(c: [u8; 4], by: u8) -> [u8; 4] {
    [
        c[0].saturating_sub(by),
        c[1].saturating_sub(by),
        c[2].saturating_sub(by),
        c[3],
    ]
}

fn dim(c: [u8; 4]) -> [u8; 4] {
    [c[0], c[1], c[2], c[3] / 2]
}

/// The fill of a node with its widget state applied.
fn tinted_fill(n: &Node) -> [u8; 4] {
    let base = rgba(n.fill);
    if !n.enabled {
        return dim(base);
    }
    if !matches!(
        n.widget,
        Widget::Button | Widget::Toggle | Widget::Slider | Widget::TextInput
    ) {
        return base;
    }
    if n.pressed {
        darken(base, 24)
    } else if n.hovered {
        lighten(base, 16)
    } else {
        base
    }
}

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

fn node(out: &mut Vec<UiDraw>, n: &Node) {
    let r = n.rect;
    if n.clip
        .is_some_and(|c| r.intersect(&c).w <= 0.0 || r.intersect(&c).h <= 0.0)
    {
        return;
    }
    rect(out, r, tinted_fill(n), n.clip);
    let accent = if n.enabled {
        rgba(n.accent)
    } else {
        dim(rgba(n.accent))
    };
    match n.widget {
        Widget::Slider | Widget::Meter if n.max != n.min => {
            let t = ((n.value - n.min) / (n.max - n.min)).clamp(0.0, 1.0);
            let fill = Rect {
                w: (r.w * t).round(),
                ..r
            };
            rect(out, fill, accent, n.clip);
        }
        Widget::Toggle if n.on => {
            let inset = (r.h.min(r.w) / 4.0).round().max(1.0);
            let mark = Rect {
                x: r.x + inset,
                y: r.y + inset,
                w: r.w - 2.0 * inset,
                h: r.h - 2.0 * inset,
            };
            rect(out, mark, accent, n.clip);
        }
        _ => {}
    }
    if n.border_width > 0.0 {
        outline(out, r, n.border_width, rgba(n.border), n.clip);
    }
    if n.focused && n.enabled {
        outline(out, r, 1.0, accent, n.clip);
    }
    // A text input shows its text inside its own box, never outside it.
    let text_clip = if n.widget == Widget::TextInput {
        Some(n.clip.map_or(r, |c| c.intersect(&r)))
    } else {
        n.clip
    };
    if !n.lines.is_empty() {
        let color = if n.enabled {
            rgba(n.text_color)
        } else {
            dim(rgba(n.text_color))
        };
        let visible = text_clip.is_none_or(|c| c.w > 0.0 && c.h > 0.0);
        if visible {
            out.push(UiDraw::Text {
                pos: [r.x + n.padding[0], r.y + n.padding[1]],
                text: n.lines.clone(),
                scale: n.text_scale,
                color,
                clip: text_clip.map(|c| [c.x, c.y, c.x + c.w, c.y + c.h]),
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
        rect(out, bar, rgba(n.text_color), text_clip);
    }
}

/// Draw commands back to front for the whole tree. A scroll container's
/// scrollbar is drawn right after the last of its descendants.
pub fn draw(tree: &Tree, size: [f32; 2]) -> UiFrame {
    let order = tree.draw_order();
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
        node(&mut out, &tree.nodes[i]);
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
}
