# The editor's screen, part 2: every action is a named command.
#
# A button and a key chord run the same command, and a button's tooltip names
# its key, so the slow way to do something points at the fast way. The
# registry also searches (`cmds.search`, forgiving of typos and synonyms): a
# command palette over it is the next step and is not built yet. What a command does is defined in the part
# that owns it; it is looked up when the command runs, so the commands can
# all be listed here, in the order the palette shows them. Parts share one
# namespace.

import commands as command_search

cmds = command_search.Registry()
# Shortcut training (a project setting, off unless asked for): a button whose
# command has a key does not run it, it says the key. Hotkey labels next to
# menu items are mostly ignored; a menu that will not do the work is not.
training = {"on": editor.setting("shortcut_training", "off") == "on"}


def typing():
    """Whether the keyboard is in a text field (which has its own Undo and
    Delete)."""
    focused = world.lookup(world.get(Interaction).focus)
    return focused is not None and focused.has(TextField)


def run_command(id, via="palette"):
    """Runs a command by its id. `via` is the door it came through: "click"
    (a button), "key" (its chord) or "palette"."""
    c = cmds.get(id)
    if via == "click" and c.shortcut and training["on"]:
        say("Shortcut training: press %s for %s" % (c.shortcut, c.title))
        return
    said = status_note[0]
    why = cmds.run(id)
    if why:
        say(why)
    elif via == "palette" and c.shortcut and status_note[0] == said:
        say("%s is also %s" % (c.title, c.shortcut))
    view_dirty[0] = True


def command_button(parent, id, text=None, prefab=None, **style):
    """A permanent button that runs a command. Its tooltip says what it does
    and its key."""
    c = cmds.get(id)
    b = button(parent, text or c.title, lambda: run_command(id, "click"), prefab, fixed=True, **style)
    ui.tooltip(b, c.title + (" (%s)" % c.shortcut if c.shortcut else ""))
    return b


def bind_shortcuts():
    """Gives every command with a key its chord. Called once, after the
    screen is built: a chord is an entity of its own with nothing to click."""
    for c in cmds:
        if c.shortcut:
            key = ui.command(c.shortcut)
            fixed_click[key.sid] = (lambda id: lambda: run_command(id, "key"))(c.id)
            handles[("chord", c.id)] = key


def need_selection():
    return None if editor.selection() is not None else "Select an entity first"


def do_undo(redo=False):
    """One stack for the scene and the scripts: says what was taken back and
    shows the place it happened."""
    if typing():
        return  # the text field undoes its own typing
    done = editor.redo() if redo else editor.undo()
    if done is None:
        say("Nothing to redo" if redo else "Nothing to undo")
        return
    label, script = done
    say(("Redid: " if redo else "Undid: ") + label + (" (%s.py)" % script if script else ""))
    tab = ui.selected_tab(tabs_root)
    if script is None and tab != 0:
        ui.select_tab(tabs_root, 0)
    elif script is not None and tab == 0:
        blk["src"] = script
        ui.select_tab(tabs_root, 2)


def show_tab(i):
    ui.select_tab(tabs_root, i)
    view_dirty[0] = True


def toggle_training():
    training["on"] = not training["on"]
    editor.set_setting("shortcut_training", "on" if training["on"] else "off")
    say("Shortcut training is on: buttons that have a key name it instead of running" if training["on"] else "Shortcut training is off")


def toggle_snap():
    on = editor.snap() == 0
    editor.set_snap(1.0 if on else 0.0)
    snap_toggle.set(Toggle(1 if on else 0))
    say("Dragged entities land on whole units" if on else "Dragged entities land where you drop them")


_add = cmds.add
# The game
_add("play", "Play or Stop", lambda: editor.request("stop" if editor.playing() else "play"), "F5", "Top bar", ("run", "start", "test", "go", "end", "halt", "quit game"))
_add("pause", "Pause or Resume", lambda: editor.request("pause"), "F7", "Top bar", ("hold", "freeze", "continue", "unpause"), lambda: None if editor.playing() else "The game is not running")
_add("step", "Step One Tick", lambda: editor.request("step"), "F8", "Top bar", ("advance", "next tick", "frame", "single step"))
_add("reload", "Reload Scripts", lambda: editor.request("reload"), None, "Top bar", ("refresh", "restart", "rerun"))
# The project
_add("undo", "Undo", lambda: do_undo(), "Ctrl+Z", "Top bar", ("back", "revert", "take back", "oops"))
_add("redo", "Redo", lambda: do_undo(redo=True), "Ctrl+Y", "Top bar", ("again", "forward", "repeat"))
_add("save", "Save Project", lambda: editor.request("save"), "Ctrl+S", "Top bar", ("write", "store", "keep", "export"))
# Entities
_add("add_entity", "Add Entity", lambda: editor.spawn("entity", editor.selection()), None, "Entities", ("new", "create", "spawn", "make", "object", "thing"))
_add("copy_entity", "Duplicate Entity", lambda: editor.duplicate(editor.selection()), "Ctrl+D", "Entities: Copy", ("copy", "clone", "another"), need_selection)
_add("delete_entity", "Delete Entity", lambda: editor.despawn(editor.selection()), None, "Entities: Delete", ("remove", "erase", "kill", "destroy", "despawn"), need_selection)
_add("snap", "Snap to Whole Units", toggle_snap, None, "Scene", ("grid", "align", "pixel", "snapping"))
# Places
for _i, _name in enumerate(("Scene", "Code", "Blocks", "Timing", "Prefabs")):
    _add("tab_%d" % _i, "Show " + _name, (lambda i: lambda: show_tab(i))(_i), "Ctrl+%d" % (_i + 1), "Tabs", ("open", "go to", "view", "switch") + {"Timing": ("phases", "order", "when"), "Code": ("python", "text", "script"), "Blocks": ("rules", "systems", "stacks")}.get(_name, ()))
_add("panes", "Next Pane", lambda: next_pane(1), "F6", "Bottom row", ("focus", "outliner", "inspector"))
_add("panes_back", "Previous Pane", lambda: next_pane(-1), "Shift+F6", "Bottom row", ("focus",))
# Blocks
_add("find_block", "Find a Block", lambda: open_palette("block"), "Ctrl+B", "Blocks: the filter", ("insert", "add block", "palette", "statement"))
_add("find_value", "Find a Value", lambda: open_palette("expr"), "Ctrl+E", "Blocks: the filter", ("insert", "expression", "number", "palette"))
_add("new_system", "New System", lambda: new_system_now(), None, "Blocks", ("rule", "stack", "function", "add system", "create"))
_add("block_up", "Move Block Up", lambda: key_move(-1), "Alt+Up", "Blocks: right-click a block", ("earlier", "raise", "reorder"))
_add("block_down", "Move Block Down", lambda: key_move(1), "Alt+Down", "Blocks: right-click a block", ("later", "lower", "reorder"))
_add("delete_block", "Delete Block", lambda: key_delete(), "Delete", "Blocks: right-click a block", ("remove", "erase", "kill"))
_add("note", "Add or Remove a Note", lambda: key_note(), "Alt+N", "Blocks: right-click a block", ("comment", "sticky", "annotate", "explain"))
_add("else", "Add Else Branch", lambda: key_else(), "Alt+L", "Blocks: right-click an if block", ("otherwise", "elif"))
# The editor itself
_add("training", "Shortcut Training On or Off", toggle_training, None, "", ("learn keys", "hotkeys", "keyboard", "practice"))
