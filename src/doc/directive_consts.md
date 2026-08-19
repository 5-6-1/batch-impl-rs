Documentation placeholder for the `@` macro-meta constant system.

`@` names expand before all other DSL processing (`@ <> # where` order):

- built-in name families: `@uint` / `@int` / `@float` / `@num` /
  `@scalar` and wildcards `@u*` / `@i*` / `@f*`;
- range families: `@u8..u128` / `@i8..i128` / `@f32..f64` (inclusive);
- `batch_trait!` user constants: a leading `@name = value;` segment
  (lazy expansion, reference checks);
- `@N` position references (resolved by codegen) and `@trait`
  (segment-level trait path).

```
# use batch_impl::batch_impl;
#[batch_impl(Box.@u*)]
trait C {}
# fn main() {}
```

**Documentation marker only — never call this function.**
