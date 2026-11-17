# The editor's screen, part 4: the status line, outliner, inspector and code
# tab, rebuilt when the document changes. Parts share one namespace.

import provenance

# ---- rebuilding the views -------------------------------------------------------------------
ACCENT, DANGER, PAPER = 0xF28C28FF, 0xD8453CFF, 0xF3EBE2FF
# The three modes, each with its own words and its own outline (a filled
# badge, an outlined one, a plain one), so the mode never rests on color.
MODES = {
    "edit": ("Editing", Paint(0x2A241EFF, 0x6A6258FF, 1.0, PAPER), "Click to select. Drag to move."),
    "play": ("Playing", Paint(ACCENT, ACCENT, 1.0, 0x1A1109FF), "The game is running: this is the live world. Stop to edit."),
    "hold": ("Paused", Paint(0x2A241EFF, ACCENT, 2.0, PAPER), "The game is held at this tick. Step advances it; Stop to edit."),
}


def mode():
    if not editor.playing():
        return "edit"
    return "hold" if editor.paused() else "play"


def build_status():
    word, paint, hint = MODES[mode()]
    mode_badge.set(paint)
    set_text(mode_badge, word)
    set_text(scene_hint, hint)
    bits = ["Tick %d" % editor.tick()] if editor.playing() else []
    bits.append("Unsaved" if editor.dirty() else "Saved")
    err = editor.error()
    note = status_note[0] or editor.log()
    if err:
        note = "Error: " + err.split("\n")[-1]
    fitted(status, "  ".join(bits) + ("  " + note if note else ""), 90)
    set_text(b_play, "Stop" if editor.playing() else "Play")
    set_text(b_pause, "Resume" if editor.paused() else "Pause")


def build_outliner():
    clear(outliner)
    sel = editor.selection()
    for sid, depth, label in editor.outliner():
        row = fitted(ui.add(T.Tab, outliner, text=""), ("  " * depth) + label, 22)
        row.set(Toggle(1 if sid == sel else 0))
        on_toggle[row.sid] = (lambda s: lambda on: (editor.select(s), view_dirty.__setitem__(0, True)))(sid)
    # What the running game made: listed, marked, and not authored.
    spawned = editor.runtime_only()
    live_view["spawned"] = tuple(sid for sid, _, _ in spawned)
    if spawned:
        ui.add(T.Muted, outliner, text="Made by the game (gone on Stop):", wrap=True, width=fill())
        for sid, depth, label in spawned:
            fitted(ui.add(T.Muted, outliner, text=""), ("  " * depth) + label + " (runtime)", 26)


handles = {}  # entities by role, for tests and scripts: ("field", path, name), ("palette", label)...
# The inspector while the game runs: each field's value and "authored" texts,
# refreshed in place by `live_tick`.
live_view = {"rows": {}, "gone": False, "spawned": ()}
_prov = {"key": None, "tab": {}, "conflicts": []}


def provenance_table():
    """Who reads and writes what, worked out again only when a script changed."""
    sources = {n: editor.source(n) for n in editor.sources()}
    key = tuple(sorted(sources.items()))
    if key != _prov["key"]:
        tab = provenance.table(sources)
        _prov.update(key=key, tab=tab, conflicts=provenance.conflicts(tab))
    return _prov["tab"]


def field_provenance(card, label, cname, fname):
    """Says where a value comes from, under its row: the systems that write
    it (so it will change when the game runs), and in the tooltip who reads
    it. Two writers in one phase is called out: which wins is decided by
    their order."""
    who = provenance.of_field(provenance_table(), cname, fname)
    full = provenance.sentence(who)
    if not full:
        return
    ui.tooltip(label, full)
    clash = [c for c in _prov["conflicts"] if c["component"] == cname and c["field"] == fname]
    if clash:
        text = "Two writers in %s: %s. Their order decides." % (clash[0]["phase"] or "the default phase", ", ".join(clash[0]["systems"]))
        line = ui.add(T.Para, card, text=text, color=DANGER, wrap=True, width=fill())
    elif who["write"] or who["unknown"]:
        text = "Written by " + ", ".join(who["write"]) if who["write"] else "Maybe written by " + ", ".join(who["unknown"])
        line = fitted(ui.add(T.Muted, card, text=""), text, 34)
        ui.tooltip(line, full)
    else:
        return
    handles[("provenance", cname, fname)] = line


def short_name(path):
    return path.split(".")[-1].split("::")[-1]


