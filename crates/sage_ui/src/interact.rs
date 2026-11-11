//! Interaction: hit testing, hover, press, focus and navigation over a laid
//! out [`Tree`]. Input is data per tick ([`UiInput`]); the only memory is
//! [`UiState`], which the ECS module stores in a singleton so it is hashed,
//! snapshotted and saved with the world.
//!
//! Pointer and navigation are separate inputs on purpose. Navigation is bits
//! (the keyboard or a gamepad can set them), so UI driven that way replays
//! from the existing `sage-input v1` recording. The pointer is not part of
//! that format yet (see docs/milestone-2.md).

use sage_core::host::UI_ADVANCE;

use crate::layout::{Direction, Tree, Widget};

pub use sage_core::host::{
    NAV_ACCEPT, NAV_BACKSPACE, NAV_CANCEL, NAV_DELETE, NAV_DOWN, NAV_END, NAV_HOME, NAV_LEFT,
    NAV_NEXT, NAV_PAGE_DOWN, NAV_PAGE_UP, NAV_PREV, NAV_RIGHT, NAV_UP,
};

/// The longest text a text input accepts, in characters.
pub const MAX_TEXT: usize = 256;

/// UI units one wheel notch scrolls.
pub const SCROLL_STEP: f32 = 12.0;

/// What the UI remembers between ticks. Ids are StableIds; 0 is "none".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiState {
    pub hot: u64,
    pub active: u64,
    pub focus: u64,
    pub prev_down: bool,
    pub prev_nav: u32,
    /// Output of the last `update`: some scroll offset changed, so the tree
    /// needs another `layout` before it is drawn. Not carried between ticks.
    pub scrolled: bool,
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
    /// Wheel notches this tick; positive scrolls the content down (towards
    /// its end).
    pub wheel: f32,
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
        !matches!(n.widget, Widget::None | Widget::Meter)
            && n.rect.contains(p)
            && n.clip.is_none_or(|c| c.contains(p))
    })
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn byte_of(s: &str, index: usize) -> usize {
    s.char_indices().nth(index).map_or(s.len(), |(b, _)| b)
}

/// The length of a scroll container's visible content area along its flow.
fn view_len(tree: &Tree, i: usize) -> f32 {
    let n = &tree.nodes[i];
    if n.direction == Direction::Row {
        n.rect.w - n.padding[0] - n.padding[2]
    } else {
        n.rect.h - n.padding[1] - n.padding[3]
    }
}

/// Scrolls container `i` by `delta` units, clamped; true if it moved.
fn scroll_by(tree: &mut Tree, i: usize, delta: f32) -> bool {
    let max = (tree.nodes[i].content - view_len(tree, i)).max(0.0);
    let n = &mut tree.nodes[i];
    let next = (n.scroll_offset + delta).clamp(0.0, max);
    let moved = next != n.scroll_offset;
    n.scroll_offset = next;
    moved
}

fn parent_of(tree: &Tree, i: usize) -> Option<usize> {
    tree.nodes[i]
        .parent
        .filter(|&p| p < tree.nodes.len() && p != i)
}

/// The nearest scrolling ancestor of `i`, not counting `i`.
fn scroll_ancestor(tree: &Tree, i: usize) -> Option<usize> {
    let mut at = parent_of(tree, i);
    for _ in 0..64 {
        let a = at?;
        if tree.nodes[a].scroll {
            return Some(a);
        }
        at = parent_of(tree, a);
    }
    None
}

