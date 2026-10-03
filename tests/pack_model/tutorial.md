# From per-position wrappers to heterogeneous combinations: `*` packs

English | [简体中文](tutorial.zh-CN.md)

This tutorial follows the independent v2 model in this directory. Its Pack semantics
are integrated into **batch-impl 0.10.0 in development (unreleased)**. The blocks show
type expressions and their generated Rust types, not complete macro programs;
method bodies still belong to batch-impl's existing mechanisms.

The model runs, and every marked example has a fixed expected result. The unified runner
generates `target/pack-model/generated_examples.rs`: ordinary Rust that checks the target
types and parameter relationships independently. The repository's Rust regression
suite separately validates the public macro.

Keep one sentence in mind: **an ordinary constructor takes a whole pack as arguments;
a starred constructor works on entries; each application opens only the current layer
of the right pack.**

## 1. Start with a pack of arguments

`*[u8, u16]` is a pack with two members. Given to ordinary `Pair`, they become two generic arguments:

<!-- example: args -->
```text
Pair *[u8, u16]
```
<!-- output: args -->
```rust
Pair<u8,u16>
```

Starring the left side instead means that its constructor processes each member of the right pack:

<!-- example: map -->
```text
(*Vec *[u8, u16],)
```
<!-- output: map -->
```rust
(Vec<u8>,Vec<u16>)
```

These are three different things: `Vec` is an ordinary constructor, `*Vec` is a pack
containing that constructor, and the outer `(...,)` is the ordinary tuple receiving
the final members. A pack is an intermediate structure, not an extra Rust type or a
variable the user must name.

A pack may contain an ordinary tuple, which still counts as one member. Only packs
are spliced during final collection.

## 2. Generate independent positions and wrap each one

Consider column storage where each column has its own element type and uses a `Vec`.
The desired shape is `(Vec<T0>, Vec<T1>, Vec<T2>)`.

`().3` generates an ordinary tuple with three fresh parameters; `*[].3` generates
a pack with three parameters. Give that pack to `*Vec`:

<!-- example: vec-fixed -->
```text
(*Vec *[].3,)
```
<!-- output: vec-fixed -->
```rust
(Vec<T0>,Vec<T1>,Vec<T2>)
```

This can describe a family of impls for layout markers or registration interfaces
over column caches. `T0`, `T1` and `T2` are independent generic parameters; a caller
may still instantiate some of them with the same concrete Rust type.

To support one through three columns, change only the length to a range:

<!-- example: vec-range -->
```text
(*Vec *[].1..=3,)
```
<!-- output: vec-range -->
```rust
(Vec<T0>,)
(Vec<T0>,Vec<T1>)
(Vec<T0>,Vec<T1>,Vec<T2>)
```

These are three targets, each with its own declarations. `..` excludes the upper
endpoint and `..=` includes it. The names shown here aid reading; the model's names
also carry generator identity, with the same relationships.

## 3. Reuse a parameter within its position

Suppose each field also stores a current value, requiring `Pair<T, Vec<T>>`.
Both occurrences of `T` within a position must agree; different positions remain independent.

The small step `*[self, Vec] T` builds a pack containing `T` and `Vec<T>`. Here `self`
is the DSL identity prefix: it returns its input. It is neither a Rust method receiver nor `@Self`.

Apply this step to every fresh parameter, then let `Pair` receive each row:

<!-- example: pair-fixed -->
```text
(*Pair (*[self, Vec] *[].3),)
```
<!-- output: pair-fixed -->
```rust
(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>,Pair<T2,Vec<T2>>)
```

Read from the inside out: generate three parameters; form each parameter together
with its `Vec`; give each row to `Pair`; collect the results in the outer tuple.

**Each row in the right pack is one complete input.** Here a row is itself a pack,
so `Pair` receives both `T0` and `Vec<T0>` as arguments. It is not forced to produce
separate `Pair<T0>` and `Pair<Vec<T0>>` targets.

Once familiar, use right-associative dots to reduce parentheses. The length family is:

<!-- example: pair-range -->
```text
(*Pair.*[self, Vec].*[].1..=3,)
```
<!-- output: pair-range -->
```rust
(Pair<T0,Vec<T0>>,)
(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>)
(Pair<T0,Vec<T0>>,Pair<T1,Vec<T1>>,Pair<T2,Vec<T2>>)
```

Spaces remain left-associative and dots remain right-associative. Start with the
previous example's spaces and explicit grouping.

## 4. Keep an ordinary tuple at every position

