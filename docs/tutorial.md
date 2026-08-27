# batch-impl Tutorial

**v0.9.6** (2026-08-27) — **the ItemImpl entry catches up with the
attribute entry**: `#[batch_impl(spec)] impl ...` now shares the full DSL —
`@` built-in constants in the matrix source; generators with `@N..` where
selectors (`GenA<()0..=12>` hoists fresh generics onto the impl, `where
@0..: SomeTrait` constrains them); the `fresh!(...)` body marker
(`type MyTuple = (fresh!(@(@T,)..))` → `(P0, P1, P2)` — repeat blocks /
fresh references in the body, wrapped in a legal macro-call spelling and
fully expanded — the output never contains a `fresh!` call); the block
model (each matrix element pairs a container with its own `impl{...}`
template — `[[Box,Rc]impl{A<(T@..)>}, Vec impl{Vec<(T@..)>}].().2..=3`;
`where{...}` composes at any position); textual substitution for
non-matching templates; variadic-segment templates (`A<(T@..)>`) driving
`fresh!` segment references.

Progressive DSL learning: from a one-line impl to advanced matrix combinations. All examples are compilable code (the code blocks of this English tutorial double as doctests), and every step's output is plain Rust — the generated impls are token-equivalent to handwritten ones.

## 0. Three systems + one operator

Every capability of batch-impl is built from three pillars (polished continuously from 0.0 to 0.6) plus one operator (0.7.0):

| Part | Notation | Role |
|---|---|---|
| **apply system** | `.` / space / `[]` / `()` | Type matrix: apply the left container/modifier to the right type, lists expand into multiple impls |
| **directive system** | `#name` / `#fill` / `#delegate` / `#blanket` | Copy signatures from the trait definition, fill bodies in bulk, delegate calls, blanket delegation |
| **constant system** | `@u*` / `@scalar` / `@u8..u128` / `@name=...` | Macro-meta layer: name and reuse type-matrix entries, pure lexical substitution |
| **`*` operator** | `*[...]` / `*(...)` | Flatten: splice a container/generator into the enclosing list — new in 0.7.0, effective in every position |

**Preprocessing order** (fixed four-stage pipeline): `@` constant expansion → `<>` angle-bracket pairing → `#` directive expansion → `where` processing. The order decides what you can write into what: `@` results may contain `<>` (paired afterwards), `#` arguments may reference `@`-expanded lists, `where` sees the complete structure last.

## 1. Starting from a One-Line impl

`#[batch_impl(...)]` annotates a trait definition; every spec in its argument generates one impl:

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize, isize, f32, f64)]
trait Numeric {}
// → impl Numeric for usize {}
// → impl Numeric for isize {}
// → impl Numeric for f32 {}
// → impl Numeric for f64 {}
```

The spec skeleton:

```text
<impl-generics> TraitName<trait-generics> TargetType { body }?
```

| Part                  | Example                              | When needed              |
|-----------------------|--------------------------------------|--------------------------|
| `<impl-generics>`     | `<T>`, `<T: Clone>`, `<const N: usize>` | when the impl block needs generic params |
| `TraitName<trait-generics>` | `MyTrait<T>`, `MyTrait<Vec<T>>` | when the trait definition has generic params |
| Target type           | `usize`, `Vec<T>`, `&str`            | required                  |
| `{ body }`            | `{ fn m(&self) -> usize { 0 } }`     | when a custom body is needed |

Multiple specs are separated by `,`: `#[batch_impl(usize, isize)]`.

## 2. Type Matrix: the space (and `.`)

**The space is the natural way to apply**: write the container/modifier and the types it takes side by side — chaining accumulates arguments left-associatively.

> **What the space actually is**: a space is **not a token** — it is the *gap between tokens* (proc-macro2 strips whitespace, so the DSL sees only adjacency). A space application therefore means "these tokens are adjacent, apply them" (`Box u8` = `Box<u8>`), which is exactly how Rust itself reads type syntax (`Box<u8>` is `Box` adjacent to `<u8>`). No explicit operator symbol is needed — the absence of a separator *is* the operator.

| Writing                    | Expansion                            |
|----------------------------|--------------------------------------|
| `Box u32`                  | `Box<u32>`                           |
| `HashMap u32 String`       | `HashMap<u32, String>` (left-associative accumulation) |
| `fn(A,B) C`                | `fn(A,B)->C` (or write `fn(A,B) -> C`) |
| `&u8`                      | `&u8` (chained modifiers)            |
| `Tr u8`                    | `impl Tr for u8` (a bare trait name) |
| `[Box, Vec] u32`           | `Box<u32>, Vec<u32>` (lists expand)  |
| `HashMap<u8> String`       | `HashMap<u8, String>` (prefilled generics appended) |
| `Box [u8, u16]`            | `Box<u8>, Box<u16>` (list distributes) |
| `[Box, Vec] [u8, u16]`     | Cartesian product, 4 entries         |

**`.` is the same operation with right-associative grouping** — reach for it only when you want **nesting** instead of accumulation. Space accumulation puts arguments side by side (`Box Box u8` = `Box<Box, u8>` — a typo for most containers); `.` nesting composes them (`Box.Box.u8` = `Box<Box<u8>>`):

| Writing                    | Expansion                            |
|----------------------------|--------------------------------------|
| `Box.Box.u8`               | `Box<Box<u8>>` (right-associative nesting) |
| `&Box u8`                  | `&Box<u8>` (modifier over the nested type) |
| `[Box, Vec] T`             | `Box<T>, Vec<T>`                     |
| `Box [T1, T2]`             | `Box<T1>, Box<T2>`                   |
| `[HashMap<K>, Vec<K>] V`   | `HashMap<K, V>, Vec<K, V>`           |

> **When to use which**: one container/modifier + one type — write them side by side (`Box u8`, `&u8`, `HashMap<u8> String`). When the type itself needs to be a composed type (`Box<Box<u8>>`, `&Box<u8>`), join the composition with `.` — the space would treat each part as a separate argument.

**The bare trait name** applies as the impl trait: `Tr u8` = `impl Tr for u8`, `Tr<A> u8` = `impl Tr<A> for u8`. Write `Tr<u8>` for the **type** `Tr<u8>`. In general, a bare `Tr` is not recommended.

Precedence from low to high: `;` < `,` < space < `.`; `()` grouping sits above all operators.

> **Note**: `Box.Vec u32` is wrong (it parses as `Box<Vec, u32>`); write `Box.Vec.u32` instead. When you miswrite it, rustc's E0107 error prints the rendered `Box<Vec, u32>` verbatim — the mistake is self-evident.

