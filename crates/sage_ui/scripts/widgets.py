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
from sage.ui import (
    Absolute,
    Access,
    Order,
    ScrollCross,
    Sticky,
    Box,
    Caret,
    Decor,
    Display,
    Flow,
    History,
    Label,
    Paint,
    Space,
    WrapFlow,
    Rect,
    Scroll,
    Slider,
    States,
    Cell,
    Rich,
    TextField,
    TextStyle,
    Toggle,
    Transition,
    Widget,
    ZIndex,
)
from sage.ui import Grid as GridC
from sage.ui import Image as ImageC
from sage.ui import Slice as SliceC

# Roles, by the number `Access.role` holds (the order of `access::Role`; a
# test keeps this list in step with it).
ROLE_NAMES = [
    "generic", "button", "checkbox", "radio", "switch", "slider", "textbox", "link", "heading", "list",
    "listitem", "table", "row", "cell", "columnheader", "tab", "tablist", "tabpanel", "tree", "treeitem",
    "menu", "menubar", "menuitem", "dialog", "tooltip", "progressbar", "separator", "navigation", "banner",
    "contentinfo", "main", "region", "form", "group", "img", "text", "combobox", "listbox", "option",
    "radiogroup", "label", "application",
]
ROLES = {name: k for k, name in enumerate(ROLE_NAMES)}


def role(e, role, name=None, description=None, level=None, hidden=None, labelled_by=None, label_for=None):
    """Says what `e` is to a screen reader. `role` is a name (`"button"`,
    `"navigation"`, `"heading"`...) or a number; widgets already imply one,
    so this is for what they cannot know: a landmark, a heading level, a
    name that is not the text shown, or `labelled_by=` / `label_for=`
    (entities). Only the arguments given change; the rest stay."""
    a = e.get(Access).copy() if e.has(Access) else Access()
    if role is not None:
        a.role = ROLES[role] if isinstance(role, str) else int(role)
    if name is not None:
        a.name = world.intern(name) if name else 0
    if description is not None:
        a.description = world.intern(description) if description else 0
    if level is not None:
        a.level = int(level)
    if hidden is not None:
        a.hidden = 1 if hidden else 0
    if labelled_by is not None:
        a.labelled_by = labelled_by.sid if labelled_by else 0
    if label_for is not None:
        a.label_for = label_for.sid if label_for else 0
    e.set(a)
    return e


COLUMN, ROW = 0, 1
START, CENTER, END, STRETCH = 0, 1, 2, 3
SPACE_BETWEEN = 3
NONE, BUTTON, TOGGLE, SLIDER, TEXT_INPUT, BLOCK, METER, CHOICE = 0, 1, 2, 3, 4, 5, 6, 7


