"""The block editor's model: Python source <-> blocks, losslessly.

Design doc 4.1: text is the source of truth and blocks are a view of it, and
`libcst` does the round trip.

How the round trip stays lossless
---------------------------------
`parse(source)` reads the source into a concrete syntax tree with `libcst`
(which keeps every comment, blank line and bit of spacing) and builds a tree
of plain dicts, the blocks. An edit rewrites the tree at the blocks it
touches and prints it again, so everything else comes back byte for byte.
New or changed blocks become text through `ast.unparse` (canonical Python)
and are parsed into the tree.

Comments become notes (sticky notes in the editor): the comment lines
directly above a statement or function are its `notes`, and a function's
docstring is its `doc`. Moving or deleting the block takes its notes with
it. A comment on the same line after a statement is its `trail`.

A function that uses Python the blocks cannot show (design doc 4.1, the
"language level") is *locked*: it appears as read-only text and is never
touched.

`libcst` must be importable; the host puts `python/vendor` on `sys.path`
(docs/dependencies.md).
"""

import ast

import libcst as cst

LANGUAGE_LEVEL = 1

# Block kinds, by shape (design doc 4.2).
STATEMENTS = (
    "assign", "change", "declare", "discard", "if", "while", "for", "return", "pass", "break", "continue",
)


# A statement nothing can follow: its block has a flat bottom (no notch), and
# no edit puts a block after it (or puts one of these anywhere but the end).
TERMINAL = ("return", "break", "continue")


def check_sequence(stmts, index, new_block):
    """Refuses an edit that would put a block after a terminal one, or a
    terminal block before another. `stmts` are the blocks the body will hold
    besides the new one; `index` is where it goes."""
    index = max(0, min(index, len(stmts)))
    if index > 0 and stmts[index - 1]["k"] in TERMINAL:
        raise ValueError("NOTHING CAN GO AFTER %s" % stmts[index - 1]["k"].upper())
    if new_block["k"] in TERMINAL and index < len(stmts):
        raise ValueError("%s MUST BE THE LAST BLOCK OF ITS BODY" % new_block["k"].upper())


class Unsupported(Exception):
    """Python the blocks cannot show. The message says what and where."""


_BINOPS = {
    ast.Add: "+", ast.Sub: "-", ast.Mult: "*", ast.Div: "/", ast.FloorDiv: "//", ast.Mod: "%",
    ast.Pow: "**", ast.BitAnd: "&", ast.BitOr: "|", ast.BitXor: "^", ast.LShift: "<<", ast.RShift: ">>",
}
_UNARY = {ast.USub: "-", ast.UAdd: "+", ast.Not: "not", ast.Invert: "~"}
_CMP = {
    ast.Eq: "==", ast.NotEq: "!=", ast.Lt: "<", ast.LtE: "<=", ast.Gt: ">", ast.GtE: ">=",
    ast.Is: "is", ast.IsNot: "is not", ast.In: "in", ast.NotIn: "not in",
}
_BOOL = {ast.And: "and", ast.Or: "or"}
_REV = lambda d: {v: k for k, v in d.items()}  # noqa: E731
_BINOP_CLS, _UNARY_CLS, _CMP_CLS, _BOOL_CLS = _REV(_BINOPS), _REV(_UNARY), _REV(_CMP), _REV(_BOOL)


# ---------------------------------------------------------------- parse

_BINOP_NAMES = {
    "Add": "+", "Subtract": "-", "Multiply": "*", "Divide": "/", "FloorDivide": "//", "Modulo": "%",
    "Power": "**", "BitAnd": "&", "BitOr": "|", "BitXor": "^", "LeftShift": "<<", "RightShift": ">>",
}
_AUG_NAMES = {k + "Assign": v for k, v in _BINOP_NAMES.items()}
_UNARY_NAMES = {"Minus": "-", "Plus": "+", "Not": "not", "BitInvert": "~"}
_CMP_NAMES = {
    "Equal": "==", "NotEqual": "!=", "LessThan": "<", "LessThanEqual": "<=", "GreaterThan": ">",
    "GreaterThanEqual": ">=", "Is": "is", "IsNot": "is not", "In": "in", "NotIn": "not in",
}


def _lit(t, v):
    return {"k": "lit", "t": t, "v": v}


