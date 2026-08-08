# batch-impl Tutorial

**v0.7.0** — 0.6.7 released; 0.7.0: the **splat** `*` prefix (flatten containers/generators into lists; left operand `*[...]` distribute / `*(...)` append), array distribution propagation, `#fill` single-item preference (`#name{...}`); 0.6.x: receiver filters, `#blanket` delegation, span diagnostics, generic-parameter families, `@N` fresh references.
(= `T^N`, a const-generic arg like `W<2>`; **breaking** — tuple generation now needs `(T,)^N`), unsuffixed number rendering,
and input-validation guards (consts nesting depth, `#blanket` `:N` cap,
`@all_*` reserved names, empty `:` depth); 0.6.7: per-impl fresh numbering
(`@N` anywhere, incl. the target type itself), `@g_i` grouped references,
top-level open extension (`{! ...}` — the macro receives
`{spec}(args){body}trait` and emits its own impl), `@all_fresh` / `@N..M`
batch where-references, error aggregation.

A progressively-learned DSL: start from a single impl line and work up to advanced matrix composition. All examples are compilable code; the product of every step is ordinary Rust — the impls the macro generates are token-for-token equivalent to handwritten ones.

## 1. Starting from a Single impl

`#[batch_impl(...)]` is annotated on a trait definition; every spec in its arguments generates one impl:

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize, isize, f32, f64)]
trait Numeric {}
// → impl Numeric for usize {}
// → impl Numeric for isize {}
// → impl Numeric for f32 {}
// → impl Numeric for f64 {}
```

The skeleton of a spec:

```text
<impl-generics> TraitName<trait-generics> target type { body }?
```

| Part | Example | When needed |
|------|---------|-------------|
| `<impl generics>` | `<T>`, `<T: Clone>`, `<const N: usize>` | when the impl block needs generic parameters |
| `TraitName<trait generics>` | `MyTrait<T>`, `MyTrait<Vec<T>>` | when the trait definition has generic parameters |
| target type | `usize`, `Vec<T>`, `&str` | required |
| `{ body }` | `{ fn m(&self) -> usize { 0 } }` | when you need a custom body |

Separate multiple specs with `,`: `#[batch_impl(usize, isize)]`.

## 2. Lists and body

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

**Distribution propagation**: `[A, B]` lists are a dispatch source — besides being a target/operand, nested positions propagate:

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

Rule: an `[A, B]` inside a tuple / generic arg expands by Cartesian product (multiple arrays → full product); nested arrays flatten recursively (`Vec<[[A,B], C]>` → `Vec<A>`/`Vec<B>`/`Vec<C>`); pow_cartesian combos containing arrays are covered by the outer distribution. Note: concrete and fresh generators with the same fresh count/shape can overlap with E0119 — rustc's call; use generators with distinct fresh counts to avoid it.

### Merging per-item and shared bodies

List items can have their own bodies, which merge with a shared body:

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    [usize { fn name() -> &'static str { "usize" } },
     isize { fn name() -> &'static str { "isize" } }]
    { fn zero() -> Self { 0 } }
)]
trait Zero {
    fn zero() -> Self;
    fn name() -> &'static str;
}
// → impl Zero for usize { fn zero() -> Self { 0 } fn name() -> &'static str { "usize" } }
// → impl Zero for isize { fn zero() -> Self { 0 } fn name() -> &'static str { "isize" } }
```

## 3. The `^` and `-` Operators

`^` and `-` are the **same operation**: the left side is a modifier/container, the right side is the target type. They differ only in associativity:
`^` is right-associative (nesting), `-` is left-associative (accumulating arguments).

Precedence, low to high: `;` < `,` < `-` < `^`; `()` grouping sits above all operators.

| Syntax | Expands to |
|--------|------------|
| `Box^T` | `Box<T>` |
| `Box^<X,Y>` | `Box<X, Y>` (multi-parameter container) |
| `Box^Box^T` | `Box<Box<T>>` (right-associative nesting) |
| `HashMap<K>^V` | `HashMap<K, V>` (prefilled generics appended) |
| `&^Box^T` | `&Box<T>` (modifiers chained) |
| `Vec-u32` | `Vec<u32>` |
| `HashMap-u32-String` | `HashMap<u32, String>` (left-associative accumulation) |
| `fn^(A,B)-C` | `fn(A,B)->C` |
| `[Box, Vec]^T` | `Box<T>, Vec<T>` |
| `Box^[T1, T2]` | `Box<T1>, Box<T2>` |
| `[Box, Vec]^[T1, T2]` | Cartesian product, 4 entries total |
| `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>` |

> **Note**: `Box^Vec-u32` is wrong (it would be read as `Box<Vec, u32>`); write `Box^Vec^u32` instead.

> **Operand strictness**: `^`/`-`/`,` require operands on both sides — `A^`, `^A`, `-A`, `,A`, `A,,B`
> all raise `compile_error!`; only a **trailing comma** (`A,` / `[A, B,]`) is allowed. Brackets such as
> `();`/`[]` are real tokens, not empty operands. `;` stays lenient as the `batch_trait!` section boundary.

### Splat (`*` flatten)

The `*` prefix **flattens a container / generator into the enclosing list**
— it appears only before `[]`/`()`:

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8, *[u16, u32, u64]])]
trait SplatList {}
// → impl SplatList for u8 {}
// → impl SplatList for u16 {} / u32 / u64

#[batch_impl((u8, u16, u32)^*(u64, usize, i8))]
trait SplatConcat {}
// → impl SplatConcat for (u8, u16, u32, u64, usize, i8) {}
//   (`^` alone nests — `*` gives flat concatenation)

#[batch_impl((*(()^3)))]
trait SplatGen {}
// → impl<T0, T1, T2> SplatGen for (T0, T1, T2) {}   // generator splat (group → tuple)
```

Semantics: inside a tuple/array, `*X` splices the elements (`[a, *[d,e,f]]` =
`[a,d,e,f]`); as a `^`/`-` right operand, `*X` appends flatly (left tuple →
concat; left generic → multi-arg `Vec^*(a,b)` = `Vec<a,b>`); the **source
bracket drives the left-operand semantics** — `*[A,B]^T` distributes
(`*[A^T,B^T]` — set, mirrors `TyArray`), `*(A,B)^T` appends (`*(A,B,...,T)` —
list, mirrors `TyTuple`). Generic args `Foo<*(a,b)>` = `Foo<a,b>` (multi-arg,
one impl — distinct from `Foo<[a,b]>` dispatch).

Two rules: `T^*(A,B,...)` ≡ `T-A-B-...` (right splat = flat argument append —
equivalent to the `-` chain, regardless of source bracket); left splat by
source — `*[A,B]^T` = `*[A^T,B^T]` (distribution; composing `X^*[A,B]^T` =
`X<A^T,B^T>`, one impl) and `*(A,B)^T` = `*(A,B,...,T)` (append).

Nested splats are idempotent (`*(*[a,b])` = `[a,b]`), empty is a no-op
(`[a, *()]` = `[a]`); `*const`/`*mut` pointers are unaffected
(disambiguated by the following token).

## 4. Generic Declarations

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Vec<T>)]
trait Collection {}
// → impl<T> Collection for Vec<T> {}
```

**Bound syntax convention** (since 0.6.1): `<>` holds only names; bounds all go into `where{...}` —

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Named<T> Vec<T> where{T: Clone} { fn n(&self) -> usize { self.len() } })]
trait Named<T: Clone> { fn n(&self) -> usize; }
```

