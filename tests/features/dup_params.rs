//! Same-name generic declarations from chained `<>` blocks merge: a duplicate
//! name collapses into one **bare** declaration and every bound of that name
//! moves into a where predicate (`impl<T> ... where T: Clone, T: Copy`) —
//! duplicate `T` declarations (E0415) are never emitted. Single declarations
//! keep their bound in the impl generics.

// The `constant` regression case below needs a param whose name shares the
// `const` prefix but is a type param — the lowercase spelling is the point of
// the test, so the camel-case lint is allowed for this file only.
#![allow(non_camel_case_types)]

use batch_impl::batch_impl;

struct Pair<T>(T, T);

/// A tuple wrapper with an accessible field (`Box`'s field is private, so the
/// move-out-of-a-reference in the `constant` body below needs a local struct).
struct Holder<T>(T);

// two same-name declarations, each with a bound -> merged + where
#[batch_impl(<T: Clone> <T: Copy> Box<T> { fn touch(&self) {} })]
trait DupBounds {
    fn touch(&self);
}

// three declarations, bounds on one name
#[batch_impl(<T: Clone> <T> <T: Copy> Vec<T> { fn touch(&self) {} })]
trait DupThree {
    fn touch(&self);
}

// duplicate bare name (no bounds) is just deduplicated
#[batch_impl(<U> <U> Option<U> { fn touch(&self) {} })]
trait DupBare {
    fn touch(&self);
}

// single declaration keeps `impl<T: Bound>` form (unchanged)
#[batch_impl(<T: Clone> Pair<T> { fn touch(&self) {} })]
trait SingleBound {
    fn touch(&self);
}

// interleaved with const params and other names
#[batch_impl(<T: Clone> <const N: usize> <T: Copy> [T; N] { fn touch(&self) {} })]
trait DupWithConst {
    fn touch(&self);
}

// A type param merely *named* `constant` is an ordinary type param, not a
// const param: its duplicate bounds must both survive (the const branch keeps
// the first declaration but drops every later duplicate bound). The body makes
// both bounds load-bearing — `require_clone` needs `Clone`, moving `self.0` out
// of a shared reference needs `Copy` — so dropping either bound fails to
// compile, which is what locks the shape-based detection.
#[batch_impl(
    <constant: Clone> <constant: Copy> NamedDup<Val=constant> Holder<constant> {
        fn moved(&self) -> (constant, constant) {
            fn require_clone<T: Clone>(_: &T) {}
            require_clone(&self.0);
            (self.0, self.0)
        }
    }
)]
trait NamedDup {
    type Val;
    fn moved(&self) -> (Self::Val, Self::Val);
}

#[test]
fn constant_named_param_keeps_both_bounds() {
    assert_eq!(Holder(7u8).moved(), (7u8, 7u8));
}

#[test]
fn same_name_decls_merge() {
    Box::new(0u8).touch();
    vec![1u16, 2].touch();
    Some(0u32).touch();
    Pair(0u64, 1u64).touch();
    [0i8; 4].touch();
}
