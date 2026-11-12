# Pickers built from the other widgets: a color picker (a saturation and value
# area, a hue slider, an alpha slider and a hex field), a date field with a
# calendar, a time field, and a file field with an in-engine browser. Each is
# a field that opens a popup, keeps its value in a component, and says so with
# an event. The calendar math is plain integer arithmetic (no `datetime`).

import sage
from sage import component
from sage.ui import (
    Absolute,
    Box,
    Changed,
    Clicked,
    Decor,
    Display,
    Drag,
    Dragged,
    FileAnswered,
    FileDialog,
    Label,
    Layer,
    Paint,
    Rect,
    Slider,
    TextChanged,
    Toggle,
    Toggled,
    Widget,
)

ui = world.ui
COLUMN, ROW = ui.COLUMN, ui.ROW
HANDLE = 8


def _text(e):
    return world.string(e.get(Label).text)


def _say(e, text):
    if _text(e) != text:
        e.set(Label(world.intern(text)))


def _fill(e, color):
    p = e.get(Paint).copy()
    p.fill = color
    e.set(p)


# ---- color -------------------------------------------------------------------------
def hsv_to_rgb(h, s, v):
    """Hue 0 to 1 (wrapping), saturation and value 0 to 1 -> red, green, blue
    0 to 1, with arithmetic only."""
    h = (h % 1.0) * 6.0
    i = int(h)
    f = h - i
    p, q, t = v * (1 - s), v * (1 - s * f), v * (1 - s * (1 - f))
    return [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i % 6]


def rgb_to_hsv(r, g, b):
    hi, lo = max(r, g, b), min(r, g, b)
    d = hi - lo
    if d == 0:
        h = 0.0
    elif hi == r:
        h = ((g - b) / d) % 6.0
    elif hi == g:
        h = (b - r) / d + 2.0
    else:
        h = (r - g) / d + 4.0
    return h / 6.0, (0.0 if hi == 0 else d / hi), hi


def pack(r, g, b, a=1.0):
    to = lambda x: max(0, min(255, int(x * 255.0 + 0.5)))
    return to(r) << 24 | to(g) << 16 | to(b) << 8 | to(a)


def unpack(color):
    return ((color >> 24 & 255) / 255.0, (color >> 16 & 255) / 255.0, (color >> 8 & 255) / 255.0, (color & 255) / 255.0)


def hex_of(color):
    return "%08X" % color if color & 255 != 255 else "%06X" % (color >> 8)


def parse_hex(text):
    """`RRGGBB` or `RRGGBBAA` (a leading `#` is fine) -> a color, or None."""
    t = text.strip().lstrip("#")
    if len(t) not in (6, 8) or any(c not in "0123456789abcdefABCDEF" for c in t):
        return None
    return int(t, 16) << 8 | 255 if len(t) == 6 else int(t, 16)


@component
class ColorPicker:
    """The value of a color field, as hue, saturation, value and alpha (the
    packed `color` is derived from them), and its parts."""

    h: sage.f32 = 0.0
    s: sage.f32 = 1.0
    v: sage.f32 = 1.0
    a: sage.f32 = 1.0
    color: sage.u32 = 0
    popup: sage.u64 = 0
    swatch: sage.u64 = 0
    readout: sage.u64 = 0
    area: sage.u64 = 0
    marker: sage.u64 = 0
    hue: sage.u64 = 0
    alpha: sage.u64 = 0
    hex: sage.u64 = 0
    base: sage.u64 = 0


@component
class AreaOf:
    """The grip over a color field's saturation and value area."""

    picker: sage.u64 = 0


@component
class ColorChanged:
    picker: sage.u64 = 0
    color: sage.u32 = 0


