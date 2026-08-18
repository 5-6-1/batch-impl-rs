# batch-impl Tutorial

**v0.8.2** (unreleased) — variadic segments (`ident@..`) in `impl{...}` templates and repeat blocks (`@(...)..`) in bodies, see §8.4;

**v0.8.1** — the `where{...}` angle-pairing hotfix (see the CHANGELOG);

**v0.7.2** — 0.7.2 adds the `batch_preview!` expansion preview, generator-splat declaration hoisting in trait args, `#blanket` by-value receiver forwarding, custom `@` constant sections for the attribute macros (reverted in 0.8.0), and user-language `@` diagnostics; 0.7.1 adds targeted diagnostics (stray/adjacent/empty tokens, typo suggestions) instead of raw rustc errors; 0.7.0 adds the **`*` flatten operator** on top of the existing skeleton, and upgrades `<>`/`()`/`[]` from "passive syntax" to "programmable structures": generic-argument positions now accept generators (`()^N`), splats (`*(A,B)`), constant families (`@u*`), lists (`[A,B]`), bindings (`Item=u32`) and nested types.

Progressive DSL learning: from a one-line impl to advanced matrix combinations. All examples are compilable code (the code blocks of this English tutorial double as doctests), and every step's output is plain Rust — the generated impls are token-equivalent to handwritten ones.

## 0. Three systems + one operator

Every capability of batch-impl is built from three pillars (polished continuously from 0.0 to 0.6) plus one operator (0.7.0):

| Part | Notation | Role |
|---|---|---|
| **apply system** | `^` / `-` / `[]` / `()` | Type matrix: apply the left container/modifier to the right type, lists expand into multiple impls |
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

## 2. Type Matrix: `^` and `-`

`^` and `-` are **the same operation**: the left side is a modifier/container, the right side the target type. They differ only in associativity: `^` is right-associative (nesting), `-` is left-associative (accumulating params).

Precedence from low to high: `;` < `,` < `-` < `^`; `()` grouping sits above all operators.

| Writing                    | Expansion                            |
|----------------------------|--------------------------------------|
| `Box^T`                    | `Box<T>`                             |
| `Box^<X,Y>`                | `Box<X, Y>` (multi-param container)  |
| `Box^Box^T`                | `Box<Box<T>>` (right-associative nesting) |
| `HashMap<K>^V`             | `HashMap<K, V>` (prefilled generics appended) |
| `&^Box^T`                  | `&Box<T>` (chained modifiers)        |
| `Vec-u32`                  | `Vec<u32>`                           |
| `HashMap-u32-String`       | `HashMap<u32, String>` (left-associative accumulation) |
| `fn^(A,B)-C`               | `fn(A,B)->C`                         |
| `[Box, Vec]^T`             | `Box<T>, Vec<T>`                     |
| `Box^[T1, T2]`             | `Box<T1>, Box<T2>`                   |
| `[Box, Vec]^[T1, T2]`      | Cartesian product, 4 entries         |
| `[HashMap<K>, Vec<K>]^V`   | `HashMap<K, V>, Vec<K, V>`           |

> **Note**: `Box^Vec-u32` is wrong (it parses as `Box<Vec, u32>`); write `Box^Vec^u32` instead. When you miswrite it, rustc's E0107 error prints the rendered `Box<Vec, u32>` verbatim — the mistake is self-evident.

> **Operand strictness**: both sides of `^`/`-`/`,` must have operands — `A^`, `^A`, `-A`, `,A`, `A,,B` all report `compile_error!`; only **trailing commas** (`A,` / `[A, B,]`) are allowed, and `()`/`[]` brackets are real tokens, not empty operands. `;` stays lenient as a `batch_trait!` section boundary.

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(Box^Vec^u32, HashMap<u8>^String)]
trait T {}
// → impl T for Box<Vec<u32>> {}
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

Rule: `[A, B]` inside a tuple/generic-arg position → Cartesian-product distribution (all combinations of multiple arrays); nested arrays recurse to leaves (`Vec<[[A,B], C]>` → `Vec<A>`/`Vec<B>`/`Vec<C>`); combos of `(X, [A,B])^N` containing arrays are covered by the outer distribution. Note: concrete generators combined with fresh generators may overlap (E0119 — same fresh count/structure); rustc catches it — use generators with different fresh counts to avoid.

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
| `f(*args)` | `T-*(A, B, C)` — unfold a generator into argument positions |
| one level of unpack | `*((a,b),)` = one `(a,b)` impl (tuples stay intact) |

