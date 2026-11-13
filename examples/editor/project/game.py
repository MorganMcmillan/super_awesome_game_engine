# A small game to edit: a hero that drifts, a rock that does not, and a
# system that moves everything with a velocity.
#
# Open it with `cargo run -p editor -- play`. Edit values in the inspector,
# press PLAY, change this text and press APPLY, or build rules in BLOCKS.

import sage
from sage import world
from sage import component, Entity


@component
class Position:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0


@component
class Velocity:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0


@component
class Size:
    hx: sage.f32 = 4.0
    hy: sage.f32 = 4.0


@component
class Player:
    pass


# Moves everything that has a velocity.
@sage.position(10, 10)
@world.system("move", query="Position, Velocity")
def move(pos: Position, vel: Velocity):
    """Add velocity to position, a little each tick."""
    # one tick is a sixtieth of a second
    pos.x += vel.x * 0.1
    pos.y += vel.y * 0.1


hero = world.entity("hero", 1)
hero.set(Position(20, 30)).set(Velocity(4, 1)).set(Size(5, 5)).add(Player)
rock = world.entity("rock", 2)
rock.set(Position(90, 40)).set(Size(8, 6))
tree = world.entity("tree", 3)
tree.set(Position(60, 15)).set(Size(3, 9))