ui.ColorField = world.prefab("UiColorField", base=ui.Dropdown)
ui.Swatch = world.prefab("UiSwatch", base=ui.Panel)
ui.HueSlider = world.prefab("UiHueSlider", base=ui.Slider)
ui.AlphaSlider = world.prefab("UiAlphaSlider", base=ui.Slider)
ui.AreaGrip = world.prefab("UiAreaGrip", base=ui.Panel)
ui.AreaGrip.set(Widget(HANDLE, 1, 0, 0)).set(Drag())
AREA = (96.0, 64.0)


def color_field(parent, color=0xF28C28FF, **style):
    """A color picker: a swatch and its hex value; click to open the editor.
    Returns the field; read `ui.color_value(field)`. A `ColorChanged` event
    is emitted on it when the color changes."""
    field = ui.add(ui.ColorField, parent, **style)
    swatch = ui.add(ui.Swatch, field, width=14, height=8)
    readout = ui.add(ui.Text, field, text="", width=ui.fill())
    popup = ui.layer(field, autofocus=False)
    box = ui.add(ui.Panel, popup, direction=COLUMN, gap=3, padding=4)
    area = ui.add(ui.Panel, box, width=AREA[0], height=AREA[1], gradient=(0x000000FF, "across"), fill=0xFFFFFFFF)
    shade = ui.add(ui.Panel, area, at=(0.0, 0.0), width=AREA[0], height=AREA[1], gradient=(0x000000FF, "down"), fill=0x00000000)
    marker = ui.add(ui.Panel, area, at=(0.0, 0.0), width=5, height=5, fill=0xFFFFFFFF, border=0x000000FF, border_width=1)
    grip = ui.add(ui.AreaGrip, area, at=(0.0, 0.0), width=AREA[0], height=AREA[1])
    hue = ui.add(ui.HueSlider, box, width=ui.fill())
    hue.set(Slider(0.0, 0.0, 1.0, 0.0))
    alpha = ui.add(ui.AlphaSlider, box, width=ui.fill())
    alpha.set(Slider(1.0, 0.0, 1.0, 0.0))
    hexf = ui.text_input(box, "", placeholder="RRGGBB", max_len=8, width=ui.fill())
    ui.role(grip, "slider", name="SATURATION AND VALUE")
    ui.role(hue, "slider", name="HUE")
    ui.role(alpha, "slider", name="ALPHA")
    ui.role(hexf, "textbox", name="HEX COLOR")
    ui.role(popup, "dialog", name="COLOR")
    r, g, b, a = unpack(color)
    h, s, v = rgb_to_hsv(r, g, b)
    field.set(ColorPicker(h, s, v, a, color, popup.sid, swatch.sid, readout.sid, area.sid, marker.sid, hue.sid, alpha.sid, hexf.sid, shade.sid))
    grip.set(AreaOf(field.sid))
    _show(field, None)
    return field


def color_value(field):
    return field.get(ColorPicker).color


def color_set(field, color):
    r, g, b, a = unpack(color)
    h, s, v = rgb_to_hsv(r, g, b)
    p = field.get(ColorPicker).copy()
    p.h, p.s, p.v, p.a = h, s, v, a
    field.set(p)
    _show(field, None, emit=True)


def _show(field, source, emit=False):
    """Brings every part of the editor in line with the picker's h, s, v, a.
    `source` is the part being edited (left alone so it does not fight)."""
    p = field.get(ColorPicker).copy()
    r, g, b = hsv_to_rgb(p.h, p.s, p.v)
    color = pack(r, g, b, p.a)
    changed = color != p.color
    p.color = color
    field.set(p)
    _fill(world.lookup(p.swatch), color)
    _say(world.lookup(p.readout), hex_of(color))
    base = pack(*hsv_to_rgb(p.h, 1.0, 1.0), 1.0)
    area = world.lookup(p.area)
    d = area.get(Decor).copy()
    d.grad_to = base
    area.set(d)
    world.lookup(p.marker).set(Absolute(p.s * AREA[0] - 2.0, (1.0 - p.v) * AREA[1] - 2.0))
    for sid, value in ((p.hue, p.h), (p.alpha, p.a)):
        if source != sid:
            sl = world.lookup(sid)
            cur = sl.get(Slider).copy()
            if cur.value != value:
                cur.value = value
                sl.set(cur)
    if source != p.hex:
        hexf = world.lookup(p.hex)
        if _text(hexf) != hex_of(color):
            hexf.set(Label(world.intern(hex_of(color))))
    if changed and emit:
        field.emit(ColorChanged(field.sid, color))


