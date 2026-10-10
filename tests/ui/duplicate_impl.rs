//! All public entry points reject definitely identical trait impl heads.
use batch_impl::{batch_impl, batch_impl_only, batch_preview, batch_trait};

#[batch_impl([u8, u8])]
trait RepeatedCandidates {}

#[batch_impl(<A> Vec<A>, <B> Vec<B>)]
trait RenamedTypes {}

#[batch_impl(
    <'a, const N: usize> (&'a [u8; N],),
    <'b, const M: usize> (&'b [u8; M],)
)]
trait RenamedLifetimesAndConsts {}

#[batch_impl(u8 #value { 1 }, u8 #value { 2 })]
trait DifferentBodies {
    fn value() -> u8;
}

trait Only {}

#[batch_impl_only(<A> Vec<A>, <B> Vec<B>)]
trait Only {}

trait ImplEntry {}

#[batch_impl(Target: <A> Vec<A>; Target: <B> Vec<B>)]
impl ImplEntry for Target {}

trait AcrossSegments {}

batch_trait! {
    AcrossSegments: <A> Vec<A>;
    AcrossSegments: <B> Vec<B>;
}

batch_preview! {
    #[batch_impl(<A> Vec<A>, <B> Vec<B>)]
    trait PreviewTrait {}
}

trait PreviewImpl {}

batch_preview! {
    #[batch_impl(@Self: u8; @Self: u8)]
    impl PreviewImpl for u8 {}
}

fn main() {}
