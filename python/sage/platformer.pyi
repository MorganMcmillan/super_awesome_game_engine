# Generated from the reflection data of the host's native components
# (sage_ecs_py `Ecs::type_stubs`). Editors read this; the engine never does.
# Do not edit by hand; regenerate it instead.

from sage.ecs import Component, f32, i32, u32, u64


class Backdrop(Component):
    """Native component `sage.platformer.Backdrop`."""

    sky_top: u32
    sky_bottom: u32
    far: u32
    near: u32
    def __init__(self, sky_top: u32 = ..., sky_bottom: u32 = ..., far: u32 = ..., near: u32 = ...) -> None: ...


class Body(Component):
    """Native component `sage.platformer.Body`."""

    floor: u64
    grounded: u32
    ceiling: u32
    wall: i32
    headroom: f32
    drop_left: u32
    drop_right: u32
    drop_through: u32
    squeezed: u32
    def __init__(self, floor: u64 = ..., grounded: u32 = ..., ceiling: u32 = ..., wall: i32 = ..., headroom: f32 = ..., drop_left: u32 = ..., drop_right: u32 = ..., drop_through: u32 = ..., squeezed: u32 = ...) -> None: ...


class Camera(Component):
    """Native component `sage.platformer.Camera`."""

    x: f32
    y: f32
    w: f32
    h: f32
    def __init__(self, x: f32 = ..., y: f32 = ..., w: f32 = ..., h: f32 = ...) -> None: ...


class Look(Component):
    """Native component `sage.platformer.Look`."""

    image: u64
    color: u32
    layer: i32
    flip: u32
    cols: u32
    rows: u32
    frame: u32
    def __init__(self, image: u64 = ..., color: u32 = ..., layer: i32 = ..., flip: u32 = ..., cols: u32 = ..., rows: u32 = ..., frame: u32 = ...) -> None: ...


class Solid(Component):
    """Native component `sage.platformer.Solid`."""

    one_way: u32
    def __init__(self, one_way: u32 = ...) -> None: ...


class Velocity(Component):
    """Native component `sage.platformer.Velocity`."""

    x: f32
    y: f32
    def __init__(self, x: f32 = ..., y: f32 = ...) -> None: ...
