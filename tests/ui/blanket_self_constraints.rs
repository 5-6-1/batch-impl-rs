use batch_impl::batch_impl;

trait Marker<T: ?Sized> {}

#[batch_impl(#blanket(@all){Box})]
trait InlineBound {
    fn read<U: Marker<Self>>(&self);
}

#[batch_impl(#blanket(@all){Box})]
trait WhereBound {
    fn read<U>(&self)
    where
        U: Marker<Self>;
}

#[batch_impl(#blanket(@all){Box})]
trait SelfSubject {
    fn read<U>(&self)
    where
        Self: Marker<U>;
}

fn main() {}
