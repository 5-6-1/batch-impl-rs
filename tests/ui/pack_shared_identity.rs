//! Copies of one position share a type, while separate positions stay distinct.
use batch_impl::{batch_impl, batch_impl_only};

struct Pair<T, U>(T, U);

#[batch_impl((*Pair (*(self, Vec) *().1),))]
trait SamePosition {}

#[batch_impl(([*Vec, *Box] *().2,))]
trait UniformChoice {}

fn same<T: SamePosition>() {}
fn uniform<T: UniformChoice>() {}

macro_rules! relay {
    ({$($spec:tt)*} () {} $trait:item) => {
        #[batch_impl_only($($spec)*)]
        $trait
    };
}

#[batch_impl((*Pair (*(self, Vec) *().1..=2),) where { @0..: Clone } #relay() {})]
trait ThroughExtension {}

struct NotClone;
fn reentered<T: ThroughExtension>() {}

fn main() {
    same::<(Pair<u8, Vec<u16>>,)>();
    uniform::<(Vec<u8>, Box<u16>)>();
    reentered::<(Pair<u8, Vec<u16>>,)>();
    reentered::<(Pair<NotClone, Vec<NotClone>>,)>();
}
