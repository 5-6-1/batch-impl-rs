# The `@` Macro-Meta Constant System

`@` is the DSL's reserved **library-owned constant namespace** — `#` is taken
by the directive mechanism, so `@` provides "name and reuse type-matrix
entries". It is pure **lexical substitution** at the macro-meta layer: the
expanded result enters the normal pipeline (`@` → `<>` pairing → `#`
directives → `where`) and participates in no in-domain parsing.

The name after `@` is looked up in a fixed order: user-defined constants
(`batch_trait!` only) → built-in name families → range families → `@all`
selectors → `@trait` → `@N` / `@g_i` positional references.

## Preprocessing position

`@` expansion is the **outermost pass**, running before `<>` pairing and
directives — the expansion output may contain flat `<...>` (e.g. the value of
`@map = HashMap<u32, String>`), which must be paired uniformly by the
subsequent `angle_collect`. A reversed order (`<>` before `@`) would pair
`Vec<@inner>`'s `@inner` into an angle group and the constant stage would
never enter it — `@` would leak into the output. The order is fixed:
`@ <> # where`.

## Built-in name families

A closed set of language-defined type collections; each expands to its
members as a list:

| Constant | Expands to |
|---|---|
| `@u*` | `u8, u16, u32, u64, u128, usize` |
| `@i*` | `i8, i16, i32, i64, i128, isize` |
| `@f*` | `f32, f64` |
| `@num` | every `@u*` + `@i*` + `@f*` member (14 types) |
| `@scalar` | the primitive scalars (the numeric families + `bool` + `char`) |

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# use std::sync::Arc;
#[batch_impl(Box @u*)]  // Box applied to every member of @u*
trait Boxed {}
// → impl Boxed for Box<u8> {} / Box<u16> / ... / Box<usize>

#[batch_impl([Rc, Arc] @scalar)]
trait ScalarPtr {}
// → impl ScalarPtr for Rc<u8> {} ... Rc<char> {} / Arc<u8> {} ... Arc<char> {}
```

## Range families

`@u8..u128`, `@i8..i128`, `@f32..f64` (inclusive) — the contiguous run of
one family, ascending by width:

| Constant | Expands to |
|---|---|
| `@u8..u128` | `u8, u16, u32, u64, u128` |
| `@i8..i128` | `i8, i16, i32, i64, i128` |
| `@f32..f64` | `f32, f64` |

`usize` / `isize` only enter name families, never range families. Endpoint
widths are validated: `@u9..u128` errors ("invalid width"), a mismatched
family (`@u8..i32`) or a descending run (`@u128..u8`) errors too.

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec @u8..u32)]  // u8, u16, u32
trait VecOf {}
// → impl VecOf for Vec<u8> {} / Vec<u16> {} / Vec<u32> {}
```

## User-defined constants (`batch_trait!` only)

A leading `@name=value;` section defines reusable constants. Values are
stored as **verbatim tokens** and expand lazily — a value can reference
built-in constants, chain other user constants, and embed arbitrary DSL
expressions:

```rust
# use batch_impl::batch_trait;
# trait A {}
# trait B<T> {}
batch_trait! {
    @uints = @u*;
    @big = @u64..u128;
    A: @uints;
    B: <T> B<T> @big;
}
// A gets u8..usize (6 impls), B gets u64/u128 (2 impls)
```

Reference-visibility rules (enforced at definition time, in
`check_value_refs`):

- a value may reference built-in constants and **previously defined** user
  constants (definition order);
- a **circular** reference (`@a=@a` / `@a=@b, @b=@a`) is rejected at
  definition — under lazy expansion it would recurse forever;
- a **forward** reference (`@a=@b` with `@b` defined later) is rejected at
  definition;
- a **bare range endpoint** (`@a=@u8` without `..`) errors at definition —
  `@u8` alone is not a constant.

`#[batch_impl]` / `#[batch_impl_only]` **do not support custom constants**
(the 0.7.2 attribute-macro feature was reverted in 0.8.0): write
attribute-macro matrices directly with `.` / space / `*` instead. Constant
names are reserved against collision: `@trait` and the whole `@all` family
cannot be redefined, and a name colliding with a built-in constant
(`@uints` would be fine, `@u*` is not) errors.

## The addressing algebra: `@N` / `@g_i` / ranges

`@`'s positional references form an **addressing algebra** — they name the
macro's own generated generics (the *fresh* generics minted by generators
like `().N`), which the user cannot know by name before expansion:

| Notation | Derivation | Expands to |
|---|---|---|
| `@g_i` | **primitive** — group g, slot i (stable across array distribution) | the i-th fresh of generator group g (`@0_0` → the first fresh of the first generator) |
| `@N` | `@g_i` flattened by document order within one impl | the N-th fresh generic name (`@0` → `P0` in a `where{@0: Clone}` predicate) |
| `@all_fresh` | all fresh generics | every fresh name, one predicate each (≡ `@0..`); **deprecated**, write `@0..` |
| `@N..=M` | a contiguous run | the fresh names N..=M, comma-separated (`@0..=1` → `P0, P1`) |
| `@N..` | an **open** run to the last fresh | every fresh name from N to the last, comma-separated (`@1..` → `P1, P2, ...`); **empty** when N is past the end |
| `@L_N..` / `@L_N..M` / `@L_N..=M` | **grouped ranges** — slice within one generator group | the group's fresh names from position N, stable across array dispatch |

