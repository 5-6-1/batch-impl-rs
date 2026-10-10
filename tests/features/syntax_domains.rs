//! Rust expressions and opaque macro inputs retain their own token grammar.

use batch_impl::{batch_impl, batch_trait};

struct Wrap<const N: usize>;

#[batch_impl(Wrap<{ "<>".len() }>)]
trait LiteralTarget {}

trait Family {
    type Out<const N: usize>;
    type Typed<T>;
}

impl Family for u8 {
    type Out<const N: usize> = [u8; N];
    type Typed<T> = T;
}

#[batch_impl(<u8 as Family>::Out<{ 2 * 3 }>)]
trait ProductTarget {}

#[batch_impl(<u8 as Family>::Typed<[u16; 2 * 3]>)]
trait ArrayTarget {}

macro_rules! opaque_type {
    (@trait #anything *pack) => {
        u32
    };
}

#[batch_impl(<u8 as Family>::Typed<opaque_type!(@trait #anything *pack)>)]
trait MacroTarget {}

#[test]
fn rust_const_and_macro_arguments_are_not_dsl() {
    fn literal<T: LiteralTarget>() {}
    fn product<T: ProductTarget>() {}
    fn array<T: ArrayTarget>() {}
    fn opaque<T: MacroTarget>() {}
    literal::<Wrap<2>>();
    product::<[u8; 6]>();
    array::<[u16; 6]>();
    opaque::<u32>();
}

trait Quoted {
    fn text() -> [&'static str; 4];
}

batch_trait! {
    Quoted: u8 {
        fn text() -> [&'static str; 4] {
            [stringify!(@trait), stringify![@trait], stringify!{@trait},
             stringify!(nested!(@trait))]
        }
    };
}

#[test]
fn segment_trait_replacement_preserves_opaque_macro_inputs() {
    assert_eq!(
        <u8 as Quoted>::text(),
        [stringify!(@trait), stringify![@trait], stringify! {@trait}, stringify!(nested!(@trait))],
    );
}

macro_rules! opaque_const {
    (@{0} <>) => {
        7
    };
}

#[batch_impl(Wrap<{ opaque_const!(@{0} <>) }>)]
trait CarrierTarget {}

#[test]
fn fresh_carriers_in_macro_input_are_not_resolved() {
    fn check<T: CarrierTarget>() {}
    check::<Wrap<7>>();
}

#[batch_impl(Synced<u16> u8 impl{Synced<>} {
    fn text() -> &'static str { stringify!(Unrelated<>) }
})]
trait Synced<T> {
    fn text() -> &'static str;
}

#[test]
fn opt_in_body_sync_preserves_macro_input() {
    // Token round-trips can normalize whitespace, but must not fill the
    // macro's angle brackets with the trait argument.
    assert_eq!(<u8 as Synced<u16>>::text().replace(' ', ""), "Unrelated<>");
}
