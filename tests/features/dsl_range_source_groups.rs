//! Each range generator keeps its source scope across independent choices.

use batch_impl::{batch_impl, batch_impl_only};

#[batch_impl(
    <F: Fn()1..3, G: Fn()1..3> CallPair<F, G> ((@0_0..), (@1_0..))
    #tag { 17 }
)]
trait CallPair<F, G> {
    fn tag() -> usize;
}

#[test]
fn source_ranges_keep_all_four_function_signatures() {
    assert_eq!(<((u8,), (u16,)) as CallPair<fn(u8), fn(u16)>>::tag(), 17);
    assert_eq!(<((u8,), (u16, u32)) as CallPair<fn(u8), fn(u16, u32)>>::tag(), 17);
    assert_eq!(<((u8, u64), (u16,)) as CallPair<fn(u8, u64), fn(u16)>>::tag(), 17);
    assert_eq!(<((u8, u64), (u16, u32)) as CallPair<fn(u8, u64), fn(u16, u32)>>::tag(), 17);
}

#[batch_impl(
    <F: Fn()0..2, G: Fn()0..2> EmptyPair<F, G> ((@0_0..), (@1_0..))
    where { @0_0..: Clone, @1_0..: Copy }
    #tag { 19 }
)]
trait EmptyPair<F, G> {
    fn tag() -> usize;
}

#[test]
fn zero_length_branches_keep_their_group_scopes() {
    assert_eq!(<((), ()) as EmptyPair<fn(), fn()>>::tag(), 19);
    assert_eq!(<((), (u16,)) as EmptyPair<fn(), fn(u16)>>::tag(), 19);
    assert_eq!(<((String,), ()) as EmptyPair<fn(String), fn()>>::tag(), 19);
    assert_eq!(<((String,), (u16,)) as EmptyPair<fn(String), fn(u16)>>::tag(), 19);
}

#[batch_impl(
    (()1, ()0..=2, ()1) where { @0_0: Clone, @1_0..: Copy, @2_0: core::fmt::Debug }
    #tag { 23 }
)]
trait FixedAroundRange {
    fn tag() -> usize;
}

#[batch_impl(((().1,)2, ().1) #tag { 29 })]
trait CopiedSource {
    fn tag() -> usize;
}

#[test]
fn fixed_generators_and_copied_values_keep_their_identities() {
    assert_eq!(<((String,), (), (u16,)) as FixedAroundRange>::tag(), 23);
    assert_eq!(<((String,), (u8,), (u16,)) as FixedAroundRange>::tag(), 23);
    assert_eq!(<((String,), (u8, u32), (u16,)) as FixedAroundRange>::tag(), 23);
    assert_eq!(<(((String,), (String,)), (u8,)) as CopiedSource>::tag(), 29);
}

macro_rules! append_after_scope {
    ({$($spec:tt)*} ($method:ident) {$value:expr} $trait:item) => {
        #[batch_impl_only((($($spec)*), ().1) where { @0_0..: Clone, @1_0: Copy } #$method { $value })]
        $trait
    };
}

#[batch_impl(()0..2 #append_after_scope(tag) { 31 })]
trait CarriedScope {
    fn tag() -> usize;
}

macro_rules! relay_scope {
    ({$($spec:tt)*} ($method:ident) {$value:expr} $trait:item) => {
        #[batch_impl_only($($spec)* #$method { $value })]
        $trait
    };
}

#[batch_impl(()0..2 where { @0_0..: Clone } #relay_scope(tag) { 37 })]
trait CarriedPredicate {
    fn tag() -> usize;
}

#[test]
fn extension_roundtrip_preserves_empty_scopes_before_new_generators() {
    assert_eq!(<((), (u8,)) as CarriedScope>::tag(), 31);
    assert_eq!(<((String,), (u16,)) as CarriedScope>::tag(), 31);
    assert_eq!(<() as CarriedPredicate>::tag(), 37);
    assert_eq!(<(String,) as CarriedPredicate>::tag(), 37);
}
