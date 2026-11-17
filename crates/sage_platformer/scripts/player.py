# The platformer kit, part 2 of 5: the player.
#
# The player is always in one *mode*, and each mode has a small function with
# its own rules:
#
#   ground  walk, run (hold RUN), brake, jump; DOWN ducks, or rolls if you are fast
#   air     steer, fall; hold JUMP to rise higher; SPIN spins, DOWN+SPIN dives
#   wall    slide down a wall you push against; JUMP kicks off it (lean away
#           to cross to the other side, or keep pushing in to go up)
#   duck    low and slow; JUMP with DOWN drops through a one-way platform
#   roll    fast and low, keeps going until it runs out; JUMP leaps out of it
#   dive    a low lunge through the air that lands in a roll
#   spin    a short hover that also hits enemies
#   hurt    knocked back; no control
#
# Speeds are units per second; the numbers are in `Tuning` (vocabulary.py).
# The host moves the player and tells it what it touched (`Body`); the rules
# below only decide the velocity.

import sage
from sage import Input, world
from sage.collision import Collider, Position
from sage.platformer import Body, Look, Velocity

kit = world.platformer
Player, Tuning, Rules, Tally = kit.Player, kit.Tuning, kit.Rules, kit.Tally
LEFT, RIGHT, DOWN, JUMP, RUN, SPIN = kit.LEFT, kit.RIGHT, kit.DOWN, kit.JUMP, kit.RUN, kit.SPIN
GROUND, AIR, WALL, DUCK, ROLL, DIVE, SPIN_MODE, HURT = (
    kit.GROUND,
    kit.AIR,
    kit.WALL,
    kit.DUCK,
    kit.ROLL,
    kit.DIVE,
    kit.SPIN_MODE,
    kit.HURT,
)
art = kit.art
LOW_MODES = (DUCK, ROLL, DIVE)


def approach(value, target, step):
    """Moves `value` toward `target` by at most `step`."""
    if value < target:
        return min(value + step, target)
    return max(value - step, target)


def enter(p, pos, col, t, mode):
    """Switches mode. Low modes shrink the box from the top, so the feet stay
    where they are."""
    p.mode = mode
    p.timer = 0
    hh = t.roll_height if mode in LOW_MODES else p.stand_height
    if abs(col.hh - hh) > 1e-4:
        pos.y += hh - col.hh
        col.hh = hh


def can_stand(body, col, p):
    """Is there room above to grow back to full height?"""
    return body.headroom >= 2.0 * (p.stand_height - col.hh) - 0.01


def jump(c, bonus=0.0):
    """Leaves the ground (or a ledge, a moment late) with speed that grows a
    little with how fast you were running."""
    c.vel.y = c.t.jump_speed + abs(c.vel.x) * c.t.jump_speed_bonus + bonus
    c.p.rise = 1
    c.p.buffer = 0
    c.p.coyote = 0
    c.p.spun = 0
    c.tally.jumps += 1
    enter(c.p, c.pos, c.col, c.t, AIR)


def land(c):
    c.p.spun = 0
    c.p.rise = 0
    c.tally.lands += 1
    if c.p.mode == DIVE:
        c.tally.rolls += 1
        enter(c.p, c.pos, c.col, c.t, ROLL)
        c.vel.x = c.p.facing * max(abs(c.vel.x), c.t.roll_speed)
    else:
        enter(c.p, c.pos, c.col, c.t, GROUND)


def gravity(c, g, floor=None):
    c.vel.y = max(c.vel.y - g * c.dt, -c.t.max_fall if floor is None else -floor)


# ----- the modes -------------------------------------------------------------------


def ground(c):
    p, vel, t = c.p, c.vel, c.t
    if c.direction != 0:
        p.facing = c.direction
        braking = vel.x * c.direction < 0
        rate = t.turn_decel if braking else t.ground_accel
        vel.x = approach(vel.x, c.direction * c.top, rate * c.dt)
    else:
        vel.x = approach(vel.x, 0.0, t.ground_decel * c.dt)
    gravity(c, t.gravity_down)
    if not c.grounded:
        p.mode, p.timer = AIR, 0  # walked off a ledge; coyote time may still jump
    elif p.buffer > 0:
        jump(c)
    elif c.held(DOWN):
        if abs(vel.x) >= t.roll_min_speed:
            c.tally.rolls += 1
            enter(p, c.pos, c.col, t, ROLL)
            vel.x = p.facing * max(abs(vel.x), t.roll_speed)
        else:
            enter(p, c.pos, c.col, t, DUCK)


