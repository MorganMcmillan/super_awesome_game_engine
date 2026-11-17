"""Where a value comes from: which systems read and write which fields.

A spreadsheet can show the cells a formula reads and the formulas that read a
cell. A game here has the same graph, written down in its rules: a system's
parameters name the components it works on, and its body says which fields
it reads and which it assigns. This module reads that out of the scripts
(through `sage.blocks`, so it sees exactly what the block editor sees), for
the inspector to show beside each value and for the single-writer check
(design doc 8.3: a field with two writers in one phase is a race).

It reads the text; it does not run it. What it cannot see through, it says:
a function the block editor has to lock is reported as "unknown" instead of
being left out.

Pure Python over `sage.blocks`: no world, no UI.
"""

import blocks as B

READ, WRITE, UNKNOWN = "read", "write", "unknown"
#: A whole component (it is set, added or removed as one), or every field of
#: something that could not be read.
ALL = "*"


def _is_rule(f):
    """A system or an observer: a function some decorator registers."""
    for d in f["decorators"]:
        func = d["func"] if d["k"] == "call" else d
        if func["k"] == "attr" and func["attr"] in ("system", "observer"):
            return True
    return False


def _component_arg(call):
    """The component a call like `e.set(Position(1, 2))`, `e.get(Position)` or
    `e.add(Frozen)` names first, or None."""
    if not call["args"]:
        return None
    a = call["args"][0]
    if a["k"] == "call":
        a = a["func"]
    if a["k"] == "name" and a["name"][:1].isupper():
        return a["name"]
    if a["k"] == "attr" and a["attr"][:1].isupper():
        return a["attr"]
    return None


def _field_of(e, scope):
    """`(component, field)` when `e` is a field of a component: `pos.x` for a
    parameter `pos`, or `thing.get(Position).x`. Else None."""
    if e["k"] != "attr":
        return None
    v = e["value"]
    if v["k"] == "name" and v["name"] in scope:
        return scope[v["name"]], e["attr"]
    if v["k"] == "call" and v["func"]["k"] == "attr" and v["func"]["attr"] == "get":
        comp = _component_arg(v)
        if comp is not None:
            return comp, e["attr"]
    return None


def _walk(b, scope, out, line, writing=False):
    """Collects `(component, field, mode, line)` from a block and everything
    inside it. `writing` marks the target of an assignment."""
    if isinstance(b, list):
        for v in b:
            _walk(v, scope, out, line, writing)
        return
    if not isinstance(b, dict):
        return
    line = b.get("line") or line
    k = b.get("k")
    hit = _field_of(b, scope)
    if hit is not None:
        out.append((hit[0], hit[1], WRITE if writing else READ, line))
        # What the field is read off is itself only read.
        _walk(b["value"], scope, out, line)
        return
    if k in ("assign", "change"):
        _walk(b["target"], scope, out, line, writing=True)
        if k == "change":  # `x += 1` reads x too
            _walk(b["target"], scope, out, line)
        _walk(b["value"], scope, out, line)
        return
    if k == "call" and b["func"]["k"] == "attr" and b["func"]["attr"] in ("set", "add", "remove"):
        comp = _component_arg(b)
        if comp is not None:
            out.append((comp, ALL, WRITE, line))
    for key, v in b.items():
        if key != "k":
            _walk(v, scope, out, line)


def accesses(source):
    """What each rule of a script touches, in source order:
    `[{"system", "phase", "component", "field", "mode", "line"}]`. `mode` is
    "read", "write" or "unknown"; `field` is a name or `"*"`. A rule that
    reads and writes a field has a row for each."""
    phases = {s["name"]: s["phase"] for s in B.systems_in(source)}
    rows = []
    for f in B.parse(source)["items"]:
        if f["k"] != "function":
            continue
        row = {"system": f["name"], "phase": phases.get(f["name"])}
        if f["locked"]:
            # Its decorators could not be read either, so it may not even be
            # a rule; say so rather than hide a possible writer.
            rows.append(dict(row, component=ALL, field=ALL, mode=UNKNOWN, line=f.get("line")))
            continue
        if not _is_rule(f):
            continue
        scope = dict(B.scope_of(f))
        found = []
        _walk(f["body"], scope, found, f.get("line"))
        seen = set()
        for comp, field, mode, line in found:
            if (comp, field, mode) not in seen:
                seen.add((comp, field, mode))
                rows.append(dict(row, component=comp, field=field, mode=mode, line=line))
    return rows


def table(sources):
    """The whole project's graph, from `{script name: source}`:
    `{component: {field: {"read": [rule], "write": [rule], "unknown": [rule]}}}`,
    where a rule is `{"system", "phase", "script", "line"}`. A script that is
    not valid Python right now is skipped (it is being typed)."""
    out = {}
    for script in sorted(sources):
        try:
            rows = accesses(sources[script])
        except ValueError:
            continue
        for r in rows:
            fields = out.setdefault(r["component"], {})
            cell = fields.setdefault(r["field"], {READ: [], WRITE: [], UNKNOWN: []})
            cell[r["mode"]].append({"system": r["system"], "phase": r["phase"], "script": script, "line": r["line"]})
    return out


def of_field(tab, component, field):
    """Who touches one field: `{"read": [...], "write": [...], "unknown":
    [...]}` of rule names, counting rules that touch the whole component and
    rules that could not be read. Names are in source order, once each."""
    out = {READ: [], WRITE: [], UNKNOWN: []}
    cells = [tab.get(component, {}).get(field), tab.get(component, {}).get(ALL), tab.get(ALL, {}).get(ALL)]
    for cell in cells:
        for mode in out:
            for rule in (cell or {}).get(mode, ()):
                if rule["system"] not in out[mode]:
                    out[mode].append(rule["system"])
    return out


def conflicts(tab):
    """Fields with more than one writer in the same phase (design doc 8.3):
    `[{"component", "field", "phase", "systems"}]`, sorted. Which one wins is
    decided by the order they run in, which nothing on screen says."""
    out = []
    for component in sorted(tab):
        if component == ALL:
            continue
        whole = tab[component].get(ALL, {}).get(WRITE, [])
        for field in sorted(tab[component]):
            if field == ALL:
                continue
            by_phase = {}
            for rule in tab[component][field][WRITE] + whole:
                names = by_phase.setdefault(rule["phase"], [])
                if rule["system"] not in names:
                    names.append(rule["system"])
            for phase in sorted(by_phase, key=lambda p: p or ""):
                if len(by_phase[phase]) > 1:
                    out.append({"component": component, "field": field, "phase": phase, "systems": by_phase[phase]})
    return out


def sentence(who):
    """One line for the inspector, from `of_field`: "Written by move. Read by
    draw, aim." Empty when nothing touches the field."""
    parts = []
    if who[WRITE]:
        parts.append("Written by " + ", ".join(who[WRITE]))
    if who[READ]:
        parts.append("Read by " + ", ".join(who[READ]))
    if who[UNKNOWN]:
        parts.append("Maybe used by " + ", ".join(who[UNKNOWN]) + " (locked)")
    return ". ".join(parts) + ("." if parts else "")
