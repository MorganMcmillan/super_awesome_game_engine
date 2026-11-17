# The editor's screen, part 8: the Timing tab (phases, the crosstab of
# modules by phase) and the Prefabs tab. Parts share one namespace.

# ---- the system that rebuilds what changed -------------------------------------------------
seen = [None]


timing_box = ui.add(ui.Panel, timing_panel, direction=COLUMN, gap=4, width=fill())
prefab_box = ui.add(ui.Panel, prefab_panel, direction=COLUMN, gap=4, width=fill())


def decl_source():
    """The script that declares phases and prefabs: the first one with any,
    else the first script."""
    names = editor.sources()
    for n in names:
        try:
            if B.declarations(editor.source(n)):
                return n
        except ValueError:
            pass
    return names[0] if names else None


def set_ref(name, var, ref):
    editor.set_source(name, B.set_decl_ref(editor.source(name), var, ref))


def add_decl(name, kind):
    src = editor.source(name)
    n = 1
    taken = {d["var"] for d in B.declarations(src)}
    base = "Phase" if kind == "phase" else "Prefab"
    while "%s%d" % (base, n) in taken:
        n += 1
    editor.set_source(name, B.add_declaration(src, kind, "%s%d" % (base, n)))


# The hover pulse's timing (design doc 5.5: "after a short hover delay, a few
# cycles only"), changed with the number fields at the top of TIMING.
settings = {"hover_delay": 15, "pulse_cycles": 3}

timing_state = {"cards": {}, "decls": [], "hover": None, "since": 0, "shown": {}}


def mark(sid, ok):
    e = world.lookup(sid)
    p = e.get(Paint) if e is not None else None
    if p is None:
        return
    _lit_paint[sid] = p.copy()
    q = p.copy()
    q.border, q.border_width = (0x59C059FF if ok else 0xD8453CFF), 2.0
    e.set(q)


def module_enabled(var, name):
    """Whether the flecs module a script declared is on, from the live world
    (`None` when it is not found): the provenance for "why is this system
    off?" (design doc 5.5)."""
    for path, on in editor.modules():
        if path == name or path.endswith("::" + name) or path.endswith("." + name):
            return on
    return None


def build_decls(box, kind):
    """The timing graph (phases, design doc 5.5) or the prefab list (5.2).
    Phases are columns left to right; drag from a phase's OUT handle to
    another phase to make that one run after it (a cycle is dimmed red while
    you drag, and refused). The crosstab below groups systems by module and
    phase; drag a system card to another cell to move it."""
    clear(box)
    if kind == "phase":
        timing_state["cards"].clear()
    name = decl_source()
    if name is None:
        return
    try:
        decls = B.declarations(editor.source(name))
        systems = B.systems_in(editor.source(name))
    except ValueError as e:
        ui.add(T.Para, box, text="Not valid Python yet: " + str(e), width=fill())
        return
    if kind == "phase":
        timing_state["decls"] = decls
    mine = [d for d in decls if d["kind"] == kind]
    head = ui.add(T.Row, box, gap=4)
    ui.add(T.Muted, head, text=("Drag Out > onto a phase to run it after that phase" if kind == "phase" else "Templates and what uses them"))
    if kind == "phase":
        for label, key, high in (("Hover Delay", "hover_delay", 120), ("Pulses", "pulse_cycles", 10)):
            ui.add(T.Muted, head, text=label)
            n = ui.number(head, settings[key], 0, high, 1, prefab=T.Field, width=30)
            on_change[n.sid] = (lambda key: lambda v: settings.__setitem__(key, int(v)))(key)
            handles[("setting", key)] = n
    button(head, "New Phase" if kind == "phase" else "New Prefab", guarded(lambda: add_decl(name, kind)), T.Primary, width=64)
    layers = B.phase_layers(decls) if kind == "phase" else [[d["var"] for d in mine]]
    row = ui.add(ui.Panel, box, direction=ROW, gap=6, align=START, width=fill())
    users_key = "phase" if kind == "phase" else "prefab"
    for li, layer in enumerate(layers):
        col = ui.add(ui.Panel, row, direction=COLUMN, gap=4)
        for var in layer:
            d = next(x for x in mine if x["var"] == var)
            card = ui.add(T.Card, col, width=110)
            ui.add(T.Heading, card, text=var)
            others = [x["var"] for x in mine if x["var"] != var]
            opts = ["Nothing"] + others
            pick = ui.select(card, opts, value=(opts.index(d["ref"]) if d["ref"] in opts else 0), width=fill())
            ui.add(T.Muted, card, text="Runs After" if kind == "phase" else "Derives from")
            on_select[pick.sid] = guarded((lambda var, opts: lambda i: set_ref(name, var, None if i == 0 else opts[i]))(var, opts))
            handles[(kind, var)] = pick
            users = [x["name"] for x in systems if x[users_key] == var]
            ui.add(T.Muted, card, text=("Systems: " + ", ".join(users)) if users else "No systems")
            if kind == "prefab":
                ui.add(T.Muted, card, text="Instances: %d" % editor.instances_of(d["name"] or var))
            else:
                hop = ui.add(T.Badge, card, text=" ", fill=0)
                out = ui.add(T.Badge, card, text="Out >", fill=0xF28C28FF, color=INK)
                ui.draggable(out, 0, 4)
                ui.drop_zone(card, (4,))
                dnd[out.sid] = ("phase_out", var)
                dnd[card.sid] = ("phase_in", var)
                handles[("phase_out", var)] = out
                handles[("phase_card", var)] = card
                timing_state["cards"][var] = (card, hop)
        if kind == "phase" and li < len(layers) - 1:
            ui.add(T.Heading, row, text=">")
    if not mine:
        ui.add(T.Muted, box, text="None declared yet: " + ("world.phase('Name', after=Other)" if kind == "phase" else "world.prefab('Name', base=Other)"))
    if kind == "phase":
        build_crosstab(box, name, decls, systems, layers)