> **Operand strictness**: both sides of `.`/space/`,` must have operands — `A.`, `.A`, `,A`, `A,,B` all report `compile_error!`; only **trailing commas** (`A,` / `[A, B,]`) are allowed, and `()`/`[]` brackets are real tokens, not empty operands. `;` stays lenient as a `batch_trait!` section boundary.

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(Box.Vec.u32, HashMap<u8> String)]
trait T {}
// → impl T for Box<Vec<u32>> {}   ← `.` nesting: Box applied to Vec<u32>
// → impl T for HashMap<u8, String> {}
```

## 3. Lists and Body

### Side-by-side lists `[A, B]`

One body is reused for all target types:

```rust
# use batch_impl::batch_impl;
#[batch_impl([usize, isize, f32] {
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged { fn tag(&self) -> &'static str; }
// → impl Tagged for usize { fn tag(&self) -> &'static str { "number" } }
// → impl Tagged for isize { ... }
// → impl Tagged for f32   { ... }
```

**Distribution propagation**: `[A, B]` lists are distribution sources — beyond being targets/operands, nested positions propagate too:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, [u16, u32, u64]))]
trait T {}
// → impl T for (u8, u16) {}
// → impl T for (u8, u32) {}
// → impl T for (u8, u64) {}

#[batch_impl(Vec<[u8, u16, u32]>)]
trait V {}
// → impl V for Vec<u8> {}
// → impl V for Vec<u16> {}
// → impl V for Vec<u32> {}
```

Rule: `[A, B]` inside a tuple/generic-arg position → Cartesian-product distribution (all combinations of multiple arrays); nested arrays recurse to leaves (`Vec<[[A,B], C]>` → `Vec<A>`/`Vec<B>`/`Vec<C>`); combos of `(X, [A,B]).N` containing arrays are covered by the outer distribution. Note: concrete generators combined with fresh generators may overlap (E0119 — same fresh count/structure); rustc catches it — use generators with different fresh counts to avoid.

### Independent/shared body merging

List items may carry independent bodies, merged with the shared body — different items coexist (writing the same item twice is a user error rustc reports):

```rust
# use batch_impl::batch_impl;
#[batch_impl([
    usize { fn name(&self) -> &'static str { "usize" } },
    isize { fn name(&self) -> &'static str { "isize" } },
    f32  { fn name(&self) -> &'static str { "f32" } },
] {
    fn zero() -> Self { Default::default() }
})]
trait Tagged { fn zero() -> Self; fn name(&self) -> &'static str; }
// → impl Tagged for usize { fn name... "usize"; fn zero() { Default::default() } }（independent name + shared zero）
// → impl Tagged for isize { fn name... "isize"; fn zero() { 0 } }
// → impl Tagged for f32   { fn name... "f32";   fn zero() { 0 } }
```

## 4. splat `*` — the Flatten Operator (the protagonist of 0.7.0)

The splat draws its intuition from Python's `*` unpacking — `[a, *b]` splices a list, `f(*args)` unfolds arguments. batch-impl's `*` is the same **single-layer unpack**: a splat splices a container/generator into the enclosing list, expanding exactly one level.

| Python | batch-impl |
|---|---|
| `[a, *b]` | `[A, *[B, C]]` — splice a list into the outer list |
| `f(*args)` | `T *(A, B, C)` — unfold a generator into argument positions |
| one level of unpack | `*((a,b),)` = one `(a,b)` impl (tuples stay intact) |

**Motivation**: `*` compresses a nested generator into a multi-arg container. Instead of hand-writing `T [A,B,C] [A,B,C] [A,B,C]` (27 combos of nested lists), one line gives the same 27 impls:

```rust
# use batch_impl::batch_impl;
struct T<A, B, C>(A, B, C);   // 3-arg container
struct A; struct B; struct C;
#[batch_impl(T *(A, B, C).3)]  // splat-pow: unfold (A,B,C).3 into three arg positions
trait Matrix27 {}
// → 27 impls: T<A,A,A> / T<A,A,B> / ... / T<C,C,C>（same as T [A,B,C] [A,B,C] [A,B,C]）
```

`*[...]` / `*(...)` splices a container/generator into the enclosing list. A splat stays a **whole unit** through parse/apply/expand and only flattens into its elements at codegen — one code path for every position.

### 4.1 In-list / in-tuple splicing

```rust
# use batch_impl::batch_impl;
# struct A; struct B; struct C;
#[batch_impl([A, *[B, C]])]
trait T {}
// → impl T for A {} / B / C（splice: `[A, *[B, C]]` = `[A, B, C]`）

#[batch_impl((A, *(B, C)))]
trait U {}
// → impl U for (A, B, C) {}（tuple splice appends）
```

### 4.2 Left operand: distribute vs append

`[]` is a **set** and `()` is a **sequence** — splat just mirrors the source bracket, so `*[A,B] T` distributes (each element applies `T`, keeping set semantics) and `*(A,B) T` appends (keeping list semantics). This is not a new rule; it preserves the underlying container's behavior, and `TySplat::Array`/`TySplat::Tuple` mirror `TyArray`/`TyTuple`.

```rust
# use batch_impl::batch_impl;
#[batch_impl(*[Vec, Box] u8)]        // array splat distributes: each element applies u8
trait T1 {}
// → impl T1 for Vec<u8> {} / Box<u8>

#[batch_impl(*(Vec<u8>, Box<u8>) u16)]  // tuple splat appends: the right operand joins
trait T2 {}
// → impl T2 for Vec<u8> {} / Box<u8> / u16（append）
```

### 4.3 Generic args and trait paths

```rust
# use batch_impl::batch_impl;
struct Pair<X, Y>(X, Y);
struct A; struct B;
#[batch_impl(Pair<*(A, B)>)]
trait G1 {}
// → impl G1 for Pair<A, B> {}（one impl, two args）

#[batch_impl(Conv<*(A, B)> Pair<A, B> #cv{unimplemented!()})]
trait Conv<T, U>: Sized { fn cv(_v: T, _o: U) -> Self; }
// → impl Conv<A, B> for Pair<A, B> { fn cv(_v: A, _o: B) -> Self { unimplemented!() } }
```

A splat power inside generic args distributes its Cartesian result one impl per pair:

```rust
# use batch_impl::batch_impl;
struct Frac<T, U>(T, U);
#[batch_impl(Frac<*(*@u*)2>)]
trait Pow {}
// → impl Pow for Frac<u8, u8> {} ... impl Pow for Frac<usize, usize> {}（36 impls）
```

### 4.4 Container rule

A group whose content is a lone splat parses as the container holding the splat as one element — `(*(a,b))` = `( *(a,b) )`, `[*(a,b)]` = `[ *(a,b) ]`; the splat element expands only in codegen.

### 4.5 Generator re-wrap

`*()N` — a generator splat — hoists fresh declarations and splats the tuple into a container:

```rust
# use batch_impl::batch_impl;
struct Pair3<A, B>(A, B);
#[batch_impl(Pair3<*()2>)]
trait GenSpl {}
// → impl<P0, P1> GenSpl for Pair3<P0, P1>（flattened into two args）
```

### 4.6 Legal positions

A splat is a **parameter-position list**: generic args / tuple / array elements / generic declarations / fn parameters / spec lists. A bare splat as a **where-predicate subject** is rejected (`*(A,B): Trait` has no defined semantics); a bare `*` that is neither a splat nor a raw pointer errors with a targeted message.

## 5. Generics `<>`

### 5.1 Declarations

`<...>` before the trait name declares impl generics — copied into the impl as-is:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Vec<T>)]
trait T2 {}
// → impl<T> T2 for Vec<T> {}
```

### 5.2 `A<>` — copied as-is

An empty `<>` copies the trait's own generics verbatim:

```rust
# use batch_impl::batch_impl;
#[batch_impl(A<> Vec<u8>)]
trait A<T, const N: usize> {}
// → impl<T, const N: usize> A<T, N> for Vec<u8> {}
```

### 5.3 Args: multi-args, nesting, bindings

```rust
# use batch_impl::batch_impl;
struct Map<K, V>(K, V);
struct A; struct B; struct C;
struct Wrap<X>(X);
#[batch_impl(Map<A, B>)]                 // multi-args
trait M1 {}
#[batch_impl(Map<Map<A, B>, C>)]         // nested structure preserved (TyGeneric nesting)
trait M2 {}
#[batch_impl(Conv<u8, Item = u8> Wrap<u8>)]  // associated-type binding (trait path)
trait Conv<T> { type Item; }
```

### 5.4 Operations inside `<>` (programmable in 0.7.0)

Generic-argument positions accept full DSL expressions — the structural landing of 0.7.0:

```rust
# use batch_impl::batch_impl;
struct Wrap<X>(X);
struct Pair3<A, B>(A, B);
struct A2; struct B2;

