# Page: STYLE. Images, 9-slice, rounded corners, shadows, gradients, fade,
# display, state variants and transitions.

import sage
from sage import component
from sage.ui import Changed, Decor, Display, Image, Paint, Shown, Slider, Toggle, Toggled

ui = world.ui
T = world.theme
page = world.add_page("STYLE")
style_root = page


@component
class Sheet:
    """A sprite-sheet player: the frame advances every `every` ticks."""

    every: sage.u32 = 10
    ticks: sage.u32 = 0


def card(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def set_decor(entity, **fields):
    d = entity.get(Decor)
    d = Decor() if d is None else d.copy()
    for key, value in fields.items():
        setattr(d, key, value)
    entity.set(d)


col_a, col_b, col_c = (ui.add(T.Column, page) for _ in range(3))

# -- images ------------------------------------------------------------------------
images = card(col_a, "IMAGES")
row = ui.add(T.Row, images)
logo = ui.image(row, "art/logo.png")
ui.add(T.Muted, row, text="NATURAL SIZE")
ui.add(T.Muted, images, text="FIT IN A 40 X 28 BOX")
row = ui.add(T.Row, images, justify=ui.SPACE_BETWEEN)
for mode in ("contain", "cover", "none"):
    box = ui.add(T.Surface, row, width=40, height=28)
    ui.image(box, "art/logo.png", width=40, height=28, fit=mode)
labels = ui.add(T.Row, images, justify=ui.SPACE_BETWEEN)
for mode in ("CONTAIN", "COVER", "NONE"):
    ui.add(T.Muted, labels, text=mode, width=40)
row = ui.add(T.Row, images)
spinner = ui.image(row, "art/spinner.png", cols=4, rows=1)
spinner.set(Sheet(8, 0))
ui.add(T.Body, row, text="SPRITE SHEET, 4 FRAMES")
ui.add(T.Muted, images, text="TINT")
row = ui.add(T.Row, images)
for tint in (0xFFFFFFFF, 0xF28C28FF, 0x2EC4B6FF, 0xA574FFFF):
    ui.image(row, "art/checker.png", width=24, height=16, tint=tint)

# -- boxes ---------------------------------------------------------------------------
boxes = card(col_b, "BOXES")
rounded = ui.add(T.Primary, boxes, text="ROUNDED, BORDER", radius=6, border=0xFFFFFFFF, border_width=2)
shadowed = ui.add(T.Secondary, boxes, text="SHADOW", radius=4, shadow=(5, 2, 0x000000C0))
gradient = ui.panel(
    boxes, width=ui.fill(), height=18, gradient=(0x2EC4B6FF, "across"), fill=0xF28C28FF
)
ui.add(T.Muted, gradient, text="GRADIENT", color=0x1A1109FF)

framed = ui.panel(boxes, width=ui.fill(), height=26, slice=("art/frame.png", (4, 4, 4, 4)), padding=(6, 8))
ui.add(T.Body, framed, text="9-SLICE FRAME")

fading = card(col_b, "FADE AND DISPLAY")
fade_group = ui.panel(fading, width=ui.fill(), height=32, fill=0x2A241EFF, padding=4, fade=0.0)
ui.add(T.Primary, fade_group, text="CHILDREN FADE TOO", width=ui.fill())
ui.add(T.Muted, fading, text="FADE")
fade_slider = ui.add(T.Track, fading)
fade_slider.set(Slider(0.0, 0.0, 10.0, 1.0))
show_extra = ui.add(T.Row, fading)
extra_box = ui.add(T.Switch, show_extra)
extra_box.set(Toggle(0))
ui.add(T.Body, show_extra, text="SHOW THE NOTE")
note = ui.add(T.Para, fading, text="THIS NOTE USES DISPLAY NONE: HIDDEN, IT TAKES NO SPACE.", display=False)

# -- states ---------------------------------------------------------------------------
states = card(col_c, "STATE VARIANTS")
ui.add(T.Muted, states, text="HOVER: EASING DIFFERS")
speeds = {}
for label, ticks in (("SLOW 16", 16), ("MEDIUM 6", 6), ("FAST 2", 2), ("INSTANT", 0)):
    b = ui.add(
        T.Secondary,
        states,
        text=label,
        transition=ticks,
        states={"hover": {"fill": 0xF28C28FF, "text": 0x1A1109FF}, "pressed": {"fill": 0xD8453CFF}},
    )
    speeds[label] = b
disabled_state = ui.add(
    T.Secondary, states, text="DISABLED", enabled=False, states={"disabled": {"fill": 0x1B1714FF, "text": 0x5A4F43FF}}
)

choices = card(col_c, "CHOICES")
ui.add(T.Muted, choices, text="A CHECKED VARIANT")
row = ui.add(T.Row, choices)
choice_a = ui.choice(row, "ONE", on=True, states={"checked": {"fill": 0xF28C28FF, "text": 0x1A1109FF}})
choice_b = ui.choice(row, "TWO", states={"checked": {"fill": 0x2EC4B6FF, "text": 0x1A1109FF}})
choice_c = ui.choice(row, "THREE", states={"checked": {"fill": 0xA574FFFF, "text": 0x1A1109FF}})


@world.observer(Changed)
def style_changed(c: Changed):
    if c.target == fade_slider.sid:
        set_decor(fade_group, fade=c.value / 10.0)


@world.observer(Toggled)
def style_toggled(t: Toggled):
    if t.target == extra_box.sid:
        note.set(Display(0 if t.on else 1))


@world.system(phase=sage.PreUpdate, query="Sheet")
def play_sheet(img: Image, s: Sheet):
    s.ticks += 1
    img.frame = (s.ticks // s.every) % 4
