use batch_impl::batch_impl;

// A `=` binding with no value silently dropped the input; it now errors with
// guidance. (A `:` bound with no value is lost earlier in angle-collect and is
// reported by rustc as E0425 "cannot find type `T`" — see dev-changelog.)
#[batch_impl(Conv<Item =>)]
trait Conv<T> {
    type Item;
}

fn main() {}