#[batch_impl(Wrap<()2>)]               // generator: <P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1)>（the tuple stays a single arg）

#[batch_impl(Pair3<*()2>)]             // generator splat: <P0,P1> Pair3<P0,P1>
trait GenSpl {}
// → impl<P0,P1> GenSpl for Pair3<P0, P1>（flattened into two args）

#[batch_impl(Wrap<@u*>)]                // constant family: 6 impls (u8..usize)
trait ConstArg {}

#[batch_impl(Wrap<[A2, B2]>)]           // array: 2 impls (Wrap<A2>/Wrap<B2>)
trait ListArg {}
```

### 5.5 Same-name inheritance and trait where inheritance

When the trait's generic params share names with the spec's args, bounds inherit automatically; renaming errors explicitly:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Box<T> where Box<T>: Clone {})]
trait B2 {}
// → impl<T> B2 for Box<T> where Box<T>: Clone {}
```

```rust,ignore
#[batch_impl(<T> Foo<U>)]  // renamed (U ≠ T) → explicit error (not silent)
trait Foo<T> {}
```

## 6. The `@` Constant System (macro-meta layer)

`@` is the DSL's reserved **library-owned constant namespace** — `#` is taken by the directive mechanism, so `@` provides "name and reuse type-matrix entries". It is pure **lexical substitution** (the macro-meta layer): the expanded result enters the pipeline and participates in no in-domain parsing.

### 6.1 Built-in constants

**Name families** (a closed set — the language-defined type collections): `@u*`, `@i*`, `@f*`, `@num`, `@scalar`. Each expands to its members as a list:

| Constant | Expands to |
|---|---|
| `@u*` | `u8, u16, u32, u64, u128, usize` |
| `@i*` | `i8, i16, i32, i64, i128, isize` |
| `@f*` | `f32, f64` |
| `@num` | every `@u*` + `@i*` + `@f*` member (14 types) |
| `@scalar` | the primitive scalars (the numeric families + `bool` + `char`) |

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box @u*)]  // Box applied to every member of @u*
trait BoxRc {}
// → impl BoxRc for Box<u8> {} / Box<u16> / ... / Box<usize>
```

**Range families**: `@u8..u128`, `@i8..i128`, `@f32..f64` (inclusive) — the
contiguous run of one family (`@u8..u128` → `u8, u16, u32, u64, u128`).
Either endpoint may be **omitted**: `@..u128` ≡ `@u8..u128`, `@u16..` ≡
`@u16..u128`, `@f32..` ≡ `@f32..f64` (the omitted side resolves to the
family's minimum/maximum; at least one endpoint anchors the family).
`usize`/`isize` only enter name families, not range families.

### 6.2 Lazy expansion and references

Constant values are stored as **verbatim tokens**; reference sites splice and expand recursively — a value can be a DSL expression (`@uints=@uint`) or a chained reference (`@a=@b`). Cycles/forward references are rejected at definition (preventing infinite recursion); a bare range endpoint reference (`@a=@u8` without `..`) errors at definition.

### 6.3 Custom constant sections (`batch_trait!` only)

A leading `@name=value;` section defines reusable constants (values may chain
references and embed DSL expressions). **`#[batch_impl]` / `#[batch_impl_only]`
do not support custom constants** — the 0.7.2 feature was reverted in 0.8.0;
write attribute-macro matrices directly with `.`/space/`*` instead:

```rust
# use batch_impl::batch_trait;
# trait A {} trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> Vec<T>;
}
```

> **Limit**: `batch_trait!` **does not support `#` directives** (`#fill`/`#delegate`/`#blanket`/open extension) — directives need the trait definition as the signature source of truth, and `batch_trait!` is a function-like macro that never sees one. Use `#[batch_impl]` / `#[batch_impl_only]` when you need directives.

### 6.4 The complete macro-meta layer: an addressing algebra + value classes

`@`'s positional references form an **addressing algebra** — not a flat list of notations:

| Notation | Derivation | Expands to |
|---|---|---|
| `@g_i` | **primitive** — group g, slot i (stable across array distribution) | the i-th fresh of generator group g (`@0_0` → the first fresh of the first generator) |
| `@N` | `@g_i` flattened by document order within one impl | the N-th fresh generic name (`@0` → `P0` in a `where{@0: Clone}` predicate) |
| `@all_fresh` | all fresh generics | every fresh name, one predicate each (≡ `@0..`); **deprecated**, write `@0..` |
| `@N..=M` | a contiguous run | the fresh names N..=M, comma-separated (`@0..=1` → `P0, P1`) |
| `@N..` | an **open** run to the last fresh | every fresh name from N to the last, comma-separated (`@1..` → `P1, P2, ...`); **empty** when N is past the end (an arity-1 impl contributes no such predicate, no error) |

Fresh display names are numbered `P0, P1, ...` in document order (a collision
with an ident the impl already writes escapes by spreadsheet-style letter
suffixes: `P0A`, `P0B`, ... `P0Z`, `P0AA`) — the expansion splices the names where the `@` sits (a where
predicate subject, a target tuple element, a generic argument), so a range
becomes several names and a `where` tail is copied per fresh.

> **Power-user tier**: `@g_i` / `@all_fresh` / `@N..M` are advanced addressing notations — start from `@u*` / `@all_methods` / `@0` and reach for them only when a predicate must name a specific fresh. The whole DSL surface is frozen since 0.7.2 (see README); these notations will not change semantics again.

