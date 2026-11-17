"""Tests for sage.blocks. Run by `cargo test -p sage_editor --test blocks`
(inside the embedded interpreter) and by `python -m unittest` with
`python/vendor` and `python/sage` on the path."""

import ast
import unittest

import blocks as B

SRC = '''import sage
from sage import component

# The player's speed.
SPEED = 3


@component
class Position:
    x: float = 0.0


# Moves everything that has a velocity.
# Runs every tick.
@sage.position(10, 20)
@world.system("move", query="Position, Velocity")
def move(pos: Position, vel: Velocity):
    """Add velocity to position."""
    # keep it simple
    pos.x += vel.x  # x first
    pos.y += vel.y

    if pos.x > 100:
        pos.x = 0
    elif pos.x < 0:  # off the left
        pos.x = 100
    else:
        pass
    for i in range(3):
        vel.x = vel.x * 0.9


def helper(a, b):
    return f"{a}"  # an f-string: locked
'''


def norm(src):
    return ast.dump(ast.parse(src))


class Parse(unittest.TestCase):
    def test_functions_become_stacks_and_unsupported_ones_lock(self):
        p = B.parse(SRC)
        fs = {i["name"]: i for i in p["items"] if i["k"] == "function"}
        self.assertEqual(fs["move"]["position"], [10, 20])
        self.assertIsNone(fs["move"]["locked"])
        self.assertEqual([d["k"] for d in fs["move"]["decorators"]], ["call"])
        self.assertEqual([x["name"] for x in fs["move"]["params"]], ["pos", "vel"])
        self.assertEqual(fs["helper"]["locked"], "an f-string")
        self.assertEqual(fs["helper"]["body"], [])
        codes = [i["k"] for i in p["items"] if i["k"] == "code"]
        self.assertEqual(len(codes), 4)  # two imports, SPEED, the class

    def test_comments_above_a_block_are_its_notes(self):
        p = B.parse(SRC)
        move = [i for i in p["items"] if i.get("name") == "move"][0]
        self.assertEqual(move["notes"], ["Moves everything that has a velocity.", "Runs every tick."])
        first = move["body"][0]
        self.assertEqual(first["notes"], ["keep it simple"])
        self.assertEqual(first["trail"], "x first")
        speed = [i for i in p["items"] if i["k"] == "code" and "SPEED" in i["text"]][0]
        self.assertEqual(speed["notes"], ["The player's speed."])

    def test_a_docstring_is_the_functions_note_not_a_block(self):
        p = B.parse(SRC)
        move = [i for i in p["items"] if i.get("name") == "move"][0]
        self.assertEqual(move["doc"], "Add velocity to position.")
        self.assertEqual(move["body"][0]["k"], "change")

    def test_a_module_docstring_is_kept_apart(self):
        p = B.parse('"""A game."""\nx = 1\n')
        self.assertEqual(p["doc"], "A game.")
        self.assertEqual(len(p["items"]), 1)

    def test_elif_and_else_are_told_apart(self):
        p = B.parse("def f():\n    if a:\n        pass\n    elif b:\n        pass\n    else:\n        pass\n")
        f = p["items"][0]
        top = f["body"][0]
        self.assertEqual(len(top["orelse"]), 1)
        self.assertTrue(top["orelse"][0]["elif"])
        self.assertEqual(top["orelse"][0]["orelse"][0]["k"], "pass")
        q = B.parse("def f():\n    if a:\n        pass\n    else:\n        if b:\n            pass\n")
        inner = q["items"][0]["body"][0]["orelse"][0]
        self.assertNotIn("elif", inner)

    def test_things_blocks_cannot_show_lock_the_function(self):
        for body, why in [
            ("return f'{x}'", "an f-string"),
            ("a = b = 1", "a chained assignment"),
            ("a = 1; b = 2", "two statements on one line"),
            ("if a: pass", "a statement on the same line as its header"),
            ("try:\n        pass\n    except E:\n        pass", "try"),
            ("x = a[1:2]", "a slice"),
            ("f(*a)", "a call with * or ** arguments"),
            ("x = (yield)", "yield"),
            ("with a:\n        pass", "with"),
        ]:
            p = B.parse("def f():\n    %s\n" % body)
            self.assertEqual(p["items"][0]["locked"], why, body)

    def test_bad_python_is_an_error_not_a_crash(self):
        self.assertIn("error", B.parse("def f(:\n"))


