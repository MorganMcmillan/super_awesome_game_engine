"""Flecs from Python: components, systems and observers (``sage.ecs``).

A script sees a ``world`` global while the engine loads it::

    import sage
    from sage import component, Entity

    @component
    class Velocity:
        x: sage.f32 = 0.0
        y: sage.f32 = 0.0

    physics = world.module("physics")

    @physics.system(phase="OnUpdate", query="!Frozen")
    def move(pos: Position, vel: Velocity):
        pos.x += vel.x * sage.TICK_DT

Systems are called once per matched entity, in StableId order. Parameters
annotated with a component class are query terms (``C | None`` is an
optional term); a parameter annotated ``Entity`` receives the entity;
unannotated parameters bind, in order, to the data terms of ``query=``,
which takes the full Flecs query language.

Component values are copied in by ``set`` and are plain values. Inside a
system or observer, parameters and ``entity.get(C)`` are *views*: writes go
straight into the world, and the view dies when the call returns. Use
``.copy()`` to keep the values.
"""

import inspect
import threading
import types
import typing

from sage import _ecs

Entity = _ecs.Entity
Value = _ecs.Value

TICK_DT = 1.0 / 60.0

# Per OS thread: each thread runs its own worlds. (threading.local does not
# work here: the engine attaches to Python from Rust threads, and each attach
# may get a fresh Python thread state.)
_stacks = {}


def _loading():
    return _stacks.setdefault(threading.get_ident(), [])


def _push_loading(world):
    _loading().append(world)


def _pop_loading():
    stack = _loading()
    stack.pop()
    if not stack:
        del _stacks[threading.get_ident()]


class _Kind:
    def __init__(self, name):
        self.name = name

    def __repr__(self):
        return f"sage.{self.name}"


f32 = _Kind("f32")
f64 = _Kind("f64")
i32 = _Kind("i32")
u32 = _Kind("u32")
i64 = _Kind("i64")
u64 = _Kind("u64")
u8 = _Kind("u8")

_PY_KINDS = {float: "f64", int: "i64", bool: "bool", Entity: "entity"}

# Built-in pipeline phases, by name.
OnLoad, PostLoad, PreUpdate, OnUpdate = "OnLoad", "PostLoad", "PreUpdate", "OnUpdate"
OnValidate, PostUpdate, PreStore, OnStore = "OnValidate", "PostUpdate", "PreStore", "OnStore"
# Built-in events.
OnAdd, OnSet, OnRemove = "OnAdd", "OnSet", "OnRemove"


def _kind_of(ann, where):
    if isinstance(ann, _Kind):
        return ann.name
    kind = _PY_KINDS.get(ann)
    if kind is None:
        raise TypeError(
            f"{where}: unsupported field type {ann!r} "
            "(use float, int, bool, Entity, or sage.f32/f64/i32/u32/i64/u64/u8; "
            "for arbitrary Python values use @component(opaque=True))"
        )
    return kind


def _current_world():
    stack = _loading()
    if not stack:
        raise RuntimeError("@component is only available while the engine loads a script")
    return stack[-1]


class Component(Value):
    """Base of every reflected component class."""

    __slots__ = ()
    __sage_fields__ = ()
    __sage_defaults__ = {}

    def __init__(self, *args, **kwargs):
        cls = type(self)
        names = [n for n, _ in cls.__sage_fields__]
        if len(args) > len(names):
            raise TypeError(f"{cls.__name__} takes at most {len(names)} positional values")
        values = dict(cls.__sage_defaults__)
        positional = dict(zip(names, args))
        values.update(positional)
        for key, value in kwargs.items():
            if key not in names:
                raise TypeError(f"{cls.__name__} has no field {key!r}")
            if key in positional:
                raise TypeError(f"{cls.__name__}: field {key!r} given twice")
            values[key] = value
        self._alloc(cls.__sage_size__)
        for key, value in values.items():
            setattr(self, key, value)

    def copy(self):
        """An owned copy of this value (or view)."""
        cls = type(self)
        return cls(**{n: getattr(self, n) for n, _ in cls.__sage_fields__})

    def __repr__(self):
        cls = type(self)
        try:
            body = ", ".join(f"{n}={getattr(self, n)!r}" for n, _ in cls.__sage_fields__)
        except ReferenceError:
            body = "<stale view>"
        return f"{cls.__name__}({body})"

    def __eq__(self, other):
        if type(self) is not type(other):
            return NotImplemented
        return all(getattr(self, n) == getattr(other, n) for n, _ in type(self).__sage_fields__)

    __hash__ = None

    @classmethod
    def system(cls, name=None, *, phase=OnUpdate, query=None, immediate=False):
        """A system over entities with this component; it binds to the first
        non-Entity parameter."""
        return _system_decorator(cls.__sage_world__, None, name, phase, query, immediate, implicit=cls)

    @classmethod
    def observer(cls, event, name=None, *, query=None):
        """An observer whose query includes this component."""
        return _observer_decorator(cls.__sage_world__, None, event, name, query, implicit=cls)


