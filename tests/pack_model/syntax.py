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
        if self.depth > 64:
            raise ModelError("depth-limit", "expression nesting exceeds 64")

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
            return atom(path, *args)
        return atom(path)

    def parse(self):
        value = self.expression()
        if self.peek() is not None:
            raise ModelError("trailing-token", f"unexpected {self.peek()}")
        return value

    def run(self):
        return self.engine.finish(self.parse())


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
