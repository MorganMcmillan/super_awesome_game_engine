# Popups for the UI kit (design doc 9.5): layers, dialogs, tooltips,
# drop-downs, combo boxes, menus and context menus. Loaded after widgets.py;
# it adds methods to `world.ui`.
#
# A layer is a node that floats above the rest, placed next to an anchor:
#
#     menu = ui.layer(button, placement="below", align="start", match_width=True)
#     ui.add(ui.Option, menu, text="One")
#     ui.open(menu)            # a press outside it, or Escape, closes it again
#
# Everything here keeps its state in components, so it is hashed and saved.

import sage
from sage import Entity, component
from sage.ui import (
    Combo,
    Context,
    ContextRequested,
    Display,
    Focused,
    Interaction,
    Label,
    Layer,
    MenuChosen,
    MenuItem,
    MenuTitle,
    Modal,
    Paint,
    Flow,
    Box,
    Caret,
    Clicked,
    Option,
    Screen,
    Select,
    Selected,
    TextChanged,
    TextStyle,
    Tooltip,
    Widget,
)

ui = world.ui

PLACE = {"below": 0, "above": 1, "right": 2, "left": 3, "point": 4}
ALIGN = {"start": 0, "center": 1, "end": 2}
FLIP, MATCH_WIDTH, DISMISS, AUTOFOCUS = 1, 2, 4, 8
CHECK, CHECKED, SEPARATOR, SUBMENU, DISABLED = 1, 2, 4, 8, 16

_COLUMN, _ROW, _START, _CENTER, _STRETCH, _BETWEEN = 0, 1, 0, 1, 3, 3
_screen = world.get(Screen)
_SCREEN_W, _SCREEN_H = _screen.w, _screen.h  # numbers, not the view


def _axis(width=None, height=None):
    def one(v):
        if v is None:
            return 0, 0.0
        if isinstance(v, ui.fill):
            return 2, v.weight
        return 1, float(v)

    (wk, w), (hk, h) = one(width), one(height)
    return Box(wk, w, hk, h)


def _prefab(name, base, *values):
    p = world.prefab(name, base=base)
    for v in values:
        p.set(v)
    return p


# ---- prefabs (a theme overrides them like any prefab) -------------------------------
ui.Popup = _prefab(
    "UiPopup",
    ui.Panel,
    _axis(),
    Flow(_COLUMN, _STRETCH, _START, 0.0, 2.0, 2.0, 2.0, 2.0),
    Paint(0x1B1F27FF, 0x3A4150FF, 1.0, 0x6AA9FFFF),
)
ui.Tip = _prefab(
    "UiTip",
    ui.Popup,
    Flow(_COLUMN, _START, _START, 0.0, 3.0, 2.0, 3.0, 2.0),
)
ui.Backdrop = _prefab(
    "UiBackdrop",
    ui.Panel,
    _axis(_SCREEN_W, _SCREEN_H),
    Flow(_COLUMN, _CENTER, _CENTER, 0.0, 0.0, 0.0, 0.0, 0.0),
    Paint(0x000000B8, 0, 0.0, 0x6AA9FFFF),
    Widget(5, 1, 0, 0),
)
ui.Dialog = _prefab(
    "UiDialog",
    ui.Panel,
    _axis(150, None),
    Flow(_COLUMN, _STRETCH, _START, 5.0, 8.0, 7.0, 8.0, 8.0),
    Paint(0x1B1F27FF, 0x6AA9FFFF, 1.0, 0x6AA9FFFF),
)
ui.Dropdown = _prefab(
    "UiDropdown",
    ui.Button,
    Flow(_ROW, _CENTER, _BETWEEN, 6.0, 4.0, 3.0, 4.0, 3.0),
)
ui.Option = _prefab("UiOption", ui.Button, _axis(ui.fill(), None))
ui.MenuTitle = _prefab("UiMenuTitle", ui.Button)
ui.MenuItem = _prefab(
    "UiMenuItem",
    ui.Button,
    _axis(ui.fill(), None),
    Flow(_ROW, _CENTER, _START, 6.0, 4.0, 3.0, 6.0, 3.0),
    Paint(0, 0, 0.0, 0x6AA9FFFF),
)
ui.MenuBar = _prefab(
    "UiMenuBar",
    ui.Panel,
    _axis(ui.fill(), None),
    Flow(_ROW, _CENTER, _START, 2.0, 2.0, 1.0, 2.0, 1.0),
)
ui.Sep = _prefab("UiSep", ui.Panel, _axis(ui.fill(), 1), Paint(0x3A4150FF, 0, 0.0, 0))


