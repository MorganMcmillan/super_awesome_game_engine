//! The kit's native components, under `sage.platformer`. Scripts import them
//! by path (`from sage.platformer import Velocity, Body, Solid, Look`).
//!
//! Position and Collider are the shared `sage.collision` ones; the rules that
//! use these (the player, enemies, scoring) are Python components the kit's
//! scripts define.

use flecs_ecs::prelude::*;
use sage_ecs_py::Ecs;

macro_rules! component {
    ($(#[$m:meta])* $name:ident { $($f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
        #[repr(C)]
        #[flecs(meta)]
        pub struct $name { $(pub $f: $t),* }
    };
}

component!(
    /// Units per second.
    Velocity { x: f32, y: f32 }
);
component!(
    /// Marks a `Velocity` + `Collider` entity as a body the `kinematics`
    /// module moves and collides with `Solid`s. The module writes everything
    /// below each tick; rules read it.
    ///
    /// `floor` is the StableId of the solid it stands on (0 for none),
    /// `wall` is -1 or 1 for a wall touching that side, `headroom` is the
    /// free space above its head, `drop_left` and `drop_right` mean a grounded
    /// body has nothing underfoot just past that side. A rule sets
    /// `drop_through` to 1 to fall through one-way platforms this tick.
    /// `squeezed` is 1 when a moving solid pushed it into another.
    Body {
        floor: u64,
        grounded: u32,
        ceiling: u32,
        wall: i32,
        headroom: f32,
        drop_left: u32,
        drop_right: u32,
        drop_through: u32,
        squeezed: u32,
    }
);
component!(
    /// Blocks bodies. `one_way` 1 blocks only a body landing on it from
    /// above. A solid with a `Velocity` is a mover: it carries the bodies
    /// standing on it and pushes the ones in its way.
    Solid { one_way: u32 }
);
component!(
    /// How a `Position` + `Collider` entity is drawn: a box filled with
    /// `color` (`0xRRGGBBAA`), or `image` (an `AssetId`; 0 for none) stretched
    /// over it and tinted by `color`. Higher `layer` draws later. `flip` 1
    /// mirrors the image. A sprite sheet is an image cut into `cols` by
    /// `rows` equal cells (0 means 1), and `frame` counts them row by row.
    /// (Fields are ordered so the struct has no padding bytes.)
    Look { image: u64, color: u32, layer: i32, flip: u32, cols: u32, rows: u32, frame: u32 }
);
component!(
    /// The camera: the world point at the middle of the view and the size of
    /// the view in world units. A rule moves it; presentation snaps it to
    /// whole units so the picture stays crisp.
    Camera { x: f32, y: f32, w: f32, h: f32 }
);
component!(
    /// The scenery behind the world: a sky gradient and two layers of
    /// triangular hills that scroll slower than the camera. Colors are
    /// `0xRRGGBBAA`.
    Backdrop { sky_top: u32, sky_bottom: u32, far: u32, near: u32 }
);

/// Registers the vocabulary with the binding. Call after the world's
/// components exist and before scripts load.
pub fn register(world: &World, ecs: &Ecs) -> Result<(), String> {
    macro_rules! register {
        ($($t:ty => $name:literal),* $(,)?) => {
            $( ecs.register_native(*world.component_named::<$t>($name).id())?; )*
        };
    }
    register!(
        Velocity => "sage::platformer::Velocity",
        Body => "sage::platformer::Body",
        Solid => "sage::platformer::Solid",
        Look => "sage::platformer::Look",
        Camera => "sage::platformer::Camera",
        Backdrop => "sage::platformer::Backdrop",
    );
    world.set(Camera {
        x: 160.0,
        y: 90.0,
        w: 320.0,
        h: 180.0,
    });
    world.set(Backdrop::default());
    Ok(())
}
