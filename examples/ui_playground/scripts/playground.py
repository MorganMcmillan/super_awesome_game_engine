# The UI playground: every widget in the UI module, themed by style prefabs
# (theme.py). Everything here is an entity; the behavior is Python observers
# and one system.
#
# Try it: click, drag the sliders, type in the name field, scroll the list,
# pick a theme (it edits the prefabs; every instance follows), open the
# dialog. Tab moves focus; Enter presses; arrows adjust sliders and move the
# caret; PageUp/PageDown scroll.

import sage
from sage import Entity, component
from sage.ui import (
    Caret,
    Changed,
    Clicked,
    Display,
    Label,
    Modal,
    Paint,
    Scroll,
    Slider,
    TextChanged,
    Toggle,
    Toggled,
    Widget,
)

ui = world.ui
T = world.theme
ui.start()


# State lives in components, not Python variables, so it is hashed and saved
# with the world (design doc 3.3).
@component
class Driver:
    """The one entity that switches pages."""

    request: sage.u8 = 0  # 3 show page `arg`; handled after the tick's events
    arg: sage.u8 = 0
    page: sage.u8 = 0  # the page being shown


@component
class CloseButton:
    """A button that closes the dialog."""


@component
class Counter:
    n: sage.u32 = 0


@component
class Autofill:
    """A slider that fills itself over time (the progress bar's animation)."""


def set_text(entity, text):
    entity.set(Label(world.intern(text)))


def text_of(entity):
    return world.string(entity.get(Label).text)


