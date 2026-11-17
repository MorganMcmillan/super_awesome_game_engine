# The editor's screen, part 7: editing blocks. The right-click menu, drops,
# the palette, and rebuilding the block tab. Parts share one namespace.

# ---- the right-click menu of a block or a stack -------------------------------------------------
@world.observer(ContextRequested)
def _block_context(c: ContextRequested):
    ctx["target"] = c.target


def menu_row(label, stack=False):
    """The row of the block menu (or the stack menu) with this label (for
    tests and scripts)."""
    return next(c for c in (stack_menu if stack else block_menu).children if c.has(MenuItem) and len(c.children) > 1 and world.string(c.children[1].get(Label).text) == label)


def block_action(action, target=None):
    """What a menu item does to the block it was opened on (`target` is the
    entity of its head; the last right-click by default)."""
    view = cur["view"]
    reg = dnd.get(target if target is not None else ctx["target"])
    if view is None or reg is None:
        return
    if reg[0] == "fnhead":
        f = view.b.find(reg[1])[0]
        if action == M_NOTE:
            view.edit(lambda bb: bb.set_notes(reg[1], f["notes"] + ["New note"]))()
        elif action == 7:
            view.edit(lambda bb: bb.set_notes(reg[1], []))()
        elif action == M_DELETE_STACK:
            delete_stack(f)
        else:
            say("That action is for a block, not a stack")
        return
    if reg[0] != "stmt":
        return
    sid, parent_id, field = reg[1], reg[2], reg[3]
    block, parent, pfield = view.b.find(sid)
    siblings = parent[pfield]
    i = siblings.index(block)
    if action == M_NOTE:
        view.edit(lambda bb: bb.set_notes(sid, (block.get("notes") or []) + ["New note"]))()
    elif action == 7:
        view.edit(lambda bb: bb.set_notes(sid, []))()
    elif action == M_DELETE:
        view.edit(lambda bb: bb.delete(sid), "Delete block")()
    elif action == M_UP:
        if i > 0 and not block.get("elif"):
            view.edit(lambda bb: bb.move(sid, parent["id"], pfield, i - 1), "Move block up")()
    elif action == M_DOWN:
        if i < len(siblings) - 1 and not block.get("elif"):
            view.edit(lambda bb: bb.move(sid, parent["id"], pfield, i + 2), "Move block down")()
    elif action == M_ELSE:
        if block["k"] != "if":
            say("Only an if block has an else")
        else:
            view.edit(lambda bb: bb.add_else(sid))()
    elif action == M_RUN:
        info = view.run_info.get(sid)
        if info is None:
            say("Run to here works on a stack with no trigger")
        else:
            view.run_to(*info)


@world.observer(MenuChosen)
def _block_menu_chosen(m: MenuChosen):
    item = world.lookup(m.target)
    if item is not None and item.parent is not None and item.parent.sid in (block_menu.sid, stack_menu.sid):
        block_action(m.id)


def shape_ok(entry, expects):
    """Shape enforces type in sockets (design doc 4.2, 4.6 [Leaning]): a slot
    that wants a test, a number or text takes a value of that type, or one
    whose type is not known here (a name, attribute, call or index: `any`).
    Types go in type slots and nowhere else."""
    if entry["shape"] == "type":
        return expects == "type"  # types go in type slots and nowhere else
    if expects == "type":
        return False
    if expects in ("bool", "num", "str") or (expects and expects.startswith("list")):
        known = "bool" if entry["shape"] == "predicate" else B.type_of(entry["block"])
        return B.type_fits(known, expects)
    return True


WANTS = {"bool": "a test", "num": "a number", "str": "text", "type": "a type"}


def wants_text(t):
    """What a slot says it wants: "a number", "a list of numbers" ..."""
    if t and t.startswith("list"):
        inner = B.element_slot(t)
        if inner is None:
            return "a list"
        if inner.startswith("list"):
            return "a list of lists"
        return "a list of " + {"num": "numbers", "str": "text", "bool": "tests"}.get(inner, "things")
    return WANTS.get(t, "another shape")


