use batch_impl::batch_impl;

#[batch_impl(
    (Vec<u8>, Vec<u16>, Box<u32>, Vec<u64>) impl{(A<T>@.., B<U>@..,)}
    #n { @T(core::mem::size_of::<@U>())+.. }
)]
trait DriverConflict {
    fn n() -> usize;
}

fn main() {}