def air(c):
    p, vel, t = c.p, c.vel, c.t
    if c.grounded and vel.y <= 0:
        return land(c)
    if p.wall_lock == 0:
        if c.direction != 0:
            p.facing = c.direction
            # Only speed up: momentum you already have is kept.
            if vel.x * c.direction < c.top:
                vel.x = approach(vel.x, c.direction * c.top, t.air_accel * c.dt)
        else:
            vel.x = approach(vel.x, 0.0, t.air_decel * c.dt)
    # Letting go of JUMP while rising cuts the jump short.
    if p.rise:
        if vel.y <= 0:
            p.rise = 0
        elif not c.held(JUMP):
            vel.y = min(vel.y, t.jump_release_speed)
            p.rise = 0
    rising_held = p.rise and vel.y > 0 and c.held(JUMP)
    gravity(c, t.gravity_up if rising_held else t.gravity_down)

    if p.buffer > 0 and p.coyote > 0:
        return jump(c)
    if c.pressed & SPIN:
        if c.held(DOWN):
            return dive(c)
        if not p.spun:
            return spin(c)
    if c.body.wall != 0 and c.direction == c.body.wall and vel.y <= 0 and p.wall_lock == 0:
        p.facing = c.direction
        p.spun = 0
        enter(p, c.pos, c.col, t, WALL)


def spin(c):
    c.p.spun = 1
    c.tally.spins += 1
    c.vel.y = max(c.vel.y, c.t.spin_lift)
    c.p.rise = 0
    enter(c.p, c.pos, c.col, c.t, SPIN_MODE)


def dive(c):
    c.tally.dives += 1
    c.vel.x = c.p.facing * c.t.dive_speed
    c.vel.y = c.t.dive_lift
    c.p.rise = 0
    enter(c.p, c.pos, c.col, c.t, DIVE)


def spin_mode(c):
    p, vel, t = c.p, c.vel, c.t
    if c.grounded and vel.y <= 0:
        return land(c)
    if c.direction != 0:
        p.facing = c.direction
        if vel.x * c.direction < c.top:
            vel.x = approach(vel.x, c.direction * c.top, t.air_accel * c.dt)
    gravity(c, t.gravity_down, floor=t.spin_fall)
    if p.timer >= t.spin_ticks:
        enter(p, c.pos, c.col, t, AIR)


def dive_mode(c):
    p, vel, t = c.p, c.vel, c.t
    if c.grounded and vel.y <= 0:
        return land(c)
    gravity(c, t.dive_gravity)
    if vel.x == 0.0 and p.timer > 2 and can_stand(c.body, c.col, p):
        enter(p, c.pos, c.col, t, AIR)  # ran into a wall: tumble


def wall_mode(c):
    p, vel, t = c.p, c.vel, c.t
    if c.grounded:
        return land(c)
    # A jump press kicks off the wall, whichever way you are leaning: lean away
    # to go across, or keep pushing in to go up the same wall.
    if p.buffer > 0 and c.body.wall != 0:
        c.tally.wall_jumps += 1
        vel.x = -c.body.wall * t.wall_jump_x
        vel.y = t.wall_jump_y
        p.facing = -c.body.wall
        p.wall_lock = t.wall_lock_ticks
        p.rise = 1
        p.buffer = 0
        return enter(p, c.pos, c.col, t, AIR)
    if c.body.wall == 0 or c.direction != c.body.wall:
        enter(p, c.pos, c.col, t, AIR)
        return gravity(c, t.gravity_down)
    vel.x = 0.0
    gravity(c, t.gravity_down, floor=t.wall_slide_speed)


