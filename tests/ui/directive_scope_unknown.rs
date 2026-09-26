// Every selected/excluded name is checked before subtraction, even if the
// final selection is empty. Rename targets are checked by Rust, not by the
// trait-item lookup, but no prefix tokens may disappear during parsing.
use batch_impl::batch_impl;

#[batch_impl(usize #fill(typo, -typo){unused})]
trait RemovedUnknown {
    fn known(&self) {}
}

#[batch_impl(usize #fill(@all, -typo){unused})]
trait UnknownExclusion {
    fn known(&self) {}
}

#[batch_impl(usize #fill([], -[typo,]){unused})]
trait UnknownEmptyExclusion {
    fn known(&self) {}
}

#[batch_impl(usize #fill([known, -[typo,],]){unused})]
trait NestedUnknownExclusion {
    fn known(&self) {}
}

#[batch_impl(usize #delegate(typo=target, -typo,){unused})]
trait RemovedUnknownRename {
    fn known(&self) {}
}

#[batch_impl(usize #delegate(typo known=target, -typo,){unused})]
trait UnknownBeforeRename {
    fn known(&self) {}
}

#[batch_impl(usize #delegate([], -typo,){unused})]
trait UnknownDelegateExclusion {
    fn known(&self) {}
}

#[batch_impl(#blanket(@all, -typo,){Box})]
trait UnknownBlanketExclusion {
    fn known(&self) {}
}

#[batch_impl(usize #delegate(7 known=target){unused})]
trait UnexpectedBeforeRename {
    fn known(&self) {}
}

#[batch_impl(usize #delegate(known=target extra){unused})]
trait UnexpectedAfterRename {
    fn known(&self) {}
}

#[batch_impl(usize #delegate(-known=target){unused})]
trait ExcludedRename {
    fn known(&self) {}
}

fn main() {}
