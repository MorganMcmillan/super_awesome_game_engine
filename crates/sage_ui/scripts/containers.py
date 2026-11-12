# Containers and forms, built from the widget kit: tabs, split panes,
# details, a tree, radio groups, `<label for>`, forms with validation, and
# templates with slots. Every piece of state is a component (so it is hashed
# and saved); the Python variables only hold entities.

import re

import sage
from sage import component
from sage.ui import (
    Absolute,
    Box,
    Clicked,
    Display,
    Dragged,
    Drag,
    Focused,
    Interaction,
    Label,
    Paint,
    Reveal,
    Rect,
    Scroll,
    Toggle,
    Toggled,
    Widget,
)

ui = world.ui
COLUMN, ROW = ui.COLUMN, ui.ROW
HANDLE, LABEL = 8, 9


def _text(e):
    return world.string(e.get(Label).text)


def _set_text(e, text):
    e.set(Label(world.intern(text)))


def _show(e, shown):
    e.set(Display(0 if shown else 1))


def _descendants(e):
    out = []
    for child in e.children:
        out.append(child)
        out.extend(_descendants(child))
    return out


# ---- tabs --------------------------------------------------------------------------
@component
class Tabs:
    selected: sage.i32 = 0
    count: sage.u32 = 0


@component
class TabOf:
    tabs: sage.u64 = 0
    index: sage.u32 = 0
    panel: sage.u64 = 0
    closed: sage.u8 = 0


@component
class TabClosed:
    tabs: sage.u64 = 0
    index: sage.u32 = 0


ui.TabButton = world.prefab("UiTabButton", base=ui.Choice)
ui.TabClose = world.prefab("UiTabClose", base=ui.Button)


def tabs(parent, titles, selected=0, closable=False, prefab=None, reorder=False, **style):
    """A tab container: a bar of toggle buttons over panels, one shown at a
    time. Returns `(root, buttons, panels)`; fill the panels. `closable` adds
    an X to each tab that hides it and emits `TabClosed`; `reorder` lets the
    tabs be dragged into a new order (`Moved` is emitted on the bar,
    `root.children[0]`)."""
    root = ui.add(ui.Panel, parent, direction=COLUMN, gap=4, **style)
    bar = ui.add(ui.Panel, root, direction=ROW, gap=2)
    ui.role(bar, "tablist")
    body = ui.add(ui.Panel, root, direction=COLUMN, width=ui.fill())
    buttons, panels = [], []
    for i, title in enumerate(titles):
        holder = ui.add(ui.Panel, bar, direction=ROW, gap=0) if closable else bar
        btn = ui.add(prefab or ui.TabButton, holder, text=title)
        panel = ui.add(ui.Panel, body, direction=COLUMN, gap=4, width=ui.fill(), display=(i == selected))
        btn.set(Toggle(1 if i == selected else 0))
        btn.set(TabOf(root.sid, i, panel.sid, 0))
        ui.role(btn, "tab")
        ui.role(panel, "tabpanel", labelled_by=btn)
        if closable:
            x = ui.add(ui.TabClose, holder, text="X")
            x.set(TabOf(root.sid, i, panel.sid, 1))
            ui.role(x, "button", name="CLOSE " + title)
        buttons.append(btn)
        panels.append(panel)
    root.set(Tabs(selected, len(titles)))
    if reorder:
        ui.reorderable(bar)
    return root, buttons, panels


def _tab_parts(root):
    """Every (button, panel) of a tab container, by index."""
    out = {}
    for e in _descendants(root):
        if e.has(TabOf):
            t = e.get(TabOf)
            if not t.closed:
                out[t.index] = (e, world.lookup(t.panel))
    return out


def select_tab(root, index):
    parts = _tab_parts(root)
    if index not in parts or _tab_hidden(parts[index][0]):
        return
    state = root.get(Tabs).copy()
    state.selected = index
    root.set(state)
    for i, (btn, panel) in parts.items():
        btn.set(Toggle(1 if i == index else 0))
        _show(panel, i == index)