```rust
# use batch_impl::batch_impl;
#[batch_impl(()2 where @0..=1: Clone)]   // range sugar: @0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1) where P0: Clone, P1: Clone

#[batch_impl(()3 where @0..: Copy)]       // = @all_fresh (from 0 to the last fresh)
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2) where P0: Copy, P1: Copy, P2: Copy

#[batch_impl(()3 where @1..: Copy)]       // open range: from index 1 on
trait OpenRange {}
// → impl<P0,P1,P2> OpenRange for (P0,P1,P2) where P1: Copy, P2: Copy
// (an arity-1 impl contributes no predicate — `@1..` is empty there)
```

`@all_fresh` and `@0..` are equivalent; **`@all_fresh` is deprecated** — the
`@N..` family is the preferred spelling (`@0..` covers the whole run, `@1..`
its tail). Existing specs keep working; new code should write `@0..`.

**Ranges work anywhere a single `@N` can** (0.9.2): beyond the where
predicates above, the range's tail may be an associated-type path, copied
per fresh — and a range in a target position re-opens against the fresh
list the spec's generators produced:

```rust
# use batch_impl::batch_impl;
struct Wrap3<A, B, C>(A, B, C);
#[batch_impl(Wrap3<*()3> where @0..: Clone { fn m(&self) {} })]
trait RangeAngle { fn m(&self); }
// → impl<P0,P1,P2> RangeAngle for Wrap3<P0,P1,P2> where P0: Clone, P1: Clone, P2: Clone

trait HasOut { type Out; }
#[batch_impl(Wrap3<*()3> where @0..: HasOut, @0..::Out: Clone { fn m(&self) {} })]
trait RangeAssoc { fn m(&self); }
// → where P0: HasOut, P0::Out: Clone, P1: HasOut, P1::Out: Clone, P2: HasOut, P2::Out: Clone
```

The fresh list a range indexes comes from the spec's generators (`*().N` /
`().N`); a range in a spec with no fresh generics reports "out of range".

**The impl-generic declaration position** works too: `<@0..>` declares every
fresh the range covers as an impl param — so a spec can put the generator in
the trait args (`GenConv<*().2>`) and reference the same fresh batch in the
declaration and the predicates:

```rust
# use batch_impl::batch_impl;
struct DeclTarget;
#[batch_impl(<@0..> GenConv<*()2> DeclTarget where @0..: Clone { fn m(&self) {} })]
trait GenConv<T, U> { fn m(&self); }
// → impl<P0,P1> GenConv<P0,P1> for DeclTarget where P0: Clone, P1: Clone
```

(An empty `<@0..>` — no fresh generators in the spec — contributes no
parameters, like an empty `@1..` predicate.)

**Grouped ranges `@L_N..`** (0.9.2) slice **within one generator group** —
the in-group counterpart of `@g_i`, stable across array dispatch. With
several generators in one spec (`<*().2>` → group 0, `<*().3>` → group 1),
`@1_0..` constrains only group 1's fresh:

```rust
# use batch_impl::batch_impl;
struct MultiTarget;
#[batch_impl(
    <@0..> <@1..> PairGen<*()2, *()3> MultiTarget where @1_0..: Clone
    { fn m(&self) {} }
)]
trait PairGen<A, B, C, D, E> { fn m(&self); }
// → impl<P0,P1,P2,P3,P4> PairGen<P0,P1,P2,P3,P4> for MultiTarget
//     where P2: Clone, P3: Clone, P4: Clone   ← group 1 only (P0,P1 unconstrained)
```

`@L_N..` (open to the group's end), `@L_N..M` and `@L_N..=M` (closed) all
work; an unknown group errors like `@g_i`.

`@N` also resolves in **value positions** — the type after `:` may carry
`@N` inside angle groups, e.g. an associated-type binding referencing
another fresh's associated type (the alga2 tuple `Module` scalar-equality
constraint):

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Module<(), ()> ()1..=4 where @0..: Module<(), (), Scalar: Copy>,
        @1..: Module<(), (), Scalar = @0::Scalar>
        impl{(A@..)} impl{@{}}
    #Scalar{@{0}::Scalar}
    #scale{( @(@A::scale(&self.@0, s),).. )}
)]
trait Module<Add, Mul> {
    type Scalar;
    fn scale(&self, s: Self::Scalar) -> Self;
}
// arity 2 → impl<P0,P1> Module<(), ()> for (P0,P1)
//   where P0: Module<(), (), Scalar: Copy>, P1: Module<(), (), Scalar: Copy>,
//         P1: Module<(), (), Scalar = P0::Scalar>
```

The shared-scalar pattern: every component from the second one on declares
`Scalar = @0::Scalar` (the first component's scalar), with `@0` resolving to
the first fresh's name. The `@1..` open range is exactly the "from the
second component on" set — it shrinks with the tuple arity and disappears
for arity 1.

On the other axis (value classes):

| Notation | Class | Use |
|---|---|---|
| `@trait` | **identity** — the current trait name/path (section-level in batch_trait) | package "generic declaration + trait name" across sections |
| `@all_methods` etc. | **selection** — extract an item set from trait_def | `#fill(@all_required_methods, -foo)` precise selection |
| `@Cow` | **built-in `#blanket` wrapper constant** — `Cow<'_>` plus its inherent constraints (`@0: ToOwned + ?Sized, @0::Owned: @trait`) | blanket-usable `Cow` delegation (see §7.4) |

`@all` family combined with `-` subtraction selects arbitrary item subsets (`#fill(@all_required_methods, -foo)`); `@all_default*` / `@all_required*` distinguish default implementations from required methods.

`X<>` (empty angle brackets on the **same-named** trait) in a where
predicate or an `impl{...}` template syncs to the spec trait application —
write `Semiring<>` instead of repeating `Semiring<Additive, Multiplicative>`:

```rust
# use batch_impl::batch_impl;
# struct Additive;
# struct Multiplicative;
#[batch_impl(
    Semiring<Additive, Multiplicative> ().1..=2 where @0..: Semiring<> {},
)]
trait Semiring<Oa, Om> {}
// → impl<P0> Semiring<Additive, Multiplicative> for (P0,)
//     where P0: Semiring<Additive, Multiplicative>
// → ... arity 2 (P1 gets the same predicate)
```

`@trait<>` is equivalent (`@trait` expands to the trait path first). A `X<>`
for any trait other than the spec's errors; a trait with no generic
arguments syncs to the bare name (`Tr<>` → `Tr`). The **body** syncs via a
**switch template** `impl{Tr<>}` — a template holding only the empty-bracket
trait, which does not match Self; it only declares that the body's `Tr<>`
references sync too (the body is arbitrary Rust, so a `Vec<>` there is not a
trait reference).

### 6.5 Bound generators: Fn-family types in impl-generic bounds