For `(T, Vec<T>)` instead of a custom `Pair`, use the ordinary tuple constructor `()`.

However, `*[]` is empty. A pack containing one `()` constructor is written `*()`:

<!-- example: tuple-fixed -->
```text
(*() (*[self, Vec] *[].3),)
```
<!-- output: tuple-fixed -->
```rust
((T0,Vec<T0>),(T1,Vec<T1>),(T2,Vec<T2>))
```

This directly represents heterogeneous field state such as a current value plus
its history. Each row has become an ordinary tuple, so splicing the outer pack
does not open that row again.

Without the inner tuple construction, the members remain flat:

<!-- example: flat-members -->
```text
(*[self, Vec] *[].2,)
```
<!-- output: flat-members -->
```rust
(T0,Vec<T0>,T1,Vec<T1>)
```

No double star is needed. Keeping structure depends on explicitly constructing an ordinary tuple.

**This covers the original three common goals: wrapping each position, reusing its
parameter, and retaining its internal structure.** The remaining combinations are optional reading.

## 5. Two common variations

### Two buffers per field type

Replace `self,Vec` with `Vec,Vec` to give each element type two containers, for example
front/back or read/write buffer layouts:

<!-- example: double-buffer -->
```text
(*() (*[Vec, Vec] *[].2),)
```
<!-- output: double-buffer -->
```rust
((Vec<T0>,Vec<T0>),(Vec<T1>,Vec<T1>))
```

These are layout targets. Swapping buffers and synchronizing state still require method bodies.

### Wrap each position more than once

First put every parameter in `Box`, then every result in `Vec`:

<!-- example: nested-wrap -->
```text
(*Vec (*Box *[].2),)
```
<!-- output: nested-wrap -->
```rust
(Vec<Box<T0>>,Vec<Box<T1>>)
```

Do not remove the grouping to get `*Vec *Box *[].2`. Spaces associate left, so that
form first constructs `Vec Box`, then appends arguments; it does not return to fill
the inside of `Box`.

Likewise, `Vec.Box` is already `Vec<Box>`. Putting it in a pack does not turn it into
a function with an implicit parameter hole. Packs introduce neither arbitrary templates nor lambdas.

## 6. Combine two axes and choose whether to keep rows

Suppose input and output type axes have two and three members respectively, and
`Map<T,U>` represents a relation between their types. Five parameters give six pairs.

<!-- example: flat-grid -->
```text
(*Map *[].2 *[].3,)
```
<!-- output: flat-grid -->
```rust
(Map<T0,U0>,Map<T1,U0>,Map<T0,U1>,Map<T1,U1>,Map<T0,U2>,Map<T1,U2>)
```

The right axis is outermost: fix `U0` and traverse `T0/T1`, then process `U1` and
`U2`. This is a Cartesian combination, not a zip, and the axis lengths may differ.

Before final collection, the pack still has rows. As above, receive each row with
an ordinary tuple constructor to produce a two-dimensional layout:

<!-- example: row-grid -->
```text
(*() (*Map *[].2 *[].3),)
```
<!-- output: row-grid -->
```rust
((Map<T0,U0>,Map<T1,U0>),(Map<T0,U1>,Map<T1,U1>),(Map<T0,U2>,Map<T1,U2>))
```

This retains both dimensions of an input/output relation matrix. Replacing the
lengths with `1..=2` and `1..=3` produces six nested targets that can implement one
trait; the Rust validator checks this family.

The corresponding **flat length family overlaps**: the generic patterns for `1×2`
and `2×1` can cover the same concrete type, producing E0119. Keeping rows resolves
this particular loss of dimension information, not every possible impl overlap.

## 7. Candidate lists choose; packs construct together

These requests differ:

| Request | Expression | Result count |
|---|---|---:|
| Use Vec or Box uniformly | `([*Vec,*Box] *[].2,)` | 2 targets |
| Choose Vec or Box at each position | `(*[[Vec,Box],] *[].2,)` | 4 targets |
| Keep both Vec and Box at each position | `(*[Vec, Box] *[].2,)` | 1 target with 4 members |

The full result of the second row is:

<!-- example: local-choice -->
```text
(*[[Vec,Box],] *[].2,)
```
<!-- output: local-choice -->
```rust
(Vec<T0>,Vec<T1>)
(Vec<T0>,Box<T1>)
(Box<T0>,Vec<T1>)
(Box<T0>,Box<T1>)
```

Candidates can also choose whole packs of different lengths:

