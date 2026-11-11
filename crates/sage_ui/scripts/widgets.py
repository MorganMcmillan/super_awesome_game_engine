# Widget prefabs for the UI module (design doc 9).
#
# Load this script after `sage_ui::setup`; it adds a `ui` object to the world
# (`world.ui`). Widgets are prefabs, so a creator extends them the way they
# extend any prefab: `world.prefab("Danger", base=world.ui.Button)`, then set
# its components. Every widget is an entity; its parent is its `ChildOf`
# target and siblings flow in creation order.
#
#     ui = world.ui
#     ui.start()                                  # the `ui` flecs module
#     menu = ui.panel(width=120, padding=6, gap=4, fill=0x20242cff)
#     ui.text(menu, "Settings")
#     play = ui.button(menu, "Play", width=ui.fill())
#
#     @world.observer(Clicked)
#     def clicked(c: Clicked):
#         if c.target == play.sid: ...
#
# Style groups (Box, Flow, Paint, TextStyle) are inherited from the prefab until
# an instance sets its own, so editing a style prefab restyles its instances.
# Passing a style argument gives the instance its own copy of that whole group.
#
# Scroll areas and wrapping text: `ui.scroll(parent, height=80)` scrolls its
# children; `wrap=True` on text breaks lines to the width the layout gives it
# (a fixed or filling width, or a stretching parent).
#
# Sizes: None fits the content, a number is fixed, `ui.fill(weight)` takes a
# share of the space left in the parent. Colors are 0xRRGGBBAA.

import sage
from sage import OnUpdate
from sage.ui import Absolute, Box, Caret, Flow, Label, Paint, Rect, Scroll, Slider, TextStyle, Toggle, Widget, ZIndex

COLUMN, ROW = 0, 1
START, CENTER, END, STRETCH = 0, 1, 2, 3
SPACE_BETWEEN = 3
NONE, BUTTON, TOGGLE, SLIDER, TEXT_INPUT, BLOCK, METER = 0, 1, 2, 3, 4, 5, 6


class Fill:
    """A size that takes a share of what the parent has left."""

    def __init__(self, weight=1.0):
        self.weight = float(weight)


def _axis(size):
    if size is None:
        return 0, 0.0
    if isinstance(size, Fill):
        return 2, size.weight
    return 1, float(size)


def _padding(p):
    if isinstance(p, (int, float)):
        return float(p), float(p), float(p), float(p)
    if len(p) == 2:
        return float(p[0]), float(p[1]), float(p[0]), float(p[1])
    return tuple(float(v) for v in p)


def _update(entity, cls, **fields):
    """Sets fields on a component, adding it first if the entity lacks it."""
    value = entity.get(cls)
    # An inherited group comes back as a read-only view of the prefab's value.
    value = cls() if value is None else value.copy()
    for key, val in fields.items():
        setattr(value, key, val)
    entity.set(value)