def build_crosstab(box, name, decls, systems, layers):
    """Modules as rows, phases as columns, systems as cards (design doc 5.5):
    a card dragged to another cell changes when it runs (sideways) or which
    module it lives in (vertically). A module that is off says so, and every
    system in it says why it is off."""
    phases = [v for layer in layers for v in layer]
    modules = [d["var"] for d in decls if d["kind"] == "module"]
    mod_names = {d["var"]: d["name"] or d["var"] for d in decls if d["kind"] == "module"}
    ui.add(T.Muted, box, text="By module (rows) and phase (columns). Drag a system to move it.")
    cols = [None] + phases
    hdr = ui.add(T.Row, box, gap=2)
    ui.add(T.Badge, hdr, text="Module", fill=0, color=0xF3EBE2FF, width=70)
    for ph in cols:
        ui.add(T.Badge, hdr, text=(ph or "Default"), fill=0x00000040, color=0xF3EBE2FF, width=60)
    for mod in [None] + modules:
        label = "Script" if mod is None else mod
        r = ui.add(T.Row, box, gap=2, align=START)
        side = ui.add(ui.Panel, r, direction=COLUMN, gap=1, width=70)
        ui.add(T.Badge, side, text=label, fill=0x00000040, color=0xF3EBE2FF)
        on = module_enabled(mod, mod_names.get(mod, mod)) if mod else None
        if mod is not None:
            if on is False:
                ui.add(T.Badge, side, text="Off", fill=0xD8453CFF, color=0xF3EBE2FF)
            t = button(side, "Enable" if on is False else "Disable", (lambda m, on: lambda: editor.set_module(m, on is False) or view_dirty.__setitem__(0, True))(mod, on), width=fill())
            handles[("module_toggle", mod)] = t
        for ph in cols:
            cell = ui.add(ui.Panel, r, direction=COLUMN, gap=1, width=60, height=30, fill=0x00000030)
            ui.drop_zone(cell, (5,))
            dnd[cell.sid] = ("cell", mod, ph)
            handles[("cell", mod, ph)] = cell
            for sy in systems:
                owner = sy["owner"] if sy["owner"] in modules else None
                if owner == mod and sy["phase"] == ph and sy["owner"] not in [d["var"] for d in decls if d["kind"] == "prefab"]:
                    card = ui.add(T.Item, cell, text=sy["name"][:9])
                    ui.draggable(card, 0, 5)
                    dnd[card.sid] = ("sys", sy["name"])
                    handles[("sys", sy["name"])] = card
                    if on is False:
                        card.set(Paint(0x6A6258FF, 0, 0.0, 0xFFFFFFFF))
                        why = ui.add(T.Muted, cell, text="Off: module %s is disabled" % mod, wrap=True, width=fill())
                        handles[("why_off", sy["name"])] = why


def timing_hover():
    """Hovering a phase pulses toward the phases that depend on it, hop by hop
    (design doc 5.5). The hop badges (+1, +2) stay as the static fallback."""
    st = timing_state
    cards = st["cards"]
    if not cards:
        return
    ptr = world.get(Pointer)
    over = None
    if ptr is not None and ptr.present:
        for var, (card, _) in cards.items():
            r = card.get(Rect)
            if r is not None and r.x <= ptr.x <= r.x + r.w and r.y <= ptr.y <= r.y + r.h:
                over = var
    if over != st["hover"]:
        st["hover"], st["since"] = over, ticks[0]
        for var, (card, hop) in cards.items():
            if st["shown"].get(var):
                set_text(hop, " ")
        st["shown"].clear()
    if over is None:
        return
    deps = {}
    for d in st["decls"]:
        if d["kind"] == "phase" and d["ref"]:
            deps.setdefault(d["ref"], []).append(d["var"])
    dist, queue = {over: 0}, [over]
    while queue:
        cur = queue.pop(0)
        for nxt in deps.get(cur, []):
            if nxt not in dist:
                dist[nxt] = dist[cur] + 1
                queue.append(nxt)
    held = ticks[0] - st["since"]
    if held < settings["hover_delay"]:  # a short hover delay
        return
    width = max(dist.values()) + 1
    cycle = (held - settings["hover_delay"]) // 8
    pulsing = cycle < settings["pulse_cycles"] * width  # a few cycles only, then it rests
    for var, k in dist.items():
        card, hop = cards[var]
        want = ("Hover" if k == 0 else "+%d" % k, bool(pulsing and cycle % width == k))
        if st["shown"].get(var) != want:
            set_text(hop, want[0])
            card.set(Paint(0x1B1714FF, 0xFFFFFFFF if want[1] else 0x3B3229FF, 3.0 if want[1] else 1.0, 0xF28C28FF))
            st["shown"][var] = want


def build_timing():
    build_decls(timing_box, "phase")


def build_prefabs():
    build_decls(prefab_box, "prefab")