_lit_paint = {}


def highlight(entry):
    """While a palette block is dragged, the slots that take it light up
    green and the ones that refuse it red."""
    for sid, reg in dnd.items():
        e = world.lookup(sid)
        if e is None:
            continue
        if reg[0] not in ("expr", "stmt"):
            continue
        if reg[0] == "expr":
            ok = entry["shape"] != "command" and shape_ok(entry, reg[2])
        elif reg[0] == "stmt":
            ok = entry["shape"] == "command"
        else:
            continue
        p = e.get(Paint)
        if p is None:
            continue
        _lit_paint[sid] = p.copy()
        q = p.copy()
        q.border, q.border_width = (0x59C059FF if ok else 0xD8453CFF), 2.0
        e.set(q)


def unhighlight():
    for sid, p in _lit_paint.items():
        e = world.lookup(sid)
        if e is not None:
            e.set(p)
    _lit_paint.clear()


@world.observer(DragStarted)
def _drag_started(d: DragStarted):
    reg = dnd.get(d.target)
    if reg and reg[0] == "phase_out":
        # Dim every target that would make a cycle (design doc 5.5).
        for sid, r in dnd.items():
            if r[0] == "phase_in":
                mark(sid, r[1] != reg[1] and not B._creates_cycle(timing_state["decls"], r[1], reg[1]))
        return
    if reg and reg[0] == "sys":
        for sid, r in dnd.items():
            if r[0] == "cell":
                mark(sid, True)
        return
    if reg and reg[0] == "pal":
        drag_now["entry"] = reg[1]
        highlight(reg[1])
        return
    if d.target in move_grips:
        start_tower(d.target)
    elif reg and reg[0] == "stmt":
        start_chain(d.target, reg[1])


@world.observer(DragEnded)
def _drag_ended(d: DragEnded):
    drag_now["entry"] = None
    unhighlight()
    kind = tower.get("kind")
    if kind is None:
        return
    info = dict(tower)
    tower.clear()
    tower["kind"] = None
    view = cur["view"]
    if kind == "stack":
        info["card"].set(ZIndex(0))
        if view is not None:
            # Where the card was aimed was a tick ahead of the pointer; it is
            # saved where the pointer let go.
            lx, ly = info.get("lead", (0, 0))
            x, y = max(0, round(info["at"][0] - lx)), max(0, round(info["at"][1] - ly))
            view.edit(lambda bb: bb.set_position(info["name"], int(x), int(y)), "Move stack " + info["name"])()
    else:
        nav = world.get(Nav)
        cancelled = nav is not None and nav.bits & NAV_CANCEL
        src = dnd.get(d.target)
        dst = None if cancelled else resolve_drop(info["inside"])
        if src is None or dst is None:
            view_dirty[0] = True  # let go over nothing, or Esc: the blocks go back
        else:
            perform_drop(src, dst, info["grab"])


def resolve_drop(inside):
    """What the pointer is over, for blocks being carried: the smallest
    drop target (a block, an end-of-body bar, a stack, then the canvas) under
    it, not counting the carried blocks and what they contain."""
    view_rect = handles["canvas"].get(Rect)
    px, py = pointer()
    if not (view_rect.x <= px < view_rect.x + view_rect.w and view_rect.y <= py < view_rect.y + view_rect.h):
        return None
    best, best_area = None, None
    for sid, reg in dnd.items():
        if reg[0] not in ("stmt", "slot", "fn", "canvas"):
            continue
        if reg[0] in ("stmt", "slot") and reg[1] in inside:
            continue
        e = world.lookup(sid)
        if e is None:
            continue
        r = e.get(Rect)
        if r.x <= px < r.x + r.w and r.y <= py < r.y + r.h:
            area = r.w * r.h
            if best is None or area < best_area:
                best, best_area = reg, area
    return best


