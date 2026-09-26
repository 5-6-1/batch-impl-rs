// Leading/consecutive commas remain errors, including in nested lists and
// delegate selections containing a rename. A single trailing comma is valid.
use batch_impl::batch_impl;

#[batch_impl(usize #fill(m,,n){0})]
trait FillConsecutive {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

#[batch_impl(usize #fill(,m){0})]
trait FillLeading {
    fn m(&self) -> u32;
}

#[batch_impl(usize #fill([,m]){0})]
trait NestedLeading {
    fn m(&self) -> u32;
}

#[batch_impl(usize #fill(@all, -[m,,]){0})]
trait ExcludedConsecutive {
    fn m(&self) -> u32;
}

#[batch_impl(usize #delegate(,m){self})]
trait DelegateLeading {
    fn m(&self) -> u32;
}

#[batch_impl(usize #delegate(m,,n){self})]
trait DelegateConsecutive {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

#[batch_impl(usize #delegate(,m=target){self})]
trait RenameLeading {
    fn m(&self) -> u32;
}

#[batch_impl(usize #delegate(m=target,,){self})]
trait RenameConsecutive {
    fn m(&self) -> u32;
}

#[batch_impl(#blanket(,m){Box})]
trait BlanketLeading {
    fn m(&self) -> u32;
}

#[batch_impl(#blanket([m,,]){Box})]
trait BlanketNestedConsecutive {
    fn m(&self) -> u32;
}

#[batch_impl(usize #fill(,){unused})]
trait CommaOnlyFill {}

#[batch_impl(usize #delegate(,){unused})]
trait CommaOnlyDelegate {}

fn main() {}
