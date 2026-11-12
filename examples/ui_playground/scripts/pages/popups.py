# Page: POPUPS. Drop-downs, a combo box (datalist), a menu bar with a submenu,
# check items and shortcuts, a context menu, tooltips, and dialogs. All of it
# is layers: floating nodes placed next to an anchor, closed by an outside
# press or Escape, with keyboard focus moving in and returning.

import sage
from sage.ui import Clicked, Combo, Interaction, Label, Layer, MenuChosen, MenuItem, MenuTitle, Select, Selected

ui = world.ui
T = world.theme
popups_page = world.add_page("POPUPS")


def pcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def say(entity, text):
    entity.set(Label(world.intern(text)))


pcol_a, pcol_b, pcol_c = (ui.add(T.Column, popups_page) for _ in range(3))

# -- drop-downs ----------------------------------------------------------------------
drops = pcard(pcol_a, "SELECT")
ui.add(T.Muted, drops, text="A DROP-DOWN LIST")
COLORS = ["ORANGE", "TEAL", "VIOLET", "RED", "GREEN", "BLUE"]
color_select = ui.select(drops, COLORS, value=0, width=ui.fill())
color_said = ui.add(T.Body, drops, text="CHOSEN: ORANGE")

combos = pcard(pcol_a, "COMBO BOX")
ui.add(T.Muted, combos, text="TYPE TO NARROW THE LIST")
FRUIT = ["APPLE", "APRICOT", "AVOCADO", "BANANA", "BLUEBERRY", "CHERRY", "GRAPE", "LEMON", "MANGO"]
fruit_combo = ui.combo(combos, FRUIT, text="", width=ui.fill())
fruit_said = ui.add(T.Body, combos, text="PICKED: NOTHING")

# -- menus ---------------------------------------------------------------------------
menus = pcard(pcol_b, "MENU BAR")
menu_bar = ui.menu_bar(menus)
file_menu = ui.menu(
    menu_bar,
    "FILE",
    [
        ("NEW", {"id": 1, "shortcut": "CTRL+N"}),
        ("OPEN", {"id": 2, "shortcut": "CTRL+O"}),
        ("RECENT", {"sub": [("ONE.TXT", {"id": 11}), ("TWO.TXT", {"id": 12}), "-", ("CLEAR", {"id": 13})]}),
        "-",
        ("SAVE", {"id": 3, "shortcut": "CTRL+S", "disabled": True}),
        ("QUIT", {"id": 4}),
    ],
)
view_menu = ui.menu(
    menu_bar,
    "VIEW",
    [
        ("GRID", {"id": 21, "check": True, "checked": True}),
        ("RULERS", {"id": 22, "check": True}),
        "-",
        ("ZOOM IN", {"id": 23, "shortcut": "+"}),
        ("ZOOM OUT", {"id": 24, "shortcut": "-"}),
    ],
)
menu_said = ui.add(T.Body, menus, text="CHOSEN: NOTHING")

context = pcard(pcol_b, "CONTEXT MENU")
context_area = ui.add(T.Chip, context, text="RIGHT-CLICK HERE", height=34)
context_popup = ui.context_menu(
    context_area,
    [("COPY", {"id": 31, "shortcut": "CTRL+C"}), ("PASTE", {"id": 32, "shortcut": "CTRL+V"}), "-", ("DELETE", {"id": 33})],
)
context_said = ui.add(T.Body, context, text="LAST: NOTHING")

# -- tooltips and dialogs ----------------------------------------------------------------
tips = pcard(pcol_c, "TOOLTIPS")
tip_button = ui.add(T.Primary, tips, text="HOVER ME")
ui.tooltip(tip_button, "A TOOLTIP APPEARS AFTER A SHORT REST.", delay=18)
slow_button = ui.add(T.Secondary, tips, text="SLOWER ONE")
ui.tooltip(slow_button, "THIS ONE WAITS LONGER.", delay=60)
tip_group = ui.add(T.Card, tips)
ui.tooltip(tip_group, "A TOOLTIP ON A GROUP: HOVERING ANYTHING INSIDE SHOWS IT.", delay=18)
tip_inner = ui.add(T.Ghost, tip_group, text="INSIDE THE GROUP")

dialogs = pcard(pcol_c, "DIALOGS")
confirm_button = ui.add(T.Primary, dialogs, text="CONFIRM...")
confirm_said = ui.add(T.Body, dialogs, text="ANSWER: NONE")
confirm_back, confirm_box = ui.dialog()
ui.add(T.Title, confirm_box, text="SURE?")
ui.add(T.Para, confirm_box, text="ESCAPE CANCELS. TAB STAYS INSIDE. FOCUS RETURNS TO THE BUTTON.")
_row = ui.add(T.Row, confirm_box)
confirm_no = ui.add(T.Secondary, _row, text="NO")
confirm_yes = ui.add(T.Primary, _row, text="YES")


@world.observer(Selected)
def popups_selected(s: Selected):
    if s.target == color_select.sid:
        say(color_said, "CHOSEN: " + COLORS[s.index])
    elif s.target == fruit_combo.sid:
        say(fruit_said, "PICKED: " + FRUIT[s.index])


@world.observer(MenuChosen)
def popups_menu(m: MenuChosen):
    names = {1: "NEW", 2: "OPEN", 4: "QUIT", 11: "ONE.TXT", 12: "TWO.TXT", 13: "CLEAR", 21: "GRID", 22: "RULERS", 23: "ZOOM IN", 24: "ZOOM OUT"}
    if m.id in (31, 32, 33):
        say(context_said, "LAST: " + {31: "COPY", 32: "PASTE", 33: "DELETE"}[m.id])
    else:
        say(menu_said, "CHOSEN: " + names.get(m.id, str(m.id)))


@world.observer(Clicked)
def popups_clicks(c: Clicked):
    if c.target == confirm_button.sid:
        ui.open(confirm_back)
    elif c.target in (confirm_no.sid, confirm_yes.sid):
        say(confirm_said, "ANSWER: " + ("YES" if c.target == confirm_yes.sid else "NO"))
        ui.close(confirm_back)


def popup_option(owner, i):
    """The i-th option entity of a select or combo (for tests and scripts)."""
    kind = Select if owner.has(Select) else Combo
    return world.lookup(owner.get(kind).popup).children[i]


def menu_item(menu, *path):
    """The menu row reached by item labels from a menu title (or popup):
    menu_item(file_menu, "RECENT", "TWO.TXT"). For tests and scripts."""
    popup = world.lookup(menu.get(MenuTitle).popup) if menu.has(MenuTitle) else menu
    row = None
    for label in path:
        row = next(c for c in popup.children if c.has(MenuItem) and len(c.children) > 1 and world.string(c.children[1].get(Label).text) == label)
        if row.get(MenuItem).sub:
            popup = world.lookup(row.get(MenuItem).sub)
    return row