**Motivation**: `*` compresses a nested generator into a multi-arg container. Instead of hand-writing `T-[A,B,C]-[A,B,C]-[A,B,C]` (27 combos of nested lists), one line gives the same 27 impls:

```rust
# use batch_impl::batch_impl;
struct T<A, B, C>(A, B, C);   // 3-arg container
struct A; struct B; struct C;
#[batch_impl(T-*(A, B, C)^3)]  // splat-pow: unfold (A,B,C)^3 into three arg positions
trait Matrix27 {}
// → 27 impls: T<A,A,A> / T<A,A,B> / ... / T<C,C,C>（same as T-[A,B,C]-[A,B,C]-[A,B,C]）
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

`[]` is a **set** and `()` is a **sequence** — splat just mirrors the source bracket, so `*[A,B]^T` distributes (each element applies `T`, keeping set semantics) and `*(A,B)^T` appends (keeping list semantics). This is not a new rule; it preserves the underlying container's behavior, and `TySplat::Array`/`TySplat::Tuple` mirror `TyArray`/`TyTuple`.

```rust
# use batch_impl::batch_impl;
#[batch_impl(*[Vec, Box]^u8)]        // array splat distributes: each element ^u8
trait T1 {}
// → impl T1 for Vec<u8> {} / Box<u8>

#[batch_impl(*(Vec<u8>, Box<u8>)^u16)]  // tuple splat appends: the right operand joins
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
#[batch_impl(Frac<*(*@u*)^2>)]
trait Pow {}
// → impl Pow for Frac<u8, u8> {} ... impl Pow for Frac<usize, usize> {}（36 impls）
```

### 4.4 Container rule

A group whose content is a lone splat parses as the container holding the splat as one element — `(*(a,b))` = `( *(a,b) )`, `[*(a,b)]` = `[ *(a,b) ]`; the splat element expands only in codegen.

### 4.5 Generator re-wrap

`*(()^N)` — a generator splat — hoists fresh declarations and splats the tuple into a container:

```rust
# use batch_impl::batch_impl;
struct Pair3<A, B>(A, B);
#[batch_impl(Pair3<*()^2>)]
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

#[batch_impl(Wrap<()^2>)]               // generator: <P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1)>（the tuple stays a single arg）

#[batch_impl(Pair3<*()^2>)]             // generator splat: <P0,P1> Pair3<P0,P1>
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
#[batch_impl(<T> Box<T> where{Box<T>: Clone})]
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

**Name families** (a closed set — the language-defined type collections): `@u*`, `@i*`, `@f*`, `@num`, `@scalar`.

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box^@u*)]  // Box applied to every member of @u*
trait BoxRc {}
// → impl BoxRc for Box<u8> {} / Box<u16> / ... / Box<usize>
```

**Range families**: `@u8..u128`, `@i8..i128`, `@f32..f64` (inclusive). `usize`/`isize` only enter name families, not range families.

### 6.2 Lazy expansion and references

Constant values are stored as **verbatim tokens**; reference sites splice and expand recursively — a value can be a DSL expression (`@uints=@uint`) or a chained reference (`@a=@b`). Cycles/forward references are rejected at definition (preventing infinite recursion); a bare range endpoint reference (`@a=@u8` without `..`) errors at definition.

### 6.3 Custom constant sections (`batch_trait!` only)

A leading `@name=value;` section defines reusable constants (values may chain
references and embed DSL expressions). **`#[batch_impl]` / `#[batch_impl_only]`
do not support custom constants** — the 0.7.2 feature was reverted in 0.8.0;
write attribute-macro matrices directly with `^`/`-`/`*` instead:

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

| Notation | Derivation | Meaning |
|---|---|---|
| `@g_i` | **primitive** — group g, slot i (stable across array distribution) | addresses a macro-generated generic (groups/slots number from 0; dangling refs are targeted errors) |
| `@N` | `@g_i` flattened by document order within one impl | references a fresh generic (`where{@0: Clone}`) |
| `@all_fresh` | all fresh generics | range sugar — "every one" |
| `@N..=M` | a contiguous run | range sugar — `@0..=1` = `@0, @1` |