def build_inspector():
    clear(inspector)
    for key in [k for k in handles if k[0] in ("field", "provenance", "keep", "live")]:
        del handles[key]
    live_view["rows"] = {}
    info = editor.inspect()
    if info is None:
        ui.add(T.Muted, inspector, text="Nothing selected")
        return
    if editor.playing():
        build_live_inspector(info)
        return
    sid, name, components, tags = info
    ui.add(T.Muted, inspector, text="Entity #%d" % sid)
    field = ui.text_input(inspector, name, placeholder="Name", prefab=T.Field, width=fill())
    on_submit[field.sid] = guarded(lambda: editor.rename(sid, text_of(field)))
    for path, cname, fields in components:
        card = ui.add(T.Card, inspector)
        head = ui.add(T.Row, card, justify=ui.SPACE_BETWEEN)
        ui.add(T.Heading, head, text=cname)
        remove = button(head, "X", guarded((lambda p: lambda: editor.remove_component(sid, p))(path)), T.Danger, width=12)
        ui.tooltip(remove, "Remove %s from this entity" % cname)
        for fname, kind, text in fields:
            row = ui.add(T.Row, card, gap=3)
            label = ui.add(T.Muted, row, text=fname, width=36)
            if kind == "bool":
                t = ui.toggle(row, on=(text == "true"))
                on_toggle[t.sid] = guarded((lambda p, f: lambda on: editor.set_field(sid, p, f, "true" if on else "false"))(path, fname))
            else:
                try:
                    value = float(text)
                except ValueError:
                    value = 0.0
                step = 0.0 if kind in ("f32", "f64") else 1.0
                n = ui.number(row, value, step=step, prefab=T.Field, width=fill())
                handles[("field", short_name(path), fname)] = n
                if kind in ("f32", "f64"):
                    on_change[n.sid] = guarded((lambda p, f: lambda v: editor.set_field(sid, p, f, repr(round(v, 4))))(path, fname))
                else:
                    on_change[n.sid] = guarded((lambda p, f: lambda v: editor.set_field(sid, p, f, str(int(v))))(path, fname))
            field_provenance(card, label, cname, fname)
    for tag in tags:
        row = ui.add(T.Row, inspector, justify=ui.SPACE_BETWEEN)
        ui.add(T.Chip, row, text=short_name(tag), width=fill())
        remove = button(row, "X", guarded((lambda p: lambda: editor.remove_component(sid, p))(tag)), T.Danger, width=12)
        ui.tooltip(remove, "Remove %s from this entity" % short_name(tag))
    options = editor.addable()
    if options:
        sel = ui.select(inspector, ["Add Component"] + [n for _, n in options], width=fill())
        on_select[sel.sid] = guarded(lambda i: editor.add_component(sid, options[i - 1][0]) if i > 0 else None)


def build_live_inspector(authored_info):
    """The inspector while the game runs. The values are the running
    world's, read-only, with what was authored beside any that differ. All
    of it is thrown away on Stop, except a value kept with Keep, which
    becomes the authored one (an edit like any other: Undo takes it back)."""
    sid, name, components, _ = authored_info
    authored = {(path, fname): text for path, _, fields in components for fname, _, text in fields}
    banner = ui.add(T.Badge, inspector, text="Running: live values")
    banner.set(MODES["play"][1])
    ui.add(T.Muted, inspector, text="Stop throws these away. Keep makes one the authored value.", wrap=True, width=fill())
    ui.add(T.Muted, inspector, text="Entity #%d  %s" % (sid, name))
    live = editor.inspect_live()
    live_view["gone"] = live is None
    if live is None:
        ui.add(T.Para, inspector, text="The game destroyed this entity. It is back when you stop.", wrap=True, width=fill())
        return
    for path, cname, fields in live[2]:
        card = ui.add(T.Card, inspector)
        ui.add(T.Heading, card, text=cname)
        for fname, _, text in fields:
            row = ui.add(T.Row, card, gap=3)
            label = ui.add(T.Muted, row, text=fname, width=36)
            value = ui.add(T.Chip, row, text=text, width=fill())
            was_text = authored.get((path, fname))
            if was_text is None:
                # The game added this component: there is nothing authored to keep it into.
                ui.add(T.Muted, card, text="Added by the game")
                continue
            keep = button(row, "Keep", guarded((lambda p, f: lambda: editor.keep(sid, p, f))(path, fname)), width=28)
            ui.tooltip(keep, "Make this the authored value of %s.%s" % (cname, fname))
            was = ui.add(T.Muted, card, text="" if text == was_text else "authored " + was_text)
            live_view["rows"][(path, fname)] = (value, was, was_text)
            handles[("live", short_name(path), fname)] = value
            handles[("keep", short_name(path), fname)] = keep
            field_provenance(card, label, cname, fname)


def build_code():
    names = editor.sources()
    if code_state["name"] not in names:
        code_state["name"] = names[0] if names else None
        code_state["synced"] = None
    clear(code_names)
    for n in names:
        b = button(code_names, n, (lambda n: lambda: (code_state.update(name=n, synced=None), view_dirty.__setitem__(0, True)))(n), width=60)
    name = code_state["name"]
    if name is not None:
        source = editor.source(name)
        # Refill the text only when the source changed under it (a block edit,
        # a reload); never while the person is typing.
        if code_state["synced"] != source:
            code_area.set(Label(world.intern(source)))
            code_state["synced"] = source
    err = editor.error()
    set_text(code_error, ("Error: " + err) if err else ("Reloaded OK" if not editor.stale() else "Changes not applied"))
