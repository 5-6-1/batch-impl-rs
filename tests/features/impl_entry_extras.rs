//! The impl entry (0.9.0) extras: `X<>` sync in the where predicates.
//!
//! The impl block is ordinary Rust (`impl Tr<Additive, Multiplicative> for
//! ...` parses verbatim), so only the where predicates are synced — the
//! `X<>` there fills with the impl's own trait args, closing the gap with
//! the trait-entry sync (arrived in 0.9.0). Variadic segments / repeat
//! blocks are **not** supported on this entry (they are not legal Rust in
//! an impl block).

use batch_impl::batch_impl;

struct Additive;
struct Multiplicative;

// ------------------------------------------------------------
// 1. `X<>` sync in the attr where predicates: `Marker<>` fills with the
//    impl's own trait args (`impl Marked<Additive, Multiplicative> for ...`
//    → `Self: Marker<Additive, Multiplicative>`).
// ------------------------------------------------------------
trait Marker<A, B> {}
impl Marker<Additive, Multiplicative> for Box<u8> {}

trait Marked<A, B> {}

#[batch_impl(Box<u8> : Box<u8> where Self: Marker<>)]
impl Marked<Additive, Multiplicative> for Box<u8> {}

#[test]
fn impl_entry_where_sync() {
    fn check<T: Marked<Additive, Multiplicative>>() {}
    check::<Box<u8>>();
}

// ------------------------------------------------------------
// 2. `X<>` sync with a matrix: every leaf gets the synced where predicate.
// ------------------------------------------------------------
trait Marker2<A, B> {}
impl Marker2<Additive, Multiplicative> for Box<u8> {}
impl Marker2<Additive, Multiplicative> for Box<u16> {}

#[batch_impl(A<B> : Box.[u8, u16] where Self: Marker2<>)]
impl MatrixMarked<Additive, Multiplicative> for A<B> {}

trait MatrixMarked<A, B> {}

#[test]
fn impl_entry_where_sync_matrix() {
    fn check<T: MatrixMarked<Additive, Multiplicative>>() {}
    check::<Box<u8>>();
    check::<Box<u16>>();
}
