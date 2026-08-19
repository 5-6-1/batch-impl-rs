use batch_impl::batch_impl;

// The impl's for-Type must match the shape template ident-for-ident — a
// binding during the shape-validity check means the for-Type doesn't carry
// the placeholder slot names.
#[batch_impl(A<B> : [Box].[u8])]
impl WrongShape for Vec<u32> {
    fn mk() -> Vec<u32> {
        Vec::new()
    }
}

trait WrongShape {
    fn mk() -> Self;
}

fn main() {}
