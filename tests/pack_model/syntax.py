"""Strict, bounded parser for the documented proposal subset, not full Rust.

No unsupported character may disappear. Generic argument lists build an ordinary
type host; their contents are never reapplied to a head during materialization.
"""
import re
from semantics import Engine, ModelError, Node, atom, choices, pack, render, star, tup

TOKEN = re.compile(r"\s+|\d+\.\.(?:=)?\d+|::|->|[A-Za-z_][A-Za-z_0-9]*|\d+|[\[\](),.*<>;&]")


class Parser:
    def __init__(self, source, engine=None):
        self.engine = engine or Engine()
        if len(source) > 100_000:
            raise ModelError("input-limit", "source is too long for this experiment")
        self.tokens = []
        pos = 0
        while pos < len(source):
            match = TOKEN.match(source, pos)
            if match is None:
                raise ModelError("unsupported-syntax", f"unrecognized input at offset {pos}")
            token = match.group()
            if any(len(n) > 20 for n in re.findall(r"\d+", token)):
                raise ModelError("number-limit", "numeric literal exceeds the experiment's integer width")
            if not token.isspace():
                self.tokens.append(token)
            pos = match.end()
        self.index = 0
        self.depth = 0

    def peek(self):
        return self.tokens[self.index] if self.index < len(self.tokens) else None

    def take(self):
        token = self.peek()
        if token is None:
            raise ModelError("missing-operand", "unexpected end of expression")
        self.index += 1
        return token

    def require(self, expected):
        actual = self.take()
        if actual != expected:
            raise ModelError("delimiter", f"expected {expected}, got {actual}")

    def enter(self):
        self.depth += 1
        # Two call sites bump this counter per level (dotted() and primary()),
        # so the guard is four times the depth the macro allows
        # (MAX_NEST_DEPTH = 128): 256 here accepts 128 levels, as measured.
        if self.depth > 256:
            raise ModelError("depth-limit", "expression nesting exceeds 128")

    def expression(self):
        value = self.dotted()
        while self.peek() not in (None, ",", ")", "]", ">", ";", "->"):
            value = self.engine.apply(value, self.dotted())
        return value

    def dotted(self):
        self.enter()
        try:
            value = self.primary()
            if self.peek() == ".":
                self.take()
                value = self.engine.apply(value, self.dotted())
            return value
        finally:
            self.depth -= 1

    def sequence(self, close):
        values, comma = [], False
        while self.peek() != close:
            values.append(self.expression())
            if self.peek() != ",":
                break
            comma = True
            self.take()
        self.require(close)
        return values, comma

    def primary(self):
        self.enter()
        try:
            return self.primary_inner()
        finally:
            self.depth -= 1

    def primary_inner(self):
        token = self.take()
        if token == "*":
            if self.peek() in ("const", "mut"):
                return Node("raw", self.take(), (self.primary(),))
            return star(self.primary())
        if token == "&":
            qualifier = ""
            if self.peek() == "mut":
                qualifier = self.take() + " "
            return Node("ref", qualifier, (self.primary(),))
        if token == "(":
            values, comma = self.sequence(")")
            return values[0] if len(values) == 1 and not comma else tup(*values)
        if token == "[":
            if self.peek() == "]":
                # `[]` is the empty candidate list; starred it is the empty pack
                # (`*[]`) and sized it is the generator (`*[].N`).
                self.take()
                return choices()
            first = self.expression()
            if self.peek() == ";":
                self.take()
                length = self.take()
                if not re.fullmatch(r"\d+|[A-Za-z_][A-Za-z_0-9]*", length):
                    raise ModelError("unsupported-syntax", "array length must be one atom in this subset")
                self.require("]")
                return Node("array", length, (first,))
            if self.peek() == "]":
                self.take()
                return Node("slice", children=(first,))
            self.require(",")
            rest, _ = self.sequence("]")
            return choices(first, *rest)
        if ".." in token:
            lo, hi = token.split("..")
            end = int(hi.lstrip("=")) + hi.startswith("=")
            return Node("range", f"{int(lo)}:{end}")
        if token.isdigit():
            return Node("num", token)
        if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", token):
            raise ModelError("unexpected-token", f"unexpected {token}")
        if token in ("dyn", "for", "impl", "unsafe", "const", "mut", "where", "async", "extern"):
            raise ModelError("unsupported-syntax", f"{token} syntax is outside this parser subset")
        if token == "fn":
            self.require("(")
            params, _ = self.sequence(")")
            ret = tup()
            if self.peek() == "->":
                self.take()
                ret = self.dotted()
            return Node("fn", children=(*params, ret))
        path = token
        while self.peek() == "::":
            self.take()
            segment = self.take()
            if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", segment):
                raise ModelError("unsupported-syntax", "expected a simple path segment")
            path += "::" + segment
        if self.peek() == "<":
            self.take()
            args, _ = self.sequence(">")
            # An argument list needs at least one type; a pack among the arguments
            # that expands to none is reported here. A pack inside a tuple or a
            # list goes through tup()/choices() and keeps vanishing (probe rows
            # mid2/mid3 agree with the macro), so this walks through packs and
            # candidate lists only and never into a tuple or a list. An empty
            # candidate list vanishes exactly like an empty pack does, and the macro
            # reports both (`Vec<[]>` and `Vec<*[]>`); the model used to report only
            # the pack, so `Vec<[]>` came back empty while its sibling errored. It
            # also used to look only at the argument itself, which missed the one
            # step of nesting a differential probe measured: `Vec<*[*[],]>`,
            # `Vec<[*[],]>`, `Vec<[*[], A]>` and `(*Vec *[*[],],)` all report on the
            # macro side, and the model rendered a bare `Vec` that does not compile.
            def empty_pack(node):
                if node.kind in ("pack", "choices"):
                    return not node.children or any(empty_pack(c) for c in node.children)
                return False

            if any(empty_pack(a) for a in args):
                raise ModelError(
                    "empty-pack-argument",
                    "this argument list requires at least one type, but the pack "
                    "expands to none",
                )
            return atom(path, *args)
        return atom(path)

    def parse(self):
        value = self.expression()
        if self.peek() is not None:
            raise ModelError("trailing-token", f"unexpected {self.peek()}")
        return value

    def run(self):
        rows = list(self.engine.finish(self.parse()))
        # `self` is the whole right operand (`self.T` applies T to it), so it may be an
        # operand but never a result - the macro reports it at the spec level, and the
        # model has to agree here rather than in the CLI: the CLI, the exhaustive audit
        # and any differential all read the model through this entry point.
        #
        # Every materialized position, not the rendered top level: comparing
        # `render(row.items[0])` caught the whole result `self` and nothing else, so one
        # level of nesting escaped - probe E's D1 measured ten spellings (`F<self>`,
        # `(self,)`, `[self]`, `*F self`, `(*[self,],)`, `F.self`, …) that the macro
        # rejects and the model happily rendered into types that cannot compile.
        for row in rows:
            for node in row.items:
                if _mentions_bare_self(node):
                    raise ModelError(
                        "bare-self",
                        "`self` is the whole right operand (`self.T` applies `T` to it), "
                        "not a type on its own",
                    )
        return rows


def _mentions_bare_self(node):
    """Whether this node or any node under it renders as the bare operand `self`."""
    if render(node).strip() == "self":
        return True
    return any(_mentions_bare_self(child) for child in getattr(node, "children", ()))


def evaluate(source):
    return Parser(source).run()


def outputs(source):
    return [render(row.items[0]) for row in evaluate(source)]


if __name__ == "__main__":
    import argparse
    from semantics import unused
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("expression")
    args = cli.parse_args()
    for result in evaluate(args.expression):
        decl = "<" + ",".join(result.params) + "> " if result.params else ""
        print(decl + render(result.items[0]))
        if unused(result):
            print("  unused generated parameters (not removed): " + ",".join(unused(result)))
