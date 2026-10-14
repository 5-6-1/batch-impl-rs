use batch_impl::batch_impl;
use std::borrow::Cow;

#[batch_impl(#blanket(@all_methods){@Cow where { @0::Owned: @trait }})]
trait Read {
    fn read(&self) -> usize;
}

impl Read for str {
    fn read(&self) -> usize {
        self.len()
    }
}

fn main() {
    let value: Cow<'_, str> = Cow::Owned(String::from("text"));
    Read::read(&value);
}