Fresh display names are numbered `P0, P1, ...` in document order (escaping
collisions by prefixing underscores: `_P1`, `__P1`) — the expansion splices
the names where the `@` sits (a where-predicate subject, a target tuple
element, a generic argument), so a range becomes several names and a `where`
tail is copied per fresh.

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

The fresh list a range indexes comes from the spec's generators
(`*().N` / `().N`); a range in a spec with no fresh generics reports
"out of range".

**The impl-generic declaration position** works too: `<@0..>` declares
every fresh the range covers as an impl param — so a spec can put the
generator in the trait args (`GenConv<*().2>`) and reference the same fresh
batch in the declaration and the predicates:

```rust
# use batch_impl::batch_impl;
struct DeclTarget;
#[batch_impl(<@0..> GenConv<*()2> DeclTarget where @0..: Clone { fn m(&self) {} })]
trait GenConv<T, U> { fn m(&self); }
// → impl<P0,P1> GenConv<P0,P1> for DeclTarget where P0: Clone, P1: Clone
```

An empty `<@0..>` (no fresh generators in the spec) contributes no
parameters, like an empty `@1..` predicate.

**Grouped ranges `@L_N..`** slice **within one generator group** — the
in-group counterpart of `@g_i`, stable across array dispatch. With several
generators in one spec (`<*().2>` → group 0, `<*().3>` → group 1),
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

**Value positions**: the type after `:` may carry `@N` inside angle groups —
e.g. an associated-type binding referencing another fresh's associated type
(the alga2 tuple `Module` scalar-equality constraint):

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

## The selection and identity classes

On the other axis (value classes):

| Notation | Class | Use |
|---|---|---|
| `@trait` | **identity** — the current trait name/path (section-level in `batch_trait!`) | package "generic declaration + trait name" across sections |
| `@all_methods` etc. | **selection** — extract an item set from trait_def | `#fill(@all_required_methods, -foo)` precise selection |
| `@Cow` | **built-in `#blanket` wrapper constant** — `Cow<'_>` plus its inherent constraints (`@0: ToOwned + ?Sized, @0::Owned: @trait`) | blanket-usable `Cow` delegation |

### `@trait`

Expands to the current trait's full path: the local trait name under
`#[batch_impl]`, the external path under `#[batch_impl_only(#ext::Trait: ...)]`.
In `batch_trait!` it is **segment-level**: after segmentation, each segment's
`@trait` is replaced with that segment's trait path — enabling cross-segment
packing reuse such as `@type_t=<T>@trait<T>` (define once, apply to every
segment's own trait).

### The `@all` family

Expands to a **Bracket group** `[a,b,c]` selecting items according to the
trait definition — unified in shape with the `@u*` list forms, so directive
arguments naturally support hand-written `[a, b]` lists and `-[a, b]`
exclusions. The selectors:

| Selector | Selects |
|---|---|
| `@all` | every item (fn + const + type) |
| `@all_methods` | every Fn method |
| `@all_constants` | every associated const |
| `@all_types` | every associated type |
| `@all_default*` | only items **with** a default implementation |
| `@all_required*` | only items **without** a default (required) |
| `@all_ref_methods` | only `&self` / `&mut self` methods |
| `@all_value_methods` | only by-value `self` methods (incl. typed receivers) |
| `@all_static_methods` | only associated functions (no receiver) |

`@all` family is exclusive to `#[batch_impl]` / `#[batch_impl_only]`
(needs the trait definition to select items); `batch_trait!` errors.

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u32> #fill(@all_methods, -push){self.len()})]
trait Len {
    fn len(&self) -> usize;
    // a default body keeps the trait satisfiable when `push` is excluded
    fn push(&mut self, v: u32) {}
}
// → impl Len for Vec<u32> { fn len(&self) -> usize { self.len() } }  (push excluded)
```

### `@Cow`

A `#blanket`-only wrapper constant: expands to `Cow<'_>` plus the inherent
constraint predicates (`@0: ToOwned + ?Sized`, `@0::Owned: @trait`) that make
delegation through `Cow` type-check. It is a **built-in of the blanket
wrapper list** — not a custom constant, and only meaningful inside
`#blanket(...){ ... @Cow ... }`.

## The generic-parameter families

`@all_type_params` / `@all_const_params` / `@all_lifetimes` expand to a flat
`<...>` generic declaration copied from the trait's own generic parameters:
type params by name (bounds are picked up by codegen's same-name
inheritance), const params as the full `const N: usize` declaration (a bare
name is E0747), lifetimes as-is. `batch_trait!` errors (no trait
definition).

```rust
# use batch_impl::batch_impl;
# struct Target;
#[batch_impl(@all_type_params TraitG<T> Target { fn m(&self) {} })]
trait TraitG<T> { fn m(&self); }
// → impl<T> TraitG<T> for Target { ... }
```

## Reserved-name rules

- `@trait` is a reserved marker (segment-level substitution) — cannot be
  used as a user constant name;
- the whole `@all` / `@all_*` family is reserved for selectors — a user
  constant with such a name would be shadowed, rejected at definition;
- a user constant colliding with a built-in constant name errors.

## Notes

- `@` in a **body** is pattern syntax (`x @ pat`) — constant expansion never
  enters `{...}` code blocks; only `where{...}` predicate groups and
  `impl{...}` shape templates are entered for `@trait` / `@`.
- `@N` positional references are resolved by **codegen**, where the impl's
  fresh list is known — preprocessing leaves `@N` untouched.
- `@all_fresh` is kept for compatibility but **deprecated**: write `@0..`.

**Documentation marker only — never call this function.**
