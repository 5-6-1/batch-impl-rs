Documentation-only guide to `#blanket`: use the directive inside `#[batch_impl(...)]`; do not invoke `batch_impl_blanket!`.

# The `#blanket` Directive — Blanket Delegation

`#blanket(args){wrapper list}` implements the trait for **every wrapper
around a fresh generic `T`**, delegating each method by deref. One spec
produces one complete impl per wrapper, forwarding to the inner type's
implementation of the same trait. Calls name that trait explicitly, so a
supertrait's same-named method cannot make delegation ambiguous. Async methods await the forwarded call;
method type and const arguments are passed explicitly, with lifetimes inferred.

## Syntax

```text
#blanket(scope){wrapper list}
```

- `scope` — the item set (same directive-domain parser as `#fill` /
  `#delegate`: `@all`-family markers, name lists, `-` subtraction);
- `wrapper list` — comma-separated **type expressions** wrapping a fresh
  generic `T`: `&`, `&mut`, `Box`, `Rc`, `Arc`, `MyPtr`, nested chains,
  `Cow<'_>`, ... Each wrapper yields one impl.

  **The fresh generic is `Sized` unless you write `@?` after the wrapper** —
  `Box@?`, `&@?`, `Rc@?`, `Arc@?` are what make the blanket cover `Box<str>`,
  `Box<[T]>` and `Box<dyn Trait>`. Without it those impls are simply absent,
  and nothing in the expansion says so: the only signal is an `E0277` at the
  *use* site (or an `E0599` miss), whose help points at the attribute span but
  never names `@?` — measured, and the reason a user reaches this section only
  after a failure. See "@? — unsized wrappers" below for the spelling and the
  resulting `where P0: Trait + ?Sized`.

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]
trait NumOps { fn inc(&mut self); }
impl NumOps for u32 { fn inc(&mut self) { *self += 1 } }
// → impl<P0> NumOps for Box<P0> where P0: NumOps { fn inc(&mut self) { <_ as NumOps>::inc(&mut **self) } }
//   (generic over the fresh: every P0: NumOps, not just u32)

#[batch_impl(#blanket(@all_methods){&, Box})]
trait Len { fn len(&self) -> usize; }
// → impl<P0> Len for &P0   where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
// → impl<P0> Len for Box<P0> where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
```

The blanket introduces one fresh generic (`impl<T: Trait> Trait for
Box<T>`). The trait's own generic parameters are copied before it.

The scope accepts one trailing comma, empty arguments (`#blanket(){Box}`),
and valid empty selections (`[]`, an empty `@all` family, or subtraction).
An empty selection still generates one impl per wrapper, with no delegated
members; Rust checks any required members left unimplemented. Every included
or excluded name must exist in the trait, even if subtraction removes it.

## Wrapper forms

### Simple wrappers

`&`, `&mut`, `Box`, `Rc`, `Arc`, any single-parameter type constructor
(`MyPtr`), a tuple with `T` inside (`(u32, T)`)...

### Nested wrappers: chain with `.`

Nested wrappers must be chained with `.`: `Box.Arc` = `Box<Arc<T>>`.
`<` prefilling is **append semantics** — `Box<Arc>.T` = `Box<Arc, T>`, an
error. Use `.` for nesting:

```rust
# use batch_impl::batch_impl;
# use std::sync::Arc;
#[batch_impl(#blanket(@all_methods){Box.Arc:2})]
trait Deep { fn deep(&self) -> u32; }
// → impl<P0> Deep for Box<Arc<P0>> where P0: Deep { fn deep(&self) -> u32 { <_ as Deep>::deep(&***self) } }
```

### `:N` deref depth

`Box.Arc:2` — the number of derefs the delegation needs beyond the `&self`
reference: N wrapper layers → N+1 stars (`&self` is a reference, deref it,
then N wrapper layers). The default is **1** for single wrappers
(`&`/`Box`/`Rc` → `**self`). Write `:N` only for nested wrappers; single
wrappers need nothing. `*const`/`*mut` (safe code cannot deref a raw
pointer to delegate), `self` (meaningless), empty elements and invalid `:N`
all error.

### `@0` — T's position marker

A wrapper whose main part contains `@0` treats `@0` as T's position —
`(u32, @0)` → `(u32, T)`. Without `@0` the wrapper is applied as
`wrapper.T` (T appended last). `@0` in the wrapper's where clause refers to
the target generic (resolved by codegen).

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box<@0>})]
trait At0 { fn tag(&self) -> u32; }
// @0 marks T's position; Box<@0> → Box<T>
```

### `@?` — unsized wrappers

A wrapper element ending in `@?` (`Box@?`, `Box<Rc@?>` — the suffix rides to
the innermost wrapper of a chain) adds `T: ?Sized` to that spec's where
clause, so the fresh generic can be an **unsized target**:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynLen { fn dlen(&self) -> usize; }
impl DynLen for str { fn dlen(&self) -> usize { self.len() } }
// → impl<P0> DynLen for Box<P0> where P0: DynLen, P0: ?Sized — the ?Sized bound lets
//   the fresh generic (and thus the target) be unsized; without `@?`, `T: DynLen`
//   implies Sized and a dyn target fails.
```

## Deref delegation details

