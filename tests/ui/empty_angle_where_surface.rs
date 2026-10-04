//! An empty `<>` on an ident that is not the annotated trait, met on the **where-predicate**
//! token path (no switch template is needed - `where` is synced unconditionally).
//!
//! The Ty-level route reports this (see `empty_angle_on_other_ident`); the token route used to
//! *delete* the brackets instead, so `where{Vec<>: Clone}` reached rustc as a bare `Vec` and the
//! only complaint was E0107 aimed at nothing. Probe D's F2 measured that drop, probe E's E-A the
//! same rule in the model.
use batch_impl::batch_impl;

#[batch_impl(u8 where{Vec<>: Clone})]
trait WhereSurface {}

fn main() {}
