//! Style resolution: what a node's colors are right now, given its state,
//! and how they ease towards a new value (transitions). Pure and
//! deterministic: transitions step in whole ticks with integer math.

use crate::layout::{Color, Node, Widget};

/// Colors a state overrides. `0` leaves that color as it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Variant {
    pub fill: Color,
    pub text: Color,
    pub border: Color,
}

/// Style variants by state (design doc 9.5). Later states win over earlier
/// ones: checked, focus, hover, pressed, disabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct States {
    pub checked: Variant,
    pub focus: Variant,
    pub hover: Variant,
    pub pressed: Variant,
    pub disabled: Variant,
}

/// The colors a node is drawn with right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub fill: Color,
    pub text: Color,
    pub border: Color,
    /// 0 opaque to 1 invisible, for the node alone (not its ancestors).
    pub fade: f32,
}

fn channels(c: Color) -> [u8; 4] {
    c.to_be_bytes()
}

fn lighten(c: Color, by: u8) -> Color {
    let [r, g, b, a] = channels(c);
    Color::from_be_bytes([
        r.saturating_add(by),
        g.saturating_add(by),
        b.saturating_add(by),
        a,
    ])
}

fn darken(c: Color, by: u8) -> Color {
    let [r, g, b, a] = channels(c);
    Color::from_be_bytes([
        r.saturating_sub(by),
        g.saturating_sub(by),
        b.saturating_sub(by),
        a,
    ])
}

fn dim(c: Color) -> Color {
    let [r, g, b, a] = channels(c);
    Color::from_be_bytes([r, g, b, a / 2])
}

fn apply(r: &mut Resolved, v: &Variant) -> bool {
    let mut any = false;
    for (slot, over) in [
        (&mut r.fill, v.fill),
        (&mut r.text, v.text),
        (&mut r.border, v.border),
    ] {
        if over != 0 {
            *slot = over;
            any = true;
        }
    }
    any
}

/// The colors of `n` from its own style and state. A widget without an
/// explicit variant for a state gets the built-in look: hover lightens the
/// fill, pressed darkens it, disabled halves the alpha.
pub fn resolve(n: &Node) -> Resolved {
    let mut r = Resolved {
        fill: n.fill,
        text: n.text_color,
        border: n.border,
        fade: n.fade,
    };
    let interactive = matches!(
        n.widget,
        Widget::Button | Widget::Toggle | Widget::Choice | Widget::Slider | Widget::TextInput
    );
    if n.on {
        apply(&mut r, &n.states.checked);
    }
    if n.focused {
        apply(&mut r, &n.states.focus);
    }
    if n.hovered && !apply(&mut r, &n.states.hover) && interactive && n.enabled {
        r.fill = lighten(r.fill, 16);
    }
    if n.pressed && !apply(&mut r, &n.states.pressed) && interactive && n.enabled {
        r.fill = darken(n.fill, 24);
    }
    if !n.enabled && !apply(&mut r, &n.states.disabled) {
        r.fill = dim(r.fill);
        r.text = dim(r.text);
        r.border = dim(r.border);
    }
    r
}

/// One tick of easing a color towards `target`: each channel closes `1 /
/// ticks` of what is left (an exponential ease with `ticks` as its time
/// constant), and moves at least one step, so it always arrives.
pub fn ease_color(cur: Color, target: Color, ticks: u32) -> Color {
    if ticks <= 1 || cur == target {
        return target;
    }
    let (c, t) = (channels(cur), channels(target));
    let mut out = [0u8; 4];
    for i in 0..4 {
        let d = i32::from(t[i]) - i32::from(c[i]);
        let mut step = d / ticks as i32;
        if step == 0 {
            step = d.signum();
        }
        out[i] = (i32::from(c[i]) + step).clamp(0, 255) as u8;
    }
    Color::from_be_bytes(out)
}

pub fn ease_f32(cur: f32, target: f32, ticks: u32) -> f32 {
    if ticks <= 1 || (target - cur).abs() < 0.005 {
        return target;
    }
    cur + (target - cur) / ticks as f32
}

