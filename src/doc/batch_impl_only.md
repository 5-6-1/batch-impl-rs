# `#[batch_impl_only]` — Batched Impls Without the Trait Definition

Same DSL as `#[batch_impl]`, but **discards the annotated trait definition**
and only emits `impl` blocks. The annotated trait serves purely as the
"signature source of truth" for the directive system.

## When to use it

- The trait is **already defined elsewhere** (another crate, another module)
  and you only need batched impl generation here;
- You want the trait definition to stay at its real home, not duplicated at
  every batch site.

The syntax is identical to `#[batch_impl]` — same DSL: type matrix
(`.` / space / `[]` / `()` / splat / `<>` / `where{...}` / `{body}`),
`@` constants, `#` directives, the `impl{...}` shape templates, the open
extension.

## How the trait definition is used

The annotated trait is **dropped from the output** — only the `impl` blocks
are emitted. It feeds the directive system:

- `#name` / `#fill` / `#delegate` read item signatures from it;
- the open extension `#name(args){body}` hands (method name list, body, the
  whole trait) to your same-named function-like macro;
- `@all`-family selectors and `@all_type_params` etc. extract item / generic
  lists from it.

```rust
# use batch_impl::batch_impl_only;
trait Greet { fn hello(&self) -> &str; }

// The annotated trait is dropped; existing definitions are unaffected.
#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; }
// → impl Greet for usize { fn hello(&self) -> &str { "hi" } }
```

## The external-trait path prefix

When the real trait is defined **elsewhere**, write the annotated dummy
trait with a **matching name** and give the real path as a prefix:

```text
#[batch_impl_only(# path::to::Trait: specs...)]
trait Trait { ... }   // dummy, dropped — the name must match the path's last segment
```

The `#` prefix marks the external path; `batch_impl` does not support it
(it emits the local trait definition, so a path prefix is meaningless). The
path prefix's last ident must match the annotated trait's name, otherwise
the DSL's `Trait<T>` matching would fail. The path is used everywhere the
trait is referenced — the generated impls' trait name and `@trait`.

```rust
# use batch_impl::batch_impl_only;
mod ext {
    pub trait Conv {
        fn conv(&self) -> u32;
    }
}
#[batch_impl_only(# ext::Conv: u32 #conv{0})]
trait Conv { fn conv(&self) -> u32; }
// → impl ext::Conv for u32 { fn conv(&self) -> u32 { 0 } }
```

## Relation to `batch_trait!`

`batch_trait!` is the function-like macro for traits already defined
elsewhere **without the directive system** (it cannot access a trait
definition — the `#` directives and `@all` selectors need one). Use
`#[batch_impl_only]` when you need directives; `batch_trait!` when you only
need the type-matrix DSL.

**Documentation marker only — never call this function.**