> **Power-user tier**: `@g_i` / `@all_fresh` / `@N..M` are advanced addressing notations — start from `@u*` / `@all_methods` / `@0` and reach for them only when a predicate must name a specific fresh. The whole DSL surface is frozen since 0.7.2 (see README); these notations will not change semantics again.

```rust
# use batch_impl::batch_impl;
#[batch_impl(()^2 where{@0..=1: Clone})]   // range sugar: @0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1) where P0: Clone, P1: Clone

#[batch_impl(()^3 where{@all_fresh: Copy})] // every fresh generic
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2) where P0: Copy, P1: Copy, P2: Copy
```

On the other axis (value classes):

| Notation | Class | Use |
|---|---|---|
| `@trait` | **identity** — the current trait name/path (section-level in batch_trait) | package "generic declaration + trait name" across sections |
| `@all_methods` etc. | **selection** — extract an item set from trait_def | `#fill(@all_required_methods, -foo)` precise selection |
| `@Cow` etc. custom | **package** — a type plus its inherent constraints | reuse a "constrained wrapper" (see §7.4) |

`@all` family combined with `-` subtraction selects arbitrary item subsets (`#fill(@all_required_methods, -foo)`); `@all_default*` / `@all_required*` distinguish default implementations from required methods.

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
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

### 7.4 `#blanket(@all_methods){wrapper matrix}` — blanket delegation

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]
trait NumOps { fn inc(&mut self); }
impl NumOps for u32 { fn inc(&mut self) { *self += 1 } }
// → impl NumOps for Box<u32> { fn inc(&mut self) { (**self).inc() } }（delegates to the wrapped u32）
```

> **By-value receivers**: `fn consume(self)` forwards as `(*self).consume()` — a by-value `self` IS the wrapper, one deref fewer (`&self` methods use `(**self)`: through the reference, then the wrapper). Moving out cannot type-check for shared wrappers (`&`/`Rc`); the generated impls carry a `#[doc]` note (proc macros have no stable warning channel, E0658). Skip such methods with `@all_ref_methods` (the trait default stays) or hand-write `#name{...}`.

#### `@Cow` — a constraint-carrying packing (the case study)

`Cow<'_>`'s deref target is `T::Owned`, not `T` — the naive `(**self)` delegation can't pass type checking. `@Cow` packs `Cow<'_>` **plus** the inherent constraint predicates (`@0: ToOwned + ?Sized, @0::Owned: @trait`), making it blanket-usable. This is the demonstration that **a constant carries reuse value only when it carries constraints**:

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

### 8.1 `where{...}` suffix

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u8> where{Vec<u8>: Clone})]
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

### 8.4 `impl{...}` Self-part shape templates (0.8.0, Ext 2)

A third trailing attachment beside `where{...}` and `{body}` — the Self-part
shape template. The three kinds attach in **any order**. The block holds a
**standard Rust type** (DSL operators are rejected): it is matched against
the leaf target type **position by position**, and an ident that **equals**
the target's ident at that position is a literal (kept as-is), while a
**different** one is a binding slot — rewritten in the target type, the where
predicates and the body. One body, adapted to every leaf:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc]^u32 impl{W<T>} { fn mk(x: u32) -> W<T> { W::new(x) } })]
trait Make { fn mk(x: u32) -> Self; }
// → impl Make for Box<u32> { fn mk(x: u32) -> Box<u32> { Box::new(x) } }
// → impl Make for Rc<u32>  { fn mk(x: u32) -> Rc<u32>  { Rc::new(x) } }
```

- `impl{T}` + `i32` → `T := i32` (a bare ident template binds the whole leaf);
- `impl{Rc<T>}` + `Rc<i32>` → `T := i32` (`Rc` is equal → literal);
- `impl{Rc<T>}` + `Box<i32>` → `Rc := Box, T := i32` (different base → slot);
- multiple `impl{...}` merge into one mapping — identical re-bindings are
  legal, conflicting ones error (`impl{X}` binds the whole leaf, `impl{X<u32>}`
  binds the base — `InconsistentBinding`);
- the attachment depth limit counts `impl{...}` like the other kinds;
- `@trait` inside the template expands to the trait path before matching.

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
#[batch_impl([Box, Rc]^@num impl{Box<u8>} #max{Box::new(u8::MAX)})]
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
     Cow<'_> impl{Cow<'_, u8>}]^@num #tag{1}
)]
trait Tag { fn tag() -> usize; }
// Box<u8>..Rc<f64> covered by the Box<u8> prototype; Cow<'_, u8>..Cow<'_, f64>
// covered by the Cow prototype — one attribute, two shape families
```

