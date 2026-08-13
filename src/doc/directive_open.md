Documentation placeholder for the open-extension protocol.

A `#name(args){body}` whose `name` is not a built-in directive expands to
a call of a user-defined function-like macro of the same name, handed the
args, body and trait definition:
`#my_ext(x){y}` → `{ my_ext!{ (x) {y} trait_def } }`.

The protocol is **top-level only**: codegen prepends the spec body, making the
four-segment input `{spec}(args){body} trait`, and your macro emits arbitrary
items (typically its own impl). The legacy in-impl form `T {m!{...}}` (no `!`,
the call lands in the impl body as associated items) is deprecated since 0.7.2
and kept only for compatibility — write new extensions against the top-level
protocol.

```
# use batch_impl::batch_impl;
macro_rules! my_ext { ($($rest:tt)*) => {}; }
#[batch_impl(Box<u32> #my_ext(x){y})]
trait O {}
# fn main() {}
```

**Documentation marker only — never call this function.**
