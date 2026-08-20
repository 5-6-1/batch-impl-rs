//! Ext 2 shape-match coverage for the full range of `syn::Type` forms:
//! slices `[A]`, fixed arrays with literal lengths `[A; 3]`, tuples
//! `(A, B, C)`, references with lifetimes `&'static A`, parenthesized
//! types, nested arrays, multi-segment paths, and verbatim forms that must
//! match themselves exactly (fn pointers / trait objects — the verbatim
//! fallback binds nothing, so only identical templates work there).
//!
//! What CANNOT be bound today (see `tests/ui/impl_*` for the locked
//! diagnostics): the length of a fixed array (`[A; N]` vs `[u8; 3]` — the
//! length compares verbatim), a lifetime argument vs a type argument
//! (`Cow<'_, A>` vs `Pair<u8, u16>` — `'_` cannot bind `u8`), and slot
//! binding inside fn-pointer / trait-object templates (verbatim fallback).

use batch_impl::batch_impl;
use std::borrow::Cow;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Slice `[A]`
// ------------------------------------------------------------
#[batch_impl([u8] impl{[A]} { fn n(&self) -> usize { self.len() } })]
trait ShapeSlice {
    fn n(&self) -> usize;
}

#[test]
fn shape_slice() {
    let s: &[u8] = &[1, 2, 3];
    assert_eq!(s.n(), 3);
}

// ------------------------------------------------------------
// 2. Triple tuple `(A, B, C)`
// ------------------------------------------------------------
#[batch_impl((u8, u16, u32) impl{(A, B, C)} { fn sum(&self) -> u32 { self.0 as u32 + self.1 as u32 + self.2 } })]
trait ShapeTriple {
    fn sum(&self) -> u32;
}

#[test]
fn shape_triple_tuple() {
    let t = (1u8, 2u16, 3u32);
    assert_eq!(t.sum(), 6);
}

// ------------------------------------------------------------
// 3. Fixed array with a literal length `[A; 3]` (length compares verbatim)
// ------------------------------------------------------------
#[batch_impl([u8; 3] impl{[A; 3]} { fn n(&self) -> usize { self.len() } })]
trait ShapeArrLit {
    fn n(&self) -> usize;
}

#[test]
fn shape_array_literal_len() {
    let a = [1u8, 2, 3];
    assert_eq!(a.n(), 3);
}

// ------------------------------------------------------------
// 4. Reference with a lifetime `&'static A` — the reference lifetime is
//    compared only structurally (ignored), the element binds
// ------------------------------------------------------------
#[batch_impl(&'static u8 impl{&'static A} { fn val(&self) -> A { **self } })]
trait ShapeRefLt {
    fn val(&self) -> u8;
}

#[test]
fn shape_ref_with_lifetime() {
    static X: u8 = 9;
    assert_eq!((&X).val(), 9);
}

// ------------------------------------------------------------
// (5. Parenthesized single-element types `(A)` are NOT matchable: the DSL
//  treats `(T)` as a transparent group — `(u8)` expands to the leaf `u8`,
//  so a `(A)` template can never see a parenthesized target. Tuples with
//  commas (`(A, B)`) are real tuples and match normally, see test 2.)
// ------------------------------------------------------------

// ------------------------------------------------------------
// 6. Nested fixed arrays `[[A; 2]; 2]`
// ------------------------------------------------------------
#[batch_impl([[u8; 2]; 2] impl{[[A; 2]; 2]} { fn n(&self) -> usize { self.len() } })]
trait ShapeNestedArr {
    fn n(&self) -> usize;
}

#[test]
fn shape_nested_array() {
    let a = [[1u8, 2], [3, 4]];
    assert_eq!(a.n(), 2);
}

// ------------------------------------------------------------
// 7. Multi-segment path `std::rc::Rc<A>` (same segment count verbatim,
//    the arg binds)
// ------------------------------------------------------------
#[batch_impl(std::rc::Rc<u8> impl{std::rc::Rc<A>} { fn val(&self) -> A { **self } })]
trait ShapePath {
    fn val(&self) -> u8;
}

#[test]
fn shape_multi_segment_path() {
    let r = std::rc::Rc::new(7u8);
    assert_eq!(r.val(), 7);
}

// ------------------------------------------------------------
// 8. fn-pointer type — verbatim fallback: an identical template matches
//    itself (zero bindings), a templated one cannot bind
// ------------------------------------------------------------
#[batch_impl(fn(u8) -> u16 impl{fn(u8) -> u16} { fn invoke(&self, x: u8) -> u16 { self(x) } })]
trait ShapeFn {
    fn invoke(&self, x: u8) -> u16;
}

#[test]
fn shape_fn_ptr_identical() {
    let f: fn(u8) -> u16 = |x| x as u16 + 1;
    assert_eq!(f.invoke(4), 5);
}

