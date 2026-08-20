# batch-impl

**v0.9.0** (2026-08-21) — the apply operators are reworded: `.` is the right-associative apply operator and **space application replaces `-`** as the left-associative combination (`Box u8` = `Box<u8>`, `<T: Clone> Vec<T>` = a declaration applied to a type); `-` keeps only its directive-domain exclusion meaning. The DSL is a **bag of blocks** (declarations / directive blocks / code blocks / types in any order, folded by `apply`), same-name generic declarations merge into a where clause (`<T: Clone><T: Copy> X` → `impl<T> ... where T: Clone, T: Copy`), `_` is a never-replaced wildcard in shape templates (`impl{B<_>}`), and `X<>` syncs to the spec trait application via a switch template (`impl{@trait<>}` / `impl{Tr<>}`, path-qualified included).

A procedural macro crate that batch-generates `impl` blocks for Rust traits — **one line of DSL, expanded into N impls**.

Beyond the core batch-impl DSL, the crate carries two deeper layers: a
**macro-meta layer** (`@` constants / selectors / positional references — a
small meta-language for composing generated generics) and an **open directive
system** (`#fill` / `#delegate` / `#blanket` + user `#name` macros, including
top-level macro injection `{! ...}`). Think of it as a batch impl generator
with a pluggable codegen protocol — the "one line" story covers the common
case; the layers below it cover the composing cases (dispatch matrices,
blanket delegation, custom codegen).

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

## Built with batch-impl

