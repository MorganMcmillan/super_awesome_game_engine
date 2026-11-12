# Landmarks and groups for screen readers. They are ordinary panels with a
# role: no new layout. (`ui.role` itself, which every kit uses, is in the
# widget kit.) Read what a screen reader would be told with
# `sage_ui::access_outline`.

ui = world.ui


def landmark(parent, role, name=None, **style):
    """A panel with a landmark role (`"navigation"`, `"banner"`,
    `"contentinfo"`, `"main"`, `"region"`...). Give `name` when there are
    several of one kind."""
    return ui.role(ui.add(ui.Panel, parent, **style), role, name=name)


def nav(parent, name=None, **style):
    """`<nav>`: a set of links to other places."""
    return landmark(parent, "navigation", name, **style)


def header(parent, name=None, **style):
    """`<header>` of a page: its banner."""
    return landmark(parent, "banner", name, **style)


def footer(parent, name=None, **style):
    """`<footer>` of a page: its content information."""
    return landmark(parent, "contentinfo", name, **style)


def main(parent, name=None, **style):
    """`<main>`: the page's central content. Use one."""
    return landmark(parent, "main", name, **style)


def section(parent, name, **style):
    """`<section>`: a region the reader can jump to; it needs a name."""
    return landmark(parent, "region", name, **style)


def separator(parent, vertical=False, **style):
    """`<hr>`: a thin rule between things."""
    size = {"width": 1, "height": ui.fill()} if vertical else {"width": ui.fill(), "height": 1}
    size.update(style)
    return ui.role(ui.add(ui.Sep, parent, **size), "separator")


def fieldset(parent, legend, **style):
    """`<fieldset>` and `<legend>`: a labelled group of controls. Returns
    `(root, body)`; put the controls in `body`."""
    root = ui.role(ui.add(ui.Panel, parent, direction=ui.COLUMN, gap=2, **style), "group")
    title = ui.add(ui.Text, root, text=legend)
    ui.role(root, None, labelled_by=title)
    ui.role(title, "text", hidden=True)
    body = ui.add(ui.Panel, root, direction=ui.COLUMN, gap=3, width=ui.fill())
    return root, body


ui.landmark, ui.nav, ui.header, ui.footer = landmark, nav, header, footer
ui.main, ui.section, ui.separator, ui.fieldset = main, section, separator, fieldset