ui.color_field, ui.color_value, ui.color_set = color_field, color_value, color_set
ui.ColorChanged = ColorChanged


@world.observer(Clicked)
def _ui_color_open(c: Clicked):
    e = world.lookup(c.target)
    if e is not None and e.has(ColorPicker):
        popup = world.lookup(e.get(ColorPicker).popup)
        if not ui.is_open(popup):
            ui.open(popup)


@world.observer(Dragged)
def _ui_color_area(d: Dragged):
    e = world.lookup(d.target)
    if e is None or not e.has(AreaOf):
        return
    field = world.lookup(e.get(AreaOf).picker)
    p = field.get(ColorPicker).copy()
    r = e.get(Rect)
    origin = e.get(Drag)
    x = origin.x + d.dx - r.x
    y = origin.y + d.dy - r.y
    p.s = max(0.0, min(1.0, x / AREA[0]))
    p.v = 1.0 - max(0.0, min(1.0, y / AREA[1]))
    field.set(p)
    _show(field, None, emit=True)


@world.observer(Changed)
def _ui_color_sliders(c: Changed):
    e = world.lookup(c.target)
    if e is None:
        return
    for field in _color_fields_of(e):
        p = field.get(ColorPicker).copy()
        if e.sid == p.hue:
            p.h = c.value
        elif e.sid == p.alpha:
            p.a = c.value
        else:
            continue
        field.set(p)
        _show(field, e.sid, emit=True)


def _color_fields_of(e):
    """The color field a part of whose popup `e` is, as a list."""
    at = e
    for _ in range(8):
        at = at.parent
        if at is None:
            return []
        if at.has(Layer):
            owner = world.lookup(at.get(Layer).anchor)
            return [owner] if owner is not None and owner.has(ColorPicker) else []
    return []


@world.observer(TextChanged)
def _ui_color_hex(t: TextChanged):
    e = world.lookup(t.target)
    if e is None:
        return
    for field in _color_fields_of(e):
        p = field.get(ColorPicker)
        if e.sid != p.hex:
            continue
        color = parse_hex(_text(e))
        if color is None:
            continue
        r, g, b, a = unpack(color)
        h, s, v = rgb_to_hsv(r, g, b)
        q = p.copy()
        q.h, q.s, q.v, q.a = (h if s > 0 and v > 0 else p.h), s, v, a
        field.set(q)
        _show(field, e.sid, emit=True)


# ---- date and time -------------------------------------------------------------------
MONTHS = ["JANUARY", "FEBRUARY", "MARCH", "APRIL", "MAY", "JUNE", "JULY", "AUGUST", "SEPTEMBER", "OCTOBER",
          "NOVEMBER", "DECEMBER"]
WEEKDAYS = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"]


def is_leap(y):
    return y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)


def days_in_month(y, m):
    return [31, 29 if is_leap(y) else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]


