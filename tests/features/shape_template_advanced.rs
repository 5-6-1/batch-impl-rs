//! The `impl{...}` shape templates (0.8.0) advanced tests: merged templates, body-type
//! rewrites, coexistence with trait generics / where, and `batch_impl_only`.
//! (split from the former single-file `tests/shape_template_impl.rs`)

use batch_impl::{batch_impl, batch_impl_only};

// ------------------------------------------------------------
// 6. Multiple `impl{...}` merged: distinct slots both bound
// ------------------------------------------------------------
#[batch_impl(Box<u32> impl{X<u32>} impl{Y<u32>} { fn mk(x: u32) -> X<u32> { X::new(x) } })]
trait ImplMerge {
    fn mk(x: u32) -> Self;
}

#[test]
fn impl_multiple_templates_merge() {
    assert_eq!(*<Box<u32> as ImplMerge>::mk(9), 9);
}

// ------------------------------------------------------------
// 7. Slot rewrite applies inside generic args of the body's type positions
// ------------------------------------------------------------
#[batch_impl(Vec<i16> impl{Container<T>} { fn head(&self) -> Option<T> { self.first().copied() } })]
trait ImplBodyType {
    fn head(&self) -> Option<i16>;
}

#[test]
fn impl_body_type_rewrite() {
    let v = vec![3i16];
    assert_eq!(v.head(), Some(3));
}

// ------------------------------------------------------------
// 8. `impl{...}` with `where{...}` and trait generics coexisting
// ------------------------------------------------------------
#[batch_impl(<T: Clone> ImplWhereMix<T> Vec<T> impl{Container<U>} where{Vec<T>: Clone} { fn n(&self) -> usize { self.len() } })]
trait ImplWhereMix<T> {
    fn n(&self) -> usize;
}

#[test]
fn impl_where_coexist() {
    let v = vec![1u8, 2, 3];
    assert_eq!(v.n(), 3);
}

// ------------------------------------------------------------
// 9. `batch_impl_only` (trait from outside) supports impl{...}
// ------------------------------------------------------------
trait Outside {
    fn tag(&self) -> &'static str;
}

#[batch_impl_only(usize impl{Num} { fn tag(&self) -> &'static str { "usize" } })]
trait Outside {
    fn tag(&self) -> &'static str;
}

#[test]
fn impl_impl_only() {
    assert_eq!(0usize.tag(), "usize");
}
