//! Interaction: hit testing, hover, press, focus and navigation over a laid
//! out [`Tree`]. Input is data per tick ([`UiInput`]); the only memory is
//! [`UiState`], which the ECS module stores in a singleton so it is hashed,
//! snapshotted and saved with the world.
//!
//! Pointer and navigation are separate inputs on purpose. Navigation is bits
//! (the keyboard or a gamepad can set them), so UI driven that way replays
//! from the existing `sage-input v1` recording. The pointer is not part of
//! that format yet (see docs/milestone-2.md).

use crate::layout::{Tree, Widget};

pub const NAV_NEXT: u32 = 1 << 0;
pub const NAV_PREV: u32 = 1 << 1;
pub const NAV_ACCEPT: u32 = 1 << 2;
pub const NAV_CANCEL: u32 = 1 << 3;
pub const NAV_LEFT: u32 = 1 << 4;
pub const NAV_RIGHT: u32 = 1 << 5;
pub const NAV_UP: u32 = 1 << 6;
pub const NAV_DOWN: u32 = 1 << 7;
pub const NAV_BACKSPACE: u32 = 1 << 8;

/// The longest text a text input accepts, in characters.
pub const MAX_TEXT: usize = 256;

/// What the UI remembers between ticks. Ids are StableIds; 0 is "none".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiState {
    pub hot: u64,
    pub active: u64,
    pub focus: u64,
    pub prev_down: bool,
    pub prev_nav: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiInput {
    /// Pointer position in UI units, or `None` when there is no pointer.
    pub pointer: Option<[f32; 2]>,
    /// The primary button is held.
    pub down: bool,
    /// `NAV_*` bits held this tick.
    pub nav: u32,
    /// A character typed this tick.
    pub ch: Option<char>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Clicked,
    Toggled(bool),
    Changed(f32),
    TextChanged,
    Submitted,
    Focused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiEvent {
    /// The StableId of the widget.
    pub target: u64,
    pub kind: EventKind,
}

fn interactive(w: Widget) -> bool {
    !matches!(w, Widget::None | Widget::Block)
}

/// The topmost node under `p`, as an index. Decoration is transparent to the
/// pointer; a disabled widget or a `Block` stops it.
pub fn hit_test(tree: &Tree, order: &[usize], p: [f32; 2]) -> Option<usize> {
    order.iter().rev().copied().find(|&i| {
        let n = &tree.nodes[i];
        n.widget != Widget::None && n.rect.contains(p)
    })
}

fn find(tree: &Tree, id: u64) -> Option<usize> {
    if id == 0 {
        return None;
    }
    tree.nodes.iter().position(|n| n.id == id)
}

/// Sets a slider from a pointer x; returns true if the value changed.
fn set_slider(tree: &mut Tree, i: usize, x: f32) -> bool {
    let n = &mut tree.nodes[i];
    let t = if n.rect.w > 0.0 {
        ((x - n.rect.x) / n.rect.w).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut v = n.min + t * (n.max - n.min);
    if n.step > 0.0 {
        v = n.min + ((v - n.min) / n.step).round() * n.step;
    }
    v = v.clamp(n.min.min(n.max), n.max.max(n.min));
    let changed = v != n.value;
    n.value = v;
    changed
}

fn step_slider(tree: &mut Tree, i: usize, dir: f32) -> bool {
    let n = &mut tree.nodes[i];
    let step = if n.step > 0.0 {
        n.step
    } else {
        (n.max - n.min) / 20.0
    };
    let v = (n.value + dir * step).clamp(n.min.min(n.max), n.max.max(n.min));
    let changed = v != n.value;
    n.value = v;
    changed
}

/// Focusable, enabled nodes in tab order (`tab`, then draw order).
fn tab_order(tree: &Tree, order: &[usize]) -> Vec<usize> {
    let mut v: Vec<(i32, usize, usize)> = order
        .iter()
        .enumerate()
        .filter(|&(_, &i)| tree.nodes[i].widget.focusable() && tree.nodes[i].enabled)
        .map(|(rank, &i)| (tree.nodes[i].tab, rank, i))
        .collect();
    v.sort();
    v.into_iter().map(|(_, _, i)| i).collect()
}

fn activate(tree: &mut Tree, i: usize, events: &mut Vec<UiEvent>) {
    let id = tree.nodes[i].id;
    match tree.nodes[i].widget {
        Widget::Button => events.push(UiEvent {
            target: id,
            kind: EventKind::Clicked,
        }),
        Widget::Toggle => {
            let on = !tree.nodes[i].on;
            tree.nodes[i].on = on;
            events.push(UiEvent {
                target: id,
                kind: EventKind::Toggled(on),
            });
        }
        Widget::TextInput => events.push(UiEvent {
            target: id,
            kind: EventKind::Submitted,
        }),
        _ => {}
    }
}

/// Advances the interaction by one tick over a tree that has been laid out.
/// Mutates widget state (toggles, sliders, text) and the hover, pressed and
/// focused flags, and returns the events in the order they happened.
pub fn update(tree: &mut Tree, st: &mut UiState, input: &UiInput) -> Vec<UiEvent> {
    let mut events = vec![];
    let order = tree.draw_order();

    // Forget ids that no longer exist or can no longer be used.
    let usable = |tree: &Tree, id: u64| {
        find(tree, id).is_some_and(|i| tree.nodes[i].enabled && interactive(tree.nodes[i].widget))
    };
    if !usable(tree, st.active) {
        st.active = 0;
    }
    if !usable(tree, st.focus) {
        st.focus = 0;
    }

    let hit = input.pointer.and_then(|p| hit_test(tree, &order, p));
    st.hot = hit.map_or(0, |i| tree.nodes[i].id);
    let press = input.down && !st.prev_down;
    let release = !input.down && st.prev_down;

    if press {
        let target = hit.filter(|&i| interactive(tree.nodes[i].widget) && tree.nodes[i].enabled);
        match target {
            Some(i) => {
                st.active = tree.nodes[i].id;
                if tree.nodes[i].widget.focusable() && st.focus != st.active {
                    st.focus = st.active;
                    events.push(UiEvent {
                        target: st.active,
                        kind: EventKind::Focused,
                    });
                }
            }
            None => st.focus = 0,
        }
    }
    if let (Some(p), Some(i)) = (input.pointer, find(tree, st.active))
        && input.down
        && tree.nodes[i].widget == Widget::Slider
        && set_slider(tree, i, p[0])
    {
        events.push(UiEvent {
            target: tree.nodes[i].id,
            kind: EventKind::Changed(tree.nodes[i].value),
        });
    }
    if release {
        if let Some(i) = find(tree, st.active)
            && st.hot == st.active
            && matches!(tree.nodes[i].widget, Widget::Button | Widget::Toggle)
        {
            activate(tree, i, &mut events);
        }
        st.active = 0;
    }

    // Navigation acts on the edge of a bit (pressed this tick, not held).
    let edge = input.nav & !st.prev_nav;
    let moves = [
        (edge & (NAV_NEXT | NAV_DOWN) != 0, true),
        (edge & (NAV_PREV | NAV_UP) != 0, false),
    ];
    for (wanted, forward) in moves {
        if !wanted {
            continue;
        }
        let tabs = tab_order(tree, &order);
        if tabs.is_empty() {
            continue;
        }
        let at = find(tree, st.focus).and_then(|f| tabs.iter().position(|&i| i == f));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => tabs.len() - 1,
            (Some(k), true) => (k + 1) % tabs.len(),
            (Some(k), false) => (k + tabs.len() - 1) % tabs.len(),
        };
        st.focus = tree.nodes[tabs[next]].id;
        events.push(UiEvent {
            target: st.focus,
            kind: EventKind::Focused,
        });
    }
    if let Some(i) = find(tree, st.focus) {
        let id = tree.nodes[i].id;
        if edge & NAV_ACCEPT != 0 {
            activate(tree, i, &mut events);
        }
        if tree.nodes[i].widget == Widget::Slider {
            for (bit, dir) in [(NAV_LEFT, -1.0), (NAV_RIGHT, 1.0)] {
                if edge & bit != 0 && step_slider(tree, i, dir) {
                    events.push(UiEvent {
                        target: id,
                        kind: EventKind::Changed(tree.nodes[i].value),
                    });
                }
            }
        }
        if tree.nodes[i].widget == Widget::TextInput {
            let mut changed = false;
            if edge & NAV_BACKSPACE != 0 {
                changed |= tree.nodes[i].text.pop().is_some();
            }
            if let Some(c) = input.ch
                && !c.is_control()
                && tree.nodes[i].text.chars().count() < MAX_TEXT
            {
                tree.nodes[i].text.push(c);
                changed = true;
            }
            if changed {
                events.push(UiEvent {
                    target: id,
                    kind: EventKind::TextChanged,
                });
            }
        }
    }
    if edge & NAV_CANCEL != 0 {
        st.focus = 0;
    }

    st.prev_down = input.down;
    st.prev_nav = input.nav;
    for n in &mut tree.nodes {
        n.hovered = n.id == st.hot && n.widget != Widget::None;
        n.pressed = n.id == st.active && input.down;
        n.focused = n.id == st.focus;
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Node, Rect, Size, layout};

    /// A column of three buttons and a slider, 100 wide.
    fn menu() -> Tree {
        let mut nodes = vec![Node {
            id: 1,
            width: Size::Fixed(100.0),
            height: Size::Fixed(80.0),
            ..Node::default()
        }];
        for (k, w) in [
            Widget::Button,
            Widget::Toggle,
            Widget::Button,
            Widget::Slider,
        ]
        .into_iter()
        .enumerate()
        {
            nodes.push(Node {
                id: 10 + k as u64,
                parent: Some(0),
                width: Size::Fill(1.0),
                height: Size::Fixed(20.0),
                widget: w,
                min: 0.0,
                max: 10.0,
                step: 1.0,
                ..Node::default()
            });
        }
        let mut t = Tree { nodes };
        layout(&mut t, [100.0, 80.0]);
        t
    }

    fn pointer(x: f32, y: f32, down: bool) -> UiInput {
        UiInput {
            pointer: Some([x, y]),
            down,
            ..UiInput::default()
        }
    }

    #[test]
    fn a_click_needs_press_and_release_on_the_same_button() {
        let mut t = menu();
        let mut st = UiState::default();
        // Hover, press, release on button 10 (y 0..20).
        assert!(update(&mut t, &mut st, &pointer(5.0, 5.0, false)).is_empty());
        assert_eq!(st.hot, 10);
        assert!(t.nodes[1].hovered);
        let e = update(&mut t, &mut st, &pointer(5.0, 5.0, true));
        assert_eq!(
            e,
            vec![UiEvent {
                target: 10,
                kind: EventKind::Focused
            }]
        );
        assert!(t.nodes[1].pressed && t.nodes[1].focused);
        let e = update(&mut t, &mut st, &pointer(5.0, 5.0, false));
        assert_eq!(
            e,
            vec![UiEvent {
                target: 10,
                kind: EventKind::Clicked
            }]
        );
        assert_eq!((st.active, st.focus), (0, 10));
        // Press on one button, drag away, release: no click.
        update(&mut t, &mut st, &pointer(5.0, 5.0, true));
        update(&mut t, &mut st, &pointer(5.0, 45.0, true));
        let e = update(&mut t, &mut st, &pointer(5.0, 45.0, false));
        assert!(e.is_empty(), "{e:?}");
        // Pressing empty space clears the focus.
        update(&mut t, &mut st, &pointer(5.0, 79.0, false));
        update(&mut t, &mut st, &pointer(500.0, 5.0, true));
        assert_eq!(st.focus, 0);
    }

    #[test]
    fn a_toggle_flips_and_reports_its_state() {
        let mut t = menu();
        let mut st = UiState::default();
        for expect in [true, false] {
            update(&mut t, &mut st, &pointer(5.0, 25.0, true));
            let e = update(&mut t, &mut st, &pointer(5.0, 25.0, false));
            assert!(e.contains(&UiEvent {
                target: 11,
                kind: EventKind::Toggled(expect)
            }));
            assert_eq!(t.nodes[2].on, expect);
        }
    }

    #[test]
    fn dragging_a_slider_snaps_to_steps_and_reports_changes_only() {
        let mut t = menu();
        let mut st = UiState::default();
        // The slider is the fourth button row, y 60..80, x 0..100, 0..10.
        let e = update(&mut t, &mut st, &pointer(34.0, 70.0, true));
        assert!(e.contains(&UiEvent {
            target: 13,
            kind: EventKind::Changed(3.0)
        }));
        // 3.1 still rounds to step 3, so nothing changed.
        let e = update(&mut t, &mut st, &pointer(31.0, 70.0, true));
        assert!(e.is_empty(), "still step 3: {e:?}");
        let e = update(&mut t, &mut st, &pointer(500.0, 70.0, true));
        assert!(
            e.contains(&UiEvent {
                target: 13,
                kind: EventKind::Changed(10.0)
            }),
            "clamped at the end while dragging off the side: {e:?}"
        );
        update(&mut t, &mut st, &pointer(500.0, 70.0, false));
        assert_eq!(st.active, 0);
    }

    #[test]
    fn navigation_moves_focus_in_tab_order_and_accepts() {
        let mut t = menu();
        let mut st = UiState::default();
        let nav = |bits| UiInput {
            nav: bits,
            ..UiInput::default()
        };
        let focused = |e: &[UiEvent]| -> Vec<u64> {
            e.iter()
                .filter(|e| e.kind == EventKind::Focused)
                .map(|e| e.target)
                .collect()
        };
        assert_eq!(focused(&update(&mut t, &mut st, &nav(NAV_NEXT))), vec![10]);
        // Holding the bit does not repeat; releasing and pressing again does.
        assert!(update(&mut t, &mut st, &nav(NAV_NEXT)).is_empty());
        update(&mut t, &mut st, &nav(0));
        assert_eq!(focused(&update(&mut t, &mut st, &nav(NAV_NEXT))), vec![11]);
        update(&mut t, &mut st, &nav(0));
        let e = update(&mut t, &mut st, &nav(NAV_ACCEPT));
        assert!(e.contains(&UiEvent {
            target: 11,
            kind: EventKind::Toggled(true)
        }));
        // Previous wraps around; the slider takes left and right.
        update(&mut t, &mut st, &nav(0));
        update(&mut t, &mut st, &nav(NAV_PREV));
        assert_eq!(st.focus, 10);
        update(&mut t, &mut st, &nav(0));
        update(&mut t, &mut st, &nav(NAV_PREV));
        assert_eq!(st.focus, 13, "wrapped to the last focusable");
        update(&mut t, &mut st, &nav(0));
        let e = update(&mut t, &mut st, &nav(NAV_RIGHT));
        assert!(e.contains(&UiEvent {
            target: 13,
            kind: EventKind::Changed(1.0)
        }));
        update(&mut t, &mut st, &nav(0));
        update(&mut t, &mut st, &nav(NAV_CANCEL));
        assert_eq!(st.focus, 0);
    }

    #[test]
    fn tab_index_overrides_the_default_order_and_disabled_widgets_are_skipped() {
        let mut t = menu();
        t.nodes[1].tab = 5; // the first button goes last
        t.nodes[3].enabled = false;
        let mut st = UiState::default();
        let nav = |bits| UiInput {
            nav: bits,
            ..UiInput::default()
        };
        let mut seen = vec![];
        for _ in 0..4 {
            update(&mut t, &mut st, &nav(NAV_NEXT));
            seen.push(st.focus);
            update(&mut t, &mut st, &nav(0));
        }
        // Tab 0: toggle (11), slider (13); then tab 5: button (10); wraps.
        assert_eq!(seen, vec![11, 13, 10, 11]);
        // A disabled button cannot be clicked.
        update(&mut t, &mut st, &pointer(5.0, 45.0, true));
        let e = update(&mut t, &mut st, &pointer(5.0, 45.0, false));
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn a_block_stops_the_pointer_and_decoration_lets_it_through() {
        let mut t = menu();
        // A modal panel over the first button, drawn above it.
        t.nodes.push(Node {
            id: 99,
            parent: None,
            width: Size::Fixed(100.0),
            height: Size::Fixed(30.0),
            widget: Widget::Block,
            z: 1,
            ..Node::default()
        });
        layout(&mut t, [100.0, 80.0]);
        let mut st = UiState::default();
        update(&mut t, &mut st, &pointer(5.0, 5.0, true));
        let e = update(&mut t, &mut st, &pointer(5.0, 5.0, false));
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(st.hot, 99);
        // The root container is decoration: the button under the pointer wins.
        let mut st = UiState::default();
        update(&mut t, &mut st, &pointer(5.0, 45.0, false));
        assert_eq!(st.hot, 12);
    }

    #[test]
    fn text_input_takes_characters_and_backspace() {
        let mut t = Tree {
            nodes: vec![Node {
                id: 7,
                width: Size::Fixed(60.0),
                height: Size::Fixed(12.0),
                widget: Widget::TextInput,
                ..Node::default()
            }],
        };
        layout(&mut t, [60.0, 12.0]);
        let mut st = UiState::default();
        update(&mut t, &mut st, &pointer(5.0, 5.0, true));
        update(&mut t, &mut st, &pointer(5.0, 5.0, false));
        assert_eq!(st.focus, 7);
        let typed = |c| UiInput {
            ch: Some(c),
            ..UiInput::default()
        };
        for c in ['h', 'i', '\u{7}', '!'] {
            update(&mut t, &mut st, &typed(c));
        }
        assert_eq!(t.nodes[0].text, "hi!", "control characters are ignored");
        let back = UiInput {
            nav: NAV_BACKSPACE,
            ..UiInput::default()
        };
        let e = update(&mut t, &mut st, &back);
        assert_eq!(t.nodes[0].text, "hi");
        assert_eq!(e[0].kind, EventKind::TextChanged);
        update(&mut t, &mut st, &UiInput::default());
        let e = update(
            &mut t,
            &mut st,
            &UiInput {
                nav: NAV_ACCEPT,
                ..UiInput::default()
            },
        );
        assert_eq!(e[0].kind, EventKind::Submitted);
        let _ = Rect::default();
    }

    #[test]
    fn stale_ids_are_forgotten() {
        let mut t = menu();
        let mut st = UiState {
            hot: 0,
            active: 4242,
            focus: 4242,
            prev_down: false,
            prev_nav: 0,
        };
        update(&mut t, &mut st, &UiInput::default());
        assert_eq!((st.active, st.focus), (0, 0));
    }
}
