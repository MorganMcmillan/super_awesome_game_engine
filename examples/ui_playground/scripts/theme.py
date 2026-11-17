# The playground's theme: a palette and a set of style prefabs.
#
# Dark warm backgrounds, orange as the primary color. Every widget in the
# playground is an instance of one of these prefabs, and none of them
# overrides a style, so `theme.apply(color)` restyles the whole UI by editing
# the prefabs: style groups (Box, Flow, Paint, TextStyle) are inherited from
# the prefab until an instance sets its own copy.
#
#   kit prefab (sage_ui widgets.py)      theme prefab
#   Panel   ->  Surface, Bar, Cols, Column, Card, Row, Backdrop, Dialog
#   Text    ->  Title, Heading, Body, Muted, Para, Chip, Badge
#   Button  ->  Primary, Secondary, Ghost, Danger, Item, Swatch
#   Choice  ->  Tab (selected = the accent color, through a `checked` style variant)
#   Toggle  ->  Switch        Slider -> Track        Meter -> Gauge
#   TextInput -> Field

from sage.ui import Box, Decor, Flow, Paint, Screen, States, TextStyle, Widget
from sage import world

ui = world.ui
COLUMN, ROW = ui.COLUMN, ui.ROW
START, CENTER, END, STRETCH, BETWEEN = ui.START, ui.CENTER, ui.END, ui.STRETCH, ui.SPACE_BETWEEN
BLOCK = 5
_screen = world.get(Screen)  # set by the host before scripts load
SCREEN_W, SCREEN_H = _screen.w, _screen.h  # numbers, not the view

# ---- palette (0xRRGGBBAA) ---------------------------------------------------
BG0 = 0x0F0D0BFF  # the app: the darkest
BG1 = 0x1B1714FF  # cards and the title bar
BG2 = 0x2A241EFF  # controls
EDGE = 0x3B3229FF  # borders
TEXT = 0xF3EBE2FF
MUTED = 0x9A8D7EFF
INK = 0x1A1109FF  # text on a bright fill
DANGER = 0xD8453CFF
SHADE = 0x000000B8  # behind a dialog
ORANGE, TEAL, VIOLET = 0xF28C28FF, 0x2EC4B6FF, 0xA574FFFF
ACCENTS = {"ORANGE": ORANGE, "TEAL": TEAL, "VIOLET": VIOLET}


def _axis(size):
    if size is None:
        return 0, 0.0
    if isinstance(size, ui.fill):
        return 2, size.weight
    return 1, float(size)


def size(width=None, height=None):
    (wk, w), (hk, h) = _axis(width), _axis(height)
    return Box(wk, w, hk, h)


def make(name, base, *values):
    prefab = world.prefab(name, base=base)
    for value in values:
        prefab.set(value)
    return prefab


def flow(direction=COLUMN, align=START, justify=START, gap=0.0, pad=(0.0, 0.0, 0.0, 0.0)):
    return Flow(direction, align, justify, float(gap), *map(float, pad))


