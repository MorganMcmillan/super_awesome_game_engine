# The table: a grid with a header row. Clicking a header sorts by that column
# (again to reverse), clicking a cell selects its row. All state is in
# components, so it is hashed and saved with the world.

import sage
from sage import component
from sage.ui import Clicked, Dragged, Drag, Grid as GridC, Label, Rect, Toggle, Toggled, Widget, Box

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


@component
class ColGrip:
    """The grip at a column header's right edge: its table, its column, and
    that column's width when the drag began."""

    grid: sage.u64 = 0
    col: sage.u32 = 0
    start: sage.f32 = 0.0


ui.ColGrip = world.prefab("UiColGrip", base=ui.Panel)
ui.ColGrip.set(Widget(8, 1, 0, 0)).set(Drag())
MIN_COLUMN = 16.0


def table(parent, header, rows, widths=None, resizable=False, **style):
    """A table: a grid with a header row of buttons (click to sort) over rows
    of cells (click to select a row). `header` is a list of titles, `rows` a
    list of rows of values, `widths` a track list (default: every column
    `auto`, the last `1fr`). Returns the grid; read `grid.get(Table)` for the
    sort and the selected row, and `ui.table_cells(grid)` for the cells.
    `resizable` puts a grip at the right edge of every header but the last:
    drag it to give that column a fixed width."""
    n = len(header)
    tracks = widths if widths is not None else ["auto"] * (n - 1) + ["1fr"]
    g = ui.grid(parent, tracks, "", (4, 2), **style)
    g.set(Table(n, len(rows), -1, 0, -1))
    ui.role(g, "table")
    for col, title in enumerate(header):
        h = ui.add(ui.TableHeader, g, text=str(title), width=ui.fill())
        h.set(HeadOf(g.sid, col))
        ui.role(h, "columnheader")
    if resizable:
        for col in range(n - 1):
            grip = ui.add(ui.ColGrip, g, cell=(col + 1, 1, 1, 1), align_self="end", width=3, height=ui.fill())
            grip.set(ColGrip(g.sid, col, 0.0))
            ui.role(grip, "separator", name="RESIZE COLUMN")
    for r, row in enumerate(rows):
        for col, value in enumerate(row):
            c = ui.add(ui.TableCell, g, text=str(value), width=ui.fill())
            c.set(CellOf(g.sid, r, col))
            ui.role(c, "cell")
    return g


def table_cells(grid):
    """The cells as a list of rows (the header is not included)."""
    t = grid.get(Table)
    kids = [c for c in list(grid.children)[t.cols:] if c.has(CellOf)]
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


def table_columns(grid):
    """The grid's column tracks as a list of strings (`["40", "auto", "1fr"]`)."""
    return world.string(grid.get(GridC).cols).split()


def table_set_column(grid, col, width):
    """Gives column `col` a fixed width (at least a few units)."""
    tracks = table_columns(grid)
    tracks[col] = "%g" % max(MIN_COLUMN, float(width))
    g = grid.get(GridC).copy()
    g.cols = world.intern(" ".join(tracks))
    grid.set(g)


ui.table_columns, ui.table_set_column = table_columns, table_set_column
ui.ColGripState = ColGrip


@world.observer(Dragged)
def _ui_col_grip(d: Dragged):
    e = world.lookup(d.target)
    if e is None or not e.has(ColGrip):
        return
    info = e.get(ColGrip).copy()
    grid = world.lookup(info.grid)
    header = grid.children[info.col]
    if d.phase == 0:
        info.start = header.get(Rect).w
        e.set(info)
        return
    table_set_column(grid, info.col, info.start + d.dx)