def _make_class(world, name, base, ns, cid, path, size, fields, defaults):
    ns = dict(ns)
    ns.setdefault("__slots__", ())
    for fname, kind, offset in fields:
        ns[fname] = world._native._field(fname, kind, offset)
    ns["__sage_id__"] = cid
    ns["__sage_path__"] = path
    ns["__sage_size__"] = size
    ns["__sage_fields__"] = tuple((f, k) for f, k, _ in fields)
    ns["__sage_defaults__"] = defaults
    ns["__sage_world__"] = world
    cls = type(name, (base,), ns)
    world._native._bind_class(cid, cls)
    return cls


def _define(world, cls, opaque):
    bases = [b for b in cls.__bases__ if b is not object]
    if len(bases) > 1:
        raise TypeError(f"{cls.__name__}: a component can have at most one base component")
    base = bases[0] if bases else None
    if base is not None and not (isinstance(base, type) and issubclass(base, Component) and "__sage_id__" in base.__dict__):
        raise TypeError(f"{cls.__name__}: its base {base.__name__} is not a @component class")
    own = dict(inspect.get_annotations(cls))
    for fname, ann in own.items():
        if isinstance(ann, str):
            raise TypeError(f"{cls.__name__}.{fname}: string annotations are not supported")
    native = world._native

    if opaque:
        if base is not None:
            raise TypeError(f"{cls.__name__}: opaque components cannot inherit")
        cid = native._define_opaque(cls.__name__)
        cls.__sage_id__ = cid
        cls.__sage_path__ = native._path(cid)
        cls.__sage_world__ = world
        native._bind_class(cid, cls)
        return cls

    fields = list(base.__sage_fields__) if base else []
    taken = {n for n, _ in fields}
    for fname, ann in own.items():
        if fname in taken:
            raise TypeError(f"{cls.__name__}.{fname} redefines a field of {base.__name__}")
        fields.append((fname, _kind_of(ann, f"{cls.__name__}.{fname}")))
    defaults = dict(base.__sage_defaults__) if base else {}
    for fname in own:
        if fname in cls.__dict__:
            defaults[fname] = cls.__dict__[fname]
    ns = {
        k: v
        for k, v in cls.__dict__.items()
        if k not in own and k not in ("__dict__", "__weakref__", "__annotations__")
    }
    if not fields:
        cid = native._define_tag(cls.__name__)
        size, offsets = 0, []
    else:
        cid, offsets, size = native._define_struct(
            cls.__name__, fields, base.__sage_id__ if base else None
        )
    return _make_class(world, cls.__name__, base or Component, ns, cid, native._path(cid), size, offsets, defaults)


def component(cls=None, *, opaque=False):
    """Declares a component. Annotated fields become a reflected Flecs struct;
    a class without fields is a tag. ``opaque=True`` stores a Python object
    instead (not hashed, snapshotted or visible to reflection)."""

    def wrap(cls):
        return _define(_current_world(), cls, opaque)

    return wrap if cls is None else wrap(cls)


