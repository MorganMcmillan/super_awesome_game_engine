# SVG documents (see `sage_ui::svg` for what is drawn: paths with every
# command including arcs, basic shapes, groups, transforms, fill, stroke and
# opacity). The document is parsed and drawn by the UI crate; this is just
# the widget.

from sage.ui import Svg

ui = world.ui


def svg(parent, markup, alt=None, **style):
    """Draws an SVG document fitted into a box (give `width` and `height`; with
    none it takes the document's own size). `alt` is what a screen reader says
    for it. A document that does not parse draws a magenta box."""
    e = ui.add(ui.Panel, parent, **style)
    e.set(Svg(world.intern(markup)))
    if alt is not None:
        ui.role(e, "img", name=alt, hidden=(alt == ""))
    return e


ui.svg = svg
