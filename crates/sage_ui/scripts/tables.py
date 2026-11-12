# The table: a grid with a header row. Clicking a header sorts by that column
# (again to reverse), clicking a cell selects its row. All state is in
# components, so it is hashed and saved with the world.

import sage
from sage import component
from sage.ui import Clicked, Label, Toggle, Toggled

ui = world.ui


@component
class Table:
    """A table's state. `sort_col` is -1 when unsorted; `selected` is the
    chosen row (-1 for none)."""

    cols: sage.u32 = 0
    rows: sage.u32 = 0
    sort_col: sage.i32 = -1
    desc: sage.u8 = 0
    selected: sage.i32 = -1


@component
class HeadOf:
    grid: sage.u64 = 0
    col: sage.u32 = 0


@component
class CellOf:
    grid: sage.u64 = 0
    row: sage.u32 = 0
    col: sage.u32 = 0


ui.TableHeader = world.prefab("UiTableHeader", base=ui.Button)
ui.TableCell = world.prefab("UiTableCell", base=ui.Choice)


def table(parent, header, rows, widths=None, **style):
    """A table: a grid with a header row of buttons (click to sort) over rows
    of cells (click to select a row). `header` is a list of titles, `rows` a
    list of rows of values, `widths` a track list (default: every column
    `auto`, the last `1fr`). Returns the grid; read `grid.get(Table)` for the
    sort and the selected row, and `ui.table_cells(grid)` for the cells."""
    n = len(header)
    tracks = widths if widths is not None else ["auto"] * (n - 1) + ["1fr"]
    g = ui.grid(parent, tracks, "", (4, 2), **style)
    g.set(Table(n, len(rows), -1, 0, -1))
    for col, title in enumerate(header):
        h = ui.add(ui.TableHeader, g, text=str(title))
        h.set(HeadOf(g.sid, col))
    for r, row in enumerate(rows):
        for col, value in enumerate(row):
            c = ui.add(ui.TableCell, g, text=str(value))
            c.set(CellOf(g.sid, r, col))
    return g


def table_cells(grid):
    """The cells as a list of rows (the header is not included)."""
    t = grid.get(Table)
    kids = list(grid.children)[t.cols:]
    return [kids[r * t.cols:(r + 1) * t.cols] for r in range(t.rows)]


def table_values(grid):
    return [[_text(c) for c in row] for row in table_cells(grid)]


def _text(e):
    return world.string(e.get(Label).text)


def _key(text):
    try:
        return (0, float(text), "")
    except ValueError:
        return (1, 0.0, text.lower())


def _select(grid, row):
    t = grid.get(Table).copy()
    t.selected = row
    grid.set(t)
    for r, cells in enumerate(table_cells(grid)):
        for c in cells:
            c.set(Toggle(1 if r == row else 0))


def table_selected(grid):
    """The selected row of a table, or -1."""
    return grid.get(Table).selected


def table_sort(grid):
    """`(column, descending)` of a table's sort, or `(-1, False)`."""
    t = grid.get(Table)
    return t.sort_col, bool(t.desc)


def table_cell_of(cell):
    """`(grid sid, row, column)` of a table cell, or None."""
    if not cell.has(CellOf):
        return None
    k = cell.get(CellOf)
    return k.grid, k.row, k.col


ui.table, ui.table_cells, ui.table_values = table, table_cells, table_values
ui.table_cell_of = table_cell_of
ui.table_selected, ui.table_sort = table_selected, table_sort


@world.observer(Clicked)
def _ui_table_header(c: Clicked):
    e = world.lookup(c.target)
    if e is None or not e.has(HeadOf):
        return
    h = e.get(HeadOf)
    grid = world.lookup(h.grid)
    t = grid.get(Table).copy()
    t.desc = 0 if t.sort_col != h.col else 1 - t.desc
    t.sort_col = h.col
    t.selected = -1
    grid.set(t)
    cells = table_cells(grid)
    data = [[_text(c) for c in row] for row in cells]
    data.sort(key=lambda row: _key(row[h.col]), reverse=bool(t.desc))
    for row, values in zip(cells, data):
        for cell, value in zip(row, values):
            cell.set(Label(world.intern(value)))
            cell.set(Toggle(0))


@world.observer(Toggled)
def _ui_table_cell(c: Toggled):
    e = world.lookup(c.target)
    if e is None or not e.has(CellOf):
        return
    k = e.get(CellOf)
    _select(world.lookup(k.grid), k.row if c.on else -1)
