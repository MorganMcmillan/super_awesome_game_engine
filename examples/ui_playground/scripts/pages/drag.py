# Page: DRAG. Drag and drop (a reorderable list, chips dropped into a bin that
# takes only their kind), a panel resized by its grip, key chords, and the
# middle and right buttons.

import sage
from sage import Entity, component
from sage.ui import (
    Clicked,
    ContextRequested,
    Display,
    DragEnded,
    DragStarted,
    DropZone,
    Dropped,
    Label,
    MiddleClicked,
    Rect,
    Toggle,
    Toggled,
)

ui = world.ui
T = world.theme
drag_page = world.add_page("DRAG")


def dcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def dsay(entity, text):
    entity.set(Label(world.intern(text)))


@component
class Tally:
    """A count kept in the world (not in a Python variable)."""

    n: sage.u32 = 0


@component
class BinContents:
    """Which gems the bin holds, one bit per payload."""

    bits: sage.u32 = 0


dcol_a, dcol_b, dcol_c = (ui.add(T.Column, drag_page) for _ in range(3))

# -- a reorderable list ---------------------------------------------------------------
sortcard = dcard(dcol_a, "REORDER")
ui.add(T.Muted, sortcard, text="DRAG A ROW ONTO ANOTHER.", wrap=True, width=ui.fill())
sort_list = ui.sortable(sortcard, ["ALPHA", "BRAVO", "CHARLIE", "DELTA"], width=ui.fill())
sort_said = ui.add(T.Body, sortcard, text="MOVED: NOTHING")


@world.observer(ui.Reordered)
def drag_reordered(r: ui.Reordered):
    dsay(sort_said, "MOVED: %d TO %d" % (r.source + 1, r.to + 1))


# -- chips and a bin ------------------------------------------------------------------
bincard = dcard(dcol_a, "DROP ZONES")
ui.add(T.Muted, bincard, text="GEMS FIT THE BIN. COINS DO NOT.", wrap=True, width=ui.fill())
chips_row = ui.add(T.Row, bincard)
gem_a = ui.draggable(ui.add(T.Badge, chips_row, text="GEM 1"), 1, 0)
gem_b = ui.draggable(ui.add(T.Badge, chips_row, text="GEM 2"), 2, 0)
coin = ui.draggable(ui.add(T.Badge, chips_row, text="COIN"), 9, 1)
bin_box = ui.add(T.Card, bincard, align=ui.CENTER)
bin_hint = ui.add(T.Muted, bin_box, text="DROP HERE", display=False)
bin_text = ui.add(T.Body, bin_box, text="BIN: EMPTY")
ui.drop_zone(bin_box, [0])
bin_box.set(BinContents(0))
drag_log = ui.add(T.Muted, bincard, text="LAST: NOTHING", wrap=True, width=ui.fill())


@world.observer(Dropped)
def drag_dropped(d: Dropped):
    if d.target != bin_box.sid:
        return
    held = bin_box.get(BinContents).bits | (1 << d.payload)
    bin_box.set(BinContents(held))
    dsay(bin_text, "BIN: " + " ".join("GEM %d" % k for k in range(32) if held >> k & 1))


@world.observer(DragStarted)
def drag_started(d: DragStarted):
    dsay(drag_log, "LAST: PICKED UP %d" % d.payload)


@world.observer(DragEnded)
def drag_ended(d: DragEnded):
    dsay(drag_log, "LAST: " + ("DROPPED" if d.dropped else "LET GO"))


@world.system(phase=sage.PostUpdate)
def drag_hint(e: Entity, z: DropZone):
    if e.sid == bin_box.sid:
        bin_hint.set(Display(0 if z.over else 1))


# -- resizing --------------------------------------------------------------------------
sizecard = dcard(dcol_b, "RESIZE")
ui.add(T.Muted, sizecard, text="DRAG THE CORNER GRIP.", wrap=True, width=ui.fill())
size_root, size_content = ui.resizable(sizecard, 120, 60, min_w=60, min_h=30, prefab=T.Card, align_self="start")
size_label = ui.add(T.Body, size_content, text="RESIZE ME")

# -- shortcuts and buttons --------------------------------------------------------------
keycard = dcard(dcol_c, "KEYS AND BUTTONS")
ui.add(T.Muted, keycard, text="CTRL+S SAVES, F2 FLIPS THE SWITCH, ALT+1 PRESSES ONE.", wrap=True, width=ui.fill())
save_button = ui.shortcut(ui.add(T.Primary, keycard, text="SAVE (CTRL+S)"), "Ctrl+S")
one_button = ui.shortcut(ui.add(T.Secondary, keycard, text="ONE (ALT+1)"), "Alt+1")
switch_row = ui.add(T.Row, keycard)
f2_switch = ui.shortcut(ui.add(T.Switch, switch_row), "F2")
ui.add(T.Body, switch_row, text="F2 SWITCH")
key_said = ui.add(T.Body, keycard, text="LAST KEY: NONE")
save_button.set(Tally(0))


@world.observer(Clicked)
def drag_clicked(c: Clicked):
    if c.target == save_button.sid:
        n = save_button.get(Tally).n + 1
        save_button.set(Tally(n))
        dsay(key_said, "SAVED %d TIMES" % n)
    elif c.target == one_button.sid:
        dsay(key_said, "ONE PRESSED")


@world.observer(Toggled)
def drag_toggled(t: Toggled):
    if t.target == f2_switch.sid:
        dsay(key_said, "SWITCH " + ("ON" if t.on else "OFF"))


mouse_button = ui.add(T.Secondary, keycard, text="MIDDLE-CLICK ME")
mouse_said = ui.add(T.Body, keycard, text="MOUSE: NOTHING")


@world.observer(MiddleClicked)
def drag_middle(m: MiddleClicked):
    if m.target == mouse_button.sid:
        dsay(mouse_said, "MOUSE: MIDDLE CLICK")