- `&self` / `&mut self` methods reach the inner through the reference AND
  the wrapper layers: depth + 1 derefs, followed by an explicit borrow.
  With one wrapper, calls are `<_ as Trait>::m(&**self, ...)` or
  `<_ as Trait>::m(&mut **self, ...)`. Rust infers `_` from the actual
  deref target; an arbitrary wrapper's target need not be its type parameter.
  Explicit `self: &Self` / `self: &mut Self` use the same rule;
- **by-value** `self` methods (`fn consume(self)`) forward as
  `<_ as Trait>::consume(*self)` — a by-value `self` IS the wrapper, so
  one deref fewer. The wrapper must permit moving out the value, or the
  inner value must be `Copy`; the generated impls
  carry a `#[doc]` note (proc macros have no stable warning channel, E0658).
  Skip such methods with `@all_ref_methods` or hand-write `#name{...}`;
- **static methods** (no receiver) delegate through the fresh generic:
  `<T as Trait>::make(...)`, with the trait's actual generic arguments
  included when present.

Async calls append `.await`. Method type and const arguments are forwarded
with a turbofish, while lifetimes remain inferred: for example,
`<_ as Trait>::read::<U, N>(&**self, value).await`.

## Assorted delegations

**Associated types and consts** are delegated by **projection** (not through
self): `type Item = <T as Trait>::Item;` / `const N: Ty = <T as Trait>::N;` —
solving "cannot delegate traits with required associated types".

**Generic associated types (GATs)** project with their own parameter names:
`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a;` — the GAT's
declarations and `where` predicates stay on the impl definition. Result
bounds such as `type Iter<'a>: Clone` stay on the trait, where Rust checks
them. Projections pass only lifetime/type/const names: a declaration
`Item<'a, U: Clone, const N: usize>` projects as `Item<'a, U, N>` (a bare
projection would be missing arguments, E0107).

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all){Box})]
trait Iterable {
    type Item;
    type Iter<'a>
    where
        Self: 'a;
}
// → impl<T: Iterable> Iterable for Box<T> {
//     type Item = <T as Iterable>::Item;
//     type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;
//   }
```

## `Self` in the signature

A method using **bare `Self`** in an ordinary parameter, return type, or
generic constraint cannot blanket-delegate: the inner type and the
wrapper's `Self` are different types. The
macro reports a targeted error with guidance (`#name{...}` for that
wrapper):

- `fn new() -> Self` — the forward `<T as Trait>::new()` returns `T`, not the wrapper
  (used to fail with rustc's E0308 at the generated impl);
- `fn cmp(&self, other: Self)` — the parameter `other: T` mismatches the
  wrapper's `Self` (E0308);
- `fn read<U: Marker<Self>>(&self)` or `where U: Marker<Self>` — a bound
  involving the wrapper does not establish the corresponding inner bound;
- `where Self: Marker<U>` — the wrapper and inner constraints differ too.

Receiver `Self` is forwarded by the deref rules above. The usual
`where Self: Sized` and outlives conditions (`Self: 'a`, or
`Self: Sized + 'a`) remain allowed; Rust checks whether the actual delegated
target satisfies them. Attribute payloads are not interpreted as constraints.

A `Self::Assoc` projection is allowed in parameters, return types, and constraints.
When the associated item is also forwarded, the wrapper's projection is the
inner type's projection (`type Assoc = <T as Trait>::Assoc`), so these method
arguments and results have matching types.

## Wrapper where predicates

A wrapper element may carry a `where{...}` predicate — merged into that
spec's where clause alongside `T: Trait` and the trait's own where
predicates (zero-analysis parallel merge):

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box where{@0: Clone}})]
trait Tagged { fn tag(&self) -> u32; }
```

## Generic traits

`trait Foo<X> where X: Clone` is supported: the trait params are copied into
the impl generics (params first, fresh `T` last — `T: Foo<X>` references X),
args = param names; trait-level where predicates pass through into the impl
where clause (single-param predicates merge into bounds via codegen's
inheritance — the blanket spec's generic X has no bound, inheritance adds
`X: Clone`).

## `@Cow` — a constraint-carrying packing

`@Cow` is a **built-in `#blanket` wrapper constant** (usable only in the
`#blanket` wrapper list). It packs `Cow<'_>` with the predicates
`@0: ToOwned + ?Sized` and `@0::Owned: @trait`. `Cow<'_, T>` dereferences
to `T`; the `T::Owned: Trait` predicate is an additional constraint included
by this constant. Both the wrapper and its packaged constraints enter the
ordinary blanket pipeline:

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowLen { fn clen(&self) -> usize; }
impl CowLen for str { fn clen(&self) -> usize { self.len() } }
impl CowLen for String { fn clen(&self) -> usize { self.len() } }
// → impl<P0> CowLen for Cow<'_, P0> where P0: CowLen, P0: ToOwned + ?Sized, P0::Owned: CowLen
//   (one generic impl over the targets satisfying these packaged constraints)
```

## Output shape

`#blanket` output is **multiple complete specs** (comma-separated) that can
only stand alone as specs (self-contained generics / target / delegation) —
attaching them to a type is meaningless. This is the multi-token output
kind in the attachment semantics; `#name` / `#fill` / `#delegate` produce
single `{...}` groups that attach or stand alone.

**Documentation marker only — never call this function.**
