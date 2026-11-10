# Pong, written in Python on Flecs (design doc Appendix A).
#
#   game states: Serving, Playing, GameOver   (a Game component, not phases)
#
#   while Playing:                     -> flecs module `play` is enabled
#       ball moves, paddles follow input
#   when ball hits paddle: bounce ball -> observer on ContactBegin
#   when ball hits side:   emit goal   -> observer on ContactBegin
#   on goal(side):
#       scorer = side.opponent         (relationship: (Opponent, player))
#       scorer.points += 1
#       go to GameOver if scorer.points == max_points, else Serving
#   on entering Serving:               -> core module, state machine
#       reset ball, wait 1s, go to Playing
#
# Everything here is Python except the engine vocabulary the host provides:
# Position, Collider and the ContactBegin event (the native `collision` flecs
# module), and the Input and Clock singletons.
#
# Simulation rules must be deterministic: no time, no randomness except
# world.rng, no dependence on set order, and all state in components.

import math

import sage
from sage import component, Entity

# Engine vocabulary: native components the host registers, imported by path.
from sage import Input
from sage.collision import Collider, ContactBegin, Position

# ----- tuning ------------------------------------------------------------------

COURT_W, COURT_H = 160.0, 90.0
WALL_HALF = 1.0
PADDLE_HALF_W, PADDLE_HALF_H = 1.0, 8.0
PADDLE_X = 6.0
PADDLE_SPEED = 90.0
BALL_HALF = 1.0
BALL_SPEED_X = 60.0
BALL_MAX_VY = 70.0  # below PADDLE_SPEED, so a paddle can always follow the ball
SERVE_TICKS = 60
MAX_POINTS = 5

# Input bits (must match the host's key bindings).
LEFT_UP, LEFT_DOWN, RIGHT_UP, RIGHT_DOWN = 1, 2, 4, 8

LEFT, RIGHT = 0, 1
NO_SIDE = 255
SERVING, PLAYING, GAME_OVER = 0, 1, 2
NO_STATE = 255
STATE_NAMES = {SERVING: "Serving", PLAYING: "Playing", GAME_OVER: "GameOver"}

# StableIds, as in the Rust version.
ID_GAME = 2
ID_WALL_TOP, ID_WALL_BOTTOM = 10, 11
ID_GOAL_LEFT, ID_GOAL_RIGHT = 20, 21
ID_PADDLE_LEFT, ID_PADDLE_RIGHT = 30, 31
ID_BALL = 40
ID_PLAYER_LEFT, ID_PLAYER_RIGHT = 50, 51

# ----- components ----------------------------------------------------------------


@component
class Velocity:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0


@component
class Ball:
    pass


@component
class Wall:
    pass


@component
class Paddle:
    side: sage.u8 = LEFT


@component
class GoalSide:
    """A side of the court; touching it concedes a point."""

    side: sage.u8 = LEFT


@component
class Player:
    side: sage.u8 = LEFT
    points: sage.u32 = 0


@component
class Opponent:
    """Relationship: (Opponent, player) on a side names who scores there."""


@component
class Goal:
    """Event, emitted on the side entity that was hit."""

    side: sage.u8 = LEFT


@component
class Game:
    state: sage.u8 = SERVING
    pending: sage.u8 = NO_STATE  # requested this tick, applied in SimReact
    winner: sage.u8 = NO_SIDE
    serve_timer: sage.u32 = SERVE_TICKS
    max_points: sage.u32 = MAX_POINTS


# ----- timing and flecs modules ---------------------------------------------------

SimInput = world.phase("SimInput", after=sage.OnUpdate)
SimMove = world.phase("SimMove", after=SimInput)
SimDetect = world.phase("SimDetect", after=SimMove)
SimReact = world.phase("SimReact", after=SimDetect)

core = world.module("core")  # always on: the game-state machine
play = world.module("play")  # on only while Playing
play.import_native("collision", phase=SimDetect)

# ----- entities --------------------------------------------------------------------


def box(name, sid, x, y, hw, hh):
    return world.entity(name, sid=sid).set(Position(x, y)).set(Collider(hw, hh))


game = world.entity("game", sid=ID_GAME).set(Game())
box("wall_top", ID_WALL_TOP, COURT_W / 2, COURT_H - WALL_HALF, COURT_W / 2, WALL_HALF).add(Wall)
box("wall_bottom", ID_WALL_BOTTOM, COURT_W / 2, WALL_HALF, COURT_W / 2, WALL_HALF).add(Wall)
left_player = world.entity("player_left", sid=ID_PLAYER_LEFT).set(Player(LEFT))
right_player = world.entity("player_right", sid=ID_PLAYER_RIGHT).set(Player(RIGHT))
box("side_left", ID_GOAL_LEFT, -2.0, COURT_H / 2, 2.0, COURT_H / 2).set(GoalSide(LEFT)).add(Opponent, right_player)
box("side_right", ID_GOAL_RIGHT, COURT_W + 2.0, COURT_H / 2, 2.0, COURT_H / 2).set(GoalSide(RIGHT)).add(Opponent, left_player)
for sid, name, side, x in [
    (ID_PADDLE_LEFT, "paddle_left", LEFT, PADDLE_X),
    (ID_PADDLE_RIGHT, "paddle_right", RIGHT, COURT_W - PADDLE_X),
]:
    box(name, sid, x, COURT_H / 2, PADDLE_HALF_W, PADDLE_HALF_H).set(Velocity()).set(Paddle(side))
