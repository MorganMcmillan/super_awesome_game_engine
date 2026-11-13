# The editor's screen: UI entities over the engine's widget kit, talking to
# the document through the `editor` object the host gives this script.
#
#   top bar    undo, redo, play / stop, step, reload, save, status
#   left       the outliner: entities as a tree, add / duplicate / delete
#   center     tabs: SCENE (a viewport; click to select, drag to move),
#              CODE (the Python text, APPLY reloads it live) and
#              BLOCKS (the same Python as blocks)
#   right      the inspector, generated from the reflected components
#
# Views are rebuilt when `editor.revision()` changes, in a PostUpdate system,
# so an observer never changes the widgets it is reacting to. The document is
# the state; nothing here is saved.

import sage
from sage import Entity
from sage import blocks as B
from sage.ui import (
    Absolute,
    Interaction,
    Nav,
    TextField,
    Pointer,
    Rect,
    Box,
    Changed,
    Clicked,
    DragEnded,
    DragStarted,
    Dragged,
    Dropped,
    Display,
    Label,
    Paint,
    Selected,
    Submitted,
    TextChanged,
    Toggle,
    Toggled,
)

ui = world.ui
T = world.theme
COLUMN, ROW = ui.COLUMN, ui.ROW
START, CENTER, STRETCH = ui.START, ui.CENTER, ui.STRETCH
fill = ui.fill
ui.start()
NAV_ACCEPT, NAV_CANCEL, NAV_SHIFT = 1 << 2, 1 << 3, 1 << 14
TERMINAL = B.TERMINAL


def text_of(e):
    return world.string(e.get(Label).text)


def set_text(e, text):
    e.set(Label(world.intern(text)))


def clear(e):
    for child in list(e.children):
        child.destruct()


# ---- events: a view registers a handler for the entity it makes ---------------------------
on_click, on_change, on_submit, on_toggle, on_select = {}, {}, {}, {}, {}
fixed_click = {}  # handlers of the buttons that are never rebuilt
fixed_submit, fixed_text, fixed_toggle = {}, {}, {}
dnd = {}  # what each draggable / drop target in the block view means, by entity sid
move_grips = {}  # stack grips: sid -> [function name, card entity, x0, y0]
view_dirty = [True]
status_note = [""]


@world.observer(Clicked)
def _clicked(c: Clicked):
    h = on_click.get(c.target) or fixed_click.get(c.target)
    if h:
        h()


@world.observer(Changed)
def _changed(c: Changed):
    h = on_change.get(c.target)
    if h:
        h(c.value)


@world.observer(Submitted)
def _submitted(s: Submitted):
    h = on_submit.get(s.target) or fixed_submit.get(s.target)
    if h:
        h()


@world.observer(TextChanged)
def _text_changed(t: TextChanged):
    h = fixed_text.get(t.target)
    if h:
        h()


@world.observer(Toggled)
def _toggled(t: Toggled):
    h = on_toggle.get(t.target) or fixed_toggle.get(t.target)
    if h:
        h(bool(t.on))


@world.observer(Selected)
def _selected(s: Selected):
    h = on_select.get(s.target)
    if h:
        h(s.index)


def button(parent, text, handler, prefab=None, fixed=False, **style):
    b = ui.add(prefab or T.Secondary, parent, text=text, **style)
    (fixed_click if fixed else on_click)[b.sid] = handler
    return b


def say(text):
    status_note[0] = text
    view_dirty[0] = True


def guarded(fn):
    """Runs an edit; a refused one says why instead of crashing the editor."""

    def run(*args):
        try:
            err = fn(*args)
        except ValueError as e:
            err = str(e)
        if err:
            say(str(err))
        view_dirty[0] = True

    return run


# ---- the frame of the screen ----------------------------------------------------------------
app = ui.add(T.Surface)
bar = ui.add(T.Bar, app, height=18)
bar_left = ui.add(T.Row, bar, width=None, gap=3)
status = ui.add(T.Muted, bar, text="")
body = ui.add(T.Cols, app)
left = ui.add(T.Card, body, width=112, height=fill())
center = ui.add(T.Column, body, width=fill(), height=fill())
right = ui.add(T.Card, body, width=150, height=fill())

# top bar
b_undo = button(bar_left, "UNDO", guarded(lambda: editor.undo() and None), fixed=True, width=34)
b_redo = button(bar_left, "REDO", guarded(lambda: editor.redo() and None), fixed=True, width=34)
b_play = button(bar_left, "PLAY", lambda: editor.request("stop" if editor.playing() else "play"), T.Primary, fixed=True, width=40)
b_pause = button(bar_left, "PAUSE", lambda: editor.request("pause"), fixed=True, width=38)
b_step = button(bar_left, "STEP", lambda: editor.request("step"), fixed=True, width=34)
b_reload = button(bar_left, "RELOAD", lambda: editor.request("reload"), fixed=True, width=44)
b_save = button(bar_left, "SAVE", lambda: editor.request("save"), fixed=True, width=34)