**[alga2](https://docs.rs/alga2) is a real user** — a modern abstract-algebra
hierarchy for Rust (the successor to [alga](https://docs.rs/alga)), with
**~900 impls generated from ~80 batch-impl DSL blocks** across 15+ types
(numbers, tuples 1–16, arrays, `Option`, `Complex`, `Quaternion`, `ModN`,
smart pointers, collections). **alga2 0.1.0 is about to be released** on
crates.io; the batch-impl DSL has been its impl generator throughout
development.

## Why use it

Hand-writing the same trait implementation for multiple types means **repetition**: the signature is copied N times, the body is copied N times, generic parameters and associated types are each written separately, and changing one place misses three. batch-impl puts the **quantity** of impls into a description outside the human brain:

- **One source of truth**: the trait definition is written only once (signature/generics/bound/where constraints), the DSL only writes "which types × what implementation", and the macro fills in the rest — signatures, generic bounds, associated type bindings, and even trait-level where constraints are **automatically inherited** from the trait definition, fully equivalent to hand-written code.
- **One-line matrix**: `[...]` lists, space/`.` application, `().N` tuple generation — one DSL line describes a "type matrix", and the macro generates one impl per cell.
- **Batch, but hand-written in feel**: `{ body }` is ordinary Rust code, `#` directives automatically copy signatures, and the generated impl is token-for-token equivalent to hand-written code — whatever rustc can verify, it can verify.

A real scenario (see `examples/simplify.rs`): 12 numeric types + 4 wrapper types + 4 tuples + some miscellaneous = **29 impls from about 15 lines of DSL**, versus about 80 lines by hand.

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
| `#name`    | directive: auto-copy the item signature from the trait definition | the body doesn't hand-write signatures |

**The space (adjacency) is the natural way to apply**: the left side is the modifier/container/trait, the right side the target type, and chaining accumulates arguments left-associatively — `HashMap u32 String` = `HashMap<u32, String>`, `fn(A, B) C` = `fn(A, B) -> C`, `Tr u8` = `impl Tr for u8` (a bare trait name applies as the impl trait; the trait name is identified by the annotated trait). Write `Tr <u8>` for the type `Tr<u8>`.

`. ` is the same operation with **right-associative** grouping for nesting: `Box.Box.T` = `Box<Box<T>>`, `HashMap.K.V` = `HashMap<K<V>>`.

Pick by the grouping shape you want: use the space to list arguments side by side, use `.` to nest.

`[A, B].[X, Y]` = a 2×2 matrix (4 impls); `(T1, T2).2` = permutations (4 ordered pairs).

## Quick start

```toml
[dependencies]
batch-impl = "0.8.1"
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

| Feature                                          | In one sentence                              | Tutorial chapter |
|--------------------------------------------------|----------------------------------------------|------------------|
| Side-by-side lists `[A, B]`                      | Implement for multiple types at once, body reused | §3 |
| Splat `*` prefix                                | Flatten containers/generators into the enclosing list — in-list splice, `.` right-operand flat append, generic multi-arg; left operand `*[...]` distribute / `*(...)` append | §4 |
| space / `.` operators                          | Left/right associativity of the same operation: accumulation vs. nesting | §2 |
| Generic automation                               | `A<>` copied as-is, same-name inheritance, trait where-clause inheritance | §5 |
| Associated type bindings                         | `Iter<Item=T>` → `type Item = T;`            | §5.3 |
| Directive system `#name`/`#fill`/`#delegate`     | Auto-copy signatures, batch-fill bodies, delegate calls | §7 |
| Blanket delegation `#blanket`                    | Generate delegated impls from a wrapper matrix in one line (any wrapper + `:N`, generic traits, assoc projections, wrapper where predicates, static methods forwarded via `t`) | §7 |
| Open extension                                   | Unknown `#name(args){body}` becomes a top-level macro call: your same-named macro receives `{spec}(args){body}trait` and emits its own impl | §7 |
| `@` constants                                    | Built-in families `@u*`/`@scalar`/`@u8..u128` + `@trait`/`@all` family/`@Cow` + `batch_trait!` leading `@name=value;` custom sections (lazy expansion, chained references; attribute macros do not support them — write matrices directly) | §6 |
| Generic parameter families                     | `@all_type_params` / `@all_const_params` / `@all_lifetimes` — generic declarations copy the trait's formal params (bounds via same-name inheritance) | §6 |
| Unified macro-meta layer `@`                      | `#` keeps only directive names; scope selection (`@all` family, incl. required/default and receiver filters) and positional references (`@N`, `@g_i`, `@all_fresh`, `@N..=M`) belong to the macro-meta layer | §6 |
| `where{...}`                                     | Unified constraint container (`<>` keeps only names), blanket constraints merged side by side | §8 |
| Tuple generation                                 | `().3`, `(T,).N`, Cartesian product, ranges  | §9 |
| Variadic segments + repeat blocks                | `ident@..` in `impl{...}` templates (cover every remaining tuple position) + `@(...)..` body repetition (`@ident` names, `@N` index cursors) — one spec covers every tuple arity | §8.4 |
| fn types / unsafe / pointers / attributes        | Full support for type-level modifiers (`unsafe fn` is the fn type; `unsafe.fn` marks the impl unsafe) | §10 |

> **Shorthand**: a single method `#fill([foo]){body}` equals `#foo{body}`; predicates + code block `where{predicates} {code block}` can be written bare as `where predicates {code block}` (see §7.2 / §8.2).

## Syntax-freeze commitment (0.7.2)

The semantics of every existing token are **final** — `.`/space, `[]`/`()`/`<>`, `where`, the `#` directives, the `@` constants, and the splat will not change behavior again. Future releases only **add** (new directives / constants / tools), refine diagnostics, and polish docs; any change to existing semantics is a deliberate breaking release (the `@N` stability commitment, now extended to the whole surface). `@g_i` / `@all_fresh` / `@N..M` are power-user tier (tutorial §6.4) — start from `@u*` / `@all_methods` / `@0`.

## Next steps

- **Full tutorial**: `docs/tutorial.md` (progressive, from a one-line impl to advanced matrix combinations)
- **Three entry points**: `#[batch_impl]` (includes the trait) / `#[batch_impl_only]` (impls only) / `batch_trait!` (batch-generate for an already declared trait, multi-section support)
- **impl entry / shape template (0.8.0)**: the **ItemImpl entry** — `#[batch_impl]` also accepts an `impl` block and batch-instantiates it from a shape-template × matrix-source (tutorial §8.5); the **`impl{...}` Self-part shape templates** — bind the generated impl's target shape and write **one prototype impl per shape family** to cover a whole matrix, incl. lifetime-bearing families like `Cow` (tutorial §8.4)
- **Variadic segments + repeat blocks (0.8.2)**: `ident@..` template segments and `@(...)..` body repetition — the alga2-style `().1..=4 where{@all_fresh: Magma} impl{(A@..,)} #combine{...}` covers every tuple arity with one spec (tutorial §8.4)
- **Expansion preview**: `batch_preview!` (wrap the `#[batch_impl(...)] trait` / `#[batch_impl(...)] impl` input and read the real expansion, plus space/`.` associativity miswrite notes)
- **Examples**: `examples/quickstart.rs` (feature demo), `examples/simplify.rs` (a real scenario with 29 impls ≈ 15 lines of DSL), `examples/typeclass.rs` (type-class style: a `Num`/`UNum`/`INum`/`FNum` hierarchy + 36 `From<bool>` impls for `Frac<T, U>`)
- **Developers**: internal architecture in `docs/architecture.md`, development changelog in `docs/dev-changelog.md`

## License

MIT OR Apache-2.0
