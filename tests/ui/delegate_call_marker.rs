use batch_impl::batch_impl;

struct Receiver;

#[batch_impl(Receiver #delegate(read){ self.#other })]
trait UnknownMarker {
    fn read(&self);
}

#[batch_impl(Receiver #delegate(read){ #call(self) })]
trait PrefixMarker {
    fn read(&self);
}

fn main() {}