`<T: Clone>` (inline bound) is still supported (bounds from the trait definition are inherited automatically when none are written), but once the **bound container is uniformly `where`**, merging multiple bounds is just "juxtaposing predicates" (the macro only concatenates tokens, zero analysis) — that is why a blanket's `T: Trait` and the wrapper predicate merge naturally.

### Nested generic merging

Each list item declares its own impl generics, automatically merged into the impl block:

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> { fn describe(&self) -> String; }
// → impl<T>    Describe<T> for Vec<T>
// → impl<T, U> Describe<T> for HashMap<T, U>
```

### const generics

```rust
# use batch_impl::batch_impl;
#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N] {
    fn len_const(&self) -> usize { N }
})]
trait ConstGeneric<const N: usize> { fn len_const(&self) -> usize; }
// → impl<const N: usize> ConstGeneric<N> for [i32; N] { ... }
```

## 5. Generic Automation (the trait definition is the single source of truth)

### `A<>` — copy the trait generics verbatim

An empty argument list means "arguments and bounds all come from the trait definition":

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<> ())]
trait Foo<T: Clone> {}
// → impl<T: Clone> Foo<T> for ()
```

Available only in `#[batch_impl]` / `#[batch_impl_only]` (both need the trait definition); `batch_trait!` has no trait definition, so `A<>` passes through verbatim.

### `A<bounds>` — the same verbatim copying

Pure associated-type bindings (`A<Item=T>`, no positional arguments) likewise copy the positional arguments and keep bindings verbatim:

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<Item=T> ())]
trait Foo<T: Clone> { type Item; }
// → impl<T: Clone> Foo<T> for () { type Item = T; }
```

`A<T, Item=U>` with positional parameters is ordinary DSL syntax (not expanded).

### Same-named inheritance for unwritten bounds

impl parameters correspond to trait parameters "by position in the trait arguments"; a parameter with the same name and no written bound inherits:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> Vec<T> { fn get(&self) -> T { self[0].clone() } })]
trait Foo<T: Clone> { fn get(&self) -> T; }
// → impl<T: Clone> Foo<T> for Vec<T> { ... }
```

Lifetime bounds (`<'a, T>` + `trait Foo<'a, T: 'a>` → `impl<'a, T: 'a>`), `'static`, and mixed bounds (`Clone + 'a`) are all inherited.

### Inheriting trait-level where clauses

The predicates of `trait Foo<T> where T: Clone` are inherited in **all forms**:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> ())]
trait Foo<T: Clone>
where
    T: Ord,
{
}
// → impl<T: Clone + Ord> Foo<T> for ()
```

- **Single-parameter predicates** (`T: Clone`) merge into the bound (inline + where concatenation); the `<T>` and `A<>` forms are equivalent;
- **All other predicates pass through verbatim** into the impl's where clause: `T::Item: Clone`, `Vec<T>: ...`, lifetime predicates (`'a: 'b`), and so on are all covered.

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> ())]
trait Foo<T>
where
    T: IntoIterator,
    T::Item: Clone,
{
}
// → impl<T: IntoIterator> Foo<T> for () where T::Item: Clone
```

### Renaming = an explicit error, never silent

An argument `X` that maps to a parameter `T` (with a bound) under a different name, or an inherited bound/predicate that refers to a parameter name such as `'a`/`U` while the impl does not declare the same name — all raise `compile_error!` with guidance (rename, or write the bound by hand). To use a different name, write `<X: ...>` yourself.

The macro does not interfere with parameters that already have written bounds (whether `T: B` implies `T: Clone` is verified by rustc, e.g. the supertrait relationship `trait B: A`).

## 6. Concise Associated Types

The `Name=value` syntax binds an associated type inside the trait's generic arguments:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Iter<Item=T> Vec<T> {
    fn count(&self) -> usize { self.len() }
})]
trait Iter {
    type Item;
    fn count(&self) -> usize;
}
// → impl<T> Iter for Vec<T> { type Item = T; fn count(&self) -> usize { self.len() } }
```

Multiple associated types and generic constraints are supported:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T, U> Pair<First=T, Second=U> (T, U))]
trait Pair {
    type First;
    type Second;
}

#[batch_impl(<T: Clone> CloneIter<Item=T> Vec<T> {
    fn first(&self) -> T { self[0].clone() }
})]
trait CloneIter {
    type Item;
    fn first(&self) -> Self::Item;
}
```

## 7. The Directive System

The `#` directives expand during preprocessing, reading item signatures/types automatically from the trait definition — no need to hand-write signatures in the body.

### `#name{body}` — assigning a single item (fn / const / type automatically pick the output format)

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #to_str{"usize"})]
trait ToString { fn to_str(&self) -> &str; }
// → impl ToString for usize { fn to_str(&self) -> &str { "usize" } }

#[batch_impl(usize #MAX_SIZE{1024})]
trait HasConst { const MAX_SIZE: usize; }
// → impl HasConst for usize { const MAX_SIZE: usize = 1024; }

#[batch_impl(usize #Item{u32})]
trait HasType { type Item; }
// → impl HasType for usize { type Item = u32; }
```

### `#fill(methods){body}` — one body for many methods