/// One tick of a transition for every field of `r`.
pub fn ease(cur: &Resolved, target: &Resolved, ticks: u32) -> Resolved {
    Resolved {
        fill: ease_color(cur.fill, target.fill, ticks),
        text: ease_color(cur.text, target.text, ticks),
        border: ease_color(cur.border, target.border, ticks),
        fade: ease_f32(cur.fade, target.fade, ticks),
    }
}

/// `c` with its alpha scaled by `1 - fade`.
pub fn faded(c: [u8; 4], fade: f32) -> [u8; 4] {
    if fade <= 0.0 {
        return c;
    }
    [
        c[0],
        c[1],
        c[2],
        (f32::from(c[3]) * (1.0 - fade.min(1.0))).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button() -> Node {
        Node {
            widget: Widget::Button,
            fill: 0x4040_40ff,
            text_color: 0xffff_ffff,
            ..Node::default()
        }
    }

    #[test]
    fn built_in_state_looks_and_explicit_variants() {
        let mut n = button();
        assert_eq!(resolve(&n).fill, 0x4040_40ff);
        n.hovered = true;
        assert_eq!(resolve(&n).fill, 0x5050_50ff);
        n.pressed = true;
        assert_eq!(
            resolve(&n).fill,
            0x2828_28ff,
            "pressed darkens the resting fill"
        );
        // An explicit hover variant replaces the built-in lighten.
        let mut n = button();
        n.hovered = true;
        n.states.hover = Variant {
            fill: 0xe874_1aff,
            text: 0x1a11_09ff,
            border: 0,
        };
        let r = resolve(&n);
        assert_eq!(
            (r.fill, r.text, r.border),
            (0xe874_1aff, 0x1a11_09ff, n.border)
        );
        // Disabled wins over hover and dims when it has no variant.
        n.enabled = false;
        let r = resolve(&n);
        assert_eq!(
            r.fill, 0xe874_1a7f,
            "disabled halves the alpha of what hover set"
        );
        n.states.disabled.fill = 0x3333_33ff;
        assert_eq!(resolve(&n).fill, 0x3333_33ff);
        // Checked and focus apply to any node; non-widgets get no built-in tint.
        let mut p = Node {
            fill: 0x1111_11ff,
            hovered: true,
            ..Node::default()
        };
        assert_eq!(resolve(&p).fill, 0x1111_11ff);
        p.on = true;
        p.states.checked.fill = 0xaaaa_aaff;
        assert_eq!(resolve(&p).fill, 0xaaaa_aaff);
        p.focused = true;
        p.states.focus.border = 0xff00_00ff;
        assert_eq!(resolve(&p).border, 0xff00_00ff);
    }

    #[test]
    fn easing_arrives_in_a_bounded_number_of_ticks_and_never_overshoots() {
        let (a, b) = (0x0000_00ffu32, 0xff80_20ffu32);
        let mut cur = a;
        let mut ticks = 0;
        while cur != b {
            let next = ease_color(cur, b, 6);
            let (c, n, t) = (channels(cur), channels(next), channels(b));
            for i in 0..4 {
                let (lo, hi) = (c[i].min(t[i]), c[i].max(t[i]));
                assert!(
                    (lo..=hi).contains(&n[i]),
                    "channel {i} overshot: {cur:x} -> {next:x}"
                );
            }
            cur = next;
            ticks += 1;
            assert!(ticks < 200, "never arrives");
        }
        // Going back down works the same.
        assert_eq!(ease_color(b, a, 1), a, "one tick is immediate");
        assert_eq!(ease_color(a, a, 6), a);
        assert_eq!(ease_f32(0.0, 1.0, 4), 0.25);
        assert_eq!(ease_f32(0.999, 1.0, 4), 1.0, "close enough snaps");
    }

    #[test]
    fn fading_scales_alpha_only() {
        assert_eq!(faded([10, 20, 30, 200], 0.5), [10, 20, 30, 100]);
        assert_eq!(faded([10, 20, 30, 200], 0.0), [10, 20, 30, 200]);
        assert_eq!(faded([10, 20, 30, 200], 2.0), [10, 20, 30, 0]);
    }
}
