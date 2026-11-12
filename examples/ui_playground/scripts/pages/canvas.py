# Page: CANVAS. Vector shapes a script draws (a chart, a moving marker) and
# an embedded simulation view the host draws and the pointer drives.

import sage
from sage import Entity, component
from sage.ui import Label, Viewport

ui = world.ui
T = world.theme
canvas_page = world.add_page("CANVAS")

BLUE, ORANGE, GREEN, GRID = 0x4FC3C0FF, 0xF28C28FF, 0x7BC96FFF, 0x3A332CFF


def ccard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def csay(entity, text):
    entity.set(Label(world.intern(text)))


@component
class Sweep:
    """The tick counter that drives the moving marker."""

    n: sage.u32 = 0


ccol_a, ccol_b = (ui.add(T.Column, canvas_page) for _ in range(2))

# -- a static chart --------------------------------------------------------------------
chartcard = ccard(ccol_a, "CHART")
ui.add(T.Muted, chartcard, text="LINES, TRIANGLES AND RECTANGLES, CUT TO THE BOX.", wrap=True, width=ui.fill())
chart = ui.canvas(chartcard, 200, 80, fill=0x14110FFF)
DATA = [10, 34, 22, 50, 41, 66, 58]
for k in range(5):
    ui.draw_line(chart, 0, 16 * k + 8, 200, 16 * k + 8, GRID)
for k in range(len(DATA) - 1):
    ui.draw_line(chart, 20 + 28 * k, 80 - DATA[k], 20 + 28 * (k + 1), 80 - DATA[k + 1], BLUE, 2)
for k, v in enumerate(DATA):
    ui.draw_ellipse(chart, 20 + 28 * k, 80 - v, 3, 3, ORANGE)
ui.draw_triangle(chart, (180, 75), (195, 75), (187, 60), GREEN)
ui.draw_rect(chart, 4, 4, 10, 6, ORANGE)
# Beyond the box: this line is cut off at the canvas edge.
ui.draw_line(chart, 150, 70, 400, 20, GREEN, 2)
chart_count = ui.shape_count(chart)

# -- an animated marker -----------------------------------------------------------------
movecard = ccard(ccol_a, "MOVING SHAPES")
mover = ui.canvas(movecard, 200, 40, fill=0x14110FFF)
mover.set(Sweep(0))


@world.system(phase=sage.PreUpdate)
def canvas_sweep(e: Entity, s: Sweep):
    n = s.n + 1
    s.n = n
    # A triangle wave without trigonometry: 0 to 160 and back every 320 ticks.
    t = n % 320
    x = t if t < 160 else 320 - t
    ui.canvas_clear(e)
    ui.draw_line(e, 0, 20, 200, 20, GRID)
    ui.draw_ellipse(e, 20 + x, 20, 8, 8, ORANGE)
    ui.draw_line(e, 20 + x, 4, 20 + x, 36, BLUE, 1)


# -- an embedded simulation --------------------------------------------------------------
viewcard = ccard(ccol_b, "VIEWPORT")
ui.add(T.Muted, viewcard, text="THE HOST DRAWS A SIMULATION HERE. DRAG THE BALL WITH THE MOUSE.", wrap=True, width=ui.fill())
view = ui.viewport(viewcard, 1, width=ui.fill(), height=120, fill=0x000000FF)
view_said = ui.add(T.Body, viewcard, text="POINTER: OUTSIDE")


@world.system(phase=sage.PostUpdate)
def canvas_view_report(e: Entity, v: Viewport):
    if v.source != 1:
        return
    text = "POINTER: " + ("OVER" if v.over else "OUTSIDE") + ("  PRESSED" if v.down & 1 else "")
    if world.string(view_said.get(Label).text) != text:
        csay(view_said, text)
