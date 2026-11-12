# Drag and drop, resizing, keyboard shortcuts and a reorderable list, built on
# the interaction layer's `Draggable`, `DropZone`, `Shortcut` and `Handle`.
# All state is in components.

import sage
from sage import component
from sage.ui import Box, Draggable, DropZone, Dragged, Dropped, Label, Order, Rect, Shortcut, Widget, Drag

ui = world.ui
COLUMN, ROW = ui.COLUMN, ui.ROW
HANDLE = 8


def draggable(e, payload=0, kind=0):
    """Lets `e` be picked up (after a few units of movement; a press that does
    not move is still a click). `payload` is any number the drop reports;
    `kind` (0 to 31) decides which drop zones take it."""
    e.set(Draggable(int(payload), int(kind)))
    return e


def drop_zone(e, kinds=(0,)):
    """Makes `e` take drops of these kinds (a kind, a list of kinds, or a bit
    set as `("bits", n)`). `DropZone.over` is 1 while a drag it accepts is
    above it."""
    if isinstance(kinds, int):
        kinds = (kinds,)
    bits = 0
    for k in kinds:
        bits |= 1 << int(k)
    e.set(DropZone(bits, 0))
    return e


_KEYS = {"CTRL": 1, "CONTROL": 1, "SHIFT": 2, "ALT": 4}


def parse_chord(text):
    """`"Ctrl+Shift+S"` -> `(key, mods)`: the key is an upper-case ASCII letter
    or digit, or `0x100 + n - 1` for `F<n>`."""
    mods, key = 0, 0
    for part in text.replace(" ", "").upper().split("+"):
        if part in _KEYS:
            mods |= _KEYS[part]
        elif len(part) == 1 and part.isalnum():
            key = ord(part)
        elif part.startswith("F") and part[1:].isdigit() and 1 <= int(part[1:]) <= 12:
            key = 0x100 + int(part[1:]) - 1
        else:
            raise ValueError("not a key chord: " + repr(text))
    if not key:
        raise ValueError("a chord needs a key: " + repr(text))
    return key, mods


def shortcut(e, chord):
    """Makes the key chord (`"Ctrl+S"`, `"F5"`, `"Alt+1"`) click `e`: a button
    presses, a toggle flips. A disabled widget ignores it."""
    key, mods = parse_chord(chord)
    e.set(Shortcut(key, mods))
    return e


ui.draggable, ui.drop_zone, ui.shortcut, ui.parse_chord = draggable, drop_zone, shortcut, parse_chord


# ---- resizing ----------------------------------------------------------------------
@component
class ResizeOf:
    """A grip that resizes `target`: its size at the press, and its limits."""

    target: sage.u64 = 0
    start_w: sage.f32 = 0.0
    start_h: sage.f32 = 0.0
    min_w: sage.f32 = 0.0
    min_h: sage.f32 = 0.0
    axes: sage.u8 = 3  # 1 width, 2 height


ui.ResizeGrip = world.prefab("UiResizeGrip", base=ui.Panel)
ui.ResizeGrip.set(Widget(HANDLE, 1, 0, 0)).set(Drag())


def resizable(parent, width, height, min_w=40.0, min_h=24.0, axes=3, prefab=None, **style):
    """A panel its user can resize by dragging a grip in the corner. Returns
    `(root, content)`: add children to `content`. `axes` is 1 for width only,
    2 for height only, 3 for both. `prefab` styles the panel."""
    root = ui.add(prefab or ui.Panel, parent, direction=COLUMN, gap=0, width=width, height=height, **style)
    content = ui.add(ui.Panel, root, direction=COLUMN, gap=4, width=ui.fill(), height=ui.fill())
    row = ui.add(ui.Panel, root, direction=ROW, justify=ui.END, width=ui.fill())
    grip = ui.add(ui.ResizeGrip, row)
    grip.set(Box(1, 8.0, 1, 8.0))
    grip.set(ResizeOf(root.sid, float(width), float(height), float(min_w), float(min_h), axes))
    return root, content


def size_of(root):
    r = root.get(Rect)
    return r.w, r.h


ui.resizable, ui.size_of = resizable, size_of


@world.observer(Dragged)
def _ui_resize_drag(d: Dragged):
    grip = world.lookup(d.target)
    if grip is None or not grip.has(ResizeOf):
        return
    info = grip.get(ResizeOf).copy()
    root = world.lookup(info.target)
    if d.phase == 0:
        r = root.get(Rect)
        info.start_w, info.start_h = r.w, r.h
        grip.set(info)
        return
    b = root.get(Box).copy()
    if info.axes & 1:
        b.width_kind, b.width = 1, max(info.min_w, info.start_w + d.dx)
    if info.axes & 2:
        b.height_kind, b.height = 1, max(info.min_h, info.start_h + d.dy)
    root.set(b)


