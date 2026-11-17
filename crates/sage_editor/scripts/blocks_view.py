# The editor's screen, part 5: drawing a script as stacks of blocks (`View`).
# Parts share one namespace.

# ---- the block editor -----------------------------------------------------------------------
CAT_COLORS = {
    "Control": 0xE6A23CFF,
    "Variables": 0xF0803CFF,
    "Operators": 0x59C059FF,
    "Values": 0x4C97FFFF,
    "Calls": 0x9966FFFF,
}
OTHER_COLORS = [0xE5598AFF, 0x2EC4B6FF, 0xC9B037FF, 0x7A9CC6FF]
STICKY, STICKY_INK = 0xF4E27AFF, 0x3A3010FF
INK = 0x1A1109FF


def cat_color(cat):
    if cat in CAT_COLORS:
        return CAT_COLORS[cat]
    return OTHER_COLORS[sum(map(ord, cat)) % len(OTHER_COLORS)]


STMT_CAT = {"assign": "Variables", "change": "Variables", "declare": "Variables", "discard": "Calls"}


def chip(parent, text, color, **style):
    return ui.add(T.Badge, parent, text=text, fill=color, color=INK, **style)


def select_block(serial):
    blk["sel"] = None if blk["sel"] == serial else serial
    view_dirty[0] = True


def set_focus(e):
    i = world.get(Interaction).copy()
    i.focus = e.sid
    world.set(i)


def pick_block(fn, sid):
    """Click selects a block. Enter on a selected block steps to the next one
    (Shift+Enter, the previous), depth-first, through the empty-branch bars."""
    nav = world.get(Nav)
    bits = nav.bits if nav is not None else 0
    view = cur["view"]
    if bits & NAV_ACCEPT and blk["sel"] == ("s", fn, sid) and view is not None:
        keys = [k for _, k in view.order]
        i = keys.index(sid)
        j = i + (-1 if bits & NAV_SHIFT else 1)
        if 0 <= j < len(keys):
            nfn, nk = view.order[j]
            blk["sel"] = ("s", nfn, nk) if not isinstance(nk, tuple) else ("b", nfn, nk[1], nk[2])
            blk["focus"] = nk
    else:
        blk["sel"] = None if blk["sel"] == ("s", fn, sid) else ("s", fn, sid)
        blk["focus"] = sid
    view_dirty[0] = True


def view_order_index():
    """Where the selection is in reading order (for tests and scripts)."""
    view, sel = cur["view"], blk["sel"]
    keys = [(("b", k[1], k[2]) if isinstance(k, tuple) else ("s", fn, k)) for fn, k in view.order]
    here = sel[:3] if sel[0] == "s" else ("b", sel[2], sel[3])
    return keys.index(here)


def pick_slot(fn, parent_id, field):
    key = ("b", fn, parent_id, field)
    nav = world.get(Nav)
    bits = nav.bits if nav is not None else 0
    view = cur["view"]
    if bits & NAV_ACCEPT and blk["sel"] == key and view is not None:
        keys = [k for _, k in view.order]
        i = keys.index(("b", parent_id, field))
        j = i + (-1 if bits & NAV_SHIFT else 1)
        if 0 <= j < len(keys):
            nfn, nk = view.order[j]
            blk["sel"] = ("s", nfn, nk) if not isinstance(nk, tuple) else ("b", nfn, nk[1], nk[2])
            blk["focus"] = nk
    else:
        blk["sel"] = None if blk["sel"] == key else key
        blk["focus"] = ("b", parent_id, field)
    view_dirty[0] = True


BIG = ("list", "dict", "set", "comp", "call", "ifexp", "lambda", "tuple")
ticks = [0]  # UI ticks, for double-click timing
double_clicks = {}


def _plain(b):
    """A copy of a parsed block without ids, spans and notes."""
    if isinstance(b, dict):
        return {k: _plain(v) for k, v in b.items() if k not in ("id", "span", "line", "end", "notes", "trail", "elif")}
    if isinstance(b, list):
        return [_plain(v) for v in b]
    return b