# left: the outliner
ui.add(T.Heading, left, text="ENTITIES")
tools = ui.add(T.Row, left, gap=2)


def do_spawn():
    sel = editor.selection()
    editor.spawn("entity", sel)


button(tools, "ADD", guarded(lambda: do_spawn() and None), fixed=True, width=28)
button(tools, "COPY", guarded(lambda: editor.selection() is not None and editor.duplicate(editor.selection()) and None), fixed=True, width=30)
button(tools, "DEL", guarded(lambda: editor.selection() is not None and editor.despawn(editor.selection()) or None), T.Danger, fixed=True, width=28)
outliner = ui.scroll(left, direction=COLUMN, gap=1, width=fill(), height=fill())

# center: tabs
tabs_root, tab_buttons, tab_panels = ui.tabs(center, ["SCENE", "CODE", "BLOCKS", "TIMING", "PREFABS"], width=fill(), height=fill())
scene_panel, code_panel, blocks_panel, timing_panel, prefab_panel = tab_panels
scene_hint = ui.add(T.Muted, scene_panel, text="CLICK TO SELECT. DRAG TO MOVE.")
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
    err = editor.set_source(name, text)
    code_state["synced"] = text
    if err:
        say(err)
    editor.request("reload")


apply_button = button(code_head, "APPLY", apply_code, T.Primary, fixed=True, width=44)


def fixed_click_button(label):
    """A permanent button by its label (for tests)."""
    return {"APPLY": apply_button}[label]

# inspector
ui.add(T.Heading, right, text="INSPECTOR")
inspector = ui.scroll(right, direction=COLUMN, gap=3, width=fill(), height=fill())

# block editor panes
blk_head = ui.add(T.Row, blocks_panel, gap=3)
suggest_toggle = ui.toggle(blocks_panel, text="SUGGEST BY TYPE", on=(editor.setting("suggest_by_type", "on") == "on"))
fixed_toggle[suggest_toggle.sid] = lambda on: (editor.set_setting("suggest_by_type", "on" if on else "off"), view_dirty.__setitem__(0, True))
pal_filter = ui.text_input(blocks_panel, "", placeholder="FILTER BLOCKS: CTRL+B BLOCKS, CTRL+E VALUES", prefab=T.Field, width=fill())
fixed_text[pal_filter.sid] = lambda: view_dirty.__setitem__(0, True)
blk_palette = ui.add(ui.Panel, blocks_panel, direction=COLUMN, gap=2, width=fill())
blk_canvas = ui.add(ui.Panel, blocks_panel, direction=COLUMN, gap=2, width=fill())
blk = {"cat": "Control", "fn": None, "sel": None, "src": None, "why": False, "mode": None, "focus": None, "entries": []}
cur = {"view": None}
glow = {"heads": {}, "ttl": {}, "lit": set()}  # statement heads by source line, and how long each has glowed
drag_now = {"entry": None}


# ---- rebuilding the views -------------------------------------------------------------------
def build_status():
    mode = "PLAYING TICK %d" % editor.tick() if editor.playing() else "EDITING"
    bits = [mode, "UNSAVED" if editor.dirty() else "SAVED"]
    err = editor.error()
    note = status_note[0] or editor.log()
    if err:
        note = "ERROR: " + err.split("\n")[-1]
    text = "  ".join(bits) + ("  " + note if note else "")
    set_text(status, text[:96])
    set_text(b_play, "STOP" if editor.playing() else "PLAY")
    set_text(b_pause, "RESUME" if editor.paused() else "PAUSE")


def build_outliner():
    clear(outliner)
    sel = editor.selection()
    for sid, depth, label in editor.outliner():
        text = ("  " * depth) + label
        row = ui.add(T.Tab, outliner, text=text[:22])
        row.set(Toggle(1 if sid == sel else 0))
        on_toggle[row.sid] = (lambda s: lambda on: (editor.select(s), view_dirty.__setitem__(0, True)))(sid)


handles = {}  # entities by role, for tests and scripts: ("field", path, name), ("palette", label)...