ui.tabs, ui.select_tab = tabs, select_tab


def selected_tab(root):
    return root.get(Tabs).selected


ui.selected_tab = selected_tab


@world.observer(Toggled)
def _ui_tab_toggled(t: Toggled):
    e = world.lookup(t.target)
    if e is None or not e.has(TabOf) or e.get(TabOf).closed:
        return
    info = e.get(TabOf)
    root = world.lookup(info.tabs)
    # Clicking the selected tab keeps it selected; any other selects it.
    select_tab(root, info.index)


@world.observer(Clicked)
def _ui_tab_close(c: Clicked):
    e = world.lookup(c.target)
    if e is None or not e.has(TabOf) or not e.get(TabOf).closed:
        return
    info = e.get(TabOf)
    root = world.lookup(info.tabs)
    parts = _tab_parts(root)
    btn, panel = parts[info.index]
    _show(btn.parent, False)
    _show(panel, False)
    btn.set(Toggle(0))
    visible = [i for i, (b, _) in sorted(parts.items()) if i != info.index and not _tab_hidden(b)]
    if root.get(Tabs).selected == info.index and visible:
        select_tab(root, visible[0])
    root.emit(TabClosed(root.sid, info.index))


def _tab_hidden(btn):
    d = btn.parent.get(Display)
    return d is not None and d.none != 0


# ---- split panes ---------------------------------------------------------------------
@component
class Split:
    ratio: sage.f32 = 0.5
    start: sage.f32 = 0.5
    vertical: sage.u8 = 0
    min_a: sage.f32 = 16.0
    min_b: sage.f32 = 16.0
    handle: sage.f32 = 4.0


@component
class HandleOf:
    split: sage.u64 = 0


ui.SplitHandle = world.prefab("UiSplitHandle", base=ui.Panel)
ui.SplitHandle.set(Widget(HANDLE, 1, 0, 0)).set(Drag())


def _apply_ratio(root, ratio):
    s = root.get(Split)
    a, h, b = root.children[:3]
    key = "height" if s.vertical else "width"
    for pane, weight in ((a, ratio * 1000.0), (b, (1.0 - ratio) * 1000.0)):
        box = pane.get(Box).copy()
        setattr(box, key + "_kind", 2)
        setattr(box, key, max(weight, 0.001))
        pane.set(box)


def split(parent, vertical=False, ratio=0.5, min_a=16.0, min_b=16.0, handle=4.0, prefab=None, **style):
    """Two panes with a draggable divider. `vertical` stacks them (the divider
    is horizontal). Returns `(root, first, second)`. The divider's drag keeps
    both panes at least `min_a` and `min_b` units."""
    root = ui.add(ui.Panel, parent, direction=COLUMN if vertical else ROW, gap=0, align=ui.STRETCH, **style)
    a = ui.add(ui.Panel, root, direction=COLUMN, gap=4)
    h = ui.add(prefab or ui.SplitHandle, root)
    if vertical:
        h.set(Box(2, 1.0, 1, float(handle)))
    else:
        h.set(Box(1, float(handle), 2, 1.0))
    h.set(HandleOf(root.sid))
    b = ui.add(ui.Panel, root, direction=COLUMN, gap=4)
    for pane in (a, b):
        pane.set(Box(2, 1.0, 2, 1.0))
    root.set(Split(ratio, ratio, 1 if vertical else 0, float(min_a), float(min_b), float(handle)))
    _apply_ratio(root, ratio)
    return root, a, b


def split_ratio(root):
    return root.get(Split).ratio


def split_set(root, ratio):
    """Moves the divider (0 collapses the first pane, 1 the second)."""
    s = root.get(Split).copy()
    s.ratio = s.start = max(0.0, min(1.0, float(ratio)))
    root.set(s)
    _apply_ratio(root, s.ratio)


ui.split, ui.split_ratio, ui.split_set = split, split_ratio, split_set