A generator can run **inside an impl-generic bound**: `Fn()N` (and
`FnMut` / `FnOnce`) generates the Fn's parameter list, its fresh params ride
out to the impl generics (`impl<P0,P1, T: Fn(P0,P1)>` — never a generic
declaration inside the predicate, which rustc rejects), and the target
references the same fresh batch. This is the "one impl per Fn arity" form:
`<R, T: Fn()0..4 R> Tr<T> (@0..)` generates one impl for each arity
0..4 (exclusive), each with the bound pinned to that arity and the target
tuple re-opened to that impl's own fresh list:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<R, T: Fn()0..3 R> MultiArity<T, R> (@0..) {
    fn arity(&self) -> usize { 0 }
})]
trait MultiArity<T, R> { fn arity(&self) -> usize; }
// → impl<R, T: Fn() -> R>         MultiArity<T, R> for ()
// → impl<R, P0, T: Fn(P0) -> R>   MultiArity<T, R> for (P0,)
// → impl<R, P0,P1, T: Fn(P0,P1)->R> MultiArity<T, R> for (P0,P1)
```

`Fn()N R` — the space-apply return type — renders `Fn(P0,..) -> R`
(equivalent to `-> R`). The dot form `Fn.().N` works too (the `.` is
optional). `FnMut` / `FnOnce` render their own trait names; a
bare `fn()N` works as a **type** (`fn` is not a trait, so it cannot
be a bound, but the same generator form appears in type positions). The
`@N..` range in the target re-opens per impl, so each arity's tuple elements
are exactly that impl's Fn parameters (the empty `@0..` of the arity-0 impl
collapses to `()`).

The target tuple's trailing comma is **optional**: `(@0..)` ≡ `(@0..,)` — a
comma-less paren holding a range placeholder re-opens as a tuple, so the
arity-1 impl still renders a real 1-tuple `(P0,)` (never a group `(P0)`).

**Several bound generators** in one spec distribute as the Cartesian product
of their arities, and the target then addresses each generator's fresh by
**grouped ranges** (`@0_0..` for the first bound's fresh, `@1_0..` for the
second's) — the flat `@N..` form indexes across all groups, so two flat
ranges in one tuple would overlap. A grouped range requires its group to
exist (a `Fn()0..N` bound's arity-0 impl has no fresh for that group, so
the reference errors there — the same rule as `@g_i`).

## 7. The Directive System `#`

Directives copy item signatures from the trait definition (methods/consts/types all supported); the body is yours to fill — "declare data, not write repetitive code".

### 7.1 `#name{body}` — single-item assignment

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #to_str{"usize"})]
trait ToString { fn to_str(&self) -> &str; }
// → impl ToString for usize { fn to_str(&self) -> &str { "usize" } }
```

### 7.2 `#fill(methods){body}` — many methods, one body

```rust
# use batch_impl::batch_impl;
#[batch_impl((u32,) #fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)})]
trait Ops { fn add(&mut self, x: u8); fn add2(&mut self, x: u8); }
```

> Filling a single method, `#fill([foo]){body}` is equivalent to the single-item directive `#foo{body}`, which is more concise.

### 7.3 `#delegate(methods){target}` — delegate calls

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box.Vec.u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

#### Renaming the delegated target: `foo = call_foo` (0.9.4)

An element `foo = call_foo` delegates the trait's `foo` method to the
target's `call_foo` method — the `#[call(...)]` mechanism of the `delegate`
crate, in the DSL's `=` binding spelling. The signature keeps `foo`; only
the call uses `call_foo`:

```rust
# use batch_impl::batch_impl;
struct Wrapper(String);
impl Wrapper {
    fn len(&self) -> usize {
        self.0.len()
    }
}
#[batch_impl(Wrapper #delegate(size = len){self})]
trait HasSize { fn size(&self) -> usize; }
// → impl HasSize for Wrapper { fn size(&self) -> usize { (self).len() } }
```

Binding semantics: every selected method binds to a target — same-name by
default, or the right of `=` when renamed. A rename whose left side is
**not yet selected** adds that method (`#delegate(size=len)` selects `size`
alone); a rename overlapping the selected set **merges**
(`#delegate(@all, size=len)` — `size` → `len`, the rest by same name, no
duplicate definition); renaming the same method twice errors.

### 7.4 `#blanket(@all_methods){wrapper matrix}` — blanket delegation

Wraps any type (smart pointers included); wrappers are comma-separated, and a `:N` suffix marks the deref depth:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]
trait NumOps { fn inc(&mut self); }
impl NumOps for u32 { fn inc(&mut self) { *self += 1 } }
// → impl NumOps for Box<u32> { fn inc(&mut self) { (**self).inc() } }（delegates to the wrapped u32）

#[batch_impl(#blanket(@all_methods){&, Box})]
trait Len { fn len(&self) -> usize; }
// → impl<T: Len> Len for &T     { fn len(&self) -> usize { (*self).len() } }
// → impl<T: Len> Len for Box<T> { fn len(&self) -> usize { (**self).len() } }
```

> **`:N` deref depth** — how many layers the delegation dereferences to reach the inner `T`. Default **1** for single wrappers (`&`, `Box`, `Rc`): the body derefs N+1 times (`&`/`Box` → `**self`). A `:N` of 2 means the wrapper itself is nested two deep — `Box.Arc:2` = `Box<Arc<T>>`, delegation `***self`. Write `:2` only for nested wrappers; single wrappers need nothing.

> **By-value receivers**: `fn consume(self)` forwards as `(*self).consume()` — a by-value `self` IS the wrapper, one deref fewer (`&self` methods use `(**self)`: through the reference, then the wrapper). Moving out cannot type-check for shared wrappers (`&`/`Rc`); the generated impls carry a `#[doc]` note (proc macros have no stable warning channel, E0658). Skip such methods with `@all_ref_methods` (the trait default stays) or hand-write `#name{...}`.

#### GATs, `Self`, and unsized targets (0.9.4)

**Generic associated types** are delegated by projection with their own
params — `trait Iterable { type Iter<'a> where Self: 'a; }` becomes
`type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;` (the bare
projection would be missing the lifetime argument, E0107). Plain assoc
types/consts keep their existing `<T as Trait>::Item` projection.

