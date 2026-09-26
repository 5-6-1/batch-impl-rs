//! Call markers are local to Rust expression templates. Macro and attribute
//! token inputs stay opaque, and bodies without a marker retain target mode.

use batch_impl::batch_impl;

struct Predicate;
impl Predicate {
    fn test(&self, value: u8) -> bool {
        value > 1
    }
}
struct Negated(Predicate);

#[batch_impl(Negated #delegate(check=test){
    #![allow(unused_labels)]
    if value > 0 { return !(self.0.#call); }
    !{ self.0.#call }
})]
trait Negate {
    fn check(&self, value: u8) -> bool;
}

#[test]
fn unary_not_is_not_a_macro_and_inner_attributes_precede_generated_items() {
    let value = Negated(Predicate);
    assert!(value.check(0));
    assert!(value.check(1));
    assert!(!value.check(2));
}

struct Inner;

impl Inner {
    fn value(&self) -> usize {
        41
    }
}

macro_rules! opaque_receiver {
    ($receiver:expr; untouched . # call) => {
        $receiver
    };
}

struct MacroOnly(Inner);
struct MacroAndCall(Inner);

#[batch_impl(
    MacroOnly #delegate(value){opaque_receiver!(&self.0; untouched.#call)},
    MacroAndCall #delegate(value){opaque_receiver!(&self.0; untouched.#call).#call + 1}
)]
trait MacroBoundary {
    fn value(&self) -> usize;
}

#[test]
fn macro_input_markers_stay_opaque_in_both_delegate_modes() {
    assert_eq!(MacroOnly(Inner).value(), 41);
    assert_eq!(MacroAndCall(Inner).value(), 42);
}

struct AttributeOnly(Inner);

#[batch_impl(AttributeOnly #delegate(value){{
    #[cfg_attr(any(), marker(untouched.#call))]
    let receiver = &self.0;
    receiver
}})]
trait AttributeBoundary {
    // The attributed local binding tests marker opacity inside a target block.
    #[allow(clippy::let_and_return)]
    fn value(&self) -> usize;
}

#[test]
fn an_attribute_marker_does_not_switch_the_body_to_template_mode() {
    assert_eq!(AttributeOnly(Inner).value(), 41);
}

struct MacroDefinition(Inner);

#[batch_impl(MacroDefinition #delegate(value){{
    macro_rules! local_receiver {
        ($receiver:expr; untouched . # call) => { $receiver };
    }
    local_receiver!(&self.0; untouched.#call)
}})]
trait MacroDefinitionBoundary {
    fn value(&self) -> usize;
}

#[test]
fn local_macro_definitions_keep_their_own_token_language() {
    assert_eq!(MacroDefinition(Inner).value(), 41);
}

trait Value {
    fn value(&self) -> usize;
}

impl Value for Inner {
    fn value(&self) -> usize {
        7
    }
}

enum LegacyTarget {
    First(Inner),
    Second(Inner),
}

// The second arm coerces the complete match to &dyn Value. Moving calls
// into the arms would silently select the inherent method in the first arm.
#[batch_impl(LegacyTarget #delegate(value){
    match self {
        Self::First(inner) => inner,
        Self::Second(inner) => inner as &dyn Value,
    }
})]
trait LegacyDispatch {
    fn value(&self) -> usize;
}

#[test]
fn a_match_without_call_markers_keeps_its_original_coercion_and_dispatch() {
    assert_eq!(LegacyTarget::First(Inner).value(), 7);
    assert_eq!(LegacyTarget::Second(Inner).value(), 7);
}

struct Addend;
struct SlotWrapper<T>(T);

impl Addend {
    fn add(&self, value: usize) -> usize {
        value + 4
    }
}

// The preferred private helper name is a shape slot, and its next candidate
// names a visible user macro. Neither may be reused for argument forwarding.
#[batch_impl(
    SlotWrapper<Addend> impl{SlotWrapper<__batch_impl_delegate_args>}
    #delegate(add){
        macro_rules! r#__batch_impl_delegate_args_ {
            () => { 7 };
        }
        self.0.#call + r#__batch_impl_delegate_args_!()
    }
)]
trait SlotNameCollision {
    fn add(&self, value: usize) -> usize;
}

#[test]
fn argument_helpers_avoid_visible_macros_and_later_shape_slot_rewrites() {
    assert_eq!(SlotWrapper(Addend).add(5), 16);
}

struct ConstFunction(Inner);

#[batch_impl(ConstFunction #delegate(value){
    const fn local() -> usize { 1 }
    let result = self.0.#call;
    result + local()
})]
trait ConstFunctionBoundary {
    fn value(&self) -> usize;
}

#[test]
fn a_nested_const_function_does_not_hide_the_following_call_marker() {
    assert_eq!(ConstFunction(Inner).value(), 42);
}
