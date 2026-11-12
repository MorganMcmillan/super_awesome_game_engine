# Page: TEXT. Text fields with selection, clipboard and undo; a password; a
# text area that wraps and scrolls; number fields.

import sage
from sage.ui import Caret, Changed, Label, Navigate, Slider, Submitted, TextChanged, TextField

ui = world.ui
T = world.theme
text_page = world.add_page("TEXT")


def tcard(parent, title):
    c = ui.add(T.Card, parent)
    ui.add(T.Heading, c, text=title)
    return c


def say(entity, text):
    entity.set(Label(world.intern(text)))


tcol_a, tcol_b, tcol_c = (ui.add(T.Column, text_page) for _ in range(3))

# -- rich text -------------------------------------------------------------------------
rich_card = tcard(tcol_b, "RICH TEXT")
ui.heading(rich_card, "TITLE", 1)
ui.heading(rich_card, "SECTION", 2)
ui.heading(rich_card, "SUBSECTION", 3)
paragraph = ui.rich(
    rich_card,
    "A PARAGRAPH CAN BE **BOLD**, HAVE `CODE`, {#4FC3C0 TINTED} AND *SLANTED* WORDS AND A [LINK](PAGE:TEXT) IN IT.",
    prefab=T.Rich,
)
link_said = ui.add(T.Body, rich_card, text="CLICKED: NOTHING")

@world.observer(Navigate)
def text_page_navigate(n: Navigate):
    say(link_said, "CLICKED: " + world.string(n.link).upper())


# -- single-line fields ----------------------------------------------------------------
lines = tcard(tcol_a, "TEXT FIELD")
ui.add(T.Muted, lines, text="SHIFT+ARROWS OR DRAG SELECT. CTRL+A C X V Z Y", wrap=True, width=ui.fill())
email_field = ui.text_input(lines, "", placeholder="NAME OR EMAIL", prefab=T.Field, max_len=40)
email_said = ui.add(T.Body, lines, text="LENGTH: 0")
short_field = ui.text_input(lines, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", prefab=T.Field, width=60)
ui.add(T.Muted, lines, text="A LONG LINE SCROLLS SIDEWAYS", wrap=True, width=ui.fill())

secret = tcard(tcol_a, "PASSWORD")
password_field = ui.text_input(secret, "", placeholder="PASSWORD", prefab=T.Field, password=True)
password_said = ui.add(T.Body, secret, text="TYPED: 0")
ui.add(T.Muted, secret, text="NO COPY, NO CUT", wrap=True, width=ui.fill())

# -- text area ------------------------------------------------------------------------
area_card = tcard(tcol_c, "TEXT AREA")
notes_area = ui.text_area(
    area_card,
    "A TEXT AREA WRAPS LONG LINES AT THE WORD. ENTER ADDS A LINE.\nUP AND DOWN MOVE BY ROW. THE WHEEL SCROLLS.",
    placeholder="NOTES",
    prefab=T.Field,
    rows=8,
)
area_said = ui.add(T.Body, area_card, text="LINES: 2")

# -- numbers ---------------------------------------------------------------------------
numbers = tcard(tcol_b, "NUMBERS")
ui.add(T.Muted, numbers, text="TYPE, UP/DOWN OR WHEEL. ENTER SETTLES", wrap=True, width=ui.fill())
row = ui.add(T.Row, numbers)
ui.add(T.Body, row, text="COUNT")
count_field = ui.number(row, 3, low=0, high=10, step=1, prefab=T.Field, width=50)
row = ui.add(T.Row, numbers)
ui.add(T.Body, row, text="SPEED")
speed_field = ui.number(row, 1.5, low=0, high=5, step=0.25, prefab=T.Field, width=50)
number_said = ui.add(T.Body, numbers, text="COUNT 3  SPEED 1.5")


def text_of_label(e):
    return world.string(e.get(Label).text)


@world.observer(TextChanged)
def text_page_changed(t: TextChanged):
    if t.target == email_field.sid:
        say(email_said, "LENGTH: %d" % len(text_of_label(email_field)))
    elif t.target == password_field.sid:
        say(password_said, "TYPED: %d" % len(text_of_label(password_field)))
    elif t.target == notes_area.sid:
        say(area_said, "LINES: %d" % (text_of_label(notes_area).count("\n") + 1))


@world.observer(Changed)
def text_page_numbers(c: Changed):
    if c.target in (count_field.sid, speed_field.sid):
        say(
            number_said,
            "COUNT %g  SPEED %g" % (count_field.get(Slider).value, speed_field.get(Slider).value),
        )
