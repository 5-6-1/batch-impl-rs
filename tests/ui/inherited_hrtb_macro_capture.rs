#![allow(unused_macros)]

use batch_impl::batch_impl;

#[allow(unused_macros)]
macro_rules! reference {
    ($lifetime:lifetime) => { &$lifetime u8 };
}
trait Bound<A, B> {}

#[batch_impl(<'a, U> Inherited<'a, U> (&'a (), U))]
trait Inherited<'x, T>
where
    for<'a> T: Bound<reference!('a), &'x u8>,
{
}

fn main() {}
