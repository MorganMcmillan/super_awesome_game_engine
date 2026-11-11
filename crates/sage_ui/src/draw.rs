//! Drawing: a laid-out, interacted tree becomes a [`UiFrame`], the plain
//! data presentation consumes (`sage_core::host`). The UI itself never
//! touches a renderer.

use sage_core::host::{UiDraw, UiFrame};

use crate::layout::{Node, Tree, Widget, rgba};

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

fn rect(out: &mut Vec<UiDraw>, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    if color[3] > 0 && w > 0.0 && h > 0.0 {
        out.push(UiDraw::Rect {
            pos: [x, y],
            size: [w, h],
            color,
        });
    }
}

fn outline(out: &mut Vec<UiDraw>, r: crate::layout::Rect, t: f32, color: [u8; 4]) {
    rect(out, r.x, r.y, r.w, t, color);
    rect(out, r.x, r.y + r.h - t, r.w, t, color);
    rect(out, r.x, r.y + t, t, r.h - 2.0 * t, color);
    rect(out, r.x + r.w - t, r.y + t, t, r.h - 2.0 * t, color);
}

/// Draw commands back to front for the whole tree.
pub fn draw(tree: &Tree, size: [f32; 2]) -> UiFrame {
    let mut out = vec![];
    for i in tree.draw_order() {
        let n = &tree.nodes[i];
        let r = n.rect;
        rect(&mut out, r.x, r.y, r.w, r.h, tinted_fill(n));
        let accent = if n.enabled {
            rgba(n.accent)
        } else {
            dim(rgba(n.accent))
        };
        match n.widget {
            Widget::Slider if n.max != n.min => {
                let t = ((n.value - n.min) / (n.max - n.min)).clamp(0.0, 1.0);
                rect(&mut out, r.x, r.y, (r.w * t).round(), r.h, accent);
            }
            Widget::Toggle if n.on => {
                let inset = (r.h.min(r.w) / 4.0).round().max(1.0);
                rect(
                    &mut out,
                    r.x + inset,
                    r.y + inset,
                    r.w - 2.0 * inset,
                    r.h - 2.0 * inset,
                    accent,
                );
            }
            _ => {}
        }
        if n.border_width > 0.0 {
            outline(&mut out, r, n.border_width, rgba(n.border));
        }
        if n.focused && n.enabled {
            outline(&mut out, r, 1.0, accent);
        }
        let mut text = n.text.clone();
        if n.widget == Widget::TextInput && n.focused {
            text.push('_');
        }
        if !text.is_empty() {
            let color = if n.enabled {
                rgba(n.text_color)
            } else {
                dim(rgba(n.text_color))
            };
            out.push(UiDraw::Text {
                pos: [r.x + n.padding[0], r.y + n.padding[1]],
                text,
                scale: n.text_scale,
                color,
            });
        }
    }
    UiFrame { size, draws: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Size, layout};

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
                    color: [255; 4]
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
}