def build_inspector():
    clear(inspector)
    for key in [k for k in handles if k[0] == "field"]:
        del handles[key]
    info = editor.inspect()
    if info is None:
        ui.add(T.Muted, inspector, text="NOTHING SELECTED")
        return
    sid, name, components, tags = info
    ui.add(T.Muted, inspector, text="ENTITY #%d" % sid)
    field = ui.text_input(inspector, name, placeholder="NAME", prefab=T.Field, width=fill())
    on_submit[field.sid] = guarded(lambda: editor.rename(sid, text_of(field)))
    for path, cname, fields in components:
        card = ui.add(T.Card, inspector)
        head = ui.add(T.Row, card, justify=ui.SPACE_BETWEEN)
        ui.add(T.Heading, head, text=cname)
        button(head, "X", guarded((lambda p: lambda: editor.remove_component(sid, p))(path)), T.Danger, width=12)
        for fname, kind, text in fields:
            row = ui.add(T.Row, card, gap=3)
            ui.add(T.Muted, row, text=fname, width=36)
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
                handles[("field", path.split(".")[-1].split("::")[-1], fname)] = n
                if kind in ("f32", "f64"):
                    on_change[n.sid] = guarded((lambda p, f: lambda v: editor.set_field(sid, p, f, repr(round(v, 4))))(path, fname))
                else:
                    on_change[n.sid] = guarded((lambda p, f: lambda v: editor.set_field(sid, p, f, str(int(v))))(path, fname))
    for tag in tags:
        row = ui.add(T.Row, inspector, justify=ui.SPACE_BETWEEN)
        ui.add(T.Chip, row, text=tag.split(".")[-1].split("::")[-1], width=fill())
        button(row, "X", guarded((lambda p: lambda: editor.remove_component(sid, p))(tag)), T.Danger, width=12)
    options = editor.addable()
    if options:
        sel = ui.select(inspector, ["ADD COMPONENT"] + [n for _, n in options], width=fill())
        on_select[sel.sid] = guarded(lambda i: editor.add_component(sid, options[i - 1][0]) if i > 0 else None)


