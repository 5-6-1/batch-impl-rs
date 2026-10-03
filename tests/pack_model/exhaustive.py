"""Independent bounded properties for the Pack v2 proposal, not full Rust.

The core corpus is every Pack/Tuple/nonempty-List tree through three nodes
over A, B, self, (), and *[]. Source spelling is checked independently from
application. No production source or sibling proposal file is changed.
"""

from collections import Counter
from hashlib import sha256
from itertools import product
import json
from paths import MODEL_ROOT, output_path

from semantics import (
    Engine, ModelError, Node, Row, atom, carry, choices, fresh_names,
    pack, render, star, tup, unused,
)
from syntax import Parser


def spell(value):
    """Exact source form for the generated nonnumeric structural corpus."""
    children = [spell(child) for child in value.children]
    if value.kind == "pack":
        return "*[" + ", ".join(children) + ("," if children else "") + "]"
    if value.kind == "tuple":
        return "(" + ",".join(children) + ("," if children else "") + ")"
    if value.kind == "choices":
        assert children, "[] is not an empty candidate list"
        return "[" + ",".join(children) + ",]"
    if value.kind == "atom":
        return value.name + ("<" + ",".join(children) + ">" if children else "")
    if value.kind == "ref":
        return "&" + value.name + "(" + children[0] + ")"
    if value.kind == "raw":
        return "*" + value.name + " (" + children[0] + ")"
    if value.kind == "slice":
        return "[" + children[0] + "]"
    if value.kind == "array":
        return "[" + children[0] + ";" + value.name + "]"
    if value.kind == "fn":
        return "fn(" + ",".join(children[:-1]) + ")->(" + children[-1] + ")"
    raise AssertionError(value.kind)


def bounded_values():
    by_size = {1: {atom("A"), atom("B"), atom("self"), pack(), tup()}}
    wrappers = (pack, tup, choices)
    for size in (2, 3):
        values = set()
        for child in by_size[size - 1]:
            values.update(wrapper(child) for wrapper in wrappers)
        for left_size in range(1, size - 1):
            right_size = size - 1 - left_size
            for left, right in product(by_size[left_size], by_size[right_size]):
                values.update(wrapper(left, right) for wrapper in wrappers)
        by_size[size] = values
    return sorted(set().union(*by_size.values()), key=lambda value: (len(spell(value)), spell(value)))


def walk(value):
    yield value
    for child in value.children:
        yield from walk(child)


def complete(rows, declarations=()):
    allowed = set(declarations)
    for row in rows:
        assert set(row.params) <= allowed, (row, allowed)
        assert len(row.params) == len(set(row.params)), row
        for value in row.items:
            assert not any(node.kind in {"pack", "choices", "decl", "range"}
                           for node in walk(value)), value
            assert fresh_names(value) <= set(row.params), row
            render(value)


def materialized_tuple(value):
    return tuple(Engine().finish(tup(value)))


def parse(source):
    parser = Parser(source)
    return parser.parse(), parser.engine


def rendered(source):
    value, engine = parse(source)
    rows = engine.finish(value)
    return [render(row.items[0]) for row in rows], rows, engine


def parameters(value):
    return set().union(*(set(node.params) for node in walk(value) if node.kind == "decl"))


counts = Counter()
values = bounded_values()
assert len(values) == 140

for value in values:
    assert star(star(value)) == star(value), value
    assert Engine().apply(atom("self"), value) == value, value
    assert Parser(spell(value)).parse() == value, spell(value)
    rows = Engine().finish(value)
    complete(rows)
    again = [item for row in rows for value in row.items for item in Engine().finish(value)]
    assert again == rows
    counts.update({"star_idempotence": 1, "structural_identity": 1,
                   "source_roundtrip": 1, "materialization_idempotence": 1})

for left, right in product(values, repeat=2):
    engine = Engine()
    result = engine.apply(left, right)
    rows = engine.finish(result)
    complete(rows)
    assert engine.group == 0
    counts["finite_type_apply_no_allocation"] += 1

    # Deliberately unused declarations must survive. Inferring parameters
    # from names present in the final types would fail this property.
    carried_engine = Engine()
    carried = carried_engine.apply(carry(("LeftP",), left), carry(("RightP",), right))
    carried_rows = carried_engine.finish(carried)
    assert carried_rows == [Row(row.items, ("LeftP", "RightP")) for row in rows]
    complete(carried_rows, ("LeftP", "RightP"))
    assert carried_engine.group == 0
    counts["explicit_declaration_provenance"] += 1

constructors = (atom("F"), atom("G"), pack(atom("F")), pack(),
                pack(pack(atom("F"))), tup(), pack(tup()))
inputs = (atom("A"), pack(), pack(atom("A")), pack(atom("A"), atom("B")),
          pack(pack(atom("A"), atom("B"))), tup(atom("A"), atom("B")))
for first, second, row in product(constructors, constructors, inputs):
    result = Engine().map_task(pack(choices(first, second)), row)
    expected = tuple(
        output for selected in (first, second)
        for output in materialized_tuple(Engine().map_task(pack(selected), row))
    )
    assert materialized_tuple(result) == expected, (first, second, row)
    counts["left_choice_keeps_current_map_task"] += 1

