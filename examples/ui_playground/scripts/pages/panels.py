# Page: PANELS. Tabs (with close buttons), a split pane, details, a tree,
# radio buttons, `<label for>`, a form with validation, and a template with
# slots.

import sage
from sage.ui import Display, Label, Rect, Reveal, Toggle, Toggled, Widget

ui = world.ui
T = world.theme
panels_page = world.add_page("PANELS")


def panel_card(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def psay(entity, text):
    entity.set(Label(world.intern(text)))


panels_col_a, panels_col_b, panels_col_c = (ui.add(T.Column, panels_page) for _ in range(3))

# -- tabs ------------------------------------------------------------------------------
tabcard = panel_card(panels_col_a, "TABS")
tab_root, tab_buttons_, tab_panels = ui.tabs(tabcard, ["ONE", "TWO", "THREE"], closable=True, reorder=True, width=ui.fill())
tab_text_one = ui.add(T.Body, tab_panels[0], text="THE FIRST PANEL")
tab_text_two = ui.add(T.Body, tab_panels[1], text="THE SECOND PANEL")
tab_text_three = ui.add(T.Body, tab_panels[2], text="THE THIRD PANEL")
tab_said = ui.add(T.Muted, tabcard, text="CLOSED: NOTHING")


@world.observer(ui.TabClosed)
def panels_tab_closed(c: ui.TabClosed):
    psay(tab_said, "CLOSED: TAB %d" % (c.index + 1))


# -- split -----------------------------------------------------------------------------
splitcard = panel_card(panels_col_a, "SPLIT PANE")
ui.add(T.Muted, splitcard, text="DRAG THE DIVIDER.", width=ui.fill())
split_root, split_a, split_b = ui.split(splitcard, ratio=0.4, min_a=30, min_b=30, width=ui.fill(), height=50)
ui.add(T.Chip, split_a, text="LEFT")
ui.add(T.Chip, split_b, text="RIGHT")

# -- details ---------------------------------------------------------------------------
detcard = panel_card(panels_col_a, "DETAILS")
details_root, details_body = ui.details(detcard, "MORE INFORMATION", width=ui.fill())
details_text = ui.add(T.Body, details_body, text="HIDDEN UNTIL OPENED.")

# -- tree ------------------------------------------------------------------------------
treecard = panel_card(panels_col_b, "TREE")
tree_root = ui.tree(
    treecard,
    [
        ("WORLD", [("ENTITIES", ["PLAYER", "ENEMY"]), "LIGHTS"]),
        ("ASSETS", ["SPRITES", "SOUNDS"]),
        "SETTINGS",
    ],
    reorder=True,
    width=ui.fill(),
)
tree_said = ui.add(T.Body, treecard, text="SELECTED: NONE")


@world.observer(ui.TreeSelected)
def panels_tree_selected(s: ui.TreeSelected):
    row = world.lookup(s.row)
    psay(tree_said, "SELECTED: " + world.string(row.get(Label).text))


# -- radio and label for ---------------------------------------------------------------
radiocard = panel_card(panels_col_b, "RADIO BUTTONS")
radio_root, radios = ui.radio_group(radiocard, ["SMALL", "MEDIUM", "LARGE"], value=1)
radio_said = ui.add(T.Body, radiocard, text="CHOSEN: MEDIUM")
SIZES = ["SMALL", "MEDIUM", "LARGE"]


@world.observer(Toggled)
def panels_radio_changed(t: Toggled):
    info = ui.radio_item_of(world.lookup(t.target))
    if info is not None and t.on:
        psay(radio_said, "CHOSEN: " + SIZES[info[1]])


labelcard = panel_card(panels_col_b, "LABEL FOR")
ui.add(T.Muted, labelcard, text="CLICK THE TEXT, NOT THE BOX.", wrap=True, width=ui.fill())
label_row = ui.add(T.Row, labelcard)
label_switch = ui.add(T.Switch, label_row)
label_text = ui.label_for(label_row, "ENABLE SOUND", label_switch)
label_name = ui.text_input(labelcard, "", placeholder="NAME", prefab=T.Field)
label_name_text = ui.label_for(labelcard, "FOCUS THE NAME", label_name)

# -- form ------------------------------------------------------------------------------
formcard = panel_card(panels_col_c, "FORM")
form_root = ui.form(formcard, width=ui.fill())
form_name = ui.form_row(form_root, "NAME", "name", required=True, min_len=2, max_len=12, prefab=T.Field)
form_age = ui.form_row(form_root, "AGE", "age", kind="number", low=1, high=120, value=30, prefab=T.Field)
form_code = ui.form_row(
    form_root, "CODE", "code", pattern=r"[A-Z]{3}-\d{2}", message="USE ABC-12", placeholder="ABC-12", prefab=T.Field
)
form_buttons = ui.add(T.Row, form_root)
form_ok = ui.add(T.Primary, form_buttons, text="SUBMIT")
form_reset = ui.add(T.Secondary, form_buttons, text="RESET")
ui.form_submit(form_root, form_ok)
ui.form_submit(form_root, form_reset, reset=True)
form_said = ui.add(T.Body, formcard, text="NOT SUBMITTED")


@world.observer(ui.FormSubmitted)
def panels_form_submitted(f: ui.FormSubmitted):
    v = ui.form_values(form_root)
    psay(form_said, "SUBMITTED " + v["name"].upper())


@world.observer(ui.FormInvalid)
def panels_form_invalid(f: ui.FormInvalid):
    psay(form_said, "%d PROBLEMS" % f.count)


# -- template and slots ----------------------------------------------------------------
slotcard = panel_card(panels_col_c, "TEMPLATE WITH SLOTS")


def dialog_card(parent, title):
    box = ui.add(T.Card, parent, width=ui.fill())
    ui.add(T.Heading, box, text=title)
    ui.slot(box, "body")
    ui.slot(box, "footer", direction=ui.ROW)
    return box


slot_card = ui.instantiate(
    dialog_card,
    slotcard,
    "A TEMPLATE",
    slots={
        "body": lambda s: ui.add(T.Body, s, text="THE CALLER FILLS THIS."),
        "footer": lambda s: ui.add(T.Ghost, s, text="OK"),
    },
)
slot_body = ui.slot_of(slot_card, "body")
slot_footer = ui.slot_of(slot_card, "footer")
