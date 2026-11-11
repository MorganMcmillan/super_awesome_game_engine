# Generated from the reflection data of the host's native components
# (sage_ecs_py `Ecs::type_stubs`). Editors read this; the engine never does.
# Do not edit by hand; regenerate it instead.

from sage.ecs import Component, f32, i32, u32, u64


class Absolute(Component):
    """Native component `sage.ui.Absolute`."""

    x: f32
    y: f32
    def __init__(self, x: f32 = ..., y: f32 = ...) -> None: ...


class Box(Component):
    """Native component `sage.ui.Box`."""

    width_kind: u32
    width: f32
    height_kind: u32
    height: f32
    def __init__(self, width_kind: u32 = ..., width: f32 = ..., height_kind: u32 = ..., height: f32 = ...) -> None: ...


class Caret(Component):
    """Native component `sage.ui.Caret`."""

    pos: u32
    def __init__(self, pos: u32 = ...) -> None: ...


class Changed(Component):
    """Native component `sage.ui.Changed`."""

    target: u64
    value: f32
    def __init__(self, target: u64 = ..., value: f32 = ...) -> None: ...


class Clicked(Component):
    """Native component `sage.ui.Clicked`."""

    target: u64
    def __init__(self, target: u64 = ...) -> None: ...


class Flow(Component):
    """Native component `sage.ui.Flow`."""

    direction: u32
    align: u32
    justify: u32
    gap: f32
    pad_left: f32
    pad_top: f32
    pad_right: f32
    pad_bottom: f32
    def __init__(self, direction: u32 = ..., align: u32 = ..., justify: u32 = ..., gap: f32 = ..., pad_left: f32 = ..., pad_top: f32 = ..., pad_right: f32 = ..., pad_bottom: f32 = ...) -> None: ...


class Focused(Component):
    """Native component `sage.ui.Focused`."""

    target: u64
    def __init__(self, target: u64 = ...) -> None: ...


class Interaction(Component):
    """Native component `sage.ui.Interaction`."""

    hot: u64
    active: u64
    focus: u64
    prev_down: u32
    prev_nav: u32
    def __init__(self, hot: u64 = ..., active: u64 = ..., focus: u64 = ..., prev_down: u32 = ..., prev_nav: u32 = ...) -> None: ...


class Label(Component):
    """Native component `sage.ui.Label`."""

    text: u64
    def __init__(self, text: u64 = ...) -> None: ...


class Modal(Component):
    """Native component `sage.ui.Modal`."""

    on: u32
    def __init__(self, on: u32 = ...) -> None: ...


class Nav(Component):
    """Native component `sage.ui.Nav`."""

    bits: u32
    def __init__(self, bits: u32 = ...) -> None: ...


class Paint(Component):
    """Native component `sage.ui.Paint`."""

    fill: u32
    border: u32
    border_width: f32
    accent: u32
    def __init__(self, fill: u32 = ..., border: u32 = ..., border_width: f32 = ..., accent: u32 = ...) -> None: ...


class Pointer(Component):
    """Native component `sage.ui.Pointer`."""

    x: f32
    y: f32
    present: u32
    down: u32
    wheel: f32
    def __init__(self, x: f32 = ..., y: f32 = ..., present: u32 = ..., down: u32 = ..., wheel: f32 = ...) -> None: ...


class Rect(Component):
    """Native component `sage.ui.Rect`."""

    x: f32
    y: f32
    w: f32
    h: f32
    def __init__(self, x: f32 = ..., y: f32 = ..., w: f32 = ..., h: f32 = ...) -> None: ...


class Screen(Component):
    """Native component `sage.ui.Screen`."""

    w: f32
    h: f32
    def __init__(self, w: f32 = ..., h: f32 = ...) -> None: ...


class Scroll(Component):
    """Native component `sage.ui.Scroll`."""

    offset: f32
    def __init__(self, offset: f32 = ...) -> None: ...


class Slider(Component):
    """Native component `sage.ui.Slider`."""

    value: f32
    min: f32
    max: f32
    step: f32
    def __init__(self, value: f32 = ..., min: f32 = ..., max: f32 = ..., step: f32 = ...) -> None: ...


class Submitted(Component):
    """Native component `sage.ui.Submitted`."""

    target: u64
    def __init__(self, target: u64 = ...) -> None: ...


class TextChanged(Component):
    """Native component `sage.ui.TextChanged`."""

    target: u64
    def __init__(self, target: u64 = ...) -> None: ...


class TextEntry(Component):
    """Native component `sage.ui.TextEntry`."""

    ch: u32
    def __init__(self, ch: u32 = ...) -> None: ...


class TextStyle(Component):
    """Native component `sage.ui.TextStyle`."""

    color: u32
    scale: f32
    wrap: u32
    def __init__(self, color: u32 = ..., scale: f32 = ..., wrap: u32 = ...) -> None: ...


class Toggle(Component):
    """Native component `sage.ui.Toggle`."""

    on: u32
    def __init__(self, on: u32 = ...) -> None: ...


class Toggled(Component):
    """Native component `sage.ui.Toggled`."""

    target: u64
    on: u32
    def __init__(self, target: u64 = ..., on: u32 = ...) -> None: ...


class Widget(Component):
    """Native component `sage.ui.Widget`."""

    kind: u32
    enabled: u32
    tab: i32
    state: u32
    def __init__(self, kind: u32 = ..., enabled: u32 = ..., tab: i32 = ..., state: u32 = ...) -> None: ...


class ZIndex(Component):
    """Native component `sage.ui.ZIndex`."""

    z: i32
    def __init__(self, z: i32 = ...) -> None: ...