#### Variadic segments and repeat blocks

An `impl{...}` template can declare a **variadic segment** with `ident@..`:
it covers every remaining tuple position from its own position onward (a
segment written after fixed elements starts at their count). The segment's
names are **aligned with the leaf position** — `(u8, A@..,)` on
`(u8, u16, u32)` yields `A1`, `A2` (there is no `A0`; the index cursor
starts at `@1`), while `(A@..,)` on `(u8, u16, u32)` yields `A0`, `A1`,
`A2`. Same-level segments split the leaf evenly (`(A@.., B@..,)` on an
arity-4 leaf → A len 2, B len 2); an uneven split errors. Segments recurse
into nested tuples (`((A@..,),(B@..,))`), and duplicate segment prefixes in
one template error.

The body repeats with `@(...)..` — a repeat block emitted once per element
of the segment(s) it references:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..,)} { fn tail(&self) -> (u8, u16, u32) { (@(@A::from(self.@0),)..) } })]
trait ShapeTail { fn tail(&self) -> (u8, u16, u32); }
// body → (A0::from(self.0), A1::from(self.1), A2::from(self.2))
//        → (u8::from(self.0), u16::from(self.1), u32::from(self.2))
```

- `@ident` inside a block is a **name reference** — the i-th element's slot
  name (`A0`, `A1`, ...), which the slot mapping then rewrites to the bound
  leaf element;
- `@N` is an **index cursor** — the numeric literal `N + i`; write the path
  prefix yourself (`self.@1` for a segment starting at leaf index 1);
- the block repeats `L` times where `L` is the common length of the segments
  it references (all referenced segments must be equal-length, else error);
- the block body's trailing `,` is the separator, emitted after every round —
  write no comma *between* side-by-side blocks (each block already
  terminates its own elements);
- nested blocks run independent rounds (Cartesian semantics);
- outside a block, `@` in a body is an error.

The alga2-style end-to-end — one spec covers every tuple arity, with
`@all_fresh` constraining every fresh generic:

```rust
# use batch_impl::batch_impl;
trait Magma { fn combine(&self, rhs: &Self) -> Self; }
impl Magma for u8 { fn combine(&self, rhs: &Self) -> Self { *self + *rhs } }
#[batch_impl(
    ()^1..=2 where{@all_fresh: Magma} impl{(A@..,)}
    #combine{( @(@A::combine(&self.@0, &rhs.@0),).. )}
)]
trait TupleMagma { fn combine(&self, rhs: &Self) -> Self; }
// → impl<A0> TupleMagma for (A0,) where A0: Magma { ... }
// → impl<A0, A1> TupleMagma for (A0, A1) where A0: Magma, A1: Magma { ... }
```

### 8.5 The ItemImpl entry (0.8.0, Ext 1)

`#[batch_impl]` also accepts an **`impl` block**: the DSL describes a
**shape template × matrix source**, every matrix leaf emits one impl, and
the slot mapping (the same "equal → keep, different → bind" rule as
`impl{...}`) rewrites the for-Type / where predicates / body. The original
impl (whose for-Type holds the placeholder slots) is withheld:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# trait Make { fn make() -> Self; }
#[batch_impl(A<B> : [Box, Rc]^[usize, isize])]
impl Make for A<B> { fn make() -> A<B> { A::new(B::default()) } }
// → impl Make for Box<usize> { fn make() -> Box<usize> { Box::new(usize::default()) } }
// → ... × 4
```

- Attr grammar: shape form `A<B> : [Box,Rc]^[usize,isize]` (template `:` matrix)
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

`(T,)^N` generates tuples of length 1..=N; `()^N` generates N fresh params:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)^3)]
trait T {}
// → impl T for (u8,) {} / (u8, u8) / (u8, u8, u8)
```

### 9.2 Cartesian products

