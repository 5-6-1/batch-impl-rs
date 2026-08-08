Documentation placeholder for the open-extension protocol.

A `#name(args){body}` whose `name` is not a built-in directive expands to
a call of a user-defined function-like macro of the same name, handed the
args, body and trait definition:
`#my_ext(x){y}` → `{ my_ext!{ (x) {y} trait_def } }`.

```
# use batch_impl::batch_impl;
macro_rules! my_ext { ($($rest:tt)*) => {}; }
#[batch_impl(Box<u32> #my_ext(x){y})]
trait O {}
# fn main() {}
```

**Documentation marker only — never call this function.**
