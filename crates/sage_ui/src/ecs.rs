//! The UI as a flecs module: native components under `sage.ui`, one system,
//! and the events widgets emit.
//!
//! A UI element is an entity with a StableId and a [`UiRect`]. Its authored
//! components (`Box`, `Flow`, `Paint`, `Label`, `ZIndex`, `Absolute`,
//! `Widget`, `Toggle`, `Slider`, `Scroll`, `Caret`) are written by users and prefabs; the system
//! writes the computed ones (`Rect`, `Widget.state`, and the values of
//! toggles, sliders and text inputs). The parent is the `ChildOf` target;
//! siblings flow in StableId order.
//!
//! Input is data in singletons the host sets each tick: [`UiPointer`],
//! [`UiNav`], [`UiTextEntry`] and [`UiScreen`]. The memory of the interaction
//! is the [`UiInteraction`] singleton, so it is hashed, snapshotted and saved
//! like the rest of the world. Events are emitted on the widget's entity
//! with the widget's StableId in the payload; Python reacts with
//! `@world.observer(Clicked)`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::rc::Rc;

use flecs_ecs::prelude::*;
use flecs_ecs::sys;
use sage_core::host::UiFrame;
use sage_ecs_py::{Ecs, StableIdC};
use sage_scene::StringTable;

use crate::access::AccessInfo;
use crate::interact::{Dnd, EventKind, LastPress, UiInput, UiState, update};
use crate::layout::{
    Align, CellNode, Direction, Edge, Fit, GradientNode, GridNode, ImageNode, Justify, LayerNode,
    Node, Placement, ShadowNode, ShapeNode, Size, SliceNode, Tree, ViewportNode, Widget, layout,
    parse_tracks,
};
use crate::style::{Resolved, States, Variant, ease, resolve};