**Bare `Self`** in a method's parameters or return cannot be blanket-
delegated (the forward emits the inner type, which cannot match the
wrapper's `Self`) — a targeted error with a `#name{...}` suggestion. A
`Self::Assoc` **return** (`fn iter(&self) -> Self::Iter`) passes — the
inner `T` carries the same associated type.

**`@?` unsized suffix**: a wrapper ending in `@?` (`Box@?`) adds `T: ?Sized`
to that spec's where clause, so the fresh generic can be an unsized target:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynLen { fn dlen(&self) -> usize; }
impl DynLen for str { fn dlen(&self) -> usize { self.len() } }
// → impl<T: DynLen + ?Sized> DynLen for Box<T> — T (and thus the target) may be unsized
```

#### `@Cow` — a constraint-carrying packing (the case study)

`@Cow` is a **built-in `#blanket` wrapper constant** (usable only in the
`#blanket` wrapper list). `Cow<'_>`'s deref target is `T::Owned`, not `T` —
the naive `(**self)` delegation can't pass type checking. `@Cow` packs
`Cow<'_>` **plus** the inherent constraint predicates (`@0: ToOwned + ?Sized,
`@0::Owned: @trait`), making it blanket-usable. This is the demonstration that
**a constant carries reuse value only when it carries constraints**:

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowLen { fn clen(&self) -> usize; }
impl CowLen for str { fn clen(&self) -> usize { self.len() } }
impl CowLen for String { fn clen(&self) -> usize { self.len() } }
// → impl CowLen for Cow<'_, str> ... / Cow<'_, String> ...（delegates via the packed predicates）
```

### 7.5 Open extension

An unknown `#name(args){body}` becomes a top-level macro call — DSL fills the spec body, you write the rest. **The deliverable of this extension point is the protocol shape itself**: batch-impl does not implement your codegen, it only guarantees the four-part input `{spec}(args){body}trait_def` reaches your same-named macro.

```rust,ignore
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test;
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self+3} trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }}})]
trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }
```

> **The protocol has converged to one shape**: the legacy **in-impl form** `T {m!{...}}` (no `!`, the call lands in the impl body as associated items) is **deprecated** since 0.7.2 (kept for compatibility — no warning channel exists, so the deprecation lives in the docs). Write new extensions against the top-level `{! m!{...}}` four-segment protocol `{spec}(args){body} trait` only.

## 8. `where` Clauses

### 8.1 `where` predicates

The `where` clause attaches predicates to the impl. The preferred spelling is
bare — `where predicate { code block }` with the code block after the
predicate, or **no code block at all** (`where A: Clone` ≡ `where A: Clone {}`
— the predicate region ends at the spec end); a `where{...}` suffix
(predicates in braces) is equivalent and still works, but writes one `{}`
layer more:

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u8> where Vec<u8>: Clone)]
trait T {}
```

### 8.2 Bare `where predicate {code block}`

Rust-style constraint/body separation (the `{...}` code block after the predicate is required):

> Equivalently, `where{predicates} {code block}` (the §8.1 suffix + a chained body) can be written bare as `where predicates {code block}`, saving one `{}` layer.

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 where u8: Clone { fn tag(&self) -> &'static str { "u8" } })]
trait T { fn tag(&self) -> &'static str; }
```

### 8.3 Predicate inheritance

Trait-level `where` clauses inherit into the impl; renaming/composite predicates referencing undeclared params error explicitly.

### 8.4 The `impl{...}` shape templates (0.8.0)

**The idea in one sentence: pattern matching + text substitution.** You write
one `impl{...}` block holding a **prototype type**, and the macro *matches*
it against each leaf target type — positions that **match** (same ident) stay
as-is, positions that **differ** become named slots, and those slot names are
*substituted* into the body (and where predicates). One body, adapted to
every leaf:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] u32 impl{W<T>} { fn mk(x: u32) -> W<T> { W::new(x) } })]
trait Make { fn mk(x: u32) -> Self; }
// → impl Make for Box<u32> { fn mk(x: u32) -> Box<u32> { Box::new(x) } }
// → impl Make for Rc<u32>  { fn mk(x: u32) -> Rc<u32>  { Rc::new(x) } }
```

How the match works, in plain terms:

- **The template is a pattern** over the leaf type, compared position by
  position: `impl{W<T>}` against the leaf `Box<u32>` → `W` ≠ `Box` so `W`
  is a slot (`W := Box`), `T` ≠ `u32` so `T` is a slot (`T := u32`).
  Against `Rc<u32>` → `W := Rc`, `T := u32`. The template itself is
  **not an impl target** — it only declares the slots.
- **The slots are substituted** — every occurrence of `W`/`T` in the body
  (and in where predicates) is replaced with the bound leaf part. That is
  the whole mechanism: pattern-match the leaf, collect slots, substitute.
- Bare `impl{T}` binds the **whole leaf** (`impl{T}` + `i32` → `T := i32`);
  `impl{Rc<T>}` + `Rc<i32>` → only `T := i32` (`Rc` matched, kept).
- Multiple `impl{...}` in one attribute merge into one mapping — identical
  re-bindings are legal, conflicting ones error.
- `@trait` inside the template expands to the trait path before matching.

The template holds a **standard Rust type** — DSL operators are rejected
inside it; `_` is a **wildcard** that matches anything and stays `_` (see
the template-matching table below).

#### Template matching: what binds and what does not

The template is matched against the leaf by **structural recursion** — every
`syn::Type` form is recognized and recursed into:

| Template form | Behaviour |
|---|---|
| `T` (bare ident) | binds the whole leaf subtree |
| `Rc<T>` / `std::rc::Rc<T>` (path, multi-segment ok) | base/segment idents: equal → literal, different → slot; generic args recurse |
| `&A` / `&mut A` / `*const A` / `*mut A` | the reference/pointer lifetime & mutability are structural; the element binds |
| `[A]` (slice), `(A, B, C)` (tuple) | elements bind position by position |
| `[A; 3]` (fixed array, literal length) | the length compares verbatim; the element binds |
| `[A; N]` (fixed array, const-param length) | the length **binds** to the leaf's length (`N := 3`; the body may use `N`) |
| `[A; ()]` | **reserved shape** — the internal variadic-segment marker (an array length of `()` cannot exist in compilable code); do not write it in a template by hand |
| `Cow<'_, A>` (lifetime arg) | `'_'` is a **wildcard** matching any lifetime; `'a` vs `'b` compares verbatim; the type arg binds |

Not bindable (kept as verbatim comparison — a targeted diagnostic instead of
a silent mis-bind):

- **slots inside fn-pointer / trait-object templates** (`fn(A) -> B`,
  `dyn A + Send`): these forms are compared verbatim — only an identical
  template matches itself;
- **cross-class argument binding** (`Cow<'_, A>` vs a 1-arg `Box<u8>` leaf;
  `Foo<A>` vs `Foo<3>`): a lifetime/const argument cannot bind to a type
  argument, and mismatched arities cannot align. Write one prototype template
  per shape family instead (below).

#### The prototype-impl pattern