@world.observer(Dragged)
def _ui_split_drag(d: Dragged):
    e = world.lookup(d.target)
    if e is None or not e.has(HandleOf):
        return
    root = world.lookup(e.get(HandleOf).split)
    s = root.get(Split).copy()
    if d.phase == 0:
        s.start = s.ratio
        root.set(s)
        return
    r = root.get(Rect)
    total = max((r.h if s.vertical else r.w) - s.handle, 1.0)
    delta = d.dy if s.vertical else d.dx
    lo, hi = s.min_a / total, 1.0 - s.min_b / total
    if lo > hi:
        lo = hi = 0.5
    s.ratio = max(lo, min(hi, s.start + delta / total))
    root.set(s)
    _apply_ratio(root, s.ratio)


# ---- details -------------------------------------------------------------------------
@component
class DetailsOf:
    body: sage.u64 = 0
    title: sage.u64 = 0


ui.DetailsHead = world.prefab("UiDetailsHead", base=ui.Choice)


def details(parent, summary, open=False, **style):
    """`<details>`: a header that shows or hides its body. Returns
    `(root, body)`."""
    root = ui.add(ui.Panel, parent, direction=COLUMN, gap=2, **style)
    head = ui.add(ui.DetailsHead, root, text=("- " if open else "+ ") + summary)
    body = ui.add(ui.Panel, root, direction=COLUMN, gap=4, width=ui.fill(), display=open)
    head.set(Toggle(1 if open else 0))
    head.set(DetailsOf(body.sid, world.intern(summary)))
    body.set(Reveal(1.0 if open else 0.0))
    ui.role(root, "group")
    ui.role(head, "button", name=summary)
    return root, body


def details_open(root):
    return bool(root.children[0].get(Toggle).on)


ui.details, ui.details_open = details, details_open


@world.observer(Toggled)
def _ui_details_toggled(t: Toggled):
    e = world.lookup(t.target)
    if e is None or not e.has(DetailsOf):
        return
    info = e.get(DetailsOf)
    # The body is shown from the first step of opening and hidden after the
    # last step of closing (`_ui_details_animate`), so the height animates.
    if t.on:
        _show(world.lookup(info.body), True)
    _set_text(e, ("- " if t.on else "+ ") + world.string(info.title))


# ---- tree ----------------------------------------------------------------------------
@component
class TreeState:
    selected: sage.u64 = 0


@component
class TreeRow:
    tree: sage.u64 = 0
    depth: sage.u32 = 0
    branch: sage.u8 = 0


@component
class ArrowOf:
    body: sage.u64 = 0
    row: sage.u64 = 0


@component
class TreeSelected:
    tree: sage.u64 = 0
    row: sage.u64 = 0


ui.TreeLabel = world.prefab("UiTreeLabel", base=ui.Choice)
ui.TreeArrow = world.prefab("UiTreeArrow", base=ui.Choice)


def _build_tree(root, parent, items, depth):
    for item in items:
        name, kids = (item, []) if isinstance(item, str) else (item[0], item[1])
        node = ui.add(ui.Panel, parent, direction=COLUMN, gap=1, width=ui.fill())
        row = ui.add(ui.Panel, node, direction=ROW, gap=2, align=ui.CENTER)
        arrow = None
        if kids:
            arrow = ui.add(ui.TreeArrow, row, text="+")
        else:
            ui.add(ui.Text, row, text=" ", width=8)
        label = ui.add(ui.TreeLabel, row, text=name)
        label.set(Toggle(0))
        label.set(TreeRow(root.sid, depth, 1 if kids else 0))
        ui.role(label, "treeitem", level=depth + 1)
        if kids:
            body = ui.add(ui.Panel, node, direction=COLUMN, gap=1, width=ui.fill(), margin=(10, 0, 0, 0), display=False)
            arrow.set(Toggle(0))
            arrow.set(ArrowOf(body.sid, label.sid))
            ui.role(arrow, "button", name="EXPAND " + name)
            _build_tree(root, body, kids, depth + 1)


