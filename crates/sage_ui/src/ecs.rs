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

use crate::interact::{EventKind, UiInput, UiState, update};
use crate::layout::{
    Align, Direction, Fit, GradientNode, ImageNode, Justify, Node, ShadowNode, Size, SliceNode,
    Tree, Widget, layout,
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
    /// `wheel` is this tick's notches; positive scrolls content down.
    UiPointer { x: f32, y: f32, present: u32, down: u32, wheel: f32 }
);
ui_component!(
    /// Input singleton: `NAV_*` bits held this tick.
    UiNav { bits: u32 }
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
    UiInteraction { hot: u64, active: u64, focus: u64, prev_down: u32, prev_nav: u32 }
);

/// The natural size of every image asset the UI may show, by `AssetId`.
/// The host registers them as it loads images (`register_image`); layout
/// uses them to size an image whose box is `auto`.
#[derive(Default, Debug)]
pub struct ImageSizes(pub BTreeMap<u64, [u32; 2]>);

/// Tells the UI the pixel size of an image asset.
pub fn register_image(ecs: &Ecs, asset: u64, size: [u32; 2]) {
    ecs.ext::<ImageSizes>().borrow_mut().0.insert(asset, size);
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
    label: Option<UiLabel>,
    shown: Option<UiShown>,
    transition: Option<u32>,
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
        let modal = e.try_get::<&UiModal>(|m| m.on != 0);
        let image = e.try_get::<&UiImage>(|i| *i);
        let slice = e.try_get::<&UiSlice>(|i| *i);
        let decor = e.try_get::<&UiDecor>(|d| *d);
        let states = e.try_get::<&UiStates>(|s| *s);
        let transition = e.try_get::<&UiTransition>(|t| t.ticks);
        let shown = e.try_get::<&UiShown>(|s| *s);
        let display = e.try_get::<&UiDisplay>(|d| d.none != 0);
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
        n.modal = modal.unwrap_or(false);
        n.hidden = display.unwrap_or(false);
        if let Some(i) = image {
            n.image = Some(ImageNode {
                asset: i.asset,
                fit: Fit::from_u32(i.fit),
                tint: i.tint,
                size: sizes
                    .0
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
            label,
            shown,
            transition,
            text,
        });
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
    let mut state = UiState {
        hot: before.hot,
        active: before.active,
        focus: before.focus,
        prev_down: before.prev_down != 0,
        prev_nav: before.prev_nav,
        scrolled: false,
    };
    let input = UiInput {
        pointer: (pointer.present != 0).then_some([pointer.x, pointer.y]),
        down: pointer.down != 0,
        nav,
        ch,
        wheel: pointer.wheel,
    };
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
        if src.label.is_some() && n.widget == Widget::TextInput && n.text != src.text {
            let id = strings.borrow_mut().intern(&n.text);
            e.set(UiLabel { text: id });
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
        prev_nav: state.prev_nav,
    });

    // Events last, so observers see the updated state.
    let by_sid: BTreeMap<u64, u64> = sources.iter().map(|s| (s.sid, s.entity)).collect();
    for ev in events {
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
        };
    }
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
        UiModal => "sage::ui::Modal",
        UiImage => "sage::ui::Image",
        UiSlice => "sage::ui::Slice",
        UiDecor => "sage::ui::Decor",
        UiStates => "sage::ui::States",
        UiTransition => "sage::ui::Transition",
        UiShown => "sage::ui::Shown",
        UiDisplay => "sage::ui::Display",
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
        UiTransition
    );
    world.set(UiPointer::default());
    world.set(UiNav::default());
    world.set(UiTextEntry::default());
    world.set(UiScreen { w: 320.0, h: 180.0 });
    world.set(UiInteraction::default());
    let strings = ecs.strings();
    let sizes = ecs.ext::<ImageSizes>();
    ecs.add_native_module("ui", move |w, phase, parent| {
        install(w, phase, parent, strings.clone(), sizes.clone())
    });
    Ok(())
}