// ------------------------------------------------------------
// 9. Trait object — verbatim fallback: identical template matches itself
// ------------------------------------------------------------
#[batch_impl(dyn Fn(u8) -> u16 + Send impl{dyn Fn(u8) -> u16 + Send} { fn invoke(&self, x: u8) -> u16 { self(x) } })]
trait ShapeDyn {
    fn invoke(&self, x: u8) -> u16;
}

#[test]
fn shape_trait_object_identical() {
    fn f(x: u8) -> u16 {
        x as u16 + 2
    }
    let d: &(dyn Fn(u8) -> u16 + Send) = &f;
    assert_eq!(d.invoke(4), 6);
}

// ------------------------------------------------------------
// 10. `Cow<'_, A>` — a template WITH a lifetime argument: the lifetime
//     compares verbatim (same shape), the element binds. This is the
//     Cow-shaped template working on a Cow-shaped leaf. What does NOT work
//     is binding a lifetime argument to a type argument (see the locked ui
//     fixture `impl_shape_lifetime_arg`): a `Cow<'_, A>` template cannot
//     destructure a plain `Box<u8>` leaf.
// ------------------------------------------------------------
#[batch_impl(Cow<'_, u8> impl{Cow<'_, A>} { fn val(&self) -> A { **self } })]
trait ShapeCow {
    fn val(&self) -> u8;
}

#[test]
fn shape_cow_same_shape() {
    let c = std::borrow::Cow::Borrowed(&5u8);
    assert_eq!(c.val(), 5);
}

// ------------------------------------------------------------
// 11. The prototype-impl pattern (user scenario): one correct implementation
//     written for `Box<u8>` covers the whole `[Box, Rc].@num` matrix — the
//     template's literals bind to each leaf (u8 := u16/.../f64, Box := Rc),
//     and the directive body is rewritten per impl. (A `const MAX` version
//     would need `Box::new`/`Rc::new` in const position — not stable, E0015
//     — so the pattern is exercised with an associated fn.)
// ------------------------------------------------------------
#[batch_impl([Box, Rc].@num impl{Box<u8>} #mk{Box::new(u8::MAX)})]
trait TMk {
    fn mk() -> Self;
}

#[test]
fn prototype_impl_covers_matrix() {
    assert_eq!(<Box<u8> as TMk>::mk(), Box::new(u8::MAX));
    assert_eq!(<Box<u16> as TMk>::mk(), Box::new(u16::MAX));
    assert_eq!(<Box<f32> as TMk>::mk(), Box::new(f32::MAX));
    assert_eq!(<Rc<u8> as TMk>::mk(), Rc::new(u8::MAX));
    assert_eq!(<Rc<usize> as TMk>::mk(), Rc::new(usize::MAX));
}

// ------------------------------------------------------------
// 12. Cow-shaped leaves in the matrix: `Cow<'_, [u8, u16]>` distributes the
//     list over the type argument (the lifetime stays `'_`); a Cow-shaped
//     prototype template (`impl{Cow<'_, u8>}`) covers them via same-shape
//     matching (u8 := u16). The trait signature stays slot-free (the slot
//     binds to u8 on one impl and u16 on the other, so the method body must
//     not return the slot type).
// ------------------------------------------------------------
#[batch_impl(Cow<'_, [u8, u16]> impl{Cow<'_, A>} { fn tag(&self) -> usize { 12 } })]
trait ShapeCowMatrix {
    fn tag(&self) -> usize;
}

#[test]
fn shape_cow_matrix_leaves() {
    let c = Cow::Borrowed(&7u8);
    assert_eq!(c.tag(), 12);
    let d: Cow<'_, u16> = Cow::Owned(9u16);
    assert_eq!(d.tag(), 12);
}

// ------------------------------------------------------------
// 13. Multi-prototype coverage: one shape family per prototype template,
//     combined in a single multi-spec attribute — `Box<u8>` covers the
//     1-arity containers, `Cow<'_, u8>` covers the Cow family (the
//     lifetime-bearing 2-arity shape). Each spec shares the same body.
// ------------------------------------------------------------
#[batch_impl(
    [Box, Rc].@num impl{Box<u8>} #tag{13},
    Cow<'_, @num> impl{Cow<'_, u8>} #tag{13}
)]
trait MultiProto {
    fn tag() -> usize;
}