def weekday(y, m, d):
    """0 for Sunday to 6 for Saturday (Sakamoto's method)."""
    t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4]
    if m < 3:
        y -= 1
    return (y + y // 4 - y // 100 + y // 400 + t[m - 1] + d) % 7


@component
class DateField:
    """A date field's value and the month its calendar shows."""

    year: sage.i32 = 2000
    month: sage.u32 = 1
    day: sage.u32 = 1
    view_year: sage.i32 = 2000
    view_month: sage.u32 = 1
    popup: sage.u64 = 0
    title: sage.u64 = 0
    grid: sage.u64 = 0
    shown: sage.u64 = 0


@component
class DayCell:
    field: sage.u64 = 0
    index: sage.u32 = 0


@component
class MonthStep:
    field: sage.u64 = 0
    step: sage.i32 = 0


@component
class DatePicked:
    field: sage.u64 = 0
    year: sage.i32 = 0
    month: sage.u32 = 0
    day: sage.u32 = 0


ui.DateField = world.prefab("UiDateField", base=ui.Dropdown)
ui.DayCell = world.prefab("UiDayCell", base=ui.Choice)
ui.MonthButton = world.prefab("UiMonthButton", base=ui.Button)


def date_field(parent, year, month, day, **style):
    """A date field: click it for a calendar. Returns the field; read
    `ui.date_value(field)` -> `(year, month, day)`. A `DatePicked` event is
    emitted on it when a day is chosen."""
    field = ui.add(ui.DateField, parent, **style)
    shown = ui.add(ui.Text, field, text="", width=ui.fill())
    popup = ui.layer(field, autofocus=True)
    box = ui.add(ui.Panel, popup, direction=COLUMN, gap=3, padding=4)
    head = ui.add(ui.Panel, box, direction=ROW, gap=3, align=ui.CENTER, justify=ui.SPACE_BETWEEN, width=ui.fill())
    prev = ui.add(ui.MonthButton, head, text="-")
    title = ui.add(ui.Text, head, text="")
    nxt = ui.add(ui.MonthButton, head, text="+")
    grid = ui.grid(box, ["14"] * 7, "", (1, 1))
    for name in WEEKDAYS:
        ui.add(ui.Text, grid, text=name, width=14)
    for k in range(42):
        cell = ui.add(ui.DayCell, grid, text="", width=14)
        cell.set(Toggle(0))
        cell.set(DayCell(field.sid, k))
    field.set(DateField(year, month, day, year, month, popup.sid, title.sid, grid.sid, shown.sid))
    prev.set(MonthStep(field.sid, -1))
    nxt.set(MonthStep(field.sid, 1))
    ui.role(popup, "dialog", name="CALENDAR")
    ui.role(prev, "button", name="PREVIOUS MONTH")
    ui.role(nxt, "button", name="NEXT MONTH")
    ui.role(grid, "table", name="DAYS")
    _draw_calendar(field)
    return field


def date_value(field):
    d = field.get(DateField)
    return d.year, d.month, d.day


def date_set(field, year, month, day):
    d = field.get(DateField).copy()
    d.year, d.month, d.day = year, month, day
    d.view_year, d.view_month = year, month
    field.set(d)
    _draw_calendar(field)


def _draw_calendar(field):
    d = field.get(DateField)
    _say(world.lookup(d.shown), "%04d-%02d-%02d" % (d.year, d.month, d.day))
    _say(world.lookup(d.title), "%s %d" % (MONTHS[d.view_month - 1], d.view_year))
    first = weekday(d.view_year, d.view_month, 1)
    n = days_in_month(d.view_year, d.view_month)
    cells = world.lookup(d.grid).children[7:]
    for k, cell in enumerate(cells):
        day = k - first + 1
        in_month = 1 <= day <= n
        _say(cell, str(day) if in_month else "")
        on = 1 if in_month and (d.view_year, d.view_month, day) == (d.year, d.month, d.day) else 0
        if cell.get(Toggle).on != on:
            cell.set(Toggle(on))
        w = cell.get(Widget).copy()
        w.enabled = 1 if in_month else 0
        cell.set(w)


ui.date_field, ui.date_value, ui.date_set = date_field, date_value, date_set
ui.days_in_month, ui.weekday, ui.is_leap = days_in_month, weekday, is_leap
ui.DatePicked = DatePicked


@world.observer(Clicked)
def _ui_date_clicks(c: Clicked):
    e = world.lookup(c.target)
    if e is None:
        return
    if e.has(DateField):
        popup = world.lookup(e.get(DateField).popup)
        if not ui.is_open(popup):
            ui.open(popup)
    elif e.has(MonthStep):
        step = e.get(MonthStep)
        field = world.lookup(step.field)
        d = field.get(DateField).copy()
        month = d.view_month - 1 + step.step
        d.view_year += month // 12
        d.view_month = month % 12 + 1
        field.set(d)
        _draw_calendar(field)


@world.observer(Toggled)
def _ui_date_day(t: Toggled):
    e = world.lookup(t.target)
    if e is None or not e.has(DayCell):
        return
    cell = e.get(DayCell)
    field = world.lookup(cell.field)
    d = field.get(DateField).copy()
    first = weekday(d.view_year, d.view_month, 1)
    day = cell.index - first + 1
    if not 1 <= day <= days_in_month(d.view_year, d.view_month):
        return
    d.year, d.month, d.day = d.view_year, d.view_month, day
    field.set(d)
    _draw_calendar(field)
    ui.close(world.lookup(d.popup))
    field.emit(DatePicked(field.sid, d.year, d.month, d.day))


@component
class TimeField:
    hour: sage.u32 = 0
    minute: sage.u32 = 0
    hours: sage.u64 = 0
    minutes: sage.u64 = 0


@component
class TimePicked:
    field: sage.u64 = 0
    hour: sage.u32 = 0
    minute: sage.u32 = 0


def time_field(parent, hour=0, minute=0, **style):
    """Hours and minutes: two number fields, each clamped to its range.
    Returns the row; read `ui.time_value(field)` -> `(hour, minute)`."""
    row = ui.add(ui.Panel, parent, direction=ROW, gap=2, align=ui.CENTER, **style)
    hours = ui.number(row, hour, low=0, high=23, step=1, width=22)
    ui.add(ui.Text, row, text=":")
    minutes = ui.number(row, minute, low=0, high=59, step=1, width=22)
    row.set(TimeField(hour, minute, hours.sid, minutes.sid))
    ui.role(row, "group", name="TIME")
    ui.role(hours, "textbox", name="HOURS")
    ui.role(minutes, "textbox", name="MINUTES")
    return row


def time_value(field):
    t = field.get(TimeField)
    return t.hour, t.minute


ui.time_field, ui.time_value, ui.TimePicked = time_field, time_value, TimePicked


@world.observer(Changed)
def _ui_time_changed(c: Changed):
    e = world.lookup(c.target)
    if e is None or e.parent is None or not e.parent.has(TimeField):
        return
    row = e.parent
    t = row.get(TimeField).copy()
    if e.sid == t.hours:
        t.hour = int(c.value)
    elif e.sid == t.minutes:
        t.minute = int(c.value)
    else:
        return
    row.set(t)
    row.emit(TimePicked(row.sid, t.hour, t.minute))


# ---- files -----------------------------------------------------------------------------
@component
class FileField:
    """A file field: the chosen path, and its popup and tree."""

    path: sage.u64 = 0
    popup: sage.u64 = 0
    tree: sage.u64 = 0
    shown: sage.u64 = 0
    native: sage.u8 = 0
    awaiting: sage.u8 = 0


@component
class PathOf:
    path: sage.u64 = 0


@component
class FilePicked:
    field: sage.u64 = 0
    path: sage.u64 = 0


ui.FileField = world.prefab("UiFileField", base=ui.Dropdown)


def file_tree(paths):
    """A list of paths (`"art/logo.png"`) as the nested entries `ui.tree`
    takes: folders first, each sorted by name."""
    root = {}
    for p in paths:
        node = root
        parts = [x for x in p.split("/") if x]
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        node[parts[-1]] = None

    def entries(node):
        names = sorted(node, key=lambda n: (node[n] is None, n))
        return [(n, entries(node[n])) if node[n] is not None else n for n in names]

    return entries(root)


def file_field(parent, paths=None, path="", native=False, save=False, **style):
    """A file field. With `paths` (`["art/logo.png", "sounds/hit.wav"]`, for
    example the asset catalog) a click opens an in-engine browser, a tree of
    them. With `native=True` it asks the host for the operating system's open
    (or, with `save`, save) dialog instead, which the window shows; the
    browser is for places with no such dialog. Returns the field; read
    `ui.file_value(field)`. A `FilePicked` event is emitted on it."""
    field = ui.add(ui.FileField, parent, **style)
    shown = ui.add(ui.Text, field, text=path or "CHOOSE A FILE", width=ui.fill())
    if native:
        field.set(FileField(world.intern(path), 0, 0, shown.sid, 2 if save else 1, 0))
        return field
    popup = ui.layer(field, match_width=False, autofocus=True)
    box = ui.add(ui.Panel, popup, direction=COLUMN, padding=4, width=140, height=90)
    area = ui.scroll(box, width=ui.fill(), height=ui.fill())
    tree = ui.tree(area, file_tree(paths), width=ui.fill())
    # Each row knows its whole path.
    stack = []
    for row in ui.tree_rows(tree):
        info = row.get(ui.TreeRow)
        del stack[info.depth:]
        stack.append(_text(row))
        row.set(PathOf(world.intern("/".join(stack))))
    field.set(FileField(world.intern(path), popup.sid, tree.sid, shown.sid, 0, 0))
    ui.role(popup, "dialog", name="FILES")
    return field


def file_value(field):
    return world.string(field.get(FileField).path)


ui.file_tree, ui.file_field, ui.file_value, ui.FilePicked = file_tree, file_field, file_value, FilePicked


@world.observer(Clicked)
def _ui_file_open(c: Clicked):
    e = world.lookup(c.target)
    if e is None or not e.has(FileField):
        return
    f = e.get(FileField)
    if f.native:
        # Ask the host for its dialog; the answer comes back through the
        # singleton and `_ui_file_answer`.
        if not f.awaiting:
            world.set(FileDialog(f.native, e.sid, 0, 0))
            nf = f.copy()
            nf.awaiting = 1
            e.set(nf)
    else:
        popup = world.lookup(f.popup)
        if not ui.is_open(popup):
            ui.open(popup)


@world.observer(FileAnswered)
def _ui_file_answer(a: FileAnswered):
    e = world.lookup(a.target)
    if e is None or not e.has(FileField):
        return
    f = e.get(FileField).copy()
    f.awaiting = 0
    if a.done == 1:
        f.path = a.path
        _say(world.lookup(f.shown), world.string(a.path))
    e.set(f)
    if a.done == 1:
        e.emit(FilePicked(e.sid, a.path))


@world.observer(ui.TreeSelected)
def _ui_file_picked(s: ui.TreeSelected):
    tree = world.lookup(s.tree)
    row = world.lookup(s.row)
    if tree is None or row is None or not row.has(PathOf) or row.get(ui.TreeRow).branch:
        return
    at = tree
    for _ in range(8):
        at = at.parent
        if at is None:
            return
        if at.has(Layer):
            break
    field = world.lookup(at.get(Layer).anchor)
    if field is None or not field.has(FileField):
        return
    path = row.get(PathOf).path
    f = field.get(FileField).copy()
    f.path = path
    field.set(f)
    _say(world.lookup(f.shown), world.string(path))
    ui.close(at)
    field.emit(FilePicked(field.sid, path))


# The state components, for scripts that read them (`ui.DateField` is the
# field's prefab, so its component is `ui.DateState`).
ui.ColorState, ui.DateState, ui.TimeState, ui.FileState = ColorPicker, DateField, TimeField, FileField
ui.hsv_to_rgb, ui.rgb_to_hsv, ui.parse_hex, ui.hex_of = hsv_to_rgb, rgb_to_hsv, parse_hex, hex_of
ui.pack, ui.unpack = pack, unpack
