# `batch_preview!` — See the Expansion Before You Compile

The DSL-aware expansion preview: wrap the exact attribute-macro form you
would feed `#[batch_impl]` and the preview reports every generated impl
through a `compile_error!` message (the only stable terminal channel a proc
macro has). The message is the expansion verbatim — the trait plus the
impls, exactly what `#[batch_impl]` emits — followed by preview-only
guidance.

The preview exists because the DSL is a type-matrix description: the
generated impls are token-equivalent to hand-written code, and seeing them
is the fastest way to check a matrix. Copy the `#[batch_impl(...)]` line
back unchanged once you are happy with the output; DSL errors surface
exactly as they would under the real attribute macro.

## Syntax

```text
batch_preview! {
    #[batch_impl(specs...)]
    trait Name { ... }
}
```

The input is the **exact** `#[batch_impl]` invocation — attribute + trait
definition. The preview runs the same preprocessing and pipeline
(`prepare_attr_expansion` + leaf collection) and renders the expansion into
the diagnostic channel.

## Example

```compile_fail
# use batch_impl::batch_preview;
batch_preview! {
    #[batch_impl(<T> Sortable<T> [Box, Rc].Vec<T> where T: Ord {
        fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
    })]
    trait Sortable<T> { fn is_sorted(&self) -> bool; }
}
```

The preview's diagnostic channel makes this block fail to compile by design
(the message IS the output); it reports one message containing the trait
and both generated impls.

## The associativity note

The preview also teaches the `.`/space associativity identity: a known
1-arity container rendered with 2+ args (`Box<Vec, u32>`) is the shape of
`Box.Vec u32` (= `Box Vec u32`, since `A.B C` = `A B C` = `A<B, C>`), and
the note suggests the nesting rewrite (`Box.Vec.u32`). This guidance is
preview-only: the compiler path never guesses, and a user type shadowing a
known container name costs a wrong note, never a wrong build.

## Relationship to the compiler path

- The preview and the real attribute macro share the same parse/expand
  authority (`collect_spec_leaves`), so the two consumers cannot drift
  apart;
- DSL errors under the preview surface with the same wording and spans as
  under `#[batch_impl]` — the preview is a faithful look, not a separate
  implementation;
- `compile_fail` marks the doctest: the preview **always** fails to compile
  (its output is a `compile_error!`), so don't use it in code that must
  build — only interactively.

**Documentation marker only — never call this function.**