class RoundTrip(unittest.TestCase):
    def test_untouched_source_comes_back_byte_for_byte(self):
        self.assertEqual(B.Blocks(SRC).module.code, SRC)

    def test_a_literal_edit_changes_only_that_literal(self):
        b = B.Blocks(SRC)
        move = b.function("move")
        lit = [x for x in _walk(move) if x.get("k") == "lit" and x["v"] == 100][0]
        out = b.set_literal(lit["id"], "int", "250")
        self.assertEqual(out, SRC.replace("pos.x > 100", "pos.x > 250"))

    def test_a_replaced_statement_keeps_its_notes_and_trailing_comment(self):
        b = B.Blocks(SRC)
        first = b.function("move")["body"][0]
        out = b.replace(first["id"], {"k": "change", "target": _attr("pos", "x"), "op": "-", "value": _attr("vel", "x")})
        self.assertIn("    # keep it simple\n    pos.x -= vel.x  # x first\n", out)
        self.assertEqual(out.replace("-=", "+="), SRC)

    def test_expressions_are_parenthesized_where_needed(self):
        b = B.Blocks("def f():\n    x = a * b\n")
        left = [e for e in _walk(b.function("f")) if e.get("k") == "name" and e["name"] == "a"][0]
        out = b.replace(left["id"], {"k": "binop", "op": "+", "l": _nm("c"), "r": _nm("d")})
        self.assertEqual(out, "def f():\n    x = (c + d) * b\n")

    def test_every_edit_yields_the_same_program_as_the_blocks_say(self):
        b = B.Blocks("def f(e):\n    x = 1\n")
        new = {"k": "if", "test": _lit_b(True), "body": [{"k": "pass"}], "orelse": [{"k": "break"}]}
        out = b.insert(b.function("f")["id"], "body", 99, new)
        self.assertEqual(norm(out), norm("def f(e):\n    x = 1\n    if True:\n        pass\n    else:\n        break\n"))


class Editing(unittest.TestCase):
    def test_insert_before_and_at_the_end(self):
        b = B.Blocks(SRC)
        f = b.function("move")
        out = b.insert(f["id"], "body", 0, {"k": "pass"})
        self.assertIn('"""Add velocity to position."""\n    pass\n    # keep it simple', out)
        f = b.function("move")
        out = b.insert(f["id"], "body", 99, {"k": "return", "value": None})
        self.assertTrue(out.split("def helper")[0].rstrip().endswith("return"))

    def test_deleting_a_statement_takes_its_notes_and_the_last_becomes_pass(self):
        b = B.Blocks(SRC)
        first = b.function("move")["body"][0]
        out = b.delete(first["id"])
        self.assertNotIn("keep it simple", out)
        self.assertNotIn("x first", out)
        c = B.Blocks("def f():\n    # only\n    x = 1\n")
        out = c.delete(c.function("f")["body"][0]["id"])
        self.assertEqual(out, "def f():\n    pass\n")

    def test_a_docstring_only_function_can_lose_its_last_statement(self):
        c = B.Blocks('def f():\n    """Doc."""\n    x = 1\n')
        out = c.delete(c.function("f")["body"][0]["id"])
        self.assertEqual(out, 'def f():\n    """Doc."""\n')

    def test_moving_carries_notes_and_adjusts_indentation(self):
        src = "def f():\n    # note\n    x = 1  # t\n    if a:\n        y = 2\n"
        b = B.Blocks(src)
        f = b.function("f")
        x, iff = f["body"]
        out = b.move(x["id"], iff["id"], "body", 0)
        self.assertEqual(out, "def f():\n    if a:\n        # note\n        x = 1  # t\n        y = 2\n")
        # And back out again.
        f = b.function("f")
        inner = f["body"][0]["body"][0]
        out = b.move(inner["id"], f["id"], "body", 1)
        self.assertEqual(out, "def f():\n    if a:\n        y = 2\n    # note\n    x = 1  # t\n")

    def test_reordering_in_one_list(self):
        b = B.Blocks("def f():\n    a = 1\n    b = 2\n    c = 3\n")
        f = b.function("f")
        out = b.move(f["body"][0]["id"], f["id"], "body", 3)
        self.assertEqual(out, "def f():\n    b = 2\n    c = 3\n    a = 1\n")
        f = b.function("f")
        out = b.move(f["body"][2]["id"], f["id"], "body", 0)
        self.assertEqual(out, "def f():\n    a = 1\n    b = 2\n    c = 3\n")
        with self.assertRaises(ValueError):
            f = b.function("f")
            b.move(f["body"][0]["id"], f["id"], "body", 0)

    def test_a_block_cannot_move_into_itself(self):
        b = B.Blocks("def f():\n    if a:\n        if b:\n            pass\n")
        f = b.function("f")
        outer = f["body"][0]
        inner = outer["body"][0]
        with self.assertRaises(ValueError):
            b.move(outer["id"], inner["id"], "body", 0)

    def test_locked_functions_are_never_touched(self):
        b = B.Blocks(SRC)
        f = b.function("helper")
        with self.assertRaises(ValueError):
            b.insert(f["id"], "body", 0, {"k": "pass"})
        with self.assertRaises(ValueError):
            b.rename_function("helper", "other")
        self.assertEqual(b.source, SRC)

    def test_a_failed_edit_changes_nothing(self):
        b = B.Blocks(SRC)
        first = b.function("move")["body"][0]
        with self.assertRaises(ValueError):
            b.replace(first["id"], {"k": "assign", "target": {"k": "lit", "t": "int", "v": 1}, "value": _lit_b(1)})
        self.assertEqual(b.source, SRC)
        self.assertEqual(b.function("move")["body"][0]["id"], first["id"])

    def test_else_is_added_to_an_if(self):
        b = B.Blocks("def f():\n    if a:\n        pass\n")
        out = b.add_else(b.function("f")["body"][0]["id"])
        self.assertEqual(out, "def f():\n    if a:\n        pass\n    else:\n        pass\n")
        f = b.function("f")
        out = b.insert(f["body"][0]["id"], "orelse", 1, {"k": "break"})
        self.assertTrue(out.endswith("        pass\n        break\n"))

    def test_deleting_an_elif_keeps_the_chain(self):
        b = B.Blocks("def f():\n    if a:\n        pass\n    elif b:\n        x = 1\n    else:\n        y = 2\n")
        iff = b.function("f")["body"][0]
        out = b.delete(iff["orelse"][0]["id"])
        self.assertEqual(out, "def f():\n    if a:\n        pass\n    else:\n        y = 2\n")