class _Parser:
    """Builds blocks from a libcst module and remembers, for every block, the
    tree node it came from (`nodes`, indexed by block id)."""

    def __init__(self, module):
        self.module = module
        self.nodes = [None]  # id 0 is unused: ids start at 1
        wrapper = cst.metadata.MetadataWrapper(module, unsafe_skip_copy=True)
        self.pos = wrapper.resolve(cst.metadata.PositionProvider)

    def _new(self, b, node):
        self.nodes.append(node)
        b["id"] = len(self.nodes) - 1
        if b["k"] in STATEMENTS or b["k"] == "function":
            r = self.pos.get(node)
            if r is not None:
                b["line"], b["end"] = r.start.line, r.end.line
        return b

    # -- expressions

    def expr(self, n):
        return self._new(self._expr(n), n)

    def _expr(self, n):
        T = type(n)
        if T is cst.Integer or T is cst.Float:
            v = ast.literal_eval(n.value)
            return _lit("int" if T is cst.Integer else "float", v)
        if T is cst.SimpleString:
            v = ast.literal_eval(n.value)
            if not isinstance(v, str):
                raise Unsupported("a bytes literal")
            return _lit("str", v)
        if T is cst.Name:
            if n.value in ("True", "False"):
                return _lit("bool", n.value == "True")
            if n.value == "None":
                return _lit("none", None)
            return {"k": "name", "name": n.value}
        if T is cst.Attribute:
            return {"k": "attr", "value": self.expr(n.value), "attr": n.attr.value}
        if T is cst.Call:
            args, kwargs = [], []
            for a in n.args:
                if a.star:
                    raise Unsupported("a call with * or ** arguments")
                if a.keyword is None:
                    if kwargs:
                        raise Unsupported("a positional argument after a keyword")
                    args.append(self.expr(a.value))
                else:
                    kwargs.append([a.keyword.value, self.expr(a.value)])
            return {"k": "call", "func": self.expr(n.func), "args": args, "kwargs": kwargs}
        if T is cst.BinaryOperation:
            op = _BINOP_NAMES.get(type(n.operator).__name__)
            if op is None:
                raise Unsupported("the operator %s" % type(n.operator).__name__)
            return {"k": "binop", "op": op, "l": self.expr(n.left), "r": self.expr(n.right)}
        if T is cst.UnaryOperation:
            return {"k": "unary", "op": _UNARY_NAMES[type(n.operator).__name__], "v": self.expr(n.expression)}
        if T is cst.BooleanOperation:
            op = "and" if isinstance(n.operator, cst.And) else "or"
            values = []

            def flat(x):
                if isinstance(x, cst.BooleanOperation) and not x.lpar and type(x.operator) is type(n.operator):
                    flat(x.left)
                    flat(x.right)
                else:
                    values.append(self.expr(x))

            flat(n.left)
            flat(n.right)
            return {"k": "boolop", "op": op, "values": values}
        if T is cst.Comparison:
            return {
                "k": "compare",
                "l": self.expr(n.left),
                "ops": [_CMP_NAMES[type(c.operator).__name__] for c in n.comparisons],
                "rs": [self.expr(c.comparator) for c in n.comparisons],
            }
        if T is cst.IfExp:
            return {"k": "ifexp", "test": self.expr(n.test), "then": self.expr(n.body), "else": self.expr(n.orelse)}
        if T is cst.Subscript:
            if not n.slice or any(not isinstance(x.slice, cst.Index) or x.slice.star for x in n.slice):
                raise Unsupported("a slice")
            if len(n.slice) == 1:
                return {"k": "index", "value": self.expr(n.value), "index": self.expr(n.slice[0].slice.value)}
            # `dict[str, int]`: several indices make a tuple.
            tup = {"k": "tuple", "items": [self.expr(x.slice.value) for x in n.slice]}
            return {"k": "index", "value": self.expr(n.value), "index": self._new(tup, n.slice[0])}
        if T in (cst.List, cst.Tuple, cst.Set):
            if any(not isinstance(e, cst.Element) for e in n.elements):
                raise Unsupported("a starred element")
            kind = {cst.List: "list", cst.Tuple: "tuple", cst.Set: "set"}[T]
            return {"k": kind, "items": [self.expr(e.value) for e in n.elements]}
        if T is cst.Dict:
            if any(not isinstance(e, cst.DictElement) for e in n.elements):
                raise Unsupported("a dict with ** in it")
            return {"k": "dict", "items": [[self.expr(e.key), self.expr(e.value)] for e in n.elements]}
        if T is cst.Lambda:
            p = n.params
            if p.kwonly_params or p.posonly_params or p.star_kwarg or not isinstance(p.star_arg, cst.MaybeSentinel) or any(x.default is not None or x.star for x in p.params):
                raise Unsupported("a lambda with defaults, * or **")
            return {"k": "lambda", "params": [x.name.value for x in p.params], "body": self.expr(n.body)}
        if T in (cst.ListComp, cst.SetComp, cst.GeneratorExp, cst.DictComp):
            clauses, f = [], n.for_in
            while f is not None:
                if f.asynchronous is not None:
                    raise Unsupported("an async comprehension")
                clauses.append({"target": self.target_any(f.target), "iter": self.expr(f.iter), "ifs": [self.expr(c.test) for c in f.ifs]})
                f = f.inner_for_in
            kind = {cst.ListComp: "list", cst.SetComp: "set", cst.GeneratorExp: "gen", cst.DictComp: "dict"}[T]
            b = {"k": "comp", "kind": kind, "clauses": clauses}
            if T is cst.DictComp:
                b["key"], b["value"] = self.expr(n.key), self.expr(n.value)
            else:
                b["elt"] = self.expr(n.elt)
            return b
        raise Unsupported(_describe(n))

    def target_any(self, n):
        if isinstance(n, cst.Name):
            return self.expr(n)
        if isinstance(n, cst.Tuple) and all(isinstance(e, cst.Element) and isinstance(e.value, cst.Name) for e in n.elements):
            return self.expr(n)
        raise Unsupported("this loop target")

    # -- statements

    def target(self, n):
        if isinstance(n, (cst.Name, cst.Attribute)):
            return self.expr(n)
        if isinstance(n, cst.Subscript):
            return self.expr(n)
        raise Unsupported("an assignment to %s" % _describe(n))

    def stmts(self, block, what="body"):
        """The statement blocks of a compound statement's body (an
        `IndentedBlock`), docstring excluded."""
        if not isinstance(block, cst.IndentedBlock):
            raise Unsupported("a statement on the same line as its header")
        return [self.stmt(s) for s in block.body]

    def stmt(self, n):
        if isinstance(n, cst.SimpleStatementLine):
            if len(n.body) != 1:
                raise Unsupported("two statements on one line")
            b = self.small(n.body[0])
        elif isinstance(n, cst.If):
            b = {"k": "if", "test": self.expr(n.test), "body": self.stmts(n.body), "orelse": []}
            if isinstance(n.orelse, cst.If):
                e = self.stmt(n.orelse)
                e["elif"] = True
                b["orelse"] = [e]
            elif isinstance(n.orelse, cst.Else):
                b["orelse"] = self.stmts(n.orelse.body)
        elif isinstance(n, cst.While):
            if n.orelse is not None:
                raise Unsupported("while with else")
            b = {"k": "while", "test": self.expr(n.test), "body": self.stmts(n.body)}
        elif isinstance(n, cst.For):
            if n.orelse is not None:
                raise Unsupported("for with else")
            if n.asynchronous is not None:
                raise Unsupported("an async for")
            b = {"k": "for", "target": self.target(n.target), "iter": self.expr(n.iter), "body": self.stmts(n.body)}
        else:
            raise Unsupported(_describe(n))
        b["notes"] = _notes(n)
        b["trail"] = _trail(n)
        return self._new(b, n)

    def small(self, n):
        T = type(n)
        if T is cst.Assign:
            if len(n.targets) != 1:
                raise Unsupported("a chained assignment")
            return {"k": "assign", "target": self.target(n.targets[0].target), "value": self.expr(n.value)}
        if T is cst.AugAssign:
            op = _AUG_NAMES.get(type(n.operator).__name__)
            if op is None:
                raise Unsupported("that operator")
            return {"k": "change", "target": self.target(n.target), "op": op, "value": self.expr(n.value)}
        if T is cst.AnnAssign:
            if not isinstance(n.target, cst.Name):
                raise Unsupported("an annotated attribute")
            return {
                "k": "declare",
                "name": n.target.value,
                "type": self.expr(n.annotation.annotation),
                "value": None if n.value is None else self.expr(n.value),
            }
        if T is cst.Expr:
            return {"k": "discard", "value": self.expr(n.value)}
        if T is cst.Return:
            return {"k": "return", "value": None if n.value is None else self.expr(n.value)}
        if T is cst.Pass:
            return {"k": "pass"}
        if T is cst.Break:
            return {"k": "break"}
        if T is cst.Continue:
            return {"k": "continue"}
        raise Unsupported(_describe(n))

    # -- functions

    def function(self, n):
        f = {
            "k": "function",
            "name": n.name.value,
            "decorators": [],
            "params": [],
            "body": [],
            "locked": None,
            "position": None,
            "doc": None,
            "notes": _notes(n),
            "trail": None,
        }
        try:
            p = n.params
            if p.star_arg is not cst.MaybeSentinel.DEFAULT and not isinstance(p.star_arg, cst.MaybeSentinel) or p.kwonly_params or p.star_kwarg or p.posonly_params:
                raise Unsupported("parameters with defaults, * or **")
            if any(x.default is not None or x.star for x in p.params):
                raise Unsupported("parameters with defaults, * or **")
            if n.returns is not None:
                raise Unsupported("a return annotation")
            if n.asynchronous is not None:
                raise Unsupported("an async function")
            f["params"] = [
                {"name": x.name.value, "type": None if x.annotation is None else self.expr(x.annotation.annotation)}
                for x in p.params
            ]
            for d in n.decorators:
                pos = _position(d.decorator)
                if pos is not None:
                    f["position"] = pos
                else:
                    f["decorators"].append(self.expr(d.decorator))
            body = n.body
            if not isinstance(body, cst.IndentedBlock):
                raise Unsupported("a statement on the same line as its header")
            rest = list(body.body)
            doc = _docstring(rest[0]) if rest else None
            if doc is not None:
                f["doc"] = doc.rstrip("\n")
                rest = rest[1:]
            f["has_doc"] = doc is not None
            f["body"] = [self.stmt(s) for s in rest]
        except Unsupported as e:
            f["locked"] = str(e)
            f["decorators"], f["params"], f["body"], f["position"] = [], [], [], None
            f["has_doc"] = False
        return self._new(f, n)


def _comment_lines(lines):
    out = []
    for l in lines:
        if l.comment is not None:
            out.append(l.comment.value[1:].strip() if l.comment.value.startswith("#") else l.comment.value)
    return out


def _notes(n):
    lines = getattr(n, "leading_lines", ())
    return _comment_lines(lines)


def _trail(n):
    tw = None
    if isinstance(n, cst.SimpleStatementLine):
        tw = n.trailing_whitespace
    elif hasattr(n, "body") and isinstance(n.body, cst.IndentedBlock):
        tw = n.body.header
    if tw is not None and tw.comment is not None:
        return tw.comment.value[1:].strip()
    return None


def _docstring(stmt):
    """The text of a docstring statement, or None."""
    if (
        isinstance(stmt, cst.SimpleStatementLine)
        and len(stmt.body) == 1
        and isinstance(stmt.body[0], cst.Expr)
        and isinstance(stmt.body[0].value, cst.SimpleString)
    ):
        try:
            v = ast.literal_eval(stmt.body[0].value.value)
        except (ValueError, SyntaxError):
            return None
        return v if isinstance(v, str) else None
    return None


def _describe(n):
    return {
        "FormattedString": "an f-string",
        "ConcatenatedString": "joined strings",
        "Await": "await",
        "Yield": "yield",
        "NamedExpr": "the walrus operator",
        "Try": "try",
        "TryStar": "try",
        "With": "with",
        "Raise": "raise",
        "Assert": "assert",
        "Del": "del",
        "Import": "an import inside a function",
        "ImportFrom": "an import inside a function",
        "Global": "global",
        "Nonlocal": "nonlocal",
        "FunctionDef": "a nested function",
        "ClassDef": "a class inside a function",
        "Match": "match",
        "Imaginary": "a complex number",
        "Ellipsis": "an ellipsis",
    }.get(type(n).__name__, type(n).__name__)


def _position(d):
    """`@sage.position(x, y)` -> [x, y]."""
    if (
        isinstance(d, cst.Call)
        and isinstance(d.func, cst.Attribute)
        and d.func.attr.value == "position"
        and isinstance(d.func.value, cst.Name)
        and d.func.value.value == "sage"
        and len(d.args) == 2
        and all(not a.star and a.keyword is None for a in d.args)
    ):
        out = []
        for a in d.args:
            v = a.value
            neg = False
            if isinstance(v, cst.UnaryOperation) and isinstance(v.operator, cst.Minus):
                neg, v = True, v.expression
            if not isinstance(v, (cst.Integer, cst.Float)):
                return None
            x = ast.literal_eval(v.value)
            out.append(-x if neg else x)
        return out
    return None


