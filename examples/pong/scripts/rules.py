# Pong's scoring rule (design doc Appendix A):
#
#   on goal(side):
#       scorer = side.opponent
#       scorer.points += 1
#       go to GameOver if scorer.points == max_points, else Serving
#
# Simulation rules must be deterministic: no time, no randomness, and no
# dependence on set iteration order.

import sage


@sage.on("goal")
def score_goal(world, goal):
    scorer = goal.scorer  # side.opponent, from the (Opponent, player) relationship
    world.add_points(scorer, 1)
    if world.points(scorer) == world.max_points:
        world.go_to("GameOver", winner=scorer)
    else:
        world.go_to("Serving")