def card(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def switch(parent, text, on=False):
    row = ui.add(T.Row, parent)
    box = ui.add(T.Switch, row)
    box.set(Toggle(1 if on else 0))
    ui.add(T.Body, row, text=text)
    return box


def labeled_row(parent, text, value):
    row = ui.add(T.Row, parent, justify=ui.SPACE_BETWEEN)
    ui.add(T.Body, row, text=text)
    return ui.add(T.Muted, row, text=value)


# ---- the screen -------------------------------------------------------------
app = ui.add(T.Surface)
bar = ui.add(T.Bar, app)
ui.add(T.Title, bar, text="UI PLAYGROUND")
nav = ui.add(T.Row, bar, gap=3)
cols = ui.add(T.Cols, app)
left, middle, right = (ui.add(T.Column, cols) for _ in range(3))

# ---- pages --------------------------------------------------------------------
# Each page is a root under `app`; only one is shown (Display none hides the
# rest). Page scripts loaded after this one call `world.add_page(name)`.
page_roots = [cols]
tab_buttons = [ui.add(T.Tab, nav, text="BASICS")]
tab_buttons[0].set(Toggle(1))
tab_sids = {tab_buttons[0].sid: 0}
page_names = ["BASICS"]


def add_page(name):
    """A new page: returns its root (a row of columns, hidden until its tab
    is chosen)."""
    root = ui.add(T.Cols, app, display=False)
    tab = ui.add(T.Tab, nav, text=name)
    tab_sids[tab.sid] = len(page_roots)
    page_names.append(name)
    page_roots.append(root)
    tab_buttons.append(tab)
    return root


world.add_page = add_page

# -- column 1: buttons, toggles ---------------------------------------------------
buttons = card(left, "BUTTONS")
row = ui.add(T.Row, buttons)
primary = ui.add(T.Primary, row, text="PRIMARY")
second = ui.add(T.Secondary, row, text="SECOND")
row = ui.add(T.Row, buttons)
ghost = ui.add(T.Ghost, row, text="GHOST")
danger = ui.add(T.Danger, row, text="DANGER")
disabled = ui.add(T.Primary, buttons, text="DISABLED", enabled=False)
clicks = ui.add(T.Muted, buttons, text="CLICKS: 0")
clicks.set(Counter(0))

toggles = card(left, "TOGGLES")
sound = switch(toggles, "SOUND", on=True)
music = switch(toggles, "MUSIC")
extras = switch(toggles, "ENABLE EXTRAS", on=True)
extra_button = ui.add(T.Ghost, toggles, text="EXTRA ACTION")

# -- column 2: sliders, text input, themes ---------------------------------------
sliders = card(middle, "SLIDERS AND METERS")
volume_label = labeled_row(sliders, "VOLUME", "7")
volume = ui.add(T.Track, sliders)
volume.set(Slider(7.0, 0.0, 10.0, 1.0))
bright_label = labeled_row(sliders, "BRIGHTNESS", "3 OF 5")
brightness = ui.add(T.Track, sliders)
brightness.set(Slider(3.0, 0.0, 5.0, 1.0))
ui.add(T.Body, sliders, text="LOADING")
progress = ui.add(T.Gauge, sliders)
progress.set(Slider(0.0, 0.0, 1.0, 0.0)).add(Autofill)

entry = card(middle, "TEXT INPUT")
ui.add(T.Muted, entry, text="YOUR NAME")
name = ui.add(T.Field, entry, text="Ada")
greeting = ui.add(T.Body, entry, text="HELLO, ADA!")

themes = card(middle, "STYLE PREFABS")
ui.add(
    T.Para,
    themes,
    text="EVERY WIDGET HERE IS AN INSTANCE OF A PREFAB. A THEME EDITS THE PREFABS, AND EVERY INSTANCE FOLLOWS.",
)
row = ui.add(T.Row, themes)
swatches = {}
swatch_of = {}
for label, color in T.accents.items():
    swatch = ui.add(T.Swatch, row, text=label, fill=color)
    swatches[swatch.sid] = color
    swatch_of[label] = swatch
swatch_orange, swatch_teal, swatch_violet = (swatch_of[k] for k in ("ORANGE", "TEAL", "VIOLET"))

# -- column 3: text styles, layout, scrolling -------------------------------------
styles = card(right, "TEXT STYLES")
ui.add(T.Title, styles, text="TITLE")
ui.add(T.Body, styles, text="BODY TEXT")
ui.add(T.Muted, styles, text="MUTED TEXT")
ui.add(T.Para, styles, text="WRAPPED TEXT BREAKS LINES TO FIT ITS PARENT.")

layout = card(right, "LAYOUT")
ui.add(T.Badge, layout, text="ABSOLUTE", at=(84, 0))
row = ui.add(T.Row, layout)
for weight in (1, 2, 1):
    ui.add(T.Chip, row, text=str(weight), width=ui.fill(weight))
row = ui.add(T.Row, layout, justify=ui.SPACE_BETWEEN)
for letter in "ABC":
    ui.add(T.Chip, row, text=letter, width=24)

scrolling = card(right, "SCROLL LIST")
scroll_area = ui.scroll(scrolling, width=ui.fill(), height=40, gap=2)
items = [ui.add(T.Item, scroll_area, text=f"ITEM {k}") for k in range(1, 11)]

dialog_button = ui.add(T.Primary, right, text="OPEN DIALOG")

keys = card(left, "KEYS")
ui.add(T.Para, keys, text="TAB MOVES FOCUS. ENTER PRESSES. ARROWS ADJUST. PAGEUP AND PAGEDOWN SCROLL.")

# ---- behavior ---------------------------------------------------------------
driver = world.entity()
driver.set(Driver(0, 0, 0))


# The dialog is built once, closed. It is a layer: a press on the backdrop
# does nothing, keyboard focus stays inside, Escape closes it, and focus
# returns to the button that opened it.
dialog_back, dialog_box = ui.dialog()
ui.add(T.Title, dialog_box, text="DIALOG")
ui.add(T.Para, dialog_box, text="A MODAL: THE BACKDROP BLOCKS THE POINTER, AND KEYBOARD FOCUS STAYS INSIDE THIS BOX.")
_row = ui.add(T.Row, dialog_box)
dialog_cancel = ui.add(T.Secondary, _row, text="CANCEL").add(CloseButton)
dialog_ok = ui.add(T.Primary, _row, text="OK").add(CloseButton)


def dialog_buttons():
    """The dialog's buttons, cancel then OK."""
    return [dialog_cancel, dialog_ok]


@world.observer(Clicked)
def on_click(c: Clicked):
    if c.target == primary.sid:
        count = clicks.get(Counter).n + 1
        clicks.set(Counter(count))
        set_text(clicks, f"CLICKS: {count}")
    elif c.target == dialog_button.sid:
        ui.open(dialog_back)
    elif c.target in swatches:
        T.apply(swatches[c.target])
    elif world.lookup(c.target).has(CloseButton):
        ui.close(dialog_back)


@world.observer(Toggled)
def on_toggle(t: Toggled):
    if t.target in tab_sids:
        d = driver.get(Driver)
        d.request = 3
        d.arg = tab_sids[t.target]
        return
    if t.target == extras.sid:
        w = extra_button.get(Widget).copy()
        w.enabled = t.on
        extra_button.set(w)


@world.observer(Changed)
def on_change(c: Changed):
    if c.target == volume.sid:
        set_text(volume_label, str(int(c.value)))
    elif c.target == brightness.sid:
        set_text(bright_label, f"{int(c.value)} OF 5")


@world.observer(TextChanged)
def on_text(t: TextChanged):
    if t.target == name.sid:
        set_text(greeting, "HELLO, " + text_of(name).upper() + "!")


@world.system(phase=sage.PreUpdate, query="Autofill")
def advance(s: Slider):
    s.value += 0.008
    if s.value > s.max:
        s.value = s.min


@world.system(phase=sage.PostUpdate)
def apply_request(e: Entity, d: Driver):
    # Switching pages happens here, after the tick's events have been
    # delivered, so an observer never changes what it is reacting to.
    if d.request != 3:
        return
    arg = d.arg
    for i, (page, tab) in enumerate(zip(page_roots, tab_buttons)):
        page.set(Display(0 if i == arg else 1))
        tab.set(Toggle(1 if i == arg else 0))
    e.set(Driver(0, 0, arg))
