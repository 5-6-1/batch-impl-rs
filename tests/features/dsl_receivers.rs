//! dsl.rs receiver-kind `@all` filter tests: `@all_ref_methods` /
//! `@all_value_methods` / `@all_static_methods` (incl. typed receivers and
//! marker-minus-marker subtraction).
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

// ============================================================
// Receiver-kind `@all` filters
// ============================================================
#[test]
fn receiver_kind_filters() {
    #[batch_impl(
        u8
        #fill(@all_ref_methods){ 7 }
        #fill(@all_value_methods){ 8 }
        #fill(@all_static_methods){ 9 }
        #C{ 10 }
        #Item{ u8 }
    )]
    trait RecvT {
        fn by_ref(&self) -> u8;
        fn by_mut(&mut self) -> u8;
        fn by_val(self) -> u8;
        fn make() -> u8;
        const C: u8;
        type Item;
    }

    let x = 5u8;
    assert_eq!(RecvT::by_ref(&x), 7);
    let mut y = 5u8;
    assert_eq!(RecvT::by_mut(&mut y), 7);
    assert_eq!(RecvT::by_val(x), 8);
    assert_eq!(<u8 as RecvT>::make(), 9);
    assert_eq!(<u8 as RecvT>::C, 10);
    let _: <u8 as RecvT>::Item = 1u8;
}

// ============================================================
// Reviewer additions: typed-receiver filter + marker-minus-marker
// ============================================================
// `@all_value_methods` includes typed receivers (`self: Box<Self>`,
// `syn::ReceiverKind::Typed`); `@all_static_methods` = no receiver.
#[batch_impl(u8 #fill(@all_value_methods){4} #fill(@all_static_methods){5})]
trait TypedRecv2 {
    fn plain(self) -> u8;
    fn boxed(self: Box<Self>) -> u8;
    fn by_ref(&self) -> u8 {
        7
    }
    fn make() -> u8;
}

// Marker-minus-marker: `@all_methods - @all_value_methods` = ref + static.
// (minus takes a resolved `[...]` list; `-@all_value_methods` expands to it)
#[batch_impl(u16 #fill(@all_methods, -@all_value_methods){6})]
trait MarkerMinus3 {
    fn by_ref(&self) -> u16;
    fn by_val(self) -> u16
    where
        Self: Sized,
    {
        0
    }
    fn make() -> u16;
}

#[test]
fn receiver_filters_review() {
    assert_eq!(TypedRecv2::plain(3u8), 4);
    assert_eq!(TypedRecv2::boxed(Box::new(3u8)), 4);
    assert_eq!(TypedRecv2::by_ref(&3u8), 7); // excluded -> default
    assert_eq!(<u8 as TypedRecv2>::make(), 5);

    assert_eq!(MarkerMinus3::by_ref(&1u16), 6);
    assert_eq!(MarkerMinus3::by_val(1u16), 0); // excluded -> default
    assert_eq!(<u16 as MarkerMinus3>::make(), 6);
}
