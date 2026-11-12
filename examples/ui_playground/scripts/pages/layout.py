# Page: LAYOUT. Margin, min and max size, percentages, aspect ratio, align
# self, relative offset, wrapping rows, grids with spans, a sortable table,
# and lists.

import sage
from sage.ui import Label, Toggled

ui = world.ui
T = world.theme
layout_page = world.add_page("LAYOUT")


def lcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def box(parent, text="", **style):
    return ui.add(T.Chip, parent, text=text, **style)


lcol_a, lcol_b, lcol_c = (ui.add(T.Column, layout_page) for _ in range(3))

# -- box model -------------------------------------------------------------------------
model = lcard(lcol_a, "MARGIN AND LIMITS")
strip = ui.add(T.Card, model, direction=ui.ROW, gap=0, padding=2)
margin_a = box(strip, "A", width=24, margin=(2, 0, 2, 0))
margin_b = box(strip, "B", width=24, margin=(10, 0, 0, 0))
limit_fill = box(strip, "MAX 40", width=ui.fill(), max_width=40)
ui.add(T.Muted, model, text="MARGINS SPACE THE BOXES. MAX CAPS A FILL.", wrap=True, width=ui.fill())
percent_row = ui.add(T.Card, model, direction=ui.ROW, gap=2, padding=2)
pct_a = box(percent_row, "25%", width=ui.pct(25))
pct_b = box(percent_row, "50%", width=ui.pct(50))
ratio = ui.add(T.Badge, model, text="2:1", width=60, aspect=2.0)
selfs = ui.add(T.Card, model, direction=ui.COLUMN, padding=2, gap=2)
self_start = box(selfs, "START", width=50, align_self="start")
self_center = box(selfs, "CENTER", width=50, align_self="center")
self_end = box(selfs, "END", width=50, align_self="end")
shifted = box(model, "SHIFTED", width=60, offset=(12, 0))

# -- wrap ------------------------------------------------------------------------------
wrapcard = lcard(lcol_b, "WRAPPING ROW")
chips = ui.add(T.Row, wrapcard, wrap_flow=3, gap=3, align=ui.START)
chip_ids = [ui.add(T.Badge, chips, text=w) for w in ["ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX", "SEVEN", "EIGHT", "NINE", "TEN"]]

gridcard = lcard(lcol_b, "GRID")
ui.add(T.Muted, gridcard, text="COLUMNS: 40 AUTO 1FR. THE WIDE CELL SPANS TWO.", wrap=True, width=ui.fill())
grid = ui.grid(gridcard, "40 auto 1fr", "", (3, 3), width=ui.fill())
cell_a = box(grid, "A", width=ui.fill())
cell_b = box(grid, "BB", width=ui.fill())
cell_c = box(grid, "C", width=ui.fill())
cell_wide = box(grid, "SPANS TWO COLUMNS", width=ui.fill(), cell=(1, 2, 2, 1))
cell_d = box(grid, "D", width=ui.fill())

# -- table and lists -------------------------------------------------------------------
tablecard = lcard(lcol_c, "TABLE")
ui.add(T.Muted, tablecard, text="CLICK A HEADER TO SORT, A ROW TO SELECT.", wrap=True, width=ui.fill())
table = ui.table(
    tablecard,
    ["NAME", "LEVEL", "GOLD"],
    [["ADA", 12, 340], ["BO", 7, 1200], ["CY", 31, 85], ["DI", 9, 560]],
    width=ui.fill(),
)
table_said = ui.add(T.Body, tablecard, text="SELECTED: NONE")

listcard = lcard(lcol_c, "LISTS")
bullets = ui.list(listcard, ["FIRST ITEM", "SECOND ITEM, WHICH IS LONG ENOUGH TO WRAP ONTO ANOTHER LINE", "THIRD"], width=ui.fill())
numbers_list = ui.list(listcard, ["PLAN", "BUILD", "SHIP"], ordered=True, width=ui.fill())


def lsay(entity, text):
    entity.set(Label(world.intern(text)))


@world.observer(Toggled)
def layout_table_selection(t: Toggled):
    cell = world.lookup(t.target)
    info = ui.table_cell_of(cell) if cell is not None else None
    if info is None or info[0] != table.sid:
        return
    if t.on:
        lsay(table_said, "SELECTED: " + ui.table_values(table)[info[1]][0])
    else:
        lsay(table_said, "SELECTED: NONE")
