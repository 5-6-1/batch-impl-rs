# batch-impl

**v0.6.1** (2026-08-05) — `@` as the single macro-meta token + blanket constraint merging + trait-aware constants (`@trait`/`@all` family/`@Cow`/`@0`).

A procedural macro crate that batch-generates `impl` blocks for Rust traits — **one line of DSL, expanded into N impls**.

```rust
use batch_impl::batch_impl;
# use std::rc::Rc;

// One body, one impl for each of the 4 types
#[batch_impl(<T> Sortable<T> [Box, Rc]^Vec<T> where T: Ord  {
    fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
})]
trait Sortable<T> { fn is_sorted(&self) -> bool; }
// → impl<T> Sortable<T> for Box<Vec<T>> where T: Ord { ... }
// → impl<T> Sortable<T> for Rc<Vec<T>>  where T: Ord { ... }

// One line generates 4 generic tuple impls
#[batch_impl(()^4)]
trait TupleTrait {}
// → impl<A>       TupleTrait for (A,) {}
// → impl<A, B>    TupleTrait for (A, B) {}
// → impl<A, B, C> TupleTrait for (A, B, C) {}
// → impl<A, B, C, D> TupleTrait for (A, B, C, D) {}
```

## Why use it

Hand-writing the same trait implementation for multiple types means **repetition**: the signature is copied N times, the body is copied N times, generic parameters and associated types are each written separately, and changing one place misses three. batch-impl puts the **quantity** of impls into a description outside the human brain:

- **One source of truth**: the trait definition is written only once (signature/generics/bound/where constraints), the DSL only writes "which types × what implementation", and the macro fills in the rest — signatures, generic bounds, associated type bindings, and even trait-level where constraints are **automatically inherited** from the trait definition, fully equivalent to hand-written code.
- **One-line matrix**: `[...]` lists, `^`/`-` application, `()^N` tuple generation — one DSL line describes a "type matrix", and the macro generates one impl per cell.
- **Batch, but hand-written in feel**: `{ body }` is ordinary Rust code, `#` directives automatically copy signatures, and the generated impl is token-for-token equivalent to hand-written code — whatever rustc can verify, it can verify.

A real scenario (see `examples/simplify.rs`): 12 numeric types + 4 wrapper types + 4 tuples + some miscellaneous = **29 impls from about 15 lines of DSL**, versus about 80 lines by hand.

## Mental model

What you write is **a description of a "type matrix"**, and batch-impl generates an impl for every cell of the matrix:

```text
#[batch_impl( <impl-generics> TraitName<trait-generics> target-type matrix { body }? )]
```

| Symbol     | Meaning                                           | Intuition                        |
|------------|---------------------------------------------------|----------------------------------|
| `^` / `-`  | apply: apply the left container/modifier to the right type | **the same operation**, only associativity differs |
| `[A, B]`   | list                                              | horizontal expansion (Cartesian product) |
| `(A, B)`   | tuple                                             | permutations (ordered pairs)     |
| `#name`    | directive: auto-copy the item signature from the trait definition | the body doesn't hand-write signatures |

`^` and `-` are **the same operation** (the left side is a modifier/container, the right side is the target type), differing only in associativity:

- `^` is **right-associative**, chaining produces nesting: `Box^Box^T` = `Box<Box<T>>`, `HashMap^K^V` = `HashMap<K<V>>`
- `-` is **left-associative**, chaining accumulates arguments: `HashMap-K-V` = `HashMap<K, V>`, `fn(A, B)-C` = `fn(A, B) -> C`

So which one to pick depends only on the grouping shape you want: use `^` to nest, use `-` to list arguments side by side.

`[A, B]^[X, Y]` = a 2×2 matrix (4 impls); `(T1, T2)^2` = permutations (4 ordered pairs).

## Quick start

```toml
[dependencies]
batch-impl = "0.6.1"
```

Requires Rust 2024 edition or newer.

```rust
use batch_impl::batch_impl;

// 1. Define the trait; the method signature is written only once
trait Describe { fn describe(&self) -> String; }

// 2. Write one DSL line: target type + body (the signature is auto-copied from the trait via #name)
#[batch_impl(
    [usize, isize] #name{"number"},
    String #name{"string"}
)]
trait Tagged { fn name(&self) -> &str; }
// → impl Tagged for usize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for isize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for String { fn name(&self) -> &str { "string" } }
```

## Feature overview

| Feature                                          | In one sentence                              | Tutorial chapter |
|--------------------------------------------------|----------------------------------------------|------------------|
| Side-by-side lists `[A, B]`                      | Implement for multiple types at once, body reused | Lists and body |
| `^` / `-` operators                              | Right/left associativity of the same operation: nesting vs. accumulation | Operators |
| Generic automation                               | `A<>` copied as-is, same-name inheritance, trait where-clause inheritance | Generic automation |
| Associated type bindings                         | `Iter<Item=T>` → `type Item = T;`            | Associated types |
| Directive system `#name`/`#fill`/`#delegate`     | Auto-copy signatures, batch-fill bodies, delegate calls | Directive system |
| Blanket delegation `#blanket`                    | Generate delegated impls from a wrapper matrix in one line (any wrapper + `:N`, generic traits, assoc projections, wrapper where predicates) | Directive system |
| Open extension                                   | Unknown `#name(args){body}` is handed to your macro with the same name | Directive system |
| `@` constants                                    | Built-in families `@uint`/`@scalar`/`@u8..u128` + `@trait`/`@all` family/`@Cow` + `batch_trait!` customization (lazy expansion, chained references) | Constant system |
| Unified macro-meta layer `@`                      | `#` keeps only directive names; scope selection (`@all` family) and positional references (`@0`) belong to the macro-meta layer | Constant system |
| `where{...}`                                     | Unified constraint container (`<>` keeps only names), blanket constraints merged side by side | where clauses |
| Tuple generation                                 | `()^3`, `(T,)^N`, Cartesian product, ranges  | Tuple generation |
| fn types / unsafe / pointers / attributes        | Full support for type-level modifiers        | Modifiers |

## Next steps

- **Full tutorial**: `docs/tutorial.md` (progressive, from a one-line impl to advanced matrix combinations)
- **Three entry points**: `#[batch_impl]` (includes the trait) / `#[batch_impl_only]` (impls only) / `batch_trait!` (batch-generate for an already declared trait, multi-section support)
- **Examples**: `examples/quickstart.rs` (feature demo), `examples/simplify.rs` (a real scenario with 29 impls ≈ 15 lines of DSL)
- **Developers**: internal architecture in `docs/architecture.md`, development changelog in `docs/dev-changelog.md`

## License

MIT OR Apache-2.0
