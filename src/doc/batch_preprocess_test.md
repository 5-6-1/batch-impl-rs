# `batch_preprocess_test!` — The Reference Open-Extension Macro

The **reference implementation of the open-extension protocol** (and the
test consumer): a function-like macro that parses the open-extension input
`name!{ {spec}(method name list){body} trait T {...} }` and emits a full
`impl Trait for {spec}` for a plain target type and a non-generic trait.
It demonstrates how to receive the protocol when writing your own
`#name(args){body}` extensions; it is not a general spec parser.

## The protocol shape it consumes

The open extension expands `#name(args){body}` into a call of your
same-named macro with this input (see the `directive_open` doc block in
`lib.rs` for the full protocol):

```text
{spec}(args){body} trait
```

1. `{spec}` — the spec body (target type + preceding blocks, merged in
   chain order, without the preceding blocks' outer braces) — **top-level
   form only**;
2. `(args)` — the method name list (parenthesized group);
3. `{body}` — the directive body (Brace group);
4. `trait` — the whole trait definition.

`batch_preprocess_test!` parses these four segments and emits a full
`impl Trait for {spec}` with one `fn signature { body }` per selected method
(the signature reused from the trait). Its top-level form expects `{spec}`
to contain only a Rust target type, such as `u16` or `Vec<u8>`, and the
trait to have no generic parameters. It does not interpret preceding body
items or impl generic declarations inside that first segment. Extensions
that need those inputs must parse and generate them themselves.

## Usage

```rust
use batch_impl::{batch_impl, batch_preprocess_test};

#[batch_impl(u16 #batch_preprocess_test(add, inc){*self + 3})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

fn main() {
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
}
```

Both methods return `u16`, so the shared expression matches their copied
`-> Self` signatures. The unknown directive supplies all four protocol
segments to `batch_preprocess_test!`; no duplicate trait declaration is
needed at the call site.

## Top-level vs deprecated in-impl form

- **Top-level form** (4 segments, with the `{spec}` group — the `{! ...}`
  block): emits a full `impl Trait for {spec}` at top level;
- **deprecated in-impl form** (3 segments, no spec group — the legacy
  `T {m!{...}}`): emits `fn signature { body }` per method (reusing the
  trait signature) — equivalent to handing the `#fill` implementation to
  the user. Kept only for compatibility; write new extensions against the
  top-level form.

## Design point

This must be a **function-like macro call** `name!{...}`, not an
`#[name[...]] trait ...` attribute — a trait is not a valid item inside an
impl block (`#[attr] trait` cannot appear in an impl), whereas a
function-like macro in an impl-body position is expanded by rustc into
associated items.

**It is a working reference implementation, not a stub** — `batch_preprocess_test!`
is exercised by the open-extension UI/functional tests. Its source is a
starting point for an extension with the target and trait restrictions
above; a more general generator must handle its own spec and trait forms.
The doc blocks of
`batch_impl_delegate!` / `batch_impl_fill!` / `batch_impl_blanket!` /
`batch_impl_name!` / `batch_impl_open!` / `batch_impl_consts!` are
documentation-only entry points for the directive docs — those six are
placeholders and must not be called, this one is real.
