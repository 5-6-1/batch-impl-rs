//! # A small "data inspection" library: 29 impls from ~15 lines of DSL
//!
//! Hand-writing these impls takes ~80 lines: the signature and body copied
//! once per numeric type, four wrapper types each repeating the
//! `(**self).xxx()` delegation, hand-written generics per tuple length,
//! separate impls for fn/HashMap/pointers/associated types… The DSL below
//! batches all of it.
//!
//! Features covered: `[...]` side-by-side lists, shared body, `.` list
//! application, `&`/`*const`/`*mut` prefixes, `where{...}` constraints,
//! `#delegate` / `#fill` / `#name` directives, tuple generation `().1..=4`,
//! left-associative `-`, associated type bindings `Item=T`, and the three
//! entry points `batch_impl` / `batch_impl_only` / `batch_trait!`.

use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

// ============================================================
// 1. 12 numeric types: `[...]` list + shared body → 12 impls
// ============================================================
// `Self::default()` is 0 for both integers and floats, so one body covers
// every numeric type.
#[batch_impl(
    [u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64] {
        fn describe(&self) -> String { format!("num:{self}") }
        fn is_zero(&self) -> bool { *self == Self::default() }
    }
)]
trait Describe {
    fn describe(&self) -> String;
    fn is_zero(&self) -> bool;
}

// ============================================================
// 2. & / Box / Rc / Arc delegate to the inner value: `[&, Box, Rc, Arc].T` → 4 impls
// ============================================================
// Why does one line cover all four? `self` is a reference to the wrapper in
// every case:
// - &T  : self is &&T, `**self` = T
// - Box: self is &Box<T>, `**self` = T
// So the delegation body is identical; `#delegate` copies the signatures and
// generates `(**self).method()`. `&` is just another list element, and `.`
// applies each of them to T at once.
#[batch_impl_only(
    <T: Describe> [&, Box, Rc, Arc].T #delegate(describe, is_zero){**self}
)]
trait Describe {
    fn describe(&self) -> String;
    fn is_zero(&self) -> bool;
}

// ============================================================
// 3. Tuple generation: `().1..=4` → 4 impls, each with its own generics
// ============================================================
#[batch_impl(
    ().1..=4 { fn describe(&self) -> &'static str { "tuple" } }
)]
trait DescribeTuple {
    fn describe(&self) -> &'static str;
}

// ============================================================
// 4. The `-` operator (left-associative, accumulates args): fn return types / HashMap<K, V>
// ============================================================
#[batch_impl(fn(i32, u32)-String)]
trait FnReturn {}

#[batch_impl(HashMap-u8-u16)]
trait KvMarker {}

// ============================================================
// 5. Associated type binding `Item=T` + `#name{body}` for a single item (const)
// ============================================================
#[batch_impl(
    <T> IterInfo<Item=T> Vec<T> {
        fn describe(&self) -> String { format!("vec:{}", self.len()) }
    }
)]
trait IterInfo {
    type Item;
    fn describe(&self) -> String;
}

#[batch_impl(u8 #MAX{255})]
trait HasMax {
    const MAX: u8;
}

// ============================================================
// 6. `#fill(args){body}`: one body shared by several methods
// ============================================================
#[batch_impl(u8 #fill(name, kind){"u8"})]
trait Kind {
    fn name(&self) -> &'static str;
    fn kind(&self) -> &'static str;
}

// ============================================================
// 7. `batch_trait!`: batch impls for an already-declared trait, multi-segment + unsafe segment
// ============================================================
trait Multi {}

/// # Safety
///
/// Demo of the `unsafe` segment syntax only; no safety invariant to document.
unsafe trait UnsafeMark {}

batch_trait!(
    Multi: u8, u16;
    unsafe UnsafeMark: u32
);

// ============================================================
// 8. Pointer prefixes `*const` / `*mut`
// ============================================================
#[batch_impl(*const.u32, *mut.i32)]
trait PtrMarker {}

// ============================================================
// Verification: 29 impls (12 numeric + 4 wrappers + 4 tuples + 2 fn/HashMap
// + 3 Multi/Unsafe + 2 assoc-type/const + 2 pointers)
// ============================================================
fn main() {
    // 1. Numeric
    assert!(0u8.is_zero());
    assert_eq!(3i32.describe(), "num:3");
    assert!(0.0f64.is_zero());

    // 2. Wrappers (same delegation body for all four)
    assert!(Box::new(0u32).is_zero());
    assert!(!Rc::new(5u32).is_zero());
    assert!(!Arc::new(5u32).is_zero());
    assert_eq!(7u64.describe(), "num:7");
    assert!(!Box::new(3i32).is_zero());

    // 3. Tuples
    assert_eq!((1u8,).describe(), "tuple");
    assert_eq!((1u8, 2u16, 3u32, 4u64).describe(), "tuple");

    // 4. fn return type / HashMap
    fn _f<T: FnReturn>(_: &T) {}
    fn _k<T: KvMarker>(_: &T) {}
    let fr: fn(i32, u32) -> String = |_, _| String::new();
    _f(&fr);
    _k(&HashMap::<u8, u16>::new());

    // 5. Associated type + const
    assert_eq!(vec![1u8, 2, 3].describe(), "vec:3");
    assert_eq!(<u8 as HasMax>::MAX, 255);

    // 6. #fill
    assert_eq!(0u8.name(), "u8");
    assert_eq!(0u8.kind(), "u8");

    // 7. batch_trait!
    fn _m<T: Multi>(_: &T) {}
    fn _u<T: UnsafeMark>(_: &T) {}
    _m(&0u8);
    _m(&0u16);
    _u(&0u32);

    // 8. Pointers
    fn _p<T: PtrMarker>(_: &T) {}
    let c: *const u32 = &5u32;
    let m: *mut i32 = &mut 5i32;
    _p(&c);
    _p(&m);

    println!("✔ ~15 lines of DSL → 29 impls, all assertions pass");
}
