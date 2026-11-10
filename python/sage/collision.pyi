# Generated from the reflection data of the host's native components
# (sage_ecs_py `Ecs::type_stubs`). Editors read this; the engine never does.
# Do not edit by hand; regenerate it instead.

from sage.ecs import Component, f32, u64


class Collider(Component):
    """Native component `sage.collision.Collider`."""

    hw: f32
    hh: f32
    def __init__(self, hw: f32 = ..., hh: f32 = ...) -> None: ...


class ContactBegin(Component):
    """Native component `sage.collision.ContactBegin`."""

    a: u64
    b: u64
    nx: f32
    ny: f32
    depth: f32
    def __init__(self, a: u64 = ..., b: u64 = ..., nx: f32 = ..., ny: f32 = ..., depth: f32 = ...) -> None: ...


class Position(Component):
    """Native component `sage.collision.Position`."""

    x: f32
    y: f32
    def __init__(self, x: f32 = ..., y: f32 = ...) -> None: ...