box("ball", ID_BALL, COURT_W / 2, COURT_H / 2, BALL_HALF, BALL_HALF).set(Velocity()).add(Ball)

# ----- rules -------------------------------------------------------------------------


@play.system(phase=SimInput)
def paddles_follow_input(vel: Velocity, paddle: Paddle):
    buttons = world.get(Input).buttons
    up, down = (LEFT_UP, LEFT_DOWN) if paddle.side == LEFT else (RIGHT_UP, RIGHT_DOWN)
    held_up, held_down = buttons & up != 0, buttons & down != 0
    if held_up and not held_down:
        vel.y = PADDLE_SPEED
    elif held_down and not held_up:
        vel.y = -PADDLE_SPEED
    else:
        vel.y = 0.0


@play.system(phase=SimMove)
def move(pos: Position, vel: Velocity):
    pos.x += vel.x * sage.TICK_DT
    pos.y += vel.y * sage.TICK_DT


@play.system(phase=SimMove)
def clamp_paddles(pos: Position, paddle: Paddle):
    lo = 2 * WALL_HALF + PADDLE_HALF_H
    pos.y = min(max(pos.y, lo), COURT_H - lo)


def seen_from_ball(c):
    """(other StableId, normal pointing from the ball to it), or None."""
    if c.a == ID_BALL:
        return c.b, (c.nx, c.ny)
    if c.b == ID_BALL:
        return c.a, (-c.nx, -c.ny)
    return None


def signum(v):
    return math.copysign(1.0, v)


@play.observer(ContactBegin)
def bounce_ball(c: ContactBegin):
    """when ball hits paddle (or wall): bounce ball"""
    seen = seen_from_ball(c)
    if seen is None:
        return
    other_sid, (nx, ny) = seen
    other, ball = world.lookup(other_sid), world.lookup(ID_BALL)
    pos, vel = ball.get(Position), ball.get(Velocity)
    if other.has(Paddle):
        paddle_y = other.get(Position).y
        pos.x -= nx * c.depth
        vel.x = -signum(nx) * abs(vel.x)
        offset = (pos.y - paddle_y) / PADDLE_HALF_H
        vel.y = min(max(vel.y + offset * 0.5 * BALL_SPEED_X, -BALL_MAX_VY), BALL_MAX_VY)
    elif other.has(Wall):
        pos.y -= ny * c.depth
        vel.y = -signum(ny) * abs(vel.y)


@play.observer(ContactBegin)
def ball_hits_side(c: ContactBegin):
    """when ball hits side: emit goal(side)"""
    seen = seen_from_ball(c)
    if seen is None:
        return
    side = world.lookup(seen[0])
    goal_side = side.get(GoalSide)
    if goal_side is not None:
        side.emit(Goal(goal_side.side))


@play.observer(Goal, query="GoalSide")
def on_goal(side: Entity, goal: Goal):
    """on goal(side): the opponent scores"""
    scorer = side.target(Opponent).get(Player)
    scorer.points += 1
    g = game.get(Game)
    if scorer.points == g.max_points:
        g.pending = GAME_OVER
        g.winner = scorer.side
    else:
        g.pending = SERVING


def enter_state(g, state):
    """State entry actions. The only place flecs module enablement changes."""
    ball = world.lookup(ID_BALL)
    g.state = state
    if state == SERVING:
        play.disable()
        ball.set(Position(COURT_W / 2, COURT_H / 2))
        ball.set(Velocity())
        g.serve_timer = SERVE_TICKS
    elif state == PLAYING:
        direction = 1.0 if world.rng.next_bool() else -1.0
        vy = world.rng.range_f32(-0.5, 0.5) * BALL_SPEED_X
        ball.set(Velocity(direction * BALL_SPEED_X, vy))
        play.enable()
    else:
        play.disable()
        ball.set(Velocity())


# Immediate: entering a state toggles a flecs module and moves the ball;
# doing it directly keeps the order of effects obvious.
@core.system(phase=SimReact, immediate=True)
def game_state_machine(g: Game):
    if g.pending == NO_STATE and g.state == SERVING:
        g.serve_timer = max(g.serve_timer - 1, 0)
        if g.serve_timer == 0:
            g.pending = PLAYING
    if g.pending != NO_STATE:
        state = g.pending
        g.pending = NO_STATE
        enter_state(g, state)


enter_state(game.get(Game), SERVING)