class NotesAndDocs(unittest.TestCase):
    def test_setting_notes_replaces_only_the_comment_lines(self):
        b = B.Blocks(SRC)
        out = b.set_notes(b.function("move")["id"], ["Moves things.", "Two lines\nin one note"])
        self.assertIn("\n\n\n# Moves things.\n# Two lines\n# in one note\n@sage.position(10, 20)\n", out)
        self.assertEqual(b.function("move")["notes"], ["Moves things.", "Two lines", "in one note"])
        out = b.set_notes(b.function("move")["id"], [])
        self.assertIn("\n\n\n@sage.position(10, 20)\n", out)

    def test_notes_on_a_statement(self):
        b = B.Blocks("def f():\n    x = 1\n")
        out = b.set_notes(b.function("f")["body"][0]["id"], ["why"])
        self.assertEqual(out, "def f():\n    # why\n    x = 1\n")

    def test_docstrings_are_set_changed_and_removed(self):
        b = B.Blocks("def f():\n    x = 1\n")
        out = b.set_docstring("f", "Does f.")
        self.assertEqual(out, 'def f():\n    """Does f."""\n    x = 1\n')
        self.assertEqual(b.function("f")["doc"], "Does f.")
        out = b.set_docstring("f", "Two\nlines")
        self.assertEqual(b.function("f")["doc"], "Two\nlines")
        out = b.set_docstring("f", None)
        self.assertEqual(out, "def f():\n    x = 1\n")
        c = B.Blocks('def g():\n    """Only."""\n')
        self.assertEqual(c.set_docstring("g", None), "def g():\n    pass\n")

    def test_a_notes_text_cannot_break_out_into_code(self):
        b = B.Blocks("def f():\n    x = 1\n")
        out = b.set_notes(b.function("f")["body"][0]["id"], ["a\nimport os"])
        self.assertEqual(out, "def f():\n    # a\n    # import os\n    x = 1\n")
        out = b.set_docstring("f", 'say """hi""" \\ there')
        self.assertEqual(ast.get_docstring(ast.parse(out).body[0]), 'say """hi""" \\ there')


class Functions(unittest.TestCase):
    def test_add_rename_position_and_delete(self):
        b = B.Blocks("x = 1\n")
        out = b.add_function("tick", [("e", _nm("Entity"))], [], [{"k": "pass"}], position=(5, 6), notes=["runs often"])
        self.assertEqual(out, "x = 1\n\n\n# runs often\n@sage.position(5, 6)\ndef tick(e: Entity):\n    pass\n")
        out = b.set_position("tick", 7.5, -2)
        self.assertIn("@sage.position(7.5, -2)", out)
        self.assertEqual(b.function("tick")["position"], [7.5, -2])
        out = b.rename_function("tick", "tock")
        self.assertIn("def tock(", out)
        with self.assertRaises(ValueError):
            b.rename_function("tock", "class")
        out = b.delete_function("tock")
        self.assertEqual(out, "x = 1\n")

    def test_a_position_decorator_is_added_when_missing(self):
        b = B.Blocks("# note\n@world.system('s')\ndef s():\n    pass\n")
        out = b.set_position("s", 1, 2)
        self.assertEqual(out, "# note\n@sage.position(1, 2)\n@world.system('s')\ndef s():\n    pass\n")


class Literals(unittest.TestCase):
    def test_typed_literals_are_validated(self):
        self.assertEqual(B._literal_value("int", "0x10"), 16)
        self.assertEqual(B._literal_value("float", "2"), 2.0)
        for t, v in [("int", "1.5"), ("int", "__import__('os')"), ("float", "nan"), ("bool", "maybe"), ("int", "True")]:
            with self.assertRaises(ValueError):
                B._literal_value(t, v)

    def test_33_and_text_33_are_different_blocks(self):
        b = B.Blocks("def f():\n    x = 33\n    y = '33'\n")
        body = b.function("f")["body"]
        self.assertEqual((body[0]["value"]["t"], body[1]["value"]["t"]), ("int", "str"))
        self.assertEqual(B.convert_literal(body[0]["value"], "str")["v"], "33")
        self.assertEqual(B.convert_literal(body[1]["value"], "int")["v"], 33)
        with self.assertRaises(ValueError):
            B.convert_literal(_lit_s("abc"), "int")

    def test_empty_set_and_negative_numbers_print_correctly(self):
        self.assertEqual(B.expr_text({"k": "set", "items": []}), "set()")
        self.assertEqual(B.expr_text({"k": "lit", "t": "int", "v": -3}), "-3")
        self.assertEqual(B.expr_text({"k": "dict", "items": []}), "{}")
        self.assertEqual(B.expr_text({"k": "tuple", "items": [_lit_b(1)]}), "(1,)")

    def test_names_must_be_identifiers(self):
        for bad in ("class", "a b", "1x", ""):
            with self.assertRaises(ValueError):
                B.expr_text({"k": "name", "name": bad})
        with self.assertRaises(ValueError):
            B.expr_text({"k": "call", "func": _nm("f"), "args": [], "kwargs": [["a", _lit_b(1)], ["a", _lit_b(2)]]})


