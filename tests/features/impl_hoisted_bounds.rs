//! Nested declarations retain their parameters and constraints on the impl entry.

use batch_impl::batch_impl;

#[test]
fn a_nested_type_bound_constrains_an_existing_parameter() {
    struct Holder<T>(T);
    trait CloneValue<T> {
        fn cloned(&self) -> T;
    }

    #[batch_impl(X: Holder<(<T: Clone> T)> impl{Holder<Element>})]
    impl<T> CloneValue<T> for X {
        fn cloned(&self) -> Element {
            self.0.clone()
        }
    }

    assert_eq!(Holder(String::from("nested")).cloned(), "nested");
}

#[test]
fn a_nested_type_declaration_can_introduce_a_parameter() {
    struct Holder<T>(T);
    trait CloneValue {
        type Value;
        fn cloned(&self) -> Self::Value;
    }

    #[batch_impl(X: Holder<(<T: Clone> T)>)]
    impl CloneValue for X {
        type Value = T;
        fn cloned(&self) -> Self::Value {
            self.0.clone()
        }
    }

    assert_eq!(Holder(String::from("introduced")).cloned(), "introduced");
}

#[test]
fn new_and_repeated_lifetimes_keep_their_outlives_bounds() {
    struct NewBorrow<'a>(&'a str);
    struct RepeatedBorrow<'a>(&'a str);
    trait StaticText {
        fn static_text(&self) -> &'static str;
    }

    #[batch_impl(X: (<'a: 'static> NewBorrow<'a>))]
    impl StaticText for X {
        fn static_text(&self) -> &'static str {
            self.0
        }
    }

    #[batch_impl(X: (<'a: 'static> RepeatedBorrow<'a>))]
    impl<'a> StaticText for X {
        fn static_text(&self) -> &'static str {
            self.0
        }
    }

    assert_eq!(NewBorrow("new lifetime").static_text(), "new lifetime");
    assert_eq!(RepeatedBorrow("same lifetime").static_text(), "same lifetime");

    struct MixedBorrow<'a, T, U>(&'a str, T, U);
    #[batch_impl(X: MixedBorrow<'a, (<T: Clone> T), ().1>)]
    impl<'a: 'static> StaticText for X {
        fn static_text(&self) -> &'static str {
            let _: T = self.1.clone();
            self.0
        }
    }
    assert_eq!(
        MixedBorrow("lifetime first", String::new(), (1u8,)).static_text(),
        "lifetime first"
    );
}

#[test]
fn new_and_repeated_const_parameters_keep_their_types() {
    trait NewLength {
        fn new_length(&self) -> usize;
    }
    trait RepeatedLength {
        fn repeated_length(&self) -> usize;
    }

    #[batch_impl(X: (<const N: usize> [u8; N]))]
    impl NewLength for X {
        fn new_length(&self) -> usize {
            N
        }
    }

    #[batch_impl(X: (<const N: usize> [u8; N]))]
    impl<const N: usize> RepeatedLength for X {
        fn repeated_length(&self) -> usize {
            N
        }
    }

    assert_eq!([1u8, 2, 3].new_length(), 3);
    assert_eq!([1u8, 2, 3, 4].repeated_length(), 4);
    assert_eq!([0u8; 0].new_length(), 0);
}

#[test]
fn outer_original_and_nested_declarations_merge_without_losing_bounds() {
    struct Holder<T>(T);
    trait Describe {
        fn describe(&self) -> String;
    }

    #[batch_impl(X: <T: AsRef<str>,> Holder<(<T: core::fmt::Display> T)>)]
    impl<T: Clone> Describe for X {
        fn describe(&self) -> String {
            format!("{}:{}", self.0.clone(), self.0.as_ref())
        }
    }

    assert_eq!(Holder(String::from("all three")).describe(), "all three:all three");
}

#[test]
fn ordinary_declarations_and_fresh_groups_share_collision_safe_names() {
    struct Mixed<A, B, C>(A, B, C);
    trait Sizes {
        fn sizes(&self) -> [usize; 3];
    }

    #[batch_impl(X: Mixed<(<P0: Clone> P0), ()0..2, ()1..3>)]
    impl<P0> Sizes for X {
        fn sizes(&self) -> [usize; 3] {
            let owned = self.0.clone();
            [
                core::mem::size_of_val(&owned),
                core::mem::size_of::<fresh!((@{0_0..},))>(),
                core::mem::size_of::<fresh!((@{1_0..},))>(),
            ]
        }
    }

    assert_eq!(Mixed(5u16, (), (1u8,)).sizes(), [2, 0, 1]);
    assert_eq!(Mixed(5u16, (), (1u8, 2u8)).sizes(), [2, 0, 2]);
    assert_eq!(Mixed(5u16, (1u8,), (2u8,)).sizes(), [2, 1, 1]);
    assert_eq!(Mixed(5u16, (1u8,), (2u8, 3u8)).sizes(), [2, 1, 2]);
}

#[test]
fn hoisted_bounds_keep_rust_const_and_macro_tokens_opaque() {
    macro_rules! size {
        (impl{anything}) => {
            3
        };
    }
    trait Bound<const N: usize> {
        fn value(&self) -> usize {
            N
        }
    }
    impl Bound<3> for u8 {}
    struct Holder<T>(T);
    trait Read {
        fn read(&self) -> usize;
    }

    #[batch_impl(X: Holder<<T: Bound<{size!(impl{anything})}>> T>)]
    impl<T> Read for X {
        fn read(&self) -> usize {
            self.0.value()
        }
    }

    assert_eq!(Holder(1u8).read(), 3);
}

#[test]
fn explicit_fresh_declarations_share_the_generated_parameters() {
    struct Single<T>(T);
    struct Many<T>(T);
    struct EmptyOrOne<T>(T);
    fn copy_inner<T: Clone>(value: &EmptyOrOne<T>) -> EmptyOrOne<T> {
        EmptyOrOne(value.0.clone())
    }
    trait CopyValue {
        fn copy_value(&self) -> Self;
    }

    #[batch_impl(X: Single<<@0: Clone> ().1>)]
    impl CopyValue for X {
        fn copy_value(&self) -> Self {
            Single(self.0.clone())
        }
    }

    #[batch_impl(X: Many<<@0..: Clone> ().2>)]
    impl CopyValue for X {
        fn copy_value(&self) -> Self {
            Many(self.0.clone())
        }
    }

    #[batch_impl(X: EmptyOrOne<<@0_0..: Clone> ()0..2>)]
    impl CopyValue for X {
        fn copy_value(&self) -> Self {
            copy_inner(self)
        }
    }

    assert_eq!(Single((String::from("single"),)).copy_value().0.0, "single");
    assert_eq!(Many((String::from("many"), 7u8)).copy_value().0, (String::from("many"), 7));
    let _: EmptyOrOne<()> = EmptyOrOne(()).copy_value();
    assert_eq!(EmptyOrOne((String::from("group"),)).copy_value().0.0, "group");
}
