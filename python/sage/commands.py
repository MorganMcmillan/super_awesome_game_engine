"""Named commands and a forgiving search over them.

Every action of a tool is one command: a title, what it does, and the doors
that lead to it (a key chord, a place on screen). A button, a shortcut and a
command palette then all run the same thing, and the palette can tell a
person the faster door for what they just did the slow way.

The search is forgiving the way people need it to be: any word of the title
may be typed, from its start or its middle; a command answers to its aliases
(two people pick the same word for a thing less than one time in five, so
"remove", "erase" and "kill" all find Delete); letters may be skipped; and
one slip of the finger is forgiven. Results come back best first, and an
empty search lists the commands in the order they were added, which never
changes under the person's hands.

Pure Python: no world, no UI.
"""


class Command:
    """One action. `run()` does it. `when()` returns a reason (text) while the
    command cannot run, or None when it can."""

    def __init__(self, id, title, run, shortcut=None, where="", aliases=(), when=None):
        self.id, self.title, self._run = id, title, run
        self.shortcut, self.where, self.aliases, self.when = shortcut, where, tuple(aliases), when

    def blocked(self):
        """Why the command cannot run now, or None."""
        return self.when() if self.when is not None else None


class Registry:
    def __init__(self):
        self._commands = {}

    def add(self, id, title, run, shortcut=None, where="", aliases=(), when=None):
        """Adds a command. `where` says where else it lives on screen ("Top
        bar", "Blocks: right-click a block"), for the palette to show."""
        if id in self._commands:
            raise ValueError("there is already a command %r" % id)
        c = Command(id, title, run, shortcut, where, aliases, when)
        self._commands[id] = c
        return c

    def __iter__(self):
        return iter(self._commands.values())

    def __len__(self):
        return len(self._commands)

    def get(self, id):
        return self._commands[id]

    def run(self, id):
        """Runs a command. Returns the reason it did not run, or None."""
        c = self._commands[id]
        why = c.blocked()
        if why is None:
            c._run()
        return why

    def search(self, text, limit=None):
        """Commands matching `text`, best first: `[(command, matched)]`, where
        `matched` is the alias that found it (None when its title did)."""
        query = " ".join(text.lower().split())
        if not query:
            found = [(c, None) for c in self]
        else:
            ranked = []
            for order, c in enumerate(self):
                best = _best(query, c)
                if best is not None:
                    ranked.append((-best[0], order, c, best[1]))
            ranked.sort(key=lambda r: r[:2])
            found = [(c, alias) for _, _, c, alias in ranked]
        return found if limit is None else found[:limit]


def _best(query, command):
    """The best `(score, alias)` of a command for a query, or None."""
    best = None
    title = score(query, command.title)
    if title:
        best = (title, None)
    for alias in command.aliases:
        s = score(query, alias)
        # An alias is a way in, not the name: it ranks just under an equal
        # match on the title.
        if s and (best is None or s - 5 > best[0]):
            best = (s - 5, alias)
    return best


def score(query, text):
    """How well `query` finds `text`, 0 for not at all. Both are compared in
    lower case. Higher is better: the whole text, the start of a word, the
    middle of a word, a slip of the finger, then letters in order."""
    text = text.lower()
    if query == text:
        return 100
    words = text.split()
    if text.startswith(query):
        return 90
    if any(w.startswith(query) for w in words):
        return 80
    # Every typed word starts a word of the text ("del st" finds Delete Stack).
    parts = query.split()
    if len(parts) > 1 and all(any(w.startswith(p) for w in words) for p in parts):
        return 75
    if query in text:
        return 60
    if len(query) >= 4 and any(_slip(query, w[: len(query) + 1]) or _slip(query, w[: len(query)]) for w in words):
        return 45
    if len(query) >= 2 and _in_order(query.replace(" ", ""), text):
        return 30
    return 0


def _in_order(letters, text):
    """Whether `letters` appear in `text` in that order (with gaps)."""
    at = 0
    for ch in letters:
        at = text.find(ch, at) + 1
        if at == 0:
            return False
    return True


def _slip(a, b):
    """Whether `a` is `b` with one slip: a letter wrong, missing or extra, or
    two neighbours swapped ("tabel" for "table")."""
    if a == b:
        return False
    if abs(len(a) - len(b)) > 1:
        return False
    i = 0
    while i < min(len(a), len(b)) and a[i] == b[i]:
        i += 1
    rest_a, rest_b = a[i:], b[i:]
    if len(a) == len(b):
        swapped = len(rest_a) >= 2 and rest_a[0] == rest_b[1] and rest_a[1] == rest_b[0] and rest_a[2:] == rest_b[2:]
        return swapped or rest_a[1:] == rest_b[1:]
    longer, shorter = (rest_a, rest_b) if len(a) > len(b) else (rest_b, rest_a)
    return longer[1:] == shorter