# ---- a reorderable list --------------------------------------------------------------
@component
class Sortable:
    count: sage.u32 = 0


@component
class SortRow:
    list: sage.u64 = 0
    index: sage.u32 = 0


@component
class Reordered:
    """Emitted on the list when a row moved: where from and to."""

    list: sage.u64 = 0
    source: sage.u32 = 0
    to: sage.u32 = 0


ui.SortRow = world.prefab("UiSortRow", base=ui.Panel)


def sortable(parent, items, **style):
    """A list whose rows are dragged to reorder. Returns the root; read
    `ui.sortable_items(root)`. A `Reordered` event is emitted on it."""
    root = ui.add(ui.Panel, parent, direction=COLUMN, gap=2, **style)
    for i, text in enumerate(items):
        row = ui.add(ui.SortRow, root, direction=ROW, gap=4, width=ui.fill(), padding=3)
        row.set(SortRow(root.sid, i))
        draggable(row, i, 31)
        drop_zone(row, (31,))
        ui.add(ui.Text, row, text=text)
    root.set(Sortable(len(items)))
    return root


def _rows(root):
    return sorted((c for c in root.children if c.has(SortRow)), key=lambda c: c.get(SortRow).index)


def _label(row):
    return world.string(row.children[0].get(Label).text)


def sortable_items(root):
    return [_label(r) for r in _rows(root)]


def _set_items(root, items):
    for row, text in zip(_rows(root), items):
        row.children[0].set(Label(world.intern(text)))


ui.sortable, ui.sortable_items = sortable, sortable_items
ui.Reordered = Reordered


@world.observer(Dropped)
def _ui_sort_dropped(d: Dropped):
    target = world.lookup(d.target)
    if target is None or not target.has(SortRow):
        return
    to = target.get(SortRow)
    root = world.lookup(to.list)
    source = world.lookup(d.source)
    if source is None or not source.has(SortRow) or source.get(SortRow).list != to.list:
        return
    src = source.get(SortRow).index
    if src == to.index:
        return
    items = sortable_items(root)
    items.insert(to.index, items.pop(src))
    _set_items(root, items)
    root.emit(Reordered(root.sid, src, to.index))


# ---- reordering by drag, for any container ---------------------------------------------------
@component
class ReorderItem:
    """A child that can be dragged to a new place among its siblings."""

    container: sage.u64 = 0


@component
class Moved:
    """Emitted on a container when a child moved: from and to (positions)."""

    container: sage.u64 = 0
    source: sage.u32 = 0
    to: sage.u32 = 0


REORDER_KIND = 30


def in_order(container):
    """The children of `container` in the order they are shown (their `Order`
    keys, ties in creation order)."""
    kids = list(container.children)
    keyed = [((k.get(Order).key if k.has(Order) else 0), i, k) for i, k in enumerate(kids)]
    return [k for _, _, k in sorted(keyed, key=lambda t: (t[0], t[1]))]


def reorderable(container, kind=REORDER_KIND):
    """Lets the user drag the children of `container` to new places among
    their siblings (tabs, tree rows, list items). Moving only changes their
    `Order` keys, so nothing is recreated and all state stays where it is. A
    `Moved` event is emitted on the container."""
    for child in container.children:
        child.set(ReorderItem(container.sid))
        draggable(child, child.sid, kind)
        drop_zone(child, (kind,))
    return container


def move_child(container, source, target):
    """Moves `source` to where `target` is among its siblings (both are
    children of `container`). Returns True if anything moved."""
    shown = in_order(container)
    ids = [k.sid for k in shown]
    if source.sid not in ids or target.sid not in ids or source.sid == target.sid:
        return False
    a, b = ids.index(source.sid), ids.index(target.sid)
    shown.insert(b, shown.pop(a))
    for key, k in enumerate(shown):
        k.set(Order(key))
    container.emit(Moved(container.sid, a, b))
    return True


ui.reorderable, ui.in_order, ui.move_child, ui.Moved = reorderable, in_order, move_child, Moved


@world.observer(Dropped)
def _ui_reorder_dropped(d: Dropped):
    target = world.lookup(d.target)
    source = world.lookup(d.source)
    if target is None or source is None or not target.has(ReorderItem) or not source.has(ReorderItem):
        return
    container = world.lookup(target.get(ReorderItem).container)
    if source.get(ReorderItem).container != container.sid:
        return
    move_child(container, source, target)
