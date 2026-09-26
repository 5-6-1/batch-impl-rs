//! `@Self` copies the current input impl's type through the constant stage.
//! These tests distinguish input-type substitution from Rust's output `Self`.

use batch_impl::batch_impl;
use std::rc::Rc;

trait Make {
    fn make() -> Self;
}

#[batch_impl(@Self: [Box, Rc] @u8..=u16)]
impl Make for Box<u8> {
    fn make() -> Self {
        Box::new(u8::MAX)
    }
}

#[test]
fn input_type_is_a_full_shape_template() {
    assert_eq!(*<Box<u8> as Make>::make(), u8::MAX);
    assert_eq!(*<Rc<u8> as Make>::make(), u8::MAX);
    assert_eq!(*<Box<u16> as Make>::make(), u16::MAX);
    assert_eq!(*<Rc<u16> as Make>::make(), u16::MAX);
}

trait DefaultMake {
    fn default_make() -> Self;
}

#[batch_impl(@Self: [Box, Rc] @u8..=u16)]
impl<T: Default> DefaultMake for Box<T> {
    fn default_make() -> Self {
        Box::new(T::default())
    }
}

#[test]
fn input_generics_follow_the_existing_shape_mapping() {
    assert_eq!(*<Box<u8> as DefaultMake>::default_make(), 0);
    assert_eq!(*<Rc<u16> as DefaultMake>::default_make(), 0);
}

trait Nested {
    fn identity(self) -> Self;
}

// The predicate constrains the source u8, while Rust Self in the method
// refers to the generated Vec<u8>. Vec<u8> deliberately does not implement Copy.
#[batch_impl(Vec<@Self> where { @Self: Copy })]
impl Nested for u8 {
    fn identity(self) -> Self {
        self
    }
}

#[test]
fn direct_type_and_where_read_the_input_type() {
    assert_eq!(vec![1u8, 2, 3].identity(), vec![1u8, 2, 3]);
}

trait Layered {
    fn output_name() -> &'static str;
}

#[batch_impl(Vec<@Self>)]
#[batch_impl(Box<@Self>)]
impl Layered for u8 {
    fn output_name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn each_stacked_attribute_reads_its_own_input() {
    assert_eq!(<Box<Vec<u8>> as Layered>::output_name(), std::any::type_name::<Box<Vec<u8>>>(),);
}

trait Sibling {
    fn bytes(&self) -> usize;
}

#[batch_impl(Vec<@Self>; Box<@Self>)]
impl Sibling for u16 {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[test]
fn sibling_specs_share_the_same_input() {
    assert_eq!(vec![3u16].bytes(), std::mem::size_of::<Vec<u16>>());
    assert_eq!(Box::new(3u16).bytes(), std::mem::size_of::<Box<u16>>());
}

struct Borrowed<'a, const N: usize>(&'a [u8; N]);

#[batch_impl(@Self)]
impl<'a, const N: usize> Borrowed<'a, N> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[test]
fn inherent_input_preserves_lifetimes_and_const_arguments() {
    assert_eq!(Borrowed(&[1, 2, 3]).len(), 3);
}

trait Listed {
    fn listed() -> Self;
}

#[batch_impl(@Self: [@Self, Rc<u8>])]
impl Listed for Box<u8> {
    fn listed() -> Self {
        Box::new(7)
    }
}

#[test]
fn input_type_can_be_a_matrix_element() {
    assert_eq!(*<Box<u8> as Listed>::listed(), 7);
    assert_eq!(*<Rc<u8> as Listed>::listed(), 7);
}

trait Attached {
    fn attached() -> Self;
}

// Both templates bind the same prototype. The constant in where becomes
// Box<T> first and then follows the ordinary Box -> Rc, T -> u16 mapping.
#[batch_impl(Box<T>: Rc<u16> impl{@Self} where { @Self: Clone })]
impl<T> Attached for Box<T> {
    fn attached() -> Self {
        Box::new(T::MAX)
    }
}

#[test]
fn templates_and_predicates_use_the_same_constant_stage() {
    assert_eq!(*<Rc<u16> as Attached>::attached(), u16::MAX);
}

macro_rules! body_marker {
    (@Self) => {
        "preserved"
    };
}

trait BodyBoundary {
    fn marker() -> &'static str;
}

#[batch_impl(@Self: u16)]
impl BodyBoundary for u8 {
    fn marker() -> &'static str {
        match body_marker!(@Self) {
            marker @ "preserved" => marker,
            _ => "changed",
        }
    }
}

#[test]
fn rust_body_patterns_and_macro_tokens_stay_untouched() {
    assert_eq!(<u16 as BodyBoundary>::marker(), "preserved");
}