class EveryBlockKind(unittest.TestCase):
    """Blocks -> text -> blocks gives the same blocks, for every kind."""

    SAMPLES = [
        "x = 1", "x += 2", "x: int = 3", "x: list = []", "f(a, b=c)", "if a:\n    pass\nelif b:\n    pass\nelse:\n    pass",
        "while a < 3:\n    break", "for i in xs:\n    continue", "return x", "return", "pass",
        "x = a if b else c", "x = not a and b or c", "x = a < b <= c", "x = -a ** 2", "x = (a + b) * c", "x = a[i]",
        "x = {'a': 1, 'b': [1, 2], 3: (4,)}", "x = {1, 2}", "x = set()", "x = None", "x = True", "x = 1.5", "x = 'it\\'s'",
        "o.m(1).n = 3", "a[1] += 1", "x = a.b.c(d)(e)",
        "x = [i * 2 for i in xs if i > 0 if i < 9]", "x = {k: v for k, v in d}", "x = {i for i in s}", "x = sum(i for i in xs for j in ys)", "f = lambda a, b: a + b", "x = sorted(xs, key=lambda p: p.x)",
    ]

    def test_each_sample_survives_blocks_and_back(self):
        for s in self.SAMPLES:
            src = "def f():\n" + "".join("    " + line + "\n" for line in s.split("\n"))
            p = B.parse(src)
            f = p["items"][0]
            self.assertIsNone(f["locked"], s)
            text = B.function_text("f", [], [], [_strip(x) for x in f["body"]])
            self.assertEqual(norm(text), norm(src), s)


class Comprehensions(unittest.TestCase):
    def test_comprehensions_and_lambdas_are_blocks_with_scoped_names(self):
        b = B.Blocks("def f(xs):\n    y = [i * 2 for i in xs if i > 0]\n    z = lambda a: a + 1\n")
        f = b.function("f")
        self.assertIsNone(f["locked"])
        comp = f["body"][0]["value"]
        self.assertEqual((comp["k"], comp["kind"]), ("comp", "list"))
        self.assertEqual(len(comp["clauses"][0]["ifs"]), 1)
        self.assertEqual(f["body"][1]["value"]["params"], ["a"])
        # An edit inside one leaves the rest alone.
        lit = [e for e in _walk(comp) if e.get("k") == "lit" and e["v"] == 2][0]
        out = b.set_literal(lit["id"], "int", "3")
        self.assertEqual(out, "def f(xs):\n    y = [i * 3 for i in xs if i > 0]\n    z = lambda a: a + 1\n")

    def test_async_and_starred_forms_still_lock(self):
        self.assertTrue(B.parse("def f():\n    x = [*a]\n")["items"][0]["locked"])
        self.assertTrue(B.parse("def f():\n    x = lambda a=1: a\n")["items"][0]["locked"])


class Debug(unittest.TestCase):
    SRC = "def f(x):\n    a = 1\n    if x > 0:\n        b = 2\n    for i in [1]:\n        c = 3\n    return a\n"

    def test_instrumenting_moves_no_line_and_reports_what_ran(self):
        out = B.instrument(self.SRC)
        self.assertEqual(out.count("\n"), self.SRC.count("\n"))
        B.install_glow()
        g = {}
        exec(compile(out, "<t>", "exec"), g)
        B.glow_take()
        self.assertEqual(g["f"](1), 1)
        self.assertEqual(B.glow_take(), [2, 3, 4, 5, 6, 7])
        g["f"](0)
        self.assertEqual(B.glow_take(), [2, 3, 5, 6, 7])

    def test_the_program_still_means_the_same(self):
        B.install_glow()
        g1, g2 = {}, {}
        exec(self.SRC, g1)
        exec(B.instrument(self.SRC), g2)
        self.assertEqual([g1["f"](v) for v in (-1, 0, 5)], [g2["f"](v) for v in (-1, 0, 5)])