def _with_phase(call, phase):
    out = _plain(call)
    out["kwargs"] = [kw for kw in out["kwargs"] if kw[0] != "phase"]
    if phase is not None:
        out["kwargs"].append(["phase", {"k": "name", "name": phase}])
    return out


def _grown(e, key, new):
    out = _plain(e)
    if key == "kwargs":
        used = {n for n, _ in out["kwargs"]}
        n, v = new
        while n in used:
            n += "_"
        out["kwargs"].append([n, v])
    else:
        out[key].append(new)
    return out


def _shrunk(e, key):
    out = _plain(e)
    out[key].pop()
    return out


class View:
    """Draws one source's functions as stacks of blocks."""

    def __init__(self, name):
        self.name = name
        self.b = B.Blocks(editor.source(name))
        self.n = 0  # statements drawn so far (the handles' numbering)
        self.missing = set()
        self.order = []  # (function, statement id) in reading order, for Enter
        self.picks = {}
        self.slots = 0
        self.is_stray = False
        self.double = double_clicks
        self.cur_f = None
        self.why = None
        if editor.error():
            line = B.error_line(editor.error())
            if line:
                self.why = self.b.block_at_line(line)
        self.lits = 0
        self.cols = {}
        self.run_info = {}
        self.heads = {}  # each stack card's block heads, to find how far its rows reach

    def edit(self, fn, label=None):
        """Returns a function that makes one edit of the blocks: `fn` changes
        them, and the new text becomes one undo step called `label`."""

        def run():
            try:
                fn(self.b)
            except ValueError as e:
                say(str(e))
                view_dirty[0] = True
                return
            err = editor.set_source(self.name, self.b.source, label or "Edit blocks in %s.py" % self.name)
            if err:
                say(err)
            blk["sel"] = None
            view_dirty[0] = True

        return run

    def sticky(self, anchor, lines, block_id, label=None, editable=True):
        """A note is its own card, apart from the stack. It is joined to the
        block it describes by a line, and moves on its own. It is placed
        (and the line drawn) by `place_notes` once the layout is known."""
        notes_ui["items"].append(dict(anchor=anchor, lines=list(lines), block=block_id, label=label, editable=editable, fn=getattr(self, "fn", None), card=getattr(self, "cur_card", None)))

    def expr(self, parent, e, cat="Values", expects=None):
        """`expects` is "bool" for a slot that wants a test (design doc 4.2:
        shape enforces type)."""
        k = e["k"]
        color = cat_color(cat)
        # Shapes (design doc 4.2): values are round, tests are angular (square
        # with a dark outline), types are square with a thick outline.
        if cat == "Types":
            shape = dict(radius=0, border=INK, border_width=2.0)
        elif k in ("compare", "boolop") or (k == "unary" and e["op"] == "not") or (k == "lit" and e["t"] == "bool"):
            shape = dict(radius=0, border=INK, border_width=1.0)
        else:
            shape = dict(radius=8)
        box = ui.add(ui.Panel, parent, direction=ROW, gap=2, align=CENTER, fill=color, padding=(4, 1, 4, 1), **shape)
        dnd[box.sid] = ("expr", e["id"], expects)
        ui.drop_zone(box, (2,))
        sel = blk["sel"] is not None and blk["sel"] == ("e", self.fn, e["id"])

        def pick():
            blk["sel"] = None if sel else ("e", self.fn, e["id"])
            view_dirty[0] = True

        def word(text):
            return ui.add(T.Badge, box, text=text, fill=0, color=INK)

        if k == "lit":
            t, v = e["t"], e["v"]
            if t == "none":
                word("None")
            elif t == "bool":
                b = button(box, "True" if v else "False", self.edit(lambda bb, e=e: bb.set_literal(e["id"], "bool", "false" if e["v"] else "true")), width=30)
            else:
                shown = repr(v) if t != "str" else v
                f = ui.text_input(box, shown, prefab=T.Field, width=max(18, 6 * len(shown) + 8))
                self.lits += 1
                handles[("lit", self.lits)] = f
                on_submit[f.sid] = self.edit(lambda bb, e=e, f=f: bb.set_literal(e["id"], e["t"], text_of(f)))
        elif k == "name" and cat == "Types" and B.is_any(e):
            # `any` turns checking off: it carries a visible marker (design doc 4.6)
            chip(box, "Any ?", 0xD8453CFF)
            ui.add(T.Badge, box, text="Checking off", fill=0, color=INK)
        elif k == "name":
            b = button(box, e["name"], pick, T.Item, width=6 * len(e["name"]) + 8)
        elif k == "attr":
            self.expr(box, e["value"], cat)
            word("." + e["attr"])
        elif k == "call":
            f = e["func"]
            if f["k"] == "attr":
                # The dynamic method call: the name is a free identifier, checked
                # when it is committed (design doc 4.5).
                self.expr(box, f["value"], "Calls")
                word(".")
                nm = ui.text_input(box, f["attr"], prefab=T.Field, width=max(24, 6 * len(f["attr"]) + 8))
                handles[("method", f["attr"])] = nm
                on_submit[nm.sid] = self.edit(lambda bb, f=f, nm=nm: bb.replace(f["id"], dict(_plain(f), attr=text_of(nm).strip())))
            else:
                self.expr(box, f, "Calls")
            word("(")
            for i, a in enumerate(e["args"]):
                self.expr(box, a, cat)
            for name, v in e["kwargs"]:
                word(name + "=")
                self.expr(box, v, cat)
            self.grow(box, e, "args", {"k": "lit", "t": "int", "v": 0})
            self.grow(box, e, "kwargs", ["arg", {"k": "lit", "t": "int", "v": 0}], hollow=True)
            word(")")
        elif k == "binop":
            want = None if e["op"] == "+" else "num"
            self.expr(box, e["l"], "Operators", expects=want)
            word({"*": "times", "**": "power", "//": "div", "%": "mod"}.get(e["op"], e["op"]))
            self.expr(box, e["r"], "Operators", expects=want)
        elif k == "unary":
            word(e["op"])
            self.expr(box, e["v"], "Operators")
        elif k == "boolop":
            for i, v in enumerate(e["values"]):
                if i:
                    word(e["op"])
                self.expr(box, v, "Operators", expects="bool")
        elif k == "compare":
            ordering = all(o in ("<", "<=", ">", ">=") for o in e["ops"])
            self.expr(box, e["l"], "Operators", expects="num" if ordering else None)
            for op, r in zip(e["ops"], e["rs"]):
                word(op)
                self.expr(box, r, "Operators", expects="num" if ordering else None)
        elif k == "ifexp":
            self.expr(box, e["then"], cat)
            word("if")
            self.expr(box, e["test"], "Operators")
            word("else")
            self.expr(box, e["else"], cat)
        elif k == "index":
            self.expr(box, e["value"], cat)
            word("[")
            self.expr(box, e["index"], cat)
            word("]")
        elif k == "list" and B.grid_shape(e):
            self.grid(box, e, expects)
        elif k in ("list", "tuple", "set"):
            # Every element of a typed list takes that type (design doc 4.4).
            elem = (B.element_slot(expects) or B.elem_type(e)) if k == "list" else None
            word({"list": "list [", "tuple": "tuple (", "set": "set {"}[k])
            for v in e["items"]:
                self.expr(box, v, cat, expects=elem)
            if k != "tuple":  # tuples are fixed-size (design doc 4.4)
                self.grow(box, e, "items", B.default_for(elem) if k == "list" else {"k": "lit", "t": "int", "v": 0})
            word({"list": "]", "tuple": ")", "set": "}"}[k])
        elif k == "lambda":
            word("lambda")
            for pn in e["params"]:
                chip(box, pn, 0x00000055)  # a scoped name: only usable inside
            word(":")
            self.expr(box, e["body"], cat)
        elif k == "comp":
            word({"list": "list of", "set": "set of", "gen": "each", "dict": "dict of"}[e["kind"]])
            if e["kind"] == "dict":
                self.expr(box, e["key"], cat)
                word(":")
                self.expr(box, e["value"], cat)
            else:
                self.expr(box, e["elt"], cat)
            for c in e["clauses"]:
                word("for")
                self.expr(box, c["target"], "Variables")
                word("in")
                self.expr(box, c["iter"], cat)
                for cond in c["ifs"]:
                    word("where")
                    self.expr(box, cond, "Operators")
        elif k == "dict":
            word("dict {")
            for key, v in e["items"]:
                self.expr(box, key, cat)
                word(":")
                self.expr(box, v, cat)
            self.grow(box, e, "items", [{"k": "lit", "t": "str", "v": "key"}, {"k": "lit", "t": "int", "v": 0}])
            word("}")
        else:
            word(k)
        if sel:
            box.set(Paint(color, 0xFFFFFFFF, 2.0, 0xFFFFFFFF))
        return box

    def grid(self, box, e, expects):
        """A list of equal lists as a grid you edit cell by cell, with a row
        or column added or dropped by the buttons: a tile map (design doc
        4.4)."""
        rows, cols = B.grid_shape(e)
        elem = B.element_slot(expects) or B.elem_type(e)
        cell_type = B.element_slot(elem)
        col = ui.add(ui.Panel, box, direction=COLUMN, gap=1)
        top = ui.add(ui.Panel, col, direction=ROW, gap=2, align=CENTER)
        ui.add(T.Badge, top, text="Grid %dx%d" % (rows, cols), fill=0, color=INK)
        for label, kw in (("+ Row", {"add_row": 1}), ("+ Col", {"add_col": 1}), ("- Row", {"add_row": -1}), ("- Col", {"add_col": -1})):
            b = button(top, label, self.edit(lambda bb, e=e, kw=kw: bb.replace(e["id"], B.grid_resize(e, **kw))), T.Item, width=6 * len(label) + 6)
            handles[("grid", label)] = b
        for r in e["items"]:
            line = ui.add(ui.Panel, col, direction=ROW, gap=1)
            for c in r["items"]:
                self.expr(line, c, "Values", expects=cell_type)

    def grow(self, box, e, key, new, hollow=False):
        """The plus button that adds an element (design doc 4.4, 4.5): a
        hollow plus adds a keyword argument. Elements can be removed with
        the minus beside it."""
        if key == "kwargs" and not hollow:
            return
        label = "=+" if hollow else "+"
        b = button(box, label, self.edit(lambda bb, e=e, key=key, new=new: bb.replace(e["id"], _grown(e, key, new))), T.Item, width=14)
        handles[("grow", e["k"], key)] = b
        if e.get(key) and not (key == "args" and False):
            button(box, "-", self.edit(lambda bb, e=e, key=key: bb.replace(e["id"], _shrunk(e, key))), T.Item, width=10)

    def slot(self, parent, e, cat="Values"):
        return self.expr(parent, e, cat)

    def stmt(self, parent, s, parent_id, field, index, count):
        sel = blk["sel"] == ("s", self.fn, s["id"])
        self.n += 1
        num = self.n
        cat = {"if": "Control", "while": "Control", "for": "Control", "return": "Control", "pass": "Control", "break": "Control", "continue": "Control"}.get(s["k"], STMT_CAT.get(s["k"], "Variables"))
        color = cat_color(cat)
        col = ui.add(ui.Panel, parent, direction=COLUMN, gap=1, align=START)
        self.cols[s["id"]] = col
        if any(B.uses_name(s, m) for m in self.missing):
            color = 0x6A6258FF  # greyed: the query no longer gives it what it uses
        head = ui.add(ui.Panel, col, direction=ROW, gap=3, align=CENTER, fill=color, radius=4, padding=(4, 2, 4, 2), wrap_flow=3, width=fill())
        if sel:
            head.set(Paint(color, 0xFFFFFFFF, 2.0, 0xFFFFFFFF))
        if self.why is not None and s["id"] == self.why["id"]:
            head.set(Paint(color, 0xD8453CFF, 3.0, 0xD8453CFF))
            ui.add(T.Badge, col, text="Why: " + (editor.error() or "").strip().split("\n")[-1][:60], fill=0xD8453CFF, color=0xF3EBE2FF)
        # Drag the block by its head onto another block to put it there; drop
        # a palette block on it to insert before it.
        if not s.get("elif"):
            ui.draggable(head, num, 1)
        ui.drop_zone(head, (1, 2))
        dnd[head.sid] = ("stmt", s["id"], parent_id, field, index)
        head.set(Context(1, block_menu.sid))
        self.heads.setdefault(self.cur_card.sid, []).append(head)
        if s.get("notes"):
            self.sticky(head, s["notes"], s["id"])
        if s.get("line"):
            glow["heads"][s["line"]] = (head, color)
        # A long value (a list, grid, call, comprehension...) goes on its own
        # line under the words, in the same color, so it is not cut off.
        cont = None
        if s["k"] in ("assign", "change", "declare") and s.get("value") and s["value"]["k"] in BIG:
            cont = ui.add(ui.Panel, col, direction=ROW, gap=3, align=CENTER, fill=color, radius=4, padding=(16, 2, 4, 2))
        # The notch under a command block (design doc 4.2). return, break and
        # continue have a flat bottom: nothing can go after them.
        if s["k"] not in TERMINAL:
            ui.add(ui.Panel, col, width=12, height=2, fill=color, offset=(10, 0))

        def word(text):
            return ui.add(T.Badge, head, text=text, fill=0, color=INK)

        k = s["k"]
        pick = button(head, ">", (lambda fn, sid: lambda: pick_block(fn, sid))(self.fn, s["id"]), T.Item, width=10)
        self.order.append((self.fn, s["id"]))
        self.picks[s["id"]] = pick
        handles[("pick", num)] = pick
        try:
            summary = B.stmt_text(_plain(s)).split("\n")[0]
        except (ValueError, KeyError):
            summary = s["k"]
        ui.role(pick, "button", name="Block %d: %s" % (num, summary))
        if self.is_stray and field == "body" and parent_id == self.cur_f["id"]:
            self.run_info[s["id"]] = (self.cur_f, index)
            handles[("stmt", num, "run_to")] = button(head, ">|", (lambda f, i: lambda: self.run_to(f, i))(self.cur_f, index), T.Item, width=14)
        if k == "assign":
            word("set")
            self.expr(head, s["target"], "Variables")
            word("to")
            self.expr(cont or head, s["value"], "Variables")
        elif k == "change":
            word("change")
            self.expr(head, s["target"], "Variables")
            word("by " + s["op"])
            self.expr(cont or head, s["value"], "Variables")
        elif k == "declare":
            word("declare " + s["name"] + ":")
            self.expr(head, s["type"], "Types", expects="type")
            if s["value"] is not None:
                word("=")
                self.expr(cont or head, s["value"], "Variables", expects=B.slot_type_for(s["type"]))
        elif k == "discard":
            word("do")
            self.expr(head, s["value"], "Calls")
        elif k == "if":
            word("else if" if s.get("elif") else "if")
            self.expr(head, s["test"], "Operators", expects="bool")
        elif k == "while":
            word("while")
            self.expr(head, s["test"], "Operators", expects="bool")
        elif k == "for":
            word("for each")
            self.expr(head, s["target"], "Variables")
            word("in")
            self.expr(head, s["iter"], "Variables")
        elif k == "return":
            word("return")
            if s["value"] is not None:
                self.expr(head, s["value"], "Variables")
        else:
            word(k)
        if s.get("trail"):
            ui.add(T.Muted, head, text="# " + s["trail"])
        if k in ("if", "while", "for"):
            inner = ui.add(ui.Panel, col, direction=COLUMN, gap=1, padding=(12, 0, 0, 0))
            self.stmts(inner, s["body"], s["id"], "body")
            if k == "if":
                if s["orelse"] and s["orelse"][0].get("elif"):
                    self.stmt(col, s["orelse"][0], s["id"], "orelse", 0, 1)
                elif s["orelse"]:
                    ui.add(T.Badge, col, text="else", fill=color, color=INK)
                    inner2 = ui.add(ui.Panel, col, direction=COLUMN, gap=1, padding=(12, 0, 0, 0))
                    self.stmts(inner2, s["orelse"], s["id"], "orelse")
                else:
                    button(col, "+ Else", self.edit(lambda bb, s=s: bb.add_else(s["id"])), width=40)
        return col

    def stmts(self, parent, stmts, parent_id, field):
        for i, s in enumerate(stmts):
            self.stmt(parent, s, parent_id, field, i, len(stmts))
        # The bottom of a body: a bar to aim at with the keyboard (Enter lands
        # on it, Ctrl+B inserts into it). An empty branch (or one holding only
        # `pass`) lights it up. A body that ends in return, break or continue
        # has no bar: nothing can go after those.
        if stmts and stmts[-1]["k"] in TERMINAL:
            return
        self.slots += 1
        empty = not stmts or all(x["k"] == "pass" for x in stmts)
        sel = blk["sel"] == ("b", self.fn, parent_id, field)
        bar = button(parent, "+ Here" if empty else "+", (lambda fn, pid, fld: lambda: pick_slot(fn, pid, fld))(self.fn, parent_id, field), T.Item, width=40 if empty else 14)
        if empty or sel:
            bar.set(Paint(0x2A241EFF, 0xFFFFFFFF if sel else 0xF28C28FF, 2.0, 0xFFFFFFFF))
        handles[("slot", self.slots)] = bar
        self.picks[("b", parent_id, field)] = bar
        self.order.append((self.fn, ("b", parent_id, field)))
        dnd[bar.sid] = ("slot", parent_id, field)
        ui.drop_zone(bar, (1, 2))
        ui.role(bar, "button", name="End of %s body%s" % (field, " (empty)" if empty else ""))

    def stray(self, card, head, f):
        """A stack with no trigger (design doc 4.1): double-click its title, or
        press RUN, to run the whole stack; RUN TO HERE on a statement runs it
        up to that block."""
        self.is_stray = True

        def run_all():
            err = editor.run_source(self.b.text(f["id"]), f["name"])
            say(("Ran " + f["name"]) if not err else "Error: " + err)

        title = button(head, "Run", run_all, T.Item, width=24)
        handles[("run", f["name"])] = title
        # Two quick clicks on the title also run it.
        clicks = self.double
        def maybe(cl=clicks):
            now = ticks[0]
            if now - cl.get(f["name"], -100) <= 30:
                run_all()
                cl[f["name"]] = -100
            else:
                cl[f["name"]] = now
        on_click[title.sid] = maybe

    def run_to(self, f, index):
        """Runs the stack's first `index + 1` top-level statements."""
        body = [_plain(s) for s in f["body"][: index + 1]]
        src = B.function_text(f["name"], [], [], body)
        err = editor.run_source(src, f["name"])
        say(("Ran to block %d" % (index + 1)) if not err else "Error: " + err)

    def function(self, parent, f, at):
        """One stack, placed on the canvas at `at` (design doc 4.1: the
        `@sage.position` decorator)."""
        self.fn = f["name"]
        self.cur_f = f
        self.is_stray = False
        card = ui.add(T.Card, parent, at=at, width=CARD_W)
        self.cur_card = card
        handles[("card", f["name"])] = card
        head = ui.add(ui.Panel, card, direction=ROW, gap=3, align=CENTER, fill=cat_color("Events"), radius=4, padding=(4, 2, 4, 2), wrap_flow=3, width=fill())
        # The whole stack moves when it is dragged by any part that is not a
        # block of its own; a block dragged from inside comes out of it.
        ui.draggable(card, 0, 3)
        move_grips[card.sid] = [f["name"], card, at[0], at[1]]
        dnd[card.sid] = ("fn", f["id"])
        ui.drop_zone(card, (1, 2))
        head.set(Context(1, stack_menu.sid))
        dnd[head.sid] = ("fnhead", f["id"])
        if f["notes"]:
            self.sticky(head, f["notes"], f["id"])
        if f.get("doc"):
            self.sticky(head, [f["doc"]], f["id"], label="Doc")
        ui.add(T.Badge, head, text="when", fill=0, color=INK)
        ui.add(T.Badge, head, text=f["name"], fill=0, color=INK)
        if f["locked"]:
            ui.add(T.Badge, head, text="Locked: " + f["locked"], fill=0xD8453CFF, color=0xF3EBE2FF)
            for line in self.b.text(f["id"]).split("\n"):
                ui.add(T.Muted, card, text=line)
            return
        for p in f["params"]:
            t = p["type"]["name"] if p["type"] and p["type"]["k"] == "name" else ""
            ui.add(T.Badge, head, text=("%s %s" % (t, p["name"])).strip(), fill=0x00000040, color=INK)
        decls = B.declarations(self.b.source)
        rel = B.relative_phases(decls)
        for d in f["decorators"]:
            if d["k"] == "call" and d["func"]["k"] == "attr" and d["func"]["attr"] == "system" and rel:
                # "run: [before resolve]": relative names for the system's
                # `phase=` (design doc 5.5)
                cur_phase = next((v["name"] for k, v in d["kwargs"] if k == "phase" and v["k"] == "name"), None)
                labels = ["Default"] + [lbl for lbl, _ in rel]
                values = [None] + [v for _, v in rel]
                at = values.index(cur_phase) if cur_phase in values else 0
                ui.add(T.Badge, head, text="Run", fill=0, color=INK)
                pick = ui.select(head, labels, value=at, width=84)
                handles[("system_phase", f["name"])] = pick
                on_select[pick.sid] = (lambda d, values: lambda i: self.edit(lambda bb: bb.replace(d["id"], _with_phase(d, values[i])))())(d, values)
            else:
                ui.add(T.Muted, head, text=B.expr_text({k: v for k, v in d.items()})[:40])
        fid = f["id"]
        # A stack whose query no longer provides what its blocks use: those
        # blocks grey out, with a one-click fix (design doc 4.7 fallback).
        self.missing = set()
        for issue in B.function_issues(f):
            self.missing.add(issue["param"])
            fix = button(card, "Add %s to the Query" % issue["component"], (lambda c: self.edit(lambda bb: B.add_to_query(bb, f["name"], c)))(issue["component"]), T.Danger, width=fill())
            handles[("fix_query", issue["component"])] = fix
        if not f["decorators"] and not f["params"]:
            self.stray(card, head, f)
        button(head, "Add Here", (lambda n: lambda: (blk.update(sel=("f", n, 0)), view_dirty.__setitem__(0, True)))(f["name"]), T.Item, width=48)
        self.stmts(ui.add(ui.Panel, card, direction=COLUMN, gap=1, padding=(10, 0, 0, 0), width=fill()), f["body"], fid, "body")


CARD_W = 270


def est_height(f):
    """A guess at a stack's height, to give stacks without a position a spot
    below the one before."""
    count = [0]

    def walk(b):
        if isinstance(b, dict):
            if b.get("k") in B.STATEMENTS:
                count[0] += 1
            for v in b.values():
                walk(v)
        elif isinstance(b, list):
            for v in b:
                walk(v)

    walk(f["body"])
    return 50 + 30 * count[0] + 45 * (len(f["notes"]) > 0) + 45 * bool(f.get("doc")) + (40 if f["locked"] else 0)
