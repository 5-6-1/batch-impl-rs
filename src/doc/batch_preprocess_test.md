# `batch_preprocess_test!` — The Reference Open-Extension Macro

The **reference implementation of the open-extension protocol** (and the
test consumer): a function-like macro that parses the open-extension input
`name!{ {spec}(method name list){body} trait T {...} }` and emits a full
`impl Trait for {spec}` — the pattern to copy when writing your own
`#name(args){body}` extensions.

## The protocol shape it consumes

The open extension expands `#name(args){body}` into a call of your
same-named macro with this input (see [`directive_open`](directive_open) for
the full protocol):

```text
{spec}(args){body} trait
```

1. `{spec}` — the spec body (target type + preceding blocks, merged in
   chain order) — **top-level form only**;
2. `(args)` — the method name list (parenthesized group);
3. `{body}` — the directive body (Brace group);
4. `trait` — the whole trait definition.

`batch_preprocess_test!` parses these four segments and emits a full
`impl Trait for {spec}` with one `fn signature { body }` per method (the
signature reused from the trait).

## Usage

```rust,ignore
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test;
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self+3} trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }}})]
trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }
// → impl AddIncU16 for u16 {
//     fn add(&mut self, x: u16) { *self + 3 }   (signature from the trait, body yours)
//     fn inc(&mut self) { *self + 3 }
//   }
```

(The `*self + 3` bodies above are illustrative — a real delegation would
use the params; the point is the protocol shape: four segments, signature
from the trait, body from the block.)

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

**Documentation marker only — never call this function.**