# ---- layers -------------------------------------------------------------------------
def layer(anchor=None, *, placement="below", align="start", gap=1.0, at=None, flip=True,
          match_width=False, dismiss=True, autofocus=True, open=False, prefab=None, **style):
    """A floating node next to `anchor` (an entity) or at `at` (x, y), closed
    unless `open`. `dismiss` closes it on a press outside it or on Escape;
    `autofocus` moves keyboard focus into it when it opens and back when it
    closes. Put widgets in it with `parent=layer`."""
    e = ui.add(prefab or ui.Popup, None, **style)
    flags = (FLIP if flip else 0) | (MATCH_WIDTH if match_width else 0)
    flags |= (DISMISS if dismiss else 0) | (AUTOFOCUS if autofocus else 0)
    x, y = (0.0, 0.0) if at is None else (float(at[0]), float(at[1]))
    if anchor is None:
        placement = "point"
    e.set(
        Layer(
            open=1 if open else 0,
            anchor=0 if anchor is None else anchor.sid,
            placement=PLACE[placement],
            align=ALIGN[align],
            gap=float(gap),
            x=x,
            y=y,
            flags=flags,
        )
    )
    return e


def open_layer(l, anchor=None, at=None, focus=None):
    """Opens a layer, optionally re-anchored, placed at a point, or with
    keyboard focus going to `focus` (an entity) instead of its first widget."""
    v = l.get(Layer).copy()
    v.open = 1
    if anchor is not None:
        v.anchor = anchor.sid
    if at is not None:
        v.x, v.y = float(at[0]), float(at[1])
    v.focus = 0 if focus is None else focus.sid
    l.set(v)


def close_layer(l):
    v = l.get(Layer).copy()
    v.open = 0
    l.set(v)


def is_open(l):
    return l.get(Layer).open != 0


ui.layer, ui.open, ui.close, ui.is_open = layer, open_layer, close_layer, is_open


def dialog(**style):
    """A modal dialog: returns (backdrop, box). Fill `box`, then `ui.open(backdrop)`.
    The backdrop blocks the pointer, keyboard focus stays inside the box, Escape
    closes it, and focus returns to what had it."""
    back = layer(None, at=(0, 0), gap=0.0, prefab=ui.Backdrop, dismiss=True, autofocus=True)
    back.set(Modal(1))
    box = ui.add(ui.Dialog, back, **style)
    return back, box


ui.dialog = dialog


# ---- tooltips -----------------------------------------------------------------------
@component
class TooltipState:
    target: sage.u64 = 0
    ticks: sage.u32 = 0
    shown: sage.u8 = 0


@component
class KitHost:
    """The entity the kit's per-tick systems run for."""


_tip_layer = layer(None, at=(0, 0), prefab=ui.Tip, dismiss=False, autofocus=False)
_tip_text = ui.add(ui.Text, _tip_layer, text="")
_kit = world.entity()
_kit.set(TooltipState())
_kit.add(KitHost)


def tooltip(entity, text, delay=24):
    """Show `text` after the pointer rests on `entity` (or anything inside it)
    for `delay` ticks."""
    entity.set(Tooltip(world.intern(text), delay))
    return entity


ui.tooltip = tooltip


@world.system(phase=sage.PostUpdate, query="KitHost")
def _ui_tooltips(e: Entity, st: TooltipState):
    inter = world.get(Interaction)
    hot, pressed = inter.hot, inter.active
    target = 0
    node = world.lookup(hot) if hot else None
    for _ in range(16):  # bubble up to the nearest node with a tooltip
        if node is None:
            break
        if node.has(Tooltip):
            target = node.sid
            break
        node = node.parent
    if pressed:
        target = 0
    if target != st.target:
        st.target, st.ticks = target, 0
        if st.shown:
            close_layer(_tip_layer)
            st.shown = 0
    elif target and not st.shown:
        st.ticks += 1
        tip = world.lookup(target)
        if st.ticks >= tip.get(Tooltip).delay:
            _tip_text.set(Label(tip.get(Tooltip).text))
            v = _tip_layer.get(Layer).copy()
            v.anchor, v.placement, v.gap, v.align = target, PLACE["below"], 2.0, ALIGN["start"]
            v.flags = FLIP
            _tip_layer.set(v)
            open_layer(_tip_layer)
            st.shown = 1


