"""Executable proposal semantics; independent of the batch-impl implementation.

Packs and choices remain structural until materialization. Declarations are
explicit carriers, never inferred from the fresh names remaining in a result.
"""
from dataclasses import dataclass
from itertools import product
from functools import wraps


class ModelError(ValueError):
    def __init__(self, code, message):
        self.code = code
        super().__init__(f"{code}: {message}")


def guarded(method):
    @wraps(method)
    def call(self, *args):
        self.depth += 1
        try:
            if self.depth > 64:
                raise ModelError("depth-limit", "semantic nesting exceeds the experiment limit")
            return method(self, *args)
        finally:
            self.depth -= 1
    return call


@dataclass(frozen=True)
class Node:
    kind: str
    name: str = ""
    children: tuple = ()
    params: tuple = ()


def atom(name, *args):
    return Node("atom", name, args)


def pack(*args):
    return Node("pack", children=args)


def choices(*args):
    return Node("choices", children=args)


def tup(*args):
    return Node("tuple", children=args)


def union(left, right):
    return tuple(dict.fromkeys((*left, *right)))


def carry(params, value):
    if value.kind == "decl":
        return carry(union(params, value.params), value.children[0])
    return Node("decl", children=(value,), params=tuple(params)) if params else value


def star(value):
    if value.kind == "decl":
        return carry(value.params, star(value.children[0]))
    if value.kind == "pack":
        return value
    # Only a candidate list is opened. A tuple, the unit type, a slice and an
    # array are *types*, so each contributes exactly one whole member: `*[A, B]`
    # is two members while `*(A, B)` is one member of the tuple type.
    if value.kind == "choices":
        return pack(*value.children)
    return pack(value)


@dataclass(frozen=True)
class Row:
    items: tuple
    params: tuple = ()