def build_code():
    names = editor.sources()
    if code_state["name"] not in names:
        code_state["name"] = names[0] if names else None
        code_state["synced"] = None
    clear(code_names)
    for n in names:
        b = button(code_names, n.upper(), (lambda n: lambda: (code_state.update(name=n, synced=None), view_dirty.__setitem__(0, True)))(n), width=60)
    name = code_state["name"]
    if name is not None:
        source = editor.source(name)
        # Refill the text only when the source changed under it (a block edit,
        # a reload); never while the person is typing.
        if code_state["synced"] != source:
            code_area.set(Label(world.intern(source)))
            code_state["synced"] = source
    err = editor.error()
    set_text(code_error, ("ERROR: " + err) if err else ("RELOADED OK" if not editor.stale() else "CHANGES NOT APPLIED"))


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

    def edit(self, fn):
        def run():
            try:
                fn(self.b)
            except ValueError as e:
                say(str(e))
                view_dirty[0] = True
                return
            err = editor.set_source(self.name, self.b.source)
            if err:
                say(err)
            blk["sel"] = None
            view_dirty[0] = True

        return run

    def sticky(self, parent, lines, block_id, label=None):
        """Comments are sticky notes joined to the block below them."""
        row = ui.add(ui.Panel, parent, direction=ROW, gap=0, align=CENTER)
        note = ui.add(ui.Panel, row, direction=COLUMN, gap=1, fill=STICKY, padding=3, width=110)
        if label:
            ui.add(T.Badge, note, text=label, fill=0, color=STICKY_INK)
        for line in lines:
            ui.add(T.Para, note, text=line, color=STICKY_INK, wrap=True, width=fill())
        ui.add(ui.Panel, row, fill=STICKY, width=8, height=2)  # the string to its block
        return row

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
                word("NONE")
            elif t == "bool":
                b = button(box, "TRUE" if v else "FALSE", self.edit(lambda bb, e=e: bb.set_literal(e["id"], "bool", "false" if e["v"] else "true")), width=30)
            else:
                shown = repr(v) if t != "str" else v
                f = ui.text_input(box, shown, prefab=T.Field, width=max(18, 6 * len(shown) + 8))
                self.lits += 1
                handles[("lit", self.lits)] = f
                on_submit[f.sid] = self.edit(lambda bb, e=e, f=f: bb.set_literal(e["id"], e["t"], text_of(f)))
        elif k == "name" and cat == "Types" and B.is_any(e):
            # `any` turns checking off: it carries a visible marker (design doc 4.6)
            chip(box, "ANY ?", 0xD8453CFF)
            ui.add(T.Badge, box, text="CHECKING OFF", fill=0, color=INK)
        elif k == "name":
            b = button(box, e["name"].upper(), pick, T.Item, width=6 * len(e["name"]) + 8)
        elif k == "attr":
            self.expr(box, e["value"], cat)
            word("." + e["attr"].upper())
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
                word(name.upper() + "=")
                self.expr(box, v, cat)
            self.grow(box, e, "args", {"k": "lit", "t": "int", "v": 0})
            self.grow(box, e, "kwargs", ["arg", {"k": "lit", "t": "int", "v": 0}], hollow=True)
            word(")")
        elif k == "binop":
            want = None if e["op"] == "+" else "num"
            self.expr(box, e["l"], "Operators", expects=want)
            word({"*": "TIMES", "**": "POWER", "//": "DIV", "%": "MOD"}.get(e["op"], e["op"]))
            self.expr(box, e["r"], "Operators", expects=want)
        elif k == "unary":
            word(e["op"].upper())
            self.expr(box, e["v"], "Operators")
        elif k == "boolop":
            for i, v in enumerate(e["values"]):
                if i:
                    word(e["op"].upper())
                self.expr(box, v, "Operators", expects="bool")
        elif k == "compare":
            ordering = all(o in ("<", "<=", ">", ">=") for o in e["ops"])
            self.expr(box, e["l"], "Operators", expects="num" if ordering else None)
            for op, r in zip(e["ops"], e["rs"]):
                word(op.upper())
                self.expr(box, r, "Operators", expects="num" if ordering else None)
        elif k == "ifexp":
            self.expr(box, e["then"], cat)
            word("IF")
            self.expr(box, e["test"], "Operators")
            word("ELSE")
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
            word({"list": "LIST [", "tuple": "TUPLE (", "set": "SET {"}[k])
            for v in e["items"]:
                self.expr(box, v, cat, expects=elem)
            if k != "tuple":  # tuples are fixed-size (design doc 4.4)
                self.grow(box, e, "items", B.default_for(elem) if k == "list" else {"k": "lit", "t": "int", "v": 0})
            word({"list": "]", "tuple": ")", "set": "}"}[k])
        elif k == "lambda":
            word("LAMBDA")
            for pn in e["params"]:
                chip(box, pn.upper(), 0x00000055)  # a scoped name: only usable inside
            word(":")
            self.expr(box, e["body"], cat)
        elif k == "comp":
            word({"list": "LIST OF", "set": "SET OF", "gen": "EACH", "dict": "DICT OF"}[e["kind"]])
            if e["kind"] == "dict":
                self.expr(box, e["key"], cat)
                word(":")
                self.expr(box, e["value"], cat)
            else:
                self.expr(box, e["elt"], cat)
            for c in e["clauses"]:
                word("FOR")
                self.expr(box, c["target"], "Variables")
                word("IN")
                self.expr(box, c["iter"], cat)
                for cond in c["ifs"]:
                    word("WHERE")
                    self.expr(box, cond, "Operators")
        elif k == "dict":
            word("DICT {")
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
        ui.add(T.Badge, top, text="GRID %dx%d" % (rows, cols), fill=0, color=INK)
        for label, kw in (("+ROW", {"add_row": 1}), ("+COL", {"add_col": 1}), ("-ROW", {"add_row": -1}), ("-COL", {"add_col": -1})):
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
        if s.get("notes"):
            self.sticky(col, s["notes"], s["id"])
        if any(B.uses_name(s, m) for m in self.missing):
            color = 0x6A6258FF  # greyed: the query no longer gives it what it uses
        head = ui.add(ui.Panel, col, direction=ROW, gap=3, align=CENTER, fill=color, radius=4, padding=(4, 2, 4, 2), wrap_flow=3, width=fill())
        if sel:
            head.set(Paint(color, 0xFFFFFFFF, 2.0, 0xFFFFFFFF))
        if self.why is not None and s["id"] == self.why["id"]:
            head.set(Paint(color, 0xD8453CFF, 3.0, 0xD8453CFF))
            ui.add(T.Badge, col, text="WHY: " + (editor.error() or "").strip().split("\n")[-1][:60].upper(), fill=0xD8453CFF, color=0xF3EBE2FF)
        # Drag the block by its head onto another block to put it there; drop
        # a palette block on it to insert before it.
        ui.draggable(head, num, 1)
        ui.drop_zone(head, (1, 2))
        dnd[head.sid] = ("stmt", s["id"], parent_id, field, index)
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
        ui.role(pick, "button", name="BLOCK %d: %s" % (num, summary.upper()))
        # controls
        if not s.get("elif"):
            if index > 0:
                handles[("stmt", num, "up")] = button(head, "^", self.edit(lambda bb, s=s: bb.move(s["id"], parent_id, field, index - 1)), width=10)
            if index < count - 1:
                handles[("stmt", num, "down")] = button(head, "v", self.edit(lambda bb, s=s: bb.move(s["id"], parent_id, field, index + 2)), width=10)
        if self.is_stray and field == "body" and parent_id == self.cur_f["id"]:
            handles[("stmt", num, "run_to")] = button(head, ">|", (lambda f, i: lambda: self.run_to(f, i))(self.cur_f, index), T.Item, width=14)
        handles[("stmt", num, "note")] = button(head, "N", self.edit(lambda bb, s=s: bb.set_notes(s["id"], [] if s.get("notes") else ["NOTE"])), width=10)
        handles[("stmt", num, "del")] = button(head, "X", self.edit(lambda bb, s=s: bb.delete(s["id"])), T.Danger, width=10)
        if k == "assign":
            word("SET")
            self.expr(head, s["target"], "Variables")
            word("TO")
            self.expr(cont or head, s["value"], "Variables")
        elif k == "change":
            word("CHANGE")
            self.expr(head, s["target"], "Variables")
            word("BY " + s["op"])
            self.expr(cont or head, s["value"], "Variables")
        elif k == "declare":
            word("DECLARE " + s["name"].upper() + " :")
            self.expr(head, s["type"], "Types", expects="type")
            if s["value"] is not None:
                word("=")
                self.expr(cont or head, s["value"], "Variables", expects=B.slot_type_for(s["type"]))
        elif k == "discard":
            word("DO")
            self.expr(head, s["value"], "Calls")
        elif k == "if":
            word("ELSE IF" if s.get("elif") else "IF")
            self.expr(head, s["test"], "Operators", expects="bool")
        elif k == "while":
            word("WHILE")
            self.expr(head, s["test"], "Operators", expects="bool")
        elif k == "for":
            word("FOR EACH")
            self.expr(head, s["target"], "Variables")
            word("IN")
            self.expr(head, s["iter"], "Variables")
        elif k == "return":
            word("RETURN")
            if s["value"] is not None:
                self.expr(head, s["value"], "Variables")
        else:
            word(k.upper())
        if s.get("trail"):
            ui.add(T.Muted, head, text="# " + s["trail"])
        if k in ("if", "while", "for"):
            inner = ui.add(ui.Panel, col, direction=COLUMN, gap=1, padding=(12, 0, 0, 0))
            self.stmts(inner, s["body"], s["id"], "body")
            if k == "if":
                if s["orelse"] and s["orelse"][0].get("elif"):
                    self.stmt(col, s["orelse"][0], s["id"], "orelse", 0, 1)
                elif s["orelse"]:
                    ui.add(T.Badge, col, text="ELSE", fill=color, color=INK)
                    inner2 = ui.add(ui.Panel, col, direction=COLUMN, gap=1, padding=(12, 0, 0, 0))
                    self.stmts(inner2, s["orelse"], s["id"], "orelse")
                else:
                    button(col, "+ ELSE", self.edit(lambda bb, s=s: bb.add_else(s["id"])), width=40)
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
        bar = button(parent, "+ HERE" if empty else "+", (lambda fn, pid, fld: lambda: pick_slot(fn, pid, fld))(self.fn, parent_id, field), T.Item, width=40 if empty else 14)
        if empty or sel:
            bar.set(Paint(0x2A241EFF, 0xFFFFFFFF if sel else 0xF28C28FF, 2.0, 0xFFFFFFFF))
        handles[("slot", self.slots)] = bar
        self.picks[("b", parent_id, field)] = bar
        self.order.append((self.fn, ("b", parent_id, field)))
        dnd[bar.sid] = ("slot", parent_id, field)
        ui.drop_zone(bar, (1, 2))
        ui.role(bar, "button", name="END OF %s BODY%s" % (field.upper(), " (EMPTY)" if empty else ""))

    def stray(self, card, head, f):
        """A stack with no trigger (design doc 4.1): double-click its title, or
        press RUN, to run the whole stack; RUN TO HERE on a statement runs it
        up to that block."""
        self.is_stray = True

        def run_all():
            err = editor.run_source(self.b.text(f["id"]), f["name"])
            say(("RAN " + f["name"].upper()) if not err else "ERROR: " + err)

        title = button(head, "RUN", run_all, T.Item, width=24)
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
        say(("RAN TO BLOCK %d" % (index + 1)) if not err else "ERROR: " + err)

    def function(self, parent, f, at):
        """One stack, placed on the canvas at `at` (design doc 4.1: the
        `@sage.position` decorator)."""
        self.fn = f["name"]
        self.cur_f = f
        self.is_stray = False
        card = ui.add(T.Card, parent, at=at, width=CARD_W)
        handles[("card", f["name"])] = card
        if f["notes"]:
            self.sticky(card, f["notes"], f["id"])
        if f.get("doc"):
            self.sticky(card, [f["doc"]], f["id"], label="DOC")
        head = ui.add(ui.Panel, card, direction=ROW, gap=3, align=CENTER, fill=cat_color("Events"), radius=4, padding=(4, 2, 4, 2), wrap_flow=3, width=fill())
        grip = ui.add(ui.ResizeGrip, head)
        grip.set(Box(1, 10.0, 1, 10.0))
        move_grips[grip.sid] = [f["name"], card, at[0], at[1]]
        handles[("grip", f["name"])] = grip
        dnd[head.sid] = ("fn", f["id"])
        ui.drop_zone(head, (1, 2))
        ui.add(T.Badge, head, text="WHEN", fill=0, color=INK)
        ui.add(T.Badge, head, text=f["name"].upper(), fill=0, color=INK)
        if f["locked"]:
            ui.add(T.Badge, head, text="LOCKED: " + f["locked"].upper(), fill=0xD8453CFF, color=0xF3EBE2FF)
            for line in self.b.text(f["id"]).split("\n"):
                ui.add(T.Muted, card, text=line)
            return
        for p in f["params"]:
            t = p["type"]["name"] if p["type"] and p["type"]["k"] == "name" else ""
            ui.add(T.Badge, head, text=("%s %s" % (t, p["name"])).strip().upper(), fill=0x00000040, color=INK)
        decls = B.declarations(self.b.source)
        rel = B.relative_phases(decls)
        for d in f["decorators"]:
            if d["k"] == "call" and d["func"]["k"] == "attr" and d["func"]["attr"] == "system" and rel:
                # "run: [before resolve]": relative names for the system's
                # `phase=` (design doc 5.5)
                cur_phase = next((v["name"] for k, v in d["kwargs"] if k == "phase" and v["k"] == "name"), None)
                labels = ["DEFAULT"] + [lbl.upper() for lbl, _ in rel]
                values = [None] + [v for _, v in rel]
                at = values.index(cur_phase) if cur_phase in values else 0
                ui.add(T.Badge, head, text="RUN", fill=0, color=INK)
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
            fix = button(card, "! %s NOT IN QUERY: ADD IT" % issue["component"].upper(), (lambda c: self.edit(lambda bb: B.add_to_query(bb, f["name"], c)))(issue["component"]), T.Danger, width=fill())
            handles[("fix_query", issue["component"])] = fix
        if not f["decorators"] and not f["params"]:
            self.stray(card, head, f)
        button(head, "N", self.edit(lambda bb, f=f: bb.set_notes(f["id"], [] if f["notes"] else ["NOTE"])), width=10)
        button(head, "ADD HERE", (lambda n: lambda: (blk.update(sel=("f", n, 0)), view_dirty.__setitem__(0, True)))(f["name"]), T.Item, width=48)
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


