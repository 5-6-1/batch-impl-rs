Documentation placeholder for the `#name{body}` fill-by-name directive.

`#name{body}` looks up the single trait item named `name` — a method, an
associated const, or an associated type — and fills it with `body` (the
body must match that item's shape).

```
# use batch_impl::batch_impl;
#[batch_impl(Box<Vec<u32>> #count{self.len()})]
trait L { fn count(&self) -> usize; }
# fn main() {}
```

**Documentation marker only — never call this function.**