def _class_for_native(world, cid):
    """Called by the binding the first time Python meets a native component."""
    existing = world._native._class(cid)
    if existing is not None:
        return existing
    cid, path, size, fields = world._native._reflect(cid)
    name = path.rsplit(".", 1)[-1]
    return _make_class(world, name, Component, {}, cid, path, size, fields, {})


# ----- native components as imports --------------------------------------------
#
# A host's native component at a path under `sage.` can be imported by that
# path: `from sage.collision import Position`, `from sage import Input`.
# Component classes belong to one world, but modules are shared by the whole
# process, so a module never stores the class: its `__getattr__` builds it
# for the world that is loading a script. That is why these imports work only
# while a script loads (at its top level). Inside a system, use the name the
# import bound.

_NATIVE_ROOT = "sage"
# Module name -> names of the native components registered under it, by any
# world in this process. Only used for membership tests.
_native_names = {}


def _native_attr(modname, name):
    if name.startswith("__") or name not in _native_names.get(modname, ()):
        raise AttributeError(f"module {modname!r} has no attribute {name!r}")
    path = f"{modname}.{name}"
    stack = _loading()
    # ImportError, not AttributeError: `from m import x` replaces an
    # AttributeError with a generic message.
    if not stack:
        raise ImportError(
            f"{path} is a native component; import it at the top level of a "
            "script (it can only be resolved while the engine loads a script)"
        )
    world = stack[-1]
    cid = world._native._lookup(path)
    if cid == 0:
        raise ImportError(f"{path} is not a component in this world")
    return _class_for_native(world, cid)


def _native_module(modname):
    """The module for `modname`, created (with its parents) if needed."""
    import sys

    module = sys.modules.get(modname)
    if module is not None:
        if modname != _NATIVE_ROOT and not getattr(module, "__sage_native__", False):
            raise ValueError(f"{modname} is not a namespace for native components")
        if "__getattr__" not in module.__dict__:
            module.__getattr__ = lambda name: _native_attr(modname, name)
        return module
    parent_name, _, leaf = modname.rpartition(".")
    parent = _native_module(parent_name)
    if leaf in parent.__dict__:
        raise ValueError(f"{modname}: {parent_name} already has an attribute {leaf!r}")
    module = types.ModuleType(modname, f"Native components under {modname} (registered by the host).")
    module.__sage_native__ = True
    module.__getattr__ = lambda name: _native_attr(modname, name)
    sys.modules[modname] = module
    setattr(parent, leaf, module)
    return module


def _expose_native(path):
    """Called by `Ecs::register_native`: makes the component at `path`
    importable if it is under `sage.`. Other paths stay reachable through
    `world.component_type`."""
    if not path.startswith(_NATIVE_ROOT + "."):
        return
    modname, _, name = path.rpartition(".")
    module = _native_module(modname)
    names = _native_names.setdefault(modname, set())
    if name not in names and name in module.__dict__:
        raise ValueError(f"{path}: {modname}.{name} is already defined by the engine")
    names.add(name)


def _is_component(ann):
    return isinstance(ann, type) and "__sage_id__" in getattr(ann, "__dict__", {}) or (
        isinstance(ann, type) and hasattr(ann, "__sage_id__")
    )


def _plan(fn, implicit, event_cls):
    """(terms, params) from a function signature."""
    where = f"{fn.__name__}"
    sig = inspect.signature(fn)
    hints = inspect.get_annotations(fn)
    terms, params = [], []
    implicit_pending = implicit is not None
    for p in sig.parameters.values():
        if p.kind not in (p.POSITIONAL_ONLY, p.POSITIONAL_OR_KEYWORD):
            raise TypeError(f"{where}: parameter {p.name!r} must be a plain positional parameter")
        ann = hints.get(p.name)
        if isinstance(ann, str):
            raise TypeError(f"{where}: string annotations are not supported ({p.name!r})")
        if ann is Entity:
            params.append(("entity", None, p.name))
            continue
        if event_cls is not None and ann is event_cls:
            params.append(("event", None, p.name))
            continue
        if implicit_pending and (ann is None or ann is implicit):
            implicit_pending = False
            params.append(("field", len(terms), p.name))
            terms.append(implicit.__sage_path__)
            continue
        optional = False
        if isinstance(ann, types.UnionType) or typing.get_origin(ann) is typing.Union:
            args = [a for a in ann.__args__ if a is not type(None)]
            if len(args) != 1 or len(ann.__args__) != 2:
                raise TypeError(f"{where}: parameter {p.name!r}: only `Component | None` unions are supported")
            ann, optional = args[0], True
        if ann is None:
            params.append(("field", None, p.name))
            continue
        if _is_component(ann):
            params.append(("field", len(terms), p.name))
            terms.append(("?" if optional else "") + ann.__sage_path__)
            continue
        raise TypeError(
            f"{where}: parameter {p.name!r}: {ann!r} is not a component "
            "(expected a @component class, Entity, or `C | None`)"
        )
    if implicit_pending:
        terms.append(implicit.__sage_path__)
    return terms, params