for left, first, second in product(constructors, inputs, inputs):
    result = Engine().map_task(pack(left), choices(first, second))
    expected = tuple(
        output for selected in (first, second)
        for output in materialized_tuple(Engine().map_task(pack(left), selected))
    )
    assert materialized_tuple(result) == expected, (left, first, second)
    counts["exposed_right_choice_is_shared"] += 1

for dimensions in (2, 3):
    for lengths in product(range(4), repeat=dimensions):
        engine = Engine()
        axes = [engine.generate(pack(), count) for count in lengths]
        result = pack(atom("Map"))
        for axis in axes:
            result = engine.apply(result, axis)
        actual = engine.finish(tup(result))
        declared = parameters(result)
        complete(actual, declared)
        axis_members = [engine.flat_pack(axis).items for axis in axes]
        expected_types = tup(*(atom("Map", *reversed(args))
                               for args in product(*reversed(axis_members))))
        assert [row.items for row in actual] == [(expected_types,)], lengths
        assert len(actual[0].params) == sum(lengths), lengths
        assert len(fresh_names(expected_types)) == (sum(lengths) if all(lengths) else 0)
        assert engine.group == sum(length > 0 for length in lengths)
        counts["axis_order_identity_and_unused_carriers"] += 1

for value in (atom("A"), tup(atom("A"), atom("B")), pack(atom("A"), atom("B")),
              choices(atom("A"), atom("B")), atom("F", pack(atom("A"), atom("B")))):
    hosts = (atom("F", value), Node("ref", "", (value,)), Node("ref", "mut ", (value,)),
             Node("raw", "const", (value,)), Node("slice", children=(value,)),
             Node("array", "2", (value,)), Node("fn", children=(value, tup())))
    for host in hosts:
        assert Parser(spell(host)).parse() == host, spell(host)
        counts["typed_host_source_roundtrip"] += 1

examples = {
    "(*[[*F,*G],] *[*[A, B],],)": ["(F<A,B>,)", "(G<A,B>,)"],
    "(*Vec [*[A,],*[B, C]],)": ["(Vec<A>,)", "(Vec<B>,Vec<C>)"],
    "(*F *[],)": ["()"],
    "(*F *[*[],],)": ["(F,)"],
    "(*[A, B],).2": ["(A,B,A,B)"],
    "(*[*[A, B],].2,)": ["(A,A)", "(A,B)", "(B,A)", "(B,B)"],
    "(self ([A,B],)).2": ["(A,A)", "(A,B)", "(B,A)", "(B,B)"],
    "Vec<*[A, B]>": ["Vec<A,B>"],
    "*Vec<*[A, B]>": ["Vec<A,B>"],
    "A::B": ["A::B"],
}
for source, expected in examples.items():
    actual, rows, _ = rendered(source)
    assert actual == expected, (source, actual, expected)
    complete(rows)
    counts["targeted_structure_and_lexer_regressions"] += 1

for source in ("&*[A, B]", "*const *[A, B]", "[*[A, B]]", "[*[A, B];2]", "fn()->*[A, B]"):
    try:
        rendered(source)
    except ModelError as error:
        assert error.code == "single-slot", (source, error.code)
    else:
        raise AssertionError(("expected single-slot error", source))
    counts["single_slot_rejects_many"] += 1

for source in ("*Vec ? A", "A+B", "T:Trait", "@Self", "A=>B"):
    try:
        Parser(source).parse()
    except ModelError:
        pass
    else:
        raise AssertionError(("unexpected unsupported syntax acceptance", source))
    counts["unsupported_syntax_rejected"] += 1

for source, wanted_groups, wanted_names in (
    ("(Vec.*[].1,).3", 1, 1),
    ("(*Pair.*[self, Vec].*[].3,)", 1, 3),
    ("(*Map *[].2 *[].0,)", 1, 2),
    ("(*Map *[].0 *[].2,)", 1, 2),
):
    value, engine = parse(source)
    declared = parameters(value)
    rows = engine.finish(value)
    complete(rows, declared)
    assert engine.group == wanted_groups
    assert len(declared) == wanted_names
    if "*[].0" in source:
        assert len(unused(rows[0])) == wanted_names
    counts["fresh_copy_and_zero_axis_diagnostics"] += 1

value, engine = parse("(*[].0..=2,)")
rows = engine.finish(value)
complete(rows, parameters(value))
assert [row.params for row in rows] == [(), ("G0P0",), ("G1P0", "G1P1")]
assert engine.group == 2
counts["zero_range_branch_uses_no_group"] += 1

report = {
    "scope": "Finite structural Pack proposal audit; not a proof of full DSL or Rust correctness",
    "model_hashes": {name: sha256((MODEL_ROOT / name).read_bytes()).hexdigest()
                     for name in ("semantics.py", "syntax.py")},
    "structural_values": len(values), "checks": dict(counts),
    "checks_total": sum(counts.values()), "new_failures": [],
}
output_path("exhaustive_results.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report, indent=2))
