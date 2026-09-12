//! Componentization: the DSL is a **bag of blocks** — declarations, directive
//! blocks, code blocks and types can appear in any order, and the chain folds
//! them with `apply`. There is no positional requirement (a `{...}` block
//! does not have to be last, a declaration does not have to be first): the
//! same spec written in three orders yields the same impl.
//!
//! (The question.rs experiment that motivated this: `<T> {body} Box T` used
//! to fail with "unexpected `{`" — attachments were stripped from the tail
//! only. The block model made every block a chain citizen.)

use batch_impl::batch_impl;
use std::collections::HashMap;

// declarations + directive block + target, three orders — identical impls
#[batch_impl(<A> <B> #tag{"ab"} HashMap<A, B>)]
trait ComposeA {
    fn tag(&self) -> &'static str;
}

#[batch_impl(#tag{"ab"} <A> <B> HashMap<A, B>)]
trait ComposeB {
    fn tag(&self) -> &'static str;
}

#[batch_impl(<A> #tag{"ab"} <B> HashMap<A, B>)]
trait ComposeC {
    fn tag(&self) -> &'static str;
}

// const declaration interleaved with a directive block
#[batch_impl(<A> #tag{"c"} <const N: usize> [A; N])]
trait ComposeD {
    fn tag(&self) -> &'static str;
}

// two attachment blocks in a row (directive body + extra code block)
#[batch_impl(#tag{"d"} <A> Box<A> { fn extra() -> u32 { 7 } })]
trait ComposeE {
    fn tag(&self) -> &'static str;
    fn extra() -> u32;
}

#[test]
fn componentization() {
    fn check_a<T: ComposeA>(t: &T) {
        assert_eq!(t.tag(), "ab");
    }
    check_a(&HashMap::<u8, u16>::new());
    fn check_b<T: ComposeB>(t: &T) {
        assert_eq!(t.tag(), "ab");
    }
    check_b(&HashMap::<u16, u8>::new());
    fn check_c<T: ComposeC>(t: &T) {
        assert_eq!(t.tag(), "ab");
    }
    check_c(&HashMap::<i8, i16>::new());

    fn check_d<T: ComposeD>(a: T) {
        assert_eq!(a.tag(), "c");
    }
    check_d([1u8, 2, 3]);

    fn check_e<T: ComposeE>(b: T) {
        assert_eq!(b.tag(), "d");
    }
    check_e(Box::new(0u8));
    assert_eq!(<Box<u8> as ComposeE>::extra(), 7);
}

// ------------------------------------------------------------
// The bare and braced spellings of one template are the **same template**
// (documented as token-equivalent): `impl Template {body}` ≡
// `impl{Template} {body}`. Regression (F1 of the review pass): the bare-region
// collector runs **before** `angle_collect`, so a `+` inside a flat angle
// argument list (`Box<dyn Fn() + Send>`) was read as a top-level bound chain and
// the bare spelling was diagnosed as an `impl <trait-object>` target, while the
// braced spelling collected. Both must generate the same impl.
// ------------------------------------------------------------
#[batch_impl(Box<dyn Fn() + Send> impl Box<dyn Fn() + Send> { fn tag(&self) -> u8 { 0 } })]
trait BareTemplateTraitEq {
    fn tag(&self) -> u8;
}

#[batch_impl(Box<dyn Fn() + Send> impl{Box<dyn Fn() + Send>} { fn tag(&self) -> u8 { 0 } })]
trait BracedTemplateTraitEq {
    fn tag(&self) -> u8;
}

// The same root makes a `{...}` inside a flat angle list (`W<{ 1 }>`) a
// const-generic argument rather than the impl body (F5): both spellings again.
#[batch_impl(W<{ 1 }> impl W<{ 1 }> { fn tag(&self) -> u8 { 0 } })]
trait BareBraceArgEq {
    fn tag(&self) -> u8;
}

#[batch_impl(W<{ 1 }> impl{W<{ 1 }>} { fn tag(&self) -> u8 { 0 } })]
trait BracedBraceArgEq {
    fn tag(&self) -> u8;
}

struct W<const N: usize>;

#[test]
fn bare_and_braced_templates_are_the_same_template() {
    // Fully qualified: the receiver implements two of the four `tag` traits, so
    // method syntax would be ambiguous.
    let f: Box<dyn Fn() + Send> = Box::new(|| {});
    assert_eq!(<Box<dyn Fn() + Send> as BareTemplateTraitEq>::tag(&f), 0);
    assert_eq!(<Box<dyn Fn() + Send> as BracedTemplateTraitEq>::tag(&f), 0);
    assert_eq!(<W<1> as BareBraceArgEq>::tag(&W::<1>), 0);
    assert_eq!(<W<1> as BracedBraceArgEq>::tag(&W::<1>), 0);
}