_MODULE_CACHE = {}


def _parse_module(source):
    """The syntax tree of `source`. Trees are immutable, so the last few are
    shared."""
    hit = _MODULE_CACHE.get(source)
    if hit is not None:
        return hit
    try:
        module = cst.parse_module(source)
    except cst.ParserSyntaxError as e:
        raise ValueError("line %s: %s" % (e.raw_line, e.message)) from None
    if len(_MODULE_CACHE) >= 6:
        _MODULE_CACHE.pop(next(iter(_MODULE_CACHE)))
    _MODULE_CACHE[source] = module
    return module


_BUILD_CACHE = {}


def _build(source):
    """The blocks, node table and tree for `source`. Rebuilding views parses
    the same text again and again (positions are the slow part), so the last
    few results are kept; they are read-only, since every edit makes a new
    tree and a new source."""
    hit = _BUILD_CACHE.get(source)
    if hit is not None:
        return hit
    out = _build_uncached(source)
    if len(_BUILD_CACHE) >= 6:
        _BUILD_CACHE.pop(next(iter(_BUILD_CACHE)))
    _BUILD_CACHE[source] = out
    return out


def _build_uncached(source):
    module = _parse_module(source)
    ps = _Parser(module)
    items = []
    program = {"source": source, "items": items, "level": LANGUAGE_LEVEL, "doc": None}
    for i, n in enumerate(module.body):
        if i == 0 and (d := _docstring(n)) is not None:
            program["doc"] = d.rstrip("\n")
            continue
        if isinstance(n, cst.FunctionDef):
            items.append(ps.function(n))
        else:
            code = module.code_for_node(n).strip("\n")
            item = {"k": "code", "text": code, "notes": _notes(n), "trail": None}
            items.append(ps._new(item, n))
    return program, ps.nodes, module


def error_line(trace):
    """The last source line a Python traceback points at in a script (the
    `File "<string>", line N` entries), or None."""
    import re

    lines = re.findall(r'File "<string>", line (\d+)', trace or "")
    return int(lines[-1]) if lines else None


def parse(source):
    """The program as blocks: `{"source", "items", "doc"}`, or, if the source
    is not valid Python, the same with an `error` and no items.

    Items are `function` blocks (some locked) and `code` items for every
    other top-level statement (imports, classes, assignments), shown as
    read-only text. Every block has an `id` that edits refer to; ids belong
    to this parse.
    """
    try:
        program, _, _ = _build(source)
    except ValueError as e:
        return {"source": source, "items": [], "error": str(e), "doc": None}
    return program


# ---------------------------------------------------------------- blocks -> Python


def to_ast_expr(b):
    k = b["k"]
    if k == "lit":
        t, v = b["t"], b.get("v")
        if t == "none":
            return ast.Constant(None)
        if t == "bool":
            return ast.Constant(bool(v))
        if t == "int":
            return _num(int(v))
        if t == "float":
            f = float(v)
            if f != f or f in (float("inf"), float("-inf")):
                raise ValueError("a float literal must be finite")
            return _num(f)
        if t == "str":
            return ast.Constant(str(v))
        raise ValueError("unknown literal type %r" % t)
    if k == "name":
        _check_identifier(b["name"])
        return ast.Name(b["name"], ast.Load())
    if k == "attr":
        _check_identifier(b["attr"])
        return ast.Attribute(to_ast_expr(b["value"]), b["attr"], ast.Load())
    if k == "call":
        kws = []
        for name, v in b.get("kwargs", []):
            _check_identifier(name)
            kws.append(ast.keyword(name, to_ast_expr(v)))
        if len({kw.arg for kw in kws}) != len(kws):
            raise ValueError("a keyword argument is given twice")
        return ast.Call(to_ast_expr(b["func"]), [to_ast_expr(a) for a in b.get("args", [])], kws)
    if k == "binop":
        return ast.BinOp(to_ast_expr(b["l"]), _BINOP_CLS[b["op"]](), to_ast_expr(b["r"]))
    if k == "unary":
        return ast.UnaryOp(_UNARY_CLS[b["op"]](), to_ast_expr(b["v"]))
    if k == "boolop":
        return ast.BoolOp(_BOOL_CLS[b["op"]](), [to_ast_expr(v) for v in b["values"]])
    if k == "compare":
        return ast.Compare(to_ast_expr(b["l"]), [_CMP_CLS[o]() for o in b["ops"]], [to_ast_expr(r) for r in b["rs"]])
    if k == "ifexp":
        return ast.IfExp(to_ast_expr(b["test"]), to_ast_expr(b["then"]), to_ast_expr(b["else"]))
    if k == "index":
        return ast.Subscript(to_ast_expr(b["value"]), to_ast_expr(b["index"]), ast.Load())
    if k == "list":
        return ast.List([to_ast_expr(e) for e in b["items"]], ast.Load())
    if k == "tuple":
        return ast.Tuple([to_ast_expr(e) for e in b["items"]], ast.Load())
    if k == "set":
        if not b["items"]:
            # `{}` is a dict; an empty set is `set()` (design doc 4.4).
            return ast.Call(ast.Name("set", ast.Load()), [], [])
        return ast.Set([to_ast_expr(e) for e in b["items"]])
    if k == "dict":
        return ast.Dict([to_ast_expr(a) for a, _ in b["items"]], [to_ast_expr(v) for _, v in b["items"]])
    if k == "lambda":
        for n in b["params"]:
            _check_identifier(n)
        args = ast.arguments([], [ast.arg(n) for n in b["params"]], None, [], [], None, [])
        return ast.Lambda(args, to_ast_expr(b["body"]))
    if k == "comp":
        gens = []
        for c in b["clauses"]:
            gens.append(ast.comprehension(_store(to_ast_expr(c["target"])), to_ast_expr(c["iter"]), [to_ast_expr(i) for i in c.get("ifs", [])], 0))
        if not gens:
            raise ValueError("a comprehension needs a for clause")
        if b["kind"] == "dict":
            return ast.DictComp(to_ast_expr(b["key"]), to_ast_expr(b["value"]), gens)
        cls = {"list": ast.ListComp, "set": ast.SetComp, "gen": ast.GeneratorExp}[b["kind"]]
        return cls(to_ast_expr(b["elt"]), gens)
    raise ValueError("%r is not an expression block" % k)


def _num(v):
    # A negative number is a unary minus in Python's grammar.
    if v < 0 or (isinstance(v, float) and str(v).startswith("-")):
        return ast.UnaryOp(ast.USub(), ast.Constant(-v))
    return ast.Constant(v)


def _check_identifier(s):
    import keyword

    if not isinstance(s, str) or not s.isidentifier() or keyword.iskeyword(s):
        raise ValueError("%r is not an identifier" % (s,))


def _store(e):
    if isinstance(e, ast.Tuple):
        e.ctx = ast.Store()
        for x in e.elts:
            _store(x)
    elif isinstance(e, (ast.Name, ast.Attribute, ast.Subscript)):
        e.ctx = ast.Store()
    else:
        raise ValueError("cannot assign to that")
    return e


def to_ast_stmt(b):
    k = b["k"]
    if k == "assign":
        return ast.Assign([_store(to_ast_expr(b["target"]))], to_ast_expr(b["value"]))
    if k == "change":
        return ast.AugAssign(_store(to_ast_expr(b["target"])), _BINOP_CLS[b["op"]](), to_ast_expr(b["value"]))
    if k == "declare":
        _check_identifier(b["name"])
        v = b.get("value")
        return ast.AnnAssign(ast.Name(b["name"], ast.Store()), to_ast_expr(b["type"]), None if v is None else to_ast_expr(v), 1)
    if k == "discard":
        return ast.Expr(to_ast_expr(b["value"]))
    if k == "if":
        return ast.If(to_ast_expr(b["test"]), _body(b["body"]), [to_ast_stmt(s) for s in b.get("orelse", [])])
    if k == "while":
        return ast.While(to_ast_expr(b["test"]), _body(b["body"]), [])
    if k == "for":
        return ast.For(_store(to_ast_expr(b["target"])), to_ast_expr(b["iter"]), _body(b["body"]), [])
    if k == "return":
        v = b.get("value")
        return ast.Return(None if v is None else to_ast_expr(v))
    if k == "pass":
        return ast.Pass()
    if k == "break":
        return ast.Break()
    if k == "continue":
        return ast.Continue()
    raise ValueError("%r is not a statement block" % k)


def _body(stmts):
    return [to_ast_stmt(s) for s in stmts] or [ast.Pass()]


