"""Regression tests and independent family-shape/declaration checks."""
from itertools import product
import unittest
from examples import CASES, readable
from semantics import Engine, ModelError, Node, atom, carry, choices, fresh_names, pack, render, star, tup, unused
from syntax import Parser, evaluate, outputs


class ModelTests(unittest.TestCase):
    def test_teaching_examples(self):
        for name, (source, expected) in CASES.items():
            with self.subTest(name=name):
                rows = evaluate(source)
                self.assertEqual([readable(row) for row in rows], expected)
                self.assertTrue(all(not unused(row) for row in rows))

    def test_parenthesized_space_and_dot_chain_agree(self):
        for n in range(5):
            self.assertEqual(evaluate(f"(*Pair (*[self, Vec] *[].{n}),)"),
                             evaluate(f"(*Pair.*[self, Vec].*[].{n},)"))
            self.assertEqual(evaluate(f"(*() (*[self, Vec] *[].{n}),)"),
                             evaluate(f"(*().*[self, Vec].*[].{n},)"))

    def test_generated_families(self):
        for n in range(7):
            for mode in ("vec", "pair", "tuple"):
                head = {"vec": "*Vec", "pair": "*Pair.*[self, Vec]", "tuple": "*().*[self, Vec]"}[mode]
                source = f"({head}.*[].{n},)"
                row, = evaluate(source)
                names = [f"G0P{i}" for i in range(n)]
                members = [{"vec": f"Vec<{t}>", "pair": f"Pair<{t},Vec<{t}>>", "tuple": f"({t},Vec<{t}>)"}[mode]
                           for t in names]
                expected = "(" + ",".join(members) + ("," if n == 1 else "") + ")"
                self.assertEqual(render(row.items[0]), expected)
                self.assertEqual(row.params, tuple(names))

    def test_range_scopes(self):
        for lo in range(4):
            for hi in range(lo, 6):
                rows = evaluate(f"(*Pair.*[self, Vec].*[].{lo}..={hi},)")
                self.assertEqual(len(rows), hi - lo + 1)
                for n, row in zip(range(lo, hi + 1), rows):
                    self.assertEqual(len(row.params), n)
                    self.assertEqual(len(row.items[0].children), n)
                    self.assertEqual(set(row.params), fresh_names(row.items[0]))
                nonempty_groups = [row.params[0].split("P")[0] for row in rows if row.params]
                self.assertEqual(len(nonempty_groups), len(set(nonempty_groups)))

    def test_multiple_axes(self):
        for dims in (2, 3):
            for lengths in product(range(4), repeat=dims):
                source = "(*Map " + " ".join(f"*[].{n}" for n in lengths) + ",)"
                row, = evaluate(source)
                groups, group = [], 0
                for n in lengths:
                    groups.append([f"G{group}P{i}" for i in range(n)])
                    group += n > 0
                indices = product(*reversed(groups))
                expected = ["Map<" + ",".join(reversed(xs)) + ">" for xs in indices]
                self.assertEqual([render(x) for x in row.items[0].children], expected)
                self.assertEqual(row.params, tuple(x for group in groups for x in group))
                self.assertEqual(len(unused(row)), sum(lengths) if 0 in lengths else 0)

    def test_zero_does_not_allocate_identity(self):
        row, = evaluate("(*Map *[].0 *[].2,)")
        self.assertEqual(row.params, ("G0P0", "G0P1"))
        self.assertEqual(render(row.items[0]), "()")

    def test_copy_keeps_fresh_identity(self):
        # Two direct tuple members are candidates for a Cartesian power.
        rows = evaluate("(().2).2")
        self.assertEqual([readable(x) for x in rows], ["(T0,T0)", "(T0,T1)", "(T1,T0)", "(T1,T1)"])
        self.assertTrue(all(x.params == ("G0P0", "G0P1") for x in rows))
        row, = evaluate("((().2),).2")
        self.assertEqual(readable(row), "((T0,T1),(T0,T1))")

    def test_map_task_preserves_row_after_left_choice(self):
        source = "*[[*F,*G],] *[*[A, B],]"
        self.assertEqual(outputs(source), ["F<A,B>", "G<A,B>"])
        self.assertEqual(outputs("*[[*[F, G],*H],] *[*[A, B],]"), ["F<A,B>", "G<A,B>", "H<A,B>"])

    def test_regular_tuple_power_does_not_splice(self):
        self.assertEqual(outputs("(*[A, B],).2"), ["(A,B,A,B)"])
        row, = evaluate("(*[],).2")
        self.assertEqual(render(row.items[0]), "()")
        self.assertEqual(row.params, ())
        row, = evaluate("(*[*[],].2,)")
        self.assertEqual(readable(row), "(T0,T1)")

    def test_candidate_scope(self):
        self.assertEqual(outputs("([A,B],).2"), ["(A,A)", "(A,B)", "(B,A)", "(B,B)"])
        self.assertEqual(outputs("(self ([A,B],)).2"), outputs("([A,B],).2"))
        self.assertEqual(outputs("(*[self, Vec] ([A,B],),)"), [
            "((A,),Vec<(A,)>)", "((A,),Vec<(B,)>)", "((B,),Vec<(A,)>)", "((B,),Vec<(B,)>)"])
        self.assertEqual(outputs("(*[self, Vec] [(A,),(B,)],)"), ["((A,),Vec<(A,)>)", "((B,),Vec<(B,)>)"])

    def test_structure_is_not_final_type_equivalence(self):
        self.assertEqual(outputs("(A,B)"), outputs("(*[A, B],)"))
        self.assertEqual(outputs("*F *[A, B]"), ["F<A>", "F<B>"])
        self.assertEqual(outputs("*F *[*[A, B],]"), ["F<A,B>"])
        self.assertEqual(outputs("*F *[]"), [])
        self.assertEqual(outputs("*F *[*[],]"), ["F"])

    def test_direct_hosts_are_not_reapplied(self):
        self.assertEqual(outputs("F<G<T>>"), ["F<G<T>>"])
        self.assertEqual(outputs("F<*[A, B],*[C, D]>"), ["F<A,B,C,D>"])
        self.assertEqual(outputs("*F *[*[A, B], *[C, D]]"), ["F<A,B>", "F<C,D>"])
        self.assertEqual(outputs("F *[(A,B), (C,D)]"), ["F<(A,B),(C,D)>"])

    def test_single_slot_and_callable_hosts(self):
        self.assertEqual(outputs("&*[A,]"), ["&A"])
        self.assertEqual(outputs("*const *[u8,]"), ["*const u8"])
        self.assertEqual(outputs("**const u8"), ["*const u8"])
        self.assertEqual(outputs("[*[u8,];4]"), ["[u8;4]"])
        self.assertEqual(outputs("fn(*[u8, u16])->*[u32,]"), ["fn(u8,u16)->u32"])
        self.assertEqual(outputs("fn(*[])->()"), ["fn()"])
        for source in ("&*[A, B]", "&*[]", "*mut *[A, B]", "[*[A, B]]", "[*[A, B];4]", "fn()->*[A, B]"):
            with self.subTest(source=source), self.assertRaises(ModelError) as error:
                evaluate(source)
            self.assertEqual(error.exception.code, "single-slot")

    def test_rejections(self):
        cases = {"*Vec ? A": "unsupported-syntax", "A #name": "unsupported-syntax",
                 "*": "missing-operand", "() .": "missing-operand", "(A,": "missing-operand",
                 "A)": "trailing-token", "*[].2..2": "empty-range", "*[].3..=2": "empty-range",
                 "*[].1000000000": "expansion-limit", "(A,B).30": "expansion-limit",
                 "*[].0..=1000000000": "expansion-limit", "*" * 300 + "A": "depth-limit",
                  # a differential probe found the model dropping this one silently: an
                  # empty pack contributes no type to an argument list, which the macro
                  # reports as a zero-impl spec.
                  "Vec<u8, *[], u16>": "empty-pack-argument",
                  # the same probe found the sibling spelling: an empty *candidate list*
                  # vanishes the same way, so `Vec<[]>` must not come back empty either.
                  "Vec<[]>": "empty-pack-argument",
                  "self": "bare-self",
                 "fn()->": "missing-operand", "A::": "missing-operand"}
        cases.update({"dyn Send": "unsupported-syntax", "unsafe fn()": "unsupported-syntax"})
        for source, code in cases.items():
            with self.subTest(source=source), self.assertRaises(ModelError) as error:
                evaluate(source)
            self.assertEqual(error.exception.code, code)
        # `[]` is the empty candidate list: starred it is the empty pack, which yields
        # no targets. (The production macro diagnoses that spec; the model describes
        # structure only, so the divergence is deliberate.)
        self.assertEqual(outputs("*[]"), [])

    def test_no_deduplication_or_declaration_pruning(self):
        self.assertEqual(outputs("[A,A]"), ["A", "A"])
        rows = evaluate("*[].2")
        self.assertEqual(len(rows), 2)
        self.assertTrue(all(len(row.params) == 2 and len(unused(row)) == 1 for row in rows))
        row, = evaluate("(*Map *[].2 *[].0,)")
        self.assertEqual(row.params, ("G0P0", "G0P1"))
        self.assertEqual(unused(row), row.params)

    def test_discarded_template_is_not_global_declaration(self):
        row, = evaluate("(*(().2),).0")
        self.assertEqual(render(row.items[0]), "()")
        self.assertEqual(row.params, ())

    def test_long_application_chain_has_a_semantic_depth_guard(self):
        # 200, not 100: the guard sits at the macro's own limit (128), so a chain that
        # overruns it has to be longer than that to exercise it at all.
        source = "(*F " + " ".join("*[A,]" for _ in range(200)) + ",)"
        with self.assertRaises(ModelError) as error:
            evaluate(source)
        self.assertEqual(error.exception.code, "depth-limit")


if __name__ == "__main__":
    unittest.main(verbosity=2)