> **Prefer `#name{body}` for a single item**: when filling exactly one
> method/const/type, write `#name{body}` (e.g. `#N{5}`) instead of
> `#fill(name){body}` — shorter and self-documenting. `#fill` is for
> **many** items (`#fill(a, b)`, `#fill(@all_required_methods)`).

> Directive arguments accept `(args)` or `[args]` — equivalent (e.g.
> `#fill[@all_methods]{0}`); square brackets are clearer when the arguments
> themselves contain parentheses. The no-argument `#name{body}` form works
> with both.

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(name, kind){"usize"})]
trait Describable { fn name(&self) -> &str; fn kind(&self) -> &str; }
// → generates a { "usize" } body for each of name and kind
```

Special markers: `@all` (all items), `@all_methods` (fn only), `@all_constants` (const only), `@all_types` (type only).

**Filtering by default-implementation state** (new in 0.6.1): trait items are split into "has a default implementation" (fn with a default body / const with a default value / type with a default type) and "no default implementation" (required — the impl must provide it). `@all_required*` / `@all_default*` select each side:

| Marker | Selected scope |
|--------|----------------|
| `@all_required_methods` | only methods without a default implementation (the impl must provide them) |
| `@all_default_methods` | only methods with a default implementation (the impl may omit them) |
| `@all_required` / `@all_default` | all items in the respective state (fn + const + type) |
| `@all_required_constants` / `@all_default_constants` | consts in the respective state |
| `@all_required_types` / `@all_default_types` | types in the respective state (**note**: a default associated type `type T = u8;` is a nightly feature (`associated_type_defaults`, E0658 on stable) — `@all_default_types` is only usable on nightly; the `type T;` declaration for `@all_required_types` works on stable) |

Using `@all_required_methods` alone means "implement only the required ones; default methods keep the trait's default implementation" (more precise than excluding one by one with `@all` + `-name`); `@all_default_methods` must be combined with the required side or handwritten items (filling only default methods leaves the required ones missing → E0046). required ∪ default = all.
The three directives (`#fill`/`#delegate`/`#blanket`) and `-` exclusion (`-@all_default_methods`) all work with these.

```rust
# use batch_impl::batch_impl;
// required ones get 1; default methods are overridden with 2
#[batch_impl(usize #fill(@all_required_methods){1} #fill(@all_default_methods){2})]
trait MixDefault {
    fn required(&self) -> u32;
    fn optional(&self) -> u32 { 100 } // default implementation, overridden by @all_default_methods
}
```

```rust
# use batch_impl::batch_impl;
// implement only the required ones; default methods keep the trait defaults
#[batch_impl(u64 #fill(@all_required_methods){3})]
trait KeepDefault {
    fn required(&self) -> u32;
    fn optional(&self) -> u32 { 7 }
}
```

**Filtering by receiver kind** (new in 0.6.2): trait methods are split into three kinds by receiver shape —
`&self` / `&mut self` (references), `self` (by value, including typed receivers such as `self: Box<Self>`),
and no receiver (associated functions / static methods):

| Marker | Selected scope |
|--------|----------------|
| `@all_ref_methods` | `&self` / `&mut self` methods |
| `@all_value_methods` | `self` (incl. typed receivers) methods |
| `@all_static_methods` | associated functions (no receiver) |

A typical use case is blanket: by-value delegation semantics depend on the wrapper's Deref/move capability, which
cannot be told apart at expansion time — use `@all_ref_methods` to delegate only reference methods, and by-value
methods keep the trait's default implementation:

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 { fn by_ref(&self) -> u8 { *self } })]
#[batch_impl(#blanket(@all_ref_methods){Box})]
trait RecvB {
    fn by_ref(&self) -> u8;
    fn by_val(self) -> u8 where Self: Sized { 0 }
}
// → impl<T> RecvB for Box<T> where T: RecvB {
//       fn by_ref(&self) -> u8 { (**self).by_ref() }   // delegated
//       // by_val is not generated → Box<T> uses the trait's default impl
//   }
// note: the `self` receiver in a default impl requires `where Self: Sized`
```

The three markers work in `#fill` / `#delegate` / `#blanket` and `-` exclusions alike

### `@0` position marker in blanket wrappers

Each `#blanket` wrapper's main part (minus `where` and `:N`) expands to
`part^T` (target appended last, e.g. `Box` → `Box<T>`) when it contains no
`@0`; with `@0`, the marker is the target's position — the wrapper is emitted
as-is and `@0` resolves to the fresh target generic, so `T` can sit anywhere:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]       // → Box<T> (T last)
trait PosTail { fn tag(&self) -> u32; }
#[batch_impl(#blanket(@all_methods){Box<@0>})]   // → Box<T> (equivalent)
trait PosBox { fn tag(&self) -> u32; }
# fn main() {}
```

`(u32, @0)` likewise expands to `(u32, T)` (T in second position) — the
delegation body is still generated from the deref depth, so non-Deref
wrappers need a where predicate or a custom `#delegate` target.

`@0` composes freely with user generics: `Rc<Box<@0>>` and `Rc^Box` expand
identically (`Rc<Box<T>>`); a custom Deref type with a const parameter
works too — `<const N: usize> #blanket(@all){MyPtrWithNum<@0, N>}` yields
`impl<const N: usize, T> Trait for MyPtrWithNum<T, N> where T: Trait` (the
user's `N` is kept, `@0` is replaced by the fresh target generic, and the
delegation body follows the deref depth).
(e.g. `#fill(@all_methods, -@all_value_methods)` = only reference + static methods).

### List subtraction `-name`

In the arguments, a `-` prefix marks an exclusion (the keep-list minus the exclude-list; exclusions win).
Used for "implement everything except one item":

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(@all,-skip_me){0})]
trait HasDefault {
    fn keep_me(&self) -> u32;
    fn skip_me(&self) -> u32 { 999 } // default implementation, kept when excluded
    const VALUE: u32;
}
// → impl HasDefault for usize {
//       fn keep_me(&self) -> u32 { 0 }
//       const VALUE: u32 = 0;
//       // skip_me is not generated; the trait's default impl is used
//   }
```

`-` may be followed by an identifier (`-foo`) or an `@all` family marker (`-@all_methods` = exclude all methods):
`#fill(@all,-@all_methods)` = only const + type items. It also applies to `#delegate`
(`#delegate(@all,-foo){target}`). An empty result after exclusion, or a missing target after `-`, raises `compile_error!`.
`-` only takes effect in directive argument domains and does not interfere with the type DSL's `-` concatenation operator.

