//! Trait associated-item declarations adapted into impl definitions and GAT
//! applications by `#name`, `#fill`, and `#blanket`.

use batch_impl::batch_impl;

#[batch_impl(u8 #Output{u16})]
trait NamedBounded {
    type Output: Clone;
}

#[batch_impl(u16 #fill(@all_types){String})]
trait FilledBounded {
    type First: Clone;
    type Second: Default;
}

#[test]
fn single_and_fill_keep_associated_type_obligations() {
    let output: <u8 as NamedBounded>::Output = 7;
    assert_eq!(output, 7);
    let first: <u16 as FilledBounded>::First = String::from("first");
    let second = <u16 as FilledBounded>::Second::default();
    assert_eq!(first.clone(), "first");
    assert!(second.is_empty());
}

#[batch_impl(u8 #Item{(&'a T, [T; N])})]
trait NamedGat {
    type Item<'a, T: Clone, const N: usize>: Clone
    where
        Self: 'a,
        T: 'a;
}

#[test]
fn single_gat_preserves_parameters_and_where() {
    let value = String::from("named");
    let item: <u8 as NamedGat>::Item<'_, String, 2> = (&value, [value.clone(), value.clone()]);
    assert_eq!(item.clone().1, ["named", "named"]);
    assert_eq!(item.0, "named");
}

#[batch_impl(u16 #fill(@all_types){(&'a T, [T; N])})]
trait FilledGat {
    type First<'a, T: Clone, const N: usize>: Clone
    where
        Self: 'a,
        T: 'a;
    type Second<'a, T: Clone, const N: usize>: Clone
    where
        Self: 'a,
        T: 'a;
}

#[test]
fn fill_gat_preserves_each_declaration() {
    let value = String::from("filled");
    let first: <u16 as FilledGat>::First<'_, String, 1> = (&value, [value.clone()]);
    let second: <u16 as FilledGat>::Second<'_, String, 2> =
        (&value, [value.clone(), value.clone()]);
    assert_eq!(first.clone().1, ["filled"]);
    assert_eq!(second.clone().1, ["filled", "filled"]);
}

#[batch_impl(#blanket(@all){Box})]
trait BlanketGat {
    type Part<'a, 'b: 'a, T: Clone + 'b, const N: usize>: Clone
    where
        Self: 'b;
}

impl BlanketGat for u8 {
    type Part<'a, 'b: 'a, T: Clone + 'b, const N: usize>
        = (&'a T, &'b T, [T; N])
    where
        Self: 'b;
}

#[test]
fn blanket_gat_applies_names_without_declarations() {
    let value = String::from("blanket");
    let part: <Box<u8> as BlanketGat>::Part<'_, '_, String, 2> =
        (&value, &value, [value.clone(), value.clone()]);
    assert_eq!(part.clone().2, ["blanket", "blanket"]);
    assert_eq!(part.0, part.1);
}

// Trait and GAT applications share the names-only conversion. A lifetime
// bound belongs on the impl declaration, never in `Trait<'a, 'b>`.
#[batch_impl(#blanket(@all){Box})]
trait LifetimeBounded<'a, 'b: 'a> {
    type Output: Clone;
    fn shorten(&self, value: &'b u8) -> &'a u8;
}

impl<'a, 'b: 'a> LifetimeBounded<'a, 'b> for u8 {
    type Output = u16;

    fn shorten(&self, value: &'b u8) -> &'a u8 {
        value
    }
}

#[test]
fn blanket_trait_applies_bounded_lifetime_names() {
    let value = 9;
    let wrapper = Box::new(0u8);
    assert_eq!(*wrapper.shorten(&value), 9);
    let output: <Box<u8> as LifetimeBounded<'_, '_>>::Output = 12;
    assert_eq!(output, 12);
}
