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