@world.observer(Dragged)
def _grip_dragged(d: Dragged):
    info = move_grips.get(d.target)
    if info is None:
        return
    name, card, x0, y0 = info
    x, y = max(0, x0 + d.dx), max(0, y0 + d.dy)
    card.set(Absolute(float(x), float(y)))
    if d.phase == 2:
        # Released: the position is source text (`@sage.position`).
        view = cur["view"]
        if view is not None:
            view.edit(lambda bb: bb.set_position(name, int(x), int(y)))()


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


WANTS = {"bool": "A TEST", "num": "A NUMBER", "str": "TEXT", "type": "A TYPE"}


def wants_text(t):
    """What a slot says it wants: "A NUMBER", "A LIST OF NUMBERS" ..."""
    if t and t.startswith("list"):
        inner = B.element_slot(t)
        if inner is None:
            return "A LIST"
        if inner.startswith("list"):
            return "A LIST OF LISTS"
        return "A LIST OF " + {"num": "NUMBERS", "str": "TEXT", "bool": "TESTS"}.get(inner, "THINGS")
    return WANTS.get(t, "ANOTHER SHAPE")


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


@world.observer(DragEnded)
def _drag_ended(d: DragEnded):
    drag_now["entry"] = None
    unhighlight()


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
    view = cur["view"]
    if src is None or dst is None or view is None:
        return

    def go(bb):
        if dst[0] == "slot":
            if src[0] == "stmt":
                bb.move(src[1], dst[1], dst[2], 10**6)
            elif src[0] == "pal" and src[1]["shape"] == "command":
                bb.insert(dst[1], dst[2], 10**6, src[1]["block"])
            return
        if src[0] == "stmt":
            if dst[0] == "stmt":
                bb.move(src[1], dst[2], dst[3], dst[4])
            elif dst[0] == "fn":
                bb.move(src[1], dst[1], "body", 10**6)
        elif src[0] == "pal":
            entry = src[1]
            if entry["shape"] == "command":
                if dst[0] == "stmt":
                    bb.insert(dst[2], dst[3], dst[4], entry["block"])
                elif dst[0] == "fn":
                    bb.insert(dst[1], "body", 10**6, entry["block"])
            elif dst[0] == "expr":
                if not shape_ok(entry, dst[2]):
                    raise ValueError("THIS SLOT WANTS %s" % wants_text(dst[2]))
                bb.replace(dst[1], entry["block"])

    view.edit(go)()


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
                raise ValueError("A VALUE CANNOT GO IN A BODY")
            bb.insert(found[0]["id"], found[2], 10**6, block)
            return
        if shape == "command":
            if found is None:
                fn = next((i for i in bb.items if i["k"] == "function" and not i["locked"]), None)
                if fn is None:
                    raise ValueError("ADD A SYSTEM FIRST")
                bb.insert(fn["id"], "body", 10**6, block)
                return
            blockf, parent, field = found
            if blockf["k"] == "function":
                bb.insert(blockf["id"], "body", 10**6, blockf and block)
            elif blockf["k"] in B.STATEMENTS and parent is not None:
                i = parent[field].index(blockf)
                bb.insert(parent["id"], field, i + 1, block)
            else:
                raise ValueError("SELECT A STATEMENT FIRST")
        else:
            if found is None or found[0]["k"] in B.STATEMENTS or found[0]["k"] == "function":
                raise ValueError("SELECT THE VALUE TO REPLACE")
            bb.replace(found[0]["id"], block)

    view.edit(go)()


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
        ui.add(T.Muted, pal, text="SHOWING WHAT FITS THE SELECTION" + ((": " + wants_text(want)) if want else ""))
    for e in shown:
        shape = {"command": "", "reporter": "( ) ", "predicate": "< > ", "type": "[ ] "}[e["shape"]]
        label = (shape + e["label"].upper().replace("[]", "_"))[:26]
        row = button(pal, label, (lambda e: lambda: insert_from_palette(e))(e), T.Item, width=6 * len(label) + 8)
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
    ui.add(T.Heading, blk_head, text=blk["src"].upper() + ".PY")
    try:
        view = View(blk["src"])
    except ValueError as e:
        # The text is not valid Python right now (a half-typed edit): blocks
        # wait until it is, and the text is never touched.
        ui.add(T.Para, blk_canvas, text="THIS FILE IS NOT VALID PYTHON YET: " + str(e).upper(), wrap=True, width=fill())
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
    ui.add(T.Muted, pal_head, text="PALETTE")
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
        view.edit(lambda bb: bb.add_function(name, [(p, {"k": "name", "name": c}) for p, c in scope_], deco, None, (10, 10 + 40 * n)))()

    button(blk_head, "NEW SYSTEM", new_system, T.Primary, width=64)
    # The stack canvas: every function is a card at its `@sage.position`
    # (stacks without one are placed in a column), scrolled both ways.
    canvas = ui.scroll(blk_canvas, both=True, direction=COLUMN, gap=0, width=fill(), height=250)
    field = ui.add(ui.Panel, canvas, width=CARD_W + 700, height=900)
    handles["canvas"] = canvas
    if prog.get("doc"):
        view.sticky(field, [prog["doc"]], 0, label="FILE").set(Absolute(10.0, 0.0))
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
    fill_palette(*palette_ctx)
    want = blk.pop("focus", None)
    if want is not None:
        target = view.picks.get(want)
        if target is not None:
            set_focus(target)
    blk["focus"] = None
    hidden = [i for i in prog["items"] if i["k"] == "code"]
    if hidden:
        ui.add(T.Muted, blk_canvas, text="%d IMPORTS AND SETUP ITEMS: SEE CODE TAB" % len(hidden))


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
        ui.add(T.Para, box, text="NOT VALID PYTHON YET: " + str(e).upper(), width=fill())
        return
    if kind == "phase":
        timing_state["decls"] = decls
    mine = [d for d in decls if d["kind"] == kind]
    head = ui.add(T.Row, box, gap=4)
    ui.add(T.Muted, head, text=("TIMING: DRAG OUT > ONTO A PHASE TO RUN IT AFTER" if kind == "phase" else "PREFABS: TEMPLATES AND WHAT USES THEM"))
    if kind == "phase":
        for label, key, high in (("HOVER DELAY", "hover_delay", 120), ("PULSES", "pulse_cycles", 10)):
            ui.add(T.Muted, head, text=label)
            n = ui.number(head, settings[key], 0, high, 1, prefab=T.Field, width=30)
            on_change[n.sid] = (lambda key: lambda v: settings.__setitem__(key, int(v)))(key)
            handles[("setting", key)] = n
    button(head, "NEW PHASE" if kind == "phase" else "NEW PREFAB", guarded(lambda: add_decl(name, kind)), T.Primary, width=64)
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
            opts = ["NOTHING"] + others
            pick = ui.select(card, opts, value=(opts.index(d["ref"]) if d["ref"] in opts else 0), width=fill())
            ui.add(T.Muted, card, text="RUNS AFTER" if kind == "phase" else "DERIVES FROM")
            on_select[pick.sid] = guarded((lambda var, opts: lambda i: set_ref(name, var, None if i == 0 else opts[i]))(var, opts))
            handles[(kind, var)] = pick
            users = [x["name"] for x in systems if x[users_key] == var]
            ui.add(T.Muted, card, text=("SYSTEMS: " + ", ".join(users)) if users else "NO SYSTEMS")
            if kind == "prefab":
                ui.add(T.Muted, card, text="INSTANCES: %d" % editor.instances_of(d["name"] or var))
            else:
                hop = ui.add(T.Badge, card, text=" ", fill=0)
                out = ui.add(T.Badge, card, text="OUT >", fill=0xF28C28FF, color=INK)
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
        ui.add(T.Muted, box, text="NONE DECLARED YET: " + ("world.phase('Name', after=Other)" if kind == "phase" else "world.prefab('Name', base=Other)"))
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
    ui.add(T.Muted, box, text="BY MODULE (ROWS) AND PHASE (COLUMNS): DRAG A SYSTEM TO MOVE IT")
    cols = [None] + phases
    hdr = ui.add(T.Row, box, gap=2)
    ui.add(T.Badge, hdr, text="MODULE", fill=0, color=0xF3EBE2FF, width=70)
    for ph in cols:
        ui.add(T.Badge, hdr, text=(ph or "DEFAULT"), fill=0x00000040, color=0xF3EBE2FF, width=60)
    for mod in [None] + modules:
        label = "SCRIPT" if mod is None else mod
        r = ui.add(T.Row, box, gap=2, align=START)
        side = ui.add(ui.Panel, r, direction=COLUMN, gap=1, width=70)
        ui.add(T.Badge, side, text=label, fill=0x00000040, color=0xF3EBE2FF)
        on = module_enabled(mod, mod_names.get(mod, mod)) if mod else None
        if mod is not None:
            if on is False:
                ui.add(T.Badge, side, text="OFF", fill=0xD8453CFF, color=0xF3EBE2FF)
            t = button(side, "ENABLE" if on is False else "DISABLE", (lambda m, on: lambda: editor.set_module(m, on is False) or view_dirty.__setitem__(0, True))(mod, on), width=fill())
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
                        why = ui.add(T.Muted, cell, text="OFF: MODULE %s IS DISABLED" % mod.upper(), wrap=True, width=fill())
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
        want = ("HOVER" if k == 0 else "+%d" % k, bool(pulsing and cycle % width == k))
        if st["shown"].get(var) != want:
            set_text(hop, want[0])
            card.set(Paint(0x1B1714FF, 0xFFFFFFFF if want[1] else 0x3B3229FF, 3.0 if want[1] else 1.0, 0xF28C28FF))
            st["shown"][var] = want