def expr_text(b):
    return ast.unparse(ast.fix_missing_locations(to_ast_expr(b)))


def stmt_text(b, indent=0):
    """Python for a statement block, every line indented by `indent` spaces."""
    node = ast.fix_missing_locations(to_ast_stmt(b))
    text = ast.unparse(node)
    pad = " " * indent
    return "\n".join(pad + line if line else line for line in text.split("\n"))


def function_text(name, params=(), decorators=(), body=None, position=None, indent=0):
    """A whole function. `params` are `(name, type_block_or_None)`, `decorators`
    are expression blocks, `body` a list of statement blocks."""
    for p, _ in params:
        _check_identifier(p)
    _check_identifier(name)
    lines = []
    pad = " " * indent
    if position is not None:
        lines.append("%s@sage.position(%s, %s)" % (pad, _fmt_num(position[0]), _fmt_num(position[1])))
    for d in decorators:
        lines.append("%s@%s" % (pad, expr_text(d)))
    args = ", ".join(p if t is None else "%s: %s" % (p, expr_text(t)) for p, t in params)
    lines.append("%sdef %s(%s):" % (pad, name, args))
    for s in (body or [{"k": "pass"}]):
        lines.append(stmt_text(s, indent + 4))
    return "\n".join(lines)


def _fmt_num(v):
    return repr(int(v)) if float(v).is_integer() else repr(float(v))


# ---------------------------------------------------------------- editing


class _Rewrite(cst.CSTTransformer):
    """Applies edits keyed by the identity of the node they change."""

    def __init__(self, edits):
        super().__init__()
        self.edits = edits

    def on_leave(self, original_node, updated_node):
        f = self.edits.get(id(original_node))
        return f(updated_node) if f else updated_node


_ATOMS = ("lit", "name", "attr", "call", "index", "list", "tuple", "set", "dict", "comp")


class Blocks:
    """A source file open in the block editor.

    Every edit method returns the new source (also kept in `source`) and
    re-parses, so ids refer to the latest parse. A failed edit raises
    `ValueError` and changes nothing.
    """

    def __init__(self, source=""):
        self.source = source
        self._load(source)

    def _load(self, source):
        self.program, self.nodes, self.module = _build(source)

    @property
    def items(self):
        return self.program["items"]

    # -- lookup

    def find(self, block_id):
        """`(block, parent, field)`: the block, the block that owns it, and
        the key it sits under (a list or a slot)."""
        found = []

        def walk(b, parent, field):
            if isinstance(b, dict):
                if b.get("id") == block_id:
                    found.append((b, parent, field))
                    return
                owner = b if "k" in b else parent
                for key, v in b.items():
                    walk(v, owner, key)
            elif isinstance(b, list):
                for v in b:
                    walk(v, parent, field)

        walk(self.program["items"], None, None)
        if not found:
            raise ValueError("no block %r" % (block_id,))
        return found[0]

    def function(self, name):
        for it in self.items:
            if it["k"] == "function" and it["name"] == name:
                return it
        raise ValueError("no function %r" % (name,))

    def _apply(self, edits, module=None):
        module = module or self.module
        new = module.visit(_Rewrite(edits))
        code = new.code
        try:
            self._load(code)
        except ValueError as e:
            self._load(self.source)
            raise ValueError("the edit makes invalid Python: %s" % e) from None
        self.source = code
        return code

    def _node(self, block_id):
        self.find(block_id)
        return self.nodes[block_id]

    def block_at_line(self, line):
        """The innermost statement block that covers a source line (1-based),
        or None: where a traceback line points ("Why?")."""
        best = []

        def walk(b):
            if isinstance(b, dict):
                if b.get("k") in STATEMENTS and b.get("line", 0) <= line <= b.get("end", -1):
                    best.append(b)
                for v in b.values():
                    walk(v)
            elif isinstance(b, list):
                for v in b:
                    walk(v)

        walk(self.program["items"])
        return min(best, key=lambda b: b["end"] - b["line"]) if best else None

    def text(self, block_id):
        """The Python text of a block, as written (comments included)."""
        self.find(block_id)
        return self.module.code_for_node(self.nodes[block_id]).strip("\n")

    # -- expressions and statements

    def replace(self, block_id, new_block):
        """Replaces an expression or a statement with `new_block`. A
        statement keeps its notes and trailing comment."""
        block, parent, field = self.find(block_id)
        k = block["k"]
        if k in ("function", "code"):
            raise ValueError("use replace_function")
        old = self.nodes[block_id]
        try:
            if k in STATEMENTS:
                if block.get("elif") and new_block["k"] != "if":
                    raise ValueError("an elif must stay an if")
                if new_block["k"] in TERMINAL and parent is not None and isinstance(parent.get(field), list):
                    sibs = parent[field]
                    if sibs.index(block) < len(sibs) - 1:
                        raise ValueError("%s MUST BE THE LAST BLOCK OF ITS BODY" % new_block["k"].upper())
                new = cst.parse_statement(stmt_text(new_block))
                keep = {"leading_lines": old.leading_lines}
                if isinstance(old, cst.SimpleStatementLine) and isinstance(new, cst.SimpleStatementLine):
                    keep["trailing_whitespace"] = old.trailing_whitespace
                elif isinstance(old, cst.If) and isinstance(new, cst.If):
                    pass
                new = new.with_changes(**keep)
            else:
                text = expr_text(new_block)
                nested = parent is not None and parent.get("k") not in STATEMENTS
                if nested and new_block["k"] not in _ATOMS and not old.lpar:
                    text = "(" + text + ")"
                new = cst.parse_expression(text)
                if old.lpar and not new.lpar:
                    new = new.with_changes(lpar=old.lpar, rpar=old.rpar)
        except (KeyError, TypeError, cst.ParserSyntaxError) as e:
            raise ValueError("not a valid block: %s" % (e,)) from None
        return self._apply({id(old): lambda _u, new=new: new})

    def set_literal(self, block_id, type_, value):
        """Changes a literal block's value (validated; design doc 4.4)."""
        block, _, _ = self.find(block_id)
        if block["k"] != "lit":
            raise ValueError("not a literal")
        return self.replace(block_id, _lit(type_, _literal_value(type_, value)))

    # -- bodies

    def _holder(self, parent, field):
        """The tree node that holds a statement list, and how many leading
        statements in it are not blocks (the docstring)."""
        node = self.nodes[parent["id"]]
        if parent["k"] == "function":
            if parent["locked"]:
                raise ValueError("the function is locked: %s" % parent["locked"])
            return node.body, 1 if parent.get("has_doc") else 0
        if field == "body":
            return node.body, 0
        if field == "orelse":
            if isinstance(node.orelse, cst.Else):
                return node.orelse.body, 0
            if isinstance(node.orelse, cst.If):
                raise ValueError("this else is an elif; edit the elif's own body")
        raise ValueError("no %s here" % field)

    def _function_locked(self, block_id):
        for it in self.items:
            if it["k"] == "function" and it["locked"] and _contains(it, block_id):
                raise ValueError("the function is locked: %s" % it["locked"])

    @staticmethod
    def _fresh(block):
        node = cst.parse_statement(stmt_text(block))
        return node.with_changes(leading_lines=())

    def insert(self, parent_id, field, index, new_block):
        """Inserts a statement into `field` ("body" or "orelse") of a
        function or a statement, before position `index` (at the end if
        past it)."""
        parent, _, _ = self.find(parent_id)
        holder, off = self._holder(parent, field)
        check_sequence(parent[field], index, new_block)
        new = self._fresh(new_block)
        at = off + max(0, min(index, len(parent[field])))

        def edit(u):
            body = list(u.body)
            body.insert(at, new)
            return u.with_changes(body=tuple(body))

        return self._apply({id(holder): edit})

    def add_else(self, if_id):
        """Gives an `if` an else branch (`pass`)."""
        block, _, _ = self.find(if_id)
        node = self.nodes[if_id]
        if block["k"] != "if" or node.orelse is not None:
            raise ValueError("this block has no free else")
        self._function_locked(if_id)
        els = cst.Else(body=cst.IndentedBlock(body=[cst.parse_statement("pass")]))
        return self._apply({id(node): lambda u: u.with_changes(orelse=els)})

    def _remove_edit(self, block, parent, field):
        """`(holder, edit)` removing a statement from its list."""
        if parent["k"] == "if" and field == "orelse" and block.get("elif"):
            pnode = self.nodes[parent["id"]]
            tail = self.nodes[block["id"]].orelse
            return pnode, lambda u: u.with_changes(orelse=tail)
        holder, off = self._holder(parent, field)
        i = parent[field].index(block) + off
        only = len(parent[field]) == 1 and not off
        pas = self._fresh({"k": "pass"})

        def edit(u):
            body = list(u.body)
            if only:
                body[i] = pas.with_changes(leading_lines=body[i].leading_lines) if False else pas
            else:
                del body[i]
            return u.with_changes(body=tuple(body))

        return holder, edit

    def delete(self, block_id):
        """Removes a statement and its notes. The last statement in a body
        becomes `pass`."""
        block, parent, field = self.find(block_id)
        if block["k"] not in STATEMENTS:
            raise ValueError("only statements are deleted")
        if parent is None:
            raise ValueError("a statement needs an owner")
        holder, edit = self._remove_edit(block, parent, field)
        return self._apply({id(holder): edit})

    def move(self, block_id, parent_id, field, index):
        """Moves a statement, with its notes and its formatting, into
        `field` of another function or statement at position `index`."""
        block, parent, pfield = self.find(block_id)
        if block["k"] not in STATEMENTS or parent is None:
            raise ValueError("only statements move")
        dest, _, _ = self.find(parent_id)
        if _contains(block, parent_id):
            raise ValueError("a block cannot move into itself")
        if block.get("elif"):
            raise ValueError("an elif moves with its if")
        moving = self.nodes[block_id]
        dholder, doff = self._holder(dest, field)
        sholder, sedit = self._remove_edit(block, parent, pfield)
        dlist = dest[field]
        at = max(0, min(index, len(dlist)))
        rest = [b for b in dlist if b is not block]
        check_sequence(rest, at - (1 if dholder is sholder and at > parent[pfield].index(block) else 0) if dholder is sholder else at, block)
        if dholder is sholder:
            # One list: take the statement out, then put it at the target
            # (index counted without it).
            si = parent[pfield].index(block)
            if at > si:
                at -= 1
            if at == si:
                raise ValueError("that is where it already is")
            i0, aft = si + doff, at + doff

            def edit(u):
                body = list(u.body)
                node = body.pop(i0)
                body.insert(aft, node)
                return u.with_changes(body=tuple(body))

            return self._apply({id(dholder): edit})

        def put(u):
            body = list(u.body)
            body.insert(doff + at, moving)
            return u.with_changes(body=tuple(body))

        return self._apply({id(sholder): sedit, id(dholder): put})

    # -- notes: comments and docstrings

    def set_notes(self, block_id, lines):
        """Replaces the comment lines directly above a statement, function or
        top-level item. Blank lines above the comments stay."""
        block, _, _ = self.find(block_id)
        if block["k"] not in STATEMENTS + ("function", "code"):
            raise ValueError("only statements and functions carry notes")
        node = self.nodes[block_id]
        lines = [l for text in lines for l in str(text).split("\n")]

        def edit(u):
            kept = []
            for l in u.leading_lines:
                if l.comment is not None:
                    break
                kept.append(l)
            new = [cst.EmptyLine(comment=cst.Comment("# " + t if t else "#")) for t in lines]
            return u.with_changes(leading_lines=tuple(kept + new))

        return self._apply({id(node): edit})

    def set_docstring(self, name, text):
        """Sets, replaces or (with None) removes a function's docstring."""
        f = self.function(name)
        if f["locked"]:
            raise ValueError("the function is locked: %s" % f["locked"])
        node = self.nodes[f["id"]]
        body = node.body

        def edit(u):
            rest = list(u.body.body)
            has = bool(rest) and _docstring(rest[0]) is not None
            if text is None:
                if has:
                    rest.pop(0)
                    if not rest:
                        rest = [cst.parse_statement("pass")]
            else:
                new = cst.parse_statement(_docstring_source(text))
                if has:
                    rest[0] = new.with_changes(leading_lines=rest[0].leading_lines)
                else:
                    rest.insert(0, new)
                    if len(rest) == 2 and _is_lone_pass(rest[1]):
                        rest.pop()
            return u.with_changes(body=u.body.with_changes(body=tuple(rest)))

        return self._apply({id(node): edit})

    # -- functions

    def add_function(self, name, params=(), decorators=(), body=None, position=None, notes=()):
        text = function_text(name, params, decorators, body, position)
        node = cst.parse_statement(text)
        lead = [cst.EmptyLine(), cst.EmptyLine()] if self.module.body else []
        lead += [cst.EmptyLine(comment=cst.Comment("# " + n)) for n in notes]
        node = node.with_changes(leading_lines=tuple(lead))
        module = self.module.with_changes(body=tuple(self.module.body) + (node,))
        self.module = module
        code = module.code
        self._load(code)
        self.source = code
        return code

    def delete_function(self, name):
        f = self.function(name)
        node = self.nodes[f["id"]]
        body = tuple(n for n in self.module.body if n is not node)
        code = self.module.with_changes(body=body).code
        self._load(code)
        self.source = code
        return code

    def set_position(self, name, x, y):
        """Moves a stack on the canvas: the `@sage.position(x, y)` decorator
        (design doc 4.1)."""
        f = self.function(name)
        if f["locked"]:
            raise ValueError("the function is locked: %s" % f["locked"])
        node = self.nodes[f["id"]]
        dec = cst.Decorator(decorator=cst.parse_expression("sage.position(%s, %s)" % (_fmt_num(x), _fmt_num(y))))

        def edit(u):
            ds = list(u.decorators)
            for i, d in enumerate(ds):
                if _position(d.decorator) is not None:
                    ds[i] = dec.with_changes(leading_lines=d.leading_lines)
                    break
            else:
                ds.insert(0, dec)
            return u.with_changes(decorators=tuple(ds))

        return self._apply({id(node): edit})

    def set_system_slot(self, name, module, phase):
        """Moves a system to a module (`None` is the script's own, `world`)
        and a phase (`None` is the default): the crosstab's cell (design doc
        5.5). Only the decorator changes."""
        f = self.function(name)
        if f["locked"]:
            raise ValueError("the function is locked: %s" % f["locked"])
        for d in f["decorators"]:
            if d["k"] == "call" and d["func"]["k"] == "attr" and d["func"]["attr"] == "system":
                new = _strip_blocks(d)
                new["func"]["value"] = {"k": "name", "name": module or "world"}
                new["kwargs"] = [kw for kw in new["kwargs"] if kw[0] != "phase"]
                if phase is not None:
                    new["kwargs"].append(["phase", {"k": "name", "name": phase}])
                return self.replace(d["id"], new)
        raise ValueError("%s is not a system" % name)

    def rename_function(self, name, new_name):
        _check_identifier(new_name)
        f = self.function(name)
        if f["locked"]:
            raise ValueError("the function is locked: %s" % f["locked"])
        node = self.nodes[f["id"]]
        return self._apply({id(node): lambda u: u.with_changes(name=cst.Name(new_name))})


