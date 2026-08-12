use batch_impl::batch_impl;

// An open-extension name within edit distance 2 of a built-in directive is
// treated as a typo and given a suggestion.
#[batch_impl(u8 #delgate{})]
trait TypoT {}

fn main() {}