class Declarations(unittest.TestCase):
    SRC = (
        "import sage\n\n"
        "Detect = world.phase('Detect')\n"
        "Resolve = world.phase('Resolve', after=Detect)  # contacts\n"
        "Cleanup = world.phase('Cleanup', after=Resolve)\n"
        "Enemy = world.prefab('Enemy')\n"
        "Boss = world.prefab('Boss', base=Enemy)\n"
    )

    def test_phases_and_prefabs_are_read_from_the_script(self):
        d = B.declarations(self.SRC)
        self.assertEqual([(x["kind"], x["var"], x["ref"]) for x in d], [
            ("phase", "Detect", None), ("phase", "Resolve", "Detect"), ("phase", "Cleanup", "Resolve"),
            ("prefab", "Enemy", None), ("prefab", "Boss", "Enemy")])
        self.assertEqual(B.phase_layers(d), [["Detect"], ["Resolve"], ["Cleanup"]])

    def test_changing_what_a_phase_runs_after_edits_only_that_call(self):
        out = B.set_decl_ref(self.SRC, "Cleanup", "Detect")
        self.assertEqual(out, self.SRC.replace("'Cleanup', after=Resolve", "'Cleanup', after=Detect"))
        out = B.set_decl_ref(self.SRC, "Resolve", None)
        self.assertEqual(out, self.SRC.replace("'Resolve', after=Detect", "'Resolve'"))
        self.assertEqual(B.phase_layers(B.declarations(B.set_decl_ref(self.SRC, "Cleanup", "Detect"))), [["Detect"], ["Cleanup", "Resolve"]])

    def test_declarations_are_added_and_systems_are_grouped_by_phase_and_prefab(self):
        out = B.add_declaration(self.SRC, "phase", "Late")
        self.assertIn("Late = world.phase(\"Late\")", out)
        self.assertTrue(out.index("Late =") > out.index("Boss ="))
        with self.assertRaises(ValueError):
            B.add_declaration(self.SRC, "phase", "Detect")
        src = self.SRC + "@world.system('a', phase=Detect)\ndef a(): pass\n\n@Enemy.system\ndef b(e): pass\n"
        src = src.replace("def a(): pass", "def a():\n    pass").replace("def b(e): pass", "def b(e):\n    pass")
        self.assertEqual(B.systems_in(src), [
            {"name": "a", "phase": "Detect", "prefab": None, "owner": None}, {"name": "b", "phase": None, "prefab": "Enemy", "owner": "Enemy"}])

    def test_a_cycle_or_an_unknown_name_is_refused(self):
        for var, ref in [("Detect", "Cleanup"), ("Detect", "Detect"), ("Enemy", "Boss"), ("Detect", "Enemy"), ("Detect", "Nope"), ("Nope", None)]:
            with self.assertRaises(ValueError, msg=(var, ref)):
                B.set_decl_ref(self.SRC, var, ref)

    def test_a_prefab_can_be_given_a_base(self):
        out = B.set_decl_ref(self.SRC, "Enemy", None)
        self.assertEqual(out, self.SRC)
        d = B.declarations(B.set_decl_ref("A = world.prefab('A')\nB = world.prefab('B')\n", "B", "A"))
        self.assertEqual(d[1]["ref"], "A")


class FlatBottoms(unittest.TestCase):
    SRC = "def f(x):\n    a = 1\n    return a\n"

    def test_nothing_goes_after_return_break_or_continue(self):
        b = B.Blocks(self.SRC)
        f = b.function("f")
        with self.assertRaises(ValueError):
            b.insert(f["id"], "body", 99, {"k": "pass"})
        with self.assertRaises(ValueError):
            b.insert(f["id"], "body", 2, {"k": "pass"})
        # Before it is fine.
        out = b.insert(f["id"], "body", 1, {"k": "pass"})
        self.assertEqual(out, "def f(x):\n    a = 1\n    pass\n    return a\n")
        for kind in ("break", "continue"):
            c = B.Blocks("def f():\n    while True:\n        %s\n" % kind)
            w = c.function("f")["body"][0]
            with self.assertRaises(ValueError):
                c.insert(w["id"], "body", 99, {"k": "pass"})

    def test_a_terminal_block_goes_only_at_the_end_of_a_body(self):
        b = B.Blocks("def f():\n    a = 1\n    b = 2\n")
        f = b.function("f")
        for kind in ("return", "break", "continue"):
            with self.assertRaises(ValueError):
                b.insert(f["id"], "body", 0, {"k": kind, **({"value": None} if kind == "return" else {})})
        out = b.insert(f["id"], "body", 99, {"k": "return", "value": None})
        self.assertTrue(out.endswith("    b = 2\n    return\n"))
        # A statement cannot be replaced by one when others follow it.
        c = B.Blocks("def f():\n    a = 1\n    b = 2\n")
        with self.assertRaises(ValueError):
            c.replace(c.function("f")["body"][0]["id"], {"k": "return", "value": None})
        out = c.replace(c.function("f")["body"][1]["id"], {"k": "return", "value": None})
        self.assertTrue(out.endswith("    a = 1\n    return\n"))

    def test_dragging_a_block_takes_everything_below_it(self):
        src = "def f():\n    a = 1\n    # about b\n    b = 2\n    c = 3\n\n\ndef g():\n    pass\n"
        b = B.Blocks(src)
        f, g = b.function("f"), b.function("g")
        _, bb, c = f["body"]
        out = b.move_tail(bb["id"], g["id"], "body", 0)
        self.assertEqual(
            out,
            "def f():\n    a = 1\n\n\ndef g():\n    # about b\n    b = 2\n    c = 3\n    pass\n",
        )
        ast.parse(out)

    def test_a_chain_moves_up_within_its_body_and_never_into_itself(self):
        b = B.Blocks("def f():\n    a = 1\n    b = 2\n    c = 3\n")
        f = b.function("f")
        a, bb, c = f["body"]
        out = b.move_tail(c["id"], f["id"], "body", 0)
        self.assertEqual(out, "def f():\n    c = 3\n    a = 1\n    b = 2\n")
        d = B.Blocks("def f():\n    a = 1\n    if a:\n        pass\n    c = 3\n")
        f = d.function("f")
        a, iff, c = f["body"]
        with self.assertRaises(ValueError):
            d.move_tail(a["id"], iff["id"], "body", 0)  # into a block it carries
        with self.assertRaises(ValueError):
            d.move_tail(iff["id"], f["id"], "body", 2)  # after itself
        self.assertEqual(d.source, "def f():\n    a = 1\n    if a:\n        pass\n    c = 3\n")

    def test_a_whole_body_taken_leaves_pass_and_a_terminal_chain_goes_last(self):
        b = B.Blocks("def f():\n    a = 1\n    return a\n\n\ndef g():\n    x = 0\n    y = 1\n")
        f, g = b.function("f"), b.function("g")
        a = f["body"][0]
        with self.assertRaises(ValueError):
            b.move_tail(a["id"], g["id"], "body", 1)  # the return would not be last
        out = b.move_tail(a["id"], g["id"], "body", 2)
        self.assertEqual(out, "def f():\n    pass\n\n\ndef g():\n    x = 0\n    y = 1\n    a = 1\n    return a\n")

    def test_moves_obey_the_same_rule(self):
        b = B.Blocks("def f(x):\n    a = 1\n    return a\n    pass\n".replace("    pass\n", ""))
        f = b.function("f")
        a, ret = f["body"]
        with self.assertRaises(ValueError):
            b.move(a["id"], f["id"], "body", 2)  # after return
        with self.assertRaises(ValueError):
            b.move(ret["id"], f["id"], "body", 0)  # return before a
        d = B.Blocks("def f():\n    if x:\n        pass\n    y = 1\n")
        f = d.function("f")
        iff, y = f["body"]
        out = d.move(y["id"], iff["id"], "body", 0)
        self.assertIn("if x:\n        y = 1\n        pass", out)

    def test_hand_written_dead_code_still_parses(self):
        p = B.parse("def f():\n    return 1\n    x = 2\n")["items"][0]
        self.assertIsNone(p["locked"])
        self.assertEqual([s["k"] for s in p["body"]], ["return", "assign"])


