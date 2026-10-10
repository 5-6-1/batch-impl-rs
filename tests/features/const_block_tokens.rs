//! Ordinary const blocks keep Rust tokens; explicit shape slots still substitute.

// Keep the reported `n @ _` spelling and explicit brace boundaries intact.
#![allow(clippy::redundant_pattern, unused_braces)]

use batch_impl::batch_impl;

struct Arr<const N: usize>;

#[batch_impl(Arr<{ let n @ _ = 2; n * 3 }>)]
trait DirectConst {}

#[test]
fn direct_const_blocks_preserve_rust_patterns() {
    fn check<T: DirectConst>() {}
    check::<Arr<6>>();
}

#[batch_impl(Associated<Item = Arr<{ let n @ _ = 2; n * 3 }>> u8)]
trait Associated {
    type Item;
}

#[test]
fn associated_const_blocks_preserve_rust_patterns() {
    let _: <u8 as Associated>::Item = Arr::<6>;
}

#[batch_impl(AssociatedArray<Item = [u8; {
    match 2 { n @ 2 => n * 3, _ => 0 }
}]> u8)]
trait AssociatedArray {
    type Item;
}

#[test]
fn associated_array_lengths_do_not_fold_numeric_patterns() {
    let value: <u8 as AssociatedArray>::Item = [7u8; 6];
    assert_eq!(value.len(), 6);
}

#[batch_impl(u8 where {
    Option<Arr<{ let n @ _ = 2; n * 3 }>>: Sized,
    [u8; { match 2 { n @ 2 => n * 3, _ => 0 } }]: Sized,
})]
trait NestedWhere {}

#[test]
fn nested_where_types_preserve_const_blocks() {
    fn check<T: NestedWhere>() {}
    check::<u8>();
}

#[batch_impl(LocalMacro<u16> u8 where {
    Arr<{
        macro_rules! local_const {
            (@{99} @trait Unrelated<>) => { 6 };
        }
        local_const!(@{99} @trait Unrelated<>)
    }>: Sized,
})]
trait LocalMacro<T> {}

#[test]
fn const_blocks_keep_local_macro_definitions_and_calls_opaque() {
    fn check<T: LocalMacro<u16>>() {}
    check::<u8>();
}

trait SourceSize {
    fn size() -> usize;
}

// This unbraced length retains the existing @Self constant substitution.
#[batch_impl(@Self: [u8; ::core::mem::size_of::<@Self>()])]
impl SourceSize for u16 {
    fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

#[test]
fn unbraced_array_lengths_keep_context_constants() {
    assert_eq!(<[u8; 2] as SourceSize>::size(), 2);
}

// Named shape slots are explicitly requested substitutions, not DSL operators.
#[batch_impl(
    <const N: usize> ShapeAssociated<N, Item = [u8; { N + 1 }]>
        #[allow(unused_braces)] Arr<2> impl{Arr<N>}
)]
trait ShapeAssociated<const N: usize> {
    type Item;
}

#[test]
fn explicit_shape_slots_still_rewrite_const_expressions() {
    let value: <Arr<2> as ShapeAssociated<2>>::Item = [9u8; 3];
    assert_eq!(value, [9; 3]);
}

macro_rules! six {
    (#token) => {
        6
    };
}

trait ImplHash {}

#[batch_impl(@Self: Arr<{ six!(#token) }>)]
impl ImplHash for u8 {}

#[test]
fn impl_entry_preserves_macro_hashes_in_const_blocks() {
    fn check<T: ImplHash>() {}
    check::<Arr<6>>();
}

macro_rules! opaque_type {
    (#token) => {
        u16
    };
}

trait ImplMacroHashes {}

#[batch_impl(@Self: (opaque_type!(#token), opaque_type![#token], opaque_type! { #token }))]
impl ImplMacroHashes for u8 {}

#[test]
fn impl_entry_preserves_hashes_in_all_type_macro_delimiters() {
    fn check<T: ImplMacroHashes>() {}
    check::<(u16, u16, u16)>();
}

#[batch_impl(
    Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>
    impl{Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>}
)]
trait RangePatternTemplate {}

#[test]
fn trait_shape_templates_preserve_open_range_patterns() {
    fn check<T: RangePatternTemplate>() {}
    check::<Arr<2>>();
}

trait ImplRangePatternTemplate {}

// Keep both concrete shapes textually identical: this tests pattern preservation,
// independently of the existing shape comparison's trailing-comma sensitivity.
#[rustfmt::skip]
#[batch_impl(@Self: Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>)]
impl ImplRangePatternTemplate for Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }> {}

#[test]
fn impl_shape_templates_preserve_open_range_patterns() {
    fn check<T: ImplRangePatternTemplate>() {}
    check::<Arr<2>>();
}
