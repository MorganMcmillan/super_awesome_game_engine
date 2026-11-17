"""Tests for sage.provenance. Run by `cargo test -p sage_editor --test blocks`
(inside the embedded interpreter) and by `python -m unittest` with
`python/vendor` and `python/sage` on the path."""

import unittest

import provenance as P

GAME = '''import sage
from sage import component, Entity


@component
class Position:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0


Move = world.phase("Move")
Draw = world.phase("Draw", after=Move)


@world.system("move", phase=Move, query="Position, Velocity")
def move(pos: Position, vel: Velocity):
    pos.x += vel.x * 0.1
    pos.y = pos.y + vel.y


@world.system("wrap", phase=Move)
def wrap(pos: Position):
    if pos.x > 160:
        pos.x = 0


@world.system("draw", phase=Draw)
def draw(e: Entity, pos: Position):
    show(pos.x, pos.y)


@world.observer(Hit)
def on_hit(e: Entity, hit: Hit):
    e.set(Velocity(0, 0))
    e.get(Health).points -= hit.damage
    e.add(Stunned)


def helper(pos):
    pos.x = 99
'''


def rows(source, system):
    return sorted((r["component"], r["field"], r["mode"]) for r in P.accesses(source) if r["system"] == system)


class Accesses(unittest.TestCase):
    def test_assigning_a_field_writes_it_and_using_one_reads_it(self):
        self.assertEqual(
            rows(GAME, "move"),
            [
                ("Position", "x", "read"),  # += reads what it changes
                ("Position", "x", "write"),
                ("Position", "y", "read"),
                ("Position", "y", "write"),
                ("Velocity", "x", "read"),
                ("Velocity", "y", "read"),
            ],
        )
        self.assertEqual(rows(GAME, "draw"), [("Position", "x", "read"), ("Position", "y", "read")])

    def test_a_plain_assignment_does_not_read_the_field(self):
        self.assertEqual(rows(GAME, "wrap"), [("Position", "x", "read"), ("Position", "x", "write")])
        only = GAME.replace("    if pos.x > 160:\n        pos.x = 0\n", "    pos.x = 0\n")
        self.assertEqual(rows(only, "wrap"), [("Position", "x", "write")])

    def test_set_add_remove_write_a_whole_component_and_get_reaches_a_field(self):
        self.assertEqual(
            rows(GAME, "on_hit"),
            [
                ("Health", "points", "read"),
                ("Health", "points", "write"),
                ("Hit", "damage", "read"),
                ("Stunned", "*", "write"),
                ("Velocity", "*", "write"),
            ],
        )

    def test_rules_carry_their_phase_and_line_and_helpers_are_not_rules(self):
        all_rows = P.accesses(GAME)
        self.assertEqual({r["system"] for r in all_rows}, {"move", "wrap", "draw", "on_hit"})
        first = next(r for r in all_rows if r["system"] == "move")
        self.assertEqual(first["phase"], "Move")
        self.assertEqual(GAME.split("\n")[first["line"] - 1].strip(), "pos.x += vel.x * 0.1")
        self.assertIsNone(next(r for r in all_rows if r["system"] == "on_hit")["phase"])

    def test_a_locked_function_is_reported_as_unknown_not_left_out(self):
        src = GAME + '\n\n@world.system("odd")\ndef odd(pos: Position):\n    with thing():\n        pos.x = 1\n'
        self.assertEqual(rows(src, "odd"), [("*", "*", "unknown")])


class Table(unittest.TestCase):
    def setUp(self):
        self.tab = P.table({"game": GAME})

    def test_who_touches_a_field_in_source_order(self):
        who = P.of_field(self.tab, "Position", "x")
        self.assertEqual(who["write"], ["move", "wrap"])
        self.assertEqual(who["read"], ["move", "wrap", "draw"])
        self.assertEqual(P.of_field(self.tab, "Position", "y")["write"], ["move"])
        self.assertEqual(P.of_field(self.tab, "Nothing", "x"), {"read": [], "write": [], "unknown": []})

    def test_a_whole_component_write_counts_for_each_of_its_fields(self):
        self.assertEqual(P.of_field(self.tab, "Velocity", "x")["write"], ["on_hit"])
        self.assertEqual(P.of_field(self.tab, "Velocity", "x")["read"], ["move"])

    def test_a_rule_knows_its_script_and_several_scripts_merge(self):
        tab = P.table({"b": '@world.system("late")\ndef late(pos: Position):\n    pos.x = 1\n', "a": GAME})
        self.assertEqual(P.of_field(tab, "Position", "x")["write"], ["move", "wrap", "late"])
        self.assertEqual(tab["Position"]["x"]["write"][-1]["script"], "b")

    def test_a_script_that_is_not_valid_python_is_skipped(self):
        tab = P.table({"good": GAME, "typing": "def broken(:\n"})
        self.assertEqual(P.of_field(tab, "Position", "y")["write"], ["move"])

    def test_two_writers_in_one_phase_are_a_conflict(self):
        self.assertEqual(
            P.conflicts(self.tab),
            [{"component": "Position", "field": "x", "phase": "Move", "systems": ["move", "wrap"]}],
        )
        # In different phases the order is written down: no conflict.
        apart = GAME.replace('@world.system("wrap", phase=Move)', '@world.system("wrap", phase=Draw)')
        self.assertEqual(P.conflicts(P.table({"game": apart})), [])

    def test_a_whole_component_writer_conflicts_with_a_field_writer(self):
        src = GAME + '\n\n@world.system("reset", phase=Move)\ndef reset(e: Entity):\n    e.set(Position(0, 0))\n'
        found = P.conflicts(P.table({"game": src}))
        self.assertIn({"component": "Position", "field": "y", "phase": "Move", "systems": ["move", "reset"]}, found)

    def test_a_locked_function_shows_up_for_every_field(self):
        src = GAME + '\n\n@world.system("odd")\ndef odd(pos: Position):\n    with thing():\n        pos.x = 1\n'
        who = P.of_field(P.table({"game": src}), "Position", "y")
        self.assertEqual(who["unknown"], ["odd"])

    def test_the_sentence_names_writers_then_readers(self):
        self.assertEqual(P.sentence(P.of_field(self.tab, "Position", "y")), "Written by move. Read by move, draw.")
        self.assertEqual(P.sentence(P.of_field(self.tab, "Hit", "damage")), "Read by on_hit.")
        self.assertEqual(P.sentence(P.of_field(self.tab, "Nothing", "x")), "")
        self.assertEqual(
            P.sentence({"read": [], "write": [], "unknown": ["odd"]}), "Maybe used by odd (locked)."
        )


def run():
    """For the embedded interpreter: runs every test, returns failures as text."""
    suite = unittest.defaultTestLoader.loadTestsFromModule(__import__(__name__))
    import io

    out = io.StringIO()
    res = unittest.TextTestRunner(stream=out, verbosity=0).run(suite)
    return "" if res.wasSuccessful() else out.getvalue()


if __name__ == "__main__":
    unittest.main()
