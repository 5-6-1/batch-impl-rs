# batch-impl

**v0.9.7** (2026-08-29) — review-fix release: golden expansion snapshots, measured expansion cost, package hygiene, Windows CI. Release notes: [CHANGELOG](CHANGELOG.md).

A procedural macro crate that batch-generates `impl` blocks for Rust traits — **one line of DSL, expanded into N impls**.

## Why use it

Hand-writing the same trait implementation for multiple types means **repetition**: the signature is copied N times, the body is copied N times, generic parameters and associated types are each written separately, and changing one place misses three. batch-impl puts the **quantity** of impls into a description outside the human brain:

- **One source of truth**: the trait definition is written only once (signature/generics/bound/where constraints), the DSL only writes "which types × what implementation", and the macro fills in the rest — signatures, generic bounds, associated type bindings, and even trait-level where constraints are **automatically inherited** from the trait definition, fully equivalent to hand-written code.
- **One-line matrix**: `[...]` lists, space/`.` application, `().N` tuple generation — one DSL line describes a "type matrix", and the macro generates one impl per cell.
- **Batch, but hand-written in feel**: `{ body }` is ordinary Rust code, `#` directives automatically copy signatures, and the generated impl is token-for-token equivalent to hand-written code — whatever rustc can verify, it can verify.

A real scenario (see `examples/simplify.rs`): 12 numeric types + 4 wrapper types + 4 tuples + some miscellaneous = **29 impls from about 15 lines of DSL**, versus about 80 lines by hand.

```rust
use batch_impl::batch_impl;
# use std::rc::Rc;

// One body, one impl for each of the 4 types
#[batch_impl(<T> Sortable<T> [Box, Rc].Vec<T> where T: Ord  {
    fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
})]
trait Sortable<T> { fn is_sorted(&self) -> bool; }
// → impl<T> Sortable<T> for Box<Vec<T>> where T: Ord { ... }
// → impl<T> Sortable<T> for Rc<Vec<T>>  where T: Ord { ... }

// One line generates a single 4-generic tuple impl (length ranges use `().1..=4`)
#[batch_impl(().4)]
trait TupleTrait {}
// → impl<A, B, C, D> TupleTrait for (A, B, C, D) {}
```

Beyond the core batch-impl DSL, the crate carries two deeper layers: a
**macro-meta layer** (`@` constants / selectors / positional references — a
small meta-language for composing generated generics) and an **open directive
system** (`#fill` / `#delegate` / `#blanket` + user `#name` macros, including
top-level macro injection `{! ...}`). Think of it as a batch impl generator
with a pluggable codegen protocol — the "one line" story covers the common
case; the layers below it cover the composing cases (dispatch matrices,
blanket delegation, custom codegen).

## Built with batch-impl

