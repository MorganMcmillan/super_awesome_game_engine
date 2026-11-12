# A canvas (vector shapes a script draws) and an embedded viewport (another
# simulation, drawn by the host and fed the pointer). State is in components.

import sage
from sage import component
from sage.ui import Absolute, Box, Display, Shape, Viewport, Widget

ui = world.ui
VIEWPORT = 10


@component
class Canvas:
    """A canvas: its size, and how many of its shape children this pass
    has used (`used` resets on `canvas_clear`)."""

    w: sage.f32 = 0.0
    h: sage.f32 = 0.0
    used: sage.u32 = 0


def canvas(parent, width, height, **style):
    """A `<canvas>`: a box of `width` x `height` units that a script draws
    shapes into with `ui.draw_line`, `draw_triangle`, `draw_ellipse` and
    `draw_rect`. Anything outside the box is cut off. Call `ui.canvas_clear`
    before drawing each frame; shapes are reused, not recreated."""
    c = ui.add(ui.Panel, parent, width=width, height=height, **style)
    c.set(Canvas(float(width), float(height), 0))
    return c


def canvas_clear(c):
    """Starts a new drawing pass: the shapes of the last one go."""
    cv = c.get(Canvas).copy()
    # Hide every shape; the ones drawn again show themselves.
    for child in c.children:
        if child.has(Shape):
            child.set(Display(1))
    cv.used = 0
    c.set(cv)


def _slot(c):
    cv = c.get(Canvas).copy()
    shapes = [e for e in c.children if e.has(Shape)]
    if cv.used < len(shapes):
        e = shapes[cv.used]
    else:
        e = ui.add(ui.Panel, c, width=cv.w, height=cv.h, at=(0.0, 0.0))
    cv.used += 1
    c.set(cv)
    e.set(Display(0))
    return e


def _shape(c, kind, pts, width, color):
    e = _slot(c)
    p = list(pts) + [(0.0, 0.0)] * (3 - len(pts))
    e.set(Shape(kind, p[0][0], p[0][1], p[1][0], p[1][1], p[2][0], p[2][1], float(width), int(color)))
    return e


def draw_line(c, x0, y0, x1, y1, color, width=1.0):
    """A line of `width` units; its ends are square."""
    return _shape(c, 1, [(x0, y0), (x1, y1)], width, color)


def draw_triangle(c, a, b, d, color):
    """A filled triangle through three `(x, y)` points."""
    return _shape(c, 2, [a, b, d], 0.0, color)


def draw_ellipse(c, cx, cy, rx, ry, color):
    """A filled ellipse (a circle when the radii match)."""
    return _shape(c, 3, [(cx, cy), (rx, ry)], 0.0, color)


def draw_rect(c, x, y, w, h, color):
    """A filled rectangle, as two triangles."""
    draw_triangle(c, (x, y), (x + w, y), (x + w, y + h), color)
    return draw_triangle(c, (x, y), (x + w, y + h), (x, y + h), color)


def shape_count(c):
    """How many shapes this pass has drawn."""
    return c.get(Canvas).used


ui.canvas, ui.canvas_clear, ui.shape_count = canvas, canvas_clear, shape_count
ui.draw_line, ui.draw_triangle, ui.draw_ellipse, ui.draw_rect = draw_line, draw_triangle, draw_ellipse, draw_rect
ui.Canvas = Canvas


# ---- viewport ----------------------------------------------------------------------
def viewport(parent, source, width=None, height=None, **style):
    """An embedded view of a simulation. The host draws it: it gives the UI
    the simulation's frame each tick under the id `source`
    (`sage_ui::set_viewport_frame`) and reads the pointer back with
    `ui.viewport_pointer`. The view keeps the simulation's aspect ratio and
    letterboxes it. Click to focus it; Tab reaches it."""
    e = ui.add(ui.Panel, parent, width=width, height=height, **style)
    e.set(Widget(VIEWPORT, 1, 0, 0))
    e.set(Viewport(int(source), 0.0, 0.0, 0, 0))
    return e


def viewport_pointer(e):
    """`(x, y, over, buttons)`: the pointer in the simulation's world units
    (y up), whether it is over the view or dragging from it, and the mouse
    buttons held (1 primary, 2 secondary, 4 middle)."""
    v = e.get(Viewport)
    return v.x, v.y, bool(v.over), v.down


ui.viewport, ui.viewport_pointer = viewport, viewport_pointer
