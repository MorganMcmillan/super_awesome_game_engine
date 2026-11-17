# Grassland: a platformer made from the platformer kit.
#
# Everything about how the player moves, how enemies and coins behave and how
# the camera works is in the kit (crates/sage_platformer/scripts/*.py). This
# file is only what is different about *this* game:
#
#   1. a couple of tuning numbers
#   2. the sky
#   3. the level, as a picture made of text
#   4. one new rule of its own: every ten coins earn a heart
#
# Delete any of it, or change it, and the kit still works.

import sage
from sage import component, world
from sage.platformer import Backdrop

kit = world.platformer

# ----- 1. tuning ---------------------------------------------------------------------
# Numbers live in `Tuning` (the player's movement) and `Rules` (everything
# else); see crates/sage_platformer/scripts/vocabulary.py for all of them.
kit.tune(run_speed=150.0, gravity_down=1250.0)

# ----- 2. the sky ----------------------------------------------------------------------
world.set(
    Backdrop(
        sky_top=kit.color(70, 130, 230),
        sky_bottom=kit.color(176, 220, 250),
        far=kit.color(120, 160, 210),
        near=kit.color(88, 146, 112),
    )
)

# ----- 3. the level --------------------------------------------------------------------
#   #  block   =  one-way platform   ^  spikes   o  coin   @  start
#   c  checkpoint   e  walker   h  platform sliding sideways   v  sliding up and down
# The chimney near the end is climbed by jumping from wall to wall.
LEVEL = """
............................................................................................................................
.............................................................................................===............................
............................................................................................#...#...........................
............................................................................................#.o.#...........................
............................................................................................#...#...........................
............................................................................................#.o.#...........................
............................................................................................#...#...........................
.................................................................................o..........#.o.#...........................
.................o...........oooo..............o............................................#...#...........................
........ooo.................======..........o.###.o.............................hhh..............................ooooooo....
.........................................o.#########.........ooo........ooooo...............................................
...@.........##................e....^^..###############..c......................e.......e...........c....e..^^.e.........o..
################...#########################################..=..###########################################################
################...#########################################.....###########################################################
"""

kit.load_level(LEVEL)

# ----- 4. a rule of this game ------------------------------------------------------------


@component
class Bonus:
    """How many hearts the coins have already paid for."""

    paid: sage.u32 = 0


kit.game.set(Bonus())


@kit.play.system(phase=kit.phases.react)
def ten_coins_make_a_heart(bonus: Bonus, score: kit.Score):
    earned = score.coins // 10
    if earned > bonus.paid:
        bonus.paid = earned
        health = world.lookup(kit.ID_PLAYER).get(kit.Health)
        health.max = min(health.max + 1, 6)
        health.hp = min(health.hp + 1, health.max)