<!-- example: branch-packs -->
```text
(*Vec [*[u8,],*[u16, u32]],)
```
<!-- output: branch-packs -->
```rust
(Vec<u8>,)
(Vec<u16>,Vec<u32>)
```

A candidate list is not a variable selected once and shared everywhere afterwards.
Candidates exposed at the current application are dispatched first. Candidate
expressions hidden inside ordinary types may choose independently after copying.
For example, `([A,B],).2` keeps four combinations, not just `(A,A)` and `(B,B)`.

## 8. Boundaries that are easy to misread

**A whole tuple differs from its members.** Without starring the generator result,
the complete tuple can be one input:

<!-- example: whole-tuple -->
```text
Pair.*[self, Vec].().2
```
<!-- output: whole-tuple -->
```rust
Pair<(T0,T1),Vec<(T0,T1)>>
```

**A star around a completed range collects the family.** This gives one target
containing two Vec types, each wrapping an entire tuple:

<!-- example: collect-family -->
```text
(*Vec *(().1..=2),)
```
<!-- output: collect-family -->
```rust
(Vec<(T0,)>,Vec<(U0,U1)>)
```

**A star does not turn a single slot into multiple slots.** Generic arguments,
tuple fields and function parameters accept multiple entries. Reference and pointer
targets, slice elements and function return types accept exactly one. `&*[u8,]`
can materialize as `&u8`; `&*[u8, u16]` must report an error.

**Commas identify containers.** For this tutorial's ordinary types and packs, `(X)`
is transparent and `(X,)` is a one-tuple; `[X]` is a slice and `[X,]` is a one-item
candidate list. Rust raw pointers `*const` and `*mut` take parsing precedence.
Existing positional-reference meta-syntax such as `@` has its own grouping contract;
Pack support does not redefine it, and the independent model does not cover it.

**An empty pack differs from a pack containing an empty tuple.** `*[]` has no members;
`*()` has one ordinary tuple constructor; `*[*[],]` has one empty argument-pack row.
The latter two also differ.

**Constructing a type does not guarantee a valid impl.** For example,
`(*Map *[].2 *[].0,)` produces `()`, but carries two unconstrained parameters and
therefore E0207. The model retains this error instead of silently deleting parameters.
Duplicate targets are not deduplicated either.

## 9. Explicitly accept the migration changes

| Existing behavior or expression | 0.10.0 Pack semantics | Preserve the previous intention |
|---|---|---|
| `*[F, G] T` appends T | Apply F and G separately — state the family as a list | `*((F, G) T)` is one member: the appended tuple type |
| `*(A, B)` collects two members | A tuple is a type, so it contributes **one** member | Write the list: `*[A, B]` |
| `*()` is the empty pack | `()` is the unit type — one member | Write the empty list: `*[]` |
| `*().N` is the generator | The star of the unit type, then a power | Write `*[].N` |
| `[] [A, B]`, `[] [A, B] N` build a slice or array | `[]` is the empty list, and applying it stays empty | Write `[[A, B]]` / `[[A, B]; N]` — the element slot takes the list |
| `(*[A, B])` implicitly supplies an outer tuple | Ordinary grouping is transparent | `(*[A, B],)` |
| `[*[A, B]]` uses the lone-splat exception as a list | Without a comma it remains a slice | `[*[A, B],]` |
| Ordinary candidates inside a pack flatten into simultaneous members | Candidates remain choices | Explicitly star the candidate layer to collect |

This is a breaking 0.10.0 semantic change, not fully compatible syntax sugar: `*`
opens **only a candidate list**, so a tuple, the unit type, a slice and an array
each contribute one whole member. Ordinary tuple power keeps its direct-slot rule:
`(*[A, B],).2` remains a single `(A,B,A,B)` target.

## 10. Run the model and inspect validation

From the repository root:

```text
python tests/pack_model/run.py
python tests/pack_model/run.py --eval "(*() (*[self, Vec] *[].3),)"
```

`syntax.py` is a strict subset parser: unsupported input reports an error rather
than silently dropping characters. It does not implement all existing library
syntax, including full declarations, where clauses, bindings, attributes and some
prefix applications. Rejection by the model does not prohibit them in the public macro.

See [contract.md](contract.md) for precise rules, corrections and production-validation
boundaries. A run writes its reports to `target/pack-model/validation.json` and
`target/pack-model/exhaustive_results.json`. Finite checks cannot prove the absence
of all defects. The current claim is limited to closing the core counterexamples
found in this audit and explaining the main constructions and teaching examples
with one set of rules.
