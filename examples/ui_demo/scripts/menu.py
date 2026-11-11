# A settings menu from the UI module's widgets. Every widget is an entity;
# the names below let the host find them without asking Python.

from sage.ui import Changed, Clicked, Label, Toggled

ui = world.ui
ui.start()

panel = ui.panel(
    width=148, padding=8, gap=5, fill=0x1B1F27FF, border=0x3A4150FF, border_width=1, name="panel"
)
ui.text(panel, "SETTINGS", scale=2, color=0xE8ECF4FF)
play = ui.button(panel, "Play", width=ui.fill(), name="play")
music = ui.toggle(panel, "Music", on=False, name="music")
ui.text(panel, "Volume", color=0xA8B0C0FF)
volume = ui.slider(panel, value=3, low=0, high=10, step=1, name="volume")
ui.text(panel, "Name", color=0xA8B0C0FF)
name = ui.text_input(panel, "", width=ui.fill(), name="name")
status = ui.text(panel, "Ready", color=0x6AA9FFFF, name="status")


def say(message):
    status.set(Label(world.intern(message), 0x6AA9FFFF, 1.0))


@world.observer(Clicked)
def clicked(c: Clicked):
    if c.target == play.sid:
        say("Starting...")


@world.observer(Toggled)
def toggled(t: Toggled):
    if t.target == music.sid:
        say("Music on" if t.on else "Music off")


@world.observer(Changed)
def changed(c: Changed):
    if c.target == volume.sid:
        say(f"Volume {int(c.value)}")