# ---- select (drop-down) and combo box -----------------------------------------------
def _label_of(e):
    return world.string(e.get(Label).text)


def select(parent, options, value=0, **style):
    """A drop-down list. `options` are strings; the button shows the chosen
    one. Reacts with `Selected` events (`target` is the returned entity)."""
    btn = ui.add(ui.Dropdown, parent, **style)
    shown = ui.add(ui.Text, btn, text=options[value] if options else "")
    ui.add(ui.Text, btn, text="V")
    popup = layer(btn, match_width=True, autofocus=True)
    for i, text in enumerate(options):
        item = ui.add(ui.Option, popup, text=text)
        item.set(Option(i, btn.sid))
    btn.set(Select(value, len(options), popup.sid, shown.sid))
    return btn


def combo(parent, options, text="", **style):
    """A text input with a list of suggestions that narrows as you type.
    Picking one fills the input and emits `Selected`."""
    inp = ui.add(ui.TextInput, parent, text=text, **style)
    popup = layer(inp, match_width=True, autofocus=False)
    for i, opt in enumerate(options):
        item = ui.add(ui.Option, popup, text=opt)
        item.set(Option(i, inp.sid))
    inp.set(Combo(popup.sid, len(options)))
    return inp


ui.select, ui.combo = select, combo


def _filter_combo(inp):
    """Hides the suggestions that do not contain the typed text and opens the
    list if any remain."""
    c = inp.get(Combo)
    popup = world.lookup(c.popup)
    needle = _label_of(inp).lower()
    any_left = False
    for opt in popup.children:
        match = needle in _label_of(opt).lower()
        opt.set(Display(0 if match else 1))
        any_left = any_left or match
    if any_left and not is_open(popup):
        open_layer(popup)
    elif not any_left and is_open(popup):
        close_layer(popup)


def _choose(owner, option):
    index = option.get(Option).index
    text = _label_of(option)
    if owner.has(Select):
        s = owner.get(Select).copy()
        s.value = index
        owner.set(s)
        world.lookup(s.label).set(Label(world.intern(text)))
        close_layer(world.lookup(s.popup))
    else:  # a combo box
        owner.set(Label(world.intern(text)))
        owner.set(Caret(len(text)))
        close_layer(world.lookup(owner.get(Combo).popup))
    owner.emit(Selected(owner.sid, index))


# ---- menus --------------------------------------------------------------------------
def _item(popup, spec):
    """One menu entry: "-" (a separator), "Text", or ("Text", {id, shortcut,
    check, checked, sub, disabled})."""
    if spec == "-":
        sep = ui.add(ui.Sep, popup)
        sep.set(MenuItem(0, SEPARATOR, 0, 0))
        return sep
    text, opts = (spec, {}) if isinstance(spec, str) else spec
    flags = 0
    if opts.get("check") or opts.get("checked"):
        flags |= CHECK | (CHECKED if opts.get("checked") else 0)
    if opts.get("disabled"):
        flags |= DISABLED
    row = ui.add(ui.MenuItem, popup, enabled=not opts.get("disabled", False))
    mark = ui.add(ui.Text, row, text="* " if flags & CHECKED else "  ", width=10)
    ui.add(ui.Text, row, text=text, width=ui.fill())
    if opts.get("shortcut"):
        ui.add(ui.Text, row, text=opts["shortcut"], color=0x7C7468FF)
    sub_sid = 0
    if opts.get("sub"):
        flags |= SUBMENU
        ui.add(ui.Text, row, text=">")
        sub = _menu_popup(row, opts["sub"], placement="right")
        sub_sid = sub.sid
    row.set(MenuItem(opts.get("id", 0), flags, sub_sid, mark.sid))
    return row


def _menu_popup(anchor, items, placement="below", at=None):
    popup = layer(anchor, placement=placement, at=at, gap=0.0, autofocus=True)
    for spec in items:
        _item(popup, spec)
    return popup


def menu_bar(parent=None, **style):
    return ui.add(ui.MenuBar, parent, **style)


def menu(bar, title, items):
    """A top-level menu in `bar`. Returns the title button. Choosing an item
    emits `MenuChosen` on it (`id` is the item's `id`)."""
    t = ui.add(ui.MenuTitle, bar, text=title)
    popup = _menu_popup(t, items)
    t.set(MenuTitle(popup.sid, bar.sid))
    return t