def asset(path):
    """The `AssetId` of an image path: the 64-bit FNV-1a hash of the path with
    `\\` turned into `/` and a leading `./` removed, as `sage_assets` computes
    it. The host must also register the image (its pixels and its size)."""
    p = path.replace("\\", "/")
    while p.startswith("./"):
        p = p[2:]
    h = 0xCBF29CE484222325
    for byte in p.encode("utf-8"):
        h = ((h ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


FIT = {"stretch": 0, "contain": 1, "cover": 2, "none": 3, "scale-down": 4}
STATES = ("checked", "focus", "hover", "pressed", "disabled")


class Pct:
    """A size that is a percentage (0 to 100) of the parent's content box."""

    def __init__(self, percent):
        self.percent = float(percent)


ALIGN_SELF = {"start": 1, "center": 2, "end": 3, "stretch": 4}


class Fill:
    """A size that takes a share of what the parent has left."""

    def __init__(self, weight=1.0):
        self.weight = float(weight)


def _axis(size):
    if size is None:
        return 0, 0.0
    if isinstance(size, Fill):
        return 2, size.weight
    if isinstance(size, Pct):
        return 3, size.percent
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
    pct = staticmethod(Pct)
    asset = staticmethod(asset)
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
        choice = world.prefab("UiChoice", base=label)
        choice.set(Widget(CHOICE, 1, 0, 0)).set(Toggle(0)).set(Paint(0x3A4150FF, 0x00000000, 0.0, 0x6AA9FFFF))
        choice.set(Flow(COLUMN, START, START, 0.0, 4.0, 3.0, 4.0, 3.0))
        text_input = world.prefab("UiTextInput", base=label)
        text_input.set(Widget(TEXT_INPUT, 1, 0, 0)).set(Paint(0x14171CFF, 0x3A4150FF, 1.0, 0x6AA9FFFF))
        text_input.set(Flow(COLUMN, START, START, 0.0, 3.0, 3.0, 3.0, 3.0)).set(Caret(0)).set(TextField()).set(History())
        meter = world.prefab("UiMeter", base=panel)
        meter.set(Widget(METER, 1, 0, 0)).set(Paint(0x2A2F3AFF, 0x00000000, 0.0, 0x6AA9FFFF))
        meter.set(Slider(0.0, 0.0, 1.0, 0.0)).set(Box(2, 1.0, 1, 6.0))
        # The prefabs. Extend them like any prefab: world.prefab("Danger", base=ui.Button).
        image = world.prefab("UiImage", base=panel)
        image.set(ImageC(0, 0, 0xFFFFFFFF, 0, 1, 1))
        # Headings: three sizes of text, and paragraphs of rich text.
        for level, scale in ((1, 3.0), (2, 2.0), (3, 1.0)):
            h = world.prefab("UiH%d" % level, base=label)
            h.set(TextStyle(0xFFFFFFFF, scale, 0))
            setattr(self, "H%d" % level, h)
        rich = world.prefab("UiRich", base=label)
        rich.set(TextStyle(0xFFFFFFFF, 1.0, 1)).set(Box(2, 1.0, 0, 0.0)).set(Paint(0x00000000, 0x00000000, 0.0, 0x6AA9FFFF))
        self.RichText = rich
        self.Panel, self.Text, self.Button, self.Image, self.Choice = panel, label, button, image, choice
        self.Toggle, self.Slider, self.TextInput, self.Meter = toggle, slider, text_input, meter

    def start(self, phase=OnUpdate):
        """Instantiates the `ui` flecs module: layout and interaction run in
        `phase`. Disabling the returned module pauses the UI."""
        return self.world.import_native("ui", phase)

    def add(self, prefab, parent=None, *, name=None, sid=None, width=None, height=None,
            direction=None, align=None, justify=None, gap=None, padding=None,
            fill=None, border=None, border_width=None, accent=None,
            text=None, color=None, scale=None, wrap=None, z=None, at=None, enabled=None, tab=None,
            image=None, fit=None, tint=None, frame=None, cols=None, rows=None, slice=None,
            radius=None, shadow=None, gradient=None, fade=None, display=None, transition=None,
            states=None, margin=None, min_width=None, max_width=None, min_height=None, max_height=None,
            aspect=None, align_self=None, offset=None, grid=None, cell=None, wrap_flow=None,
            order=None, sticky=None):
        """An instance of `prefab` under `parent` with the given style.

        Spacing: `margin` (a number, `(x, y)` or `(l, t, r, b)`), `min_*`/`max_*`
        sizes, `aspect` (width / height), `align_self` ("start", "center",
        "end", "stretch") and `offset=(dx, dy)` (moves it after layout).
        Grids: `grid=("100 1fr auto", "auto", (col_gap, row_gap))` makes a
        grid; `cell=(col, row, col_span, row_span)` places a child (1-based,
        0 for the next free slot). `wrap_flow=gap` wraps children onto new
        lines."""
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
        space = {}
        if margin is not None:
            left, top, right, bottom = _padding(margin)
            space.update(margin_left=left, margin_top=top, margin_right=right, margin_bottom=bottom)
        for key, val in (("min_width", min_width), ("max_width", max_width), ("min_height", min_height),
                         ("max_height", max_height), ("aspect", aspect)):
            if val is not None:
                space[key] = float(val)
        if align_self is not None:
            space["align_self"] = ALIGN_SELF[align_self] if isinstance(align_self, str) else align_self
        if offset is not None:
            space.update(offset_x=float(offset[0]), offset_y=float(offset[1]))
        if space:
            _update(e, Space, **space)
        if grid is not None:
            tracks, rows_, *gaps = grid
            cg, rg = (gaps[0] if isinstance(gaps[0], (tuple, list)) else (gaps[0], gaps[0])) if gaps else (0, 0)
            e.set(GridC(self.world.intern(tracks), self.world.intern(rows_ or ""), float(cg), float(rg)))
        if cell is not None:
            col_, row_, *spans = cell
            e.set(Cell(col_, row_, spans[0] if spans else 1, spans[1] if len(spans) > 1 else 1))
        if wrap_flow is not None:
            e.set(WrapFlow(1, float(wrap_flow)))
        if order is not None:
            e.set(Order(int(order)))
        if sticky is not None:
            e.set(Sticky(float(sticky), 1))
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
        if image is not None or fit is not None or tint is not None or frame is not None:
            img = e.get(ImageC)
            img = ImageC(0, 0, 0xFFFFFFFF, 0, 1, 1) if img is None else img.copy()
            if image is not None:
                img.asset = asset(image) if isinstance(image, str) else image
            if fit is not None:
                img.fit = FIT[fit] if isinstance(fit, str) else fit
            if tint is not None:
                img.tint = tint
            if frame is not None:
                img.frame = frame
            if cols is not None:
                img.cols = cols
            if rows is not None:
                img.rows = rows
            e.set(img)
        if slice is not None:
            sl_asset, (left, top, right, bottom) = slice
            sl_asset = asset(sl_asset) if isinstance(sl_asset, str) else sl_asset
            e.set(SliceC(sl_asset, float(left), float(top), float(right), float(bottom), 0xFFFFFFFF))
        decor = {}
        if radius is not None:
            decor["radius"] = float(radius)
        if shadow is not None:
            blur, dy, color_ = shadow
            decor.update(shadow_blur=float(blur), shadow_dy=float(dy), shadow_color=color_)
        if gradient is not None:
            to, direction_ = gradient
            decor.update(grad_to=to, grad_dir={"down": 1, "across": 2}[direction_])
        if fade is not None:
            decor["fade"] = float(fade)
        if decor:
            _update(e, Decor, **decor)
        if display is not None:
            e.set(Display(0 if display else 1))
        if transition is not None:
            e.set(Transition(int(transition)))
        if states is not None:
            fields = {}
            for state, colors in states.items():
                if state not in STATES:
                    raise ValueError(f"unknown state {state!r}; use one of {STATES}")
                for part, value in colors.items():
                    if part not in ("fill", "text", "border"):
                        raise ValueError(f"unknown state color {part!r}; use fill, text or border")
                    fields[f"{state}_{part}"] = value
            _update(e, States, **fields)
        return e

    role = staticmethod(role)
    ROLE_NAMES = ROLE_NAMES

    def panel(self, parent=None, **style):
        return self.add(self.Panel, parent, **style)

    def text(self, parent, text, **style):
        return self.add(self.Text, parent, text=text, **style)

    def heading(self, parent, text, level=1, **style):
        """`<h1>` to `<h3>`: the same text at three sizes (3, 2 and 1 times
        the glyph size; the face is a bitmap)."""
        e = self.add(getattr(self, "H%d" % max(1, min(3, level))), parent, text=text, **style)
        return role(e, "heading", level=level)

    def rich(self, parent, markup, **style):
        """A paragraph of rich text that wraps to its width. The markup:
        `**bold**`, `` `code` ``, `{#RRGGBB colored}` and `[label](target)`
        for a link (a click emits `Navigate`). A backslash writes a special
        character literally. `prefab=` picks the style prefab."""
        e = self.add(style.pop("prefab", None) or self.RichText, parent, text="", **style)
        e.set(Rich(world.intern(markup)))
        return e

    def link(self, parent, label, target, **style):
        """A single link: rich text that is all one link."""
        return role(self.rich(parent, "[%s](%s)" % (label, target), **style), "link")

    def button(self, parent, text, **style):
        return self.add(self.Button, parent, text=text, **style)

    def choice(self, parent, text, on=False, **style):
        """A button that stays selected (a toggle button, no check mark). Its
        look when selected is the `checked` entry of `states=`."""
        e = self.add(self.Choice, parent, text=text, **style)
        e.set(Toggle(1 if on else 0))
        return e

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
            role(self.text(row, text), "label", label_for=e)
        return e

    def slider(self, parent, value=0.0, low=0.0, high=1.0, step=0.0, **style):
        e = self.add(self.Slider, parent, **style)
        e.set(Slider(float(value), float(low), float(high), float(step)))
        return e

    def _field(self, e, flags, placeholder, max_len):
        e.set(TextField(flags=flags, max_len=max_len, placeholder=world.intern(placeholder) if placeholder else 0))
        return e

    def text_input(self, parent, text="", placeholder="", max_len=0, password=False, **style):
        """A one-line text field. `placeholder` shows while it is empty;
        `password` shows asterisks and refuses copy and cut. `prefab=` picks
        the style prefab (any instance of the text input). Selection with
        Shift or the mouse, Ctrl+arrows by word, Ctrl+A/C/X/V/Z/Y."""
        e = self.add(style.pop("prefab", None) or self.TextInput, parent, text=text, **style)
        return self._field(e, 2 if password else 0, placeholder, max_len)

    def text_area(self, parent, text="", placeholder="", rows=4, max_len=0, **style):
        """Several lines: Enter adds a line, long lines wrap, Up/Down move by
        row, the wheel and PageUp/PageDown scroll. `rows` sets its height."""
        style.setdefault("height", rows * 9 + 6)
        e = self.add(style.pop("prefab", None) or self.TextInput, parent, text=text, **style)
        return self._field(e, 1, placeholder, max_len or 4096)

    def number(self, parent, value=0.0, low=0.0, high=0.0, step=0.0, **style):
        """A number field: type a value (it settles on Enter or when focus
        leaves), Up/Down or the wheel step it, and it clamps to the range
        (when `high > low`) and snaps to `step`. The value is its `Slider`;
        `Changed` reports it."""
        text = ("%g" % value) if value == int(value) else ("%.6g" % value)
        e = self.add(style.pop("prefab", None) or self.TextInput, parent, text=text, **style)
        e.set(Slider(float(value), float(low), float(high), float(step)))
        return self._field(e, 4, "", 24)

    def image(self, parent, source, alt=None, **style):
        """An `<img>`: `source` is a path or an `ui.asset(path)` id. With no
        size it is as big as the image; `fit=` places it in a bigger box.
        `alt` is the text a screen reader says for it ("" marks it as
        decoration and leaves it out)."""
        e = self.add(self.Image, parent, image=source, **style)
        if alt is not None:
            role(e, "img", name=alt, hidden=(alt == ""))
        return e

    def meter(self, parent, value=0.0, low=0.0, high=1.0, **style):
        """A read-only bar (a progress bar). Set its `Slider` value to move it."""
        e = self.add(self.Meter, parent, **style)
        e.set(Slider(float(value), float(low), float(high), 0.0))
        return e

    def grid(self, parent, cols, rows="", gap=0, **style):
        """A grid container. `cols` and `rows` are track lists: `"100 1fr auto"`
        or a list like `[100, "1fr", "auto"]` (a number is a fixed size, `Nfr`
        takes a share of what is left, `auto` fits the content; rows beyond
        the list are auto). `gap` is a number or `(column gap, row gap)`.
        Children fill the cells row by row; give one `cell=(col, row,
        col_span, row_span)` to place it."""
        join = lambda t: t if isinstance(t, str) else " ".join(str(v) for v in t)
        return self.add(self.Panel, parent, grid=(join(cols), join(rows), gap), **style)

    def list(self, parent, items, ordered=False, start=1, **style):
        """`<ul>` and `<ol>`: a column of rows, each a marker (a bullet, or
        the number) and the item's text. `items` are strings (or `(markup,)`
        for rich text). Returns the list of row entities."""
        col = role(self.add(self.Panel, parent, direction=COLUMN, gap=2, **style), "list")
        rows_ = []
        for k, item in enumerate(items):
            row = role(self.add(self.Panel, col, direction=ROW, gap=4, align=START, width=self.fill()), "listitem")
            mark = ("%d." % (start + k)) if ordered else "-"
            role(self.add(self.Text, row, text=mark, width=12 if ordered else 6), "text", hidden=True)
            self.rich(row, item, width=self.fill()) if isinstance(item, tuple) else self.add(self.Text, row, text=item, wrap=True, width=self.fill())
            rows_.append(row)
        return rows_

    def scroll(self, parent=None, both=False, **style):
        """A panel whose content scrolls along its flow direction (the wheel,
        PageUp/PageDown, or tabbing to a hidden widget). Give it a fixed or
        filling size along that direction, or there is nothing to scroll.
        `both=True` also scrolls across the flow (Shift and the wheel); give
        something inside it a width (or height) larger than the view. A child
        made with `sticky=inset` holds that far from the top once scrolled
        past."""
        e = self.add(self.Panel, parent, **style)
        e.set(Scroll(0.0))
        if both:
            e.set(ScrollCross(1, 0.0))
        return e


world.ui = Ui(world)
