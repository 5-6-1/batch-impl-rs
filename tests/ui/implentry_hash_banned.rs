use batch_impl::batch_impl;

// `#` directives are banned on the ItemImpl entry.
#[batch_impl(W : u8 #tag{7})]
impl BadHash for W {
    fn tag(&self) -> u32 {
        7
    }
}

trait BadHash {
    fn tag(&self) -> u32;
}

fn main() {}