def _system_decorator(world, parent, name, phase, query, immediate, implicit=None, implicit_terms=()):
    """`implicit`: a component bound to the first parameter (`@Component.system`).
    `implicit_terms`: extra terms that bind nothing, such as `(IsA, prefab)`."""
    if callable(name) and not isinstance(name, str):
        # Used bare: @world.system
        return _system_decorator(world, parent, None, phase, query, immediate, implicit, implicit_terms)(name)
    native = world._native

    def wrap(fn):
        terms, params = _plan(fn, implicit, None)
        fixed = ["sage.StableId"] + terms + list(implicit_terms)
        params = [(k, None if i is None else i + 1, n) for k, i, n in params]
        expr = ", ".join(fixed + ([query] if query else []))
        native._create(
            "system", name or fn.__name__, parent, native._resolve_phase(phase), expr,
            len(fixed), params, fn, immediate, [], 0,
        )
        return fn

    return wrap


def _observer_decorator(world, parent, event, name, query, implicit=None, implicit_terms=()):
    native = world._native
    events = list(event) if isinstance(event, (list, tuple)) else [event]
    event_ids, event_cls = [], None
    for ev in events:
        if isinstance(ev, str):
            event_ids.append(native._resolve_event(ev))
        elif _is_component(ev):
            event_ids.append(ev.__sage_id__)
            event_cls = ev if event_cls is None else event_cls
        else:
            raise TypeError(f"observer event {ev!r} is not an event name or component class")

    def wrap(fn):
        terms, params = _plan(fn, implicit, event_cls)
        fixed = terms + list(implicit_terms)
        parts = fixed + ([query] if query else [])
        expr = ", ".join(parts) if parts else "_"
        native._create(
            "observer", name or fn.__name__, parent, 0, expr, len(fixed), params, fn,
            False, event_ids, event_cls.__sage_id__ if event_cls is not None else 0,
        )
        return fn

    return wrap


# TODO: why can't this inherit from Entity?
class Prefab:
    """A template entity (design doc 5.1). Instances point at it with
    ``(IsA, prefab)`` and get a copy of its components. Prefabs themselves are
    skipped by queries and systems."""

    def __init__(self, world, entity):
        self.world = world
        self.entity = entity

    @property
    def _term(self):
        return f"(IsA, {self.entity.path})"

    def system(self, name=None, *, phase=OnUpdate, query=None, immediate=False):
        """A system over this prefab's instances, including instances of
        prefabs derived from it. The implicit `(IsA, prefab)` term binds no
        parameter."""
        return _system_decorator(
            self.world, None, name, phase, query, immediate, implicit_terms=[self._term]
        )

    def observer(self, event, name=None, *, query=None):
        """An observer that only reacts on this prefab's instances."""
        return _observer_decorator(self.world, None, event, name, query, implicit_terms=[self._term])

    def instantiate(self, name=None, sid=None):
        """A new entity that is an instance of this prefab."""
        return self.world.entity(name, sid).is_a(self.entity)

    # The prefab's own components (copied into instances created after).
    def set(self, value):
        self.entity.set(value)
        return self

    def add(self, *items):
        self.entity.add(*items)
        return self

    def remove(self, *items):
        self.entity.remove(*items)
        return self

    def get(self, cls):
        return self.entity.get(cls)

    def has(self, *items):
        return self.entity.has(*items)

    def __repr__(self):
        return f"Prefab({self.entity.path})"


