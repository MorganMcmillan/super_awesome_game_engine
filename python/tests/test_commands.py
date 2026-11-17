"""Tests for sage.commands. Run by `cargo test -p sage_editor --test blocks`
(inside the embedded interpreter) and by `python -m unittest` with
`python/sage` on the path."""

import unittest

import commands as C


def registry(log=None):
    log = log if log is not None else []
    r = C.Registry()
    r.add("play", "Play", lambda: log.append("play"), shortcut="F5", where="Top bar", aliases=("run", "start", "test"))
    r.add("stop", "Stop", lambda: log.append("stop"), where="Top bar", aliases=("end", "halt"))
    r.add("delete_block", "Delete Block", lambda: log.append("del"), shortcut="Delete", aliases=("remove", "erase", "kill"))
    r.add("delete_stack", "Delete Stack", lambda: log.append("dels"), aliases=("remove",))
    r.add("add_note", "Add Note", lambda: log.append("note"), shortcut="Alt+N", aliases=("comment", "sticky"))
    r.add("table", "Show Entity Table", lambda: log.append("table"), aliases=("spreadsheet", "grid"))
    return r


def titles(r, text):
    return [c.title for c, _ in r.search(text)]


class Search(unittest.TestCase):
    def test_an_empty_search_lists_everything_in_the_order_added(self):
        r = registry()
        self.assertEqual(titles(r, ""), ["Play", "Stop", "Delete Block", "Delete Stack", "Add Note", "Show Entity Table"])
        self.assertEqual(titles(r, "   "), titles(r, ""))
        self.assertEqual(len(r.search("", limit=2)), 2)

    def test_the_start_of_any_word_finds_a_command_in_any_case(self):
        r = registry()
        self.assertEqual(titles(r, "pl"), ["Play"])
        self.assertEqual(titles(r, "NOTE"), ["Add Note"])
        self.assertEqual(titles(r, "stack")[0], "Delete Stack")

    def test_the_middle_of_a_word_finds_it_below_the_starts(self):
        r = registry()
        # "lock" is inside "Block"; "ta" starts Table and Stack has it inside.
        self.assertEqual(titles(r, "lock"), ["Delete Block"])
        self.assertEqual(titles(r, "ta")[0], "Show Entity Table")
        self.assertIn("Delete Stack", titles(r, "ta"))

    def test_aliases_find_a_command_and_say_which_word_did(self):
        r = registry()
        found = r.search("erase")
        self.assertEqual([(c.title, a) for c, a in found], [("Delete Block", "erase")])
        self.assertEqual(titles(r, "run"), ["Play"])
        self.assertEqual(titles(r, "spread"), ["Show Entity Table"])
        # Both deletes answer to "remove", in the order they were added.
        self.assertEqual(titles(r, "remove"), ["Delete Block", "Delete Stack"])

    def test_a_title_match_ranks_above_an_alias_match(self):
        r = C.Registry()
        r.add("a", "Open Project", lambda: None, aliases=("stop",))
        r.add("b", "Stop", lambda: None)
        self.assertEqual(titles(r, "stop"), ["Stop", "Open Project"])

    def test_several_words_each_start_a_word(self):
        r = registry()
        self.assertEqual(titles(r, "del st"), ["Delete Stack"])
        self.assertEqual(titles(r, "del bl"), ["Delete Block"])

    def test_one_slip_of_the_finger_is_forgiven(self):
        r = registry()
        self.assertEqual(titles(r, "tabel"), ["Show Entity Table"])  # swapped
        self.assertEqual(titles(r, "delte")[0], "Delete Block")  # a letter missing
        self.assertEqual(titles(r, "deleete")[0], "Delete Block")  # a letter extra
        self.assertEqual(titles(r, "plsy"), ["Play"])  # a wrong letter
        # Short words are not guessed at: too many would match.
        self.assertEqual(titles(r, "pxy"), [])

    def test_letters_in_order_are_the_last_resort(self):
        r = registry()
        self.assertEqual(titles(r, "dlblk"), ["Delete Block"])
        self.assertEqual(titles(r, "zzz"), [])

    def test_scores_are_ordered_best_first(self):
        s = C.score
        self.assertTrue(s("play", "Play") > s("pl", "Play") > s("la", "Play") > 0)
        self.assertTrue(s("note", "Add Note") > s("ote", "Add Note"))
        self.assertEqual(s("xyz", "Play"), 0)


class Running(unittest.TestCase):
    def test_a_command_runs_by_its_id(self):
        log = []
        r = registry(log)
        self.assertIsNone(r.run("play"))
        self.assertEqual(log, ["play"])
        self.assertEqual(r.get("play").shortcut, "F5")
        self.assertEqual(len(r), 6)

    def test_a_blocked_command_says_why_and_does_nothing(self):
        log, playing = [], [False]
        r = C.Registry()
        r.add("stop", "Stop", lambda: log.append("stop"), when=lambda: None if playing[0] else "The game is not running")
        self.assertEqual(r.run("stop"), "The game is not running")
        self.assertEqual(log, [])
        playing[0] = True
        self.assertIsNone(r.run("stop"))
        self.assertEqual(log, ["stop"])

    def test_an_id_is_used_once(self):
        r = registry()
        with self.assertRaises(ValueError):
            r.add("play", "Play Again", lambda: None)


def run():
    """For the embedded interpreter: runs every test, returns failures as text."""
    suite = unittest.defaultTestLoader.loadTestsFromModule(__import__(__name__))
    import io

    out = io.StringIO()
    res = unittest.TextTestRunner(stream=out, verbosity=0).run(suite)
    return "" if res.wasSuccessful() else out.getvalue()


if __name__ == "__main__":
    unittest.main()
