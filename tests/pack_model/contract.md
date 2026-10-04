# Pack v2: semantic contract and audit limits

English | [简体中文](contract.zh-CN.md)

Status: integrated into the unreleased 0.10.0 development version; this directory
remains an independent semantic model. Start with the
[tutorial](tutorial.md). This document defines rules precisely and helps find
counterexamples; it is not prerequisite reading for beginners.

## 1. Structures and representations

Ordinary Ty keeps its existing meaning, including ordinary types, tuples and
candidate lists. The new unified structure is `Pack(items)`, holding direct members
without remembering a tuple/list origin. `Choice(items)` represents candidates;
`Decl(params,body)` models the existing fresh-declaration carrier. These names are
internal notation, not user syntax.

`*X` crosses declaration carriers to process their bodies. If X is a Pack, return
that same Pack; for an ordinary tuple or candidate list, take its direct members;
otherwise retain the complete type as one member. It does not recursively open
ordinary types. Parse raw pointers `*const`/`*mut` before this rule.

For the ordinary Ty and pack expressions covered here, commas distinguish one-item
containers: `(X)` is transparent, `(X,)` a tuple, `[X]` a slice and `[X,]` a candidate.
Version 0.10.0 intentionally removes the lone-splat promotion exception. Existing
positional-reference meta-syntax such as `(@0..)` has its own grouping contract;
this model neither implements nor redefines it, and does not remove those spellings.
Empty `[]` remains the existing slice-constructor prefix, not an empty candidate;
the independent parser has not implemented that prefix.

`Pack(Pack(A,B),Pack(C,D))` and `Pack(A,B,C,D)` may splice to the same final members,
but behave differently under further application. Equal final Rust types do not
establish DSL equivalence in arbitrary contexts.

## 2. Two recursive operations

`Apply(L,R)` is ordinary application. `MapTask(L,row)` preserves a complete input
row during that mapping task; it is neither new syntax nor an observable variable.

Apply proceeds in this order:

1. Propagate declaration carriers on both sides, left declarations first, merging repeated fresh identities.
2. Dispatch exposed right candidates, then exposed left candidates; continue with Apply in each branch. Reordering can change fresh-generation counts.
3. For a right range, execute Apply for each number; reject an empty range.
4. An ordinary tuple or Pack receiving a number follows section 3.
5. For a left Pack and right Pack, enumerate only the right Pack's direct rows, run MapTask for each, and retain the results in a new outer pack. Otherwise run MapTask with the complete right value.
6. Other cases use that ordinary Ty's existing apply. `self` returns the complete right value; ordinary constructors append it as an argument or slot. A right pack waits within that slot for collection, without re-entering outer mapping.

MapTask proceeds in this order:

1. Propagate declaration carriers as above.
2. Dispatch exposed right/left candidates in that order; each branch **continues MapTask**, not Apply, which would split the row again.
3. For a left Pack, continue MapTask on its direct members, preserving the left pack structure.
4. For an ordinary left leaf, call ordinary Apply with the row as a complete input.

The v1 correction matters: step 2 originally omitted left candidates, allowing
`*[[*F,*G],] *[*[A, B],]` to split its row again. V2 produces two candidates,
`F<A,B>` and `G<A,B>`, without adding another kind of marker.

## 3. Generation and copying

Ordinary tuple `.N` retains the existing direct-slot rule: an empty tuple generates
N fresh parameters, a one-slot tuple copies that slot N times, and multiple slots
produce N-way Cartesian selections. **Do not first splice Packs or candidates inside slots.**

Pack `.N` is an explicit pack consumer. First splice nested Pack layers and their
declaration carriers, retaining ordinary candidates. Then apply the same empty,
single-member and multi-member rules, with each output still a Pack. Whether the
pack is empty is determined by this consumed member list.

Consequently:

```text
(*[A, B],).2  -> one (A,B,A,B) target
(*[],).2     -> (), with no fresh parameters
*[*[],].2    -> power of an empty pack, generating two fresh parameters
```

Ranges produce candidate families with declarations belonging to their generation
branches. `.0` allocates no fresh group ID. Copying generated parameters copies
references to the same identities; only executing a generation rule allocates identities.

`self E` is an identity on completed structural values. It does not promise algebraic
commutation with pending numeric/range operands, which are not ordinary Rust types
that can be placed arbitrarily.

## 4. Materialization and hosts

Structural expressions become ordinary types at the end:

- Choice retains candidate branches; choices in sibling slots form Cartesian combinations.
- Pack recursively splices pack layers and merges declarations, without opening ordinary tuples or types into outer members.
- Ordinary tuple fields, generic arguments and function parameters accept multiple materialized slots.
- Reference targets, pointer targets, slice/array elements and function returns require exactly one type in each branch; zero or multiple types produce a single-slot diagnostic.
- A bare target pack yields one target per member, without automatic deduplication.

