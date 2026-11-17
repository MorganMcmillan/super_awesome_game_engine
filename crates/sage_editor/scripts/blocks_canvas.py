# The editor's screen, part 6: the stack canvas. Carrying stacks and blocks
# with the pointer, and notes as cards beside their blocks. Parts share one
# namespace.

note_grips = {}  # a note's drag handle: sid -> its key


@world.observer(Dragged)
def _note_dragged(d: Dragged):
    """A note moves on its own: its offset from where its block puts it."""
    key = note_grips.get(d.target)
    if key is None:
        return
    off = notes_ui["off"]
    if d.phase == 0:
        notes_ui["base"] = off.get(key, (0.0, 0.0))
        return
    bx, by = notes_ui.get("base", (0.0, 0.0))
    off[key] = (bx + d.dx, by + d.dy)


def pointer():
    p = world.get(Pointer)
    return p.x, p.y


def press_point():
    """Where the drag in progress was pressed (the pointer has already moved
    by the time it is reported)."""
    d = world.get(Dnd)
    return d.x, d.y


def start_tower(sid):
    """The whole stack follows the pointer until it is let go."""
    name, card, x0, y0 = move_grips[sid]
    r = card.get(Rect)
    px, py = press_point()
    tower.update(kind="stack", name=name, card=card, grab=(px - r.x, py - r.y), at=(x0, y0))
    card.set(ZIndex(30))


def chain_ids(view, stmt_id):
    """The statement and every statement below it in its body (Scratch's
    drag), and all ids inside them."""
    block, parent, field = view.b.find(stmt_id)
    chain = parent[field][parent[field].index(block):]
    inside = set()

    def walk(b):
        if isinstance(b, dict):
            if "id" in b:
                inside.add(b["id"])
            for v in b.values():
                walk(v)
        elif isinstance(b, list):
            for v in b:
                walk(v)

    walk(chain)
    return chain, inside


def start_chain(sid, stmt_id):
    """A block dragged out of its stack takes the blocks below it along: they
    leave the stack's flow and follow the pointer, held where they were
    picked up. They sit under the pointer, so the kit's own drop target
    (the topmost thing under it) would be the blocks themselves: the drop is
    worked out here instead (`resolve_drop`), ignoring what is carried."""
    view = cur["view"]
    if view is None:
        return
    try:
        chain, inside = chain_ids(view, stmt_id)
    except ValueError:
        return
    cols = [view.cols[c["id"]] for c in chain if c["id"] in view.cols]
    if not cols:
        return
    first = cols[0].get(Rect)
    px, py = press_point()
    offs = []
    for c in cols:
        r = c.get(Rect)
        offs.append((r.x - first.x, r.y - first.y))
        # Out of the flow a block has nothing to fill: keep the size it had,
        # or it is re-measured every tick and flickers.
        b = c.get(Box).copy()
        b.width_kind, b.width, b.height_kind, b.height = 1, r.w, 1, r.h
        c.set(b)
        c.set(Absolute(0.0, 0.0))
        c.set(ZIndex(30))
    for key, reg in dnd.items():
        if (reg[0] == "stmt" and reg[1] in inside) or (reg[0] == "slot" and reg[1] in inside):
            e = world.lookup(key)
            if e is not None:
                e.set(DropZone(0, 0))
    tower.update(kind="chain", stmt=stmt_id, cols=cols, offs=offs, abs=[(0.0, 0.0)] * len(cols), grab=(px - first.x, py - first.y), inside=inside)


def follow_pointer():
    """Every tick while something is carried: it keeps its place under the
    pointer (each step corrects the last one's error, so padding and clips
    do not matter)."""
    kind = tower.get("kind")
    if kind is None:
        return
    px, py = pointer()
    gx, gy = tower["grab"]
    # What is laid out now is shown on the next tick, when the pointer will
    # have moved on: aim where it is going, or the block trails the cursor by
    # as far as the cursor travels in a tick.
    lx, ly = tower.get("last", (px, py))
    tower["last"] = (px, py)
    tower["lead"] = (px - lx, py - ly)
    px, py = px + (px - lx), py + (py - ly)
    if kind == "stack":
        card = tower["card"]
        r = card.get(Rect)
        ax, ay = tower["at"]
        ax, ay = round(ax + (px - gx - r.x)), round(ay + (py - gy - r.y))
        tower["at"] = (float(max(0, ax)), float(max(0, ay)))
        card.set(Absolute(*tower["at"]))
        return
    for i, c in enumerate(tower["cols"]):
        r = c.get(Rect)
        ox, oy = tower["offs"][i]
        ax, ay = tower["abs"][i]
        # Whole units: layout snaps each edge on its own, so a fractional
        # position makes a block's size change from tick to tick.
        ax, ay = round(ax + (px - gx + ox - r.x)), round(ay + (py - gy + oy - r.y))
        tower["abs"][i] = (ax, ay)
        c.set(Absolute(ax, ay))