def tree(parent, items, reorder=False, **style):
    """A tree view. `items` are names or `(name, [children])`. Click a name to
    select it, the arrow to expand a branch; Tab and the arrow keys move
    through the rows that are showing. `reorder` lets rows be dragged to a
    new place among their siblings (`Moved` is emitted on the root or the
    branch's body). Returns the root; read `ui.tree_selected(root)`."""
    root = ui.add(ui.Panel, parent, direction=COLUMN, gap=1, **style)
    root.set(TreeState(0))
    ui.role(root, "tree")
    _build_tree(root, root, items, 0)
    if reorder:
        _make_reorderable(root)
    return root


def _make_reorderable(container):
    """Every list of sibling rows in the tree can be reordered."""
    ui.reorderable(container)
    for node in container.children:
        if len(node.children) > 1:
            _make_reorderable(node.children[1])


def tree_selected(root):
    sid = root.get(TreeState).selected
    return world.lookup(sid) if sid else None


def tree_rows(root):
    """Every row's name entity, depth first."""
    return [e for e in _descendants(root) if e.has(TreeRow)]


def tree_expand(root, name, expand=True):
    """Expands or collapses the branch called `name`."""
    for e in _descendants(root):
        if e.has(ArrowOf) and _text(world.lookup(e.get(ArrowOf).row)) == name:
            _toggle_arrow(e, expand)


def _toggle_arrow(arrow, on):
    arrow.set(Toggle(1 if on else 0))
    _show(world.lookup(arrow.get(ArrowOf).body), on)
    _set_text(arrow, "-" if on else "+")


ui.tree, ui.tree_selected, ui.tree_rows, ui.tree_expand = tree, tree_selected, tree_rows, tree_expand


@world.observer(Toggled)
def _ui_tree_toggled(t: Toggled):
    e = world.lookup(t.target)
    if e is None:
        return
    if e.has(ArrowOf):
        _toggle_arrow(e, bool(t.on))
    elif e.has(TreeRow):
        info = e.get(TreeRow)
        root = world.lookup(info.tree)
        for row in tree_rows(root):
            row.set(Toggle(1 if row.sid == e.sid else 0))
        root.set(TreeState(e.sid))
        root.emit(TreeSelected(root.sid, e.sid))


# ---- radio buttons ---------------------------------------------------------------------
@component
class RadioGroup:
    selected: sage.i32 = -1
    count: sage.u32 = 0


@component
class RadioItem:
    group: sage.u64 = 0
    index: sage.u32 = 0


ui.RadioButton = world.prefab("UiRadioButton", base=ui.Choice)
ui.RadioButton.set(Box(1, 10.0, 1, 10.0))


def radio_group(parent, options, value=0, **style):
    """A group of radio buttons: exactly one is chosen. Returns
    `(group, buttons)`; read `ui.radio_value(group)`."""
    group = ui.add(ui.Panel, parent, direction=COLUMN, gap=3, **style)
    ui.role(group, "radiogroup")
    buttons = []
    for i, text in enumerate(options):
        row = ui.add(ui.Panel, group, direction=ROW, gap=4, align=ui.CENTER)
        btn = ui.add(ui.RadioButton, row, text="")
        btn.set(Toggle(1 if i == value else 0))
        btn.set(RadioItem(group.sid, i))
        ui.role(btn, "radio")
        label_for(row, text, btn)
        buttons.append(btn)
    group.set(RadioGroup(value, len(options)))
    return group, buttons


def radio_value(group):
    return group.get(RadioGroup).selected


def radio_set(group, index):
    g = group.get(RadioGroup).copy()
    g.selected = index
    group.set(g)
    for e in _descendants(group):
        if e.has(RadioItem):
            e.set(Toggle(1 if e.get(RadioItem).index == index else 0))


ui.radio_group, ui.radio_value, ui.radio_set = radio_group, radio_value, radio_set