**[alga2](https://docs.rs/alga2) is a real user** — a modern abstract-algebra
hierarchy for Rust (the successor to [alga](https://docs.rs/alga)), with
**~900 impls generated from ~80 batch-impl DSL blocks** across 15+ types
(numbers, tuples 1–16, arrays, `Option`, `Complex`, `Quaternion`, `ModN`,
smart pointers, collections). **alga2 0.1.0 is released** on
crates.io; the batch-impl DSL has been its impl generator throughout
development.

## Expansion cost

The DSL is a proc macro — the work happens at compile time, not runtime.
Measured on the author's machine (stable Rust, `cargo test --lib perf`):
a 1024-impl spec at the expansion ceiling (`(u8, u16, u32, u64).5` Cartesian
tuple power) expands in **~0.2 ms/impl**; a typical 4-impl spec is ~2.5 ms.
The measurement runs the same pipeline the attribute entry uses, at
proc-macro2 level (rustc's own type-checking is not included).

## Mental model

What you write is **a description of a "type matrix"**, and batch-impl generates an impl for every cell of the matrix:

```text
#[batch_impl( <impl-generics> TraitName<trait-generics> target-type matrix { body }? )]
```

| Symbol     | Meaning                                           | Intuition                        |
|------------|---------------------------------------------------|----------------------------------|
| space / `.` | apply: apply the left container/modifier to the right type | **the same operation**, only associativity differs |
| `[A, B]`   | list                                              | horizontal expansion (Cartesian product) |
| `(A, B)`   | tuple                                             | permutations (ordered pairs)     |
| `*[...]` / `*(...)` | splat: flatten into the enclosing list | `[a, *[b,c]]` = `[a,b,c]`; left `*[...]` distributes / `*(...)` appends |
| `#name`    | directive: auto-copy the item signature from the trait definition | the body doesn't hand-write signatures; `-` exclusion in directive args (`#fill(@all, -foo)`) is the only surviving use of the retired `-` operator |

**The space (adjacency) is the natural way to apply**: the left side is the modifier/container/trait, the right side the target type, and chaining accumulates arguments left-associatively — `HashMap u32 String` = `HashMap<u32, String>`, `fn(A, B) C` = `fn(A, B) -> C`, `Tr u8` = `impl Tr for u8` (a bare trait name applies as the impl trait; the trait name is identified by the annotated trait). Write `Tr<u8>` for the type `Tr<u8>`.

`. ` is the same operation with **right-associative** grouping, for **nesting** only: `Box.Box.u8` = `Box<Box<u8>>`, `HashMap<K> String` = `HashMap<K, String>` (space works here too). In a mixed expression the dot binds **before** the space: `Box Vec . u8` = `Box<Vec<u8>>` (the dot nests `Vec . u8` first), whereas `Box Vec u8` = `Box<Vec, u8>` — the space lists both as separate arguments.

Pick by the grouping shape you want: use the space to list arguments side by side, use `.` to nest.

`[A, B] [X, Y]` = a 2×2 matrix (4 impls); `(T1, T2).2` = permutations (4 ordered pairs).

## Quick start

```toml
[dependencies]
batch-impl = "0.9.7"
```

Requires Rust 1.95 or newer (edition 2024). The MSRV is deliberate: the
codegen uses `Cell::update` and match-arm if-let guards (stabilized around
1.87/1.88), and 1.95 keeps a comfortable stable margin (see the developer
changelog for the exact adoption record).

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

// 3. 0.6.2: one-line blanket — delegation impls for every wrapper type
//    (instance methods forward via deref; @all_ref_methods selects only
//    reference-receiver methods, by-value ones keep the trait default)
# use std::rc::Rc;
#[batch_impl(#blanket(@all_ref_methods){&, Box, Rc})]
trait Describe2 { fn describe(&self) -> String; }
// → impl<T> Describe2 for &T    where T: Describe2 { fn describe(&self) -> String { (**self).describe() } }
// → impl<T> Describe2 for Box<T> where T: Describe2 { ... }
// → impl<T> Describe2 for Rc<T>  where T: Describe2 { ... }
```

## Feature overview

**Core (80% of use cases — start here):** side-by-side lists, space/`.` application, `where{...}`, tuple generation, and the splat cover most real matrices; §1–§5 of the tutorial are enough. Everything below the core line is a deeper layer (macro-meta `@`, directives, shape templates) — useful when you need it, ignorable when you don't.

| Feature                                          | In one sentence                              | Tutorial chapter | Tier |
|--------------------------------------------------|----------------------------------------------|------------------|------|
| Side-by-side lists `[A, B]`                      | Implement for multiple types at once, body reused | §3 | core |
| space / `.` operators                          | Left/right associativity of the same operation: accumulation vs. nesting | §2 | core |
| `where{...}`                                     | Unified constraint container (`<>` keeps only names), blanket constraints merged side by side | §8 | core |
| Tuple generation                                 | `().3`, `(T,).N`, Cartesian product, ranges  | §9 | core |
| Splat `*` prefix                                | Flatten containers/generators into the enclosing list — in-list splice, `.` right-operand flat append, generic multi-arg; left operand `*[...]` distribute / `*(...)` append | §4 | core |
| Generic automation                               | `A<>` copied as-is, same-name inheritance, trait where-clause inheritance | §5 | core |
| Associated type bindings                         | `Iter<Item=T>` → `type Item = T;`            | §5.3 | core |
| fn types / unsafe / pointers / attributes        | Full support for type-level modifiers (`unsafe fn` is the fn type; `unsafe.fn` marks the impl unsafe) | §10 | core |
| `@` constants                                    | Built-in families `@u*`/`@scalar`/`@u8..u128` + `@trait`/`@all` family/`@Cow` + `batch_trait!` leading `@name=value;` custom sections (lazy expansion, chained references; attribute macros do not support them — write matrices directly) | §6 | advanced |
| Generic parameter families                     | `@all_type_params` / `@all_const_params` / `@all_lifetimes` — generic declarations copy the trait's formal params (bounds via same-name inheritance) | §6 | advanced |
| Unified macro-meta layer `@`                      | `#` keeps only directive names; scope selection (`@all` family, incl. required/default and receiver filters) and positional references (`@N`, `@g_i`, `@all_fresh`, `@N..=M`) belong to the macro-meta layer | §6 | advanced |
| Directive system `#name`/`#fill`/`#delegate`     | Auto-copy signatures, batch-fill bodies, delegate calls | §7 | advanced |
| Blanket delegation `#blanket`                    | Generate delegated impls from a wrapper matrix in one line (any wrapper + `:N`, generic traits, assoc projections, wrapper where predicates, static methods forwarded via `t`) | §7 | advanced |
| Open extension                                   | Unknown `#name(args){body}` becomes a top-level macro call: your same-named macro receives `{spec}(args){body}trait` and emits its own impl | §7 | advanced |
| Variadic segments + repeat blocks                | `ident@..` in `impl{...}` templates (cover every remaining tuple position) + `@(...)..` body repetition (`@ident` names, `@N` index cursors) — one spec covers every tuple arity | §8.4 | advanced |

> **Shorthand**: a single method `#fill([foo]){body}` equals `#foo{body}`; predicates + code block `where{predicates} {code block}` can be written bare as `where predicates {code block}` (see §7.2 / §8.2).

## Syntax-freeze commitment (0.7.2)

The semantics of every existing token are **final** — `.`/space, `[]`/`()`/`<>`, `where`, the `#` directives, the `@` constants, and the splat will not change behavior again. Future releases only **add** (new directives / constants / tools), refine diagnostics, and polish docs; any change to existing semantics is a deliberate breaking release (the `@N` stability commitment, now extended to the whole surface). `@g_i` / `@all_fresh` / `@N..M` are power-user tier (tutorial §6.4) — start from `@u*` / `@all_methods` / `@0`.

## Next steps

- **Full tutorial**: `docs/tutorial.md` (progressive, from a one-line impl to advanced matrix combinations)
- **Three entry points**: `#[batch_impl]` (includes the trait) / `#[batch_impl_only]` (impls only) / `batch_trait!` (batch-generate for an already declared trait, multi-section support)
- **impl entry / shape template (0.8.0)**: the **ItemImpl entry** — `#[batch_impl]` also accepts an `impl` block and batch-instantiates it from a shape-template × matrix-source (tutorial §8.5); the **`impl{...}` Self-part shape templates** — bind the generated impl's target shape and write **one prototype impl per shape family** to cover a whole matrix, incl. lifetime-bearing families like `Cow` (tutorial §8.4)
- **Variadic segments + repeat blocks (0.8.2)**: `ident@..` template segments and `@(...)..` body repetition — the alga2-style `().1..=4 where @0..: Magma impl{(A@..)} #combine{...}` covers every tuple arity with one spec (tutorial §8.4)
- **Expansion preview**: `batch_preview!` (wrap the `#[batch_impl(...)] trait` / `#[batch_impl(...)] impl` input and read the real expansion, plus space/`.` associativity miswrite notes)
- **Examples**: `examples/quickstart.rs` (feature demo), `examples/simplify.rs` (a real scenario with 29 impls ≈ 15 lines of DSL), `examples/typeclass.rs` (type-class style: a `Num`/`UNum`/`INum`/`FNum` hierarchy + 36 `From<bool>` impls for `Frac<T, U>`)
- **Developers**: internal architecture in `docs/architecture.md`, development changelog in `docs/dev-changelog.md`

## License

MIT OR Apache-2.0
