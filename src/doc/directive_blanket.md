# The `#blanket` Directive — Blanket Delegation

`#blanket(args){wrapper list}` implements the trait for **every wrapper
around a fresh generic `T`**, delegating each method by deref. One spec
produces one complete impl per wrapper — the automated form of hand-writing
`<T: Trait> wrapper.T #delegate(selected){*…*self}` for each wrapper.

## Syntax

```text
#blanket(scope){wrapper list}
```

- `scope` — the item set (same directive-domain parser as `#fill` /
  `#delegate`: `@all`-family markers, name lists, `-` subtraction);
- `wrapper list` — comma-separated **type expressions** wrapping a fresh
  generic `T`: `&`, `&mut`, `Box`, `Rc`, `Arc`, `MyPtr`, nested chains,
  `Cow<'_>`, ... Each wrapper yields one impl.

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

The fresh generic is the impl's only generic (`impl<T: Trait> Trait for
Box<T>`); the trait's own generic params are copied first (params first,
fresh `T` last — `T: Foo<X>` references `X`; reversed order is E0401).

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
// → impl<T: Deep> Deep for Box<Arc<T>> { fn deep(&self) -> u32 { (***self).deep() } }
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
// → impl DynLen for Box<dyn DynLen>? — the ?Sized bound lets the fresh generic
//   (and thus the target) be unsized; without `@?`, `T: DynLen` implies Sized
//   and a dyn target fails.
```

## Deref delegation details

- `&self` / `&mut self` methods reach the inner through the reference AND
  the wrapper layers: `(**self)` = depth + 1 derefs;
- **by-value** `self` methods (`fn consume(self)`) forward as `(*self).m()`
  — a by-value `self` IS the wrapper, so one deref fewer (0.7.2 fix: the
  extra star dereferenced the inner type, E0614). Moving the value out
  cannot type-check for shared wrappers (`&`, `Rc`); the generated impls
  carry a `#[doc]` note (proc macros have no stable warning channel, E0658).
  Skip such methods with `@all_ref_methods` or hand-write `#name{...}`;
- **static methods** (no receiver) delegate through the fresh generic:
  `t::make(...)` — valid because the blanket impl carries the `T: Trait`
  bound.

## Assorted delegations

**Associated types and consts** are delegated by **projection** (not through
self): `type Item = <T as Trait>::Item;` / `const N: Ty = <T as Trait>::N;` —
solving "cannot delegate traits with required associated types".

**Generic associated types (GATs)** project with their own params:
`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a;` — the GAT's
parameters are passed through the projection (a bare projection would be
missing the lifetime argument, E0107).

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

A method taking or returning **bare `Self`** cannot blanket-delegate: the
forward emits the inner type, which cannot match the wrapper's `Self`. The
macro reports a targeted error with guidance (`#name{...}` for that
wrapper):

- `fn new() -> Self` — the forward `t::new()` returns `T`, not the wrapper
  (used to fail with rustc's E0308 at the generated impl);
- `fn cmp(&self, other: Self)` — the parameter `other: T` mismatches the
  wrapper's `Self` (E0308).

A `Self::Assoc` **projection return** (`fn iter(&self) -> Self::Iter`) is
fine and passes through — the inner `T` carries the same assoc type.
`Self::Assoc` in parameters errors (the parameter type would be `T::Iter`,
not the wrapper's).

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
`#blanket` wrapper list). `Cow<'_>`'s deref target is `T::Owned`, not `T` —
the naive `(**self)` delegation cannot pass type checking. `@Cow` packs
`Cow<'_>` **plus** the inherent constraint predicates (`@0: ToOwned +
?Sized`, `@0::Owned: @trait`), making it blanket-usable — the demonstration
that **a constant carries reuse value only when it carries constraints**:

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowLen { fn clen(&self) -> usize; }
impl CowLen for str { fn clen(&self) -> usize { self.len() } }
impl CowLen for String { fn clen(&self) -> usize { self.len() } }
// → impl CowLen for Cow<'_, str> ... / Cow<'_, String> ...（delegates via the packed predicates）
```

## Output shape

`#blanket` output is **multiple complete specs** (comma-separated) that can
only stand alone as specs (self-contained generics / target / delegation) —
attaching them to a type is meaningless. This is the multi-token output
kind in the attachment semantics; `#name` / `#fill` / `#delegate` produce
single `{...}` groups that attach or stand alone.

**Documentation marker only — never call this function.**