### `#delegate(methods){target}` — delegation

Delegates methods to same-named calls on the target expression:

```rust
# use batch_impl::batch_impl;
// Vec<u32> gets its body via #name; Box<Vec<u32>> delegates to it
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }

// blanket impl pattern: concrete type + reference delegation
#[batch_impl(i32 #to_i32{*self}, <T: ToI32> &T #delegate(to_i32){**self})]
trait ToI32 { fn to_i32(&self) -> i32; }
// → impl<T: ToI32> ToI32 for &T { fn to_i32(&self) -> i32 { (**self).to_i32() } }
```

> **Delegation limits**: `#delegate` only supports **methods** (const / type items error); its arguments support only
> `self` and plain identifiers (pattern parameters such as `(a, b)` cannot be forwarded and error). The remaining limits
> are the same as blanket delegation — `*const`/`*mut`, `self`, and empty lists all raise `compile_error!`.

### Combining directives with the DSL

Directives can be freely chained with operators and `{body}` suffixes:

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    usize #name{"usize"} { fn kind(&self) -> &str { "number" } }
)]
trait Tagged { fn name(&self) -> &str; fn kind(&self) -> &str; }

#[batch_impl(<T: std::fmt::Display> Vec<T> #t10{self.len()})]
trait Len { fn t10(&self) -> usize; }
```

### Extension mechanism (open directive system)

An unrecognized `#name(args){body}` becomes a **top-level macro invocation**:
it expands to the `!`-marked block `{ ! name!{(args){body} trait ...} }`, and
codegen emits the call at top level with the spec body prepended — the user's
same-named macro receives `{spec}(args){body} trait ...` (4 segments, the spec
body first) and expands into arbitrary items, typically its own impl. This
means the directive system is **open**: `#fill`/`#delegate` are library
implementations of the "read the trait → generate" idea, an open directive is
a user-macro implementation of the same idea.

```rust
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test; // test-only open-extension macro: parses {spec}(names){body} trait → generates an impl
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}
// → trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; }
// → batch_preprocess_test!{ {usize} (add,inc){*self+1} trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; } }
//   → the macro expands to: impl AddInc for usize {
//       fn add(&self) -> Self { *self + 1 } fn inc(&self) -> Self { *self + 1 }
//     }
```

The same top-level protocol is available by hand: attach `{! m!{...}}` to a
spec (`T {! m!{...}}` — user-written input, same 4 segments). Without the
`!` (`T {m!{...}}`) the macro call stays in the impl body (associated items —
write the full input including the trait yourself). A `{!}` block must be
the last block of the spec, and there can be at most one.

> Note: this is a "user-defined `#fill`" — each type can attach its own
> (`usize #batch_preprocess_test(...){...}, isize #batch_preprocess_test(...){...}`),
> and the trait definition still comes only from the trait output by `#[batch_impl]`, without duplication.

### `#blanket(methods){wrapper list}` — blanket delegation

Generates delegating impls in bulk for wrapper types: every element in `{wrapper list}` **may be any type expression**
(`&` / `&mut` / `Box` / `Rc` / `Arc` / `MyPtr` / `Box^Arc` / `Cow<'_>`...), each producing a complete delegation spec. First implement the trait for the inner type, then blanket-cover the wrappers:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl(u32 { fn name(&self) -> String { self.to_string() } })]
#[batch_impl(#blanket(@all){&, Box, Rc})]
trait Name {
    fn name(&self) -> String;
}
// → impl Name for u32 { ... }                       // first batch_impl
// → impl<T> Name for &T    where T: Name { fn name(&self) -> String { (**self).name() } }
// → impl<T> Name for Box<T> where T: Name { ... }   // blanket: one delegated body per wrapper
// → impl<T> Name for Rc<T>  where T: Name { ... }
```

**Nested wrappers use `^` chains** (target type = wrapper expression `^T`, where T is a fresh generic); `<` prefill is append semantics (`Box<Arc>^T` = `Box<Arc, T>`, wrong):
`Box^Arc:2` → `Box<Arc<T>>`; `Cow<'_>` → `Cow<'_, T>`.

**Deref depth of the delegating body**: 1 by default (`**self`); nesting requires an explicit `:N` (the number of `*`s =
N + 1, e.g. `Box^Arc:2` → `***self`). The macro never guesses the Deref depth inside a wrapper — forgetting
`:N` on a nested wrapper degrades into a rustc method-not-found error.

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl(u32 { fn deep(&self) -> u32 { *self } })]
#[batch_impl(#blanket(deep){Box^Rc:2, Box^Box^Box:3})]
trait Deep {
    fn deep(&self) -> u32;
}
```

`methods` is the same as for `#delegate` (`@all` / `@all_methods` / an explicit method-name list).

**Wrapper constraint predicates**: a wrapper element may end with `where{...}` (after `:N`); the predicates join
the impl's where clause — this handles wrappers whose deref target ≠ T (e.g. `Cow<'_, T>`'s deref target is `T::Owned`,
so a blanket default-delegating to T needs the extra constraints). In the predicates,
`@0` refers to the target generic (fresh T) and `@trait` refers to the local trait name; the built-in `@Cow` constant
is the packaged `Cow<'_>` + its intrinsic constraints:

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}})]
trait CowName { fn len(&self) -> usize; }
// → impl<T> CowName for Cow<'_, T>
//       where T: CowName, T: ToOwned + ?Sized, T::Owned: CowName
// equivalent form (built-in constant):
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowName2 { fn len(&self) -> usize; }
```

**Generic trait support** (`trait Foo<X: Clone>`): trait parameters are copied verbatim as impl generics
(`impl<X: Clone, T: Foo<X>> Foo<X> for wrapper<T> where ...`); trait-level where predicates pass through.

**Assoc type / const delegation**: when `@all` includes const/type items, projections are generated —
`type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;` — so traits with required associated types can also be blanket-covered.

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<u32> u32 {
    type Item = u8;
    fn m(&self) -> u32 { *self }
})]
#[batch_impl(#blanket(@all){&, Box})]
trait Foo<X: Clone> {
    type Item;
    fn m(&self) -> X;
}
// → impl<X: Clone, T> Foo<X> for Box<T> where T: Foo<X> {
//     type Item = <T as Foo<X>>::Item;
//     fn m(&self) -> X { (**self).m() }
//   }
```

