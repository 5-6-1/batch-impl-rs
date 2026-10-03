//! An integer that does not fit `usize` is still an integer: it must not be
//! blamed on "float/string/char literals are not types", which sends the reader
//! to the wrong fix. A number in a type position is an arity or a `.N` length.
use batch_impl::batch_impl;

#[batch_impl((u8,).18446744073709551616)]
trait Big {
    fn big(&self) -> u8;
}

fn main() {}