class FlecsModule:
    """A flecs module: a namespace for systems and observers that can be
    disabled as a whole (pause and feature-off, design doc 5.5)."""

    def __init__(self, world, entity):
        self.world = world
        self.entity = entity

    def system(self, name=None, *, phase=OnUpdate, query=None, immediate=False):
        return _system_decorator(self.world, self.entity.id, name, phase, query, immediate)

    def observer(self, event, name=None, *, query=None):
        return _observer_decorator(self.world, self.entity.id, event, name, query)

    def module(self, name):
        return FlecsModule(self.world, self.world._native._entity(self.world._native._module(name, self.entity.id)))

    def import_native(self, name, phase=OnUpdate):
        """Instantiates a native flecs module under this one."""
        native = self.world._native
        e = native._import_native(name, native._resolve_phase(phase), self.entity.id)
        return FlecsModule(self.world, native._entity(e))

    def enable(self):
        self.entity.enable()

    def disable(self):
        self.entity.disable()

    @property
    def enabled(self):
        return self.entity.enabled

    def __repr__(self):
        return f"FlecsModule({self.entity.path})"


class World:
    """The world a script runs in (the ``world`` global)."""

    def __init__(self, native):
        self._native = native

    # --- decorators -------------------------------------------------------
    def system(self, name=None, *, phase=OnUpdate, query=None, immediate=False):
        """Registers a system in the script's flecs module. Systems run in
        registration (source) order within a phase."""
        return _system_decorator(self, None, name, phase, query, immediate)

    def observer(self, event, name=None, *, query=None):
        """Registers an observer for `event` (a component class used as an
        event, or "OnAdd"/"OnSet"/"OnRemove")."""
        return _observer_decorator(self, None, event, name, query)

    # --- structure ----------------------------------------------------------
    def module(self, name):
        return FlecsModule(self, self._native._entity(self._native._module(name)))

    def import_native(self, name, phase=OnUpdate):
        native = self._native
        e = native._import_native(name, native._resolve_phase(phase), None)
        return FlecsModule(self, native._entity(e))

    def prefab(self, name, base=None, sid=None):
        """A prefab, optionally derived from another (`base`): instances of the
        derived prefab also match `@base.system`."""
        native = self._native
        b = None if base is None else base.entity.id
        return Prefab(self, native._entity(native._prefab(name, b, sid)))

    def phase(self, name, after=OnUpdate):
        """A pipeline phase that runs after `after` (design doc 5.5)."""
        native = self._native
        return native._entity(native._phase(name, native._resolve_phase(after)))

    def component_type(self, path):
        """The Python class of a native (Rust) reflected component. Components
        under `sage.` can also be imported by path (`from sage.collision
        import Position`)."""
        cid = self._native._lookup(path)
        if cid == 0:
            raise KeyError(f"no component at {path!r}")
        return _class_for_native(self, cid)

    # --- entities ---------------------------------------------------------
    def entity(self, name=None, sid=None):
        return self._native.entity(name, sid)

    def lookup(self, sid):
        return self._native.lookup(sid)

    def lookup_path(self, path):
        return self._native.lookup_path(path)

    def intern(self, text):
        """The id of `text` in the world's string table (0 for ""). Components
        store the id; the text is saved with scenes and save games."""
        return self._native.intern(text)

    def string(self, text_id):
        """The text for an id from `intern`, or None."""
        return self._native.string(text_id)

    def migrations(self, text):
        """Adds schema migrations in the ``sage-migrations v1`` text form."""
        self._native.migrations(text)

    def set(self, value):
        self._native.set(value)

    def get(self, cls):
        return self._native.get(cls)

    @property
    def rng(self):
        return self._native.rng

    @property
    def module_entity(self):
        return self._native._entity(self._native.script_module)

    def __repr__(self):
        return f"World({self._native._path(self._native.script_module)})"
