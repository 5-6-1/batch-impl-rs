# `batch_preview!` — See the Expansion Before You Compile

Wrap an attributed trait or impl to inspect its generated Rust. The preview
reports the expansion through `compile_error!`: **compilation fails on
purpose**, and the diagnostic contains the result. It does not install the
displayed trait or impls into your program.

Keep the attribute and item unchanged when removing the outer
`batch_preview! { ... }` wrapper. Then build normally: previewing tokens
does not check whether fields, methods, bounds or overlapping impls satisfy
Rust's rules.

The displayed text can retain the attribute used to select the preview
entry, including `#[batch_impl(...)]`. Inspect the generated targets and
members, then restore the original attributed program as above; do not
paste the diagnostic text back as fully expanded source.

## Syntax

The wrapper takes one item with its attribute:

| Input | Expansion shown in the diagnostic |
|---|---|
| `#[batch_impl(...)] trait Name { ... }` | The trait definition and generated impls |
| `#[batch_impl_only(...)] trait Name { ... }` | Generated impls only; the signature mirror is discarded |
| `#[batch_impl(...)] impl Trait for Type { ... }` | Generated impls only; the input impl is replaced |

An inherent `impl Type { ... }` works through the same impl entry. The
examples below are complete programs once the crate is a dependency. Run
each with `cargo check`; each deliberately produces a preview diagnostic.

## A local trait

```compile_fail
use batch_impl::{batch_impl, batch_preview};

batch_preview! {
    #[batch_impl([u8, u16] #describe{format!("number: {self}")})]
    trait Describe { fn describe(&self) -> String; }
}

fn main() {}
```

The message reports two impls and contains `trait Describe`,
`impl Describe for u8` and `impl Describe for u16`. After removing the
preview wrapper, add `assert_eq!(7u8.describe(), "number: 7");` and
`assert_eq!(8u16.describe(), "number: 8");` inside `main` and run `cargo run`.

## An existing trait with a signature mirror

```compile_fail
use batch_impl::{batch_impl_only, batch_preview};

struct UserId(u64);
struct OrderId(u64);

batch_preview! {
    #[batch_impl_only(#std::fmt::Display: [UserId, OrderId]
        #fmt{write!(f, "{}", self.0)})]
    trait Display {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
    }
}

fn main() {}
```

The message contains two `std::fmt::Display` impls, for `UserId` and
`OrderId`, and **no trait definition**. The local `trait Display` is the
signature mirror used by `#fmt`; it does not redefine `std::fmt::Display`.

## An existing impl without a signature mirror

If you already have a complete implementation, reuse that impl directly:

```compile_fail
use batch_impl::{batch_impl, batch_preview};

struct UserId(u64);
struct OrderId(u64);

batch_preview! {
    #[batch_impl(@Self: [UserId, OrderId])]
    impl std::fmt::Display for UserId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
}

fn main() {}
```

This message also contains exactly two `std::fmt::Display` impls and no
trait definition. `@Self` reads the input self type, `UserId`; including
`UserId` in the target list keeps its implementation alongside `OrderId`'s.
Both types provide the `.0` field used by the body. The macro does not
infer different field accesses for differently structured targets.

For either Display example, remove the preview wrapper and put
`assert_eq!(UserId(7).to_string(), "7");` and
`assert_eq!(OrderId(8).to_string(), "8");` inside `main`. `cargo run` then
checks the actual implementations and runs both assertions.

## The associativity note

The trait-entry preview also teaches the `.`/space associativity identity:
a known 1-arity container rendered with 2+ args (`Box<Vec, u32>`) is the shape of
`Box.Vec u32` (= `Box Vec u32`, since `A.B C` = `A B C` = `A<B, C>`), and
the note suggests the nesting rewrite (`Box.Vec.u32`). This guidance is
preview-only: the compiler path never guesses, and a user type shadowing a
known container name costs a wrong note, never a wrong build.

## Relationship to the compiler path

- Trait inputs use the same preprocessing, leaf collection and impl
  generation as the trait attribute entries (`prepare_attr_expansion`,
  `collect_spec_leaves`, `generate_impl`). The selected attribute controls
  whether the trait definition is included.
- Impl inputs call the real `expand_impl_entry` pipeline, including
  constant expansion, shape matching and impl assembly. They do not take
  the trait preview's leaf-collection route.
- `compile_fail` marks these doctests because the preview deliberately
  reports an error. A normal build after removing the wrapper is still
  needed to validate the displayed Rust.

**This is a real macro, not a documentation-only entry point** — it is the expansion channel described above, and `tests/ui/preview_ok.rs` plus `tests/ui/preview_miswrite.rs` lock its output shape and the miswrite note.
