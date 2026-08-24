# The Open-Extension Protocol — `#name(args){body}` for User Macros

An unknown `#name(args){body}` — a directive name that is **not** a built-in
(`fill` / `delegate` / `blanket`) — expands to a call of a user-defined
function-like macro of the same name, handed the args, body and trait
definition:

```text
#my_ext(x){y}   →   { my_ext!{ (x) {y} trait_def } }
```

**The deliverable of this extension point is the protocol shape itself**:
batch-impl does not implement your codegen — it only guarantees the input
reaches your same-named macro. Your macro emits arbitrary items (typically
its own impl).

## The top-level protocol (the only supported form)

The protocol is **top-level only** since 0.6.7: codegen prepends the spec
body (target type + preceding blocks, merged in chain order into one Brace
group), making the macro input **four segments**:

```text
{spec}(args){body} trait
```

1. `{spec}` — the spec body: the target type plus the spec's preceding
   blocks, merged in chain order into one `{...}` group;
2. `(args)` — the directive arguments (a parenthesized group, verbatim);
3. `{body}` — the directive body (a Brace group, verbatim);
4. `trait` — the whole annotated trait definition.

```rust,ignore
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test;
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self+3} trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }}})]
trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }
```

The `!` inside the block marks **top-level emission**: codegen strips it,
prepends the spec body, and emits the macro call at top level (no impl
generated). The `{! ...}` block must be the last block of the spec.

The reference implementation is [`batch_preprocess_test!`](batch_preprocess_test) —
a function-like macro that parses the four segments and emits a full
`impl Trait for {spec}` — the pattern to copy for your own extensions.

## Writing a top-level extension macro

Your macro receives `{spec}(args){body} trait`. A minimal template:

```rust,ignore
macro_rules! my_extension {
    ({ $spec:tt } ( $($args:tt)* ) { $($body:tt)* } trait $trait:item) => {
        // emit your own impl (or any items) here
    };
}
```

The args are a parenthesized group (the `(args)` part), the body a Brace
group (the `{body}` part), and the trait definition is a full `trait` item —
all accessible as `tt` fragments if your macro is written with
`macro_rules!`-style parsing; a proc-macro can parse them with `syn`.

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
