Documentation placeholder for the `#delegate` directive.

`#delegate(args){target}` generates one delegation call per selected
method: each becomes `fn m(&self, ...) -> R { (target).m(...) }`. The
`self` argument is skipped; the remaining arguments are forwarded (named
params as-is, non-identifier patterns renamed to `arg{i}` when they
cannot be used as an expression).

```
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box<Vec<u32>> #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
# fn main() {}
```

**Documentation marker only — never call this function.**