def duck(c):
    p, vel, t = c.p, c.vel, c.t
    if c.direction != 0:
        p.facing = c.direction
    vel.x = approach(vel.x, 0.0, t.ground_decel * c.dt)
    gravity(c, t.gravity_down)
    standing_room = can_stand(c.body, c.col, p)
    if not c.grounded:
        if standing_room:
            enter(p, c.pos, c.col, t, AIR)
    elif c.pressed & JUMP and c.held(DOWN):
        c.body.drop_through = 1  # fall through a one-way platform
        p.buffer = 0
    elif p.buffer > 0 and standing_room:
        jump(c)
    elif not c.held(DOWN) and standing_room:
        enter(p, c.pos, c.col, t, GROUND)


def roll(c):
    p, vel, t = c.p, c.vel, c.t
    gravity(c, t.gravity_down)
    if c.grounded and vel.y <= 0:
        vel.x = approach(vel.x, p.facing * t.walk_speed * 0.5, t.roll_decel * c.dt)
    standing_room = can_stand(c.body, c.col, p)
    if p.buffer > 0 and c.grounded and standing_room:
        return jump(c, bonus=10.0)  # a roll's speed carries into the jump
    over = p.timer >= t.roll_ticks or abs(vel.x) < 1.0
    if over and standing_room:
        enter(p, c.pos, c.col, t, GROUND if c.grounded else AIR)
    elif over:
        # Stuck under a low ceiling: crawl, steering allowed.
        vel.x = (c.direction or p.facing) * 40.0


def hurt_mode(c):
    p, vel, t = c.p, c.vel, c.t
    gravity(c, t.gravity_down)
    if p.timer >= c.r.hurt_ticks:
        enter(p, c.pos, c.col, t, GROUND if c.grounded else AIR)


MODES = {
    GROUND: ground,
    AIR: air,
    WALL: wall_mode,
    DUCK: duck,
    ROLL: roll,
    DIVE: dive_mode,
    SPIN_MODE: spin_mode,
    HURT: hurt_mode,
}


# ----- the system ------------------------------------------------------------------------


class Tick:
    """A bag of the things one tick of the rules needs."""

    def __init__(self, **values):
        self.__dict__.update(values)


@kit.play.system(phase=kit.phases.input)
def player_control(
    pos: Position, vel: Velocity, body: Body, col: Collider, p: Player, look: Look
):
    """Reads the buttons, runs the rules of the current mode, sets the color."""
    buttons = world.get(Input).buttons
    pressed = buttons & ~p.prev
    p.prev = buttons

    def held(bit):
        return buttons & bit != 0

    t = world.get(Tuning)
    c = Tick(
        pos=pos, vel=vel, body=body, col=col, p=p, look=look, t=t, r=world.get(Rules),
        tally=kit.game.get(Tally), pressed=pressed, held=held, dt=sage.TICK_DT,
        direction=(1 if held(RIGHT) else 0) - (1 if held(LEFT) else 0),
        top=t.run_speed if held(RUN) else t.walk_speed,
        grounded=body.grounded != 0,
    )
    if p.invuln > 0:
        p.invuln -= 1
    if p.wall_lock > 0:
        p.wall_lock -= 1
    p.timer += 1
    if pressed & JUMP:
        p.buffer = t.buffer_ticks
    elif p.buffer > 0:
        p.buffer -= 1
    if c.grounded:
        p.coyote = t.coyote_ticks
    elif p.coyote > 0:
        p.coyote -= 1

    MODES[p.mode](c)

    # Plain boxes need a way to show the mode: the color. (A sprite does not
    # get tinted; the artist draws the modes.)
    mode = p.mode
    if mode in (DUCK, ROLL):
        color = art.player_duck
    elif mode == DIVE:
        color = art.player_dive
    elif mode == SPIN_MODE:
        color = art.player_spin if p.timer % 4 < 2 else art.player
    elif mode == HURT:
        color = art.player_hurt
    else:
        color = art.player
    if look.image != 0:
        color = 0xFFFFFFFF  # an artist's sprite is shown as drawn, not tinted
    if p.invuln > 0 and (p.invuln // 3) % 2 == 0:
        color = (color & 0xFFFFFF00) | 0x70  # blink while invulnerable
    look.color = color
    look.flip = 1 if p.facing < 0 else 0
    if look.cols > 1:
        look.frame = mode  # a sheet with a cell per mode, in MODE_NAMES order


kit.enter = enter
kit.can_stand = can_stand
kit.approach = approach
