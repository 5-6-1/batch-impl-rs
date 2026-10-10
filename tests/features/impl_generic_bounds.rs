//! Repeated impl-entry generic declarations retain their bounds.

use batch_impl::batch_impl;
use std::marker::PhantomData;

struct BorrowView<'a, 'b>(&'a u8, PhantomData<&'b ()>);

trait Reborrow<'b> {
    fn reborrow(&self) -> &'b u8;
}

#[batch_impl(@Self: <'a: 'b> @Self)]
impl<'a, 'b> Reborrow<'b> for BorrowView<'a, 'b> {
    fn reborrow(&self) -> &'b u8 {
        self.0
    }
}

#[test]
fn duplicate_lifetime_bounds_reach_the_generated_impl() {
    let value = 7;
    let view = BorrowView(&value, PhantomData);
    assert_eq!(*view.reborrow(), 7);
}
