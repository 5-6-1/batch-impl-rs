//! Same-name generic declarations from chained `<>` blocks merge: a duplicate
//! name collapses into one **bare** declaration and every bound of that name
//! moves into a where predicate (`impl<T> ... where T: Clone, T: Copy`) —
//! duplicate `T` declarations (E0415) are never emitted. Single declarations
//! keep their bound in the impl generics.

use batch_impl::batch_impl;

struct Pair<T>(T, T);

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

#[test]
fn same_name_decls_merge() {
    Box::new(0u8).touch();
    vec![1u16, 2].touch();
    Some(0u32).touch();
    Pair(0u64, 1u64).touch();
    [0i8; 4].touch();
}
