# The editor's screen, part 3: the frame (bars, panes, tabs) and the dialogs
# and menus that are made once. Parts share one namespace; see base.py.

# ---- the frame of the screen ----------------------------------------------------------------
app = ui.add(T.Surface)
bar = ui.add(T.Bar, app, height=18)
bar_left = ui.add(T.Row, bar, width=None, gap=3)
# Editing or playing is the boundary that loses work in other editors: it is
# said in words, in a badge whose outline changes too, never by color alone.
mode_badge = ui.add(T.Badge, bar, text="Editing")
status = ui.add(T.Muted, bar, text="")
body = ui.add(T.Cols, app)
left = ui.add(T.Card, body, width=112, height=fill())
center = ui.add(T.Column, body, width=fill(), height=fill())
right = ui.add(T.Card, body, width=150, height=fill())

# top bar
b_undo = command_button(bar_left, "undo", width=34)
b_redo = command_button(bar_left, "redo", width=34)
b_play = command_button(bar_left, "play", "Play", T.Primary, width=40)
b_pause = command_button(bar_left, "pause", "Pause", width=38)
b_step = command_button(bar_left, "step", "Step", width=34)
b_reload = command_button(bar_left, "reload", "Reload", width=44)
b_save = command_button(bar_left, "save", "Save", width=34)

# left: the outliner
ui.add(T.Heading, left, text="Entities")
tools = ui.add(T.Row, left, gap=2)
command_button(tools, "add_entity", "Add", width=28)
command_button(tools, "copy_entity", "Copy", width=30)
command_button(tools, "delete_entity", "Del", T.Danger, width=28)
outliner = ui.scroll(left, direction=COLUMN, gap=1, width=fill(), height=fill())

# center: tabs
tabs_root, tab_buttons, tab_panels = ui.tabs(center, ["Scene", "Code", "Blocks", "Timing", "Prefabs"], width=fill(), height=fill())
scene_panel, code_panel, blocks_panel, timing_panel, prefab_panel = tab_panels
scene_head = ui.add(T.Row, scene_panel, gap=6)
scene_hint = ui.add(T.Muted, scene_head, text="Click to select. Drag to move.")
snap_toggle = ui.toggle(scene_head, text="Snap to Whole Units", on=editor.snap() > 0)
fixed_toggle[snap_toggle.sid] = lambda on: editor.set_snap(1.0 if on else 0.0)
scene_view = ui.viewport(scene_panel, 1, width=fill(), height=300)

# code tab
code_state = {"name": None, "synced": None}
code_head = ui.add(T.Row, code_panel, gap=3)
code_names = ui.add(T.Row, code_head, gap=2, width=None)
code_area = ui.text_area(code_panel, "", rows=22, width=fill(), prefab=T.Field)
code_error = ui.add(T.Para, code_panel, text="")


def apply_code():
    name = code_state["name"]
    if name is None:
        return
    text = text_of(code_area)
    err = editor.set_source(name, text, "Edit %s.py in Code" % name)
    code_state["synced"] = text
    if err:
        say(err)
    editor.request("reload")


apply_button = button(code_head, "Apply", apply_code, T.Primary, fixed=True, width=44)


def fixed_click_button(label):
    """A permanent button by its label (for tests)."""
    return {"Apply": apply_button}[label]

# inspector
ui.add(T.Heading, right, text="Inspector")
inspector = ui.scroll(right, direction=COLUMN, gap=3, width=fill(), height=fill())

# block editor panes
blk_head = ui.add(T.Row, blocks_panel, gap=3)
suggest_toggle = ui.toggle(blocks_panel, text="Suggest by Type", on=(editor.setting("suggest_by_type", "on") == "on"))
fixed_toggle[suggest_toggle.sid] = lambda on: (editor.set_setting("suggest_by_type", "on" if on else "off"), view_dirty.__setitem__(0, True))
pal_filter = ui.text_input(blocks_panel, "", placeholder="Filter blocks: Ctrl+B for blocks, Ctrl+E for values", prefab=T.Field, width=fill())
fixed_text[pal_filter.sid] = lambda: view_dirty.__setitem__(0, True)
blk_palette = ui.add(ui.Panel, blocks_panel, direction=COLUMN, gap=2, width=fill())
blk_canvas = ui.add(ui.Panel, blocks_panel, direction=COLUMN, gap=2, width=fill())
blk = {"cat": "Control", "fn": None, "sel": None, "src": None, "why": False, "mode": None, "focus": None, "entries": []}
cur = {"view": None}
glow = {"heads": {}, "ttl": {}, "lit": set()}  # statement heads by source line, and how long each has glowed
drag_now = {"entry": None}
notes_ui = {"items": [], "lines": None, "canvas": None, "field": None, "off": {}, "shown": None}
ctx = {"target": 0}
tower = {"kind": None}  # what is being carried: a chain of blocks, or a whole stack

# One shared right-click menu for blocks and stacks (the actions that used to
# be a row of tiny buttons on every block).
M_NOTE, M_DELETE, M_UP, M_DOWN, M_ELSE, M_RUN = 1, 2, 3, 4, 5, 6
block_menu = ui.context_menu(
    blk_canvas,
    [
        ("Add Note", {"id": M_NOTE, "shortcut": "Alt+N"}),
        ("Remove Note", {"id": 7}),
        "-",
        ("Move Up", {"id": M_UP, "shortcut": "Alt+Up"}),
        ("Move Down", {"id": M_DOWN, "shortcut": "Alt+Down"}),
        ("Add Else Branch", {"id": M_ELSE, "shortcut": "Alt+L"}),
        ("Run to Here", {"id": M_RUN}),
        "-",
        ("Delete Block", {"id": M_DELETE, "shortcut": "Del"}),
    ],
)

# The menu of a stack's hat. Deleting a stack does not ask: it is one step
# on the undo stack like any other edit, and the status line says so.
M_DELETE_STACK = 8
_stack_anchor = world.entity()
_stack_anchor.set(Rect())
stack_menu = ui.context_menu(
    _stack_anchor,
    [
        ("Add Note", {"id": M_NOTE}),
        ("Remove Note", {"id": 7}),
        "-",
        ("Delete Stack", {"id": M_DELETE_STACK}),
    ],
)


def delete_stack(f):
    name = f["name"]
    cur["view"].edit(lambda bb: bb.delete_function(name), "Delete stack " + name)()
    say("Deleted the stack %s. Undo (Ctrl+Z) brings it back." % name)
