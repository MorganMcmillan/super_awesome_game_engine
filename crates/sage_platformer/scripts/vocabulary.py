# The platformer kit, part 1 of 5: vocabulary, tuning, timing and the
# `world.platformer` namespace the other parts (and your game) use.
#
# A kit is open: everything a beginner writes is also what is written here.
# Read the parts in this order:
#
#   vocabulary.py  what exists (components), how it is tuned, when it runs
#   player.py      the player: walk, run, jump, wall jump, roll, dive, spin
#   actors.py      enemies, coins, spikes, checkpoints, moving platforms
#   level.py       turning a text picture into entities
#   rules.py       the game state machine (pause, death, respawn) and camera
#
# The host provides only what must be fast or must be exact: Position and
# Collider (`sage.collision`), the `collision` and `kinematics` flecs modules,
# and Velocity, Body, Solid, Look, Camera and Backdrop (`sage.platformer`).
# A game script then says what is different about *this* game:
#
#     from sage import world
#     kit = world.platformer
#     kit.tune(gravity_down=900, run_speed=170)
#     kit.load_level(LEVEL)
#
# Units: 1 world unit is 1 pixel at the kit's native view (320 by 180), a tile
# is 16 units, y points up. Speeds are units per second.

import types

import sage
from sage import component, world
from sage.platformer import Backdrop

# ----- input ----------------------------------------------------------------------
# Button bits, as the host's input map sets them (`sage_platformer::buttons`).
LEFT, RIGHT, DOWN, UP, JUMP, RUN, SPIN, PAUSE = 1, 2, 4, 8, 16, 32, 64, 128

# ----- stable ids ---------------------------------------------------------------------
# Things the rules look up by name. Level entities are numbered from FIRST_TILE
# in the order they appear in the level text, so a level always loads the same.
ID_GAME = 2
ID_PLAYER = 10
ID_FACE = 11
FIRST_TILE = 1000

# ----- game states and player modes -----------------------------------------------------
# A game state is not a pipeline phase (design doc 14): it is data.
PLAYING, PAUSED, DEAD = 0, 1, 2
NO_STATE = 255

# What the player is doing right now. Each mode has its own rules in player.py.
GROUND, AIR, WALL, DUCK, ROLL, DIVE, SPIN_MODE, HURT = range(8)
MODE_NAMES = ["ground", "air", "wall", "duck", "roll", "dive", "spin", "hurt"]

# ----- components ----------------------------------------------------------------------


@component
class Tuning:
    """The numbers of how the player moves: what decides how the game feels.
    One instance, on the world. Change it with `kit.tune(...)`."""

    # walking and running
    walk_speed: sage.f32 = 80.0
    run_speed: sage.f32 = 140.0
    ground_accel: sage.f32 = 520.0
    ground_decel: sage.f32 = 640.0  # friction with no button held
    turn_decel: sage.f32 = 1000.0  # braking when you push the other way
    air_accel: sage.f32 = 380.0
    air_decel: sage.f32 = 90.0
    # gravity and jumping
    gravity_up: sage.f32 = 780.0  # rising with the jump button held
    gravity_down: sage.f32 = 1200.0  # falling, or rising after letting go
    max_fall: sage.f32 = 360.0
    jump_speed: sage.f32 = 330.0
    jump_speed_bonus: sage.f32 = 0.12  # extra jump per unit of horizontal speed
    jump_release_speed: sage.f32 = 120.0  # letting go cuts a rise to this
    coyote_ticks: sage.u32 = 6  # a jump still works this long after a ledge
    buffer_ticks: sage.u32 = 6  # a jump pressed this early still counts
    # wall slide and wall jump
    wall_slide_speed: sage.f32 = 60.0
    wall_jump_x: sage.f32 = 150.0
    wall_jump_y: sage.f32 = 300.0
    wall_lock_ticks: sage.u32 = 12  # steering toward the wall is ignored
    # roll
    roll_min_speed: sage.f32 = 90.0  # fast enough to roll when you press down
    roll_speed: sage.f32 = 170.0
    roll_decel: sage.f32 = 90.0
    roll_ticks: sage.u32 = 40
    roll_height: sage.f32 = 4.0  # half height while ducking, rolling, diving
    # dive
    dive_speed: sage.f32 = 200.0
    dive_lift: sage.f32 = 90.0
    dive_gravity: sage.f32 = 600.0
    # mid-air spin
    spin_ticks: sage.u32 = 20
    spin_lift: sage.f32 = 120.0
    spin_fall: sage.f32 = 70.0  # fall speed is capped to this while spinning