@world.observer(Dropped)
def _block_dropped(d: Dropped):
    src, dst = dnd.get(d.source), dnd.get(d.target)
    if src and dst and src[0] == "phase_out" and dst[0] == "phase_in":
        guarded(lambda: set_ref(decl_source(), dst[1], src[1]))()
        return
    if src and dst and src[0] == "sys" and dst[0] == "cell":
        name = decl_source()
        def move_system():
            bb = B.Blocks(editor.source(name))
            err = editor.set_source(name, bb.set_system_slot(src[1], dst[1], dst[2]))
            return err
        guarded(move_system)()
        return
    if tower.get("kind") == "chain":
        return  # carried blocks are dropped by `_drag_ended`
    perform_drop(src, dst, tower.get("grab", (10.0, 6.0)))


def perform_drop(src, dst, grab):
    """Drops what `src` names on what `dst` names (both entries of `dnd`)."""
    view = cur["view"]
    if src is None or dst is None or view is None:
        return

    def go(bb):
        if dst[0] == "canvas":
            if src[0] == "stmt":
                x, y = pointer()
                fr = notes_ui["field"].get(Rect)
                gx, gy = grab
                n = 1
                names = {i.get("name") for i in bb.items}
                while "stack_%d" % n in names:
                    n += 1
                name = "stack_%d" % n
                bb.add_function(name, [], [], None, (max(0, int(x - fr.x - gx)), max(0, int(y - fr.y - gy))))
                nf = bb.function(name)
                bb.move_tail(src[1], nf["id"], "body", 1)
                bb.delete(bb.function(name)["body"][0]["id"])
            return
        if dst[0] == "slot":
            if src[0] == "stmt":
                bb.move_tail(src[1], dst[1], dst[2], 10**6)
            elif src[0] == "pal" and src[1]["shape"] == "command":
                bb.insert(dst[1], dst[2], 10**6, src[1]["block"])
            return
        if src[0] == "stmt":
            if dst[0] == "stmt":
                bb.move_tail(src[1], dst[2], dst[3], dst[4])
            elif dst[0] == "fn":
                bb.move_tail(src[1], dst[1], "body", 10**6)
        elif src[0] == "pal":
            entry = src[1]
            if entry["shape"] == "command":
                if dst[0] == "stmt":
                    bb.insert(dst[2], dst[3], dst[4], entry["block"])
                elif dst[0] == "fn":
                    bb.insert(dst[1], "body", 10**6, entry["block"])
            elif dst[0] == "expr":
                if not shape_ok(entry, dst[2]):
                    raise ValueError("This slot wants %s" % wants_text(dst[2]))
                bb.replace(dst[1], entry["block"])

    view.edit(go, "Insert block" if src[0] == "pal" else "Move blocks")()


def find_selected(view):
    """The block the selection names, in this parse: `(block, parent, field)`."""
    sel = blk["sel"]
    if sel is None:
        return None
    kind, fn, bid = sel[0], sel[1], sel[2]
    try:
        if kind == "b":
            return view.b.find(bid)[0], None, sel[3]
        if kind == "f":
            return view.b.function(fn), None, None
        return view.b.find(bid)
    except ValueError:
        return None


def insert_from_palette(entry):
    name = blk["src"]
    try:
        view = View(name)
    except ValueError as e:
        say(str(e))
        return
    shape = entry["shape"]
    block = entry["block"]
    sel = blk["sel"]
    found = find_selected(view)

    def go(bb):
        if sel is not None and sel[0] == "b":
            # An end-of-body bar is selected: the block goes at its bottom.
            if shape != "command":
                raise ValueError("A value cannot go in a body")
            bb.insert(found[0]["id"], found[2], 10**6, block)
            return
        if shape == "command":
            if found is None:
                fn = next((i for i in bb.items if i["k"] == "function" and not i["locked"]), None)
                if fn is None:
                    raise ValueError("Add a system first")
                bb.insert(fn["id"], "body", 10**6, block)
                return
            blockf, parent, field = found
            if blockf["k"] == "function":
                bb.insert(blockf["id"], "body", 10**6, blockf and block)
            elif blockf["k"] in B.STATEMENTS and parent is not None:
                i = parent[field].index(blockf)
                bb.insert(parent["id"], field, i + 1, block)
            else:
                raise ValueError("Select a statement first")
        else:
            if found is None or found[0]["k"] in B.STATEMENTS or found[0]["k"] == "function":
                raise ValueError("Select the value to replace")
            bb.replace(found[0]["id"], block)

    view.edit(go, "Insert " + entry["label"])()


