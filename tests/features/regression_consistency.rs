//! regression.rs §7: `batch_impl` vs `batch_trait!` consistency — the same
//! DSL generates equivalent impls under both macros (10 specs).
//! (split from the former single-file `tests/regression.rs`)

use batch_impl::{batch_impl, batch_trait};

// --- basic types ---
trait CmpBase {}
#[batch_impl(usize)]
trait CmpAttrBase {}
batch_trait!(CmpBase: usize);

#[test]
fn cmp_basic() {
    fn _a<T: CmpAttrBase>() {}
    fn _b<T: CmpBase>() {}
    _a::<usize>();
    _b::<usize>();
}

// --- generics ---
trait CmpGeneric {}
#[batch_impl(<T> Vec<T>)]
trait CmpAttrGeneric {}
batch_trait!(CmpGeneric: <T> Vec<T>);

#[test]
fn cmp_generic() {
    fn _a<T: CmpAttrGeneric>() {}
    fn _b<T: CmpGeneric>() {}
    _a::<Vec<i32>>();
    _b::<Vec<i32>>();
}

// --- trait generics + custom body ---
trait CmpTraitGen<T> {
    fn wrap(val: T) -> Self;
}
#[batch_impl(<T> CmpAttrTraitGen<T> i32 {
    fn wrap(_val: T) -> Self { 0 }
})]
trait CmpAttrTraitGen<T> {
    fn wrap(val: T) -> Self;
}
batch_trait!(
    CmpTraitGen: <T> CmpTraitGen<T> i32 {
        fn wrap(_val: T) -> Self { 0 }
    }
);

#[test]
fn cmp_trait_generic_with_body() {
    let a: i32 = CmpAttrTraitGen::<String>::wrap(String::new());
    let b: i32 = CmpTraitGen::<String>::wrap(String::new());
    assert_eq!(a, 0);
    assert_eq!(b, 0);
}

// --- parallel lists ---
trait CmpList {
    fn tag(&self) -> &'static str;
}
#[batch_impl([u8, u16] { fn tag(&self) -> &'static str { "cmp" } })]
trait CmpAttrList {
    fn tag(&self) -> &'static str;
}
batch_trait!(
    CmpList: [u8, u16] { fn tag(&self) -> &'static str { "cmp" } }
);

#[test]
fn cmp_parallel_list() {
    assert_eq!(CmpAttrList::tag(&0u8), "cmp");
    assert_eq!(CmpList::tag(&0u16), "cmp");
}

// --- . operator (reference prefix) ---
trait CmpCaret {}
#[batch_impl(&.u32)]
trait CmpAttrCaret {}
batch_trait!(CmpCaret: &.u32);

#[test]
fn cmp_caret_prefix() {
    fn _a<T: CmpAttrCaret>() {}
    fn _b<T: CmpCaret>() {}
    _a::<&u32>();
    _b::<&u32>();
}

// --- nested . ---
trait CmpNestedCaret {}
#[batch_impl(Box.Box.isize)]
trait CmpAttrNestedCaret {}
batch_trait!(CmpNestedCaret: Box.Box.isize);

#[test]
fn cmp_nested_caret() {
    fn _a<T: CmpAttrNestedCaret>() {}
    fn _b<T: CmpNestedCaret>() {}
    _a::<Box<Box<isize>>>();
    _b::<Box<Box<isize>>>();
}

// --- . through [] ---
trait CmpCaretBracket {}
#[batch_impl(Box.[Box.isize])]
trait CmpAttrCaretBracket {}
batch_trait!(CmpCaretBracket: Box.[Box.isize]);

#[test]
fn cmp_caret_through_bracket() {
    fn _a<T: CmpAttrCaretBracket>() {}
    fn _b<T: CmpCaretBracket>() {}
    _a::<Box<[Box<isize>]>>();
    _b::<Box<[Box<isize>]>>();
}

// --- const generics ---
trait CmpConst<const N: usize> {
    fn val() -> usize {
        N
    }
}
#[batch_impl(<const N: usize> CmpAttrConst<N> [i32; N])]
trait CmpAttrConst<const N: usize> {
    fn val() -> usize {
        N
    }
}
batch_trait!(CmpConst: <const N: usize> CmpConst<N> [i32; N]);

#[test]
fn cmp_const_generic() {
    let a = <[i32; 5] as CmpAttrConst<5>>::val();
    let b = <[i32; 5] as CmpConst<5>>::val();
    assert_eq!(a, 5);
    assert_eq!(b, 5);
}

// --- lifetimes ---
#[allow(dead_code)]
trait CmpLifetime<'a, T> {}
#[allow(dead_code)]
#[batch_impl(<'a, T: 'a> CmpAttrLifetime<'a, T> &'a T)]
trait CmpAttrLifetime<'a, T> {}
batch_trait!(CmpLifetime: <'a, T: 'a> CmpLifetime<'a, T> &'a T);

#[test]
fn cmp_lifetime() {
    fn _a<'a, T: 'a>()
    where
        &'a T: CmpAttrLifetime<'a, T>,
    {
    }
    fn _b<'a, T: 'a>()
    where
        &'a T: CmpLifetime<'a, T>,
    {
    }
    // compiling is enough
    let _ = _a::<'static, i32>;
    let _ = _b::<'static, i32>;
}

// --- path traits ---
mod cmp_mod {
    pub trait PathTrait {}
}
trait CmpPath {}
#[batch_impl(u32)]
trait CmpAttrPath {}
batch_trait!(CmpPath: u32; cmp_mod::PathTrait: u32);

#[test]
fn cmp_path_trait() {
    fn _a<T: CmpAttrPath>() {}
    fn _b<T: CmpPath>() {}
    fn _c<T: cmp_mod::PathTrait>() {}
    _a::<u32>();
    _b::<u32>();
    _c::<u32>();
}