`[A, B]^[C, D]` full combinations; `*(A,B)^2` splat pow produces a Cartesian combo list:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc]^[u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>（4 entries）
```

Matrices can be wrapped into containers or const-generic fixed arrays (`([u8, u16],)^2` etc.).

## 10. The Modifier Gallery

| Modifier | Meaning | Example |
|---|---|---|
| `&` / `&mut` | reference | `&^Box^T` = `&Box<T>` |
| `*const` / `*mut` | raw pointer | `*const^T` = `*const T` |
| `unsafe` | unsafe fn | `unsafe^fn^(A,B)-C` |
| `#[...]` attributes | attribute on the impl | `#[cfg(...)]` gating |
| `!` | never type | `!^T` |

## 11. Three Entry Points

- **`#[batch_impl]`** — annotates the trait definition, re-emits it and generates impls (one trait per macro).
- **`#[batch_impl_only]`** — generates impls only, the trait comes from outside (for traits you don't own, or already declared):

```rust
# use batch_impl::batch_impl_only;
# struct Wrapper<T>(T);
# trait Conv<T> { fn conv() -> T; }
#[batch_impl_only(Conv<bool> Wrapper<bool> #conv{false})]
trait Conv<T> { fn conv() -> T; }
// → impl Conv<bool> for Wrapper<bool> { fn conv() -> bool { false } }（trait not re-emitted）
```

- **`batch_trait!`** — a function-like macro for an already-declared trait, multi-section support, custom `@name=value;` constant sections, no directives.
- **ItemImpl entry (0.8.0, Ext 1)** — `#[batch_impl]` also accepts an `impl` block: batch-instantiate a hand-written impl from a shape template × matrix source (see §8.5).

## 12. Error Hints

batch-impl's errors are **compile-time diagnostics** pointing at the user-visible token closest to the root (macro-generated artifacts fall back to the macro-call line):

- **Missing operand**: `A^` / `^A` / `,A` — `compile_error!` with a clear message
- **Unknown `@` constant**: lists the built-in names (`@u*`/`@i*`/`@f*`/`@scalar`/`@num` + range families)
- **Constant cycle/forward reference**: rejected at definition (prevents infinite recursion)
- **`@N`/`@g_i` out of range or dangling**: `@5` beyond the impl's generated generic count / `@2_0` group missing — targeted errors in user language, no reserved `_Param_*_BatchGen_` names leaked (and no raw rustc E0412 either)
- **Splat as a where-predicate subject**: explicitly rejected (`A, B: Trait` has no defined semantics)
- **Generic rename breaks inheritance**: renaming a trait generic param = explicit error, never silent
- **Bare `*` (neither splat nor pointer)**: targeted error instead of rustc raw-pointer confusion
- **Empty range** (`@u16..u8`): "no impls generated for empty range"
- **`=`/`:` in concrete-type args**: bindings/bounds are trait-path/declaration-only — targeted error (`Assoc<Item = u32>` with a struct reports "binding args are only valid on a trait path")
- **Adjacent types without an operator**: `A B` / `Vec<T>U` / `[A B]` — "missing `^` / `-` / `,`" instead of rendering invalid Rust
- **Stray `;`/`=`/`@`/`#` in a type position**: targeted error (the `=` of `..=` excluded — no cascading second diagnostic)
- **Trailing tokens after an `fn` parameter list**: `fn(A) B` / `fn(A)->` — unexpected-token error (a return type is `-> B` or `-B`)
- **Blanket method returns `Self`**: `#blanket` cannot delegate a method returning `Self`/`Self::Assoc` (forwarding yields the inner type, not the wrapper's `Self`) — error with a `#name{...}` suggestion
- **Empty binding/bound value**: `Conv<Item =>` / `Conv<T:> X` — "missing a value" / "missing a bound"
- **Non-integer type literal**: `1.5` / `"hi"` / `'a'` — only an integer (usize) is a type
- **Non-integer range endpoint**: `1..x` / `A..B` — "needs integer endpoints"
- **Malformed array length**: `[u8; 3; 4]` / `[u8;]` — "missing or malformed"
- **`+`/`?`/`.` at a type start**: `+A` / `?Sized` / `.foo` — "not valid at the start of a type"
- **Unknown-directive typo suggestion**: `#delgate` / `#blanlet` — "did you mean `#delegate`?" (open-extension names farther than 2 stay silent)
