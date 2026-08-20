//! The `impl{...}` shape templates (0.8.0) boundary-case tests: three-plus merged
//! templates, redundant identical bindings, `@trait` inside the template,
//! coexistence with unsafe impls / attributes, and `batch_trait!` support.
//! (new test module; split-style organization per the features/ layout)

use batch_impl::{batch_impl, batch_trait};

// ------------------------------------------------------------
// 1. Three-plus `impl{...}` merged — distinct slots all bound
// ------------------------------------------------------------
#[batch_impl(
    Box<u32> impl{X<u32>} impl{Y<u32>} impl{Z<u32>}
    { fn mk(x: u32) -> X<u32> { X::new(x) } }
)]
trait BndT1 {
    fn mk(x: u32) -> Self;
}

#[test]
fn impl_three_templates_merge() {
    assert_eq!(*<Box<u32> as BndT1>::mk(3), 3);
}

// ------------------------------------------------------------
// 2. Redundant identical binding across templates: `impl{X}` and
//    `impl{X}` bind the same slot to the same subtree — legal. The body
//    avoids `X::new` (a bare-ident slot binds the whole leaf; an
//    associated-fn call through it would render `Box<u32>::new` with
//    Alone-`<` tokens, which rustc misreads as a comparison — E0178).
// ------------------------------------------------------------
#[batch_impl(Box<u32> impl{X} impl{X} { fn mk(x: u32) -> X { Box::new(x) } })]
trait BndT2 {
    fn mk(x: u32) -> Self;
}

#[test]
fn impl_redundant_identical_binding() {
    assert_eq!(*<Box<u32> as BndT2>::mk(4), 4);
}

// ------------------------------------------------------------
// 3. `@trait` inside the template expands to the trait path before the
//    match: `impl{@trait<T>}` + a target shaped `BndT3<u8>` — the trait
//    path ident differs from the leaf base, so it binds as a slot
// ------------------------------------------------------------
struct BndT3<T>(T);

#[batch_impl(BndT3<u8> impl{@trait<T>} { fn tag(&self) -> u32 { 3 } })]
trait BndMarker {
    fn tag(&self) -> u32;
}

#[test]
fn impl_at_trait_template() {
    let b = BndT3(1u8);
    assert_eq!(b.tag(), 3);
}

// ------------------------------------------------------------
// 4. `impl{...}` with an unsafe trait (all generated impls are unsafe)
// ------------------------------------------------------------
/// # Safety
///
/// Marker trait for the demo only; no real unsafe semantics.
#[batch_impl(Box<u8> impl{X} { fn tag(&self) -> u32 { 4 } })]
unsafe trait BndT4 {
    fn tag(&self) -> u32;
}

unsafe impl BndT4 for u8 {
    fn tag(&self) -> u32 {
        4
    }
}

#[test]
fn impl_unsafe_trait() {
    assert_eq!(<Box<u8> as BndT4>::tag(&Box::new(1)), 4);
}

// ------------------------------------------------------------
// 5. `batch_trait!` supports the `impl{...}` attachment too (the trailing
//    peel runs in the shared parse layer)
// ------------------------------------------------------------
trait BtImpl {}

batch_trait!(BtImpl: i32 impl{T});

#[test]
fn impl_batch_trait() {
    fn check<T: BtImpl>() {}
    check::<i32>();
}

// ------------------------------------------------------------
// 6. `impl{...}` on an attribute-carrying spec (the attr passes through)
// ------------------------------------------------------------
#[batch_impl(#[allow(dead_code)] Box<u8> impl{X} { fn tag(&self) -> u32 { 6 } })]
trait BndT6 {
    fn tag(&self) -> u32;
}

#[test]
fn impl_with_attribute() {
    assert_eq!(<Box<u8> as BndT6>::tag(&Box::new(1)), 6);
}

// ------------------------------------------------------------
// 7. `impl{...}` coexisting with `#fill` directives (the directive-copied
//    body is rewritten by the slot mapping like a handwritten body)
// ------------------------------------------------------------
#[batch_impl(Box<u8> impl{X} #fill(@all_methods){7})]
trait ComboFill {
    fn a(&self) -> u32;
    fn b(&self) -> u32;
}

#[test]
fn impl_with_fill_directive() {
    let b = Box::new(1u8);
    assert_eq!(b.a(), 7);
    assert_eq!(b.b(), 7);
}

// ------------------------------------------------------------
// 8. `impl{...}` with `@N` where references: the template matches a
//    generator-tuple leaf (`().2` → `(P0, P1)`), the where predicate
//    references the fresh generics
// ------------------------------------------------------------
#[batch_impl(().2 impl{(A, B)} where{@0: Clone} { fn n(&self) -> usize { 2 } })]
trait ComboAtN {
    fn n(&self) -> usize;
}

#[test]
fn impl_with_at_n_where() {
    let t = (1u8, 2u16);
    assert_eq!(t.n(), 2);
}

// ------------------------------------------------------------
// 9. `#blanket` + `impl{...}`: the blanket spec carries the template as a
//    trailing attachment — the template binds the blanket's fresh target
//    (here `Box<T>`); the delegated body is unaffected
// ------------------------------------------------------------
#[batch_impl(u8 { fn tag9(&self) -> u32 { 9 } })]
#[batch_impl(#blanket(tag9){Box} impl{X})]
trait ComboBlanket {
    fn tag9(&self) -> u32;
}

#[test]
fn impl_with_blanket() {
    assert_eq!(Box::new(5u8).tag9(), 9);
}