class Theme:
    def __init__(self):
        fill = ui.fill()
        # Layout.
        self.Surface = make("Surface", ui.Panel, size(fill, fill), flow(gap=6, pad=(6, 6, 6, 6)))
        self.Bar = make("Bar", ui.Panel, size(fill, None), flow(ROW, CENTER, BETWEEN, 12, (8, 2, 8, 2)))
        self.Cols = make("Cols", ui.Panel, size(fill, fill), flow(ROW, STRETCH, START, gap=6))
        self.Column = make("Column", ui.Panel, size(fill, fill), flow(COLUMN, STRETCH, START, gap=6))
        self.Card = make("Card", ui.Panel, size(fill, None), flow(COLUMN, STRETCH, START, 4, (6, 5, 6, 6)))
        self.Row = make("Row", ui.Panel, size(fill, None), flow(ROW, CENTER, START, 4))
        self.Chip = make("Chip", ui.Text, size(fill, 11), flow(COLUMN, START, START, 0, (3, 2, 3, 2)))
        self.Badge = make("Badge", ui.Text, size(None, None), flow(COLUMN, START, START, 0, (3, 1, 3, 1)))
        # The kit's popup prefabs are edited in place by `apply`: the kit
        # creates their instances itself (dialogs, tooltips, menus).
        # Text.
        self.Title = make("Title", ui.Text, size(None, None))
        self.Heading = make("Heading", ui.Text, size(None, None))
        self.Body = make("Body", ui.Text, size(None, None))
        self.Muted = make("Muted", ui.Text, size(None, None))
        self.Para = make("Para", ui.Text, size(fill, None))
        self.Rich = make("Rich", ui.RichText, size(fill, None))
        # Controls.
        for name in ("Primary", "Secondary", "Ghost", "Danger", "Swatch"):
            setattr(self, name, make(name, ui.Button, size(fill, None)))
        self.Item = make("Item", ui.Button, size(fill, None))
        self.Tab = make("Tab", ui.Choice, size(None, None))
        ui.role(self.Tab, "tab")
        ui.role(self.Title, "heading", level=1)
        ui.role(self.Heading, "heading", level=2)
        self.Switch = make("Switch", ui.Toggle)
        ui.role(self.Switch, "switch")
        self.Track = make("Track", ui.Slider)
        self.Gauge = make("Gauge", ui.Meter)
        self.Field = make("Field", ui.TextInput, size(None, 14))
        self.accents = ACCENTS
        self.apply(ORANGE)

    def apply(self, accent):
        """Restyles every instance of every theme prefab by editing the
        prefabs. Only colors change, so only Paint and TextStyle are set."""
        self.accent = accent

        def paint(prefab, fill, border=0, width=0.0, ring=accent):
            prefab.set(Paint(fill, border, float(width), ring))

        def text(prefab, color, scale=1.0, wrap=0):
            prefab.set(TextStyle(color, float(scale), wrap))

        clear = 0x00000000
        paint(self.Surface, BG0)
        paint(self.Bar, BG1, EDGE, 1)
        for plain in (self.Cols, self.Column, self.Row):
            paint(plain, clear)
        paint(self.Card, BG1, EDGE, 1)
        paint(self.Chip, BG2, EDGE, 1)
        paint(self.Badge, accent, ring=TEXT)
        # Kit prefabs: popups, menus, drop-downs, dialogs.
        ui.Popup.set(Paint(BG1, accent, 1.0, accent))
        ui.Tip.set(Paint(BG2, EDGE, 1.0, accent))
        ui.Tip.set(TextStyle(TEXT, 1.0, 0))
        ui.Backdrop.set(Paint(SHADE, 0, 0.0, accent))
        ui.Dialog.set(Paint(BG1, accent, 1.0, accent))
        ui.Dropdown.set(Paint(BG2, EDGE, 1.0, accent))
        ui.Dropdown.set(TextStyle(TEXT, 1.0, 0))
        ui.Option.set(Paint(0, 0, 0.0, accent))
        ui.Option.set(TextStyle(TEXT, 1.0, 0))
        ui.Option.set(States(hover_fill=BG2, hover_text=accent, pressed_fill=EDGE))
        ui.MenuItem.set(Paint(0, 0, 0.0, accent))
        ui.MenuItem.set(TextStyle(TEXT, 1.0, 0))
        ui.MenuItem.set(States(hover_fill=BG2, hover_text=accent, pressed_fill=EDGE, disabled_text=MUTED))
        ui.MenuTitle.set(Paint(0, 0, 0.0, accent))
        ui.MenuTitle.set(TextStyle(TEXT, 1.0, 0))
        ui.MenuTitle.set(States(hover_fill=BG2, hover_text=accent, pressed_fill=EDGE))
        ui.MenuBar.set(Paint(BG1, EDGE, 1.0, accent))
        ui.Sep.set(Paint(EDGE, 0, 0.0, accent))
        for bar in (ui.Scrubber, ui.Volume, ui.HueSlider, ui.AlphaSlider):
            bar.set(Paint(BG2, EDGE, 1.0, accent))
        for button in (ui.PlayButton, ui.MonthButton):
            button.set(Paint(BG2, EDGE, 1.0, accent))
            button.set(TextStyle(TEXT, 1.0, 0))
            button.set(States(hover_fill=EDGE, hover_text=accent, pressed_fill=BG0))
        ui.DayCell.set(Paint(BG2, 0, 0.0, accent))
        ui.DayCell.set(TextStyle(TEXT, 1.0, 0))
        ui.DayCell.set(States(hover_fill=EDGE, checked_fill=accent, checked_text=INK))
        ui.Swatch.set(Paint(accent, EDGE, 1.0, accent))
        ui.SortRow.set(Paint(BG2, EDGE, 1.0, accent))
        ui.SortRow.set(TextStyle(TEXT, 1.0, 0))
        ui.ResizeGrip.set(Paint(accent, 0, 0.0, accent))
        ui.DropBar.set(Paint(accent, 0, 0.0, accent))
        ui.TabButton.set(Paint(BG2, EDGE, 1.0, accent))
        ui.TabButton.set(TextStyle(MUTED, 1.0, 0))
        ui.TabButton.set(States(checked_fill=accent, checked_text=INK, hover_text=TEXT))
        ui.TabClose.set(Paint(BG2, EDGE, 1.0, accent))
        ui.TabClose.set(TextStyle(MUTED, 1.0, 0))
        ui.TabClose.set(States(hover_fill=DANGER, hover_text=TEXT))
        ui.SplitHandle.set(Paint(EDGE, 0, 0.0, accent))
        ui.SplitHandle.set(States(hover_fill=accent, pressed_fill=accent))
        ui.DetailsHead.set(Paint(0, 0, 0.0, accent))
        ui.DetailsHead.set(TextStyle(accent, 1.0, 0))
        ui.DetailsHead.set(States(hover_fill=BG2, hover_text=TEXT))
        ui.TreeLabel.set(Paint(0, 0, 0.0, accent))
        ui.TreeLabel.set(TextStyle(TEXT, 1.0, 0))
        ui.TreeLabel.set(States(hover_fill=BG2, checked_fill=accent, checked_text=INK))
        ui.TreeArrow.set(Paint(0, 0, 0.0, accent))
        ui.TreeArrow.set(TextStyle(accent, 1.0, 0))
        ui.RadioButton.set(Paint(BG0, EDGE, 1.0, accent))
        ui.RadioButton.set(Decor(radius=5.0))
        ui.RadioButton.set(States(checked_fill=accent, hover_border=accent))
        ui.LabelText.set(TextStyle(TEXT, 1.0, 0))
        ui.FormError.set(TextStyle(DANGER, 1.0, 0))
        ui.TableHeader.set(Paint(BG2, EDGE, 1.0, accent))
        ui.TableHeader.set(TextStyle(accent, 1.0, 0))
        ui.TableHeader.set(States(hover_fill=EDGE, pressed_fill=BG0))
        ui.TableCell.set(Paint(0, 0, 0.0, accent))
        ui.TableCell.set(TextStyle(TEXT, 1.0, 0))
        ui.TableCell.set(States(hover_fill=BG2, checked_fill=accent, checked_text=INK))
        paint(self.Primary, accent, ring=TEXT)
        paint(self.Secondary, BG2, EDGE, 1)
        paint(self.Ghost, clear, accent, 1)
        paint(self.Danger, DANGER, ring=TEXT)
        paint(self.Swatch, BG2, ring=TEXT)
        paint(self.Item, BG2)
        paint(self.Switch, BG2, EDGE, 1)
        paint(self.Track, BG2)
        paint(self.Gauge, BG2)
        paint(self.Field, BG0, EDGE, 1)
        paint(self.Rich, 0, 0, 0)

        text(self.Title, TEXT, 2)
        text(self.Heading, accent)
        text(self.Body, TEXT)
        text(self.Muted, MUTED)
        text(self.Para, TEXT, wrap=1)
        text(self.Rich, TEXT, wrap=1)
        text(ui.H1, TEXT, 3)
        text(ui.H2, accent, 2)
        text(ui.H3, TEXT, 1)
        text(self.Chip, TEXT)
        text(self.Badge, INK)
        text(self.Primary, INK)
        text(self.Secondary, TEXT)
        text(self.Ghost, accent)
        text(self.Danger, TEXT)
        text(self.Swatch, INK)
        text(self.Item, TEXT)
        paint(self.Tab, BG2, EDGE, 1)
        text(self.Tab, MUTED)
        self.Tab.set(States(checked_fill=accent, checked_text=INK, hover_text=TEXT))
        text(self.Field, TEXT)


world.theme = Theme()