def _strip_blocks(b):
    """A copy of parsed blocks without ids, spans and notes."""
    if isinstance(b, dict):
        return {k: _strip_blocks(v) for k, v in b.items() if k not in ("id", "span", "line", "end", "notes", "trail", "elif")}
    if isinstance(b, list):
        return [_strip_blocks(v) for v in b]
    return b


def _docstring_source(text):
    body = str(text).replace("\\", "\\\\").replace('"""', '\\"\\"\\"')
    if "\n" in body:
        return '"""%s\n"""' % body.rstrip("\n")
    return '"""%s"""' % body


def _is_lone_pass(stmt):
    return isinstance(stmt, cst.SimpleStatementLine) and len(stmt.body) == 1 and isinstance(stmt.body[0], cst.Pass)


def _contains(block, block_id):
    found = []

    def walk(b):
        if isinstance(b, dict):
            if b.get("id") == block_id:
                found.append(1)
            for v in b.values():
                walk(v)
        elif isinstance(b, list):
            for v in b:
                walk(v)

    walk(block)
    return bool(found)


def _literal_value(type_, value):
    """A literal's value from what a person typed, checked with
    `ast.literal_eval` so nothing else can get in (design doc 4.4)."""
    if type_ == "none":
        return None
    if type_ == "str":
        return str(value)
    if type_ == "bool":
        if isinstance(value, bool):
            return value
        if str(value).strip().lower() in ("true", "false"):
            return str(value).strip().lower() == "true"
        raise ValueError("%r is not a bool" % (value,))
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        v = value
    else:
        try:
            v = ast.literal_eval(str(value).strip())
        except (ValueError, SyntaxError):
            raise ValueError("%r is not a number" % (value,)) from None
    if isinstance(v, bool) or not isinstance(v, (int, float)):
        raise ValueError("%r is not a number" % (value,))
    if type_ == "int":
        if isinstance(v, float):
            if not v.is_integer():
                raise ValueError("%r is not a whole number" % (value,))
            v = int(v)
        return v
    if type_ == "float":
        v = float(v)
        if v != v or v in (float("inf"), float("-inf")):
            raise ValueError("a float literal must be finite")
        return v
    raise ValueError("unknown literal type %r" % (type_,))