class Ui:
    fill = staticmethod(Fill)
    # Layout constants, for `direction=`, `align=` and `justify=`.
    COLUMN, ROW = COLUMN, ROW
    START, CENTER, END, STRETCH, SPACE_BETWEEN = START, CENTER, END, STRETCH, SPACE_BETWEEN

    def __init__(self, world):
        self.world = world
        self.childof = world.lookup_path("flecs.core.ChildOf")
        panel = world.prefab("UiPanel")
        panel.set(Rect()).set(Box()).set(Flow()).set(Paint())
        label = world.prefab("UiLabel", base=panel)
        label.set(Label(0)).set(TextStyle(0xFFFFFFFF, 1.0, 0))
        button = world.prefab("UiButton", base=label)
        button.set(Widget(BUTTON, 1, 0, 0)).set(Paint(0x3A4150FF, 0x00000000, 0.0, 0x6AA9FFFF))
        button.set(Flow(COLUMN, START, START, 0.0, 4.0, 3.0, 4.0, 3.0))
        toggle = world.prefab("UiToggle", base=label)
        toggle.set(Widget(TOGGLE, 1, 0, 0)).set(Paint(0x3A4150FF, 0x00000000, 0.0, 0x6AA9FFFF))
        toggle.set(Toggle(0)).set(Box(1, 10.0, 1, 10.0))
        slider = world.prefab("UiSlider", base=panel)
        slider.set(Widget(SLIDER, 1, 0, 0)).set(Paint(0x2A2F3AFF, 0x00000000, 0.0, 0x6AA9FFFF))
        slider.set(Slider(0.0, 0.0, 1.0, 0.0)).set(Box(2, 1.0, 1, 8.0))
        text_input = world.prefab("UiTextInput", base=label)
        text_input.set(Widget(TEXT_INPUT, 1, 0, 0)).set(Paint(0x14171CFF, 0x3A4150FF, 1.0, 0x6AA9FFFF))
        text_input.set(Flow(COLUMN, START, START, 0.0, 3.0, 3.0, 3.0, 3.0)).set(Caret(0))
        meter = world.prefab("UiMeter", base=panel)
        meter.set(Widget(METER, 1, 0, 0)).set(Paint(0x2A2F3AFF, 0x00000000, 0.0, 0x6AA9FFFF))
        meter.set(Slider(0.0, 0.0, 1.0, 0.0)).set(Box(2, 1.0, 1, 6.0))
        # The prefabs. Extend them like any prefab: world.prefab("Danger", base=ui.Button).
        self.Panel, self.Text, self.Button = panel, label, button
        self.Toggle, self.Slider, self.TextInput, self.Meter = toggle, slider, text_input, meter

    def start(self, phase=OnUpdate):
        """Instantiates the `ui` flecs module: layout and interaction run in
        `phase`. Disabling the returned module pauses the UI."""
        return self.world.import_native("ui", phase)

    def add(self, prefab, parent=None, *, name=None, sid=None, width=None, height=None,
            direction=None, align=None, justify=None, gap=None, padding=None,
            fill=None, border=None, border_width=None, accent=None,
            text=None, color=None, scale=None, wrap=None, z=None, at=None, enabled=None, tab=None):
        """An instance of `prefab` under `parent` with the given style."""
        e = prefab.instantiate(name, sid)
        if parent is not None:
            e.add((self.childof, parent))
        if width is not None or height is not None:
            b = e.get(Box)
            w = _axis(width) if width is not None else (b.width_kind, b.width)
            h = _axis(height) if height is not None else (b.height_kind, b.height)
            e.set(Box(w[0], w[1], h[0], h[1]))
        flow = {}
        for key, val in (("direction", direction), ("align", align), ("justify", justify), ("gap", gap)):
            if val is not None:
                flow[key] = val
        if padding is not None:
            left, top, right, bottom = _padding(padding)
            flow.update(pad_left=left, pad_top=top, pad_right=right, pad_bottom=bottom)
        if flow:
            _update(e, Flow, **flow)
        paint = {k: v for k, v in (("fill", fill), ("border", border), ("border_width", border_width), ("accent", accent)) if v is not None}
        if paint:
            _update(e, Paint, **paint)
        if text is not None:
            e.set(Label(self.world.intern(text)))
        style = {k: v for k, v in (("color", color), ("scale", None if scale is None else float(scale)), ("wrap", None if wrap is None else int(wrap))) if v is not None}
        if style:
            _update(e, TextStyle, **style)
        if z is not None:
            e.set(ZIndex(z))
        if at is not None:
            e.set(Absolute(float(at[0]), float(at[1])))
        widget = {k: v for k, v in (("enabled", None if enabled is None else int(enabled)), ("tab", tab)) if v is not None}
        if widget:
            _update(e, Widget, **widget)
        return e

    def panel(self, parent=None, **style):
        return self.add(self.Panel, parent, **style)

    def text(self, parent, text, **style):
        return self.add(self.Text, parent, text=text, **style)

    def button(self, parent, text, **style):
        return self.add(self.Button, parent, text=text, **style)

    def toggle(self, parent, text=None, on=False, **style):
        """A check box. With `text` it sits in a row beside its label; the
        returned entity is the box either way."""
        row = None
        if text is not None:
            row = self.add(self.Panel, parent, direction=ROW, gap=4, align=CENTER)
            parent = row
        e = self.add(self.Toggle, parent, **style)
        e.set(Toggle(1 if on else 0))
        if text is not None:
            self.text(row, text)
        return e

    def slider(self, parent, value=0.0, low=0.0, high=1.0, step=0.0, **style):
        e = self.add(self.Slider, parent, **style)
        e.set(Slider(float(value), float(low), float(high), float(step)))
        return e

    def text_input(self, parent, text="", **style):
        return self.add(self.TextInput, parent, text=text, **style)

    def meter(self, parent, value=0.0, low=0.0, high=1.0, **style):
        """A read-only bar (a progress bar). Set its `Slider` value to move it."""
        e = self.add(self.Meter, parent, **style)
        e.set(Slider(float(value), float(low), float(high), 0.0))
        return e

    def scroll(self, parent=None, **style):
        """A panel whose content scrolls along its flow direction (the wheel,
        PageUp/PageDown, or tabbing to a hidden widget). Give it a fixed or
        filling size along that direction, or there is nothing to scroll."""
        e = self.add(self.Panel, parent, **style)
        e.set(Scroll(0.0))
        return e


world.ui = Ui(world)
