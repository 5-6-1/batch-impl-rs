//! Generated delegate parameters avoid user bindings and later substitutions.

use batch_impl::batch_impl;

struct Inner;

impl Inner {
    fn numbers(&self, a: u8, b: u8, c: u8, d: u8, e: u8) -> [u8; 5] {
        [a, b, c, d, e]
    }

    fn join(&self, first: String, second: String) -> String {
        format!("{first}:{second}")
    }

    fn add(&self, first: u8, second: u8) -> u8 {
        first + second
    }

    fn pairs(&self, first: u8, second: (u8, u8), third: (u8, u8)) -> u8 {
        first + second.0 + second.1 + third.0 + third.1
    }
}

struct PlainOuter(Inner);

#[batch_impl(PlainOuter #delegate(numbers){self.0})]
trait PlainForward {
    fn numbers(&self, _: u8, r#arg0: u8, _: u8, arg1: u8, arg0_: u8) -> [u8; 5];
}

#[test]
fn delegate_names_reserve_later_raw_and_suffixed_parameters() {
    assert_eq!(PlainOuter(Inner).numbers(1, 2, 3, 4, 5), [1, 2, 3, 4, 5]);
}

struct TemplateOuter(Inner);

#[batch_impl(
    TemplateOuter #delegate(combine=join){
        let arg0 = &self.0;
        arg0.#call
    }
)]
trait TemplateForward {
    fn combine(&self, _: String, arg0: String) -> String;
}

#[test]
fn delegate_template_forwards_anonymous_and_shadowed_named_parameters() {
    assert_eq!(TemplateOuter(Inner).combine("left".into(), "right".into()), "left:right");
}

fn __batch_impl_arg0() -> u8 {
    7
}

struct BodyOuter(Inner);

#[batch_impl(BodyOuter #delegate(add){self.0.#call + r#__batch_impl_arg0()})]
trait BodyForward {
    fn add(&self, _: u8, _: u8) -> u8;
}

#[test]
fn delegate_names_do_not_capture_identifiers_used_only_in_the_body() {
    assert_eq!(BodyOuter(Inner).add(2, 3), 12);
}

struct PatternOuter(Inner);

#[batch_impl(PatternOuter #delegate(pairs){self.0})]
trait PatternForward {
    fn pairs(&self, _: u8, (arg0, arg1): (u8, u8), (ref arg2, _): (u8, u8)) -> u8 {
        arg0 + arg1 + *arg2
    }
}

#[test]
fn delegate_names_reserve_bindings_inside_forwardable_and_replaced_patterns() {
    assert_eq!(PatternOuter(Inner).pairs(1, (2, 3), (4, 5)), 15);
}

struct ShapeOuter<T>(T);

#[batch_impl(
    ShapeOuter<Inner> impl{ShapeOuter<__batch_impl_arg0>} #delegate(add){self.0}
)]
trait ShapeForward {
    fn add(&self, _: u8, _: u8) -> u8;
}

#[test]
fn delegate_names_avoid_shape_slots_outside_the_directive() {
    assert_eq!(ShapeOuter(Inner).add(2, 3), 5);
}

#[batch_impl(#blanket(@all_methods){Box})]
trait BlanketForward {
    fn forward(&self, (r#arg0, arg1): (u8, u8), __batch_impl_delegate_args: u8) -> u8 {
        arg0 + arg1 + __batch_impl_delegate_args
    }
}

impl BlanketForward for Inner {}

#[test]
fn blanket_keeps_named_and_structured_parameters_on_the_shared_forwarding_path() {
    assert_eq!(Box::new(Inner).forward((2, 3), 7), 12);
}

struct PrivateOuter(Inner);

#[batch_impl(PrivateOuter #delegate(numbers){self.0})]
trait PrivateForward {
    fn numbers(
        &self, _: u8, r#__batch_impl_arg0: u8, _: u8, __batch_impl_arg1: u8, __batch_impl_arg0_: u8,
    ) -> [u8; 5];
}

#[test]
fn private_parameter_names_still_reserve_later_raw_and_suffixed_bindings() {
    assert_eq!(PrivateOuter(Inner).numbers(1, 2, 3, 4, 5), [1, 2, 3, 4, 5]);
}

mod surrounding_unit_struct {
    use super::Inner;
    use batch_impl::batch_impl;

    #[allow(non_camel_case_types)]
    struct arg0;
    struct Outer(Inner);

    #[batch_impl(Outer #delegate(add){self.0})]
    trait Forward {
        fn add(&self, _: u8, _: u8) -> u8;
    }

    #[test]
    fn ordinary_outer_unit_struct_does_not_become_a_parameter_pattern() {
        let _ = arg0;
        assert_eq!(Outer(Inner).add(2, 3), 5);
    }
}

mod surrounding_variant {
    use super::Inner;
    use batch_impl::batch_impl;

    enum Marker {
        #[allow(non_camel_case_types)]
        arg0,
    }
    use Marker::arg0;
    struct Outer(Inner);

    #[batch_impl(Outer #delegate(add){self.0})]
    trait Forward {
        fn add(&self, _: u8, _: u8) -> u8;
    }

    #[test]
    fn ordinary_outer_unit_variant_does_not_become_a_parameter_pattern() {
        let _ = arg0;
        assert_eq!(Outer(Inner).add(2, 3), 5);
    }
}

mod surrounding_constant {
    use super::Inner;
    use batch_impl::batch_impl;

    #[allow(non_upper_case_globals)]
    const arg0: u8 = 7;
    macro_rules! original {
        () => {
            arg0
        };
    }
    struct Outer(Inner);

    #[batch_impl(Outer #delegate(add){self.0.#call + original!()})]
    trait Forward {
        fn add(&self, _: u8, _: u8) -> u8;
    }

    #[test]
    fn ordinary_outer_constant_and_opaque_macro_keep_their_meaning() {
        assert_eq!(Outer(Inner).add(2, 3), 12);
    }
}