# ---- notes: cards beside the stacks, joined to their blocks by lines ----------------------
NOTE_W = 118
LINE_COLOR = 0xC9B037FF


def note_key(it):
    return (it["fn"], it["label"], it["lines"][0] if it["lines"] else "")


def make_notes(field):
    """One card per note, apart from the stack it belongs to; `notes_tick`
    puts each beside its block and draws the line between them."""
    cards = []
    for it in notes_ui["items"]:
        card = ui.add(ui.Panel, field, direction=COLUMN, gap=1, fill=STICKY, padding=3, width=NOTE_W, radius=3, at=(0.0, 0.0))
        key = note_key(it)
        grip = ui.add(ui.ResizeGrip, card)
        grip.set(Box(1, float(NOTE_W - 6), 1, 5.0))
        grip.set(Paint(0xD9C24FFF, 0, 0.0, 0))
        note_grips[grip.sid] = key
        handles[("note_grip", it["fn"], it["lines"][0] if it["lines"] else "")] = grip
        if it["label"]:
            ui.add(T.Badge, card, text=it["label"], fill=0, color=STICKY_INK)
        inputs = []
        for line in it["lines"]:
            if it["editable"]:
                f = ui.text_input(card, line, prefab=T.Field, width=fill(), fill=0xFBF1B5FF, color=STICKY_INK)
                inputs.append(f)
                handles[("note", it["fn"], line)] = f
            else:
                ui.add(T.Para, card, text=line, color=STICKY_INK, wrap=True, width=fill())
        if it["editable"]:
            on_submit_all = (lambda it, inputs: lambda: save_note(it, [text_of(f) for f in inputs]))(it, inputs)
            for f in inputs:
                on_submit[f.sid] = on_submit_all
        cards.append((it, card))
    notes_ui["cards"] = cards


def save_note(it, lines):
    view = cur["view"]
    if view is None:
        return
    lines = [l for l in lines if l.strip()]
    if it["label"] == "Doc":
        view.edit(lambda bb: bb.set_docstring(it["fn"], " ".join(lines) or None), "Edit description")()
    else:
        view.edit(lambda bb: bb.set_notes(it["block"], lines), "Edit note")()


def notes_tick():
    """Places each note beside its block (plus however far it was dragged)
    and draws a line from the block to the note."""
    cards = notes_ui.get("cards") or []
    field, canvas = notes_ui["field"], notes_ui["canvas"]
    if not cards or field is None or world.lookup(field.sid) is None or world.lookup(canvas.sid) is None:
        return
    fr = field.get(Rect)
    placed = {}
    drawn = []
    for it, card in cards:
        anchor = it["anchor"]
        if world.lookup(card.sid) is None or world.lookup(anchor.sid) is None:
            return
        ar = anchor.get(Rect)
        if ar.w <= 0:
            continue
        stacked = placed.get(anchor.sid, 0.0)
        cr = card.get(Rect)
        placed[anchor.sid] = stacked + max(cr.h, 20.0) + 6
        ox, oy = notes_ui["off"].get(note_key(it), (0.0, 0.0))
        # Beside the stack (and past any row that sticks out of it).
        right = ar.x + ar.w
        owner = it["card"]
        if owner is not None and world.lookup(owner.sid) is not None:
            o = owner.get(Rect)
            right = max(right, o.x + o.w)
            for h in notes_ui["heads"].get(owner.sid, ()):
                if world.lookup(h.sid) is not None:
                    hr = h.get(Rect)
                    right = max(right, hr.x + hr.w)
        x = right - fr.x + 22 + ox
        y = ar.y - fr.y + stacked + oy
        card.set(Absolute(float(x), float(y)))
        drawn.append((ar.x - fr.x + ar.w, ar.y - fr.y + ar.h / 2, x, y + max(cr.h, 20.0) / 2))
    sig = tuple((round(a), round(b), round(c), round(d)) for a, b, c, d in drawn)
    if sig == notes_ui["shown"]:
        return
    notes_ui["shown"] = sig
    ui.canvas_clear(canvas)
    for x0, y0, x1, y1 in drawn:
        ui.draw_line(canvas, x0, y0, x1, y1, LINE_COLOR, 1.5)
        ui.draw_ellipse(canvas, x0, y0, 2.5, 2.5, LINE_COLOR)