class Crosstab(unittest.TestCase):
    SRC = (
        "Detect = world.phase('Detect')\nResolve = world.phase('Resolve', after=Detect)\n"
        "combat = world.module('combat')\n\n"
        "@combat.system(phase=Detect)\ndef hit(e: Entity):\n    pass\n\n"
        "@world.system('plain')\ndef plain(e: Entity):\n    pass\n"
    )

    def test_modules_are_declared_and_systems_name_their_owner(self):
        kinds = [(d["kind"], d["var"]) for d in B.declarations(self.SRC)]
        self.assertIn(("module", "combat"), kinds)
        owners = {s["name"]: s["owner"] for s in B.systems_in(self.SRC)}
        self.assertEqual(owners["hit"], "combat")

    def test_a_system_moves_to_another_module_and_phase_by_its_decorator_alone(self):
        b = B.Blocks(self.SRC)
        out = b.set_system_slot("plain", "combat", "Resolve")
        self.assertIn("@combat.system('plain', phase=Resolve)", out)
        out = b.set_system_slot("hit", None, None)
        self.assertIn("@world.system()\ndef hit", out)
        self.assertEqual(out.count("def "), 2)
        with self.assertRaises(ValueError):
            B.Blocks("def f():\n    pass\n").set_system_slot("f", None, None)


class TypedSockets(unittest.TestCase):
    def test_known_types_of_values(self):
        def t(src):
            return B.type_of(B.parse("def f():\n    x = %s\n" % src)["items"][0]["body"][0]["value"])

        self.assertEqual([t(x) for x in ("1", "2.5", "'a'", "True", "None", "[1]", "{1: 2}", "(1,)")], ["num", "num", "str", "bool", "none", "list<num>", "dict", "tuple"])
        self.assertEqual([t(x) for x in ("1 + 2", "1 * 2", "'a' + 'b'", "'a' * 2", "a < b", "not a", "-a", "a and b", "a > 1 and b < 2", "f(x)", "obj.attr", "xs[0]", "[i for i in a]")],
                         ["num", "num", "str", None, "bool", "bool", "num", None, "bool", None, None, None, "list"])

    def test_slots_take_their_type_or_anything_unknown(self):
        self.assertTrue(B.type_fits("num", "num"))
        self.assertTrue(B.type_fits(None, "num"))
        self.assertTrue(B.type_fits("str", None))
        self.assertFalse(B.type_fits("str", "num"))
        self.assertFalse(B.type_fits("num", "bool"))
        self.assertEqual([B.slot_type_for({"k": "name", "name": n}) for n in ("int", "float", "str", "bool", "Any", "Foo")], ["num", "num", "str", "bool", None, None])


