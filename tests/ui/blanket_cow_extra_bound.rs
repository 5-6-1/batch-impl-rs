use batch_impl::batch_impl;
use std::borrow::Cow;

trait Extra {}

#[batch_impl(#blanket(@all_methods){@Cow where { @0: Extra }})]
trait Read {
    fn read(&self) -> usize;
}

impl Read for str {
    fn read(&self) -> usize { self.len() }
}
impl Read for String {
    fn read(&self) -> usize { self.len() }
}

fn main() {
    Read::read(&Cow::Borrowed("text"));
}
