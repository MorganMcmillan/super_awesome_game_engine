# The editor's screen: UI entities over the engine's widget kit, talking to
# the document through the `editor` object the host gives this script.
#
#   top bar    undo, redo, play / stop, step, reload, save, status
#   left       the outliner: entities as a tree, add / duplicate / delete
#   center     tabs: SCENE (a viewport; click to select, drag to move),
#              CODE (the Python text, APPLY reloads it live) and
#              BLOCKS (the same Python as blocks)
#   right      the inspector, generated from the reflected components
#
# Views are rebuilt when `editor.revision()` changes, in a PostUpdate system,
# so an observer never changes the widgets it is reacting to. The document is
# the state; nothing here is saved.

import sage
from sage import world
from sage import Entity
from sage import blocks as B
from sage.ui import (
    Absolute,
    Interaction,
    Nav,
    TextField,
    Pointer,
    Rect,
    Box,
    Changed,
    Clicked,
    Context,
    ContextRequested,
    MenuChosen,
    MenuItem,
    ZIndex,
    DragEnded,
    DragStarted,
    Dragged,
    Dropped,
    Display,
    Dnd,
    DropZone,
    Label,
    Paint,
    ScrollCross,
    Selected,
    Submitted,
    TextChanged,
    Toggle,
    Toggled,
)

ui = world.ui
T = world.theme
COLUMN, ROW = ui.COLUMN, ui.ROW
START, CENTER, STRETCH = ui.START, ui.CENTER, ui.STRETCH
fill = ui.fill
ui.start()
NAV_ACCEPT, NAV_CANCEL, NAV_SHIFT = 1 << 2, 1 << 3, 1 << 14
TERMINAL = B.TERMINAL


def text_of(e):
    return world.string(e.get(Label).text)


def set_text(e, text):
    e.set(Label(world.intern(text)))


def clear(e):
    for child in list(e.children):
        child.destruct()


def short(text, limit):
    """`text`, cut to `limit` characters with "..." when it is longer. What
    is cut must be reachable some other way: see `fitted`."""
    return text if len(text) <= limit else text[: max(1, limit - 3)] + "..."


def fitted(e, text, limit):
    """Shows `text` on `e`, cut to `limit` characters. Nothing is cut
    silently: a cut text ends in "..." and the whole of it is the tooltip."""
    set_text(e, short(text, limit))
    if len(text) > limit:
        ui.tooltip(e, text)
    return e


# ---- events: a view registers a handler for the entity it makes ---------------------------
on_click, on_change, on_submit, on_toggle, on_select = {}, {}, {}, {}, {}
fixed_click = {}  # handlers of the buttons that are never rebuilt
fixed_submit, fixed_text, fixed_toggle = {}, {}, {}
dnd = {}  # what each draggable / drop target in the block view means, by entity sid
move_grips = {}  # stack grips: sid -> [function name, card entity, x0, y0]
view_dirty = [True]
status_note = [""]
every_tick = []  # functions a part wants called after each UI tick (see main.py)


@world.observer(Clicked)
def _clicked(c: Clicked):
    h = on_click.get(c.target) or fixed_click.get(c.target)
    if h:
        h()


@world.observer(Changed)
def _changed(c: Changed):
    h = on_change.get(c.target)
    if h:
        h(c.value)


@world.observer(Submitted)
def _submitted(s: Submitted):
    h = on_submit.get(s.target) or fixed_submit.get(s.target)
    if h:
        h()


@world.observer(TextChanged)
def _text_changed(t: TextChanged):
    h = fixed_text.get(t.target)
    if h:
        h()


@world.observer(Toggled)
def _toggled(t: Toggled):
    h = on_toggle.get(t.target) or fixed_toggle.get(t.target)
    if h:
        h(bool(t.on))


@world.observer(Selected)
def _selected(s: Selected):
    h = on_select.get(s.target)
    if h:
        h(s.index)


def button(parent, text, handler, prefab=None, fixed=False, **style):
    b = ui.add(prefab or T.Secondary, parent, text=text, **style)
    (fixed_click if fixed else on_click)[b.sid] = handler
    return b


def say(text):
    status_note[0] = text
    view_dirty[0] = True


def guarded(fn):
    """Runs an edit; a refused one says why instead of crashing the editor."""

    def run(*args):
        try:
            err = fn(*args)
        except ValueError as e:
            err = str(e)
        if err:
            say(str(err))
        view_dirty[0] = True

    return run