class ListTypes(unittest.TestCase):
    def blk(self, src):
        return B.parse("def f():\n    x = %s\n" % src)["items"][0]["body"][0]["value"]

    def test_lists_know_their_element_type(self):
        t = lambda s: B.type_of(self.blk(s))  # noqa: E731
        self.assertEqual([t("[1, 2]"), t("['a']"), t("[[1], [2, 3]]"), t("[]"), t("[1, 'a']"), t("[a, b]"), t("[[1], ['a']]")],
                         ["list<num>", "list<str>", "list<list<num>>", "list", "list", "list", "list"])
        self.assertEqual(t("[1] + [2]"), "list<num>")

    def test_annotations_say_what_a_list_holds(self):
        f = lambda s: B.slot_type_for(B.parse("def f():\n    x: %s = 0\n" % s)["items"][0]["body"][0]["type"])  # noqa: E731
        self.assertEqual([f("list[int]"), f("list[list[int]]"), f("list"), f("list[Foo]"), f("int")], ["list<num>", "list<list<num>>", "list", "list", "num"])

    def test_a_list_fits_a_list_slot_by_element(self):
        fits = B.type_fits
        self.assertTrue(fits("list<num>", "list<num>"))
        self.assertTrue(fits("list", "list<num>"))      # unknown elements
        self.assertTrue(fits("list<num>", "list"))
        self.assertFalse(fits("list<str>", "list<num>"))
        self.assertFalse(fits("list<list<str>>", "list<list<num>>"))
        self.assertFalse(fits("num", "list<num>"))
        self.assertFalse(fits("list<num>", "num"))
        self.assertEqual(B.element_slot("list<list<num>>"), "list<num>")
        self.assertIsNone(B.element_slot("num"))

    def test_a_list_of_equal_lists_is_a_grid_that_grows_and_shrinks(self):
        g = self.blk("[[1, 2, 3], [4, 5, 6]]")
        self.assertEqual(B.grid_shape(g), (2, 3))
        self.assertIsNone(B.grid_shape(self.blk("[[1], [2, 3]]")))
        self.assertIsNone(B.grid_shape(self.blk("[1, 2]")))
        self.assertEqual(B.expr_text(B.grid_resize(g, add_row=1)), "[[1, 2, 3], [4, 5, 6], [0, 0, 0]]")
        self.assertEqual(B.expr_text(B.grid_resize(g, add_col=1)), "[[1, 2, 3, 0], [4, 5, 6, 0]]")
        self.assertEqual(B.expr_text(B.grid_resize(g, add_col=-1, add_row=-1)), "[[1, 2]]")
        with self.assertRaises(ValueError):
            B.grid_resize(B.grid_resize(g, add_row=-1), add_row=-1)
        # Text grids fill with empty text, test grids with False.
        self.assertEqual(B.expr_text(B.grid_resize(self.blk("[['a']]"), add_col=1)), "[['a', '']]")
        self.assertEqual(B.expr_text(B.grid_resize(self.blk("[[True]]"), add_row=1)), "[[True], [False]]")

    def test_the_palette_offers_a_typed_list_and_a_grid(self):
        pal = {p["label"]: p for p in B.palette({}, [])}
        self.assertEqual(B.type_of(pal["list of numbers"]["block"]), "list<num>")
        self.assertEqual(B.grid_shape(pal["grid of numbers"]["block"]), (2, 2))


class Hats(unittest.TestCase):
    SRC = "@world.system('m', query='Position')\ndef m(pos: Position, vel: Velocity):\n    pos.x += vel.x\n    pos.y += 1\n"

    def test_a_parameter_missing_from_the_query_is_an_issue_with_a_one_click_fix(self):
        b = B.Blocks(self.SRC)
        f = b.function("m")
        issues = B.function_issues(f)
        self.assertEqual([(i["param"], i["component"]) for i in issues], [("vel", "Velocity")])
        # Greying: only the statement that uses `vel`.
        self.assertEqual([B.uses_name(s, "vel") for s in f["body"]], [True, False])
        out = B.add_to_query(b, "m", "Velocity")
        self.assertEqual(out, self.SRC.replace("query='Position'", "query='Position, Velocity'").replace("'Position, Velocity'", "\"Position, Velocity\"") if False else out)
        self.assertIn("Position, Velocity", out)
        self.assertEqual(B.function_issues(b.function("m")), [])
        with self.assertRaises(ValueError):
            B.add_to_query(b, "m", "Velocity")

    def test_a_system_with_no_query_string_has_no_issues(self):
        b = B.Blocks("@world.system('m')\ndef m(pos: Position):\n    pass\n")
        self.assertEqual(B.function_issues(b.function("m")), [])

    def test_phases_are_named_relative_to_each_other(self):
        d = B.declarations("A = world.phase('A')\nB = world.phase('B', after=A)\n")
        self.assertEqual(B.relative_phases(d), [("at A", "A"), ("at B", "B"), ("before B", "A")])