@component
class Rules:
    """The numbers of everything that is not the player's own movement:
    bounces, damage, death, enemies and the camera. One instance."""

    # enemies, damage, death
    stomp_bounce: sage.f32 = 240.0
    stomp_bounce_held: sage.f32 = 340.0  # holding jump bounces higher
    hurt_ticks: sage.u32 = 24
    hurt_knock_x: sage.f32 = 110.0
    hurt_knock_y: sage.f32 = 220.0
    invuln_ticks: sage.u32 = 90
    respawn_ticks: sage.u32 = 50
    fall_death: sage.f32 = 48.0  # below the level by this much is a fall
    walker_speed: sage.f32 = 28.0
    # camera
    camera_lead: sage.f32 = 28.0
    camera_ease: sage.f32 = 0.12
    camera_dead_y: sage.f32 = 24.0


@component
class Game:
    state: sage.u32 = PLAYING
    pending: sage.u32 = NO_STATE  # requested; applied by the state machine
    timer: sage.u32 = 0
    prev: sage.u32 = 0  # buttons last tick, for edges
    width: sage.f32 = 320.0  # the level's size, set by load_level
    height: sage.f32 = 180.0


@component
class Player:
    mode: sage.u32 = AIR
    timer: sage.u32 = 0  # ticks in this mode
    facing: sage.i32 = 1
    prev: sage.u32 = 0
    coyote: sage.u32 = 0
    buffer: sage.u32 = 0
    wall_lock: sage.u32 = 0
    rise: sage.u32 = 0  # 1 while rising from a jump you can still cut short
    spun: sage.u32 = 0  # 1 once the spin of this jump is used
    invuln: sage.u32 = 0
    stand_height: sage.f32 = 7.0  # half height when upright


@component
class Health:
    hp: sage.u32 = 3
    max: sage.u32 = 3


@component
class Respawn:
    """Where the player comes back: the start, or the last checkpoint."""

    x: sage.f32 = 0.0
    y: sage.f32 = 0.0


@component
class Score:
    coins: sage.u32 = 0
    stomps: sage.u32 = 0
    deaths: sage.u32 = 0


@component
class Tally:
    """How many times each thing happened. Rules add to it; the host compares
    it from tick to tick to know which sound to play, so the simulation never
    talks to the speakers."""

    jumps: sage.u32 = 0
    lands: sage.u32 = 0
    wall_jumps: sage.u32 = 0
    rolls: sage.u32 = 0
    dives: sage.u32 = 0
    spins: sage.u32 = 0
    coins: sage.u32 = 0
    stomps: sage.u32 = 0
    hurts: sage.u32 = 0
    checkpoints: sage.u32 = 0
    deaths: sage.u32 = 0


@component
class Enemy:
    """Tag: something that hurts you unless you stomp or attack it."""


@component
class Walker:
    """An enemy that walks, turning at walls and ledges."""

    dir: sage.i32 = -1


@component
class Coin:
    value: sage.u32 = 1


@component
class Hazard:
    damage: sage.u32 = 1


@component
class Checkpoint:
    active: sage.u32 = 0


@component
class Mover:
    """A platform that swings back and forth: `range` units either side of
    where it starts, along x (or y when `vertical` is 1)."""

    x0: sage.f32 = 0.0
    y0: sage.f32 = 0.0
    range: sage.f32 = 48.0
    speed: sage.f32 = 30.0
    vertical: sage.u32 = 0
    dir: sage.i32 = 1


@component
class Face:
    """The little eye that shows which way the player looks (the art is plain
    boxes, so something has to)."""


# ----- timing and flecs modules ------------------------------------------------------------

SimInput = world.phase("SimInput", after=sage.OnUpdate)  # rules decide velocities
SimMove = world.phase("SimMove", after=SimInput)  # kinematics moves and collides
SimDetect = world.phase("SimDetect", after=SimMove)  # collision announces contacts
SimReact = world.phase("SimReact", after=SimDetect)  # rules react to contacts
SimCamera = world.phase("SimCamera", after=SimReact)