#[test]
fn multi_prototype_covers_families() {
    assert_eq!(<Box<u8> as MultiProto>::tag(), 13);
    assert_eq!(<Box<f64> as MultiProto>::tag(), 13);
    assert_eq!(<Rc<u16> as MultiProto>::tag(), 13);
    assert_eq!(<Cow<'_, u8> as MultiProto>::tag(), 13);
    assert_eq!(<Cow<'_, i32> as MultiProto>::tag(), 13);
}

// ------------------------------------------------------------
// 14. User shorthand 1: the two prototype specs wrapped in one list with a
//     shared trailing body (`[...] #tag{13}` — the shared body merges into
//     every leaf of both specs)
// ------------------------------------------------------------
#[batch_impl(
    [[Box, Rc].@num impl{Box<u8>},
     Cow<'_, @num> impl{Cow<'_, u8>}] #tag{14}
)]
trait ProtoListShared {
    fn tag() -> usize;
}

#[test]
fn prototype_list_shared_body() {
    assert_eq!(<Box<u8> as ProtoListShared>::tag(), 14);
    assert_eq!(<Box<f64> as ProtoListShared>::tag(), 14);
    assert_eq!(<Rc<u16> as ProtoListShared>::tag(), 14);
    assert_eq!(<Cow<'_, u8> as ProtoListShared>::tag(), 14);
    assert_eq!(<Cow<'_, i32> as ProtoListShared>::tag(), 14);
}

// ------------------------------------------------------------
// 15. User shorthand 2: the container+prototype pairs in one list, `.@num`
//     applied to the WHOLE list — each pair distributes the type family
//     (`[Box,Rc] impl{Box<u8>}` → Box<u8>..Rc<f64> with the Box<u8>
//     prototype; `Cow<'_> impl{Cow<'_,u8>}` → the Cow family)
// ------------------------------------------------------------
#[batch_impl(
    [[Box, Rc] impl{Box<u8>},
     Cow<'_> impl{Cow<'_, u8>}].@num #tag{15}
)]
trait ProtoListPow {
    fn tag() -> usize;
}

#[test]
fn prototype_list_pow() {
    assert_eq!(<Box<u8> as ProtoListPow>::tag(), 15);
    assert_eq!(<Box<f64> as ProtoListPow>::tag(), 15);
    assert_eq!(<Rc<u16> as ProtoListPow>::tag(), 15);
    assert_eq!(<Cow<'_, u8> as ProtoListPow>::tag(), 15);
    assert_eq!(<Cow<'_, i32> as ProtoListPow>::tag(), 15);
}

// ------------------------------------------------------------
// 16. Fixed-array LENGTH binding: `[A; N]` — a bare const-param name in the
//     length slot binds to the leaf's length expression (N := 3). The body
//     may reference N (rewritten per impl).
// ------------------------------------------------------------
#[batch_impl([u8; 3] impl{[A; N]} { fn n16(&self) -> usize { N } })]
trait ShapeArrLenBound {
    fn n16(&self) -> usize;
}

#[test]
fn shape_array_length_binding() {
    let a = [1u8, 2, 3];
    assert_eq!(a.n16(), 3);
}

// ------------------------------------------------------------
// 17. `'_'` anonymous-lifetime wildcard: the template's `'_'` matches ANY
//     lifetime in the leaf (here a named `'a`, declared by the impl
//     generics), while the type argument still binds
// ------------------------------------------------------------
#[batch_impl(<'a> Cow<'a, u8> impl{Cow<'_, A>} { fn val17(&self) -> A { **self } })]
trait ShapeCowWildcard {
    fn val17(&self) -> u8;
}

#[test]
fn shape_cow_lifetime_wildcard() {
    // `Cow<'_, u8>` leaf (the impl-generic `'a` instantiates to the actual
    // lifetime): the template's `'_` matches any lifetime, A := u8
    let c = Cow::Borrowed(&1u8);
    assert_eq!(c.val17(), 1);
    // a `'static` leaf is matched by the same wildcard
    fn assert_trait<T: ShapeCowWildcard>() {}
    assert_trait::<Cow<'static, u8>>();
}

// ------------------------------------------------------------
// 18. `_` type wildcard: `Box<_>` matches ANY `Box<T>` — the `_` is a pure
//     placeholder, it never binds a slot and never gets replaced
// ------------------------------------------------------------
#[batch_impl(Box<u8> impl{Box<_>} { fn val18(&self) -> u8 { **self } })]
trait ShapeTypeWildcard {
    fn val18(&self) -> u8;
}

#[batch_impl([u8; 3] impl{[A; _]} { fn n18(&self) -> usize { self.len() } })]
trait ShapeLenWildcard {
    fn n18(&self) -> usize;
}

#[test]
fn shape_underscore_wildcard() {
    let b = Box::new(9u8);
    assert_eq!(b.val18(), 9);
    // the same `Box<_>` template covers a different element type
    fn assert_trait<T: ShapeTypeWildcard>() {}
    assert_trait::<Box<u8>>();

    let a = [1u8, 2, 3];
    assert_eq!(a.n18(), 3);
    fn assert_len<T: ShapeLenWildcard>() {}
    assert_len::<[u8; 3]>();
}
