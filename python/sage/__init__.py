"""Scripting API. Decorators only mark functions; the engine discovers them.

The engine embeds this file as the ``sage`` package (``sage_script``). The
Flecs binding (``sage_ecs_py``) then adds ``sage._ecs`` (native),
``sage.ecs`` (``ecs.py`` in this directory) and re-exports the names below.
The ``TYPE_CHECKING`` block only tells editors about those re-exports; it
never runs.

A host's native components under ``sage.`` are importable by path while a
script loads (``from sage import Input``, ``from sage.collision import
Position``). ``_native.pyi`` and the other generated stubs here describe
them for editors.
"""

import typing as _typing

if _typing.TYPE_CHECKING:
    from sage import ecs as ecs
    from sage._native import *  # generated: native components directly under sage
    from sage.ecs import (
        TICK_DT as TICK_DT,
        Component as Component,
        Entity as Entity,
        FlecsModule as FlecsModule,
        OnAdd as OnAdd,
        OnLoad as OnLoad,
        OnRemove as OnRemove,
        OnSet as OnSet,
        OnStore as OnStore,
        OnUpdate as OnUpdate,
        OnValidate as OnValidate,
        PostLoad as PostLoad,
        PostUpdate as PostUpdate,
        Prefab as Prefab,
        PreStore as PreStore,
        PreUpdate as PreUpdate,
        World as World,
        component as component,
        f32 as f32,
        f64 as f64,
        i32 as i32,
        i64 as i64,
        u8 as u8,
        u32 as u32,
        u64 as u64,
    )


def on(event):
    """Marks a function as the handler for `event`, e.g. ``@sage.on("goal")``."""
    if not isinstance(event, str):
        raise TypeError("event name must be a string")
    def mark(fn):
        fn.__sage_event__ = event
        return fn
    return mark


def position(x, y):
    """Where a function's block stack sits on the block editor's canvas
    (design doc 4.1). It has no effect when the game runs."""
    for v in (x, y):
        if isinstance(v, bool) or not isinstance(v, (int, float)):
            raise TypeError("position takes two numbers")

    def mark(fn):
        fn.__sage_position__ = (x, y)
        return fn

    return mark
