//! Explicit impl-entry specs preserve mapping and attachment boundaries.

use batch_impl::batch_impl;

struct Wrap<T>(T);

trait Build {
    type Item;
    fn build(value: Self::Item) -> Self;
}

#[batch_impl(Target: Wrap<u8> impl{Wrap<Element>} where{Element: Copy})]
impl Build for Target {
    type Item = Element;

    fn build(value: Element) -> Self {
        Wrap(value)
    }
}

#[test]
fn final_leaf_attachment_maps_associated_types_signatures_and_bodies() {
    let value: Wrap<u8> = Build::build(7);
    assert_eq!(value.0, 7);
}

macro_rules! size {
    (impl{anything}) => {
        3
    };
}

struct Arr<const N: usize>;

trait Width {
    fn width() -> usize;
}

#[batch_impl(Target: Arr<{size!(impl{anything})}>)]
impl Width for Target {
    fn width() -> usize {
        3
    }
}

#[test]
fn rust_const_and_macro_tokens_are_not_nested_attachments() {
    assert_eq!(<Arr<3> as Width>::width(), 3);
}