Constraints: `*const`/`*mut` (safe code cannot dereference raw pointers to delegate), `self` (meaningless), and empty elements / illegal `:N` all error — write `#delegate` by hand instead. by-value receiver methods
(`fn consume(self)`) have delegation semantics that depend on the wrapper's Deref/move capability, which cannot be told apart at macro expansion time — everything is allowed through and rustc has the final say.

**Static-method delegation** (new in 0.6.2): methods without a receiver (associated functions in
`@all_static_methods` / `@all_methods`) are forwarded through the blanket generic `t` — the delegating body is
`t::make(...)` instead of a deref chain (static methods have no `self` to dereference). Direct calls, nested
wrappers (`Box<Box<u8>>`), and argument forwarding all reach the underlying impl through the `t: Trait` bound —
the same forwarding semantics as the `<t as Trait>::Item` projection for assoc items:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_static_methods){Box})]
trait StaticT {
    fn make() -> u8;
    fn pair(a: u8, b: u8) -> u16;
}
impl StaticT for u8 {
    fn make() -> u8 { 7 }
    fn pair(a: u8, b: u8) -> u16 { (a as u16) * 10 + b as u16 }
}
// → impl<T> StaticT for Box<T> where T: StaticT {
//       fn make() -> u8 { T::make() }
//       fn pair(a: u8, b: u8) -> u16 { T::pair(a, b) }
//   }
// calls: <Box<u8> as StaticT>::make() → T::make() → u8::make() → 7
//        <Box<Box<u8>> as StaticT>::make() → recursive delegation (Box<u8>: StaticT bound)
```

## 8. where Clauses

### The `where{...}` suffix

The `where{...}` suffix follows the target type and holds pass-through where predicates; several merge together:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where{ T: Ord } {
    fn sort(&self) -> Vec<T> { let mut v = self.clone(); v.sort(); v }
})]
trait Sortable<T> { fn sort(&self) -> Vec<T>; }
// → impl<T: Clone> Sortable<T> for Vec<T> where T: Ord { ... }

#[batch_impl(<A> <B> PairAB<A, B> (A, B) where{A: Clone} where{B: Clone} {
    fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) }
})]
trait PairAB<A, B> { fn pair(&self) -> (A, B); }
```

### Bare `where predicates {code block}`

Rust-style bare writing is also supported (common to all three interfaces); the `{...}` code block after the predicates must exist;
the predicate region ends at the first `{...}` code block (`ident!{...}` macro-call bodies and code blocks inside `<N = {5}>` angle brackets don't count), and comma-separated predicates are not split across specs:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<A> <B> PairAB<A, B> (A, B) where A: Clone, B: Clone {
    fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) }
})]
trait PairAB<A, B> { fn pair(&self) -> (A, B); }
// → impl<A, B> PairAB<A, B> for (A, B) where A: Clone, B: Clone { ... }
```

Multiple `where` segments can be written in sequence (`where A: Clone where B: Clone`), equivalent to the older multiple `where{...}` form.

## 9. fn Types

```rust
# use batch_impl::batch_impl;
#[batch_impl(fn^(i32, u32))]
trait FnSimple {}

// fn type with an appended return type
#[batch_impl(fn(i32, u32)-String)]
trait FnWithReturn {}

// fn types generated in bulk (Cartesian product)
#[batch_impl(fn-(i32, u32)^2)]
trait FnTupleGen {}
// → impl FnTupleGen for fn(i32, i32) {}
// → impl FnTupleGen for fn(i32, u32) {}
// → impl FnTupleGen for fn(u32, i32) {}
// → impl FnTupleGen for fn(u32, u32) {}
```

`fn(A, B)-C` is equivalent to `fn(A, B) -> C` (the `-` appends a return
type). Note that `->` is **not** a DSL operator — do not try
`(A, B)->C` (a `(` group followed by `->` cannot be parsed).

`unsafe fn(...)` types: when `unsafe` immediately precedes `fn`, it modifies the fn type itself, unrelated to the
unsafe impl marker of `unsafe^T` (`unsafe^fn(...)` is "unsafe impl targeting an fn type"):

```rust
# use batch_impl::batch_impl;
#[batch_impl(unsafe fn(i32, u32) -> u32)]
trait UnsafeFnType {}
// → impl UnsafeFnType for unsafe fn(i32, u32) -> u32 {}

#[batch_impl(unsafe fn^(i32, u32) - i64)]
trait UnsafeFnType2 {}
// → impl UnsafeFnType2 for unsafe fn(i32, u32) -> i64 {}
```

> **`unsafe` disambiguation rules**: a bare `unsafe` (followed by `^`/`-` or standing alone) = unsafe impl marker;
> `unsafe fn...` = an unsafe fn type; `unsafe <other type>` (juxtaposed, no operator) = error
> (almost certainly a typo that forgot `^`; write `unsafe^T`).

## 10. The Full Modifier Reference

| Modifier | Meaning |
|----------|---------|
| `&` | reference (`&^T` → `&T`) |
| `&mut` | mutable reference (`&mut^T` → `&mut T`) |
| `*const` | raw pointer (`*const^T` → `*const T`) |
| `*mut` | mutable raw pointer (`*mut^T` → `*mut T`) |
| `self` | identity (`self^T` → `T`) |
| `unsafe` | bare `unsafe^T` = unsafe impl marker |
| `#[attr]` | attribute prefix (`#[attr]^T` → attribute prepended to the impl) |
| `[]` | empty base (`[]^T` → `[T]`, `[]-T-N` → `[T; N]`) |
| `[T]` | slice (`[T]^N` → fixed-size array `[T; N]`) |

```rust
# use batch_impl::batch_impl;
#[batch_impl(unsafe^usize, isize)]
unsafe trait UnsafePartial {}
// all impls of an unsafe trait are automatically unsafe

#[batch_impl(*const^u32, *mut^i32)]
trait PtrMarker {}

#[batch_impl(*const^Box^u32)]
trait ConstPtrChain {}
// → impl ConstPtrChain for *const Box<u32> {}

#[batch_impl(#[allow(dead_code)]^usize, isize)]
trait AttrSimple {}
```

A prefix acting on a **whole list** is automatically distributed to each item (`#[attr] [u8, u16]` and
`& [u8, u16]` both expand to one impl per item, each carrying the prefix/modifier).

### Array/slice builders

```rust
# use batch_impl::batch_impl;
#[batch_impl([]^u8)]          // → impl ArrSlice for [u8] {}
trait ArrSlice {}

#[batch_impl([u8]^3)]         // → impl ArrLit for [u8; 3] {}
trait ArrLit {}

#[batch_impl(<const N: usize> [u8]^N)]  // → impl<const N: usize> ArrConst for [u8; N] {}
trait ArrConst {}

#[batch_impl([u8]^1..3)]      // → impl ArrRange for [u8; 1] {} and [u8; 2] {}
trait ArrRange {}
```

### Complex-type pass-through

Unrecognized types pass through verbatim:

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}
```

## 11. Tuple Generation and Matrices

When the right side of `^` is a number or a range, tuples of the specified lengths are generated (numbers are only used as exponents):

| Syntax | Expands to |
|--------|------------|
| `()^3` | `(A, B, C)` (with 3 generic parameters) |
| `(T,)^3` | `(T, T, T)` |
| `(Box<u8>,)^2` | `(Box<u8>, Box<u8>)` (elements may be generic types) |
| `(T1, T2)^2` | Cartesian product `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |
| `()^1..3` | `(A,), (A, B)` (lengths 1 to 2) |
| `()^1..=3` | `(A,), (A, B), (A, B, C)` (lengths 1 to 3) |
| `(T,)^2..4` | `(T, T), (T, T, T)` (lengths 2 to 3) |