class Engine:
    def __init__(self, limit=1024, work_limit=100_000):
        self.group = 0
        self.limit = limit
        self.work_limit = work_limit
        self.work = 0
        self.depth = 0

    def tick(self, amount=1):
        self.work += amount
        if self.work > self.work_limit:
            raise ModelError("work-limit", "proposal interpreter work budget exceeded")

    def bounded(self, count):
        if count > self.limit:
            raise ModelError("expansion-limit", f"{count} exceeds {self.limit}")

    @guarded
    def flat_pack(self, value):
        """The numeric pack consumer flattens packs, not ordinary choices."""
        if value.kind == "decl":
            row = self.flat_pack(value.children[0])
            return Row(row.items, union(value.params, row.params))
        if value.kind != "pack":
            return Row((value,))
        items, params = (), ()
        for child in value.children:
            row = self.flat_pack(child)
            items += row.items
            params = union(params, row.params)
        self.bounded(len(items))
        return Row(items, params)

    def generate(self, value, count):
        self.bounded(count)
        if count < 0:
            raise ModelError("number", "negative arity")
        row = self.flat_pack(value) if value.kind == "pack" else Row(value.children)
        members = row.items
        make = pack if value.kind == "pack" else tup
        if not members:
            if count == 0:
                return carry(row.params, make())
            group = self.group
            self.group += 1
            names = tuple(f"G{group}P{i}" for i in range(count))
            body = make(*(Node("fresh", name) for name in names))
            return carry(union(row.params, names), body)
        if len(members) == 1:
            return carry(row.params, make(*(members * count)))
        size = 1
        for _ in range(count):
            size *= len(members)
            self.bounded(size)
        self.tick(size * max(count, 1))
        body = choices(*(make(*xs) for xs in product(members, repeat=count)))
        return carry(row.params, body)

    @guarded
    def map_task(self, left, right):
        """An existing row stays whole, even after a left choice selects a pack."""
        self.tick()
        if left.kind == "decl":
            return carry(left.params, self.map_task(left.children[0], right))
        if right.kind == "decl":
            return carry(right.params, self.map_task(left, right.children[0]))
        if right.kind == "choices":
            return choices(*(self.map_task(left, x) for x in right.children))
        if left.kind == "choices":
            return choices(*(self.map_task(x, right) for x in left.children))
        if left.kind == "pack":
            return pack(*(self.map_task(x, right) for x in left.children))
        return self.apply(left, right)

    @guarded
    def apply(self, left, right):
        self.tick()
        if left.kind == "decl":
            return carry(left.params, self.apply(left.children[0], right))
        if right.kind == "decl":
            return carry(right.params, self.apply(left, right.children[0]))
        if right.kind == "choices":
            return choices(*(self.apply(left, x) for x in right.children))
        if left.kind == "choices":
            return choices(*(self.apply(x, right) for x in left.children))
        if right.kind == "range":
            lo, hi = map(int, right.name.split(":"))
            if lo >= hi:
                raise ModelError("empty-range", "range must contain at least one arity")
            self.bounded(hi - lo)
            return choices(*(self.apply(left, Node("num", str(n))) for n in range(lo, hi)))
        if right.kind == "num" and left.kind in ("tuple", "pack"):
            return self.generate(left, int(right.name))
        if left.kind == "pack":
            if right.kind == "pack":
                return pack(*(self.map_task(left, row) for row in right.children))
            return self.map_task(left, right)
        if left == atom("self"):
            return right
        if left.kind in ("atom", "tuple"):
            self.bounded(len(left.children) + 1)
            return Node(left.kind, left.name, (*left.children, right))
        raise ModelError("invalid-head", f"{left.kind} is not a constructor in this model")

    def combine(self, rows):
        result = [Row(())]
        for variants in rows:
            self.bounded(len(result) * len(variants))
            self.tick(len(result) * len(variants))
            result = [Row(a.items + b.items, union(a.params, b.params))
                      for a in result for b in variants]
            for row in result:
                self.bounded(len(row.items))
        return result

    def one(self, value):
        rows = self.slots(value)
        if any(len(row.items) != 1 for row in rows):
            raise ModelError("single-slot", "this host requires exactly one type")
        return rows

    @guarded
    def slots(self, value):
        self.tick()
        if value.kind == "decl":
            return [Row(row.items, union(value.params, row.params))
                    for row in self.slots(value.children[0])]
        if value.kind == "choices":
            out = [row for x in value.children for row in self.slots(x)]
            self.bounded(len(out))
            return out
        if value.kind == "pack":
            return self.combine(self.slots(x) for x in value.children)
        if value.kind in ("ref", "raw", "slice", "array"):
            rows = self.one(value.children[0])
        elif value.kind == "fn":
            params = self.combine(self.slots(x) for x in value.children[:-1])
            returns = self.one(value.children[-1])
            self.bounded(len(params) * len(returns))
            rows = [Row(p.items + r.items, union(p.params, r.params)) for p in params for r in returns]
        else:
            rows = self.combine(self.slots(x) for x in value.children)
        return [Row((Node(value.kind, value.name, row.items),), row.params) for row in rows]

    def finish(self, value):
        rows = [Row((item,), row.params) for row in self.slots(value) for item in row.items]
        self.bounded(len(rows))
        return rows


def fresh_names(value):
    names = {value.name} if value.kind == "fresh" else set()
    for child in value.children:
        names.update(fresh_names(child))
    return names


def unused(row):
    used = set().union(*(fresh_names(x) for x in row.items))
    return tuple(name for name in row.params if name not in used)


def render(value):
    parts = [render(x) for x in value.children]
    if value.kind == "tuple":
        return "(" + ",".join(parts) + ("," if len(parts) == 1 else "") + ")"
    if value.kind in ("atom", "fresh", "num"):
        return value.name + ("<" + ",".join(parts) + ">" if parts else "")
    if value.kind == "slice":
        return f"[{parts[0]}]"
    if value.kind == "array":
        return f"[{parts[0]};{value.name}]"
    if value.kind == "ref":
        return "&" + value.name + parts[0]
    if value.kind == "raw":
        return "*" + value.name + " " + parts[0]
    if value.kind == "fn":
        suffix = "" if parts[-1] == "()" else "->" + parts[-1]
        return "fn(" + ",".join(parts[:-1]) + ")" + suffix
    raise ModelError("unmaterialized", f"cannot render {value.kind} as Rust")