def build_timing():
    build_decls(timing_box, "phase")


def build_prefabs():
    build_decls(prefab_box, "prefab")


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
        say("SELECT A BLOCK FIRST")
        return
    view, block, parent, field, i = sel
    count = len(parent[field])
    to = i - 1 if delta < 0 else i + 2
    if (delta < 0 and i == 0) or (delta > 0 and i >= count - 1):
        return
    view.edit(lambda bb: bb.move(block["id"], parent["id"], field, to))()


def key_delete():
    # Delete also edits text: a block is only deleted when focus is not in a
    # text field.
    fe = world.lookup(world.get(Interaction).focus)
    if fe is not None and fe.has(TextField):
        return
    sel = selected_stmt()
    if sel is None:
        say("SELECT A BLOCK FIRST")
        return
    view, block = sel[0], sel[1]
    view.edit(lambda bb: bb.delete(block["id"]))()


def key_note():
    sel = selected_stmt()
    if sel is None:
        say("SELECT A BLOCK FIRST")
        return
    view, block = sel[0], sel[1]
    view.edit(lambda bb: bb.set_notes(block["id"], [] if block.get("notes") else ["NOTE"]))()


def key_else():
    sel = selected_stmt()
    if sel is None or sel[1]["k"] != "if":
        say("SELECT AN IF BLOCK FIRST")
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


