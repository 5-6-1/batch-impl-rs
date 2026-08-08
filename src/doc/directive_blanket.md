Documentation placeholder for the `#blanket` directive.

`#blanket(args){wrapper list}` implements the trait for every wrapper
around a fresh generic `T`, delegating each method by deref. Wrappers may
carry a `:N` deref-depth annotation and a `where{...}` predicate; a
wrapper whose main part contains `@0` treats `@0` as T's position
(`(u32, @0)` → `(u32, T)`), otherwise it is applied as `wrapper^T`.

```
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]
trait B { fn tag(&self) -> u32; }
# fn main() {}
```

**Documentation marker only — never call this function.**