Direct `F<...>` constructs an argument host. Materializing `F<*[A, B],*[C, D]>`
produces `F<A,B,C,D>`; do not replay these received arguments as a new apply chain.
This closes the previously identified stage-dependent-equivalence loophole.

An implementation may delay physical splicing, but must not remap across those
ordinary-type boundaries. Raw intermediate structures and materialized Rust types
are not freely interchangeable expressions.

Keep `Pack()` distinct from `Pack(Pack())`. Given to `*F`, the former produces zero
entries while the latter produces one F with no arguments.

That row cannot supply an *argument*, though: an argument slot needs at least one
member to splice, so `*F *[*[],]` — one entry whose argument list is empty — is an
error, while `*F *[]` stays legal because it produces no entry at all. (Round-7 probe E
measured the macro rejecting the former and this model accepting it; the macro is the
consistent reading, since an empty pack yields no argument for the host to take.)

**Known divergence.** This model does not implement that refusal yet: it renders `F` for
`*F *[*[],]`, a reading pinned by `test_model.py` and relied on by the finite corpus
(`exhaustive.py`). Two attempts to add the check locally failed against those dependants,
which is what makes the alignment a corpus-wide change rather than a local one. Until that
change is made deliberately, the rule above describes the **macro**, and the model is known
to be behind it on this one shape.

## 5. Candidate and declaration scopes

Only dispatch candidates exposed at the current operation; do not preselect choices
inside ordinary types globally. Thus `([A,B],).2` keeps four combinations, and copied
hidden candidates may expand independently. To select one complete input, expose
the candidate at that application, for example `[(A,),(B,)]`.

MapTask dispatches its currently exposed input candidate before passing the same
selected input to each constructor. A left candidate selecting a Pack must not
cause the current row to be split again.

Fresh declarations are not reconstructed from the names surviving in the final
types. Every Decl carrier entering the output is preserved completely; repeated
references to one identity declare it only once.

Two cases intentionally differ:

- `(*Map *[].2 *[].0,)` retains two carried declarations for target `()`; Rust reports E0207.
- `(*(().2),).0` repeats the entire ordinary-tuple template slot zero times. Its declaration carrier does not enter the result, leaving `()` with no declarations.

This follows which declaration carriers enter the result structurally; it is not
unused-parameter pruning. Complete checks for explicit parameters, constraints and
associated-type projections still belong to the existing implementation/Rust.
The model does not claim to solve E0207 in general.

## 6. Confirmed capabilities and boundaries

Covered: per-position wrappers; repeated uses of one fresh within a position;
per-position tuples or Pair types; length families; uniform and per-position
constructor choices; two/three-axis Cartesian combinations; and collecting the
same pack results as flat members or ordinary row tuples.

Not introduced: zip, indexing, lambdas, arbitrary templates with holes, bindings,
or automatic cross-slot constraints. `Vec.Box` is an already constructed type,
not a function awaiting T to produce `Vec<Box<T>>`.

The six row-preserving Map matrix impls for two positive length ranges compile;
their flat counterparts produce E0119. This demonstrates the value of retaining
rows, not a coherence guarantee for arbitrary combinations.

Declaration blocks accept only valid parameter declarations. A pack does not turn
constructed types into parameter names. Where clauses, impl templates, bodies and
directive arguments retain their existing syntax domains. Pack support does not
extend `*` into those domains or claim implementation validation for every
declaration/constraint carrier.

## 7. Model validation and production boundaries

The model uses only the Python standard library, independently of the production
macro. Run `python tests/pack_model/run.py`; all generated files go under
`target/pack-model/`. The parser's supported source forms are visible in `syntax.py`;
unsupported characters are rejected explicitly.

It covers ordinary names/paths, direct generic arguments, tuples/candidates, star,
space/dot, numbers/ranges, slices/fixed arrays, references/raw pointers and function
type hosts. Some existing prefix applications, such as `fn.(A,B)`, are not recreated
in the independent parser. Rejection by the model does not remove them from the DSL.

Audit layers:

1. Teaching/regression cases with handwritten expected results, plus independent generation, zero-axis and declaration assertions.
2. Finite small-structure enumeration: identity, star idempotence, source round-tripping, materialization, declaration provenance and continuation of mapping tasks.
3. Rust validation: one trait per output family, compiled with original model declarations; E0119/E0207 negatives are also pinned.
4. Automatic comparison of marked examples and outputs in both tutorial languages.

Parsing depth, semantic recursion, output size and work each have limits to keep
model execution bounded. The production implementation has the project's own
size and quality checks.

Complete Ty dispatch, declaration/constraint binding, attribute/directive carriers,
lifetime/const generics, hygiene/spans, complete pointer/prefix lexing, expansion
limits and real consumer projects require separate production-macro tests. Running
the model does not run those tests; integration does not expand this parser's coverage.

Found model counterexamples have regression checks. No unresolved counterexample
remained in the enumerated domain; this neither proves the absence of all defects
nor establishes release readiness.