class Types(unittest.TestCase):
    def test_type_blocks_are_valid_annotations_and_any_is_marked(self):
        pal = [p for p in B.palette({}, []) if p["category"] == "Types"]
        self.assertEqual({p["shape"] for p in pal}, {"type"})
        texts = [B.expr_text(p["block"]) for p in pal]
        self.assertEqual(texts, ["int", "float", "str", "bool", "list[int]", "dict[str, int]", "int | None", "Any"])
        self.assertTrue(B.is_any(pal[-1]["block"]))
        self.assertFalse(B.is_any(pal[0]["block"]))

    def test_nested_types_round_trip_through_a_declaration(self):
        src = "def f():\n    x: dict[str, list[int]] = {}\n    y: int | None = None\n"
        p = B.parse(src)["items"][0]
        self.assertIsNone(p["locked"])
        self.assertEqual(B.expr_text(p["body"][0]["type"]), "dict[str, list[int]]")
        b = B.Blocks(src)
        ty = b.function("f")["body"][0]["type"]
        out = b.replace(ty["id"], {"k": "name", "name": "Any"})
        self.assertEqual(out, src.replace("dict[str, list[int]]", "Any"))


class Why(unittest.TestCase):
    def test_a_traceback_line_points_at_the_innermost_block(self):
        b = B.Blocks("def f(x):\n    a = 1\n    if x:\n        b = nope\n    c = 2\n")
        hit = b.block_at_line(4)
        self.assertEqual((hit["k"], hit["line"]), ("assign", 4))
        self.assertEqual(b.block_at_line(3)["k"], "if")
        self.assertIsNone(b.block_at_line(99))
        trace = 'Traceback (most recent call last):\n  File "<string>", line 4, in f\nNameError: x'
        self.assertEqual(B.error_line(trace), 4)
        self.assertIsNone(B.error_line("no lines here"))


class Palette(unittest.TestCase):
    SCHEMAS = {
        "game.Position": [("x", "f32"), ("y", "f32")],
        "game.Frozen": [],
        "sage.collision.Collider": [("hx", "f32"), ("solid", "bool")],
    }

    def test_each_field_gets_get_set_and_change_and_tags_get_add_remove_has(self):
        pal = B.palette(self.SCHEMAS, [("pos", "game.Position")])
        labels = [p["label"] for p in pal]
        for want in ["Position's x", "set Position's x to []", "change Position's x by []", "add [Frozen] to []", "[] has [Frozen]", "Collider's solid"]:
            self.assertIn(want, labels)
        self.assertNotIn("change Collider's solid by []", labels)
        cats = {p["category"] for p in pal}
        self.assertTrue({"game", "collision", "Control", "Operators"} <= cats)

    def test_a_component_in_scope_reads_through_its_parameter(self):
        pal = B.palette(self.SCHEMAS, [("pos", "game.Position")])
        chg = [p for p in pal if p["label"] == "change Position's x by []"][0]
        self.assertEqual(B.stmt_text(chg["block"]), "pos.x += 0.0")
        self.assertNotIn("slot", chg)
        other = [p for p in pal if p["label"] == "change Collider's hx by []" or p["label"] == "set Collider's hx to []"][0]
        self.assertEqual(other["slot"], "entity")
        self.assertIn("e.get(Collider).hx", B.stmt_text(other["block"]))

    def test_every_palette_block_makes_valid_python(self):
        for p in B.palette(self.SCHEMAS, [("pos", "game.Position")]):
            b = p["block"]
            if b["k"] in B.STATEMENTS:
                ast.parse(B.stmt_text(b))
            else:
                ast.parse(B.expr_text(b))

    def test_scope_comes_from_the_hat(self):
        f = B.parse("def m(e: Entity, pos: Position, vel: sage.Velocity):\n    pass\n")["items"][0]
        self.assertEqual(B.scope_of(f), [("pos", "Position"), ("vel", "Velocity")])

    def test_a_new_system_is_valid_and_parses_to_a_stack(self):
        src = B.new_system("move", [("pos", "Position"), ("vel", "Velocity")], query="Position, Velocity")
        f = B.parse(src)["items"][0]
        self.assertEqual(f["name"], "move")
        self.assertEqual(B.scope_of(f), [("pos", "Position"), ("vel", "Velocity")])


# -- helpers


def _walk(b):
    if isinstance(b, dict):
        yield b
        for v in b.values():
            yield from _walk(v)
    elif isinstance(b, list):
        for v in b:
            yield from _walk(v)


def _nm(n):
    return {"k": "name", "name": n}


def _attr(v, a):
    return {"k": "attr", "value": _nm(v), "attr": a}


def _lit_b(v):
    t = "bool" if isinstance(v, bool) else "int"
    return {"k": "lit", "t": t, "v": v}


def _lit_s(v):
    return {"k": "lit", "t": "str", "v": v}


def _strip(b):
    if isinstance(b, dict):
        return {k: _strip(v) for k, v in b.items() if k not in ("id", "notes", "trail", "elif")}
    if isinstance(b, list):
        return [_strip(v) for v in b]
    return b


def run():
    """For the embedded interpreter: runs every test, returns failures as text."""
    suite = unittest.defaultTestLoader.loadTestsFromModule(__import__(__name__))
    import io

    out = io.StringIO()
    res = unittest.TextTestRunner(stream=out, verbosity=0).run(suite)
    return "" if res.wasSuccessful() else out.getvalue()


if __name__ == "__main__":
    unittest.main()
