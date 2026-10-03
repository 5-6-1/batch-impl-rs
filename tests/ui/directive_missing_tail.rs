//! A directive name whose tail is missing or malformed: four shapes that reach the
//! three unprefixed literals in `directives/dispatch.rs`. Two of those literals carry
//! identical text in different branches, so each branch needs its own trigger:
//!
//!   #fill          -> the bare-name branch
//!   #fill(@all)    -> args, and no group after them
//!   #wrap          -> the bare-name branch (an unknown name)
//!   #wrap(a)[b]    -> args followed by a group that is not `{body}`
//!
//! Before the fix these messages carried no `batch-impl: ` prefix, which is exactly why
//! the diagnostic-lock guard could not see them: it finds candidates with
//! `find("batch-impl: ")`.
use batch_impl::batch_impl;

#[batch_impl(u8 #fill)]
trait FillBare {
    fn f(&self);
}

#[batch_impl(u8 #fill(@all))]
trait FillArgsNoBody {
    fn f(&self);
}

#[batch_impl(usize #wrap)]
trait UnknownBare {}

#[batch_impl(usize #wrap(a)[b])]
trait UnknownWrongGroup {}

fn main() {}
