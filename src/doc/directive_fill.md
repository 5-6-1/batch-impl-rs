Documentation placeholder for the `#fill` directive.

`#fill(args){body}` copies each selected trait item's signature and
substitutes `body` as its implementation. Selection supports the `@all`
families (`@all_methods`, `@all_ref_methods`, `@all_default_methods`,
...), individual names, and `-` subtraction (`#fill(@all_methods, -foo)`).

```
# use batch_impl::batch_impl;
#[batch_impl(Vec<u32> #fill(@all_methods){0})]
trait F { fn zero(&self) -> u32; }
# fn main() {}
```

**Documentation marker only — never call this function.**
