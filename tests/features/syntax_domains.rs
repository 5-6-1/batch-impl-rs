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
fn opt_in_body_sync_rewrites_macro_input() {
    assert_eq!(<u8 as Synced<u16>>::text().replace(' ', ""), "Unrelated<u16>");
}

#[batch_impl(<u8 as Family>::Typed<fn(#[allow(unused)] u8)>)]
trait AttributeTarget {}

#[batch_impl(<u8 as Family>::Typed<fn(
    #[cfg_attr(all(), allow(unused))] fn(#[allow(unused)] u16)
)>)]
trait NestedAttributeTarget {}

#[test]
fn qualified_types_preserve_function_parameter_attributes() {
    fn check<T: AttributeTarget>() {}
    fn nested<T: NestedAttributeTarget>() {}
    check::<fn(u8)>();
    nested::<fn(fn(u16))>();
}

macro_rules! opaque_index {
    (@0) => {
        u8
    };
    (([@0])) => {
        u16
    };
    (nested!(@0)) => {
        u32
    };
    (@{0}) => {
        u64
    };
}

#[batch_impl(OpaqueAssociated<Item = (
    opaque_index!(@0),
    opaque_index![@0],
    opaque_index! { @0 },
    opaque_index!(([@0])),
    opaque_index!(nested!(@0)),
    opaque_index!(@{0}),
    @0,
)> ().1)]
trait OpaqueAssociated {
    type Item;
}

#[test]
fn associated_values_keep_macro_tokens_and_resolve_visible_fresh() {
    let value: <(bool,) as OpaqueAssociated>::Item = (1u8, 2u8, 3u8, 4u16, 5u32, 6u64, true);
    assert_eq!(value, (1, 2, 3, 4, 5, 6, true));
}