> Note: `(T)` is a group, `(T,)` is the 1-tuple — `(T)^N` strips the
> group and equals `T^N` (for a plain type `^N` is a const-generic
> argument: `(W)^2 = W<2>` where `W` is a type with a const generic; to
> generate tuples write `(T,)^N`).
> `(<u8>)` is invalid: a `<` right after `(` is not a legal type — a
> 1-tuple needs a complete type plus a comma, e.g. `(Box<u8>,)`.
> `(<Clone>)^N` (bound-base) is not supported; use
> `()^N where{@0: Clone, ...}` instead.

```rust
# use batch_impl::batch_impl;
#[batch_impl(()^1..=4 { fn describe(&self) -> &'static str { "tuple" } })]
trait DescribeTuple { fn describe(&self) -> &'static str; }
// → 4 impls: (A,), (A, B), (A, B, C), (A, B, C, D)
```

### Wrapping the whole matrix in const-generic fixed-size arrays

`[]` as the base of a `-` accumulation chain:

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    <const N: usize> []-[&, self, Box]^[u8, i8, ()^0..3]-N
)]
trait FixedMatrix {}
// → impl<const N: usize> FixedMatrix for [&u8; N]   { }
// → impl<const N: usize> FixedMatrix for [Box<i8>; N] { }
// → impl<const N: usize, A> FixedMatrix for [(A,); N] { }  // tuple fresh generics are hoisted automatically
// → ...
```

### `@` Constants — built-in type-family names

The `@` macro-meta layer has **three dimensions**:

| Dimension | Markers | Role |
|-----------|---------|------|
| **Constants** | `@u*` / `@num` / `@scalar` / `@u8..u128` / custom `@name=value;` | type-family lists, expanded before parsing |
| **Selectors** | `@all` family (`@all_methods` / `@all_required*` / `@all_ref_methods` / `@all_type_params` ...) | item-set selection for directive scopes (see §7) |
| **Positional references** | `@N` / `@g_i` / `@all_fresh` / `@N..M` | naming macro-generated fresh generics (next section) |

All three are **pure lexical substitution** — they expand to tokens before any DSL
parsing, so they compose freely with the type DSL (`[Box, Rc]^@uints`), directives
(`#fill(@all)`), and where predicates (`where{@0..=2: Copy}`).

Common type matrices don't have to be written by hand: `@` constants expand to literal lists during preprocessing, equivalent to writing them out.

| Constant | Expands to |
|----------|------------|
| `@u*` | `[u8, u16, u32, u64, u128, usize]` (unsigned family wildcard) |
| `@i*` | `[i8, i16, i32, i64, i128, isize]` (signed family wildcard) |
| `@f*` | `[f32, f64]` (float family wildcard) |
| `@num` | `@u* + @i* + @f*` (14) |
| `@scalar` | `@num + [bool, char]` (16) |
| `@u8..u128` | `[u8, u16, u32, u64, u128]` (**endpoints inclusive**; `@i8..i128` / `@f32..f64` work the same) |

```rust
# use batch_impl::batch_impl;
#[batch_impl(@scalar)]
trait ScalarTrait {}
// → 16 impls: one each for u8..char
```

All three entry points (`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`) support the built-in
constants. `batch_trait!` additionally supports **custom constants**: leading `@name=value;` sections in the macro arguments define them,
and later sections reuse them. Values are **arbitrary tokens** (**lazily expanded** — stored verbatim, and recursively expanded after concatenation at the point of reference), so values can directly contain DSL operations and chain references to other constants:

```rust
# use batch_impl::batch_trait;
# use std::rc::Rc;
trait TraitA {}
trait TraitB {}
batch_trait!(
    @nums=[u8, u16, u32];
    @uints=@u*;                       // references a built-in constant (wildcard family)
    @wrapped=[Box, Rc]^@nums;          // value contains DSL operations (evaluated at the reference site)
    @chain=@wrapped;                   // chained reference to a user constant
    TraitA: @chain;
    TraitB: [Box, Rc]^@uints;
);
```

**Reference visibility**: inside a constant definition you may only reference **built-in constants or user constants already defined before it** —
circular references (`@a=@a`) and forward references (`@a=@b` where `@b` is defined later) error at the definition site.

Unknown `@xxx`, illegal range endpoints, custom constants colliding with built-ins, and circular/forward references all raise `compile_error!`.

### Section-level `@trait` in `batch_trait!` (reusing "generic declarations + trait name" across sections)

In `batch_trait!`'s multiple sections, each section has a different trait name — the `@trait` inside constant values is **replaced per section with that section's trait path** after sectioning:

```rust
# use batch_impl::batch_trait;
# trait A<T> {} trait B<T> {}
batch_trait!{
    @type_t = <T> @trait <T>;   // packs "generic declaration + this segment's trait name"
    A: @type_t [&, Box]^T;      // → <T> A<T> [&, Box]^T
    B: @type_t Box^[T, Vec<T>]; // → <T> B<T> Box^[T, Vec<T>]
}
```

### Completing the macro-meta layer: `@trait` / `@all` family / `@Cow` / `@0`

`batch_impl` / `batch_impl_only` hold the trait definition, and the macro-meta layer additionally provides trait-aware
constants (`batch_trait!` is a function-like macro that can't get the definition, so it errors on the markers below):

| Marker | Expands to | Use case |
|--------|------------|----------|
| `@trait` | the trait's full path (`batch_impl` = local name, `batch_impl_only` = external path); in `batch_trait!` it is **section-level**: expands to that section's trait path | blanket wrapper where predicates; `batch_trait!` packing "generic declarations + trait name" across sections; the **trait-name part of a top-level spec** (`<T> @trait<T> Vec<T>`) |
| `@all` / `@all_methods` / `@all_constants` / `@all_types` | `[item names, ...]` (Bracket group) | directive scope selection — `#fill(@all)` is equivalent to the old `#fill(#all)` |
| `@all_required*` / `@all_default*` | Bracket groups filtered by default-implementation state | fill only the required / override only the defaulted |
| `@all_ref_methods` / `@all_value_methods` / `@all_static_methods` | Bracket groups filtered by receiver kind (`&self`/`&mut self` / `self` / associated functions) | delegate only reference methods (bypassing the uncertain by-value delegation semantics); `#blanket(@all_ref_methods){Box}` |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | generic-parameter families: expand to a **flat `<...>` generic declaration** (type parameters as bare names, const parameters in full `const N: usize` form, lifetimes verbatim) | generic declarations copied verbatim from the trait's parameters (bounds via same-named inheritance); `#[batch_impl(@all_lifetimes @all_type_params Borrowed<'a, T> &'a T)]` — consecutive declarations keep lifetimes first |
| `@Cow` | `Cow<'_>` + intrinsic constraint predicates | blanket wrapping (deref target = `T::Owned`) |
| `@N` (positional reference) | the name of the **Nth fresh generic** of *that impl* (of the form `_Param_{N}_BatchGen_`) — every impl renumbers its fresh to `0..N` in document order, so `@N` works across specs and range-generated impls | in blanket wrapper predicates `@0` = the target generic (the only fresh); in tuple generation `()^N`, `@k` = the kth fresh generic; also usable in the target type itself (`Box<@0>`); **user generics are written by name** (they don't participate in `@N` indexing) |
| `@g_i` (grouped reference) | group g, position i of that generating site (`_Param_{g}_{i}_BatchGen_`) — **stable across array-dispatch impls** (a group absent from an impl errors instead of silently shifting) | `()^3-()^3 where{@0_0: Clone}` = the left generator's first fresh, `@1_0` = the right generator's first; also usable in the target type |
| `@all_fresh` | every fresh generic of that impl (predicate-subject only) | `where{@all_fresh: Clone}` bounds all fresh generics |
| `@N..M` / `@N..=M` (range) | a contiguous fresh range (predicate-subject only) | `where{@0..=2: Copy}` bounds the first three freshes |

> `@N` resolves by number: in where predicates at the codegen stage, in the target
> type at the parse layer (the type-domain boundary);
> `@trait` has been moved earlier: `batch_impl` expands it at the constant stage (the trait path is known),
> and `batch_trait!` replaces it per section (recursively entering where groups).

**Choosing between `@N` and `@g_i`**: `@N` numbers fresh generics per impl in
document order — simple, but the meaning shifts across array-dispatch impls
(each impl renumbers from 0). `@g_i` names the exact generating site (group g,
position i) — **stable across dispatched impls**; use it when a where predicate
must refer to a specific generator's fresh in a dispatch
(`[Box, ()^2]^()^2`). `@all_fresh` / `@N..M` are the batch forms for
"every fresh" / "a contiguous run".

**Stability**: the `@N` numbering semantics were revised across 0.6.4 →
0.6.7 (per-impl numbering + document order + target-type channel). The current
mechanism (per-impl sweep to `_Param_0..N_BatchGen_`, `@N` = pure construction)
is considered **final** — any future change is treated as a deliberate
breaking release.

**Learning note**: the `@` layer is a small meta-language — the accumulated
markers have real learning cost. You can go far with just `@u*` / `@num` /
`@scalar` (constants) + `@all_methods` (selector) + `@0` (the blanket target).
The grouped / batch / range references exist for the composing scenarios —
reach for them when a predicate needs to name a specific fresh generic, not
earlier.

After the `@all` family expands into Bracket groups, normal directive-argument parsing applies: **`#` is no longer a scope marker** —
`#` now only appears in the single form of a directive name (`#fill`/`#delegate`/`#blanket`/open extensions), and scope
selection is uniformly owned by the macro-meta layer. Subtraction is unaffected: `#fill(@all, -foo)`, `#fill(@all, -[a,b])`.

**Directive arguments support `[a, b]` lists**: `#fill([m1, m2]){...}`; `-` exclusions can also be written
`-[a, b]` (the `@all` expansion already has this shape, and hand-writing it is equivalent).

**`@N` positional references in where predicates** — the generic names the macro generates are unknown to the user
(fresh names); constrain them by position:

```rust
# use batch_impl::batch_impl;
// tuple-generated fresh generics: @0 = the 0th, @1 = the 1st
#[batch_impl(()^2 where{@0: Clone, @1: Copy} { fn tmk() -> u32 { 2 } })]
trait TupleWhereAt { fn tmk() -> u32; }
// → impl<A: Clone, B: Copy> TupleWhereAt for (A, B) { fn tmk() -> u32 { 2 } }

// user generics: write their names directly (@N only indexes macro-generated fresh generics)
#[batch_impl(<T> AtWhere<T> Vec<T> where{T: Default} { fn an(&self) -> usize { self.len() } })]
trait AtWhere<T: Clone> { fn an(&self) -> usize; }
// → impl<T: Clone + Default> AtWhere<T> for Vec<T> { ... }

// batch references: @all_fresh bounds every fresh; @N..=M bounds a range
#[batch_impl(()^3-()^3 where{@all_fresh: Clone, @0..=2: Copy})]
trait BatchWhereAt {}
```

(In blanket wrapper predicates `@0` = the target generic, fresh T — see §7 `#blanket`; `@trait` can also appear
in ordinary where predicates, e.g. `where{@0: @trait<T>}`.)

**Generic-parameter families** (0.6.4): the generic declaration is copied verbatim from the trait's parameters (type parameters as bare names, const parameters as full declarations, lifetimes verbatim) — bounds are filled in automatically by same-named inheritance:

```rust
# use batch_impl::batch_impl;
#[batch_impl(@all_type_params GenT<T> Vec<T> { fn head(&self) -> T { self[0].clone() } })]
trait GenT<T: Clone> { fn head(&self) -> T; }
// → impl<T: Clone> GenT<T> for Vec<T> { fn head(&self) -> T { self[0].clone() } }

#[batch_impl(@all_lifetimes @all_type_params Borrowed<'a, T> &'a T { fn get(&self) -> &'a T { *self } })]
trait Borrowed<'a, T: Clone> { fn get(&self) -> &'a T; }
// → impl<'a, T: Clone> Borrowed<'a, T> for &'a T { ... } (consecutive declarations keep lifetimes first)
```

> **Relation to `A<>`**: the `A<>` expansion (§5) **simultaneously** includes the parameter declarations (with bounds) and the arguments — it is itself "fully automatic" (`#[batch_impl(Foo<> Vec<T>)]` is one line copying declarations + arguments + bounds verbatim). `@all_type_params` is the granularity that **automates only the declaration** (use it when the arguments need to be custom). **Don't stack them**: `@all_type_params Foo<>` would make the two declaration sources duplicate (rustc E0403) — pick one.

## 12. Three Entry Points

| Macro | Purpose |
|-------|---------|
| `#[batch_impl]` | attribute macro annotated on a trait definition; the macro arguments are the DSL |
| `#[batch_impl_only]` | same, but discards the trait definition and only outputs impl blocks |
| `batch_trait!` | function-like macro that generates impls in bulk for already-declared traits (supports multiple traits) |

All three accept the same DSL arguments.

### `#[batch_impl_only]`

For cases where the trait is already defined elsewhere and you only need bulk impls. The trait definition still has to be written (it is only read for method signatures), but the output does not include the trait:

```rust
# use batch_impl::batch_impl_only;
# trait Greet { fn hello(&self) -> &str; } // the real trait is defined elsewhere
#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; } // this dummy definition is discarded
// → impl Greet for usize { fn hello(&self) -> &str { "hi" } }
```

A `#path::to::Trait:` path prefix is supported, generating impls for traits defined in external modules
(the trailing identifier of the path must match the local dummy trait name; `#[batch_impl]` does not support this prefix):

```rust
# use batch_impl::batch_impl_only;
# mod ext { pub mod traits { pub trait TraitName {} } }
# use ext::traits::TraitName;
#[batch_impl_only(#ext::traits::TraitName: usize, isize)]
trait TraitName { }
// → impl ext::traits::TraitName for usize {}
// → impl ext::traits::TraitName for isize {}
```

### `batch_trait!`

Generates impls in bulk for already-declared traits; `;` separates multiple trait sections. Syntax:
`[unsafe] TraitPath: impl-specs`; the right side of `:` accepts the type DSL and `@` constants (the same
type syntax as `#[batch_impl]`), and additionally supports multiple trait sections, path traits (e.g.
`foo::C`), and unsafe sections:

```rust
use batch_impl::batch_trait;

trait A {}
trait B<T> {}
unsafe trait UnsafeTrait {}

batch_trait!(
    A: usize, isize;
    B: <T> B<T> Vec<T>;
    unsafe UnsafeTrait: usize
);
```

> **Limitation**: `batch_trait!` does **not support `#` directives** (`#fill`/`#delegate`/`#blanket`/
> open extensions) — directives need the trait definition as the signature source of truth, while `batch_trait!` is a function-like
> macro that can't get the trait definition. When you need directives, use `#[batch_impl]` / `#[batch_impl_only]` instead.

## 13. Error Messages

All DSL syntax errors are reported through `compile_error!()` with **English messages** (since 0.6.2), pointing as
precisely as possible at the offending token in the source (span diagnostics; tokens inside groups and the `Err`
return path show the macro invocation line), and never panic:

| Bad input | Error message (excerpt) |
|-----------|--------------------------|
| `batch_trait!(;)` | `batch_trait! expects a trait name` |
| `batch_trait!(A)` | `batch_trait! expects ':' to separate the trait name and impl-specs` |
| `batch_trait!(A: B::)` | `batch_trait! expects an ident as the trait name` |
| `A^` (missing right operand) | `batch-impl: missing operand after '^' (e.g. 'T^U')` — points at the `^` itself |
| `A,,B` | `batch-impl: missing operand between consecutive commas ',,`'` |
| `3..2` (empty range) | `batch-impl: range '3..2' is empty (start not below end); no impls will be generated` |
| `^2000` (over the limit) | `batch-impl: tuple '^2000' expands to 2000 items (limit 1024); likely exponential/range/Cartesian typo` |
| nesting depth over 128 | `batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket)` |
| `@unknown` | `batch-impl: unknown @ constant '@unknown'; built-ins: '@u*' ...` |
| `@u32..u8` (endpoints reversed) | `batch-impl: range start is greater than end: 'u32..u8'` |
| `@a=@a` (circular reference) | `batch-impl: constant '@a' references unknown '@a' (undefined or defined later; ...)` |
| `#fill()` (empty arguments) | `batch-impl: the directive's argument list cannot be empty` |
| missing target after `-` | `batch-impl: directive arguments cannot be empty` |
| bare `where` without a code block | `batch-impl: 'where' predicates are missing a code block {...}` |
| inherited predicate referring to an undeclared parameter | `batch-impl: trait argument 'X' maps to parameter 'T' (bound 'IntoIterator'); automatic inheritance requires the same name; rename to 'T' or write the bound manually` |
| `#blanket` illegal wrapper | `batch-impl: #blanket ...` (`*const`/`*mut`, `self`, empty elements, illegal `:N`, and non-forwardable pattern parameters all error) |
