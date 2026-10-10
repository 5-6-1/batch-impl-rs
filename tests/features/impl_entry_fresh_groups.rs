//! The impl body resolves the same source groups as its target and predicates.

use batch_impl::batch_impl;

trait EmptyScope {
    fn bytes() -> usize;
}

#[batch_impl(T: ()0..2)]
impl EmptyScope for T {
    fn bytes() -> usize {
        core::mem::size_of::<fresh!((@{0_0..},))>()
    }
}

#[test]
fn a_zero_length_range_group_remains_known_in_the_body() {
    assert_eq!(<() as EmptyScope>::bytes(), 0);
    assert_eq!(<(u16,) as EmptyScope>::bytes(), 2);
}

trait SeparateScopes {
    fn bytes() -> [usize; 4];
}

#[batch_impl(T: (()0..2, ()1..3))]
impl SeparateScopes for T {
    fn bytes() -> [usize; 4] {
        [
            core::mem::size_of::<fresh!((@{0_0..},))>(),
            core::mem::size_of::<fresh!((@{1_0..},))>(),
            core::mem::size_of::<fresh!((@{0..},))>(),
            core::mem::size_of::<(fresh!(@(@All,)..))>(),
        ]
    }
}

#[test]
fn grouped_flat_and_implicit_segment_references_share_one_context() {
    assert_eq!(<((), (u8,)) as SeparateScopes>::bytes(), [0, 1, 1, 1]);
    assert_eq!(<((), (u8, u8)) as SeparateScopes>::bytes(), [0, 2, 2, 2]);
    assert_eq!(<((u8,), (u8,)) as SeparateScopes>::bytes(), [1, 1, 2, 2]);
    assert_eq!(<((u8,), (u8, u8)) as SeparateScopes>::bytes(), [1, 2, 3, 3]);
}
