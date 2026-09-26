Documentation-only guide for `#name(args){body}`; do not invoke `batch_impl_open!`.

# The Open-Extension Protocol — `#name(args){body}` for User Macros

An unknown `#name(args){body}` — a directive name that is **not** a built-in
(`fill` / `delegate` / `blanket`) — expands to a call of a user-defined
function-like macro of the same name, handed the args, body and trait
definition:

```text
Target #my_ext(x){y}   →   my_ext!{ {Target} (x) {y} trait_def }
```

**The deliverable of this extension point is the protocol shape itself**:
batch-impl does not implement your codegen — it only guarantees the input
reaches your same-named macro. Your macro emits arbitrary items (typically
its own impl).

## The top-level protocol (recommended form)

The `#name(args){body}` form emits its macro call at top level: codegen
prepends the spec body (target type + preceding blocks, merged in chain
order into one Brace group), making the macro input **four segments**:

```text
{spec}(args){body} trait
```

1. `{spec}` — the spec body: the target type plus the spec's preceding
   blocks, merged in chain order into one `{...}` group. The preceding
   blocks' outer braces are removed; their contents follow the target;
2. `(args)` — the directive arguments (a parenthesized group, verbatim);
3. `{body}` — the directive body (a Brace group, verbatim);
4. `trait` — the whole annotated trait definition.

You can also write the top-level macro call explicitly:

```rust
use batch_impl::{batch_impl, batch_preprocess_test};

#[batch_impl(u16 {! batch_preprocess_test! {
    (add, inc) {*self + 3}
    trait AddInc {
        fn add(&self) -> Self;
        fn inc(&self) -> Self;
    }
}})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

fn main() {
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
}
```

The `!` inside the block marks **top-level emission**: codegen strips it,
prepends the spec body, and emits the macro call at top level (no impl
generated). The `{! ...}` block must be the last block of the spec.

The reference implementation is [`batch_preprocess_test!`](batch_preprocess_test) —
a function-like macro that parses the four segments and emits a full
`impl Trait for {spec}`. Its top-level generator supports a plain target
type and a non-generic trait; it does not parse arbitrary body items or
impl generic declarations in `{spec}`.

The protocol runs before type materialization, so `{spec}` can still contain
Pack expressions and candidates. A receiver that supports those expressions can
capture the spec tokens and send them through `batch_impl_only` again; treating
every spec as `$target:ty` does not support the complete DSL.

Generated fresh declarations also travel as structured `@{group_position}`
carriers, together with their references and bounds. Forward these tokens
unchanged: converting them to names before re-entry would lose identity and
the selected branch's fresh scope. Re-entry reserves carried declarations
before evaluating new generators, so an adapter may prepend or append a
generator without merging its parameters with those already in the spec.
Ordinary position references do not reserve or create declarations.

## Writing a top-level extension macro

Your macro receives `{spec}(args){body} trait`. Capture all tokens in the
spec group with repetition, and capture the entire trait with `$trait:item`
(without another literal `trait` before it). This adapter forwards the
four segments to the reference consumer and generates a working impl for
the multi-token target `Vec<u8>`:

```rust
use batch_impl::batch_impl;

macro_rules! my_extension {
    ({ $($spec:tt)* } ( $($args:tt)* ) { $($body:tt)* } $trait:item) => {
        batch_impl::batch_preprocess_test! {
            { $($spec)* } ( $($args)* ) { $($body)* } $trait
        }
    };
}

#[batch_impl(Vec<u8> #my_extension(length){self.len()})]
trait Length {
    fn length(&self) -> usize;
}

fn main() {
    assert_eq!(vec![1u8, 2, 3].length(), 3);
}
```

The args are a parenthesized group (the `(args)` part), the body a Brace
group (the `{body}` part), and the trait definition is a full `trait` item.
An extension that inspects the trait's contents can match its structure
with `macro_rules!`, or use a proc macro to parse it with `syn`.

The adapter above inherits the reference consumer's restrictions. The
general protocol can carry more than a target type, so `$spec:ty` is not a
general replacement for `$($spec:tt)*`. For example, this extension receives
a preceding body item and emits data describing the input instead of an impl:

```rust
use batch_impl::batch_impl;

macro_rules! inspect_extension {
    ({ $($spec:tt)* } ( $($args:tt)* ) { $($body:tt)* } $trait:item) => {
        const PARTS: [&str; 4] = [
            stringify!($($spec)*), stringify!($($args)*),
            stringify!($($body)*), stringify!($trait),
        ];
    };
}

#[batch_impl(Vec<u8> { const TAG: usize = 9; } #inspect_extension(length){7})]
trait Length {
    fn length(&self) -> usize;
}

fn main() {
    assert!(PARTS[0].contains("Vec"));
    assert!(PARTS[0].contains("const TAG"));
    assert_eq!(PARTS[1], "length");
    assert_eq!(PARTS[2], "7");
    assert!(PARTS[3].starts_with("trait Length"));
}
```

## The deprecated in-impl form

The legacy **in-impl form** `T {m!{...}}` (no `!` — the call lands in the
impl body as associated items) is **deprecated** since 0.7.2 and kept only
for compatibility. It has **three** segments (no spec group):
`(args){body} trait` — the macro emits `fn signature { body }` per method,
reusing the trait signature (equivalent to handing the `#fill`
implementation to the user). Write new extensions against the top-level
`{! m!{...}}` four-segment protocol only; no warning channel exists, so the
deprecation lives in the docs.

## Design constraints

- **Why a function-like macro call, not an attribute**: a trait is not a
  valid item inside an impl block (`#[attr] trait` cannot appear in an
  impl), whereas a function-like macro in an impl-body position is expanded
  by rustc into associated items. `name!{...}` is the only shape that works
  in both the top-level and the deprecated in-impl positions.
- **Name collisions with built-ins**: an item or macro named `fill` /
  `delegate` / `blanket` is looked up verbatim (no builtin-typo guard) —
  an open-extension typo expands and surfaces as rustc's own "macro not
  found".

**Documentation marker only — never call this function.**