core = world.module("core")  # always on: the game state machine
play = world.module("play")  # on only while PLAYING
play.import_native("kinematics", phase=SimMove)
play.import_native("collision", phase=SimDetect)

# ----- the world's singletons -----------------------------------------------------------------

world.set(Tuning())
world.set(Rules())
game = world.entity("game", sid=ID_GAME)
game.set(Game()).set(Score()).set(Tally())

# ----- the namespace -----------------------------------------------------------------------------


def tune(**values):
    """`kit.tune(gravity_down=900, run_speed=170)`: change tuning numbers, in
    `Tuning` (movement) or `Rules` (everything else)."""
    groups = [(cls, world.get(cls)) for cls in (Tuning, Rules)]
    for name, value in values.items():
        for cls, instance in groups:
            if name in [n for n, _ in cls.__sage_fields__]:
                setattr(instance, name, value)
                break
        else:
            raise TypeError(f"there is no tuning number called {name!r}")
    for _, instance in groups:
        world.set(instance)


def color(r, g, b, a=255):
    """`0xRRGGBBAA`, the way `Look` and `Backdrop` store colors."""
    return (r << 24) | (g << 16) | (b << 8) | a


def asset(path):
    """The id of an image or sound file, for `Look(image=kit.asset("x.png"))`.
    It is the same number the asset store gives that path (FNV-1a of the
    normalized path), so renaming a file is the only way to break it."""
    h = 0xCBF29CE484222325
    for byte in path.replace("\\", "/").removeprefix("./").encode():
        h = ((h ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


# Placeholder art: one color per role. A real artist replaces these with
# `Look(image=...)` on the same entities; nothing else changes.
art = types.SimpleNamespace(
    player=color(220, 64, 56),
    player_duck=color(190, 52, 48),
    player_spin=color(255, 255, 255),
    player_dive=color(255, 150, 40),
    player_hurt=color(255, 190, 180),
    face=color(255, 255, 255),
    eye=color(30, 30, 40),
    dirt=color(122, 84, 52),
    dirt_dark=color(108, 74, 46),
    grass=color(84, 172, 72),
    one_way=color(176, 132, 84),
    platform=color(96, 140, 196),
    walker=color(150, 70, 170),
    coin=color(250, 204, 40),
    spike=color(200, 205, 215),
    checkpoint=color(120, 120, 130),
    checkpoint_on=color(80, 220, 120),
    wall=0,
)

world.set(
    Backdrop(
        sky_top=color(92, 148, 252),
        sky_bottom=color(188, 224, 252),
        far=color(130, 170, 214),
        near=color(96, 150, 120),
    )
)

world.platformer = types.SimpleNamespace(
    # modules and phases
    core=core,
    play=play,
    phases=types.SimpleNamespace(
        input=SimInput, move=SimMove, detect=SimDetect, react=SimReact, camera=SimCamera
    ),
    # components
    Tuning=Tuning,
    Rules=Rules,
    Game=Game,
    Player=Player,
    Health=Health,
    Respawn=Respawn,
    Score=Score,
    Tally=Tally,
    Enemy=Enemy,
    Walker=Walker,
    Coin=Coin,
    Hazard=Hazard,
    Checkpoint=Checkpoint,
    Mover=Mover,
    Face=Face,
    # constants
    LEFT=LEFT,
    RIGHT=RIGHT,
    DOWN=DOWN,
    UP=UP,
    JUMP=JUMP,
    RUN=RUN,
    SPIN=SPIN,
    PAUSE=PAUSE,
    ID_GAME=ID_GAME,
    ID_PLAYER=ID_PLAYER,
    ID_FACE=ID_FACE,
    FIRST_TILE=FIRST_TILE,
    PLAYING=PLAYING,
    PAUSED=PAUSED,
    DEAD=DEAD,
    NO_STATE=NO_STATE,
    GROUND=GROUND,
    AIR=AIR,
    WALL=WALL,
    DUCK=DUCK,
    ROLL=ROLL,
    DIVE=DIVE,
    SPIN_MODE=SPIN_MODE,
    HURT=HURT,
    MODE_NAMES=MODE_NAMES,
    # helpers and art
    game=game,
    tune=tune,
    color=color,
    asset=asset,
    art=art,
)