def convert_literal(block, type_):
    """Changes a literal's type by cast rules (design doc 4.4): numbers
    convert to each other when exact, anything converts to text, text to a
    number only if it reads as one, bool to int. Raises `ValueError`
    when the value cannot be kept."""
    if block["k"] != "lit":
        raise ValueError("not a literal")
    v, t = block.get("v"), block["t"]
    if t == type_:
        return dict(block)
    if type_ == "str":
        return _lit("str", "None" if t == "none" else str(v))
    if type_ == "none":
        return _lit("none", None)
    if t == "str":
        return _lit(type_, _literal_value(type_, v))
    if t == "none":
        return _lit(type_, {"int": 0, "float": 0.0, "bool": False}[type_])
    if t == "bool" and type_ in ("int", "float"):
        return _lit(type_, int(v) if type_ == "int" else float(v))
    return _lit(type_, _literal_value(type_, v))


# ---------------------------------------------------------------- debugging: glowing blocks


GLOW_HITS = {}  # by OS thread: each editor reads only what its own thread ran


def _glow(line, value=None):
    import threading

    GLOW_HITS.setdefault(threading.get_ident(), set()).add(line)
    return value


def glow_take():
    """The source lines that ran on this thread since the last call (for
    glowing blocks)."""
    import threading

    hits = sorted(GLOW_HITS.pop(threading.get_ident(), ()))
    return hits


def install_glow():
    """Makes `_sage_glow` visible to instrumented scripts."""
    import builtins

    builtins._sage_glow = _glow


class _Instrument(cst.CSTTransformer):
    METADATA_DEPENDENCIES = (cst.metadata.PositionProvider,)

    def __init__(self):
        super().__init__()
        self.depth = 0

    def visit_FunctionDef(self, node):
        self.depth += 1

    def leave_FunctionDef(self, original, updated):
        self.depth -= 1
        return updated

    def _call(self, node, value=None):
        line = self.get_metadata(cst.metadata.PositionProvider, node).start.line
        args = [cst.Arg(cst.Integer(str(line)))]
        if value is not None:
            args.append(cst.Arg(value))
        return cst.Call(cst.Name("_sage_glow"), args)

    def leave_SimpleStatementLine(self, original, updated):
        if self.depth and len(updated.body) >= 1 and not _docstring(updated):
            hit = cst.Expr(self._call(original))
            return updated.with_changes(body=[hit, *updated.body])
        return updated

    def leave_If(self, original, updated):
        if self.depth:
            return updated.with_changes(test=self._call(original, updated.test))
        return updated

    def leave_While(self, original, updated):
        if self.depth:
            return updated.with_changes(test=self._call(original, updated.test))
        return updated

    def leave_For(self, original, updated):
        if self.depth:
            return updated.with_changes(iter=self._call(original, updated.iter))
        return updated


def instrument(source):
    """The same program with every statement in a function reporting its line
    to `_sage_glow` as it runs, without moving any line (a statement's report
    shares its line; a test or iterable is wrapped). Used while debugging in
    the editor; it changes no state, so a world hashes the same."""
    module = cst.parse_module(source)
    return cst.metadata.MetadataWrapper(module).visit(_Instrument()).code


# ---------------------------------------------------------------- phases and prefabs (design doc 5.4, 5.5)


def _decl_calls(module):
    """`(kind, name, call_node, assign_line_node)` for top-level
    `X = world.phase("X", after=Y)` and `X = world.prefab("X", base=Y)`."""
    out = []
    for n in module.body:
        if not (isinstance(n, cst.SimpleStatementLine) and len(n.body) == 1 and isinstance(n.body[0], cst.Assign)):
            continue
        a = n.body[0]
        v = a.value
        if (
            isinstance(v, cst.Call)
            and isinstance(v.func, cst.Attribute)
            and isinstance(v.func.value, cst.Name)
            and v.func.value.value == "world"
            and v.func.attr.value in ("phase", "prefab", "module")
            and len(a.targets) == 1
            and isinstance(a.targets[0].target, cst.Name)
        ):
            out.append((v.func.attr.value, a.targets[0].target.value, v, n))
    return out


def declarations(source):
    """Phases and prefabs a script declares: `[{"kind", "var", "name", "ref"}]`
    where `ref` is the `after=` (phases) or `base=` (prefabs) variable."""
    module = _parse_module(source)
    out = []
    for kind, var, call, _ in _decl_calls(module):
        key = "after" if kind == "phase" else "base"
        ref = None
        for a in (call.args if kind != "module" else []):
            if a.keyword is not None and a.keyword.value == key and isinstance(a.value, cst.Name):
                ref = a.value.value
        name = None
        if call.args and isinstance(call.args[0].value, cst.SimpleString):
            name = ast.literal_eval(call.args[0].value.value)
        out.append({"kind": kind, "var": var, "name": name, "ref": ref})
    return out


def _creates_cycle(decls, var, ref):
    graph = {d["var"]: d["ref"] for d in decls}
    graph[var] = ref
    seen, cur = set(), var
    while cur is not None and cur in graph:
        if cur in seen:
            return True
        seen.add(cur)
        cur = graph[cur]
    return False


def set_decl_ref(source, var, ref):
    """Makes phase or prefab `var` run after (or derive from) `ref`, or
    nothing (`None`). A cycle is refused (design doc 5.5 pitfall c); so is a
    name that is not declared. Everything else in the file is untouched."""
    decls = declarations(source)
    mine = [d for d in decls if d["var"] == var]
    if not mine:
        raise ValueError("no phase or prefab %r" % (var,))
    if ref is not None:
        _check_identifier(ref)
        if ref not in {d["var"] for d in decls if d["kind"] == mine[0]["kind"]}:
            raise ValueError("%r is not a declared %s" % (ref, mine[0]["kind"]))
        if _creates_cycle(decls, var, ref):
            raise ValueError("that would make %s depend on itself" % var)
    module = _parse_module(source)
    key = "after" if mine[0]["kind"] == "phase" else "base"
    for kind, v, call, line in _decl_calls(module):
        if v != var:
            continue
        args = [a for a in call.args if not (a.keyword is not None and a.keyword.value == key)]
        if ref is not None:
            args.append(cst.Arg(value=cst.Name(ref), keyword=cst.Name(key), equal=cst.AssignEqual(cst.SimpleWhitespace(""), cst.SimpleWhitespace(""))))
        # Keep commas right: every arg but the last carries one.
        fixed = []
        for i, a in enumerate(args):
            comma = cst.Comma(whitespace_after=cst.SimpleWhitespace(" ")) if i < len(args) - 1 else cst.MaybeSentinel.DEFAULT
            fixed.append(a.with_changes(comma=comma))
        new = call.with_changes(args=fixed)
        return module.deep_replace(call, new).code
    raise ValueError("no phase or prefab %r" % (var,))


def add_declaration(source, kind, var):
    """Appends `Var = world.phase("Var")` (or `world.prefab`) after the last
    declaration, else after the imports, else at the top."""
    _check_identifier(var)
    if kind not in ("phase", "prefab"):
        raise ValueError("kind is phase or prefab")
    if any(d["var"] == var for d in declarations(source)):
        raise ValueError("%s is already declared" % var)
    module = _parse_module(source)
    node = cst.parse_statement('%s = world.%s("%s")' % (var, kind, var))
    body = list(module.body)
    at = 0
    for i, n in enumerate(body):
        if any(n is d[3] for d in _decl_calls(module)):
            at = i + 1
        elif at == 0 and isinstance(n, cst.SimpleStatementLine) and n.body and isinstance(n.body[0], (cst.Import, cst.ImportFrom)):
            at = i + 1
    body.insert(at, node)
    return module.with_changes(body=body).code


def systems_in(source):
    """Which functions run in which phase or belong to which prefab:
    `[{"name", "phase", "prefab"}]` from `phase=` keywords and
    `@Prefab.system` decorators."""
    out = []
    for item in parse(source)["items"]:
        if item["k"] != "function":
            continue
        phase = prefab = None
        for d in item["decorators"]:
            if d["k"] == "call":
                for k, v in d["kwargs"]:
                    if k == "phase":
                        phase = v["name"] if v["k"] == "name" else (v["attr"] if v["k"] == "attr" else None)
                f = d["func"]
                if f["k"] == "attr" and f["attr"] in ("system", "observer") and f["value"]["k"] == "name" and f["value"]["name"] != "world":
                    prefab = f["value"]["name"]
            elif d["k"] == "attr" and d["attr"] in ("system", "observer") and d["value"]["k"] == "name":
                prefab = d["value"]["name"]
        out.append({"name": item["name"], "phase": phase, "prefab": prefab, "owner": prefab})
    return out


