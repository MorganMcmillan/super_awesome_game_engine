# The platformer kit, part 5 of 5: the rules of the game itself.
#
#   state machine   PLAYING, PAUSED and DEAD, and what entering each one does
#   falling out     below the level is a fall: health to zero
#   face            the eye follows the player
#   camera          follows the player, looks ahead, stays inside the level
#
# A game state is data (`Game.state`), not a pipeline phase. Pausing is the
# `play` flecs module being disabled, which switches off every system and
# observer in it, native ones included.

from sage import Input, world
from sage.collision import Collider, Position
from sage.platformer import Camera, Velocity

kit = world.platformer
Tuning, Rules, Game, Player, Health, Respawn, Score, Tally, Face = (
    kit.Tuning,
    kit.Rules,
    kit.Game,
    kit.Player,
    kit.Health,
    kit.Respawn,
    kit.Score,
    kit.Tally,
    kit.Face,
)
PLAYING, PAUSED, DEAD, NO_STATE = kit.PLAYING, kit.PAUSED, kit.DEAD, kit.NO_STATE
ID_PLAYER, PAUSE = kit.ID_PLAYER, kit.PAUSE
play, core = kit.play, kit.core


def respawn():
    """Puts the player back at the last checkpoint with full health."""
    player = world.lookup(ID_PLAYER)
    t = world.get(Tuning)
    pos, vel, col = player.get(Position), player.get(Velocity), player.get(Collider)
    p, spot, health = player.get(Player), player.get(Respawn), player.get(Health)
    pos.x, pos.y = spot.x, spot.y
    vel.x = vel.y = 0.0
    col.hh = p.stand_height
    health.hp = health.max
    p.invuln = 60
    p.wall_lock = p.buffer = p.coyote = p.rise = p.spun = 0
    p.mode, p.timer = kit.AIR, 0
    cam = world.get(Camera)
    cam.x, cam.y = pos.x, pos.y


def enter_state(g, state):
    """What entering a state does. Module switches come last: they change the
    world's structure."""
    t = world.get(Tuning)
    if state == PLAYING:
        if g.state == DEAD:
            respawn()
        g.state = state
        play.enable()
    elif state == PAUSED:
        g.state = state
        play.disable()
    else:
        g.state = state
        g.timer = world.get(Rules).respawn_ticks
        kit.game.get(Score).deaths += 1
        kit.game.get(Tally).deaths += 1
        play.disable()


# Immediate: entering a state toggles a flecs module, and doing it directly
# keeps the order of effects obvious.
@core.system(phase=kit.phases.input, immediate=True)
def game_state_machine(g: Game):
    buttons = world.get(Input).buttons
    pressed = buttons & ~g.prev
    g.prev = buttons
    if g.pending == NO_STATE:
        if g.state == PLAYING and pressed & PAUSE:
            g.pending = PAUSED
        elif g.state == PAUSED and pressed & PAUSE:
            g.pending = PLAYING
        elif g.state == DEAD:
            g.timer = max(g.timer - 1, 0)
            if g.timer == 0:
                g.pending = PLAYING
    if g.pending != NO_STATE:
        state = g.pending
        g.pending = NO_STATE
        enter_state(g, state)


@play.system(phase=kit.phases.react)
def fall_out_of_the_world(pos: Position, p: Player, health: Health):
    if pos.y < -world.get(Rules).fall_death and health.hp > 0:
        health.hp = 0
        kit.game.get(Game).pending = DEAD


@play.system(phase=kit.phases.camera, query="Face")
def face_follows_the_player(pos: Position):
    player = world.lookup(ID_PLAYER)
    at, size, p = player.get(Position), player.get(Collider), player.get(Player)
    pos.x = at.x + p.facing * 2.5
    pos.y = at.y + size.hh * 0.45


@play.system(phase=kit.phases.camera)
def camera_follows_the_player(pos: Position, p: Player):
    r = world.get(Rules)
    g = kit.game.get(Game)
    cam = world.get(Camera)
    cam.x += (pos.x + p.facing * r.camera_lead - cam.x) * r.camera_ease
    if pos.y - cam.y > r.camera_dead_y:
        cam.y = pos.y - r.camera_dead_y
    elif cam.y - pos.y > r.camera_dead_y:
        cam.y = pos.y + r.camera_dead_y
    half_w, half_h = cam.w / 2, cam.h / 2
    cam.x = min(max(cam.x, half_w), max(g.width - half_w, half_w))
    cam.y = min(max(cam.y, half_h), max(g.height - half_h, half_h))


kit.respawn = respawn
