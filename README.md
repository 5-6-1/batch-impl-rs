# batch-impl

Repository sources (GitHub `main`): English | [简体中文](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/README.md)

**v0.10.2.** Breaking changes and migration from 0.9.7 are in the [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/CHANGELOG.md).

Repository-source links open public `main`, which may not contain local changes.
Run `cargo doc --no-deps --open` locally for this checkout's English documentation;
its top navigation stays within that build. Read the Chinese tutorial and
reference from the same source checkout.

A procedural macro crate that generates Rust trait implementations for a list or matrix of types.

## Why use it

- **Keep related implementations together.** Write which types share a body; changing that body updates the whole group.
- **Copy signatures from your trait.** `#method{body}` fills in the named method's signature, so your implementation only supplies its body.
- **Grow from lists to families.** Add generic containers, tuple lengths or delegated wrappers when the task needs them. The generated implementations are ordinary Rust, checked by rustc.

If you already have an ordinary Rust `impl`, you can [reuse that implementation](#reuse-an-existing-impl), including for an external trait, without a signature mirror. For signature-copying, filling or delegation directives, a local trait supplies its annotated definition; an external trait needs a mirror maintained through `batch_impl_only`. See [entry choices](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#11-entry-points).

## Quick start

Requires **Rust 1.95 or newer**. This page describes the **0.10.2 source tree**. To try the published 0.9.7 instead, use its [versioned documentation](https://docs.rs/batch-impl/0.9.7/batch_impl/).

To try this checkout, create a small application next to it:

```text
work/
  batch-impl/    # this source checkout
  demo/         # your new application
```

Run `cargo new demo` from `work/`. In `demo/Cargo.toml`, replace the existing empty `[dependencies]` section with:

```toml
[dependencies]
batch-impl = { path = "../batch-impl" }
```

Use the checkout containing the changes you want to test. Local uncommitted changes are available through this path dependency; a Git dependency cannot retrieve them. The published `batch-impl = "0.10.0"` release is on crates.io; the checkout is for changes that are not published yet.

Copy this complete program into `demo/src/main.rs`:

```rust
use batch_impl::batch_impl;

#[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(12u16.describe(), "number: 12");
    assert_eq!(300u32.describe(), "number: 300");
    println!("{}", 7u8.describe());
}
```

Run `cargo run` from `demo/`. All three assertions pass and the program prints `number: 7`.

When reducing the targets, write `u8` for one type or `[u8,]` for a one-element list; **`[u8]` means the slice type**. Lists allow a trailing comma.

`[u8, u16, u32]` selects three types. `#describe{...}` copies the signature of `Describe::describe` and gives each implementation the same body. The name after `#` is your trait's member name, not a fixed keyword. One of the three generated implementations is:

```text
impl Describe for u8 {
    fn describe(&self) -> String { format!("number: {self}") }
}
```

## Reuse an existing impl

If you already have an implementation, add the attribute to it. Here is another complete program:

```rust
use batch_impl::batch_impl;

struct UserId(u64);
struct OrderId(u64);

#[batch_impl(@Self: [UserId, OrderId])]
impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id:{}", self.0)
    }
}

fn main() {
    assert_eq!(UserId(7).to_string(), "id:7");
    assert_eq!(OrderId(12).to_string(), "id:12");
}
```

`@Self` denotes the input impl's self type (`UserId` here); the list supplies the generated targets. The generated implementations replace the original impl, so include `UserId` to keep its implementation. This entry reuses complete methods and needs neither `#fmt` nor a signature mirror of `Display`.

The fields, methods, constructors and bounds used by the implementation must work for every target; here both types have a displayable `.0` field. See the tutorial's [entry choices](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#11-entry-points) for the details.

## Continue with the same trait

Return to the quick start's `Describe` and replace that program with this extension. It adds a generic `Vec<T>` implementation and delegates `Box<T>` to any inner `T` that implements `Describe`:

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")},
    <T> Vec<T> #describe{format!("{} items", self.len())},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(vec![1, 2, 3].describe(), "3 items");
    assert_eq!(Describe::describe(&Box::new(7u8)), "number: 7");
}
```

The `<T> Vec<T>` entry declares an impl generic. `#blanket` generates `impl<T: Describe> Describe for Box<T>`; its reference-receiver methods forward to the inner value. The last assertion calls that wrapper implementation explicitly.

For a type with different behavior, give it a separate spec and let the remaining types share a body. **Local and shared bodies merge; neither overrides the other.** Supplying the same method twice is an error. The [continuous exercise](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#1-implement-and-call-one-method) walks through adding types, methods, special behavior, generic bounds and wrapper forwarding.

For a wrapper you own, `#delegate` forwards through a field. For an enum with different inner types, `inner.#call` forwards inside each branch. Continue in the [delegation tutorial](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#7-the-directive-system-).

## When the result is unexpected

Temporarily put the attribute and its trait or impl inside `batch_impl::batch_preview!`:

```text
batch_impl::batch_preview! {
    #[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
    trait Describe { fn describe(&self) -> String; }
}
```

Run `cargo check` and read the generated Rust in the diagnostic. **The preview deliberately reports a compile error to display its output.** Compare it with the implementation you intended, adjust the DSL, then remove the preview wrapper and compile normally. The [preview tool](https://github.com/5-6-1/batch-impl-rs/blob/main/src/doc/batch_preview.md) also provides complete examples for `batch_impl_only` and ordinary impl entries.

For example, `Box.Vec u32` expands to `Box<Vec, u32>`. Rust may complain about missing generic arguments or `allocator_api`; the diagnostic need not show the whole generated type. To express nesting, write `Box (Vec u32)`, `Box.Vec.u32`, or the ordinary Rust type `Box<Vec<u32>>`.

## Reading type expressions

Start with ordinary Rust types and lists. Space applies arguments from left to right; `.` groups from the right for nesting:

| Writing | Meaning |
|---|---|
| `[u8, u16]` | One implementation for each type |
| `Vec u8` | `Vec<u8>` |
| `HashMap u32 String` | `HashMap<u32, String>` |
| `Box (Vec u8)` / `Box.Vec.u8` | `Box<Vec<u8>>` |
| `Box.Vec u32` | `Box<Vec, u32>` — `.` binds first, so the space appends a *second* argument (three probes read this as a mistake) |
| `[Box, Vec] [u8, u16]` | Four container/type combinations |
| `().3` | One generic three-element tuple implementation |
| `(*Vec *[].3,)` | One tuple with independently generated `Vec<P0>`, `Vec<P1>`, `Vec<P2>` members (fresh generics are named `P0…`) |

Space and `.` remain left- and right-associative respectively; `.` binds before space. Parentheses make the intended grouping explicit. The [tutorial](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md) develops these rules through examples; the [reference](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md) records their boundaries.

## Feature overview

The first learning path is lists and a shared method body, then generics, constraints and delegation. The rest is available as needed; compact expressions are optional.

| Feature | Use it for | Tutorial source (GitHub main) |
|---|---|---|
| Lists and `#method{body}` | Give several types one implementation body | [§1](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#1-implement-and-call-one-method), [§3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#3-lists-and-body) |
| Ordinary impl entry | Apply an implementation you already wrote to several targets | [§11](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#11-entry-points) |
| Space / `.` and grouping | Apply generic arguments and nest containers | [§2](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#2-type-matrix-the-space-and-) |
| Generic inheritance and associated types | Reuse trait parameters, bounds and associated-type bindings | [§5](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#5-generics-) |
| `where` | Add constraints to a group of implementations | [§8.1–§8.3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#8-where-clauses) |
| `#fill`, `#delegate`, `#blanket` | Fill several members or forward methods to inner types | [§7](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#7-the-directive-system-) |
| Packs `*` | Map construction rules over members and splice arguments | [§4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#4-packs---mapping-and-splicing) |
| `@` constants and positional references | Select type families or refer to generated parameters | [§6](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#6-the--constant-system-macro-meta-layer) |
| Tuple lengths and Cartesian powers | Generate tuple families and type combinations | [§9](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#9-tuple-generation-and-matrices) |
| Shape templates | Instantiate an implementation pattern across nested types | [§8.4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#84-the-impl-shape-templates-080) |
| Type modifiers | References, pointers, function types, attributes and unsafe impls | [§10](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#10-the-modifier-gallery) |
| Open directives and repeat blocks | Extend generation or repeat a body over tuple positions | [§7](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#7-the-directive-system-), [§8.4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md#84-the-impl-shape-templates-080) |

For shared code, choose the spelling that makes the generated implementations easiest to review. Use a named type, explicit generic arguments or parentheses when they communicate the task better than a compressed matrix expression.

## Built with batch-impl

[alga2](https://docs.rs/alga2) uses batch-impl for an abstract-algebra hierarchy across numbers, tuples, arrays, smart pointers and other types. The repository's [simplify example](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs) shows a smaller complete case: **30 implementations from about 15 lines of DSL**.

The library is most useful when many implementations follow the same rules or need coordinated updates. For a few independent implementations, compare the amount of repetition with what your teammates would need to learn. External traits used through `batch_impl_only` also carry the cost of maintaining their signature mirrors.

## Expansion cost

The macro runs at compile time. `cargo test --lib perf -- --nocapture` measures the expansion pipeline, excluding rustc's type checking. On the author's machine, nine stable-Rust runs measured a 1024-impl Cartesian spec at **0.10–0.20 ms/impl** and a typical four-impl spec at **0.6–2.6 ms**. These are observations, not performance guarantees; the test prints current measurements when run.

## Compatibility and migration

Existing token semantics are covered by the syntax-freeze commitment introduced in 0.7.2. Compatible releases add capabilities, improve diagnostics and correct behavior that contradicts the documented rules. Deliberate syntax changes require a compatibility boundary and migration notes.

This development cycle targets **0.10.0**, rather than the previously planned 0.9.8, because it includes deliberate breaking changes. Cargo's `"0.9.7"` requirement permits 0.9.8 but excludes 0.10.0; see [Cargo's version rules](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#default-requirements).

- Replace removed `@all_fresh` with `@0..`.
- `*` opens candidate lists only: write `*[F,G] T` to map both constructors. A tuple is a type, so `*(F,G)` is one member — the tuple — and `*(F,G) T` appends `T` into it. A lone pack no longer turns a group into a container: use `(*X,)` / `[*X,]`. See [the Pack migration rules](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md#46-branches-coherence-and-migration).
- Rename custom constants named `Self`; `@Self` is now reserved for the impl entry's input self type.
- Named type-family ranges follow Rust's endpoint convention: `@u8..u16` selects only `u8`; use `@u8..=u16` to keep both. Omitted upper endpoints still include the family's **maximum** - `@u16..` is `@u16..=u128`, and neither `usize` nor `isize` belongs to a range family.

The [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/CHANGELOG.md) records the full migration. The source and docs on `main` describe ongoing development; use a release's versioned documentation when maintaining that release.

**How to check your migration.** Two measurements catch nearly everything, and both are cheap:

- **Count the impls.** `cargo expand --lib | grep -c 'impl.* for '`, or grep for one trait (`cargo expand --lib | grep -c 'impl.*EuclideanDomain'`), and compare with the old release. A deliberate boundary change shows up as a **missing impl**, not as an error — ranges are the usual cause (`@u8..u64` excludes `u64` now; `@u8..=u64` includes it), and the failure usually surfaces far away as `the trait bound … is not satisfied`.
- **Read the expansion.** Wrap a spec in `batch_preview!`: it prints the number of impls it generated and every target type as `compile_error!` text, so "this spec produced fewer impls than I meant" is visible before anything downstream breaks.

Neither check needs a release of its own: both run against the tree you are migrating.

## Next steps

- [Tutorial](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md): start with one useful implementation, then follow the task-based reading route.
- [Reference manual](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md): syntax rules, legal positions, diagnostics and limits.
- [Runnable quickstart](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/quickstart.rs), [simplify](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs), and [typeclass](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/typeclass.rs): complete examples; run with `cargo run --example quickstart` (or the other example name).
- [Architecture](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/architecture.md), [development guide](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/development-guide.md), and [developer changelog](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/dev-changelog.md): contributor documentation.

## License

MIT OR Apache-2.0
