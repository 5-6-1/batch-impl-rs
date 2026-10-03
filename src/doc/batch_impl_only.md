# `#[batch_impl_only]` — Batched Impls for an Existing Trait

Same DSL as `#[batch_impl]`, but **discards the annotated trait definition**
and only emits `impl` blocks. The annotated declaration is a local signature
mirror supplied to the macro; the macro does not read the existing trait's
definition from another module or dependency.

## When to use it

Use it when the trait is **already defined elsewhere** (another crate or
module) and you need batched impl generation without emitting another trait
definition. You still provide the signatures used by generation at the batch
site.

If you already have a complete `impl` and only need to reuse its code for
other types, apply `#[batch_impl(@Self: [UserId, OrderId])]` to that impl
instead. This works for external traits without a signature mirror. Each
target must support the fields, methods and constraints used by the shared
implementation; see the [impl-entry preview example](crate::batch_preview).

The syntax is identical to `#[batch_impl]` — same DSL: type matrix
(`.` / space / `[]` / `()` / packs `*X` / `<>` / `where{...}` / `{body}`),
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

Keep the supplied signatures, generics and constraints aligned with the real
trait. Rust checks the generated impl against that trait, but this is not a
complete mirror-consistency check. For example, a new default method in the
external trait may leave the impl valid while remaining absent from the
mirror's `@all` selection.

```rust
# use batch_impl::batch_impl_only;
trait Greet { fn hello(&self) -> &str; }

// The annotated trait is dropped; existing definitions are unaffected.
#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; }
// → impl Greet for usize { fn hello(&self) -> &str { "hi" } }
```

## Where the generated impls find the trait

Without a path prefix the impls name the trait by the annotated mirror's own
ident, so that name has to resolve **where the impls land**: a submodule needs
`use crate::TheTrait;` even though the same module also contains the mirror.
The failure is easy to misread — `E0405: cannot find trait \`X\` in this scope`
points at the mirror's definition, which looks correct precisely because the
mirror is what you wrote. Adding the import, or using the path prefix below, is
the fix; the macro cannot do it for you, because a bare name carries no path.

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

**This is a real attribute macro, not a documentation-only entry point** — apply it to a trait definition; the examples above are compiled as doctests.