@world.observer(Toggled)
def _ui_radio_toggled(t: Toggled):
    e = world.lookup(t.target)
    if e is None or not e.has(RadioItem):
        return
    info = e.get(RadioItem)
    radio_set(world.lookup(info.group), info.index)


# ---- label for ---------------------------------------------------------------------------
@component
class LabelFor:
    target: sage.u64 = 0


ui.LabelText = world.prefab("UiLabelText", base=ui.Text)
ui.LabelText.set(Widget(LABEL, 1, 0, 0))


def label_for(parent, text, control, **style):
    """`<label for>`: text that, when clicked, focuses a text field or
    flips a check box, radio button or toggle button."""
    e = ui.add(ui.LabelText, parent, text=text, **style)
    e.set(LabelFor(control.sid))
    ui.role(e, "label", label_for=control)
    return e


ui.label_for = label_for


@world.observer(Clicked)
def _ui_label_clicked(c: Clicked):
    e = world.lookup(c.target)
    if e is None or not e.has(LabelFor):
        return
    target = world.lookup(e.get(LabelFor).target)
    if target is None:
        return
    w = target.get(Widget)
    if w is not None and w.kind in (2, 7):  # a toggle or a choice
        on = 0 if target.get(Toggle).on else 1
        target.set(Toggle(on))
        target.emit(Toggled(target.sid, on))
    elif w is not None and w.kind == 1:
        target.emit(Clicked(target.sid))
    elif w is not None:
        i = world.get(Interaction).copy()
        i.focus = target.sid
        world.set(i)
        target.emit(Focused(target.sid))


# ---- forms ---------------------------------------------------------------------------------
@component
class Form:
    errors: sage.u32 = 0


@component
class FormField:
    form: sage.u64 = 0
    required: sage.u8 = 0
    numeric: sage.u8 = 0
    min_len: sage.u32 = 0
    max_len: sage.u32 = 0
    has_range: sage.u8 = 0
    low: sage.f32 = 0.0
    high: sage.f32 = 0.0
    pattern: sage.u64 = 0
    message: sage.u64 = 0
    error: sage.u64 = 0
    name: sage.u64 = 0
    initial: sage.u64 = 0


@component
class SubmitFor:
    form: sage.u64 = 0
    reset: sage.u8 = 0


@component
class FormSubmitted:
    form: sage.u64 = 0


@component
class FormInvalid:
    form: sage.u64 = 0
    count: sage.u32 = 0


ui.FormError = world.prefab("UiFormError", base=ui.Text)


def form(parent, **style):
    """A `<form>`: fields added with `ui.form_row` are validated when a button
    registered with `ui.form_submit` is clicked."""
    root = ui.add(ui.Panel, parent, direction=COLUMN, gap=5, **style)
    root.set(Form(0))
    ui.role(root, "form")
    return root


def form_row(form_, label, name, kind="text", *, required=False, min_len=0, max_len=0, pattern=None,
             low=None, high=None, message=None, placeholder="", value="", prefab=None, **style):
    """A labelled field with room for its error. `kind` is "text", "password"
    or "number". Returns the input."""
    row = ui.add(ui.Panel, form_, direction=COLUMN, gap=1, width=ui.fill())
    style.setdefault("width", ui.fill())
    label_row = ui.add(ui.Panel, row, direction=ROW)
    if kind == "number":
        inp = ui.number(row, value or 0, low=low or 0, high=high or 0, step=style.pop("step", 0), prefab=prefab, **style)
    else:
        inp = ui.text_input(row, str(value), placeholder=placeholder, max_len=max_len, password=(kind == "password"),
                            prefab=prefab, **style)
    label_for(label_row, label + (" *" if required else ""), inp)
    err = ui.add(ui.FormError, row, text="", display=False)
    inp.set(FormField(
        form=form_.sid, required=1 if required else 0, numeric=1 if kind == "number" else 0,
        min_len=min_len, max_len=max_len, has_range=1 if (low is not None or high is not None) else 0,
        low=float(low if low is not None else 0), high=float(high if high is not None else 0),
        pattern=world.intern(pattern) if pattern else 0, message=world.intern(message) if message else 0,
        error=err.sid, name=world.intern(name), initial=world.intern(str(value)),
    ))
    # Siblings flow in creation order: the label row came first, so the label
    # sits above its input, and the error below.
    return inp


