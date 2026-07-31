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
// - 宏调用 `m![]` 的透传，以及宏体与指令 / 裸 where 的边界

use batch_impl::{batch_impl, batch_impl_only, batch_trait};

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
        std::iter::repeat_n(self[0].clone(), N).collect()
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

// ============================================================
// 17. 宏调用 m![] 与 DSL 的交互
//     - m![] 作为目标类型 / 泛型实参 / where 谓词透传
//     - m![] 宏体是透传的宏参数：指令（#name）与裸 where 不得进入宏体
// ============================================================
macro_rules! ty {
    () => { Vec<u8> };
}
macro_rules! passthrough {
    ($($t:tt)*) => { $($t)* };
}

#[batch_impl(ty![])]
trait MacroBracketA {}
#[batch_impl(passthrough![Vec<u8>])]
trait MacroBracketB {}
#[batch_impl(Box<ty![]>)]
trait MacroBracketC {}

#[batch_impl(
    <T> MacroBracketFnRet<T> Vec<T> where T: Fn() -> ty![]
    { fn ok(&self) -> bool { true } }
)]
trait MacroBracketFnRet<T> {
    fn ok(&self) -> bool;
}

#[test]
fn macro_bracket_passthrough() {
    fn a<T: MacroBracketA>(_: &T) {}
    fn b<T: MacroBracketB>(_: &T) {}
    fn c<T: MacroBracketC>(_: &T) {}
    a(&vec![1u8]);
    b(&vec![1u8]);
    c(&Box::new(vec![1u8]));
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok());
}

// --- m![] 宏体不展开指令、不处理裸 where ---
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

macro_rules! len_ty {
    (#len{ $n:expr }) => {
        u8
    };
}

#[batch_impl_only(
    usize #len{5},
    len_ty![#len{5}] #len{6}
)]
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

#[test]
fn macro_bracket_directive_not_expanded() {
    assert_eq!(0usize.len(), 5);
    assert_eq!(0u8.len(), 6);
}

trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

macro_rules! m2 {
    (where) => { Vec<u8> };
}

#[batch_impl_only(
    <T> MacroBracketWhere<T> Vec<T> where T: Fn() -> m2![where]
    { fn ok2(&self) -> bool { true } }
)]
trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

#[test]
fn macro_bracket_where_not_processed() {
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok2());
}

// ============================================================
// 18. 路径前缀 `#path::to::Trait:`（batch_impl_only）
//     - 生成 impl 引用外部模块中的真实 trait
//     - dummy trait 仍用于指令签名读取
//     - DSL 中 `Trait<T>` 通过路径末段 ident 识别为 trait 泛型应用
// ============================================================
mod ext {
    pub mod traits {
        pub trait PathPrefixTrait {
            fn tag(&self) -> &'static str;
        }

        pub trait PathPrefixGen<T> {
            fn head(&self) -> T;
        }
    }
}

// dummy trait 被 batch_impl_only 丢弃，此处导入真实 trait 以便方法调用
use ext::traits::{PathPrefixGen, PathPrefixTrait};

#[batch_impl_only(
    #ext::traits::PathPrefixTrait: usize #tag{"usize"}, isize #tag{"isize"}
)]
trait PathPrefixTrait {
    fn tag(&self) -> &'static str;
}

#[test]
fn cmp_path_prefix_directive() {
    assert_eq!(0usize.tag(), "usize");
    assert_eq!(0isize.tag(), "isize");
}

#[batch_impl_only(
    #ext::traits::PathPrefixGen: <T: Clone> PathPrefixGen<T> Vec<T>
    { fn head(&self) -> T { self[0].clone() } }
)]
trait PathPrefixGen<T> {
    fn head(&self) -> T;
}

#[test]
fn cmp_path_prefix_trait_generic() {
    assert_eq!(vec![1i32].head(), 1);
    assert_eq!(vec![String::from("x")].head(), "x");
}

// ============================================================
// 19. 数组/切片 builder：`TyPrimitiveArray` 合并 TySlice + TyFixedArray
//     - `[]^T` => `[T]`（空基座包出切片）
//     - `[T]^N` => `[T; N]`（数字字面量 / const 泛型 / 范围 / 列表）
//     - `<const N> []-X-N` => `[X; N]`：整矩阵包进 const 泛型数组
//     - `()^N` 的 fresh 泛型元组作为泛型实参/数组元素时自动外提
// ============================================================
#[batch_impl([]^u8)]
trait ArrSlice {}

#[batch_impl([u8]^3)]
trait ArrLit {}

#[batch_impl(<const N: usize> [u8]^N)]
trait ArrConst {}

#[batch_impl([u8]^1..3)]
trait ArrRange {}

#[batch_impl([u8]^[1, 2, 4])]
trait ArrList {}

#[batch_impl(<const N: usize> []-[&, self, Box]^[u8, i8, ()^0..3]-N)]
trait ArrMatrix {}

#[batch_impl(Box^()^0..3)]
trait ArrTupleGeneric {}

#[test]
fn primitive_array_rules() {
    fn s<T: ArrSlice + ?Sized>(_: &T) {}
    fn l<T: ArrLit>(_: &T) {}
    fn c<T: ArrConst>(_: &T) {}
    fn r<T: ArrRange>(_: &T) {}
    fn ls<T: ArrList>(_: &T) {}
    fn m<T: ArrMatrix>(_: &T) {}
    fn tg<T: ArrTupleGeneric>(_: &T) {}

    s(&[1u8, 2][..]);
    l(&[0u8; 3]);
    c(&[0u8; 7]);
    r(&[0u8; 1]);
    r(&[0u8; 2]);
    ls(&[0u8; 1]);
    ls(&[0u8; 4]);
    m(&[&5u8; 2]);
    m(&[5i8; 2]);
    m(&[(); 2]);
    m(&[(1u8, 2i8); 2]);
    let bx: [Box<u8>; 2] = [Box::new(1), Box::new(2)];
    m(&bx);
    tg(&Box::new(()));
    tg(&Box::new((1u8,)));
    tg(&Box::new((1u8, 2u16)));
}