def selected_expectation():
    """What the selection takes: `("expr", type)` for a value slot (its input
    type), `("stmt", None)` for a statement or a bar, `("any", None)` when
    nothing is selected or suggestions are not narrowed by type (a project
    setting: `suggest_by_type`)."""
    sel = blk["sel"]
    if sel is None or editor.setting("suggest_by_type", "on") != "on":
        return "any", None
    if sel[0] == "e":
        for reg in dnd.values():
            if reg[0] == "expr" and reg[1] == sel[2]:
                return "expr", reg[2]
        return "expr", None
    return "stmt", None


def fill_palette(pal, entries):
    # Ctrl+B searches the block (command) entries, Ctrl+E the value and type
    # entries; the filter box narrows them. Otherwise the category decides.
    # With a slot selected, only what fits its input type is suggested: the
    # suggestion's output type is checked against the slot's input type.
    needle = text_of(pal_filter).strip().lower()
    mode = blk["mode"]
    kind, want = selected_expectation()
    shown = []
    for e in entries:
        if mode == "block" and e["shape"] != "command":
            continue
        if mode == "expr" and e["shape"] == "command":
            continue
        if kind == "expr" and (e["shape"] == "command" or not shape_ok(e, want)):
            continue
        if kind == "stmt" and e["shape"] != "command":
            continue
        # Without a search or a selection, the category decides.
        if mode is None and kind == "any" and e["category"] != blk["cat"]:
            continue
        if mode is None and kind != "any" and not needle and e["category"] != blk["cat"] and False:
            continue
        if needle and needle not in e["label"].lower():
            continue
        shown.append(e)
    blk["entries"] = shown
    if kind != "any":
        ui.add(T.Muted, pal, text="Showing what fits the selection" + ((": " + wants_text(want)) if want else ""))
    for e in shown:
        shape = {"command": "", "reporter": "( ) ", "predicate": "< > ", "type": "[ ] "}[e["shape"]]
        full = shape + e["label"].replace("[]", "_")
        label = short(full, 26)
        row = button(pal, label, (lambda e: lambda: insert_from_palette(e))(e), T.Item, width=6 * len(label) + 8)
        if label != full:
            ui.tooltip(row, full)
        row.set(Paint(cat_color(e["category"]), 0, 0.0, 0xFFFFFFFF))
        handles[("palette", e["label"])] = row
        ui.draggable(row, 0, 2)
        dnd[row.sid] = ("pal", e)