def form_submit(form_, button, reset=False):
    """Makes `button` submit the form (or `reset` it to its first values)."""
    button.set(SubmitFor(form_.sid, 1 if reset else 0))


def _fields(form_):
    return [e for e in _descendants(form_) if e.has(FormField)]


def _problem(f):
    info = f.get(FormField)
    text = _text(f)
    if info.required and not text.strip():
        return "REQUIRED"
    if not text:
        return None
    if info.numeric:
        try:
            v = float(text)
        except ValueError:
            return "NOT A NUMBER"
        if info.has_range and not (info.low <= v <= info.high):
            return "BETWEEN %g AND %g" % (info.low, info.high)
    if info.min_len and len(text) < info.min_len:
        return "AT LEAST %d CHARACTERS" % info.min_len
    if info.max_len and len(text) > info.max_len:
        return "AT MOST %d CHARACTERS" % info.max_len
    if info.pattern and not re.fullmatch(world.string(info.pattern), text):
        return world.string(info.message) if info.message else "WRONG FORMAT"
    return None


def form_validate(form_):
    """Checks every field, shows the errors, and returns how many there are."""
    errors = 0
    for f in _fields(form_):
        err = world.lookup(f.get(FormField).error)
        problem = _problem(f)
        if problem:
            errors += 1
            _set_text(err, problem)
            _show(err, True)
        else:
            _show(err, False)
    state = form_.get(Form).copy()
    state.errors = errors
    form_.set(state)
    return errors


def form_values(form_):
    """`{name: text}` of every field."""
    return {world.string(f.get(FormField).name): _text(f) for f in _fields(form_)}


ui.form, ui.form_row, ui.form_submit = form, form_row, form_submit
ui.form_validate, ui.form_values = form_validate, form_values


@world.observer(Clicked)
def _ui_form_buttons(c: Clicked):
    b = world.lookup(c.target)
    if b is None or not b.has(SubmitFor):
        return
    info = b.get(SubmitFor)
    form_ = world.lookup(info.form)
    if info.reset:
        for f in _fields(form_):
            _set_text(f, world.string(f.get(FormField).initial))
            _show(world.lookup(f.get(FormField).error), False)
        form_.set(Form(0))
        return
    n = form_validate(form_)
    if n:
        form_.emit(FormInvalid(form_.sid, n))
    else:
        form_.emit(FormSubmitted(form_.sid))


# ---- templates and slots -----------------------------------------------------------------------
@component
class Slot:
    name: sage.u64 = 0


def slot(parent, name, **style):
    """A named place in a template where the caller's children go."""
    e = ui.add(ui.Panel, parent, **style)
    e.set(Slot(world.intern(name)))
    return e


def slot_of(root, name):
    """The slot called `name` inside `root`."""
    want = world.intern(name)
    for e in _descendants(root):
        if e.has(Slot) and e.get(Slot).name == want:
            return e
    return None


def instantiate(template, parent, *args, slots=None, **kwargs):
    """Builds `template(parent, *args, **kwargs)`, a function that returns its
    root and marks places with `ui.slot`, then fills each slot with
    `slots[name](slot_entity)`, which adds the caller's children."""
    root = template(parent, *args, **kwargs)
    for name, build in (slots or {}).items():
        target = slot_of(root, name)
        if target is None:
            raise KeyError("the template has no slot " + repr(name))
        build(target)
    return root


ui.slot, ui.slot_of, ui.instantiate = slot, slot_of, instantiate


def radio_item_of(e):
    """`(group sid, index)` of a radio button, or None."""
    if not e.has(RadioItem):
        return None
    i = e.get(RadioItem)
    return i.group, i.index


