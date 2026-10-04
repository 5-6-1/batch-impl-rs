# batch-impl Tutorial

**v0.10.0 — in development (unreleased).** This tutorial describes the development working tree. See [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/CHANGELOG.md) for pending changes and migration notes.

**Repository sources (GitHub main)**: English · [简体中文](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md) · [README](https://github.com/5-6-1/batch-impl-rs/blob/main/README.md) · [Reference manual](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md)

The GitHub links above and throughout this page open repository sources;
public `main` may not yet match this local working tree. Links between this
tutorial's sections stay on the current page.

Start by implementing a method for one type, then extend the same program
through five changes. The macro generates ordinary Rust impls; if you
already have a complete impl, you can start from that instead.

## 0. Choose a Reading Path

For a first pass, complete [§1's continuous task](#1-implement-and-call-one-method):
add types, add a method, give a special type its own spec, add a generic
bound, and forward a wrapper's methods. If you already have an ordinary
impl, or are implementing an external trait, start with
[the impl entry in §1.6](#16-start-from-an-ordinary-display-impl).
Both are everyday routes; neither requires shape templates or generators.

Use the remaining chapters by topic. Learn ordinary types and generics
before packs, which become useful when you need several type arguments or
per-position wrappers. The task already introduces signature copying and simple
delegation. Consult complete directive rules, generators and custom
extensions when needed, rather than reading every chapter in order.

With the dependency configured as described in the
[README](https://github.com/5-6-1/batch-impl-rs/blob/main/README.md), each complete
program in §1 can replace `src/main.rs`; run it with `cargo run`. Steps
§1.1–§1.5 preserve the previous behavior and introduce only their stated
change; §1.6 is a separate program for the other entry route.
Code blocks from §2 onward are independent examples. Some include lines starting with `# ` for
rustdoc's hidden setup: when copying Markdown source, remove that prefix
and keep the rest of the line. It is a documentation convention, not DSL
syntax. The English Rust blocks are doctests unless marked `ignore`;
diagnostic demonstrations below use `text` blocks.

| If you want to… | Go to | Tier |
|---|---|---|
| extend one working program step by step | [§1](#1-implement-and-call-one-method) | basic |
| batch an ordinary impl, including one for an external trait | [§1.6](#16-start-from-an-ordinary-display-impl) | basic |
| maintain targets, signatures and exceptions | [§1.7](#17-maintain-the-same-describe-trait) | as needed |
| wrap a matrix of types (space, `.`, lists) | [§2](#2-type-matrix-the-space-and-) | basic |
| combine independent and shared bodies | [§3](#3-lists-and-body) | basic |
| declare generics, inherit or add bounds, use a qualified type | [§5](#5-generics-) | basic |
| use ordinary Rust `where` bounds | [§8.1–§8.3](#81-where-predicates) | basic |
| consult complete directive rules | [§7](#7-the-directive-system-) | as needed |
| map members and splice arguments (`*`) | [§4](#4-packs---mapping-and-splicing), after generics | as needed |
| generate tuples, every arity, and Cartesian matrices | [§9](#9-tuple-generation-and-matrices) | as needed |
| use references, pointers, `unsafe`, attributes, `!`, `self` | [§10](#10-the-modifier-gallery) | as needed |
| pick between the entry macros | [§11](#11-entry-points) | as needed |
| see what an error means | [§12](#12-error-hints) | as needed |
| read a whole real file | [§13](#13-real-scenarios-the-three-bundled-examples) | as needed |
| address generated parameters (`@N` / `@g_i` / ranges) | [§6](#6-the--constant-system-macro-meta-layer) | advanced |
| extend impls with shape templates and staged substitution | [§8.4–§8.5](#84-the-impl-shape-templates-080) | advanced |

## 1. Implement and Call One Method

Start by giving `u8` a callable `describe` method:

```rust
use batch_impl::batch_impl;

#[batch_impl(u8 #describe{format!("number: {self}")})]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    println!("{}", 7u8.describe());
}
```

Run `cargo run`: the assertions pass and the program prints `number: 7`.
`u8` is the target type. `#describe{...}` copies `Describe::describe`'s
signature into the impl and supplies its body; `self` is the value receiving
the call.

The name after `#` is the trait member's actual name. `#describe` fills
`describe`; the notation `#name{body}` in documentation means “put the member
name here,” not a fixed keyword named `name`. The trait stays available for
ordinary Rust method calls.

### 1.1 Add Types

Replace the single `u8` with `[u8, u16, u32]` and add assertions for the
new types. Every step supplies a complete program: replace the previous
version and run `cargo run` again.

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")}
)]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    println!("{}", 7u8.describe());
}
```

To return to one target, write `u8` or `[u8,]`. **In a type position,
`[u8]` is a slice type, not a list containing only `u8`**; `[u8; 3]` is a
fixed-length array. Directive member selectors are a different context:
for example, `#fill([describe])` does not need a trailing comma.

### 1.2 Add a Method

Add `kind` to the same trait and supply its implementation with `#kind`.
The three existing `describe` implementations stay unchanged. Both methods
are called through ordinary Rust syntax.

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    println!("{}", 7u8.describe());
}
```

The two directives fill two separate members. Each body contains only its
return expression; the trait still supplies the signatures. See
[§7.1](#71-namebody--single-item-assignment)
for the complete form.

### 1.3 Give a Special Type Its Own Spec

`bool` needs a different description and kind, so give it a separate spec,
separated from the numeric spec by a comma. Do not put it in a list with a
shared `describe` body and expect its local body to override that method:
**combining bodies appends members; it does not override them. Rust rejects
duplicate definitions of the same member.**

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    println!("{}", 7u8.describe());
}
```

The integers share one implementation, while `bool` gets another; the two
sets of targets do not overlap. Independent and shared bodies can add
different members. See [§3](#3-lists-and-body)
for a complete example of combining them.

### 1.4 Add a Generic Bound

Now add a spec for `Vec<T>`. `<T: std::fmt::Debug>` declares the impl's
generic parameter and bound. The `format!("{self:?}")` expression needs
elements that implement `Debug`, so the bound serves a purpose. Keep the
existing integer and `bool` implementations.

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{self:?}")} #kind{"list"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    println!("{}", 7u8.describe());
}
```

This spec generates `impl<T: std::fmt::Debug> Describe for Vec<T>`.
The declaration uses an ordinary Rust bound; generated parameter names and
positional references are not needed. More rules for ordinary generics are
in [§5](#5-generics-).

### 1.5 Forward Box Methods

Finally add `#blanket(@all_ref_methods){Box}`. It forwards both reference
receiver methods on `Box<T>` to the inner `T`. The generated impl requires
`T: Describe`, so the previous integers, `bool` and `Vec<T>` can all be
wrapped.

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{self:?}")} #kind{"list"},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    let boxed = Box::new(vec![1u8, 2]);
    assert_eq!(Describe::describe(&boxed), "[1, 2]");
    assert_eq!(Describe::kind(&boxed), "list");
    println!("{}", 7u8.describe());
}
```

The two `Describe::...(&boxed)` calls explicitly check the trait impl for
`Box` itself, so method-call autoderef cannot hide a missing wrapper impl.
This step changes neither the trait nor the two method signatures.
[§7.4](#74-blanketall_methodswrapper-matrix--blanket-delegation)
covers other receivers and associated items.

**You can start using the library here.** Lists, member bodies, separate
specs, ordinary generics and simple forwarding cover many batches of impls.
For an existing complete impl, see [§1.6](#16-start-from-an-ordinary-display-impl);
for later maintenance, see [§1.7](#17-maintain-the-same-describe-trait).
Consult the remaining chapters when a task calls for them.

Space application is a normal spelling (`Vec u8`); make nesting explicit
with parentheses, as in `Box (Vec u8)`, or use `Box.Vec.u8`. Put each spec
on its own line and indent longer method bodies. `rustfmt` does not
guarantee formatting that understands this DSL's macro arguments, so check
the grouping and layout of a matrix yourself.

### 1.6 Start from an Ordinary Display Impl

If you already have an ordinary impl, adding an attribute to it is another
everyday starting point. This separate, complete program reuses a `Display`
implementation for local `UserId` and `OrderId` types. Its signatures come
from the impl you wrote; no copy of the external trait definition is needed.

```rust
use batch_impl::batch_impl;

use std::fmt;

struct UserId(u64);
struct OrderId(u64);

#[batch_impl(@Self: [UserId, OrderId])]
impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn main() {
    assert_eq!(UserId(7).to_string(), "7");
    assert_eq!(OrderId(12).to_string(), "12");
}
```

In the added attribute, `@Self` reads the input impl's self type, `UserId`;
the list on the right supplies its two targets. The generated impls replace
the input block, so include `UserId` to keep its implementation. The `fmt`
signature, field access and `write!` are ordinary Rust. Both types have the
same `.0` field, making the body suitable for both; Rust's orphan rules and
other checks still apply.

Choose a local trait definition with `#method{body}`, or an existing complete
impl with `@Self: target list`, according to the code you already have.
Explicit shape templates and staged substitution are further techniques,
covered in [§8.5](#85-the-impl-entry-080-itemimpl).

### 1.7 Maintain the Same Describe Trait

Continue from [§1.5](#15-forward-box-methods), rather than the separate
`Display` example. Suppose you now need `u64`, a prefix argument for the
description, and a special format for `u32`:

| Requirement | Change by hand | What the macro still handles |
|---|---|---|
| Add `u64` | Add it to the integer target list | Generate the same members for it |
| Add `prefix: &str` to `describe` | Change the trait signature, use `prefix` in each business body, and supply the argument at call sites | Copy the new signature; `Box` delegation forwards the new argument |
| Give `u32` a special format | Remove `u32` from the shared list and give it a separate spec | Generate each target's impl without conflicting implementations for one type |

Copying a signature does not rewrite business logic or callers. The final
program keeps `kind` unchanged and updates the bodies that need the new
format. The existing `#blanket` needs no change:

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u64] #describe{format!("{prefix}number: {self}")} #kind{"integer"},
    u32 #describe{format!("{prefix}wide number: {self}")} #kind{"integer"},
    bool #describe{format!("{prefix}bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{prefix}{self:?}")} #kind{"list"},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self, prefix: &str) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe("value: "), "value: number: 7");
    assert_eq!(8u16.describe(""), "number: 8");
    assert_eq!(10u64.describe(""), "number: 10");
    assert_eq!(9u32.describe("value: "), "value: wide number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(9u32.kind(), "integer");
    assert_eq!(true.describe(""), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(""), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    let boxed = Box::new(vec![1u8, 2]);
    assert_eq!(Describe::describe(&boxed, "value: "), "value: [1, 2]");
    assert_eq!(Describe::kind(&boxed), "list");
    assert_eq!(
        Describe::describe(&Box::new(9u32), "value: "),
        "value: wide number: 9"
    );
    println!("{}", 7u8.describe(""));
}
```

Targets retaining their existing business logic keep the previous format
with an empty prefix. The new `u32` format also passes through `Box<u32>`
delegation. The assertions cover the added target,
changed signature, special implementation and argument forwarding.

### Inspecting the Expansion

Once the program works, use expansion preview when a changed type expression or
signature is hard to diagnose. Temporarily wrap the annotated trait in
`batch_impl::batch_preview!`:

```text
batch_impl::batch_preview! {
    #[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
    trait Describe { fn describe(&self) -> String; }
}
```

Run `cargo check`. Preview deliberately reports the generated Rust through
`compile_error!`, so a failed compilation here is expected; it does not by
itself mean the DSL input is invalid. Check the target types and method
signatures in the output, restore the ordinary attribute form, and rerun
`cargo run`. Preview is a temporary inspection step.

For a method-free marker trait, the targets alone are enough. For more
involved impls, the spec can also name generics and the trait application:

```text
<impl-generics> TraitName<trait-generics> TargetType { body }?
```

| Part                  | Example                              | When needed              |
|-----------------------|--------------------------------------|--------------------------|
| `<impl-generics>`     | `<T>`, `<T: Clone>`, `<const N: usize>` | when the impl block needs generic params |
| `TraitName<trait-generics>` | `MyTrait<T>`, `MyTrait<Vec<T>>` | when the trait definition has generic params |
| Target type           | `usize`, `Vec<T>`, `&str`            | required                  |
| `#method{body}`        | `#describe{format!("number: {self}")}` | to copy that method's signature and supply its body |
| `{ body }`            | `{ fn m(&self) -> usize { 0 } }`     | when a custom body is needed |

Multiple specs are separated by `,`: `#[batch_impl(usize, isize)]`.

**What this buys you at scale.** [simplify.rs](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs) gets **30 impls** out of roughly **15 lines** of DSL (hand-writing them takes ~80), and [typeclass.rs](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/typeclass.rs) covers a class hierarchy plus 36 `From<bool>` instances. Both are compiled by CI, and §13 walks through them when you are ready for a larger example.

## 2. Type Matrix: the space (and `.`)

**The space is the natural way to apply**: write the container/modifier and the types it takes side by side — chaining accumulates arguments left-associatively.

`Box u32` supplies one argument to `Box`. `HashMap u32 String` supplies
two arguments to `HashMap`. To make an argument itself a composed type,
group it: `Box (Vec u32)` gives `Box<Vec<u32>>`.

| Writing                    | Expansion                            |
|----------------------------|--------------------------------------|
| `Box u32`                  | `Box<u32>`                           |
| `Box (Vec u32)`            | `Box<Vec<u32>>` (grouped inner type) |
| `HashMap u32 String`       | `HashMap<u32, String>` (left-associative accumulation) |
| `fn(A,B) C`                | `fn(A,B)->C` (or write `fn(A,B) -> C`) |
| `&u8`                      | `&u8` (chained modifiers)            |
| `Tr u8`                    | `impl Tr for u8` (a bare trait name) |
| `[Box, Vec] u32`           | `Box<u32>, Vec<u32>` (lists expand)  |
| `HashMap<u8> String`       | `HashMap<u8, String>` (prefilled generics appended) |
| `Box [u8, u16]`            | `Box<u8>, Box<u16>` (list distributes) |
| `[Box, Vec] [u8, u16]`     | Cartesian product, 4 entries         |

**`.` is the same operation with right-associative grouping.** It provides
another way to write nesting: `Box.Vec.u32` and `Box (Vec u32)` both give
`Box<Vec<u32>>`. Without grouping, spaces accumulate arguments:
`Box Vec u32` gives `Box<Vec, u32>`.

| Writing                    | Expansion                            |
|----------------------------|--------------------------------------|
| `Box.Box.u8`               | `Box<Box<u8>>` (right-associative nesting) |
| `Box.Vec.u32`              | `Box<Vec<u32>>` (same as `Box (Vec u32)`) |
| `&Box u8`                  | `&Box<u8>` (modifier over the nested type) |
| `[Box, Vec] T`             | `Box<T>, Vec<T>`                     |
| `Box [T1, T2]`             | `Box<T1>, Box<T2>`                   |
| `[HashMap<K>, Vec<K>] V`   | `HashMap<K, V>, Vec<K, V>`           |

> **When to use which**: write a container and its arguments side by side
> (`Box u8`, `HashMap<u8> String`). For nested containers, group the inner
> type with parentheses (`Box (Vec u32)`) or use dots (`Box.Vec.u32`).

**The annotated trait's own name** applies as the impl trait: with the attribute on `trait Tr`, writing `Tr u8` gives `impl Tr for u8` and `Tr<A> u8` gives `impl Tr<A> for u8`. Any *other* bare ident is a type, not a trait head — with the attribute on `Tr`, a spec `Other u8` produces `impl Tr for Other<u8>`. Write `Tr<u8>` for the **type** `Tr<u8>`. In general, a bare `Tr` is not recommended.

Precedence from low to high: `;` < `,` < space < `.`; `()` grouping sits above all operators.

> **Mixed grouping**: in `Box.Vec u32`, the dot groups first, then the space
> adds another argument, producing `Box<Vec, u32>`. If you meant
> `Box<Vec<u32>>`, write `Box (Vec u32)` or `Box.Vec.u32`.

> **Operand strictness**: both sides of `.`/space/`,` must have operands — `A.`, `.A`, `,A`, `A,,B` all report `compile_error!`; only **trailing commas** (`A,` / `[A, B,]`) are allowed, and `()`/`[]` brackets are real tokens, not empty operands. `;` stays lenient as a `batch_trait!` section boundary.

```rust
use batch_impl::batch_impl;
use std::collections::HashMap;
#[batch_impl(Box (Vec u32), HashMap<u8> String)]
trait T {}
// → impl T for Box<Vec<u32>> {}   (also written Box.Vec.u32)
// → impl T for HashMap<u8, String> {}
```

## 3. Lists and Body

### Side-by-side lists `[A, B]`

One body is reused for all target types. This is an alternative to §1.1's
`#describe` form: writing a complete method in the shared body gives the
same three implementations.

```rust
use batch_impl::batch_impl;
#[batch_impl([u8, u16, u32] {
    fn describe(&self) -> String { format!("number: {self}") }
})]
trait Describe { fn describe(&self) -> String; }
// → impl Describe for u8 { fn describe(&self) -> String { format!("number: {self}") } }
// → impl Describe for u16 { ... }
// → impl Describe for u32 { ... }
```

**Distribution propagation**: `[A, B]` lists are distribution sources — beyond being targets/operands, nested positions propagate too:

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, [u16, u32, u64]))]
trait T {}
// → impl T for (u8, u16,) {}
// → impl T for (u8, u32,) {}
// → impl T for (u8, u64,) {}

#[batch_impl(Vec<[u8, u16, u32]>)]
trait V {}
// → impl V for Vec<u8> {}
// → impl V for Vec<u16> {}
// → impl V for Vec<u32> {}
```

`[A, B]` in a tuple or generic argument distributes over the target types;
multiple lists produce all combinations. Nested lists expand too:
`Vec<[[A,B], C]>` generates `Vec<A>`, `Vec<B>` and `Vec<C>`.
For a list reduced to one type, keep `[A,]` or write `A`; `[A]` is a slice type.

### Independent/shared body merging

List items may carry independent bodies that combine with the shared body.
The members are appended to the same impl, without replacement by name;
Rust rejects duplicate definitions of one member. Here the independent
`name` and shared `zero` are different members, so they can be combined:

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
// → impl Tagged for usize { fn zero() -> Self { Default::default() } fn name... "usize" }
// → impl Tagged for isize { fn zero() -> Self { Default::default() } fn name... "isize" }
// → impl Tagged for f32   { fn zero() -> Self { Default::default() } fn name... "f32" }
```

For a type that needs a different implementation of the same method, split
it into a separate spec as in
[§1.3](#13-give-a-special-type-its-own-spec),
so each target receives only one definition.

## 4. Packs `*` — Mapping and Splicing

A pack is a list of type expressions waiting for a host. `*X` opens the direct
members of a **candidate list**; every other type — including a tuple, the unit
type, a slice and an array — becomes a one-member pack. The star is what tells
`*[A, B]` (two members) from `*(A, B)` (one member of the tuple type), and `*[]`
(the empty pack) from `*()` (one member of the unit type).

Three steps cover the common cases: **open members, apply a rule, place the
result**. `(*Vec *[].3,)` opens three fresh parameters, wraps each in `Vec`,
then places the results in one tuple.

### 4.1 In-list / in-tuple splicing

An ordinary type inside a pack remains whole. Nested packs splice, but an
ordinary tuple is still one type.

```rust
use batch_impl::batch_impl;

#[batch_impl([u8, *[u16, u32]])]
trait Each {}

#[batch_impl((u8, *[u16, u32]))]
trait Together {}

#[batch_impl(*(u8, u16))]
trait OneTuple {}

fn main() {
    fn each<T: Each>() {}
    fn together<T: Together>() {}
    fn one<T: OneTuple>() {}
    each::<u32>();
    together::<(u8, u16, u32)>();
    one::<(u8, u16)>();
}
```

### 4.2 Left operand: apply one rule to each member

A left pack maps its members over the right operand. `*[Vec, Box] u8`
produces `Vec<u8>` and `Box<u8>`.
An ordinary left type keeps the right pack as one argument slot:
`Pair *[u8, u16]` becomes `Pair<u8, u16>` when that slot is consumed.

When both operands are packs, each **direct right member is one row**.
All left members receive that whole row. Right rows are outermost; left
members are innermost. An already-built row is not opened again during
that mapping task.

```rust
use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl((*Vec *[].1..=3,))]
trait Wrapped {}

#[batch_impl((*Pair (*[self, Vec] *[].1..=3),))]
trait Paired {}

#[batch_impl((*() (*[self, Vec] *[].3),))]
trait Rows {}

fn main() {
    fn wrapped<T: Wrapped>() {}
    fn paired<T: Paired>() {}
    fn rows<T: Rows>() {}
    wrapped::<(Vec<u8>, Vec<bool>)>();
    paired::<(Pair<u8, Vec<u8>>, Pair<bool, Vec<bool>>)>();
    rows::<((u8, Vec<u8>), (bool, Vec<bool>), (i32, Vec<i32>))>();
}
```

`self` returns its whole argument. Thus `*[self, Vec]` builds the two
members `T, Vec<T>` for each independently generated `T`. `*Pair`
consumes each such row as generic arguments; `*()` consumes each row
as tuple elements. This is the same mapping rule in both examples.

Space remains left-associative. Use `(*Vec (*Box *[].2),)` for
`(Vec<Box<T0>>, Vec<Box<T1>>)`; `*Vec *Box *[].2` first builds
`Vec<Box>`, then appends another argument.

### 4.3 Generic args and trait paths

Literal angle brackets consume their argument slots; they do not replay
application. Both ordinary type arguments and trait arguments can splice
packs.

```rust
use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl(Pair<*[u8, u16]>)]
trait Concrete {}

#[batch_impl(Convert<*[u8, u16]> Pair<u8, u16>)]
trait Convert<A, B> {}

#[batch_impl(Pair<*[u8, u16].2>)]
trait Matrix {}

fn main() {
    fn concrete<T: Concrete>() {}
    fn convert<T: Convert<u8, u16>>() {}
    fn matrix<T: Matrix>() {}
    concrete::<Pair<u8, u16>>();
    convert::<Pair<u8, u16>>();
    matrix::<Pair<u8, u16>>();
    matrix::<Pair<u16, u8>>();
}
```

The last expression selects a two-position Cartesian product, then fills
two generic argument slots. An ordinary choice remains a branch:
`Pair<*[u8, [u16, u32]]>` gives two impls, not three arguments.

### 4.4 Container rule

Parentheses do not inspect the type inside them: `(X)` is a group,
`(X,)` is a tuple. Likewise `[X]` is a slice and `[X,]` is a choice
list. Consequently `(*[u8, u16])` is a grouped pack (two target impls),
while `(*[u8, u16],)` is one tuple. `[*[u8, u16]]` is invalid: a
slice has exactly one element-type slot.

A **list distributes** where a pack is rejected: the element slot of a slice or
array takes a list, so `[[u8, u16]]` is two slices and `[[u8, u16]; 4]` is
`[u8; 4]` and `[u16; 4]` — the way to wrap a family when there is no builder
spelling. A pack in the same slot is the cardinality error above.

Nested prefixes are idempotent: `*(*X)` is `*X`. There is no separate
double-star operation. To retain a row, put it in an ordinary tuple or
generic host, as in §4.2.

### 4.5 Generators and dimensions

`*[].N` generates a pack of `N` independent parameters. Copying a
generated member preserves its identity; executing another generator
creates another group. `*[].0` creates no parameters.

Emptiness is a *base*, not a target: `*[]` on its own is a spec that expands to
nothing and is diagnosed (write the targets out, or size it), while `*[].N` is
the generator above. An empty pack in an argument position (`Vec<*[]>`) is
diagnosed too.

Ordinary tuple powers still repeat their **direct slots**:
`([u8, u16],).2` has four combinations, and `(*[u8, u16],).2`
materializes as `(u8, u16, u8, u16)`. Pack powers first splice nested
packs, then use the resulting members as their choices.

Two axes use the same application rule. Keeping rows makes the dimensions
visible in the Rust type:

```rust
use batch_impl::batch_impl;

struct Map<T, U>(T, U);

#[batch_impl((*() (*Map *[].1..=2 *[].1..=3),))]
trait Grid {}

fn main() {
    fn grid<T: Grid>() {}
    grid::<(
        (Map<u8, bool>, Map<u16, bool>),
        (Map<u8, i32>, Map<u16, i32>),
        (Map<u8, char>, Map<u16, char>),
    )>();
}
```

There are six shapes. A fixed `(*Map *[].2 *[].3,)` instead splices
the six members into one flat tuple and shares five parameters.
Flattening *ranges* of both dimensions can generate overlapping impls:
the `1 × 2` and `2 × 1` patterns can describe the same Rust type.
The macro keeps both; rustc reports E0119. It also keeps unused generic
declarations: an empty second axis can leave E0207. See the Pack model
[worked examples](https://github.com/5-6-1/batch-impl-rs/blob/main/tests/pack_model/tutorial.md).

### 4.6 Legal positions

Tuple elements, generic and trait arguments, and callable parameters accept
multiple members. Reference and pointer targets, slice/array element types,
function returns, individual bounds and associated-type binding values
require exactly one type **in each branch**. Empty or multi-member packs
there produce a targeted error.

Declaration blocks splice names (`<*[A, B]>`); a fresh generator cannot
declare names there because its own declarations have no target to inhabit.
Constructed types are not parameter declarations either: `<*[Vec<u8>,]>` is an error.
Raw pointers `*const T` and `*mut T` keep their Rust meaning. A bare
`*` with no block is an error.

`where{...}` predicates and `impl{...}` shape templates remain standard
Rust type domains with their existing `@` substitutions. Bodies and
directive arguments retain their own syntax. They do not acquire pack operators.
Qualified-path continuations (`::Assoc<...>`) and trait paths following `as` in
`<T as Trait>` remain ordinary Rust paths, without pack splicing inside them.
See the [reference manual](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md),
§2 and §4.

## 5. Generics `<>`

### 5.1 Declarations

`<...>` before the trait name declares impl generics — copied into the impl as-is (bounds and `const` parameters included):

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T: Clone> Box<T>)]
trait CloneBox {}
// → impl<T: Clone> CloneBox for Box<T> {}

#[batch_impl(<const N: usize> [u8; N])]
trait ArrayLen {}
// → impl<const N: usize> ArrayLen for [u8; N] {}
```

### 5.2 `A<>` — copied as-is

An empty `<>` copies the trait's own generics - bounds and `where` predicates included - as the impl's generics:

```rust
# use batch_impl::batch_impl;
#[batch_impl(A<> Vec<u8>)]
trait A<T, const N: usize> {}
// → impl<T, const N: usize> A<T, N> for Vec<u8> {}
```

This shorthand belongs to the **spec head** (the trait application): it is what
declares those formals. Written anywhere else — e.g. on the target — an empty
`<>` is the *sync marker* of §6.4 instead, which fills in the spec's arguments
(`Swap2<>` → `Swap2<T>`); it never emits a declaration in the middle of a spec.

### 5.3 Args: multi-args, nesting, bindings

```rust
# use batch_impl::batch_impl;
struct Map<K, V>(K, V);
struct A; struct B; struct C;
struct Wrap<X>(X);
#[batch_impl(Map<A, B>)]                 // multi-args
trait M1 {}
#[batch_impl(Map<Map<A, B>, C>)]         // the nested type stays one argument
trait M2 {}
#[batch_impl(Conv<u8, Item = u8> Wrap<u8>)]  // associated-type binding (trait path)
trait Conv<T> { type Item; }
```

### 5.4 Operations inside `<>` (programmable in 0.7.0)

> **Optional extension.** Ordinary generics only need parameter declarations
> and uses. The following forms generate combinations inside arguments;
> if you do not need generators yet, continue to §5.5.

Generic-argument positions accept full DSL expressions — the structural landing of 0.7.0:

```rust
# use batch_impl::batch_impl;
struct Wrap<X>(X);
struct Pair3<A, B>(A, B);
struct A2; struct B2;

#[batch_impl(Wrap<()2>)]               // generator: <P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1,)>(the tuple stays a single arg)

#[batch_impl(Pair3<*[].2>)]             // generator splat: <P0,P1> Pair3<P0,P1>
trait GenSpl {}
// → impl<P0,P1> GenSpl for Pair3<P0, P1>(flattened into two args)

#[batch_impl(Wrap<@u*>)]                // constant family: 6 impls (u8..usize)
trait ConstArg {}

#[batch_impl(Wrap<[A2, B2]>)]           // array: 2 impls (Wrap<A2>/Wrap<B2>)
trait ListArg {}
```

### 5.5 Same-name inheritance and trait where inheritance

When the trait's generic params share names with the spec's args, bounds inherit automatically — and inheritance is **positional**, so a renamed parameter keeps its predicate too:

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Box<T> where Box<T>: Clone {})]
trait B2 {}
// → impl<T> B2 for Box<T> where Box<T>: Clone {}
```

```rust
# use batch_impl::batch_impl;
// positional inheritance: the trait's `T` pairs with the spec's first argument
#[batch_impl(<X> Store<X> usize)]
trait Store<T>
where
    T: Clone,
{}
// → impl<X: Clone> Store<X> for usize
```

### 5.6 Qualified types: `<T as Tr>::Assoc`

A qualified type picks an associated item through `::`, and every spelling of it
is accepted: the projection (`<T as Tr>::Assoc`), the qualified path
(`Foo<T>::Assoc`) and the turbofish (`Foo::<u8>::Assoc`, rendered as
`Foo<u8>::Assoc`). They work anywhere a type does — as the target, as a generic
argument, inside a bound, and nested:

```rust
# use batch_impl::batch_impl;
trait Tr { type Assoc; }
struct S;
impl Tr for S { type Assoc = u8; }
impl Tr for u8 { type Assoc = u16; }
struct Holder<T>(T);
struct Wrap<T>(T);

// target: `impl Q for Holder<<S as Tr>::Assoc>` (= `Holder<u8>`)
#[batch_impl(Holder<<S as Tr>::Assoc>)]
trait Q {}

// nested projection as a generic argument:
// `impl Q2 for Vec<<<S as Tr>::Assoc as Tr>::Assoc>` (= `Vec<u16>`)
#[batch_impl(Vec<<<S as Tr>::Assoc as Tr>::Assoc>)]
trait Q2 {}

// inside a bound: `<T: From<<S as Tr>::Assoc>> Q3<T> Wrap<T>`
#[batch_impl(<T: From<<S as Tr>::Assoc>> Q3<T> Wrap<T>)]
trait Q3<T> {}
```

The `::`-tail is **plain Rust path text**: it is never re-parsed, so a DSL token
inside it (`Holder<T>::Assoc<@0>`) is rejected with a targeted error instead of
leaking into the generated impl.

### 5.7 Bound positions, global paths and named fn parameters

Three further Rust spellings are accepted:

```rust
# use batch_impl::batch_impl;
// Associated-type bindings are legal on **any** trait path in a bound position:
// `dyn`, `for<'a>` and an inline bound all take them. A plain type's args are
// still a plain type list (`Vec<Item = u8>` stays a targeted error).
#[batch_impl(Box<dyn Iterator<Item = u8>>)]
trait Q4 {}

// A leading `::` makes a path **global** — at the start of a spec, nested in an
// args list, and before a `::`-tail.
#[batch_impl(::std::vec::Vec<u8>)]
trait Q5 {}

// A `fn(...)` **pointer** type may name its parameters. The name stays verbatim
// and the type after `:` is parsed, so DSL operands work inside it
// (`fn(v: Box<u8>) -> u8`).
#[batch_impl(fn(x: u8) -> u8)]
trait Q6 {}
```

`Fn(x: u8)` is **not** accepted: rustc itself rejects named parameters in the
`Trait(...)` sugar ("does not support named parameters"), so the DSL reports
that rule instead of leaking a confusing `expected type` error.

**A target that starts with `::` needs an explicit boundary**: `<...>` (after an ident) and `::` *continue* the current path, while the space and `.` are element boundaries, so the juxtaposed form glues the head and an absolute-path target into one path — the trait then lands in type position (E0782). Write the `.`:

```rust
# use batch_impl::batch_impl;
// `@trait<u8> . ::std::string::String` → impl TrE<u8> for ::std::string::String
#[batch_impl(@trait<u8> . ::std::string::String)]
trait TrE<T = usize> { fn tag(&self) -> u8 { 7 } }
```

With a trait head the space and `.` are equivalent, and in edition 2024 `::name` names an **external crate** (write `crate::...` for this crate's root). The full rule and its boundary table — the glued form, a group appending an argument, a one-element spec — are in `docs/reference.md` §1.2.

## 6. The `@` Constant System (macro-meta layer)

`@` is the DSL's reserved **library-owned constant namespace** — `#` is taken by the directive mechanism, so `@` provides "name and reuse type-matrix entries". It is pure **lexical substitution** (the macro-meta layer): the expanded result enters the pipeline and participates in no in-domain parsing.

> **Read as needed.** §6 covers constants and addressing generated parameters.
> The basic signature-copying directives from §1 are detailed in §7.1–§7.2;
> delegation and extensions in §7.3–§7.5 can wait. For ordinary bounds,
> tuples and modifiers, continue to §8.1–§8.3, §9 and §10.

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

**Range families** select a contiguous run of one family. As in Rust,
`..` excludes the written upper endpoint and `..=` includes it:
`@u8..u16` → `u8`, while `@u8..=u16` → `u8, u16`.
`@u8..=u128`, `@i8..=i128`, and `@f32..=f64` select their full width families.
Either endpoint may be **omitted**: `@..u16` ≡ `@u8..u16`,
`@..=u16` ≡ `@u8..=u16`, `@u16..` ≡ `@u16..=u128`, and
`@f32..` ≡ `@f32..=f64`. At least one endpoint anchors the family;
`..=` always requires an upper endpoint.
`usize`/`isize` only enter name families, not range families.
Descending ranges and empty exclusive ranges such as `@u8..u8` or `@..u8`
are reported; they do not silently produce an empty type matrix.

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
| `@N..=M` | a contiguous run | the fresh names N..=M, comma-separated (`@0..=1` → `P0, P1`) |
| `@N..` | an **open** run to the last fresh | every fresh name from N to the last, comma-separated (`@1..` → `P1, P2, ...`); **empty** when N is past the end (an arity-1 impl contributes no such predicate, no error) |

Fresh display names are numbered `P0, P1, ...` in document order (a collision
with an ident the impl already writes escapes by spreadsheet-style letter
suffixes: `P0A`, `P0B`, ... `P0Z`, `P0AA`) — the expansion splices the names where the `@` sits (a where
predicate subject, a target tuple element, a generic argument), so a range
becomes several names and a `where` tail is copied per fresh.

> **Power-user tier**: `@g_i` / `@N..M` are advanced addressing notations — start from `@u*` / `@all_methods` / `@0` and reach for them only when a predicate must name a specific fresh. See the [README](https://github.com/5-6-1/batch-impl-rs/blob/main/README.md) for the compatibility policy and the [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/CHANGELOG.md) for explicit migration notes.

> **The `_` in a reference is the group/position separator**, not Rust's digit
> separator: `@1_0` is group 1, position 0. Write `@1000` (no separator) when you
> mean the flat index 1000; the reference's §5 boundary material covers the
> `@1_000` reading.

```rust
# use batch_impl::batch_impl;
#[batch_impl(()2 where @0..=1: Clone)]   // range sugar: @0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1,) where P0: Clone, P1: Clone

#[batch_impl(()3 where @0..: Copy)]       // from 0 to the last fresh
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2,) where P0: Copy, P1: Copy, P2: Copy

#[batch_impl(()3 where @1..: Copy)]       // open range: from index 1 on
trait OpenRange {}
// → impl<P0,P1,P2> OpenRange for (P0,P1,P2,) where P1: Copy, P2: Copy
// (an arity-1 impl contributes no predicate — `@1..` is empty there)
```

**`@all_fresh` has been removed**: replace existing uses with `@0..`.
The `@N..` family covers the whole run with `@0..`, or its tail with `@1..`.

**Ranges work anywhere a single `@N` can** (0.9.2): beyond the where
predicates above, the range's tail may be an associated-type path, copied
per fresh — and a range in a target position re-opens against the fresh
list the spec's generators produced:

```rust
# use batch_impl::batch_impl;
struct Wrap3<A, B, C>(A, B, C);
#[batch_impl(Wrap3<*[].3> where @0..: Clone { fn m(&self) {} })]
trait RangeAngle { fn m(&self); }
// → impl<P0,P1,P2> RangeAngle for Wrap3<P0,P1,P2> where P0: Clone, P1: Clone, P2: Clone

trait HasOut { type Out; }
#[batch_impl(Wrap3<*[].3> where @0..: HasOut, @0..::Out: Clone { fn m(&self) {} })]
trait RangeAssoc { fn m(&self); }
// → where P0: HasOut, P1: HasOut, P2: HasOut, P0::Out: Clone, P1::Out: Clone, P2::Out: Clone
```

The fresh list a range indexes comes from the spec's generators (`*[].N` /
`().N`); a **closed** range in a spec with no fresh generics reports "out of range", while an **open** one (`@0..`) contributes no predicate and no error (an arity-1 impl contributes no such predicate).

**The impl-generic declaration position** works too: `<@0..>` declares every
fresh the range covers as an impl param — so a spec can put the generator in
the trait args (`GenConv<*[].2>`) and reference the same fresh batch in the
declaration and the predicates:

```rust
# use batch_impl::batch_impl;
struct DeclTarget;
#[batch_impl(<@0..> GenConv<*[].2> DeclTarget where @0..: Clone { fn m(&self) {} })]
trait GenConv<T, U> { fn m(&self); }
// → impl<P0,P1> GenConv<P0,P1> for DeclTarget where P0: Clone, P1: Clone
```

(An empty `<@0..>` — no fresh generators in the spec — contributes no
parameters, like an empty `@1..` predicate.)

**Grouped ranges `@L_N..`** (0.9.2) slice **within one generator group** —
the in-group counterpart of `@g_i`, stable across array dispatch. With
several generators in one spec (such as `PairGen<*[].2, *[].3>`), the first is group 0 and the second group 1;
`@1_0..` constrains only group 1's fresh:

```rust
# use batch_impl::batch_impl;
struct MultiTarget;
#[batch_impl(
    <@0..> <@1..> PairGen<*[].2, *[].3> MultiTarget where @1_0..: Clone
    { fn m(&self) {} }
)]
trait PairGen<A, B, C, D, E> { fn m(&self); }
// → impl<P0,P1,P2,P3,P4> PairGen<P0,P1,P2,P3,P4> for MultiTarget
//     where P2: Clone, P3: Clone, P4: Clone   ← group 1 only (P0,P1 unconstrained)
```

`@L_N..` (open to the group's end), `@L_N..M` (excluding M), and
`@L_N..=M` (including M) all
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
// arity 2 → impl<P0,P1> Module<(), ()> for (P0,P1,)
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
| `@Self` | **input type** — the self type received by this impl attribute | reuse the prototype as a template or type argument (§8.5) |
| `@all_methods` etc. | **selection** — extract an item set from trait_def | `#fill(@all_required_methods, -foo)` precise selection |
| `@Cow` | **built-in `#blanket` wrapper constant** — `Cow<'_>` plus its packaged constraints (`@0: ToOwned + ?Sized, @0::Owned: @trait`) | blanket-usable `Cow` delegation (see §7.4) |

`@all` family combined with `-` subtraction selects arbitrary item subsets (`#fill(@all_required_methods, -foo)`); `@all_default*` / `@all_required*` distinguish default implementations from required methods.

`X<>` (empty angle brackets) syncs to the spec trait application — write
`Semiring<>` instead of repeating `Semiring<Additive, Multiplicative>`:

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

It fills the marker wherever a type sits, and `@trait<>` is equivalent (`@trait` expands to the trait path first). Two things to know when you write one: the marker is **ident-agnostic** — it is the spec's arguments that go in, so `Other<>` becomes `Other<…spec args…>` and an arity mismatch there is rustc's to report — and the **body** syncs only through a **switch template** `impl{Tr<>}` (the body is arbitrary Rust, so a `Vec<>` there is not a trait reference). The complete list of synced surfaces is `docs/reference.md` §13.2.

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
// → impl<R, T: Fn(P0) -> R, P0>  MultiArity<T, R> for (P0,)
// → impl<R, T: Fn(P0,P1)->R, P0,P1> MultiArity<T, R> for (P0,P1,)
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

The arguments may be a name list or an `@all` family, with `-name` exclusions:

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 #fill(@all_methods, -extra){ 0 })]
trait Markers {
    fn marker(&self) -> u8;
    fn extra(&self) -> u8 {
        7
    }
}
// → impl Markers for u8 { fn marker(&self) -> u8 { 0 } }   (the excluded `extra` keeps its default)
```

The shared scope syntax accepts a trailing comma (`#fill([marker,],){0}`)
and empty selections: `#fill(){...}`, `#fill([]){...}`, an empty `@all`
family, or a subtraction that removes every selected item. `#fill` and
`#delegate` then generate no members; `#blanket` still generates its wrapper
impls. Rust checks any required members left unimplemented. Every explicitly
named trait item must exist, including names being excluded; subtraction
cannot hide a typo such as `#fill(typo, -typo){...}`.

### 7.3 `#delegate(methods){...}` — delegate calls

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box.Vec.u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

For delegation inside a branch, write `receiver.#call`. It is a complete
call to the current method, including automatically forwarded arguments;
no `()` is needed. If a recognized marker appears, the whole directive content
becomes the method body. Without one, it remains the target expression shown
above. No separate template marker is needed.

```rust
# use batch_impl::batch_impl;
enum Buffer { Text(String), Bytes(Vec<u8>) }

#[batch_impl(Buffer #delegate(@all_methods, size = len){
    match self {
        Self::Text(inner) => inner.#call,
        Self::Bytes(inner) => inner.#call,
    }
})]
trait BufferOps {
    fn size(&self) -> usize;
    fn truncate(&mut self, len: usize);
}

let mut text = Buffer::Text("abcd".into());
let mut bytes = Buffer::Bytes(vec![1, 2, 3, 4]);
text.truncate(2);
bytes.truncate(3);
assert_eq!(text.size(), 2);
assert_eq!(bytes.size(), 3);
```

The two receiver types stay in their own branches. `size = len` makes each
call in `size` use `inner.len()`; `truncate` uses `inner.truncate(len)`.
Receivers can also be expressions such as `self.inner.as_ref().#call`, and
`inner.#call.into()` continues from the call result. Method type/const
parameters are forwarded explicitly; lifetimes stay inferred. No `.await`
is added automatically: write `inner.#call.await` when required.
Appending `()` would call the result itself, so `inner.#call()` requires a
callable return value.

Recognition is limited to this delegate body's expressions. Macro tokens,
attributes and nested item definitions are not rewritten and do not trigger
the body form. Ordinary `.call(...)` methods and open-extension directive
names are unchanged. See reference §6.5 for the complete rules.

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
// → impl<P0> NumOps for Box<P0> where P0: NumOps { fn inc(&mut self) { <_ as NumOps>::inc(&mut **self) } }
//   (generic over the fresh — every `P0: NumOps`, not just `u32`)

#[batch_impl(#blanket(@all_methods){&, Box})]
trait Len { fn len(&self) -> usize; }
// → impl<P0> Len for &P0     where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
// → impl<P0> Len for Box<P0> where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
```

Calls explicitly name the current trait, so same-named supertrait methods cannot make delegation ambiguous. Receiver types are inferred from the actual dereferenced value; static methods use `<T as Trait>::method(...)`. Async methods append `.await`, and method type/const arguments are passed explicitly, for example `<_ as Trait>::read::<U, N>(&**self, value).await`; lifetimes remain inferred.

> **`:N` deref depth** — how many wrapper layers to dereference. Default **1** for single wrappers (`&`, `Box`, `Rc`): a reference receiver needs N+1 derefs plus an explicit borrow (`&**self` or `&mut **self`). `Box.Arc:2` = `Box<Arc<T>>`, with a shared call such as `<_ as Trait>::method(&***self)`. An arbitrary wrapper's deref target need not be its type parameter. Single wrappers need no depth suffix.

> **By-value receivers**: `fn consume(self)` forwards as `<_ as Trait>::consume(*self)` — a by-value `self` IS the wrapper, one deref fewer. The wrapper must permit moving out the value, or the inner value must be `Copy`; generated impls carry a `#[doc]` note. Skip such methods with `@all_ref_methods` (the trait default stays) or hand-write `#name{...}`. Explicit `self: &Self` / `self: &mut Self` use the corresponding reference rule.

#### GATs, `Self`, and unsized targets (0.9.4)

**Generic associated types** are delegated by projection with their own
parameter names — `trait Iterable { type Iter<'a>: Clone where Self: 'a; }` becomes
`type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;` (the bare
projection would be missing the lifetime argument, E0107). Plain assoc
types/consts keep their existing `<T as Trait>::Item` projection. Result
bounds such as `: Clone` stay on the trait declaration; impl definitions
keep the GAT parameter declarations and `where` predicates, and pass only
lifetime/type/const names to the projection. `#fill` and `#name` use the same
associated-type declaration rule.

**Bare `Self`** in a method's ordinary parameters, return, or generic
constraints cannot be blanket-delegated: the wrapper and inner type differ.
This includes `U: Marker<Self>`, `where U: Marker<Self>`, and
`where Self: Marker<U>`; the error suggests a `#name{...}` body.
Receiver `Self`, `where Self: Sized`, and outlives conditions such as
`Self: 'a` or `Self: Sized + 'a` remain allowed, with Rust checking the
delegated target's obligations. `Self::Assoc` is allowed in parameters,
returns, and constraints: forwarding the associated item makes the two
projections equal. Attribute payloads are not read as constraints.

**`@?` unsized suffix**: a wrapper ending in `@?` (`Box@?`) adds `T: ?Sized`
to that spec's where clause, so the fresh generic can be an unsized target:

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynLen { fn dlen(&self) -> usize; }
impl DynLen for str { fn dlen(&self) -> usize { self.len() } }
// → impl<P0> DynLen for Box<P0> where P0: DynLen, P0: ?Sized — the fresh (and thus the target) may be unsized
```

#### `@Cow` — a constraint-carrying packing (the case study)

`@Cow` is a **built-in `#blanket` wrapper constant** (usable only in the
`#blanket` wrapper list). It packs `Cow<'_>` with
`@0: ToOwned + ?Sized` and `@0::Owned: @trait`. `Cow<'_, T>` dereferences
to `T`; `T::Owned: Trait` is an additional constraint included by this
constant. The wrapper and packaged predicates enter the ordinary blanket
pipeline together:

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

### 7.5 Open extension

A `#name(args){body}` whose name is not `fill`, `delegate` or `blanket` calls
a function-like macro with that name, even if the trait has a member with
the same name. The form without `(args)`, `#name{body}`, assigns a member.
This example uses the
library's reference open-extension macro, `batch_preprocess_test!`, to
generate two method implementations. It is not just a preprocessing or
token-display tool.

```rust
use batch_impl::{batch_impl, batch_preprocess_test};

#[batch_impl(u16 #batch_preprocess_test(add,inc){*self + 3})]
trait AddIncU16 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

fn main() {
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
}
```

batch-impl passes the target, selected method names, body and trait
definition through the four-part `{spec}(args){body} trait_def` protocol.
The reference macro copies signatures from the trait, uses `*self + 3` as
the return value, and emits a complete impl. For your own extension, your
macro decides what code to generate. The manual four-part form is described
in the [open-extension API documentation (repository source)](https://github.com/5-6-1/batch-impl-rs/blob/main/src/doc/directive_open.md).

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

Rust-style constraint/body separation (when a body is present, its `{...}` follows the predicates):

> Equivalently, `where{predicates} {code block}` (the §8.1 suffix + a chained body) can be written bare as `where predicates {code block}`, saving one `{}` layer.

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 where u8: Clone { fn tag(&self) -> &'static str { "u8" } })]
trait T { fn tag(&self) -> &'static str; }
```

### 8.3 Predicate inheritance

Trait-level `where` clauses inherit into the impl by **positional substitution**: a renamed parameter keeps its predicate, and the predicate text follows the argument at the same position (§5.5, reference §7.2). A predicate naming something the impl does not declare is passed through verbatim, so rustc reports the unknown type. The rules for what a predicate may contain — `@N` references, `Trait<>` fills, what the final check rejects — are in `docs/reference.md` §7.

### 8.4 The `impl{...}` shape templates (0.8.0)

> **Advanced layer — skippable.** §8.1–§8.3 cover ordinary `where` bounds;
> this section goes further into shape templates. For batching an ordinary
> impl, §1.6 already gives the common entry. Read these matching rules when
> you need to substitute individual parts of a type into an implementation.

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

The template holds a **standard Rust type** — DSL operators are rejected inside it, `_` is a wildcard that matches anything, and an array length may bind a const parameter (`impl{[A; N]}` binds `N := 3`, usable in the body). Function-pointer parameter and return types match recursively; trait-object templates still compare verbatim. In generic arguments, a declared const name can bind a const value (`Wrap<N>` with `const N: usize` against `Wrap<3>`); declarations distinguish it from a type name, and actual type/lifetime/const kinds do not cross-bind. The full table, including the reserved `[A; ()]` shape, is in `docs/reference.md` §8.2.

#### The prototype-impl pattern

Write **one correct implementation for a representative leaf**, and the
"equal → keep, different → bind" rule adapts it to every leaf of the matrix:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] @num impl{Box<u8>} #max{Box::new(u8::MAX)})]
trait TMax { fn max() -> Self; }
// → impl TMax for Box<u8>  { fn max() -> Self  { Box::new(u8::MAX) } }
// → impl TMax for Box<u16> { fn max() -> Self { Box::new(u16::MAX) } }
// → impl TMax for Rc<f64>  { fn max() -> Self  { Rc::new(f64::MAX) } }
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

What the missing declaration looks like (all four measured):

```text
@{0} in a body, with no template at all
  → batch-impl: a `@{N}` fresh reference in the body requires the `impl{@{}}`
    body-slot switch (declare it on the spec, e.g. `impl{@{}}`); without it,
    `@` in a body starts a repeat block

`impl{@{}}` present, but the impl has no fresh to name
  → batch-impl: `@0` is out of range — this impl has 0 fresh generics
    (numbered from 0 in document order; user-written params are addressed by name)

a cursor-only block with `impl{@{}}` but without the fresh-binding switch
  → batch-impl: a repeat block needs a driving segment or a fresh-binding
    switch (`impl{@0..}`) to determine its length

a cursor-only block whose only driver is a shape template, but which has no repeat driver for it
  → no DSL diagnostic — the block reaches rustc, which reports
    expected one of `.`, `;`, `?`, `}`, or an operator, found `,`
    (locked by tests/ui/impl_shape_repeat_no_driver.rs)
```

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
`@0..` constraining every fresh generic:

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
// → impl<P0, P1> TupleMagma for (P0, P1,) where P0: Magma, P1: Magma { ... }
```

### 8.5 The impl entry (0.8.0, ItemImpl)

> **Further uses of the entry.** See
> [§1.6](#16-start-from-an-ordinary-display-impl)
> for its everyday form. This section adds explicit templates, inheritance
> rules and staged substitution; they are not prerequisites for using it.

**The whole impl block becomes the prototype.** Hand `#[batch_impl]` an
ordinary Rust impl and write `@Self: matrix`. The constant copies its input
self type as the template. Each matrix leaf is matched against that template;
the resulting replacements reach the self type, trait arguments, where
predicates and body. The generated impls replace the input block:

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# trait Make { fn make() -> Self; }
#[batch_impl(@Self: [Box, Rc] [usize, isize])]
impl Make for Box<u8> { fn make() -> Self { Box::new(u8::default()) } }
// → impl Make for Box<usize> { fn make() -> Self { Box::new(usize::default()) } }
// → ... × 4
# assert_eq!(*<Box<usize> as Make>::make(), 0);
# assert_eq!(*<Rc<isize> as Make>::make(), 0);
```

An explicit template (`A<B>: matrix`, or `(A, B): pairs`) can still describe
positions across the block independently of its self type. `@Self` is the
input type; ordinary Rust `Self` keeps its Rust meaning. Each stacked
attribute reads its own input type, and later shape mapping treats a copied
type just like the same tokens written by hand (reference §9.2).

The entry takes `template : matrix` (`A<B> : [Box,Rc] [usize,isize]`) or the direct form (`<T> Box<T>`), `;`-separates several specs, allows `@trait` in generic-declaration bounds and `where` predicates (custom `@` constants and `#` directives are rejected here), and preserves the block's own generics, `where` clause and `unsafe`. An **empty** spec list is a no-op: the attribute only *derives* impls, so with nothing to derive the block comes back unchanged. The rules are in `docs/reference.md` §9.2–§9.3.

**Stacked attributes are stages of one derivation.** A second (third, …)
`#[batch_impl]` above the block is not another spec list: rustc expands the
outermost first, this entry re-emits the rest on the impls it derives, and the
next stage then expands **on those impls** — so the stages run in source order
over the accumulating block, a slot one stage leaves in place is bound by the
next, and an empty stage is the identity. A plain attribute between two stages
belongs to the expansion level where it is written, which is also what scopes a
`#[cfg]` there (a rule stated in `docs/reference.md` §9.4):

```rust
# use batch_impl::batch_impl;
# struct Pair<A, B>(A, B);
# trait Tag { fn tag(&self) -> u32; }
#[batch_impl(A : [u8, u16])]      // stage 1 binds `A`
#[batch_impl(B : [u32, u64])]     // stage 2 binds the `B` stage 1 left alone
impl Tag for Pair<A, B> { fn tag(&self) -> u32 { 0 } }
// → impl Tag for Pair<u8,u32> / Pair<u8,u64> / Pair<u16,u32> / Pair<u16,u64>
```

**Why the order is needed, not just a convention** (the rule is in `docs/reference.md` §9.5). A *shape family* — container forms that are not the same head (`Vec<T>`, `[T; 4]`, `Box<[T]>`, `&[T]`) — needs one prototype per family in §8.4's pattern, because a single template cannot match four differently shaped heads. Two stages say it directly: stage 1 introduces the **shape with the element slot left open**, stage 2 fills that slot, and stage 2's substitution reaches *inside* what stage 1 produced (`B` lands in four different positions, one of them behind a reference):

```rust
# use batch_impl::batch_impl;
# trait Elem { fn elem_bytes(&self) -> usize; }
#[batch_impl(A : [Vec<B>, [B; 4], Box<[B]>, &'static [B]])]
#[batch_impl(B : [u8, u64])]
impl Elem for A { fn elem_bytes(&self) -> usize { std::mem::size_of::<B>() } }
// → impl Elem for Vec<u8> / Vec<u64> / [u8; 4] / [u64; 4]
//                 / Box<[u8]> / Box<[u64]> / &'static [u8] / &'static [u64]
```

Swapping the two attributes breaks it: the element gets bound while the block
does not mention it yet, and the shape stage then introduces a `B` that nothing
binds any more — measured as four `E0425: cannot find type `B`` (one per shape
leaf) instead of eight working impls. The stage order is what makes "shape
first, element second" expressible at all, and it is the order rustc's attribute
expansion gives (outermost first) — locked by
`tests/features/impl_entry_chain.rs`.

## 9. Tuple Generation and Matrices

### 9.1 Tuple generators, arities and the power suffix

Here, *fresh* means a generic parameter generated by the macro, such as
`P0` or `P1`. Concrete and generic generators may cover the same target,
especially with matching parameter counts and type structures; Rust reports
E0119 for overlapping impls. Check for overlap when combining generators,
and separate their target structures or arities.

Lists inside generators still distribute: combinations produced by
`(X, [A,B]).N` continue expanding their lists into the final targets.

Four spellings, all measured:

| Spelling | What it generates | Example |
|---|---|---|
| `()N` | **N fresh parameters** (a generator) — the carrier decides how they are spliced | `Pair3<*[].2>` → `impl<P0, P1> … for Pair3<P0, P1>` |
| `*[].N` | the same generator **spliced**, so a carrier can append its parameters | `T.*[].2` → `<P0,P1>T<P0,P1>` |
| `(A, B,)N` | the **N-fold Cartesian product** of the elements (tuples of length N) | `(u8, u16,)2` → 4 impls |
| `().1..=M` | one impl **per tuple arity** 1..=M, each with its own fresh parameters (the "ranges" of README's table) | `().1..=3` → `impl<P0> … for (P0,)`, `impl<P0,P1> … for (P0, P1,)`, `impl<P0,P1,P2> … for (P0, P1, P2,)`; going past the family is **not** diagnosed by the macro — the reader gets a bare `E0599` on the tuple, whose help suggests a field method of the same name (measured) |

The power is the **`.N` suffix** (`(u8, u16).2` = four tuple impls); the juxtaposed form `()N` / `(u8, u16)2` is accepted as well, while the old `^` spelling is rejected with its own retirement message (§12). The suffix binds to its block, so `Box.*[].2` applies the generator to `Box` rather than to something else.

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)3)]
trait T {}
// → impl T for (u8, u8, u8,) {}   (the 3-fold product of the one-element tuple)
```

```rust
# use batch_impl::batch_impl;
#[batch_impl(().1..=3)]
trait Arities {}
// → impl<P0> Arities for (P0,) {} / impl<P0,P1> … for (P0, P1,) / impl<P0,P1,P2> … for (P0, P1, P2,)
```

### 9.2 Cartesian products

`[A, B] [C, D]` full combinations; a splat power — `(*[A, B]).2` or the juxtaposed `*[A, B]2` — produces a Cartesian combo list, which **a host must consume**: `(*[u8, u16],).2` is that one tuple `(u8, u16, u8, u16,)`, while a bare `(*[u8, u16]).2` makes every combination a target of its own, so the repeated ones collide (`E0119`):

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] [u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>(4 entries)
```

Matrices can be wrapped into containers or const-generic fixed arrays (`([u8, u16],)2` etc.).

## 10. The Modifier Gallery

The complete modifier table (`&`/`&mut`, `*const`/`*mut`, `unsafe`, `#[...]`, `!`, `self`) is in `docs/reference.md` §3.8. This section keeps the three whose *reading* is easy to get wrong.

`&`, `*const`, `*mut`, `unsafe`, `fn` types and attributes are all supported:

```rust
# use batch_impl::batch_impl;
#[batch_impl(&str, &mut [u8], *const u8, *mut u8)]
trait Ptrs {}

#[batch_impl(unsafe fn(u8) -> u8)]
trait FnT {}

#[batch_impl(#[cfg(all())] u8)]
trait Attr {}
// → impl Ptrs for &str {} / &mut [u8] / *const u8 / *mut u8, impl FnT for unsafe fn(u8) -> u8,
//   impl Attr for u8 {} (the attribute rides onto the generated impl — `#[repr(C)]` is *not* legal there)
```

**`self`** is the identity prefix: `self T` = `T`. In a matrix it acts as a **bare-type placeholder** — `[Box, self] u8` generates both `Box<u8>` and the bare `u8`:

```rust
# use batch_impl::batch_impl;
#[batch_impl([Box, self] u8 { fn tag(&self) -> &'static str { "x" } })]
trait WrapOrBare { fn tag(&self) -> &'static str; }
// → impl WrapOrBare for Box<u8> { ... } / impl WrapOrBare for u8 { ... }
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

**Array and slice types**: `[u8; 3]` is a fixed array, `[u8]` a slice:

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8; 3], [u8], &[u8])]
trait Slices {}
// → impl Slices for [u8; 3] {} / [u8] / &[u8]
```

## 11. Entry Points

The table distinguishes generation entries from helper macros.
`batch_impl`, `batch_impl_only` and `batch_trait` offer different trait/impl
inputs. `batch_preprocess_test!` consumes the open-extension protocol,
while `batch_preview!` takes an annotated Rust item — `#[batch_impl(...)] trait … {}` or the matching
`impl` — to preview; these do not share one complete input grammar. Each entry's
argument rules are in rustdoc, with trait-path and inheritance rules in
reference §9.

| Entry | Form | Note |
|---|---|---|
| `#[batch_impl]` | attribute macro on a `trait` definition | re-emits the trait and generates impls |
| `#[batch_impl]` | attribute macro on an `impl` block (the **impl entry**, 0.8.0) | batch-instantiates a hand-written impl from a shape template × matrix |
| `#[batch_impl_only]` | attribute macro on a `trait` definition | generates impls only, the trait comes from outside (prefix `# path::To::Trait:` to rename — write it **once, in front of the attribute's list**: it belongs to the attribute, not to a spec, so repeating it per spec only produces a misleading "`#std` must be followed by `(args)`/`[args]` or a code block `{body}`") |
| `batch_trait!` | function-like macro | sections plus custom `@name=value;` constant sections; **no** `#` directives |
| `batch_preprocess_test!` | reference open-extension macro | consumes protocol input and emits a complete impl; the legacy in-impl input emits associated items |
| `batch_preview!` | diagnostic channel | prints the expansion as `compile_error!` text (the only stable terminal channel) **and, on the trait entry, the number of impls it produced** (the impl entry prints one stream and reports no count), which is the quickest way to check that a spec generated what you meant |

```rust
# use batch_impl::batch_impl_only;
# mod path { pub mod to { pub trait Conv<T> { fn conv() -> T; } } }
# struct Wrapper<T>(T);
#[batch_impl_only(# path::to::Conv: Conv<bool> Wrapper<bool> #conv{false})]
trait Conv<T> { fn conv() -> T; }
// → impl Conv<bool> for Wrapper<bool> { fn conv() -> bool { false } }(trait not re-emitted)
```

```rust
# use batch_impl::batch_trait;
# trait A {} trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> Vec<T>;
}
```

- **`batch_trait!`** — a function-like macro for a trait that is already declared: sections, custom `@name=value;` constant sections, **no** directives (§6.3).
- **The impl entry (0.8.0, ItemImpl)** — `#[batch_impl]` also accepts an `impl` block: batch-instantiate a hand-written impl from a shape template × matrix source (§8.5).

## 12. Error Hints

batch-impl's errors are **compile-time diagnostics** that try to point at a
relevant user-visible token; without an available source location they may
point at the macro call. One invocation can collect several independent
errors, and Rust may issue further diagnostics. Fix the first specific
error and compile again. Common cases include:

- **Missing operand**: `A.` / `.A` / `,A`
- **`@N`/`@g_i` out of range or dangling**: `@5` beyond the impl's generated generic count, or a missing `@2_0` group — the fresh generics are numbered from 0 in document order and print as `P0`, `P1`, …; a dangling reference is intercepted in the macro, never a raw rustc E0412
- **`where` predicate that is not a Rust predicate**: `where{ A B }` (a missing `:`) is reported once the predicate is final, with the fix named; a **splat** in a predicate is reported too, because that clause is token-level all the way to the output
- **`=`/`:` in the wrong argument list**: bounds and bindings belong on a trait path (`Conv<Item = u32> X`) or in a bound (`T: Iterator<Item = u8>`, same inside `dyn` / `for<'a>`); a `<>` **declaration** block declares parameters, so a binding there is reported with the spelling that works
- **A fresh generator in a `<>` declaration block**: write it on the type instead — `T.*[].2` to splice the generated parameters, `T<()2>` to keep them as one tuple argument
- **The retired `^` power**: `(u8, u16)^2` / `T^()^2` get their own message; the power is the `.N` suffix (`(u8, u16).2`, `T.*[].2`)

Everything else — every class with its **exact wording** and the fixture that locks it — is `docs/reference.md` §10.

## 13. Real Scenarios: the Three Bundled Examples

The chapters above teach one mechanism at a time. `examples/` is where they are **combined** into whole files, and CI compiles them, so they cannot drift. That also means an example cannot fail on purpose unless it is isolated: one that exists to show a diagnostic has to declare `required-features` (or stay out of the default target set), or `cargo test` builds it and the deliberate failure turns the gate red — measured, by a probe that lost a run to exactly that:

| Example | What it is | What it shows |
|---|---|---|
| `examples/quickstart.rs` (~320 lines) | a runnable single-file tour — `cargo run --example quickstart` prints one `…: OK` line per demo plus a summary | one demo per mechanism (§1–§8) |
| `examples/simplify.rs` (~170 lines) | a small "data inspection" library: **30 impls** from ~15 lines of DSL (hand-written: ~80 lines) | lists + shared body, wrapper delegation, tuple generation, space application, associated-type bindings, `#name`/`#fill`/`#delegate`, pointers, three entries |
| `examples/typeclass.rs` (~120 lines) | a type-class hierarchy (`Num` → `UNum`/`INum`/`FNum`) plus `From<bool>` for a generic fraction | `@` families inside `batch_trait!`, splat pow (36 instances), trait arguments substituting into copied bodies |

### 13.1 `simplify.rs` — one trait for twelve numerics

```text
#[batch_impl(
    [u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64] {
        fn describe(&self) -> String { format!("num:{self}") }
        fn is_zero(&self) -> bool { *self == Self::default() }
    }
)]
trait Describe {
    fn describe(&self) -> String;
    fn is_zero(&self) -> bool;
}
```

One list and one body → **12 impls**: the list expands into impls (§3),
and the complete methods explicitly written in `{…}` are used for every
target. This example does not use signature-copying directives; `#describe`
and `#is_zero` from §7 would be an alternative. `Self::default()` is zero
for every numeric type, so a single expression covers all twelve.

The rest of the file covers four wrappers delegating to the inner value in one line (`[&, Box, Rc, Arc].T`, §7.3), tuple generation `().1..=4`, left-associative space application (`fn(i32, u32) String`, `HashMap u8 u16`), an associated-type binding with `#name{…}` for a single const, `#fill(name, kind){"u8"}` for one body shared by two methods, a `batch_trait!` segment, and `*const` / `*mut` targets.

### 13.2 `typeclass.rs` — a class hierarchy and 36 instances

```text
#[batch_impl_only(
    From<bool>
    Frac<*(*@u*).2>
    #from{
        Frac { positive: true, num: value.into(), denom: true.into() }
    }
)]
pub trait From<T>: Sized {
    fn from(value: T) -> Self;
}
```

Three mechanisms meet here: the trait application `From<bool>` **pins** the trait's parameter, so the copied signature `fn from(value: T)` becomes `fn from(value: bool)` (§7.1); the splat pow `Frac<*(*@u*).2>` feeds the `@u*` list into **both** generic positions — 6 × 6 = 36 impls (§4); and `#from{…}` supplies the single body the whole family shares (§7.1).

The hierarchy above it shows the other half of the pattern: `Num` is defined and filled by `#[batch_impl]`, while its subclasses are declared and then filled **one line per class** by `batch_trait!` with `@` families (§6.1) — exactly what a type-class needs.

### 13.3 Where to go next

- mechanisms: §1–§12 above, then the [reference manual](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md) for legality, diagnostics and ceilings;
- the API documentation: [entry point and directive guides](https://github.com/5-6-1/batch-impl-rs/tree/main/src/doc);
- the internal map: [architecture](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/architecture.md).
