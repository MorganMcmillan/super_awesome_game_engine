# Type stub for `sage.ecs` (ecs.py beside it). Editors read this instead of
# ecs.py; the engine never does. It describes the script-facing API in terms
# a type checker can follow, which the runtime code is too dynamic for:
#
# - Field kinds (`sage.f32`, ...) are markers at run time; here they are
#   aliases for the Python type a field holds, so `x: sage.f32 = 0.0` checks.
# - `@component` builds the class at run time; here it is a dataclass
#   transform, so `Player(LEFT)` and `Velocity(x=1.0)` get constructor hints.
#
# Keep it in step with ecs.py.

from typing import Any, Callable, Self, TypeAlias, TypeVar, dataclass_transform, overload

from sage._ecs import Entity as Entity, Pair as Pair, Value as Value, WorldRng

_T = TypeVar("_T")
_F = TypeVar("_F", bound=Callable[..., Any])

TICK_DT: float

f32: TypeAlias = float
f64: TypeAlias = float
i32: TypeAlias = int
u32: TypeAlias = int
i64: TypeAlias = int
u64: TypeAlias = int
u8: TypeAlias = int

# A pipeline phase: a built-in phase name or an entity from `world.phase`.
Phase: TypeAlias = str | Entity
# An observer event: "OnAdd"/"OnSet"/"OnRemove", or a component class.
Event: TypeAlias = str | type[Any]

OnLoad: str
PostLoad: str
PreUpdate: str
OnUpdate: str
OnValidate: str
PostUpdate: str
PreStore: str
OnStore: str
OnAdd: str
OnSet: str
OnRemove: str

class Component(Value):
    """Base of every reflected component class. Classes from
    `world.component_type` are typed as this; their fields are dynamic."""

    def __init__(self, *args: Any, **kwargs: Any) -> None: ...
    def copy(self) -> Self:
        """An owned copy of this value (or view)."""
    def __getattr__(self, name: str) -> Any: ...
    def __setattr__(self, name: str, value: Any) -> None: ...
    @overload
    @classmethod
    def system(cls, name: _F, /) -> _F: ...
    @overload
    @classmethod
    def system(
        cls, name: str | None = None, *, phase: Phase = ..., query: str | None = None, immediate: bool = False
    ) -> Callable[[_F], _F]: ...
    @classmethod
    def observer(cls, event: Event | list[Event] | tuple[Event, ...], name: str | None = None, *, query: str | None = None) -> Callable[[_F], _F]: ...

@overload
@dataclass_transform()
def component(cls: type[_T], /) -> type[_T]:
    """Declares a component. Annotated fields become a reflected Flecs struct;
    a class without fields is a tag. ``opaque=True`` stores a Python object
    instead (not hashed, snapshotted or visible to reflection)."""
@overload
def component(*, opaque: bool = False) -> Callable[[type[_T]], type[_T]]: ...

class Prefab:
    """A template entity (design doc 5.1). Instances point at it with
    ``(IsA, prefab)`` and get a copy of its components. Prefabs themselves are
    skipped by queries and systems."""

    world: World
    entity: Entity
    @overload
    def system(self, name: _F, /) -> _F: ...
    @overload
    def system(
        self, name: str | None = None, *, phase: Phase = ..., query: str | None = None, immediate: bool = False
    ) -> Callable[[_F], _F]: ...
    def observer(self, event: Event | list[Event] | tuple[Event, ...], name: str | None = None, *, query: str | None = None) -> Callable[[_F], _F]: ...
    def instantiate(self, name: str | None = None, sid: int | None = None) -> Entity: ...
    def set(self, value: object) -> Self: ...
    def add(self, *items: object) -> Self: ...
    def remove(self, *items: type[Any] | Entity | Pair | object) -> Self: ...
    def get(self, cls: type[_T]) -> _T | None: ...
    def has(self, *items: type[Any] | Entity | Pair | object) -> bool: ...

class FlecsModule:
    """A flecs module: a namespace for systems and observers that can be
    disabled as a whole (pause and feature-off, design doc 5.5)."""

    world: World
    entity: Entity
    @overload
    def system(self, name: _F, /) -> _F: ...
    @overload
    def system(
        self, name: str | None = None, *, phase: Phase = ..., query: str | None = None, immediate: bool = False
    ) -> Callable[[_F], _F]: ...
    def observer(self, event: Event | list[Event] | tuple[Event, ...], name: str | None = None, *, query: str | None = None) -> Callable[[_F], _F]: ...
    def module(self, name: str) -> FlecsModule: ...
    def import_native(self, name: str, phase: Phase = ...) -> FlecsModule:
        """Instantiates a native flecs module under this one."""
    def enable(self) -> None: ...
    def disable(self) -> None: ...
    @property
    def enabled(self) -> bool: ...

class World:
    """The world a script runs in (the ``world`` global)."""

    @overload
    def system(self, name: _F, /) -> _F: ...
    @overload
    def system(
        self, name: str | None = None, *, phase: Phase = ..., query: str | None = None, immediate: bool = False
    ) -> Callable[[_F], _F]:
        """Registers a system in the script's flecs module. Systems run in
        registration (source) order within a phase."""
    def observer(self, event: Event | list[Event] | tuple[Event, ...], name: str | None = None, *, query: str | None = None) -> Callable[[_F], _F]:
        """Registers an observer for `event` (a component class used as an
        event, or "OnAdd"/"OnSet"/"OnRemove")."""
    def module(self, name: str) -> FlecsModule: ...
    def import_native(self, name: str, phase: Phase = ...) -> FlecsModule: ...
    def prefab(self, name: str, base: Prefab | None = None, sid: int | None = None) -> Prefab:
        """A prefab, optionally derived from another (`base`): instances of the
        derived prefab also match `@base.system`."""
    def phase(self, name: str, after: Phase = ...) -> Entity:
        """A pipeline phase that runs after `after` (design doc 5.5)."""
    def component_type(self, path: str) -> type[Component]:
        """The Python class of a native (Rust) reflected component. Components
        under `sage.` can also be imported by path (`from sage.collision
        import Position`), which editors understand."""
    def entity(self, name: str | None = None, sid: int | None = None) -> Entity: ...
    def lookup(self, sid: int) -> Entity | None: ...
    def lookup_path(self, path: str) -> Entity | None: ...
    def intern(self, text: str) -> int:
        """The id of `text` in the world's string table (0 for ""). Components
        store the id; the text is saved with scenes and save games."""
    def string(self, text_id: int) -> str | None: ...
    def migrations(self, text: str) -> None:
        """Adds schema migrations in the `sage-migrations v1` text form."""
    def set(self, value: object) -> None: ...
    def get(self, cls: type[_T]) -> _T | None: ...
    @property
    def rng(self) -> WorldRng: ...
    @property
    def module_entity(self) -> Entity: ...
