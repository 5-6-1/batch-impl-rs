// batch-impl 高价值回归测试。
//
// 这里收集从原 `examples/tests.rs` 抽出的关键 corner case 与一致性
// 验证用例（`tests/dsl.rs` 未覆盖到的部分）：
// - 嵌套尖括号（`>>`）
// - 路径类型（`std::collections::HashMap<K, V>`）
// - const 泛型 + 类型标注混合
// - 生命周期泛型
// - dyn trait + 多重 bound
// - `batch_impl` 与 `batch_trait!` 在 10 种 spec 下的一致性

use batch_impl::{batch_impl, batch_trait};

// ============================================================
// 1. 嵌套尖括号 Vec<Vec<T>> —— 验证 `>>` 不破坏深度跟踪
// ============================================================
#[batch_impl(<T> Vec<Vec<T>>)]
trait NestedGeneric {}

#[test]
fn nested_angle_brackets() {
    fn _check<T: NestedGeneric>() {}
    _check::<Vec<Vec<i32>>>();
    _check::<Vec<Vec<String>>>();
}

// ============================================================
// 2. 路径类型 std::collections::HashMap<K, V>
// ============================================================
#[batch_impl(<K, V> std::collections::HashMap<K, V>)]
trait PathType {}

#[test]
fn path_type_with_generics() {
    fn _check<T: PathType>() {}
    _check::<std::collections::HashMap<i32, String>>();
}

// ============================================================
// 3. const 泛型 <const N: usize> [i32; N]
// ============================================================
#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N] {
    fn len_const(&self) -> usize { N }
    fn first(&self) -> i32 { self[0] }
})]
trait ConstGeneric<const N: usize> {
    fn len_const(&self) -> usize;
    fn first(&self) -> i32;
}

#[test]
fn const_generic_array() {
    let arr: [i32; 5] = [10, 20, 30, 40, 50];
    assert_eq!(arr.len_const(), 5);
    assert_eq!(arr.first(), 10);
}

// ============================================================
// 4. 类型标注 + const 泛型混合 <T: Clone, const N: usize>
//    验证 DSL 不被 `<T : Clone , const N : usize>` 中的空格 / 逗号干扰
#[batch_impl(<T: Clone, const N: usize> MixedGeneric<T, N> [T; N] {
    fn repeat_inner(&self) -> Vec<T> {
        std::iter::repeat(self[0].clone()).take(N).collect()
    }
})]
trait MixedGeneric<T, const N: usize> {
    fn repeat_inner(&self) -> Vec<T>;
}

#[test]
fn mixed_type_bound_and_const_generic() {
    let arr: [String; 3] =
        [String::from("hi"), String::from("hi"), String::from("hi")];
    assert_eq!(arr.repeat_inner().len(), 3);
}

// ============================================================
// 5. 生命周期泛型 <'a, T: 'a> &'a T
// ============================================================
#[allow(dead_code)]
#[batch_impl(<'a, T: 'a> LifetimeTrait<'a, T> &'a T)]
trait LifetimeTrait<'a, T> {}

#[test]
fn lifetime_generic() {
    fn _check<'a, T: 'a>()
    where
        &'a T: LifetimeTrait<'a, T>,
    {
    }
    _check::<'static, i32>();
}

// ============================================================
// 6. dyn trait + 多重 bound（`+ Send + Sync`）
// ============================================================
#[batch_impl(dyn std::fmt::Display + Send + Sync)]
trait DynMarkerMultiBound {}

#[test]
fn dyn_trait_with_multi_bounds() {
    fn _check<T: DynMarkerMultiBound + ?Sized>() {}
    _check::<dyn std::fmt::Display + Send + Sync>();
}

// ============================================================
// 7. `batch_impl` 与 `batch_trait!` 一致性
//
// 10 组平行 spec：同一 DSL 在两个宏下应生成等价的 impl。
// 编译时检查 +少量运行时 assert_eq。
// ============================================================

// --- 基础类型 ---
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

// --- 泛型 ---
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

// --- trait 泛型 + 自定义 body ---
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

// --- 并列列表 ---
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

// --- ^ 运算符（引用前缀） ---
trait CmpCaret {}
#[batch_impl(&^u32)]
trait CmpAttrCaret {}
batch_trait!(CmpCaret: &^u32);

#[test]
fn cmp_caret_prefix() {
    fn _a<T: CmpAttrCaret>() {}
    fn _b<T: CmpCaret>() {}
    _a::<&u32>();
    _b::<&u32>();
}

// --- 嵌套 ^ ---
trait CmpNestedCaret {}
#[batch_impl(Box^Box^isize)]
trait CmpAttrNestedCaret {}
batch_trait!(CmpNestedCaret: Box^Box^isize);

#[test]
fn cmp_nested_caret() {
    fn _a<T: CmpAttrNestedCaret>() {}
    fn _b<T: CmpNestedCaret>() {}
    _a::<Box<Box<isize>>>();
    _b::<Box<Box<isize>>>();
}

// --- ^ 穿透 [] ---
trait CmpCaretBracket {}
#[batch_impl(Box^[Box^isize])]
trait CmpAttrCaretBracket {}
batch_trait!(CmpCaretBracket: Box^[Box^isize]);

#[test]
fn cmp_caret_through_bracket() {
    fn _a<T: CmpAttrCaretBracket>() {}
    fn _b<T: CmpCaretBracket>() {}
    _a::<Box<[Box<isize>]>>();
    _b::<Box<[Box<isize>]>>();
}

// --- const 泛型 ---
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

// --- 生命周期 ---
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
    // 编译期通过即可
    let _ = _a::<'static, i32>;
    let _ = _b::<'static, i32>;
}

// --- 路径 trait ---
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
