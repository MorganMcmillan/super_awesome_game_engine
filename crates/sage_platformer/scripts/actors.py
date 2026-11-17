# The platformer kit, part 3 of 5: everything that is not the player.
#
#   walkers   turn at walls and ledges; stomp them, or spin, roll or dive into them
#   movers    platforms that swing back and forth, carrying whoever stands on them
#   coins     touch to collect
#   spikes    touch to be hurt (and thrown clear)
#   flags     checkpoints: touch one and you come back there
#
# Touching is the host's job: its `collision` flecs module announces every new
# overlap as a ContactBegin event. One observer reads them for the player.

import sage
from sage import Input, world
from sage.collision import Collider, ContactBegin, Position
from sage.platformer import Body, Look, Velocity

kit = world.platformer
Tuning, Rules, Tally, Player = kit.Tuning, kit.Rules, kit.Tally, kit.Player
Walker, Mover, Coin, Hazard, Checkpoint = kit.Walker, kit.Mover, kit.Coin, kit.Hazard, kit.Checkpoint
Health, Score, Respawn, Game = kit.Health, kit.Score, kit.Respawn, kit.Game
ID_PLAYER, JUMP = kit.ID_PLAYER, kit.JUMP
HURT, AIR, SPIN_MODE, ROLL, DIVE = kit.HURT, kit.AIR, kit.SPIN_MODE, kit.ROLL, kit.DIVE
art = kit.art
play = kit.play


# ----- walkers and movers --------------------------------------------------------------


@play.system(phase=kit.phases.input)
def walkers_walk(vel: Velocity, body: Body, w: Walker, look: Look):
    t, r = world.get(Tuning), world.get(Rules)
    if body.grounded:
        at_wall = body.wall == w.dir
        at_ledge = body.drop_left if w.dir < 0 else body.drop_right
        if at_wall or at_ledge:
            w.dir = -w.dir
    vel.x = w.dir * r.walker_speed
    look.flip = 1 if w.dir < 0 else 0  # sprites face right as drawn
    vel.y = max(vel.y - t.gravity_down * sage.TICK_DT, -t.max_fall)


@play.system(phase=kit.phases.input)
def movers_swing(pos: Position, vel: Velocity, m: Mover):
    offset = (pos.y - m.y0) if m.vertical else (pos.x - m.x0)
    if offset >= m.range:
        m.dir = -1
    elif offset <= -m.range:
        m.dir = 1
    speed = m.dir * m.speed
    vel.x, vel.y = (0.0, speed) if m.vertical else (speed, 0.0)


# ----- getting hurt ------------------------------------------------------------------------


def hurt(player, source_x, damage=1):
    """Hurts the player: lose health, get thrown away from `source_x`, and be
    safe for a moment. Zero health asks the game state machine for DEAD."""
    t, r = world.get(Tuning), world.get(Rules)
    p, pos, vel, col = (player.get(c) for c in (Player, Position, Velocity, Collider))
    if p.invuln > 0 or p.mode == HURT:
        return
    health = player.get(Health)
    health.hp = health.hp - min(damage, health.hp)
    p.invuln = r.invuln_ticks
    vel.x = (1.0 if pos.x >= source_x else -1.0) * r.hurt_knock_x
    vel.y = r.hurt_knock_y
    p.rise = 0
    kit.game.get(Tally).hurts += 1
    kit.enter(p, pos, col, t, HURT)
    if health.hp == 0:
        kit.game.get(Game).pending = kit.DEAD


def stomp(player, buttons):
    """Bounces the player off an enemy. Holding JUMP bounces higher."""
    t, r = world.get(Tuning), world.get(Rules)
    p, vel = player.get(Player), player.get(Velocity)
    held = buttons & JUMP != 0
    vel.y = r.stomp_bounce_held if held else r.stomp_bounce
    p.rise = 1 if held else 0
    p.spun = 0
    p.invuln = max(p.invuln, 6)
    if p.mode != AIR:
        kit.enter(p, player.get(Position), player.get(Collider), t, AIR)


# ----- contacts ------------------------------------------------------------------------------


@play.observer(ContactBegin)
def player_touches(c: ContactBegin):
    """Every new overlap with the player. (Pairs come as `a < b`, and the
    player's StableId is smaller than everything in a level.)"""
    if c.a != ID_PLAYER:
        return
    other = world.lookup(c.b)
    if other is None:
        return
    player = world.lookup(ID_PLAYER)
    tally = kit.game.get(Tally)

    coin = other.get(Coin)
    if coin is not None:
        kit.game.get(Score).coins += coin.value
        tally.coins += 1
        other.destruct()  # last: it changes the world's structure
        return

    flag = other.get(Checkpoint)
    if flag is not None:
        if not flag.active:
            flag.active = 1
            at, size = other.get(Position), other.get(Collider)
            spot = player.get(Respawn)
            spot.x = at.x
            spot.y = at.y - size.hh + player.get(Player).stand_height + 1.0
            other.get(Look).color = art.checkpoint_on
            tally.checkpoints += 1
        return

    spikes = other.get(Hazard)
    if spikes is not None:
        hurt(player, other.get(Position).x, spikes.damage)
        return

    if other.get(Walker) is not None:
        p = player.get(Player)
        vel = player.get(Velocity)
        feet = player.get(Position).y - player.get(Collider).hh
        above = feet > other.get(Position).y
        if above and vel.y <= 0:
            stomp(player, world.get(Input).buttons)
            kit.game.get(Score).stomps += 1
            tally.stomps += 1
            other.destruct()
        elif p.mode in (SPIN_MODE, ROLL, DIVE):
            kit.game.get(Score).stomps += 1
            tally.stomps += 1
            other.destruct()
        else:
            hurt(player, other.get(Position).x)


kit.hurt = hurt
