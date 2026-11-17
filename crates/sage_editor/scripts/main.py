# The editor's screen, part 11: what runs every tick. Glow, rebuilding the
# views that changed, the keys. Parts share one namespace.

def glow_tick():
    """Blocks glow while the game runs their lines (design doc 10.7 item 5)."""
    for line in B.glow_take():
        glow["ttl"][line] = 10
    for line in list(glow["ttl"]):
        t = glow["ttl"][line] - 1
        entry = glow["heads"].get(line)
        if entry is not None:
            head, color = entry
            if t > 0 and line not in glow["lit"]:
                head.set(Paint(color, 0xFFFFFFFF, 3.0, 0xFFFFFFFF))
                glow["lit"].add(line)
            elif t <= 0 and line in glow["lit"]:
                head.set(Paint(color, 0, 0.0, 0xFFFFFFFF))
                glow["lit"].discard(line)
        if t <= 0:
            del glow["ttl"][line]
        else:
            glow["ttl"][line] = t


def prune():
    """Forgets handlers and handles of widgets that no longer exist (a view
    was rebuilt), so the tables stay as small as what is on screen."""
    for table in (on_click, on_change, on_submit, on_toggle, on_select, dnd, move_grips):
        for sid in [k for k in table if world.lookup(k) is None]:
            del table[sid]
    for key in [k for k, e in handles.items() if getattr(e, "sid", None) is None or world.lookup(e.sid) is None]:
        del handles[key]


# Views are rebuilt only where they are seen: the status, outliner and
# inspector always, and the center tab that is showing. A tab that was not
# showing when something changed is rebuilt the moment it is opened.
TAB_BUILDERS = {1: build_code, 2: build_blocks, 3: build_timing, 4: build_prefabs}
built = {}
stamp = [0]


def rebuild_changed(changed):
    if changed:
        stamp[0] += 1
        build_status()
        build_outliner()
        build_inspector()
    tab = ui.selected_tab(tabs_root)
    builder = TAB_BUILDERS.get(tab)
    if builder is not None and (changed or built.get(tab) != stamp[0]):
        builder()
        built[tab] = stamp[0]
    prune()


def rebuild_all():
    """Rebuilds every view now (tests and scripts)."""
    stamp[0] += 1
    build_status()
    build_outliner()
    build_inspector()
    for tab, builder in TAB_BUILDERS.items():
        builder()
        built[tab] = stamp[0]
    prune()


def selected_stmt():
    """`(view, block, parent, field, index)` for the selected statement."""
    view = cur["view"]
    sel = blk["sel"]
    if view is None or sel is None or sel[0] != "s":
        return None
    try:
        block, parent, field = view.b.find(sel[2])
    except ValueError:
        return None
    if parent is None or block["k"] not in B.STATEMENTS:
        return None
    return view, block, parent, field, parent[field].index(block)


def key_move(delta):
    sel = selected_stmt()
    if sel is None:
        say("Select a block first")
        return
    view, block, parent, field, i = sel
    count = len(parent[field])
    to = i - 1 if delta < 0 else i + 2
    if (delta < 0 and i == 0) or (delta > 0 and i >= count - 1):
        return
    view.edit(lambda bb: bb.move(block["id"], parent["id"], field, to), "Move block up" if delta < 0 else "Move block down")()


def key_delete():
    # Delete also edits text: a block is only deleted when focus is not in a
    # text field.
    fe = world.lookup(world.get(Interaction).focus)
    if fe is not None and fe.has(TextField):
        return
    sel = selected_stmt()
    if sel is None:
        say("Select a block first")
        return
    view, block = sel[0], sel[1]
    view.edit(lambda bb: bb.delete(block["id"]), "Delete block")()


def key_note():
    sel = selected_stmt()
    if sel is None:
        say("Select a block first")
        return
    view, block = sel[0], sel[1]
    view.edit(lambda bb: bb.set_notes(block["id"], [] if block.get("notes") else ["Note"]))()