def context_menu(target, items):
    """A menu that opens at the pointer on a secondary press over `target`."""
    popup = _menu_popup(None, items)
    target.set(Context(1, popup.sid))
    return popup


ui.menu_bar, ui.menu, ui.context_menu = menu_bar, menu, context_menu


def _close_tree(popup):
    """Closes a menu popup and every submenu below it."""
    close_layer(popup)
    for child in popup.children:
        if child.has(MenuItem):
            sub = child.get(MenuItem).sub
            if sub:
                s = world.lookup(sub)
                if s is not None and is_open(s):
                    _close_tree(s)


def _root_popup(item):
    """The top menu layer an item belongs to, following submenu anchors."""
    n = item
    for _ in range(16):
        parent = n.parent
        if parent is None:
            lay = n.get(Layer)
            if lay is not None and lay.anchor:
                anchor = world.lookup(lay.anchor)
                if anchor is not None and anchor.has(MenuItem):
                    n = anchor  # a submenu: continue from the item that opened it
                    continue
            return n
        n = parent
    return n


def _choose_item(item):
    m = item.get(MenuItem)
    flags = m.flags
    if flags & (SEPARATOR | DISABLED):
        return
    if flags & SUBMENU:
        sub = world.lookup(m.sub)
        if not is_open(sub):
            open_layer(sub)
        return
    if flags & CHECK:
        flags ^= CHECKED
        v = m.copy()
        v.flags = flags
        item.set(v)
        world.lookup(m.mark).set(Label(world.intern("* " if flags & CHECKED else "  ")))
    item.emit(MenuChosen(item.sid, m.id))
    _close_tree(_root_popup(item))


# ---- one observer for everything above ---------------------------------------------
@world.observer(Clicked)
def _ui_popup_clicks(c: Clicked):
    e = world.lookup(c.target)
    if e is None:
        return
    if e.has(Select):
        popup = world.lookup(e.get(Select).popup)
        if is_open(popup):
            close_layer(popup)
        else:
            idx = e.get(Select).value
            kids = popup.children
            open_layer(popup, focus=kids[idx] if 0 <= idx < len(kids) else None)
    elif e.has(Option):
        owner = world.lookup(e.get(Option).owner)
        if owner is not None:
            _choose(owner, e)
    elif e.has(MenuItem):
        _choose_item(e)
    elif e.has(MenuTitle):
        popup = world.lookup(e.get(MenuTitle).popup)
        if is_open(popup):
            _close_tree(popup)
        else:
            open_layer(popup)


@world.observer(Focused)
def _ui_popup_focus(f: Focused):
    e = world.lookup(f.target)
    if e is not None and e.has(Combo):
        _filter_combo(e)


@world.observer(TextChanged)
def _ui_popup_text(t: TextChanged):
    e = world.lookup(t.target)
    if e is not None and e.has(Combo):
        _filter_combo(e)


@world.observer(ContextRequested)
def _ui_popup_context(c: ContextRequested):
    e = world.lookup(c.target)
    if e is not None and e.has(Context):
        popup = world.lookup(e.get(Context).popup)
        if popup is not None:
            open_layer(popup, at=(c.x, c.y))


@world.system(phase=sage.PostUpdate, query="KitHost")
def _ui_menu_hover(e: Entity):
    """Moving over another menu title switches menus while one is open;
    moving over an item with a submenu opens it and closes its siblings'."""
    hot = world.get(Interaction).hot
    h = world.lookup(hot) if hot else None
    if h is None:
        return
    if h.has(MenuTitle) and h.parent is not None:
        mine = world.lookup(h.get(MenuTitle).popup)
        if is_open(mine):
            return
        for sibling in h.parent.children:
            if sibling.has(MenuTitle):
                other = world.lookup(sibling.get(MenuTitle).popup)
                if other is not None and is_open(other):
                    _close_tree(other)
                    open_layer(mine)
                    return
    elif h.has(MenuItem) and h.parent is not None:
        m = h.get(MenuItem)
        for sibling in h.parent.children:
            if sibling.sid != h.sid and sibling.has(MenuItem):
                sub = sibling.get(MenuItem).sub
                if sub:
                    s = world.lookup(sub)
                    if s is not None and is_open(s):
                        _close_tree(s)
        if m.flags & SUBMENU and not m.flags & DISABLED:
            sub = world.lookup(m.sub)
            if not is_open(sub):
                open_layer(sub, focus=None)
