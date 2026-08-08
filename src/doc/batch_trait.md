Function-like macro that generates `impl` blocks for a declared trait in batch.

Syntax: `unsafe? Trait path: impl-specs;`, with `;` separating multiple trait segments.
After each segment's `:` comes a DSL expression (type DSL + `@` constants, same as
`#[batch_impl]`).

**`#` directives are not supported** (`#fill`/`#delegate`/`#blanket`/open extension):
directives need the trait definition as the signature source of truth, which `batch_trait!`
as a function-like macro cannot access; use `#[batch_impl]` / `#[batch_impl_only]` when
you need directives.

## Examples

```
# use batch_impl::batch_trait;
trait A {}
trait B<T> {}
unsafe trait UnsafeTrait{}

batch_trait!(
    A: usize, isize;
    B: <T> B<T> Vec<T>;
    unsafe UnsafeTrait: usize
);
```

Path traits (such as `foo::C`) are supported too; see tests/regression.rs.