# Events the kit emits, for observers: `@world.observer(ui.TabClosed)`.
ui.radio_item_of = radio_item_of
ui.Form, ui.FormField, ui.TreeRow, ui.ArrowOf = Form, FormField, TreeRow, ArrowOf


def tree_arrows(root):
    """The arrow of every branch, depth first."""
    return [e for e in _descendants(root) if e.has(ArrowOf)]


ui.tree_arrows = tree_arrows
ui.TabClosed, ui.TreeSelected = TabClosed, TreeSelected
ui.FormSubmitted, ui.FormInvalid = FormSubmitted, FormInvalid


# ---- virtualized list ---------------------------------------------------------------------
@component
class VirtualList:
    """A long list drawn by a few rows: how many items, how tall a row is,
    which item the first row shows, and where the text comes from (a template
    with `%d`, or the items themselves joined by newlines)."""

    count: sage.u32 = 0
    row_h: sage.f32 = 10.0
    first: sage.i32 = -1
    template: sage.u64 = 0
    items: sage.u64 = 0
    inner: sage.u64 = 0
    rows: sage.u32 = 0


def virtual_list(parent, count=0, items=None, template="ITEM %d", rows=6, row_h=10, **style):
    """A scrolling list of any length. Only `rows + 1` row entities exist,
    reused as it scrolls, so a million items cost the same as ten. Give
    `items` (a list of strings) or a `template` with `%d` and a `count`.
    Returns the scroll container; read `ui.virtual_first(list)` for the item
    at the top, and `ui.virtual_row(list, k)` for a row's entity."""
    if items is not None:
        count = len(items)
    box = ui.scroll(parent, height=rows * row_h, **style)
    inner = ui.add(ui.Panel, box, width=ui.fill(), height=count * row_h)
    for k in range(rows + 1):
        ui.add(ui.Text, inner, text="", at=(0.0, float(k * row_h)), width=ui.fill(), height=row_h)
    box.set(VirtualList(count, float(row_h), -1, world.intern(template), world.intern("\n".join(items)) if items is not None else 0, inner.sid, rows))
    ui.role(box, "list")
    return box


def virtual_first(box):
    return box.get(VirtualList).first


def virtual_row(box, k):
    return world.lookup(box.get(VirtualList).inner).children[k]


def _virtual_text(v, i, lines):
    if lines is not None:
        return lines[i]
    return world.string(v.template) % i


ui.virtual_list, ui.virtual_first, ui.virtual_row = virtual_list, virtual_first, virtual_row
ui.VirtualState = VirtualList


@world.system(phase=sage.PreUpdate)
def _ui_virtual_rows(e: sage.Entity, v: VirtualList, s: Scroll):
    first = int(s.offset // v.row_h)
    if first == v.first:
        return
    nv = v.copy()
    nv.first = first
    e.set(nv)
    lines = world.string(v.items).split("\n") if v.items else None
    inner = world.lookup(v.inner)
    for k, row in enumerate(inner.children):
        i = first + k
        if i < v.count:
            text = _virtual_text(v, i, lines)
            row.set(Absolute(0.0, float(i * v.row_h)))
            if world.string(row.get(Label).text) != text:
                row.set(Label(world.intern(text)))
            ui.role(row, None, hidden=False)
        else:
            row.set(Label(world.intern("")))
            ui.role(row, None, hidden=True)


DETAILS_TICKS = 8


@world.system(phase=sage.PreUpdate)
def _ui_details_animate(e: sage.Entity, d: DetailsOf, tg: Toggle):
    body = world.lookup(d.body)
    t = body.get(Reveal).amount
    want = 1.0 if tg.on else 0.0
    if t == want:
        return
    t = min(want, t + 1.0 / DETAILS_TICKS) if want > t else max(want, t - 1.0 / DETAILS_TICKS)
    body.set(Reveal(t))
    if t == 0.0:
        _show(body, False)