def key_else():
    sel = selected_stmt()
    if sel is None or sel[1]["k"] != "if":
        say("Select an if block first")
        return
    view, block = sel[0], sel[1]
    view.edit(lambda bb: bb.add_else(block["id"]))()


def open_palette(mode):
    ui.select_tab(tabs_root, 2)
    blk["mode"] = mode
    set_focus(pal_filter)
    view_dirty[0] = True


def insert_first_match():
    if blk["entries"]:
        insert_from_palette(blk["entries"][0])
        blk["mode"] = None
        pal_filter.set(Label(0))


fixed_submit[pal_filter.sid] = insert_first_match


def leave_palette():
    blk["mode"] = None
    blk["sel"] = None if blk["sel"] and blk["sel"][0] == "b" else blk["sel"]
    pal_filter.set(Label(0))
    view_dirty[0] = True


panes = [0]


def next_pane(step=1):
    """F6 and Shift+F6 move between the outliner, the middle and the inspector."""
    panes[0] = (panes[0] + step) % 3
    targets = [outliner.children[0] if list(outliner.children) else None, tab_buttons[0], inspector.children[0] if list(inspector.children) else None]
    t = targets[panes[0]]
    if t is not None:
        set_focus(t)


def new_system_now():
    make = blk.get("new_system")
    if ui.selected_tab(tabs_root) != 2 or make is None:
        ui.select_tab(tabs_root, 2)
        say("Blocks is open: New System adds a rule here")
        return
    make()


# The block keys, as real buttons so each can be seen, tabbed to and pressed.
# Each runs a command; its chord is the command's (see commands.py).
keys = ui.add(T.Row, app, gap=2)
for _label, _id in (
    ("Blocks", "find_block"),
    ("Values", "find_value"),
    ("Up", "block_up"),
    ("Down", "block_down"),
    ("Delete", "delete_block"),
    ("Note", "note"),
    ("Else", "else"),
    ("Panes", "panes"),
    ("Back", "panes_back"),
):
    _chord = cmds.get(_id).shortcut
    _text = "%s %s" % (_label, "Del" if _chord == "Delete" else _chord)
    handles[("key", _chord)] = command_button(keys, _id, _text, T.Item, width=6 * len(_text) + 8)
bind_shortcuts()


def live_tick():
    """While the game runs, the inspector's values are the running world's:
    they are refreshed in place a few times a second (never rebuilt under
    the pointer), and entities the game made or destroyed redraw the lists."""
    if not editor.playing() or ticks[0] % 6:
        return
    info = editor.inspect_live()
    if (info is None) != live_view["gone"]:
        view_dirty[0] = True
    elif info is not None:
        for path, _, fields in info[2]:
            for fname, _, text in fields:
                row = live_view["rows"].get((path, fname))
                if row is None:
                    view_dirty[0] = True  # the game gave it a component
                    continue
                value, was, authored = row
                if world.lookup(value.sid) is None:
                    continue
                set_text(value, text)
                set_text(was, "" if text == authored else "authored " + authored)
    spawned = tuple(sid for sid, _, _ in editor.runtime_only())
    if spawned != live_view["spawned"]:
        view_dirty[0] = True


every_tick.append(live_tick)


def refresh_views():
    """Called by the host after every UI tick, outside the systems: views are
    rebuilt after the tick's events were delivered, so an observer never
    changes the widgets it is reacting to."""
    ticks[0] += 1
    nav = world.get(Nav)
    if nav is not None and nav.bits & NAV_CANCEL and (blk["sel"] is not None or blk["mode"] is not None):
        blk["sel"] = None
        leave_palette()
    glow_tick()
    follow_pointer()
    notes_tick()
    timing_hover()
    for tick in every_tick:
        tick()
    rev = editor.revision()
    changed = rev != seen[0] or view_dirty[0]
    seen[0] = rev
    view_dirty[0] = False
    rebuild_changed(changed)
    if changed:
        status_note[0] = ""
