# The platformer kit, part 4 of 5: levels are pictures made of text.
#
#     kit.load_level("""
#     ; a comment line (it is not part of the picture)
#     ..........................
#     ......o.o.....e...........
#     ..@..####....####....c....
#     ##########^^##########.###
#     """)
#
#   #  solid block (grass on top where nothing is above it)
#   =  one-way platform: jump up through it, land on top, DOWN+JUMP to drop
#   ^  spikes
#   o  coin
#   @  where the player starts
#   c  checkpoint
#   e  walker enemy
#   h  moving platform, sliding sideways (a run of h is one wide platform)
#   v  moving platform, sliding up and down
#   .  nothing (a space also means nothing)
#
# The picture's bottom-left corner is the world's origin, a character is one
# 16 by 16 tile, and every entity gets the next StableId from
# `kit.FIRST_TILE`, in reading order, so the same picture always loads the
# same way.

from sage import world
from sage.collision import Collider, Position
from sage.platformer import Body, Camera, Look, Solid, Velocity

kit = world.platformer
art = kit.art
TILE = 16.0
MOVER_RANGE = 48.0
MOVER_SPEED = 30.0


def spawn_player(x, y):
    """The player and the little eye that shows which way it looks. `x, y` is
    the middle of the box."""
    player = world.entity("player", sid=kit.ID_PLAYER)
    player.set(Position(x, y)).set(Collider(5.0, 7.0)).set(Velocity()).set(Body())
    player.set(Look(color=art.player, layer=10))
    player.set(kit.Player()).set(kit.Health()).set(kit.Respawn(x, y))
    face = world.entity("face", sid=kit.ID_FACE)
    face.set(Position(x, y)).set(Collider(1.5, 1.5)).set(Look(color=art.eye, layer=11))
    face.add(kit.Face)
    return player


def load_level(text):
    """Builds the level from its picture. Returns (width, height) in units."""
    lines = [ln.rstrip() for ln in text.splitlines() if not ln.lstrip().startswith(";")]
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    if not lines:
        raise ValueError("load_level: the picture is empty")
    rows = len(lines)
    cols = max(len(ln) for ln in lines)
    grid = [ln.replace(" ", ".").ljust(cols, ".") for ln in lines]
    width, height = cols * TILE, rows * TILE

    next_sid = [kit.FIRST_TILE]

    def make(name=None):
        sid = next_sid[0]
        next_sid[0] += 1
        return world.entity(name, sid=sid)

    def solid(x, y, hw, hh, color, layer, one_way=0):
        e = make()
        e.set(Position(x, y)).set(Collider(hw, hh)).set(Solid(one_way))
        if color:
            e.set(Look(color=color, layer=layer))
        return e

    start = None
    row = 0
    while row < rows:
        col = 0
        while col < cols:
            ch = grid[row][col]
            cx = col * TILE + TILE / 2
            cy = (rows - 1 - row) * TILE + TILE / 2
            if ch == "#":
                top = row == 0 or grid[row - 1][col] != "#"
                shade = art.grass if top else (art.dirt if (col + row) % 2 == 0 else art.dirt_dark)
                solid(cx, cy, TILE / 2, TILE / 2, shade, 0)
            elif ch == "=":
                solid(cx, cy + TILE / 2 - 3.0, TILE / 2, 3.0, art.one_way, 1, one_way=1)
            elif ch == "^":
                e = make()
                e.set(Position(cx, cy - TILE / 2 + 4.0)).set(Collider(6.0, 4.0))
                e.set(Look(color=art.spike, layer=2)).set(kit.Hazard())
            elif ch == "o":
                e = make()
                e.set(Position(cx, cy)).set(Collider(4.0, 4.0))
                e.set(Look(color=art.coin, layer=3)).set(kit.Coin())
            elif ch == "c":
                e = make()
                e.set(Position(cx, cy - TILE / 2 + 12.0)).set(Collider(2.0, 12.0))
                e.set(Look(color=art.checkpoint, layer=2)).set(kit.Checkpoint())
            elif ch == "e":
                e = make()
                e.set(Position(cx, cy - TILE / 2 + 6.0)).set(Collider(6.0, 6.0))
                e.set(Velocity()).set(Body()).set(Look(color=art.walker, layer=8))
                e.set(kit.Walker()).add(kit.Enemy)
            elif ch in "hv":
                end = col
                while end < cols and grid[row][end] == ch:
                    end += 1
                x0 = (col + end) * TILE / 2
                y0 = cy + TILE / 2 - 3.0
                half = (end - col) * TILE / 2
                e = solid(x0, y0, half, 3.0, art.platform, 1, one_way=1)
                e.set(Velocity()).set(
                    kit.Mover(x0=x0, y0=y0, range=MOVER_RANGE, speed=MOVER_SPEED,
                              vertical=1 if ch == "v" else 0)
                )
                col = end - 1
            elif ch == "@":
                if start is not None:
                    raise ValueError("load_level: two player starts (@)")
                start = (cx, cy - TILE / 2 + 7.0)
            elif ch != ".":
                raise ValueError(f"load_level: unknown character {ch!r} at row {row + 1}, column {col + 1}")
            col += 1
        row += 1

    # Invisible walls keep the player inside the picture's left and right edges.
    for x in (-8.0, width + 8.0):
        solid(x, height / 2, 8.0, height / 2 + 256.0, 0, 0)

    if start is None:
        raise ValueError("load_level: the picture has no player start (@)")
    spawn_player(*start)
    g = kit.game.get(kit.Game)
    g.width, g.height = width, height
    kit.game.set(g)
    # Look at the start from the first tick.
    cam = world.get(Camera)
    cam.x, cam.y = start
    world.set(cam)
    return width, height


kit.load_level = load_level
kit.spawn_player = spawn_player