def phase_layers(decls):
    """Phases as columns left to right: a phase sits one column right of the
    phase it runs after (design doc 5.5)."""
    phases = {d["var"]: d for d in decls if d["kind"] == "phase"}
    depth = {}

    def d(v, seen=()):
        if v in depth:
            return depth[v]
        ref = phases[v]["ref"]
        depth[v] = 0 if ref is None or ref not in phases or ref in seen else d(ref, seen + (v,)) + 1
        return depth[v]

    for v in phases:
        d(v)
    layers = {}
    for v, k in depth.items():
        layers.setdefault(k, []).append(v)
    return [sorted(layers[k]) for k in sorted(layers)]


# ---------------------------------------------------------------- palette from reflection


def _short(path):
    return path.replace("::", ".").rsplit(".", 1)[-1]


def _category(path):
    parts = path.replace("::", ".").split(".")
    return parts[-2] if len(parts) > 1 else "game"


def _block(label, shape, template, **extra):
    d = {"label": label, "shape": shape, "block": template}
    d.update(extra)
    return d


def _name(i):
    return {"k": "name", "name": i}


def _attr(v, a):
    return {"k": "attr", "value": v, "attr": a}


_NUMERIC = ("u8", "i32", "u32", "i64", "u64", "f32", "f64")


def _default(kind):
    if kind == "bool":
        return _lit("bool", False)
    if kind in ("f32", "f64"):
        return _lit("float", 0.0)
    return _lit("int", 0)


def core_palette():
    """The blocks every program has (design doc 5.5: Scratch's Control,
    Operators, Variables), as palette entries."""
    t = _lit("bool", True)
    return [
        _block("if [] then", "command", {"k": "if", "test": t, "body": [{"k": "pass"}], "orelse": []}, category="Control"),
        _block("if [] then else", "command", {
            "k": "if", "test": t, "body": [{"k": "pass"}], "orelse": [{"k": "pass"}]}, category="Control"),
        _block("while []", "command", {"k": "while", "test": t, "body": [{"k": "pass"}]}, category="Control"),
        _block("for each [] in []", "command", {
            "k": "for", "target": _name("item"), "iter": _name("items"), "body": [{"k": "pass"}]}, category="Control"),
        _block("return", "command", {"k": "return", "value": None}, category="Control"),
        _block("pass", "command", {"k": "pass"}, category="Control"),
        _block("break", "command", {"k": "break"}, category="Control"),
        _block("continue", "command", {"k": "continue"}, category="Control"),
        _block("set [] to []", "command", {"k": "assign", "target": _name("x"), "value": _lit("int", 0)}, category="Variables"),
        _block("change [] by []", "command", {
            "k": "change", "target": _name("x"), "op": "+", "value": _lit("int", 1)}, category="Variables"),
        _block("declare [] : [] = []", "command", {
            "k": "declare", "name": "x", "type": _name("int"), "value": _lit("int", 0)}, category="Variables"),
        _block("do []", "command", {"k": "discard", "value": {"k": "call", "func": _name("print"), "args": [], "kwargs": []}}, category="Variables"),
        _block("[] + []", "reporter", {"k": "binop", "op": "+", "l": _lit("int", 0), "r": _lit("int", 0)}, category="Operators"),
        _block("[] - []", "reporter", {"k": "binop", "op": "-", "l": _lit("int", 0), "r": _lit("int", 0)}, category="Operators"),
        _block("[] * []", "reporter", {"k": "binop", "op": "*", "l": _lit("int", 0), "r": _lit("int", 0)}, category="Operators"),
        _block("[] / []", "reporter", {"k": "binop", "op": "/", "l": _lit("int", 0), "r": _lit("int", 1)}, category="Operators"),
        _block("[] < []", "predicate", {"k": "compare", "l": _lit("int", 0), "ops": ["<"], "rs": [_lit("int", 0)]}, category="Operators"),
        _block("[] == []", "predicate", {"k": "compare", "l": _lit("int", 0), "ops": ["=="], "rs": [_lit("int", 0)]}, category="Operators"),
        _block("[] and []", "predicate", {"k": "boolop", "op": "and", "values": [t, t]}, category="Operators"),
        _block("[] or []", "predicate", {"k": "boolop", "op": "or", "values": [t, t]}, category="Operators"),
        _block("not []", "predicate", {"k": "unary", "op": "not", "v": t}, category="Operators"),
        _block("whole number", "reporter", _lit("int", 0), category="Values"),
        _block("decimal number", "reporter", _lit("float", 0.0), category="Values"),
        _block("text", "reporter", _lit("str", ""), category="Values"),
        _block("true", "predicate", _lit("bool", True), category="Values"),
        _block("none", "reporter", _lit("none", None), category="Values"),
        _block("list", "reporter", {"k": "list", "items": []}, category="Values"),
        _block("list of numbers", "reporter", {"k": "list", "items": [_lit("int", 0), _lit("int", 0), _lit("int", 0)]}, category="Values"),
        _block("grid of numbers", "reporter", {"k": "list", "items": [{"k": "list", "items": [_lit("int", 0), _lit("int", 0)]}, {"k": "list", "items": [_lit("int", 0), _lit("int", 0)]}]}, category="Values"),
        _block("empty set", "reporter", {"k": "set", "items": []}, category="Values"),
        _block("call [] ( )", "reporter", {"k": "call", "func": _name("f"), "args": [], "kwargs": []}, category="Calls"),
        _block("method [].[] ( )", "reporter", {
            "k": "call", "func": _attr(_name("obj"), "method"), "args": [], "kwargs": []}, category="Calls"),
    ]


def _tn(i):
    return {"k": "name", "name": i}


def type_palette():
    """Type blocks (design doc 4.6): square, nested, and `any` as the visible
    escape hatch."""
    out = []
    for n in ("int", "float", "str", "bool"):
        out.append(_block(n, "type", _tn(n), category="Types"))
    out.append(_block("list of []", "type", {"k": "index", "value": _tn("list"), "index": _tn("int")}, category="Types"))
    out.append(_block("dict of [] to []", "type", {"k": "index", "value": _tn("dict"), "index": {"k": "tuple", "items": [_tn("str"), _tn("int")]}}, category="Types"))
    out.append(_block("[] or none", "type", {"k": "binop", "op": "|", "l": _tn("int"), "r": _lit("none", None)}, category="Types"))
    out.append(_block("any (checking off)", "type", _tn("Any"), category="Types"))
    return out


def type_of(b):
    """The type a value block is known to have, for typed sockets (design doc
    4.2, 4.4, 4.6): `"num"`, `"str"`, `"bool"`, `"none"`, `"tuple"`, `"set"`,
    `"dict"`, a list as `"list<T>"` (`"list<num>"`, `"list<list<num>>"`, or
    plain `"list"` when its elements are mixed, empty or unknown), or `None`
    when it is not known (a name, call, attribute or index: `any`).
    Editor-side only; nothing is checked at run time."""
    k = b["k"]
    if k == "lit":
        return {"int": "num", "float": "num", "str": "str", "bool": "bool", "none": "none"}[b["t"]]
    if k == "binop":
        l, r = type_of(b["l"]), type_of(b["r"])
        if b["op"] == "+":
            return l if l == r and l is not None and (l in ("num", "str") or l.startswith("list")) else None
        return "num" if l == "num" and r == "num" else None
    if k == "unary":
        return "bool" if b["op"] == "not" else "num"
    if k in ("compare",):
        return "bool"
    if k == "boolop":
        return "bool" if all(type_of(v) == "bool" for v in b["values"]) else None
    if k == "list":
        e = elem_type(b)
        return "list<%s>" % e if e else "list"
    if k in ("tuple", "set", "dict"):
        return k
    if k == "comp":
        return {"list": "list", "set": "set", "dict": "dict", "gen": None}[b["kind"]]
    return None


def elem_type(b):
    """The one type every element of a list block has, or `None` (empty,
    mixed or unknown). A list of lists has the element type `list<...>`."""
    kinds = {type_of(x) for x in b.get("items", [])}
    return kinds.pop() if len(kinds) == 1 and None not in kinds else None


def slot_type_for(annotation):
    """What an annotation says a value is: `int` or `float` a number, `str`
    text, `bool` a test, `list[int]` a list of numbers (nested lists too);
    anything else (`any`, a class) is not checked."""
    if not annotation:
        return None
    if annotation.get("k") == "name":
        n = annotation["name"]
        return {"int": "num", "float": "num", "str": "str", "bool": "bool", "list": "list"}.get(n)
    if annotation.get("k") == "index" and annotation["value"].get("k") == "name" and annotation["value"].get("name") == "list":
        inner = slot_type_for(annotation["index"])
        return "list<%s>" % inner if inner else "list"
    return None


