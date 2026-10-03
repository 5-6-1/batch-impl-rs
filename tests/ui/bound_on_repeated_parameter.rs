//! A bound on a parameter that the power repeats.
//!
//! Public syntax: the tuple power rewrites its template into N fresh parameters, so a bound
//! on the template has nowhere to live. The message used to call this an internal error,
//! which tells a user the macro is broken rather than their spec.
use batch_impl::batch_impl;

#[batch_impl((<T: Clone>,).2)]
trait BoundOnRepeated {}

fn main() {}