Write **one correct implementation for a representative leaf**, and the
"equal → keep, different → bind" rule adapts it to every leaf of the matrix:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] @num impl{Box<u8>} #max{Box::new(u8::MAX)})]
trait TMax { fn max() -> Self; }
// → impl TMax for Box<u8>  { fn max() -> Box<u8>  { Box::new(u8::MAX) } }
// → impl TMax for Box<u16> { fn max() -> Box<u16> { Box::new(u16::MAX) } }
// → impl TMax for Rc<f64>  { fn max() -> Rc<f64>  { Rc::new(f64::MAX) } }
```

Each shape family needs its own prototype (a `Cow<'_, u8>` template covers
the Cow family — the lifetime `'_'` wildcard matches any leaf lifetime).
Combine families in one attribute, either as separate specs or as pairs with
a list-wide distribution:

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
# use std::rc::Rc;
#[batch_impl(
    [[Box, Rc] impl{Box<u8>},
     Cow<'_> impl{Cow<'_, u8>}] @num #tag{1}
)]
trait Tag { fn tag() -> usize; }
// Box<u8>..Rc<f64> covered by the Box<u8> prototype; Cow<'_, u8>..Cow<'_, f64>
// covered by the Cow prototype — one attribute, two shape families
```

#### Variadic segments and repeat blocks

An `impl{...}` template can declare a **variadic segment** with `ident@..`:
it covers every remaining tuple position from its own position onward (a
segment written after fixed elements starts at their count). A trailing
segment needs **no comma** — `impl{(A@..)}` and `impl{(A@..,)}` are
equivalent (0.9.2; the trailing comma is supplied automatically so the
template still parses as a tuple). The segment's elements are addressed by
their absolute leaf position, but they carry **no derived names** — writing
`@A..` does not occupy or declare `A1`, `A2`, ... anywhere. To name a
specific element, write it as an ordinary fixed element next to the segment
(`impl{(A0, @A..,)}` binds `A0 := ` leaf\[0\] through the normal slot channel,
and the body references the plain ident `A0`). Same-level segments split the
leaf evenly (`(A@.., B@..,)` on an arity-4 leaf → A len 2, B len 2); an
uneven split errors. Segments recurse into nested tuples
(`((A@..,),(B@..,))`), and duplicate segment prefixes in one template error.

The body repeats with `@(...)..` — a repeat block emitted once per element
of the segment(s) it references (the `$( ... )*` semantics of Rust's
declaration macros: each round splices the actual bound element):

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..)} { fn tail(&self) -> (u8, u16, u32) { (@(@A::from(self.@0)),..) } })]
trait ShapeTail { fn tail(&self) -> (u8, u16, u32); }
// body → (u8::from(self.0), u16::from(self.1), u32::from(self.2))
```

- `@ident` inside a block is an **element reference** — round `i` splices
  the segment's i-th bound leaf element **directly** into the output (no
  intermediate spelling exists between the expansion and the rendered impl);
- a fixed template element written next to the segment (`A0` in
  `impl{(A0, @A..,)}`) is an ordinary slot: the body writes the plain ident
  `A0` wherever the named element is needed;
- `@N` is an **index cursor** — the numeric literal `N + i`; write the path
  prefix yourself (`self.@1` for a segment starting at leaf index 1);
- the block repeats `L` times, and the length comes from one of three
  sources: the segments referenced inside (`@ident`, all equal-length),
  a **declared driver** (`@A(self.@0,)..` — the segment named right after
  `@`, useful for cursor-only bodies), or — for a cursor-only block with no
  declared driver — the template's **unique segment** (an arity-shape with
  several segments rejects the ambiguous cursor-only form);
- each repeat block's trailing `,` is the separator, emitted after every
  round — write no comma *between* side-by-side blocks (every block already
  emits its own element separators); alternatively write the separator
  between `)` and `..` (`@(x),..`) so it is emitted only *between* rounds,
  never after the last one; commas inside the `{...}` code block follow
  ordinary Rust rules — the DSL separator above applies only to repeat
  blocks, not to code bodies;
- nested blocks run independent rounds (Cartesian semantics) — the output
  is the product of the nesting levels' round counts, capped at 65536
  output tokens per body (`repeat-block expansion produces N tokens (limit
  65536)` beyond that);
- outside a block, `@` in a body is an error; the segment elements cannot be
  spelled `@{...}` — that form holds **fresh position references** only:
  `@{0}` is the impl's first fresh generic (display name `P0`). A `@{N}`
  in the body requires a declared body slot — `impl{@{}}`, or the
  fresh-binding switch `impl{@0..}` whose rounds consume `@{N}` (the
  "declare what you use" rule). `@{@N}` is the **per-round** form: the
  cursor `@N` becomes `N + round`, so a cursor-only block names each
  round's own fresh — `(@(@{@N}::foo()),..)` on three freshs expands to
  `(P0::foo(), P1::foo(), P2::foo())`.

A cursor-only block generates element references without naming the types —
the tuple-to-tuple re-shaping case:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..)} { fn elems(&self) -> (u8, u16, u32) { (@(self.@0,)..) } })]
trait ShapeElems { fn elems(&self) -> (u8, u16, u32); }
// body → (self.0, self.1, self.2)
// (the single-segment template supplies the length; `@A(self.@0,)..` is the
//  explicit spelling, also valid for multi-segment templates)
```

The alga2-style end-to-end — one spec covers every tuple arity, with
`@0..` (≡ `@all_fresh`) constraining every fresh generic:

```rust
# use batch_impl::batch_impl;
trait Magma { fn combine(&self, rhs: &Self) -> Self; }
impl Magma for u8 { fn combine(&self, rhs: &Self) -> Self { *self + *rhs } }
#[batch_impl(
    ()1..=2 where @0..: Magma impl{(A@..)}
    #combine{( @(@A::combine(&self.@0, &rhs.@0),).. )}
)]
trait TupleMagma { fn combine(&self, rhs: &Self) -> Self; }
// → impl<P0> TupleMagma for (P0,) where P0: Magma { ... }
// → impl<P0, P1> TupleMagma for (P0, P1) where P0: Magma, P1: Magma { ... }
```

### 8.5 The impl entry (0.8.0, ItemImpl)

**Same idea, bigger template: the whole impl block becomes the pattern.**
Instead of a separate `impl{...}` attachment, you hand `#[batch_impl]` an
ordinary `impl` block whose for-Type holds the placeholder slots
(`impl Make for A<B>`), plus a `template : matrix` source. Every matrix leaf
is matched against the for-Type (`A<B>`), the slots (`A := Box, B := usize`)
are substituted into the whole block — for-Type, where predicates and body —
and one impl per leaf is emitted. The original impl (holding the slots) is
withheld:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# trait Make { fn make() -> Self; }
#[batch_impl(A<B> : [Box, Rc] [usize, isize])]
impl Make for A<B> { fn make() -> A<B> { A::new(B::default()) } }
// → impl Make for Box<usize> { fn make() -> Box<usize> { Box::new(usize::default()) } }
// → ... × 4
```

In one sentence: **write one impl with placeholders, get one impl per matrix
cell — the same match-and-substitute as §8.4, applied to the whole block
instead of just a body.**

- Attr grammar: shape form `A<B> : [Box,Rc] [usize,isize]` (template `:` matrix)
  or the direct form `<T> Box<T>` (generic declaration + for-type, N = 1);
  `;` separates multiple specs (`W:u8; W:u16`), the single-spec case is the
  common one;
- `@trait` (→ the impl's trait path) is allowed in generic-decl bounds and
  where predicates; custom `@` constants, `@N`/`@g_i` refs and `#` directives
  are rejected on this entry;
- the impl's own generics / where clause / `unsafe` are preserved; the bare
  where region also ends at a depth-0 `;` or the end of the stream.

## 9. Tuple Generation and Matrices