def element_slot(wanted):
    """What each element of a slot that wants `wanted` must be: the `T` of
    `list<T>`, else `None` (anything)."""
    if wanted and wanted.startswith("list<") and wanted.endswith(">"):
        return wanted[5:-1]
    return None


def type_fits(value_type, wanted):
    """Whether a value of `value_type` may go in a slot that wants `wanted`
    (an unknown value fits anywhere; a slot with no wish takes anything; a
    plain `list` fits any list type, and a `list<T>` fits a plain `list`)."""
    if wanted is None or value_type is None or value_type == wanted:
        return True
    if value_type.startswith("list") and wanted.startswith("list"):
        v, w = element_slot(value_type), element_slot(wanted)
        return v is None or w is None or type_fits(v, w)
    return False


def default_for(type_):
    """A new element for a list of `type_`: 0, "", False, or an empty list."""
    if type_ == "num":
        return _lit("int", 0)
    if type_ == "str":
        return _lit("str", "")
    if type_ == "bool":
        return _lit("bool", False)
    if type_ and type_.startswith("list"):
        return {"k": "list", "items": []}
    return _lit("int", 0)


def grid_shape(b):
    """`(rows, cols)` when a list block is a grid: a non-empty list of equal
    length, non-empty lists, as a tile map is (design doc 4.4); else `None`."""
    items = b.get("items", [])
    if b.get("k") != "list" or not items or any(x["k"] != "list" or not x["items"] for x in items):
        return None
    widths = {len(x["items"]) for x in items}
    return (len(items), widths.pop()) if len(widths) == 1 else None


def grid_resize(b, add_row=0, add_col=0):
    """A grid block with a row or column added (`1`) or dropped (`-1`), new
    cells filled with the grid's element default. Pure: returns a new block."""
    shape = grid_shape(b)
    if shape is None:
        raise ValueError("not a grid")
    rows, cols = shape
    import copy

    out = copy.deepcopy(_strip_blocks(b))
    cell = elem_type({"items": out["items"][0]["items"]})
    if add_col:
        if add_col < 0 and cols <= 1:
            raise ValueError("a grid keeps at least one column")
        for r in out["items"]:
            if add_col > 0:
                r["items"].append(default_for(cell))
            else:
                r["items"].pop()
    if add_row:
        if add_row < 0 and rows <= 1:
            raise ValueError("a grid keeps at least one row")
        if add_row > 0:
            out["items"].append({"k": "list", "items": [default_for(cell) for _ in range(len(out["items"][0]["items"]))]})
        else:
            out["items"].pop()
    return out


def is_any(block):
    """Whether a type block is the `any` escape hatch (shown with a marker)."""
    return block.get("k") == "name" and block.get("name") in ("Any", "any")


def _query_terms(call):
    for k, v in call.get("kwargs", []):
        if k == "query" and v["k"] == "lit" and v["t"] == "str":
            terms = []
            for t in v["v"].split(","):
                t = t.strip().lstrip("!").lstrip("?")
                t = t.split("(")[0].split("[")[0].strip()
                terms.append(t.split(".")[-1])
            return v, terms
    return None, None


def function_issues(f):
    """Problems with a stack's hat (design doc 4.7 fallback): parameters whose
    component is not in the hat's explicit `query=` string. Each is
    `{"param", "component", "query_id"}`; fixing it adds the component to the
    query. Stacks without a `query=` take their query from the annotations and
    have none."""
    out = []
    if f.get("locked"):
        return out
    for d in f["decorators"]:
        if d["k"] != "call":
            continue
        q, terms = _query_terms(d)
        if q is None:
            continue
        for param, comp in scope_of(f):
            if comp not in terms:
                out.append({"param": param, "component": comp, "query_id": q["id"], "query": q["v"]})
    return out


def add_to_query(blocks, function_name, component):
    """Adds `component` to the hat's `query=` string."""
    f = blocks.function(function_name)
    for issue in function_issues(f):
        if issue["component"] == component:
            new = issue["query"].strip()
            new = (new + ", " if new else "") + component
            return blocks.replace(issue["query_id"], _lit("str", new))
    raise ValueError("%s is already in the query" % component)


def uses_name(block, name):
    """Whether a block mentions the variable `name` (to grey out what an
    edited query no longer provides)."""
    found = []

    def walk(b):
        if isinstance(b, dict):
            if b.get("k") == "name" and b.get("name") == name:
                found.append(1)
            for v in b.values():
                walk(v)
        elif isinstance(b, list):
            for v in b:
                walk(v)

    walk(block)
    return bool(found)


def relative_phases(decls, current=None):
    """The hat's drop-down in relative names (design doc 5.5): for each phase,
    "at P", and "before P" when P runs after something (the phase it needs).
    Returns `[(label, phase_var)]`; `"before P"` names the phase P runs after."""
    phases = {d["var"]: d for d in decls if d["kind"] == "phase"}
    out = []
    for var in sorted(phases):
        out.append(("at " + var, var))
        ref = phases[var]["ref"]
        if ref in phases:
            out.append(("before " + var, ref))
    return out


def palette(schemas, scope=()):
    """Blocks generated from the reflected schema (design doc 4.7).

    `schemas` maps a component path to its fields, `{path: [(field, kind)]}`;
    a component with no fields is a tag. `scope` is what the stack's hat
    makes available as the implicit "this": `[(param_name, component_path)]`.

    Each field gives get, set and (for numbers) change blocks, each tag gives
    add, remove and has. A component in scope reads through its parameter
    (`pos.x`); any other component needs an entity socket
    (`e.get(Position).x`). Each module (the path without the last segment) is
    a category.
    """
    out = core_palette() + type_palette()
    scoped = dict((path, name) for name, path in scope)
    for path in sorted(schemas):
        fields = schemas[path]
        comp = _short(path)
        cat = _category(path)
        if not fields:
            ent = _name("e")
            for verb, label, shape in (("add", "add [%s] to []", "command"), ("remove", "remove [%s] from []", "command"), ("has", "[] has [%s]", "predicate")):
                t = {"k": "call", "func": _attr(ent, verb), "args": [_name(comp)], "kwargs": []}
                if verb == "has":
                    out.append(_block(label % comp, shape, t, category=cat, component=path, slot="entity"))
                else:
                    out.append(_block(label % comp, shape, {"k": "discard", "value": t}, category=cat, component=path, slot="entity"))
            continue
        for field, kind in fields:
            if path in scoped:
                base, via = _attr(_name(scoped[path]), field), None
            else:
                got = {"k": "call", "func": _attr(_name("e"), "get"), "args": [_name(comp)], "kwargs": []}
                base, via = _attr(got, field), "entity"
            extra = {"category": cat, "component": path, "field": field, "kind": kind}
            if via:
                extra["slot"] = via
            out.append(_block("%s's %s" % (comp, field), "predicate" if kind == "bool" else "reporter", base, **extra))
            out.append(_block("set %s's %s to []" % (comp, field), "command", {"k": "assign", "target": base, "value": _default(kind)}, **extra))
            if kind in _NUMERIC:
                out.append(_block("change %s's %s by []" % (comp, field), "command", {"k": "change", "target": base, "op": "+", "value": _default(kind)}, **extra))
    return out


def new_system(name, scope, phase=None, query=None):
    """Source for a new system stack: the hat (design doc 5.4, 4.7) with the
    components in `scope` as its query terms. `scope` is
    `[(param_name, component_short_name)]`."""
    args = [(p, _name(c)) for p, c in scope]
    deco = [{"k": "call", "func": _attr(_name("world"), "system"), "args": [_lit("str", name)], "kwargs": (
        ([["phase", _name(phase)]] if phase else []) + ([["query", _lit("str", query)]] if query else []))}]
    return function_text(name, args, deco, [{"k": "pass"}])


def scope_of(function):
    """What a stack's hat makes available: its annotated parameters, as
    `[(param_name, annotation_name)]`. A parameter annotated `Entity`, or
    with no annotation, is "this" and not a component."""
    out = []
    for p in function.get("params", []):
        t = p.get("type")
        if t and t["k"] == "name" and t["name"] != "Entity":
            out.append((p["name"], t["name"]))
        elif t and t["k"] == "attr":
            out.append((p["name"], t["attr"]))
    return out


# -- a readable summary of a block, for the "text" tooltip and tests

def summarize(b):
    k = b["k"]
    if k in STATEMENTS:
        return stmt_text(b).split("\n")[0]
    return expr_text(b)