def build_blocks():
    glow["heads"].clear()
    glow["lit"].clear()
    names = editor.sources()
    if blk["src"] not in names:
        blk["src"] = names[0] if names else None
    for k in [k for k in handles if k[0] in ("stmt", "lit")]:
        del handles[k]
    clear(blk_head)
    clear(blk_palette)
    clear(blk_canvas)
    if blk["src"] is None:
        return
    ui.add(T.Heading, blk_head, text=blk["src"] + ".py")
    try:
        view = View(blk["src"])
    except ValueError as e:
        # The text is not valid Python right now (a half-typed edit): blocks
        # wait until it is, and the text is never touched.
        ui.add(T.Para, blk_canvas, text="This file is not valid Python yet: " + str(e), wrap=True, width=fill())
        blk["new_system"] = None
        return
    cur["view"] = view
    prog = view.b.program
    # palette: categories from reflection, then the blocks of the chosen one
    schemas = {p: [tuple(f) for f in fs] for p, fs in editor.schemas().items()}
    scope = []
    found = find_selected(view)
    fn = None
    if found:
        sel = blk["sel"]
        fn = view.b.function(sel[1]) if sel[0] != "e" and sel[1] else (view.b.function(sel[1]) if sel[1] else None)
    if fn is None:
        fn = next((i for i in view.b.items if i["k"] == "function" and not i["locked"]), None)
    if fn:
        for pname, cname in B.scope_of(fn):
            for path in schemas:
                if path.replace("::", ".").split(".")[-1] == cname:
                    scope.append((pname, path))
    entries = B.palette(schemas, scope)
    cats = []
    for e in entries:
        if e["category"] not in cats:
            cats.append(e["category"])
    if blk["cat"] not in cats:
        blk["cat"] = cats[0]
    pal_head = ui.add(T.Row, blk_palette, gap=4)
    ui.add(T.Muted, pal_head, text="Palette")
    sel_box = ui.select(pal_head, cats, value=cats.index(blk["cat"]), width=90)
    on_select[sel_box.sid] = lambda i: (blk.update(cat=cats[i]), view_dirty.__setitem__(0, True))
    pal = ui.add(ui.Panel, blk_palette, direction=ROW, gap=2, wrap_flow=2, width=fill())
    for k in [k for k in handles if k[0] == "palette"]:
        del handles[k]
    palette_ctx = (pal, entries)
    # canvas: one card per function, other top-level code as quiet text
    sel_entity = editor.inspect()
    comps = [c[1] for c in sel_entity[2]] if sel_entity else []

    def new_system():
        n = 1
        existing = {i.get("name") for i in view.b.items}
        while "system_%d" % n in existing:
            n += 1
        name = "system_%d" % n
        scope_ = [(c.lower(), c) for c in comps[:2]]
        deco = [{"k": "call", "func": {"k": "attr", "value": {"k": "name", "name": "world"}, "attr": "system"},
                 "args": [{"k": "lit", "t": "str", "v": name}],
                 "kwargs": [["query", {"k": "lit", "t": "str", "v": ", ".join(c for _, c in scope_)}]] if scope_ else []}]
        view.edit(lambda bb: bb.add_function(name, [(p, {"k": "name", "name": c}) for p, c in scope_], deco, None, (10, 10 + 40 * n)), "New system " + name)()

    blk["new_system"] = new_system
    handles[("new_system",)] = button(blk_head, "New System", lambda: run_command("new_system", "click"), T.Primary, width=64)
    # The stack canvas: every function is a card at its `@sage.position`
    # (stacks without one are placed in a column), scrolled both ways.
    canvas = ui.scroll(blk_canvas, both=True, direction=COLUMN, gap=0, width=fill(), height=250)
    field = ui.add(ui.Panel, canvas, width=CARD_W + 700, height=900)
    handles["canvas"] = canvas
    # Dropping a block on bare canvas starts a stack of its own.
    ui.drop_zone(field, (1,))
    dnd[field.sid] = ("canvas",)
    notes_ui.update(items=[], cards=[], field=field, shown=None, canvas=ui.canvas(field, CARD_W + 700, 900, at=(0.0, 0.0)))
    if prog.get("doc"):
        anchor = ui.add(ui.Panel, field, width=1, height=1, at=(10.0, 0.0))
        view.sticky(anchor, [prog["doc"]], 0, label="File", editable=False)
    auto_y = 60
    for item in prog["items"]:
        if item["k"] == "function":
            pos = item["position"]
            if pos is None:
                at = (10.0, float(auto_y))
                auto_y += est_height(item) + 10
            else:
                at = (float(pos[0]), float(pos[1]))
            view.function(field, item, at)
    notes_ui["heads"] = view.heads
    make_notes(field)
    fill_palette(*palette_ctx)
    want = blk.pop("focus", None)
    if want is not None:
        target = view.picks.get(want)
        if target is not None:
            set_focus(target)
    blk["focus"] = None
    hidden = [i for i in prog["items"] if i["k"] == "code"]
    if hidden:
        ui.add(T.Muted, blk_canvas, text="%d imports and setup items. See the Code tab." % len(hidden))