### 9.1 Tuple generators

`(T,)N` generates tuples of length 1..=N; `()N` generates N fresh params:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)3)]
trait T {}
// → impl T for (u8,) {} / (u8, u8) / (u8, u8, u8)
```

### 9.2 Cartesian products

`[A, B] [C, D]` full combinations; `*(A,B)2` splat pow produces a Cartesian combo list:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] [u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>（4 entries）
```

Matrices can be wrapped into containers or const-generic fixed arrays (`([u8, u16],)2` etc.).

## 10. The Modifier Gallery

| Modifier | Meaning | Example |
|---|---|---|
| `&` / `&mut` | reference | `& Box<T>` — nest a composed type with `.`: `&.Box u8` = `&Box<u8>` |
| `*const` / `*mut` | raw pointer | `*const T` = `*const T` |
| `unsafe` | unsafe fn / unsafe impl marker | `unsafe.fn.(A, B) C` = `unsafe impl ... for fn(A, B) -> C` |
| `#[...]` attributes | attribute on the impl | `#[cfg(...)]` gating |
| `!` | never as a fn return type | `fn(A) -> !` (a `!` block has no apply meaning; a trailing `{...}` belongs to the impl) |
| `self` | identity prefix | `self T` = `T` — in a matrix, `[Box, self] u8` = `Box<u8>` + bare `u8` |

**`self`** is the identity prefix: `self T` = `T`. In a matrix it acts as a **bare-type placeholder** — `[Box, self] u8` generates both `Box<u8>` and the bare `u8`:

```rust
# use batch_impl::batch_impl;
#[batch_impl([Box, self] u8 { fn tag(&self) -> &'static str { "x" } })]
trait WrapOrBare { fn tag(&self) -> &'static str; }
// → impl WrapOrBare for Box<u8> {} / impl WrapOrBare for u8 {}
```

> **`!` (never) as a fn return type**: `fn(A) -> !` is legal — the `!` block has no apply meaning, and a trailing `{...}` belongs to the impl:

```rust
# use batch_impl::batch_impl;
#[batch_impl(fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } })]
trait NeverRet { fn call(&self, x: u8) -> !; }
// → impl NeverRet for fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } }
```

> **`unsafe` has two roles** — `unsafe fn(A) -> B` is an *unsafe fn type*: the impl itself stays safe (`impl Tr for unsafe fn(A) -> B`). To mark the **impl** unsafe, apply `unsafe` with `.`: `unsafe.fn(A) -> B` = `unsafe impl Tr for fn(A) -> B`. If you find yourself writing `unsafe fn(...)` and expecting an unsafe impl, that is the wrong form.

**Arbitrarily nested types are native**: `HashMap<String, Vec<(u8, u16)>>`, `Result<Box<dyn Fn(u8) -> u16>, String>` etc. write and parse directly — the DSL covers nearly every type form, no "passthrough" needed.

## 11. Three Entry Points

- **`#[batch_impl]`** — annotates the trait definition, re-emits it and generates impls (one trait per macro).
- **`#[batch_impl_only]`** — generates impls only, the trait comes from outside (for traits you don't own, or already declared). A `# path::To::Trait:` prefix declares the external trait's real path (requires at least one `::`; `@trait` and path references then use it):

```rust
# use batch_impl::batch_impl_only;
# mod path { pub mod to { pub trait Conv<T> { fn conv() -> T; } } }
# struct Wrapper<T>(T);
#[batch_impl_only(# path::to::Conv: Conv<bool> Wrapper<bool> #conv{false})]
trait Conv<T> { fn conv() -> T; }
// → impl Conv<bool> for Wrapper<bool> { fn conv() -> bool { false } }（trait not re-emitted）
```

- **`batch_trait!`** — a function-like macro for an already-declared trait, multi-section support, custom `@name=value;` constant sections, no directives.
- **The impl entry (0.8.0, ItemImpl)** — `#[batch_impl]` also accepts an `impl` block: batch-instantiate a hand-written impl from a shape template × matrix source (see §8.5).

## 12. Error Hints

batch-impl's errors are **compile-time diagnostics** pointing at the user-visible token closest to the root (macro-generated artifacts fall back to the macro-call line):

- **Missing operand**: `A.` / `.A` / `,A` — `compile_error!` with a clear message
- **Unknown `@` constant**: lists the built-in names (`@u*`/`@i*`/`@f*`/`@scalar`/`@num` + range families)
- **Constant cycle/forward reference**: rejected at definition (prevents infinite recursion)
- **`@N`/`@g_i` out of range or dangling**: `@5` beyond the impl's generated generic count / `@2_0` group missing — targeted errors in user language (the fresh generics are numbered from 0 in document order); the generated names are the user-visible display names (`P0`, `P1`, ...) and the reference is intercepted in the macro — never a raw rustc E0412
- **Splat as a where-predicate subject**: explicitly rejected (`A, B: Trait` has no defined semantics)
- **Generic rename breaks inheritance**: renaming a trait generic param = explicit error, never silent
- **Bare `*` (neither splat nor pointer)**: targeted error instead of rustc raw-pointer confusion
- **Empty range** (`@u16..u8`): "no impls generated for empty range"
- **`=`/`:` in concrete-type args**: bindings/bounds are trait-path/declaration-only — targeted error (`Assoc<Item = u32>` with a struct reports "binding args are only valid on a trait path")
- **Stray `;`/`=`/`@`/`#`/`-` in a type position**: targeted error (the `=` of `..=` excluded — no cascading second diagnostic; a lone `-` is the retired operator — the exclusion lives only in directive lists)
- **Trailing tokens after an `fn` parameter list**: `fn(A) B` / `fn(A)->` — unexpected-token error (a return type is `-> B` or `fn(A) B`)
- **Blanket method takes/returns bare `Self`**: `#blanket` cannot delegate a
  method taking or returning bare `Self` (forwarding yields the inner type,
  not the wrapper's `Self`) — error with a `#name{...}` suggestion. A
  `Self::Assoc` **return** (`fn iter(&self) -> Self::Iter`) is fine — the
  inner `T` carries the same associated type
- **Empty binding/bound value**: `Conv<Item =>` / `Conv<T:> X` — "missing a value" / "missing a bound"
- **Non-integer type literal**: `1.5` / `"hi"` / `'a'` — only an integer (usize) is a type
- **Non-integer range endpoint**: `1..x` / `A..B` — "needs integer endpoints"
- **Malformed array length**: `[u8; 3; 4]` / `[u8;]` — "missing or malformed"
- **`+` at a type start**: `+A` — "not valid at the start of a type" (`+` belongs in a bound, e.g. `T: Clone + Send`); a leading `.` reports a missing operand; `?`-prefixed types (`?Sized`) pass through and rustc reports them
- **Unknown directive**: no builtin-typo guard — a `#name(args){body}` that is
  not a built-in directive and not a trait item name expands to your
  same-named macro (the open extension); a typo surfaces as rustc's own
  "macro not found"
