//! Wrapper target detection and predicates preserve opaque Rust token groups.

// The explicit braces are the boundary under test.
#![allow(unused_braces)]

use batch_impl::batch_impl;

#[test]
fn blanket_preserves_const_patterns_and_appends_after_opaque_markers() {
    struct Wrap<const N: usize, T>(T);
    impl<const N: usize, T> std::ops::Deref for Wrap<N, T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.0
        }
    }

    // The `@0` belongs to stringify!, not the wrapper's target position.
    // The const range pattern likewise belongs to Rust, not fresh syntax.
    #[batch_impl(#blanket(@all_methods){
        Wrap<{ let _ = stringify!(@0); 3 }> where {
            [u8; { match 2 { n @ ..=3 => n, _ => 0 } }]: Sized,
            @0: @trait
        }
    })]
    trait Read {
        fn read(&self) -> u8;
    }

    impl Read for u8 {
        fn read(&self) -> u8 {
            *self
        }
    }

    assert_eq!(Read::read(&Wrap::<3, _>(7u8)), 7);
}
