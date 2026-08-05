// Error: impl parameter renamed (X maps to parameter T), auto-inheritance requires identical names
use batch_impl::batch_impl;

#[batch_impl(<X> A<X> ())]
trait A<T: Clone> {}

fn main() {}
