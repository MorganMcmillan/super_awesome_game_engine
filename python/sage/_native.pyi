# Generated from the reflection data of the host's native components
# (sage_ecs_py `Ecs::type_stubs`). Editors read this; the engine never does.
# Do not edit by hand; regenerate it instead.

from sage.ecs import Component, u32, u64


class Clock(Component):
    """Native component `sage.Clock`."""

    tick: u64
    def __init__(self, tick: u64 = ...) -> None: ...


class Input(Component):
    """Native component `sage.Input`."""

    buttons: u32
    def __init__(self, buttons: u32 = ...) -> None: ...