macro_rules! ui_component {
    ($(#[$m:meta])* $name:ident { $($f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
        #[repr(C)]
        #[flecs(meta)]
        pub struct $name { $(pub $f: $t),* }
    };
}

ui_component!(
    /// Computed layout: the node's rectangle. Having one makes an entity a
    /// UI node.
    UiRect { x: f32, y: f32, w: f32, h: f32 }
);
ui_component!(
    /// Authored size per axis: kind 0 auto, 1 fixed, 2 fill (the value is the
    /// size or the weight).
    UiBox { width_kind: u32, width: f32, height_kind: u32, height: f32 }
);
ui_component!(
    /// How a node flows its children: direction (0 column, 1 row), cross
    /// alignment (0 start, 1 center, 2 end, 3 stretch), main justification
    /// (0 start, 1 center, 2 end, 3 space between), gap and padding.
    UiFlow {
        direction: u32,
        align: u32,
        justify: u32,
        gap: f32,
        pad_left: f32,
        pad_top: f32,
        pad_right: f32,
        pad_bottom: f32,
    }
);
ui_component!(
    /// Colors as `0xRRGGBBAA`. `accent` colors slider fill, toggle mark and
    /// focus ring.
    UiPaint { fill: u32, border: u32, border_width: f32, accent: u32 }
);
ui_component!(
    /// Text content: `text` is an id from `world.intern`. Per instance.
    UiLabel { text: u64 }
);
ui_component!(
    /// Text style: `color` is `0xRRGGBBAA`, `scale` multiplies the font
    /// (0 means 1), `wrap` 1 breaks lines to fit the node's width. Inherited
    /// from the prefab unless the instance sets its own.
    UiTextStyle { color: u32, scale: f32, wrap: u32 }
);
ui_component!(
    /// Added to the parent's z. Higher draws later and wins the pointer.
    UiZIndex { z: i32 }
);
ui_component!(
    /// Takes the node out of the flow: an offset from the parent's content
    /// box.
    UiAbsolute { x: f32, y: f32 }
);
ui_component!(
    /// What the node does: kind 0 none, 1 button, 2 toggle, 3 slider,
    /// 4 text input, 5 block. `state` is computed: bit 0 hovered, 1 pressed,
    /// 2 focused.
    UiWidget { kind: u32, enabled: u32, tab: i32, state: u32 }
);
ui_component!(
    /// Makes a node a scroll container: `offset` is how far its content is
    /// shifted along its flow direction. Computed by wheel, page and focus
    /// scrolling, clamped to the content.
    UiScroll { offset: f32 }
);
ui_component!(
    /// Marks a node as a modal (`on` 1): while one exists, only its subtree
    /// can take keyboard focus.
    UiModal { on: u32 }
);
ui_component!(
    /// A text input's caret, as a character index.
    UiCaret { pos: u32 }
);
ui_component!(
    /// What a text input keeps besides its text and caret. `flags`: 1 several
    /// lines (Enter adds a line, text wraps), 2 masked (a password), 4
    /// numeric (a number field: its `Slider` holds the value, range and step).
    /// `max_len` 0 means the default. `placeholder` is a string id shown
    /// while the text is empty. `anchor` is the other end of the selection,
    /// `view` the first visible column (or row), `want` the column vertical
    /// motion aims for plus one (0 for none).
    UiTextField {
        flags: u32,
        max_len: u32,
        placeholder: u64,
        anchor: u32,
        view: u32,
        want: u32
    }
);
ui_component!(
    /// A text input's undo and redo stacks: up to six snapshots each (string
    /// id and caret), oldest first, and what the last edit was.
    UiHistory {
        last: u32,
        un: u32,
        rn: u32,
        u0: u64, uc0: u32, u1: u64, uc1: u32, u2: u64, uc2: u32, u3: u64, uc3: u32, u4: u64, uc4: u32, u5: u64, uc5: u32,
        r0: u64, rc0: u32, r1: u64, rc1: u32, r2: u64, rc2: u32, r3: u64, rc3: u32, r4: u64, rc4: u32, r5: u64, rc5: u32
    }
);
ui_component!(
    /// Clipboard singleton. `text` is the string id of the clipboard's text
    /// (the host puts the system clipboard there before a paste); `out` is 1
    /// when a copy or cut changed it and the host should publish it.
    UiClipboard { text: u64, out: u32 }
);
ui_component!(
    /// Rich text: `markup` is a string id in the small markup form (`**bold**`,
    /// `` `code` ``, `[label](target)`, `{#RRGGBB colored}`). It replaces the
    /// node's `Label` as the text. Clicking a link emits `Navigate`.
    UiRich { markup: u64 }
);
ui_component!(UiNavigate {
    target: u64,
    link: u64
});
ui_component!(
    /// Spacing and limits, a style group: margins (left, top, right, bottom),
    /// smallest and largest size (0 for none), `aspect` (width over height,
    /// 0 for none), `align_self` (0 as the parent, 1 start, 2 center, 3 end,
    /// 4 stretch), and a relative `offset` that moves the node after layout.
    UiSpace {
        margin_left: f32,
        margin_top: f32,
        margin_right: f32,
        margin_bottom: f32,
        min_width: f32,
        min_height: f32,
        max_width: f32,
        max_height: f32,
        aspect: f32,
        align_self: u32,
        offset_x: f32,
        offset_y: f32
    }
);
ui_component!(
    /// Makes a node a grid. `cols` and `rows` are string ids of track lists
    /// like `"100 1fr auto"` (a number is a fixed size, `Nfr` a share of what
    /// is left, `auto` fits the content; rows past the list are auto).
    UiGrid { cols: u64, rows: u64, col_gap: f32, row_gap: f32 }
);
ui_component!(
    /// A grid child's cell: 1-based column and row (0 for the next free slot)
    /// and the tracks it spans (0 counts as 1).
    UiCell { col: u32, row: u32, col_span: u32, row_span: u32 }
);
ui_component!(
    /// Children that do not fit along the flow continue on a new line, `gap`
    /// apart (like `flex-wrap`).
    UiWrapFlow { on: u32, gap: f32 }
);
ui_component!(
    /// A drag handle's state: where the pointer was at the press (`on` 1
    /// while it is held).
    UiDrag { x: f32, y: f32, on: u32 }
);
ui_component!(UiDragged {
    target: u64,
    phase: u32,
    dx: f32,
    dy: f32
});
ui_component!(
    /// Marks a node that can be picked up and dropped: what it carries (any
    /// number the game gives meaning to) and its kind, 0 to 31.
    UiDraggable { payload: u64, kind: u32 }
);
ui_component!(
    /// Marks a node that accepts drops: a bit set of the kinds it takes (bit
    /// `k` for kind `k`). `over` is 1 while a drag of an accepted kind is
    /// above it.
    UiDropZone { kinds: u32, over: u32 }
);
ui_component!(
    /// A key chord that acts as a click on this node (on a node with no
    /// widget, a command: see `ui.command`). `key` is an upper-case
    /// ASCII letter or digit, or `0x100 + n - 1` for F`n`; `mods` is a bit set
    /// of 1 Ctrl, 2 Shift, 4 Alt.
    UiShortcut { key: u32, mods: u32 }
);
ui_component!(
    /// A vector shape drawn in the node's rectangle (a canvas item), in points
    /// relative to the node's top-left. `kind` 1 is a line from (x0, y0) to
    /// (x1, y1) of `width`; 2 a filled triangle through the three points; 3 a
    /// filled ellipse centered on (x0, y0) with radii (x1, y1).
    UiShape {
        kind: u32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: u32
    }
);
ui_component!(
    /// An embedded view of a simulation the host draws: which one (`source`,
    /// the id the host registers frames under) and, written by the UI each
    /// tick, the pointer in that simulation's world units (`x`, `y`, y up),
    /// whether it is over the view or dragging from it, and the mouse
    /// buttons held (1 primary, 2 secondary, 4 middle).
    UiViewport {
        source: u32,
        x: f32,
        y: f32,
        over: u32,
        down: u32
    }
);
ui_component!(
    /// What a node says about itself to a screen reader: a role (a number
    /// from `access::Role`; 0 keeps the one its widget implies), a heading
    /// level, `hidden` to leave it out, an explicit name and description
    /// (string ids), the node that names it (`labelled_by`) and, on a label,
    /// the control it names (`label_for`).
    UiAccess {
        role: u32,
        level: u32,
        hidden: u32,
        name: u64,
        description: u64,
        labelled_by: u64,
        label_for: u64
    }
);
ui_component!(
    /// A media player's state (see `ui.media_controls`): what it plays
    /// (`source`, an asset id the host knows; 0 for none), whether it is
    /// playing, its position and length in ticks, its volume (0 to 1) and
    /// whether it loops. The UI advances `position`; the host reads this to
    /// drive audio (`sage_platform::media`) and writes nothing back.
    UiMedia {
        source: u64,
        playing: u32,
        position: f32,
        length: f32,
        volume: f32,
        looping: u32
    }
);
ui_component!(
    /// An SVG document drawn to fit the node (`markup` is a string id). See
    /// `svg` for what it draws.
    UiSvg { markup: u64 }
);
ui_component!(
    /// Where a node goes among its siblings (CSS `order`): lower first, ties in
    /// creation order. Reordering rows is swapping keys.
    UiOrder { key: i32 }
);
ui_component!(
    /// A flow child of a scroll container that holds at `inset` from the top
    /// (left) of the view once scrolled past (`position: sticky`).
    UiSticky { inset: f32, on: u32 }
);
ui_component!(
    /// Marks a scroll container that also scrolls across its flow, and holds
    /// that offset (Shift and the wheel move it).
    UiScrollCross { on: u32, offset: f32 }
);
ui_component!(
    /// Native file dialog request and answer (a singleton). The UI sets
    /// `request` (1 open, 2 save) and `target` (the field that asked); the
    /// host shows its dialog, then sets `done` (1 a file was chosen, 2 it was
    /// cancelled) and `result` (a string id of the path).
    UiFileDialog {
        request: u32,
        target: u64,
        result: u64,
        done: u32
    }
);
ui_component!(
    /// The host's answer to a native file dialog, emitted on the field that
    /// asked: `done` 1 with the path (a string id), or 2 for a cancel.
    UiFileAnswered { target: u64, path: u64, done: u32 }
);
ui_component!(
    /// Input singleton: text an input method is composing, not yet committed
    /// (a string id; 0 for none). The focused text input draws it at its caret.
    UiComposition { text: u64 }
);
ui_component!(
    /// How much of an auto-height node's height shows, 0 to 1 (see
    /// `Node::reveal`). Absent means all of it.
    UiReveal { amount: f32 }
);
ui_component!(
    /// Input singleton: an action asked for by assistive technology (see
    /// `perform_access_action`). `kind` 1 focuses `target`, 2 clicks it; 0
    /// is none. The UI clears it once handled.
    UiAccessAction { target: u64, kind: u32 }
);
ui_component!(UiDragStarted {
    target: u64,
    payload: u64
});
ui_component!(UiDropped {
    target: u64,
    source: u64,
    payload: u64
});
ui_component!(UiDragEnded {
    target: u64,
    dropped: u32
});
ui_component!(UiMiddleClicked { target: u64 });
ui_component!(UiToggle { on: u32 });
ui_component!(UiSlider {
    value: f32,
    min: f32,
    max: f32,
    step: f32
});

ui_component!(
    /// An image in the node (`<img>`): `asset` is an `AssetId` (see
    /// `ui.asset(path)` in the kit), `fit` 0 stretch, 1 contain, 2 cover,
    /// 3 none, 4 scale-down, `tint` multiplies its colors. A sprite sheet
    /// has `cols` by `rows` frames (0 means 1) and shows `frame`.
    UiImage { asset: u64, fit: u32, tint: u32, frame: u32, cols: u32, rows: u32 }
);
ui_component!(
    /// A 9-slice image drawn as the node's background; the insets are in
    /// image pixels.
    UiSlice { asset: u64, left: f32, top: f32, right: f32, bottom: f32, tint: u32 }
);
ui_component!(
    /// Decoration: corner `radius`, a soft shadow (`shadow_blur` 0 for
    /// none), a two-color gradient from the fill to `grad_to` (`grad_dir` 0
    /// none, 1 down, 2 across), and `fade` from 0 opaque to 1 invisible,
    /// which multiplies down the tree.
    UiDecor {
        radius: f32,
        shadow_blur: f32,
        shadow_dy: f32,
        shadow_color: u32,
        grad_to: u32,
        grad_dir: u32,
        fade: f32,
    }
);
ui_component!(
    /// Style variants by state: the fill, text and border color each state
    /// overrides (0 leaves it). Later states win: checked, focus, hover,
    /// pressed, disabled. A button with no hover variant lightens, one with
    /// no pressed variant darkens, and a disabled one is dimmed.
    UiStates {
        checked_fill: u32, checked_text: u32, checked_border: u32,
        focus_fill: u32, focus_text: u32, focus_border: u32,
        hover_fill: u32, hover_text: u32, hover_border: u32,
        pressed_fill: u32, pressed_text: u32, pressed_border: u32,
        disabled_fill: u32, disabled_text: u32, disabled_border: u32,
    }
);
ui_component!(
    /// Colors ease to a new state instead of jumping: each tick closes
    /// `1 / ticks` of the remaining gap (so `ticks` is a time constant; the
    /// color is within a few units of its target after about three times
    /// that, and arrives exactly shortly after).
    UiTransition { ticks: u32 }
);
ui_component!(
    /// Computed: the colors a transition has reached.
    UiShown { fill: u32, text: u32, border: u32, fade: f32 }
);
ui_component!(
    /// `display: none` when `none` is 1: the node and its subtree take no
    /// space, draw nothing and take no pointer or focus.
    UiDisplay { none: u32 }
);

ui_component!(
    /// A floating node (popup, dropdown list, menu, tooltip, dialog): a root
    /// of the tree, drawn above everything else, placed next to the node
    /// whose StableId is `anchor` (or at `x`, `y`) and closed while `open`
    /// is 0. `placement`: 0 below, 1 above, 2 right, 3 left, 4 at the point;
    /// `align`: 0 start, 1 center, 2 end; `flags`: 1 flip when it would not
    /// fit, 2 as wide as the anchor, 4 dismiss on an outside press or
    /// Escape (the engine then sets `open` to 0 and emits `Dismissed`),
    /// 8 move focus into it when it opens (to the node with StableId
    /// `focus` if that is set, else the first focusable one). `was_open` and
    /// `restore` are computed: focus returns to `restore` when it closes.
    UiLayer {
        open: u32,
        anchor: u64,
        placement: u32,
        align: u32,
        gap: f32,
        x: f32,
        y: f32,
        flags: u32,
        focus: u64,
        was_open: u32,
        restore: u64,
    }
);
ui_component!(
    /// `on` 1: a secondary press over this node emits `ContextRequested`.
    /// `popup` is the menu layer the kit opens there (`ui.context_menu`).
    UiContext { on: u32, popup: u64 }
);
ui_component!(
    /// Text shown in a tooltip after the pointer rests on the node for
    /// `delay` ticks; `text` is an id from `world.intern`. The kit's tooltip
    /// system does the showing.
    UiTooltip { text: u64, delay: u32 }
);
ui_component!(
    /// A drop-down: this entity is the button showing the chosen option;
    /// `value` is its index (-1 for none), `popup` the StableId of the layer
    /// holding the options, and `label` the text node showing the chosen
    /// option. Made by `ui.select`.
    UiSelect { value: i32, count: u32, popup: u64, label: u64 }
);
ui_component!(
    /// One option of a select or combo box: its `index`, and the StableId of
    /// the `owner` it belongs to.
    UiOption { index: i32, owner: u64 }
);
ui_component!(
    /// A combo box (a text input with a filtered list of suggestions): the
    /// `popup` layer, and how many options are listed.
    UiCombo { popup: u64, count: u32 }
);
ui_component!(
    /// A menu item. `flags`: 1 check item, 2 checked, 4 separator, 8 opens a
    /// submenu, 16 disabled. `sub` is the submenu layer's StableId and `mark`
    /// the node that shows the check.
    UiMenuItem { id: u32, flags: u32, sub: u64, mark: u64 }
);
ui_component!(
    /// A menu bar title: the `popup` menu it opens and the bar it is in.
    UiMenuTitle { popup: u64, bar: u64 }
);
ui_component!(UiSelected {
    target: u64,
    index: i32
});
ui_component!(UiMenuChosen {
    target: u64,
    id: u32
});
ui_component!(UiDismissed { target: u64 });
ui_component!(UiContextRequested {
    target: u64,
    x: f32,
    y: f32
});
ui_component!(UiClicked { target: u64 });
ui_component!(UiToggled {
    target: u64,
    on: u32
});
ui_component!(UiChanged {
    target: u64,
    value: f32
});
ui_component!(UiTextChanged { target: u64 });
ui_component!(UiSubmitted { target: u64 });
ui_component!(UiFocused { target: u64 });

ui_component!(
    /// Input singleton: the pointer, in UI units. `present` 0 means none.
    /// `down` is a set of held buttons: 1 primary, 2 secondary, 4 middle.
    /// `wheel` is this tick's notches; positive scrolls content down.
    UiPointer { x: f32, y: f32, present: u32, down: u32, wheel: f32 }
);
ui_component!(
    /// Input singleton: `NAV_*` bits held this tick.
    UiNav { bits: u32 }
);
ui_component!(
    /// Input singleton: a key chord pressed this tick (`key` 0 for none); see
    /// [`UiShortcut`].
    UiChord { key: u32, mods: u32 }
);
ui_component!(
    /// State singleton: a press on a draggable node being followed, and the
    /// middle button's press.
    UiDnd {
        source: u64,
        x: f32,
        y: f32,
        dragging: u32,
        middle_active: u64,
        prev_middle: u32,
        press_age: u32,
        press_x: f32,
        press_y: f32,
        press_id: u64,
        clicks: u32,
        press_link: u32,
    }
);
ui_component!(
    /// Input singleton: the character typed this tick (0 for none).
    UiTextEntry { ch: u32 }
);
ui_component!(
    /// The UI's size in UI units.
    UiScreen { w: f32, h: f32 }
);
ui_component!(
    /// State singleton: what the interaction remembers between ticks.
    UiInteraction {
        hot: u64,
        active: u64,
        focus: u64,
        prev_down: u32,
        prev_secondary: u32,
        prev_nav: u32,
    }
);

const HISTORY_SLOTS: usize = 6;

fn history_from(h: &UiHistory, strings: &StringTable) -> crate::text::History {
    use crate::text::{History, Kind, Snapshot};
    let u = [
        (h.u0, h.uc0),
        (h.u1, h.uc1),
        (h.u2, h.uc2),
        (h.u3, h.uc3),
        (h.u4, h.uc4),
        (h.u5, h.uc5),
    ];
    let r = [
        (h.r0, h.rc0),
        (h.r1, h.rc1),
        (h.r2, h.rc2),
        (h.r3, h.rc3),
        (h.r4, h.rc4),
        (h.r5, h.rc5),
    ];
    let snap = |&(t, c): &(u64, u32)| Snapshot {
        text: strings.get(t).unwrap_or("").to_string(),
        caret: c as usize,
    };
    History {
        undo: u[..(h.un as usize).min(HISTORY_SLOTS)]
            .iter()
            .map(snap)
            .collect(),
        redo: r[..(h.rn as usize).min(HISTORY_SLOTS)]
            .iter()
            .map(snap)
            .collect(),
        last: Kind::from_u32(h.last),
    }
}

fn history_to(h: &crate::text::History, strings: &mut StringTable) -> UiHistory {
    let mut o = UiHistory {
        last: h.last.to_u32(),
        un: h.undo.len().min(HISTORY_SLOTS) as u32,
        rn: h.redo.len().min(HISTORY_SLOTS) as u32,
        ..UiHistory::default()
    };
    let mut put = |list: &[crate::text::Snapshot]| -> [(u64, u32); HISTORY_SLOTS] {
        let mut out = [(0u64, 0u32); HISTORY_SLOTS];
        for (k, snap) in list.iter().take(HISTORY_SLOTS).enumerate() {
            out[k] = (strings.intern(&snap.text), snap.caret as u32);
        }
        out
    };
    let u = put(&h.undo);
    let r = put(&h.redo);
    (o.u0, o.uc0) = u[0];
    (o.r0, o.rc0) = r[0];
    (o.u1, o.uc1) = u[1];
    (o.r1, o.rc1) = r[1];
    (o.u2, o.uc2) = u[2];
    (o.r2, o.rc2) = r[2];
    (o.u3, o.uc3) = u[3];
    (o.r3, o.rc3) = r[3];
    (o.u4, o.uc4) = u[4];
    (o.r4, o.rc4) = r[4];
    (o.u5, o.uc5) = u[5];
    (o.r5, o.rc5) = r[5];
    o
}

/// The natural size of every image asset the UI may show, by `AssetId`.
/// The host registers them as it loads images (`register_image`); layout
/// uses them to size an image whose box is `auto`.
#[derive(Default, Debug)]
pub struct ImageSizes {
    pub images: BTreeMap<u64, [u32; 2]>,
    /// What each embedded simulation last drew, by the id a `Viewport`
    /// component names (`set_viewport_frame`). Host data, not saved.
    pub viewports: BTreeMap<u32, sage_core::host::Frame>,
}

/// Tells the UI the pixel size of an image asset.
pub fn register_image(ecs: &Ecs, asset: u64, size: [u32; 2]) {
    ecs.ext::<ImageSizes>()
        .borrow_mut()
        .images
        .insert(asset, size);
}

/// Gives the `Viewport` components with this `source` the frame to draw. Call
/// it each tick with what the embedded simulation just produced; read the
/// pointer back from the component. The simulation, its state and its saves
/// belong to the host: the UI only draws and forwards.
pub fn set_viewport_frame(ecs: &Ecs, source: u32, frame: sage_core::host::Frame) {
    ecs.ext::<ImageSizes>()
        .borrow_mut()
        .viewports
        .insert(source, frame);
}

const STATE_HOVERED: u32 = 1;
const STATE_PRESSED: u32 = 2;
const STATE_FOCUSED: u32 = 4;

/// What was read for one node, kept to write back only what changed.
struct Source {
    entity: u64,
    sid: u64,
    rect: UiRect,
    widget: Option<UiWidget>,
    toggle: Option<UiToggle>,
    slider: Option<UiSlider>,
    scroll: Option<UiScroll>,
    caret: Option<UiCaret>,
    drag: Option<UiDrag>,
    zone: Option<UiDropZone>,
    viewport: Option<UiViewport>,
    cross: Option<UiScrollCross>,
    field: Option<UiTextField>,
    hist: Option<UiHistory>,
    label: Option<UiLabel>,
    shown: Option<UiShown>,
    transition: Option<u32>,
    layer: Option<UiLayer>,
    text: String,
}

fn entity_rows(world: &WorldRef) -> Vec<(u64, u64)> {
    let mut rows: Vec<(u64, u64)> = vec![];
    world
        .new_query::<(&StableIdC, &UiRect)>()
        .each_entity(|e, (sid, _)| rows.push((sid.value, *e.id())));
    rows.sort();
    rows
}

/// Reads the entities into a tree in StableId order. `computed` keeps the
/// stored hover, pressed and focused flags instead of leaving them for
/// `update` to compute.
fn build(
    world: &WorldRef,
    strings: &StringTable,
    sizes: &ImageSizes,
    computed: bool,
) -> (Tree, Vec<Source>) {
    let rows = entity_rows(world);
    let index: BTreeMap<u64, usize> = rows.iter().enumerate().map(|(i, r)| (r.1, i)).collect();
    let mut tree = Tree::default();
    let mut sources = vec![];
    let w = world.ptr_mut();
    for &(sid, entity) in &rows {
        let e = world.entity_from_id(entity);
        let rect = e.try_get::<&UiRect>(|r| *r).unwrap_or_default();
        let boxed = e.try_get::<&UiBox>(|b| *b);
        let flow = e.try_get::<&UiFlow>(|f| *f);
        let paint = e.try_get::<&UiPaint>(|p| *p);
        let label = e.try_get::<&UiLabel>(|l| *l);
        let style = e.try_get::<&UiTextStyle>(|t| *t);
        let z = e.try_get::<&UiZIndex>(|z| z.z);
        let abs = e.try_get::<&UiAbsolute>(|a| *a);
        let widget = e.try_get::<&UiWidget>(|w| *w);
        let toggle = e.try_get::<&UiToggle>(|t| *t);
        let slider = e.try_get::<&UiSlider>(|s| *s);
        let scroll = e.try_get::<&UiScroll>(|s| *s);
        let caret = e.try_get::<&UiCaret>(|c| *c);
        let field = e.try_get::<&UiTextField>(|c| *c);
        let rich = e.try_get::<&UiRich>(|c| *c);
        let drag = e.try_get::<&UiDrag>(|c| *c);
        let draggable = e.try_get::<&UiDraggable>(|c| *c);
        let zone = e.try_get::<&UiDropZone>(|c| *c);
        let shape = e.try_get::<&UiShape>(|c| *c);
        let access = e.try_get::<&UiAccess>(|c| *c);
        let svg = e.try_get::<&UiSvg>(|c| *c);
        let order = e.try_get::<&UiOrder>(|c| c.key);
        let reveal = e.try_get::<&UiReveal>(|c| c.amount);
        let sticky = e.try_get::<&UiSticky>(|c| *c);
        let cross = e.try_get::<&UiScrollCross>(|c| *c);
        let viewport = e.try_get::<&UiViewport>(|c| *c);
        let shortcut = e.try_get::<&UiShortcut>(|c| *c);
        let space = e.try_get::<&UiSpace>(|c| *c);
        let grid = e.try_get::<&UiGrid>(|c| *c);
        let cell = e.try_get::<&UiCell>(|c| *c);
        let wrapflow = e.try_get::<&UiWrapFlow>(|c| *c);
        let hist = e.try_get::<&UiHistory>(|c| *c);
        let modal = e.try_get::<&UiModal>(|m| m.on != 0);
        let image = e.try_get::<&UiImage>(|i| *i);
        let slice = e.try_get::<&UiSlice>(|i| *i);
        let decor = e.try_get::<&UiDecor>(|d| *d);
        let states = e.try_get::<&UiStates>(|s| *s);
        let transition = e.try_get::<&UiTransition>(|t| t.ticks);
        let shown = e.try_get::<&UiShown>(|s| *s);
        let display = e.try_get::<&UiDisplay>(|d| d.none != 0);
        let layer = e.try_get::<&UiLayer>(|l| *l);
        let tooltip = e.try_get::<&UiTooltip>(|_| true);
        let context = e.try_get::<&UiContext>(|c| c.on != 0);
        let parent = unsafe { sys::ecs_get_parent(w, entity) };
        let mut n = Node {
            id: sid,
            parent: index.get(&parent).copied(),
            z: z.unwrap_or(0),
            absolute: abs.map(|a| [a.x, a.y]),
            ..Node::default()
        };
        if let Some(b) = boxed {
            n.width = Size::from_parts(b.width_kind, b.width);
            n.height = Size::from_parts(b.height_kind, b.height);
        }
        if let Some(f) = flow {
            n.direction = Direction::from_u32(f.direction);
            n.align = Align::from_u32(f.align);
            n.justify = Justify::from_u32(f.justify);
            n.gap = f.gap;
            n.padding = [f.pad_left, f.pad_top, f.pad_right, f.pad_bottom];
        }
        if let Some(p) = paint {
            n.fill = p.fill;
            n.border = p.border;
            n.border_width = p.border_width;
            n.accent = p.accent;
        }
        let mut text = String::new();
        if let Some(l) = label {
            text = strings.get(l.text).unwrap_or("").to_string();
            n.text = text.clone();
        }
        if let Some(d) = drag.filter(|d| d.on != 0) {
            n.drag = Some([d.x, d.y]);
        }
        n.access = access.map(|a| AccessInfo {
            role: a.role,
            level: a.level,
            hidden: a.hidden != 0,
            name: strings.get(a.name).unwrap_or("").to_string(),
            description: strings.get(a.description).unwrap_or("").to_string(),
            labelled_by: a.labelled_by,
            label_for: a.label_for,
        });
        if let Some(v) = svg {
            match crate::svg::parse(strings.get(v.markup).unwrap_or("")) {
                Ok(doc) => {
                    // With no size of its own, the node is as big as the
                    // document's view box.
                    if matches!(n.width, Size::Auto) {
                        n.width = Size::Fixed(doc.view[2]);
                    }
                    if matches!(n.height, Size::Auto) {
                        n.height = Size::Fixed(doc.view[3]);
                    }
                    n.svg = Some(doc);
                }
                Err(_) => n.svg_error = true,
            }
        }
        n.order = order.unwrap_or(0);
        n.reveal = reveal.unwrap_or(1.0);
        n.sticky = sticky.filter(|s| s.on != 0).map(|s| s.inset);
        if let Some(c) = cross.filter(|c| c.on != 0) {
            n.scroll_cross = true;
            n.scroll_offset_cross = c.offset;
        }
        n.shape = shape.map(|s| ShapeNode {
            kind: s.kind,
            pts: [[s.x0, s.y0], [s.x1, s.y1], [s.x2, s.y2]],
            width: s.width,
            color: s.color,
        });
        n.viewport = viewport.map(|v| ViewportNode {
            frame: sizes.viewports.get(&v.source).cloned(),
            ..ViewportNode::default()
        });
        n.draggable = draggable.map(|d| (d.payload, d.kind));
        n.drop_kinds = zone.map_or(0, |z| z.kinds);
        n.shortcut = shortcut.filter(|s| s.key != 0).map(|s| [s.key, s.mods]);
        if let Some(sp) = space {
            n.margin = [
                sp.margin_left,
                sp.margin_top,
                sp.margin_right,
                sp.margin_bottom,
            ];
            n.size_min = [sp.min_width, sp.min_height];
            n.size_max = [sp.max_width, sp.max_height];
            n.aspect = sp.aspect;
            n.align_self = match sp.align_self {
                1 => Some(Align::Start),
                2 => Some(Align::Center),
                3 => Some(Align::End),
                4 => Some(Align::Stretch),
                _ => None,
            };
            n.offset = [sp.offset_x, sp.offset_y];
        }
        if let Some(g) = grid {
            n.grid = Some(GridNode {
                cols: parse_tracks(strings.get(g.cols).unwrap_or("")),
                rows: parse_tracks(strings.get(g.rows).unwrap_or("")),
                col_gap: g.col_gap,
                row_gap: g.row_gap,
            });
        }
        if let Some(cl) = cell {
            n.cell = Some(CellNode {
                col: cl.col,
                row: cl.row,
                col_span: cl.col_span,
                row_span: cl.row_span,
            });
        }
        if let Some(w) = wrapflow {
            n.flow_wrap = w.on != 0;
            n.cross_gap = w.gap;
        }
        if let Some(r) = rich {
            let parsed = crate::text::parse_markup(strings.get(r.markup).unwrap_or(""));
            n.text = parsed.text.clone();
            n.hoverable |= !parsed.links.is_empty();
            n.rich = Some(parsed);
        }
        if let Some(t) = style {
            n.text_color = t.color;
            n.text_scale = if t.scale > 0.0 { t.scale } else { 1.0 };
            n.wrap = t.wrap != 0;
        }
        if let Some(sc) = scroll {
            n.scroll = true;
            n.scroll_offset = sc.offset;
        }
        if let Some(c) = caret {
            n.caret = c.pos as usize;
        }
        if let Some(t) = field {
            n.field.multiline = t.flags & 1 != 0;
            n.field.masked = t.flags & 2 != 0;
            n.field.numeric = t.flags & 4 != 0;
            if t.max_len != 0 {
                n.field.max_len = t.max_len as usize;
            }
            n.field.placeholder = strings.get(t.placeholder).unwrap_or("").to_string();
            n.field.anchor = t.anchor as usize;
            n.field.view = t.view as usize;
            n.field.want = (t.want as usize).wrapping_sub(1);
        } else {
            n.field.anchor = n.caret;
        }
        if let Some(h) = &hist {
            n.field.hist = history_from(h, strings);
        }
        n.modal = modal.unwrap_or(false);
        n.hidden = display.unwrap_or(false);
        n.context = context.unwrap_or(false);
        n.hoverable |= tooltip.unwrap_or(false);
        if let Some(l) = layer {
            n.hidden |= l.open == 0;
            n.layer = Some(LayerNode {
                anchor: l.anchor,
                placement: Placement::from_u32(l.placement),
                align: Edge::from_u32(l.align),
                gap: l.gap,
                point: [l.x, l.y],
                flip: l.flags & 1 != 0,
                match_width: l.flags & 2 != 0,
                dismiss: l.flags & 4 != 0,
            });
            // Layers float above ordinary content unless told otherwise.
            n.z += 1000;
        }
        if let Some(i) = image {
            n.image = Some(ImageNode {
                asset: i.asset,
                fit: Fit::from_u32(i.fit),
                tint: i.tint,
                size: sizes
                    .images
                    .get(&i.asset)
                    .map_or([0.0; 2], |s| [s[0] as f32, s[1] as f32]),
                frame: i.frame,
                cols: i.cols,
                rows: i.rows,
            });
        }
        if let Some(sl) = slice {
            n.slice = Some(SliceNode {
                asset: sl.asset,
                insets: [sl.left, sl.top, sl.right, sl.bottom],
                tint: sl.tint,
            });
        }
        if let Some(d) = decor {
            n.radius = d.radius;
            n.fade = d.fade;
            if d.shadow_blur > 0.0 {
                n.shadow = Some(ShadowNode {
                    blur: d.shadow_blur,
                    dy: d.shadow_dy,
                    color: d.shadow_color,
                });
            }
            if d.grad_dir != 0 {
                n.gradient = Some(GradientNode {
                    to: d.grad_to,
                    vertical: d.grad_dir == 1,
                });
            }
        }
        if let Some(s) = states {
            let v = |fill, text, border| Variant { fill, text, border };
            n.states = States {
                checked: v(s.checked_fill, s.checked_text, s.checked_border),
                focus: v(s.focus_fill, s.focus_text, s.focus_border),
                hover: v(s.hover_fill, s.hover_text, s.hover_border),
                pressed: v(s.pressed_fill, s.pressed_text, s.pressed_border),
                disabled: v(s.disabled_fill, s.disabled_text, s.disabled_border),
            };
        }
        if let Some(sh) = shown {
            n.shown = Some(Resolved {
                fill: sh.fill,
                text: sh.text,
                border: sh.border,
                fade: sh.fade,
            });
        }
        if let Some(w) = widget {
            n.widget = Widget::from_u32(w.kind);
            n.enabled = w.enabled != 0;
            n.tab = w.tab;
            if computed {
                n.hovered = w.state & STATE_HOVERED != 0;
                n.pressed = w.state & STATE_PRESSED != 0;
                n.focused = w.state & STATE_FOCUSED != 0;
            }
        }
        if let Some(t) = toggle {
            n.on = t.on != 0;
        }
        if let Some(s) = slider {
            n.value = s.value;
            n.min = s.min;
            n.max = s.max;
            n.step = s.step;
        }
        tree.nodes.push(n);
        sources.push(Source {
            entity,
            sid,
            rect,
            widget,
            toggle,
            slider,
            scroll,
            caret,
            drag,
            zone,
            viewport,
            cross,
            field,
            hist,
            label,
            shown,
            transition,
            layer,
            text,
        });
    }
    // The pre-edit text goes to the focused text input.
    let pre = world.get::<&UiComposition>(|c| c.text);
    let focus = world.get::<&UiInteraction>(|i| i.focus);
    if pre != 0
        && let Some(text) = strings.get(pre)
        && let Some(n) = tree
            .nodes
            .iter_mut()
            .find(|n| n.id == focus && n.widget == Widget::TextInput)
    {
        n.field.preedit = text.to_string();
    }
    (tree, sources)
}

/// One tick of the UI: layout, interaction, then write back and emit.
fn step(world: &WorldRef, strings: &RefCell<StringTable>, sizes: &RefCell<ImageSizes>) {
    let (mut tree, sources) = build(world, &strings.borrow(), &sizes.borrow(), false);
    let screen = world.get::<&UiScreen>(|s| *s);
    layout(&mut tree, [screen.w, screen.h]);

    let pointer = world.get::<&UiPointer>(|p| *p);
    let nav = world.get::<&UiNav>(|n| n.bits);
    let ch = world
        .get::<&UiTextEntry>(|t| char::from_u32(t.ch))
        .filter(|c| *c != '\0');
    let before = world.get::<&UiInteraction>(|i| *i);
    let dnd_before = world.get::<&UiDnd>(|d| *d);
    let mut state = UiState {
        hot: before.hot,
        active: before.active,
        focus: before.focus,
        prev_down: before.prev_down != 0,
        prev_secondary: before.prev_secondary != 0,
        prev_nav: before.prev_nav,
        prev_middle: dnd_before.prev_middle != 0,
        middle_active: dnd_before.middle_active,
        dnd: Dnd {
            source: dnd_before.source,
            origin: [dnd_before.x, dnd_before.y],
            dragging: dnd_before.dragging != 0,
        },
        last_press: LastPress {
            age: dnd_before.press_age,
            pos: [dnd_before.press_x, dnd_before.press_y],
            id: dnd_before.press_id,
            count: dnd_before.clicks,
        },
        press_link: dnd_before.press_link,
        scrolled: false,
    };
    let chord = world.get::<&UiChord>(|c| *c);
    let input = UiInput {
        pointer: (pointer.present != 0).then_some([pointer.x, pointer.y]),
        down: pointer.down & 1 != 0,
        secondary: pointer.down & 2 != 0,
        middle: pointer.down & 4 != 0,
        access: {
            let a = world.get::<&UiAccessAction>(|a| *a);
            (a.kind != 0).then_some((a.target, a.kind))
        },
        chord: (chord.key != 0).then_some([chord.key, chord.mods]),
        nav,
        ch,
        wheel: pointer.wheel,
        paste: {
            let t = world.get::<&UiClipboard>(|c| c.text);
            strings.borrow().get(t).map(str::to_string)
        },
    };
    world.set(UiAccessAction::default());
    // A modal drops focus that is outside it; a layer that opens remembers
    // the focus from before that, to give it back on close.
    let focus_before_update = state.focus;
    let events = update(&mut tree, &mut state, &input);
    if state.scrolled {
        // Scrolling moved the content: place it again before it is stored.
        layout(&mut tree, [screen.w, screen.h]);
    }

    for (src, n) in sources.iter().zip(&tree.nodes) {
        let e = world.entity_from_id(src.entity);
        let rect = UiRect {
            x: n.rect.x,
            y: n.rect.y,
            w: n.rect.w,
            h: n.rect.h,
        };
        if rect != src.rect {
            e.set(rect);
        }
        if let Some(old) = src.widget {
            let state_bits = (n.hovered as u32 * STATE_HOVERED)
                | (n.pressed as u32 * STATE_PRESSED)
                | (n.focused as u32 * STATE_FOCUSED);
            if state_bits != old.state {
                e.set(UiWidget {
                    state: state_bits,
                    ..old
                });
            }
        }
        if let Some(old) = src.toggle
            && (old.on != 0) != n.on
        {
            e.set(UiToggle { on: n.on as u32 });
        }
        if let Some(old) = src.scroll
            && old.offset != n.scroll_offset
        {
            e.set(UiScroll {
                offset: n.scroll_offset,
            });
        }
        if let Some(old) = src.caret
            && old.pos as usize != n.caret
        {
            e.set(UiCaret {
                pos: n.caret as u32,
            });
        }
        if let Some(old) = src.slider
            && old.value != n.value
        {
            e.set(UiSlider {
                value: n.value,
                ..old
            });
        }
        if let Some(old) = src.drag {
            let new = match n.drag {
                Some([x, y]) => UiDrag { x, y, on: 1 },
                None => UiDrag { on: 0, ..old },
            };
            if new != old {
                e.set(new);
            }
        }
        if let Some(old) = src.viewport {
            let new = match &n.viewport {
                Some(v) => {
                    let world = v.frame.as_ref().map_or([0.0; 2], |f| f.world_size);
                    let (scale, origin) = crate::draw::viewport_fit(n.rect, world);
                    match v.pointer {
                        Some(p) if scale > 0.0 => UiViewport {
                            x: (p[0] + n.rect.x - origin[0]) / scale,
                            y: world[1] - (p[1] + n.rect.y - origin[1]) / scale,
                            over: 1,
                            down: v.buttons,
                            ..old
                        },
                        _ => UiViewport {
                            over: 0,
                            down: 0,
                            ..old
                        },
                    }
                }
                None => old,
            };
            if new != old {
                e.set(new);
            }
        }
        if let Some(old) = src.cross
            && old.offset != n.scroll_offset_cross
        {
            e.set(UiScrollCross {
                offset: n.scroll_offset_cross,
                ..old
            });
        }
        if let Some(old) = src.zone
            && (old.over != 0) != n.drop_over
        {
            e.set(UiDropZone {
                over: n.drop_over as u32,
                ..old
            });
        }
        if let Some(old) = src.field {
            let new = UiTextField {
                anchor: n.field.anchor as u32,
                view: n.field.view as u32,
                want: n.field.want.wrapping_add(1) as u32,
                ..old
            };
            if new != old {
                e.set(new);
            }
        }
        if let Some(old) = src.hist {
            let h = history_to(&n.field.hist, &mut strings.borrow_mut());
            if h != old {
                e.set(h);
            }
        }
        if src.label.is_some() && n.widget == Widget::TextInput && n.text != src.text {
            let id = strings.borrow_mut().intern(&n.text);
            e.set(UiLabel { text: id });
        }
    }
    // Layers: a dismissed layer closes itself, and focus moves into a layer
    // that opened and returns from one that closed.
    let dismissed: Vec<u64> = events
        .iter()
        .filter(|e| e.kind == EventKind::Dismissed)
        .map(|e| e.target)
        .collect();
    for (k, src) in sources.iter().enumerate() {
        let Some(old) = src.layer else {
            continue;
        };
        let mut next = old;
        if dismissed.contains(&src.sid) {
            next.open = 0;
        }
        let open = next.open != 0;
        let was = old.was_open != 0;
        if open && !was {
            next.restore = if state.focus != 0 {
                state.focus
            } else {
                focus_before_update
            };
            if next.flags & 8 != 0 {
                let want = if next.focus != 0 {
                    next.focus
                } else {
                    crate::interact::first_focusable(&tree, k)
                };
                if want != 0 {
                    state.focus = want;
                }
            }
        } else if !open && was {
            if next.restore != 0 && tree.nodes.iter().any(|n| n.id == next.restore) {
                state.focus = next.restore;
            }
            next.restore = 0;
        }
        next.was_open = open as u32;
        if next != old {
            world.entity_from_id(src.entity).set(next);
        }
    }

    // Transitions: ease the shown colors towards what the state resolves to.
    // A node that has never been shown starts at its target (no animation on
    // the first frame).
    for (src, n) in sources.iter().zip(&tree.nodes) {
        let Some(ticks) = src.transition else {
            continue;
        };
        let target = resolve(&Node {
            shown: None,
            ..n.clone()
        });
        let next = match src.shown {
            Some(old) => ease(
                &Resolved {
                    fill: old.fill,
                    text: old.text,
                    border: old.border,
                    fade: old.fade,
                },
                &target,
                ticks,
            ),
            None => target,
        };
        let shown = UiShown {
            fill: next.fill,
            text: next.text,
            border: next.border,
            fade: next.fade,
        };
        if src.shown != Some(shown) {
            world.entity_from_id(src.entity).set(shown);
        }
    }
    world.set(UiInteraction {
        hot: state.hot,
        active: state.active,
        focus: state.focus,
        prev_down: state.prev_down as u32,
        prev_secondary: state.prev_secondary as u32,
        prev_nav: state.prev_nav,
    });
    world.set(UiDnd {
        source: state.dnd.source,
        x: state.dnd.origin[0],
        y: state.dnd.origin[1],
        dragging: state.dnd.dragging as u32,
        middle_active: state.middle_active,
        prev_middle: state.prev_middle as u32,
        press_age: state.last_press.age,
        press_x: state.last_press.pos[0],
        press_y: state.last_press.pos[1],
        press_id: state.last_press.id,
        clicks: state.last_press.count,
        press_link: state.press_link,
    });

    // Events last, so observers see the updated state.
    let by_sid: BTreeMap<u64, u64> = sources.iter().map(|s| (s.sid, s.entity)).collect();
    for ev in events {
        if let EventKind::Copied(text) = &ev.kind {
            let id = strings.borrow_mut().intern(text);
            world.set(UiClipboard { text: id, out: 1 });
            continue;
        }
        let Some(&entity) = by_sid.get(&ev.target) else {
            continue;
        };
        let e = world.entity_from_id(entity);
        let target = ev.target;
        match ev.kind {
            EventKind::Clicked => e.emit(&UiClicked { target }),
            EventKind::Toggled(on) => e.emit(&UiToggled {
                target,
                on: on as u32,
            }),
            EventKind::Changed(value) => e.emit(&UiChanged { target, value }),
            EventKind::TextChanged => e.emit(&UiTextChanged { target }),
            EventKind::Submitted => e.emit(&UiSubmitted { target }),
            EventKind::Focused => e.emit(&UiFocused { target }),
            EventKind::Dismissed => e.emit(&UiDismissed { target }),
            EventKind::ContextRequested(x, y) => e.emit(&UiContextRequested { target, x, y }),
            EventKind::Navigate(link) => {
                let link = strings.borrow_mut().intern(&link);
                e.emit(&UiNavigate { target, link })
            }
            EventKind::Drag(phase, dx, dy) => e.emit(&UiDragged {
                target,
                phase,
                dx,
                dy,
            }),
            EventKind::MiddleClicked => e.emit(&UiMiddleClicked { target }),
            EventKind::DragStarted(payload) => e.emit(&UiDragStarted { target, payload }),
            EventKind::Dropped(source, payload) => e.emit(&UiDropped {
                target,
                source,
                payload,
            }),
            EventKind::DragEnded(dropped) => e.emit(&UiDragEnded {
                target,
                dropped: dropped as u32,
            }),
            EventKind::Copied(_) => {}
        };
    }
}

/// The accessibility tree of the current UI (see `access`): what a screen
/// reader would be told. Reads only.
pub fn access_tree(world: &World, ecs: &Ecs) -> Vec<crate::access::AccessNode> {
    let screen = world.get::<&UiScreen>(|s| *s);
    let focus = world.get::<&UiInteraction>(|i| i.focus);
    let strings = ecs.strings();
    let sizes = ecs.ext::<ImageSizes>();
    let (mut tree, _) = build(&world.world(), &strings.borrow(), &sizes.borrow(), true);
    layout(&mut tree, [screen.w, screen.h]);
    crate::access::tree(&tree, focus)
}

/// Tells the UI the answer to a native file dialog it asked for (see
/// [`UiFileDialog`]): `Some(path)` for a chosen file, `None` for a cancel. The
/// field that asked gets a `FileAnswered` event.
pub fn answer_file_dialog(world: &World, ecs: &Ecs, path: Option<&str>) {
    let d = world.get::<&UiFileDialog>(|d| *d);
    let result = path.map_or(0, |p| ecs.intern(p));
    let done = if path.is_some() { 1 } else { 2 };
    world.set(UiFileDialog {
        request: 0,
        target: 0,
        result: 0,
        done: 0,
    });
    if let Some(e) = ecs.entity_of_sid(d.target) {
        world.entity_from_id(e).emit(&UiFileAnswered {
            target: d.target,
            path: result,
            done,
        });
    }
}

/// Queues an action from assistive technology for the next tick: focus the
/// node with this StableId, or click it. Returns false for an action the UI
/// does not do.
pub fn perform_access_action(world: &World, target: u64, action: accesskit::Action) -> bool {
    let kind = match action {
        accesskit::Action::Focus => 1,
        accesskit::Action::Click => 2,
        _ => return false,
    };
    world.set(UiAccessAction { target, kind });
    true
}

/// The accessibility tree as an `accesskit` update, for a platform adapter.
pub fn access_update(world: &World, ecs: &Ecs) -> accesskit::TreeUpdate {
    let screen = world.get::<&UiScreen>(|s| *s);
    let focus = world.get::<&UiInteraction>(|i| i.focus);
    crate::access_kit::tree_update(&access_tree(world, ecs), focus, [screen.w, screen.h])
}

/// [`access_tree`] as indented text, one node a line.
pub fn access_outline(world: &World, ecs: &Ecs) -> String {
    crate::access::outline(&access_tree(world, ecs))
}

/// The current UI as plain draw data, for presentation. Reads only.
pub fn frame(world: &World, ecs: &Ecs) -> UiFrame {
    let screen = world.get::<&UiScreen>(|s| *s);
    let strings = ecs.strings();
    let sizes = ecs.ext::<ImageSizes>();
    let (mut tree, _) = build(&world.world(), &strings.borrow(), &sizes.borrow(), true);
    // Layout is pure and cheap, and gives each node its clip and wrapped
    // lines, which are not stored in components.
    layout(&mut tree, [screen.w, screen.h]);
    crate::draw::draw(&tree, [screen.w, screen.h])
}

fn install(
    w: *mut sys::ecs_world_t,
    phase: u64,
    parent: u64,
    strings: Rc<RefCell<StringTable>>,
    sizes: Rc<RefCell<ImageSizes>>,
) -> Result<u64, String> {
    let world = unsafe { WorldRef::from_ptr(w) };
    let named = |parent: u64, name: &str| {
        let name = CString::new(name).unwrap();
        let desc = sys::ecs_entity_desc_t {
            name: name.as_ptr(),
            parent,
            sep: c"".as_ptr(),
            ..Default::default()
        };
        unsafe { sys::ecs_entity_init(w, &desc) }
    };
    let module = named(parent, "ui");
    unsafe { sys::ecs_add_id(w, module, sys::EcsModule) };
    let prev = unsafe { sys::ecs_set_scope(w, module) };
    // Immediate: it reads and writes many entities in order, and observers
    // of its events run before the next system sees the state.
    world
        .system_named::<(&StableIdC, &UiRect)>("Update")
        .kind(phase)
        .immediate(true)
        .run(move |mut it| {
            // Drain the iterator: the work is done over the whole world.
            while it.next() {}
            step(&it.world(), &strings, &sizes);
        });
    unsafe { sys::ecs_set_scope(w, prev) };
    Ok(module)
}

/// Registers the UI vocabulary and the `ui` native flecs module on a world
/// bound to `ecs`. Scripts then `from sage.ui import Button...` and
/// instantiate the module with `world.import_native("ui", phase)`.
pub fn setup(world: &World, ecs: &Ecs) -> Result<(), String> {
    macro_rules! register {
        ($($t:ty => $name:literal),* $(,)?) => {
            $( ecs.register_native(*world.component_named::<$t>($name).id())?; )*
        };
    }
    register!(
        UiRect => "sage::ui::Rect",
        UiBox => "sage::ui::Box",
        UiFlow => "sage::ui::Flow",
        UiPaint => "sage::ui::Paint",
        UiLabel => "sage::ui::Label",
        UiTextStyle => "sage::ui::TextStyle",
        UiZIndex => "sage::ui::ZIndex",
        UiAbsolute => "sage::ui::Absolute",
        UiWidget => "sage::ui::Widget",
        UiToggle => "sage::ui::Toggle",
        UiScroll => "sage::ui::Scroll",
        UiCaret => "sage::ui::Caret",
        UiTextField => "sage::ui::TextField",
        UiRich => "sage::ui::Rich",
        UiDrag => "sage::ui::Drag",
        UiDragged => "sage::ui::Dragged",
        UiDraggable => "sage::ui::Draggable",
        UiDropZone => "sage::ui::DropZone",
        UiShortcut => "sage::ui::Shortcut",
        UiAccess => "sage::ui::Access",
        UiMedia => "sage::ui::Media",
        UiSvg => "sage::ui::Svg",
        UiAccessAction => "sage::ui::AccessAction",
        UiReveal => "sage::ui::Reveal",
        UiFileDialog => "sage::ui::FileDialog",
        UiComposition => "sage::ui::Composition",
        UiFileAnswered => "sage::ui::FileAnswered",
        UiOrder => "sage::ui::Order",
        UiSticky => "sage::ui::Sticky",
        UiScrollCross => "sage::ui::ScrollCross",
        UiShape => "sage::ui::Shape",
        UiViewport => "sage::ui::Viewport",
        UiDragStarted => "sage::ui::DragStarted",
        UiDropped => "sage::ui::Dropped",
        UiDragEnded => "sage::ui::DragEnded",
        UiMiddleClicked => "sage::ui::MiddleClicked",
        UiChord => "sage::ui::Chord",
        UiDnd => "sage::ui::Dnd",
        UiSpace => "sage::ui::Space",
        UiGrid => "sage::ui::Grid",
        UiCell => "sage::ui::Cell",
        UiWrapFlow => "sage::ui::WrapFlow",
        UiNavigate => "sage::ui::Navigate",
        UiHistory => "sage::ui::History",
        UiClipboard => "sage::ui::Clipboard",
        UiModal => "sage::ui::Modal",
        UiImage => "sage::ui::Image",
        UiSlice => "sage::ui::Slice",
        UiDecor => "sage::ui::Decor",
        UiStates => "sage::ui::States",
        UiTransition => "sage::ui::Transition",
        UiShown => "sage::ui::Shown",
        UiDisplay => "sage::ui::Display",
        UiLayer => "sage::ui::Layer",
        UiContext => "sage::ui::Context",
        UiTooltip => "sage::ui::Tooltip",
        UiDismissed => "sage::ui::Dismissed",
        UiSelect => "sage::ui::Select",
        UiOption => "sage::ui::Option",
        UiCombo => "sage::ui::Combo",
        UiMenuItem => "sage::ui::MenuItem",
        UiMenuTitle => "sage::ui::MenuTitle",
        UiSelected => "sage::ui::Selected",
        UiMenuChosen => "sage::ui::MenuChosen",
        UiContextRequested => "sage::ui::ContextRequested",
        UiSlider => "sage::ui::Slider",
        UiClicked => "sage::ui::Clicked",
        UiToggled => "sage::ui::Toggled",
        UiChanged => "sage::ui::Changed",
        UiTextChanged => "sage::ui::TextChanged",
        UiSubmitted => "sage::ui::Submitted",
        UiFocused => "sage::ui::Focused",
        UiPointer => "sage::ui::Pointer",
        UiNav => "sage::ui::Nav",
        UiTextEntry => "sage::ui::TextEntry",
        UiScreen => "sage::ui::Screen",
        UiInteraction => "sage::ui::Interaction",
    );
    // Style groups are inherited: an instance reads its prefab's value until
    // it sets its own, so editing a style prefab restyles every instance that
    // has not overridden that group (design doc 5.2 and 9.2). Everything else
    // (content, widget state, layout results) is copied per instance.
    macro_rules! inherit {
        ($($t:ty),*) => {
            $( unsafe {
                sys::ecs_add_id(
                    world.ptr_mut(),
                    *world.component::<$t>().id(),
                    ecs_pair(sys::EcsOnInstantiate, sys::EcsInherit),
                );
            } )*
        };
    }
    inherit!(
        UiBox,
        UiFlow,
        UiPaint,
        UiTextStyle,
        UiDecor,
        UiSlice,
        UiStates,
        UiTransition,
        UiSpace
    );
    world.set(UiPointer::default());
    world.set(UiNav::default());
    world.set(UiChord::default());
    world.set(UiAccessAction::default());
    world.set(UiFileDialog::default());
    world.set(UiComposition::default());
    world.set(UiDnd::default());
    world.set(UiTextEntry::default());
    world.set(UiClipboard::default());
    world.set(UiScreen { w: 320.0, h: 180.0 });
    world.set(UiInteraction::default());
    let strings = ecs.strings();
    let sizes = ecs.ext::<ImageSizes>();
    ecs.add_native_module("ui", move |w, phase, parent| {
        install(w, phase, parent, strings.clone(), sizes.clone())
    });
    Ok(())
}