# The keys, as real buttons so each can be seen, tabbed to and pressed. The
# chords are the kit's (a letter, digit, F key, arrow or Delete).
keys = ui.add(T.Row, app, gap=2)
for _label, _chord, _fn in (
    ("BLOCKS CTRL+B", "Ctrl+B", lambda: open_palette("block")),
    ("VALUES CTRL+E", "Ctrl+E", lambda: open_palette("expr")),
    ("UP ALT+UP", "Alt+Up", lambda: key_move(-1)),
    ("DOWN ALT+DOWN", "Alt+Down", lambda: key_move(1)),
    ("DELETE DEL", "Delete", key_delete),
    ("NOTE ALT+N", "Alt+N", key_note),
    ("ELSE ALT+L", "Alt+L", key_else),
    ("PANES F6", "F6", lambda: next_pane(1)),
    ("BACK SHIFT+F6", "Shift+F6", lambda: next_pane(-1)),
):
    _b = button(keys, _label, _fn, T.Item, fixed=True, width=6 * len(_label) + 8)
    ui.shortcut(_b, _chord)
    handles[("key", _chord)] = _b
for _i, _tb in enumerate(tab_buttons):
    ui.shortcut(_tb, "Ctrl+%d" % (_i + 1))


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
    timing_hover()
    rev = editor.revision()
    changed = rev != seen[0] or view_dirty[0]
    seen[0] = rev
    view_dirty[0] = False
    rebuild_changed(changed)
    if changed:
        status_note[0] = ""
