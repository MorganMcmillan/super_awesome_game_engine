# Page: PICKERS. A color picker, a date field with a calendar, a time field
# and a file field with an in-engine browser. Each opens a popup, keeps its
# value in a component, and emits an event when it changes.

import sage
from sage.ui import Label

ui = world.ui
T = world.theme
pickers_page = world.add_page("PICKERS")


def kcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def ksay(entity, text):
    entity.set(Label(world.intern(text)))


kcol_a, kcol_b = (ui.add(T.Column, pickers_page) for _ in range(2))

# -- color --------------------------------------------------------------------------------
colorcard = kcard(kcol_a, "COLOR")
ui.add(T.Muted, colorcard, text="DRAG THE SQUARE, SLIDE THE HUE AND ALPHA, OR TYPE HEX.", wrap=True, width=ui.fill())
color_pick = ui.color_field(colorcard, 0xF28C28FF, width=ui.fill())
picked_color = ui.add(T.Body, colorcard, text="COLOR: F28C28")


@world.observer(ui.ColorChanged)
def pickers_color(c: ui.ColorChanged):
    ksay(picked_color, "COLOR: %08X" % c.color)


# -- date and time -----------------------------------------------------------------------------
datecard = kcard(kcol_a, "DATE AND TIME")
date_pick = ui.date_field(datecard, 2026, 11, 11, width=ui.fill())
date_said = ui.add(T.Body, datecard, text="DATE: 2026-11-11")
time_pick = ui.time_field(datecard, 9, 30)
time_said = ui.add(T.Body, datecard, text="TIME: 09:30")


@world.observer(ui.DatePicked)
def pickers_date(d: ui.DatePicked):
    ksay(date_said, "DATE: %04d-%02d-%02d" % (d.year, d.month, d.day))


@world.observer(ui.TimePicked)
def pickers_time(t: ui.TimePicked):
    ksay(time_said, "TIME: %02d:%02d" % (t.hour, t.minute))


# -- files ---------------------------------------------------------------------------------
filecard = kcard(kcol_b, "FILE")
ui.add(T.Muted, filecard, text="THE BROWSER IS A TREE OF PATHS (HERE, THE ASSET CATALOG).", wrap=True, width=ui.fill())
FILES = ["art/logo.png", "art/spinner.png", "art/checker.png", "sounds/hit.wav", "sounds/music/theme.wav", "README.txt"]
file_pick = ui.file_field(filecard, FILES, width=ui.fill())
file_said = ui.add(T.Body, filecard, text="FILE: NONE")
ui.add(T.Muted, filecard, text="OR ASK THE SYSTEM FOR ITS OWN DIALOG:", wrap=True, width=ui.fill())
native_pick = ui.file_field(filecard, native=True, width=ui.fill())
native_said = ui.add(T.Body, filecard, text="SYSTEM FILE: NONE", wrap=True, width=ui.fill())


@world.observer(ui.FilePicked)
def pickers_file(f: ui.FilePicked):
    if f.field == native_pick.sid:
        ksay(native_said, "SYSTEM FILE: " + world.string(f.path))
    else:
        ksay(file_said, "FILE: " + world.string(f.path).upper())
