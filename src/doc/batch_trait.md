# `batch_trait!` — Batched Impls for Traits Defined Elsewhere

Function-like macro that generates `impl` blocks for a declared trait in
batch — the entry point for traits **already defined elsewhere** (another
crate / module) when you only need the type-matrix DSL.

## Syntax

```text
batch_trait!( segment [; segment]* );
segment := unsafe? Trait-path : impl-specs
```

- `Trait-path` — the trait's path (`A`, `foo::B`, `crate::ext::C`);
- `impl-specs` — the DSL expression (type matrix + `@` constants), same as
  `#[batch_impl]`;
- `;` separates multiple trait segments (consecutive `;` and a trailing one
  are allowed);
- `unsafe` marks every impl in that segment as an unsafe impl.

```rust
# use batch_impl::batch_trait;
trait A {}
trait B<T> {}
unsafe trait UnsafeTrait {}

batch_trait!(
    A: usize, isize;
    B: <T> B<T> Vec<T>;
    unsafe UnsafeTrait: usize
);
```

## Capability matrix vs `#[batch_impl]`

| Capability | `batch_trait!` | `#[batch_impl]` / `#[batch_impl_only]` |
|---|---|---|
| type-matrix DSL (`.`, space, `[]`, `()`, splat) | ✅ | ✅ |
| built-in `@` constants (`@u*`, ranges, ...) | ✅ | ✅ |
| **user** `@name=value;` constant sections | ✅ (leading only) | ❌ (reverted in 0.8.0) |
| `@trait` | ✅ (segment-level — each segment's own path) | ✅ (local name / external path) |
| `@all` selectors, `@all_type_params` etc. | ❌ | ✅ |
| `#` directives (`#fill` / `#delegate` / `#blanket` / open extension) | ❌ | ✅ |
| `impl{...}` shape templates | ✅ | ✅ |

**`#` directives are not supported**: directives need the trait definition
as the signature source of truth, which `batch_trait!` as a function-like
macro cannot access. Use `#[batch_impl]` / `#[batch_impl_only]` when you
need directives.

## User constant sections

A leading `@name=value;` section defines reusable constants (values may
chain references and embed DSL expressions); the section must come **before
all trait segments**:

```rust
# use batch_impl::batch_trait;
# trait A {}
# trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> @uints;
}
```

Reserved names: `@trait` (segment-level marker), the whole `@all` family,
and any built-in constant name — redefining them errors at the definition.

## Segment-level `@trait`

`@trait` in a `batch_trait!` segment expands to **that segment's** trait
path — enabling cross-segment packing reuse:

```rust
# use batch_impl::batch_trait;
# trait A<T> {}
# trait B<T> {}
batch_trait! {
    @type_t = <T> @trait<T>;
    A: @type_t Vec<u8>;
    B: @type_t Vec<u16>;
}
// A's @trait is A, B's @trait is B — one definition, applied to each segment's own trait.
```

## When to prefer which entry

- trait defined locally, need directives → `#[batch_impl]`;
- trait defined elsewhere, need directives → `#[batch_impl_only]`;
- trait defined elsewhere, type-matrix only → `batch_trait!`.

**Documentation marker only — never call this function.**