/// Scrolls every container above `i` just far enough to show it.
fn reveal(tree: &mut Tree, i: usize) -> bool {
    let mut moved = false;
    let mut at = scroll_ancestor(tree, i);
    for _ in 0..64 {
        let Some(a) = at else { break };
        let c = tree.nodes[a].clone();
        let n = tree.nodes[i].rect;
        let (n0, n1, c0, c1) = if c.direction == Direction::Row {
            (
                n.x,
                n.x + n.w,
                c.rect.x + c.padding[0],
                c.rect.x + c.rect.w - c.padding[2],
            )
        } else {
            (
                n.y,
                n.y + n.h,
                c.rect.y + c.padding[1],
                c.rect.y + c.rect.h - c.padding[3],
            )
        };
        if n0 < c0 {
            moved |= scroll_by(tree, a, n0 - c0);
        } else if n1 > c1 {
            moved |= scroll_by(tree, a, n1 - c1);
        }
        at = scroll_ancestor(tree, a);
    }
    moved
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

/// Is `i` the node `root` or inside its subtree?
fn within(tree: &Tree, i: usize, root: usize) -> bool {
    let mut at = Some(i);
    for _ in 0..64 {
        match at {
            Some(a) if a == root => return true,
            Some(a) => at = parent_of(tree, a),
            None => return false,
        }
    }
    false
}

/// The topmost modal (last in draw order), if there is one.
fn topmost_modal(tree: &Tree, order: &[usize]) -> Option<usize> {
    order.iter().rev().copied().find(|&i| tree.nodes[i].modal)
}

/// Focusable, enabled nodes in tab order (`tab`, then draw order), limited to
/// the topmost modal's subtree while there is a modal.
fn tab_order(tree: &Tree, order: &[usize]) -> Vec<usize> {
    let modal = topmost_modal(tree, order);
    let mut v: Vec<(i32, usize, usize)> = order
        .iter()
        .enumerate()
        .filter(|&(_, &i)| tree.nodes[i].widget.focusable() && tree.nodes[i].enabled)
        .filter(|&(_, &i)| modal.is_none_or(|m| within(tree, i, m)))
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
    st.scrolled = false;
    let focus_before = st.focus;

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
    // Focus cannot stay outside a modal.
    if let (Some(m), Some(f)) = (topmost_modal(tree, &order), find(tree, st.focus))
        && !within(tree, f, m)
    {
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
                if tree.nodes[i].widget == Widget::TextInput
                    && let Some(p) = input.pointer
                {
                    let n = &mut tree.nodes[i];
                    let col = (p[0] - (n.rect.x + n.padding[0])) / (UI_ADVANCE * n.text_scale);
                    n.caret = (col.round().max(0.0) as usize).min(char_len(&n.text));
                }
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

    // The wheel scrolls the topmost scroll container under the pointer.
    let container_under = |tree: &Tree| {
        input.pointer.and_then(|p| {
            order.iter().rev().copied().find(|&i| {
                let n = &tree.nodes[i];
                n.scroll && n.rect.contains(p) && n.clip.is_none_or(|c| c.contains(p))
            })
        })
    };
    if input.wheel != 0.0
        && let Some(i) = container_under(tree)
    {
        st.scrolled |= scroll_by(tree, i, input.wheel * SCROLL_STEP);
    }
    let page = (input.nav & !st.prev_nav) & (NAV_PAGE_UP | NAV_PAGE_DOWN);
    if page != 0 {
        let target = find(tree, st.focus)
            .and_then(|f| scroll_ancestor(tree, f))
            .or_else(|| container_under(tree))
            .or_else(|| order.iter().copied().find(|&i| tree.nodes[i].scroll));
        if let Some(i) = target {
            let step = view_len(tree, i) * 0.9;
            let dir = if page & NAV_PAGE_DOWN != 0 { 1.0 } else { -1.0 };
            st.scrolled |= scroll_by(tree, i, dir * step);
        }
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
            let n = &mut tree.nodes[i];
            let len = char_len(&n.text);
            n.caret = n.caret.min(len);
            if edge & NAV_LEFT != 0 && n.caret > 0 {
                n.caret -= 1;
            }
            if edge & NAV_RIGHT != 0 && n.caret < len {
                n.caret += 1;
            }
            if edge & NAV_HOME != 0 {
                n.caret = 0;
            }
            if edge & NAV_END != 0 {
                n.caret = len;
            }
            let mut changed = false;
            if edge & NAV_BACKSPACE != 0 && n.caret > 0 {
                let at = byte_of(&n.text, n.caret - 1);
                n.text.remove(at);
                n.caret -= 1;
                changed = true;
            }
            if edge & NAV_DELETE != 0 && n.caret < char_len(&n.text) {
                let at = byte_of(&n.text, n.caret);
                n.text.remove(at);
                changed = true;
            }
            if let Some(c) = input.ch
                && !c.is_control()
                && char_len(&n.text) < MAX_TEXT
            {
                let at = byte_of(&n.text, n.caret);
                n.text.insert(at, c);
                n.caret += 1;
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
    // A widget focused by navigation scrolls into view.
    if st.focus != focus_before
        && let Some(i) = find(tree, st.focus)
    {
        st.scrolled |= reveal(tree, i);
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
            scrolled: false,
        };
        update(&mut t, &mut st, &UiInput::default());
        assert_eq!((st.active, st.focus), (0, 0));
    }

    /// A 100x40 scroll column holding six 20-unit buttons (120 tall).
    fn list() -> Tree {
        let mut nodes = vec![Node {
            id: 1,
            width: Size::Fixed(100.0),
            height: Size::Fixed(40.0),
            scroll: true,
            ..Node::default()
        }];
        for k in 0..6u64 {
            nodes.push(Node {
                id: 10 + k,
                parent: Some(0),
                width: Size::Fill(1.0),
                height: Size::Fixed(20.0),
                widget: Widget::Button,
                ..Node::default()
            });
        }
        let mut t = Tree { nodes };
        layout(&mut t, [100.0, 100.0]);
        t
    }

    /// Runs one tick the way the ECS module does: update, then layout again
    /// if something scrolled.
    fn tick(t: &mut Tree, st: &mut UiState, input: &UiInput) -> Vec<UiEvent> {
        let e = update(t, st, input);
        if st.scrolled {
            layout(t, [100.0, 100.0]);
        }
        e
    }

    #[test]
    fn the_wheel_scrolls_the_container_under_the_pointer_and_clamps() {
        let mut t = list();
        let mut st = UiState::default();
        assert_eq!((t.nodes[0].content, t.nodes[0].scroll_offset), (120.0, 0.0));
        let wheel = |dy, x, y| UiInput {
            pointer: Some([x, y]),
            wheel: dy,
            ..UiInput::default()
        };
        tick(&mut t, &mut st, &wheel(1.0, 5.0, 5.0));
        assert_eq!(t.nodes[0].scroll_offset, SCROLL_STEP);
        assert_eq!(t.nodes[1].rect.y, -SCROLL_STEP, "content moved up");
        // Off the container the wheel does nothing.
        tick(&mut t, &mut st, &wheel(1.0, 5.0, 90.0));
        assert_eq!(t.nodes[0].scroll_offset, SCROLL_STEP);
        for _ in 0..20 {
            tick(&mut t, &mut st, &wheel(1.0, 5.0, 5.0));
        }
        assert_eq!(t.nodes[0].scroll_offset, 80.0, "clamped to content - view");
        tick(&mut t, &mut st, &wheel(-20.0, 5.0, 5.0));
        assert_eq!(t.nodes[0].scroll_offset, 0.0);
    }

    #[test]
    fn clipped_content_cannot_be_hit_and_a_click_lands_on_the_visible_row() {
        let mut t = list();
        let mut st = UiState::default();
        // The sixth button is at y 100..120, far below the 40-unit view.
        let below = UiInput {
            pointer: Some([5.0, 100.0]),
            ..UiInput::default()
        };
        tick(&mut t, &mut st, &below);
        assert_eq!(st.hot, 0, "outside the container: nothing is hit");
        // A pointer inside the view but over a clipped-out part of the tree
        // cannot reach it either: scroll so the second row is at y 0..20.
        t.nodes[0].scroll_offset = 20.0;
        layout(&mut t, [100.0, 100.0]);
        tick(
            &mut t,
            &mut st,
            &UiInput {
                pointer: Some([5.0, 5.0]),
                ..UiInput::default()
            },
        );
        assert_eq!(st.hot, 11, "the second button is now at the top");
    }

    #[test]
    fn navigating_to_a_hidden_widget_scrolls_it_into_view() {
        let mut t = list();
        let mut st = UiState::default();
        let nav = |bits| UiInput {
            nav: bits,
            ..UiInput::default()
        };
        let mut seen = vec![];
        for _ in 0..6 {
            tick(&mut t, &mut st, &nav(NAV_NEXT));
            tick(&mut t, &mut st, &nav(0));
            let n = &t.nodes[t.nodes.iter().position(|n| n.id == st.focus).unwrap()];
            seen.push((st.focus, n.rect.y >= 0.0 && n.rect.y + n.rect.h <= 40.0));
        }
        assert!(seen.iter().all(|&(_, visible)| visible), "{seen:?}");
        assert_eq!(st.focus, 15);
        assert_eq!(t.nodes[0].scroll_offset, 80.0);
        // Wrapping back to the first scrolls back up.
        tick(&mut t, &mut st, &nav(NAV_NEXT));
        assert_eq!((st.focus, t.nodes[0].scroll_offset), (10, 0.0));
    }

    #[test]
    fn page_keys_scroll_by_most_of_a_view() {
        let mut t = list();
        let mut st = UiState::default();
        let nav = |bits| UiInput {
            nav: bits,
            ..UiInput::default()
        };
        tick(&mut t, &mut st, &nav(NAV_PAGE_DOWN));
        assert_eq!(t.nodes[0].scroll_offset, 36.0, "90% of the 40-unit view");
        tick(&mut t, &mut st, &nav(0));
        tick(&mut t, &mut st, &nav(NAV_PAGE_UP));
        assert_eq!(t.nodes[0].scroll_offset, 0.0);
    }

    #[test]
    fn the_caret_moves_inserts_and_deletes_in_the_middle() {
        let mut t = Tree {
            nodes: vec![Node {
                id: 7,
                width: Size::Fixed(120.0),
                height: Size::Fixed(12.0),
                padding: [3.0, 2.0, 3.0, 2.0],
                widget: Widget::TextInput,
                text: "héllo".into(),
                ..Node::default()
            }],
        };
        layout(&mut t, [120.0, 12.0]);
        let mut st = UiState::default();
        // Clicking places the caret: x = 3 + 6 * 2 puts it after two glyphs.
        tick(&mut t, &mut st, &pointer(3.0 + 6.0 * 2.0 + 1.0, 5.0, true));
        tick(&mut t, &mut st, &pointer(3.0 + 6.0 * 2.0 + 1.0, 5.0, false));
        assert_eq!(t.nodes[0].caret, 2);
        let press = |t: &mut Tree, st: &mut UiState, bits: u32, ch: Option<char>| {
            let e = tick(
                t,
                st,
                &UiInput {
                    nav: bits,
                    ch,
                    ..UiInput::default()
                },
            );
            tick(t, st, &UiInput::default());
            e
        };
        press(&mut t, &mut st, 0, Some('X'));
        assert_eq!((t.nodes[0].text.as_str(), t.nodes[0].caret), ("héXllo", 3));
        press(&mut t, &mut st, NAV_BACKSPACE, None);
        assert_eq!((t.nodes[0].text.as_str(), t.nodes[0].caret), ("héllo", 2));
        press(&mut t, &mut st, NAV_BACKSPACE, None);
        assert_eq!(
            (t.nodes[0].text.as_str(), t.nodes[0].caret),
            ("hllo", 1),
            "multi-byte characters delete whole"
        );
        press(&mut t, &mut st, NAV_DELETE, None);
        assert_eq!((t.nodes[0].text.as_str(), t.nodes[0].caret), ("hlo", 1));
        press(&mut t, &mut st, NAV_END, None);
        assert_eq!(t.nodes[0].caret, 3);
        press(&mut t, &mut st, NAV_RIGHT, None);
        assert_eq!(t.nodes[0].caret, 3, "the caret stops at the end");
        press(&mut t, &mut st, NAV_HOME, None);
        press(&mut t, &mut st, NAV_LEFT, None);
        assert_eq!(t.nodes[0].caret, 0);
        let e = press(&mut t, &mut st, NAV_BACKSPACE, None);
        assert!(e.is_empty(), "nothing to delete before the first character");
        press(&mut t, &mut st, 0, Some('>'));
        assert_eq!(t.nodes[0].text, ">hlo");
    }

    #[test]
    fn a_meter_is_transparent_to_the_pointer_and_takes_no_focus() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    id: 1,
                    width: Size::Fixed(50.0),
                    height: Size::Fixed(10.0),
                    widget: Widget::Button,
                    ..Node::default()
                },
                Node {
                    id: 2,
                    parent: Some(0),
                    width: Size::Fill(1.0),
                    height: Size::Fill(1.0),
                    widget: Widget::Meter,
                    ..Node::default()
                },
            ],
        };
        layout(&mut t, [50.0, 10.0]);
        let mut st = UiState::default();
        tick(&mut t, &mut st, &pointer(5.0, 5.0, true));
        let e = tick(&mut t, &mut st, &pointer(5.0, 5.0, false));
        assert_eq!(e.iter().filter(|e| e.kind == EventKind::Clicked).count(), 1);
        assert_eq!(
            st.hot, 1,
            "the button under the meter is hot, not the meter"
        );
    }

    #[test]
    fn a_modal_traps_keyboard_focus_in_its_subtree() {
        let mut t = menu();
        // A dialog over the menu: a modal block holding one button.
        t.nodes.push(Node {
            id: 90,
            width: Size::Fixed(100.0),
            height: Size::Fixed(80.0),
            widget: Widget::Block,
            modal: true,
            z: 5,
            ..Node::default()
        });
        t.nodes.push(Node {
            id: 91,
            parent: Some(t.nodes.len() - 1),
            width: Size::Fixed(40.0),
            height: Size::Fixed(12.0),
            widget: Widget::Button,
            ..Node::default()
        });
        layout(&mut t, [100.0, 80.0]);
        let mut st = UiState {
            focus: 11, // a toggle in the menu behind the dialog
            ..UiState::default()
        };
        let nav = |bits| UiInput {
            nav: bits,
            ..UiInput::default()
        };
        tick(&mut t, &mut st, &nav(0));
        assert_eq!(st.focus, 0, "focus outside the modal is dropped");
        for _ in 0..3 {
            tick(&mut t, &mut st, &nav(NAV_NEXT));
            tick(&mut t, &mut st, &nav(0));
            assert_eq!(st.focus, 91, "the only focusable widget in the modal");
        }
        // Without the modal, navigation reaches the menu again.
        t.nodes[5].modal = false;
        tick(&mut t, &mut st, &nav(NAV_NEXT));
        assert_ne!(st.focus, 91);
    }
}
