use batch_impl::{batch_impl, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

// ============================================================
// 1. 基础用法：为具体类型直接实现
//    #[batch_impl(usize, isize)]
// ============================================================

#[batch_impl(usize, isize)]
trait Numeric {}

fn check_numeric<T: Numeric>(_: &T) {}

fn test_basic() {
    check_numeric(&0usize);
    check_numeric(&0isize);
    println!("  1. basic: OK");
}

// ============================================================
// 2. 带泛型：<T>Vec<T>
//    #[batch_impl(<T> Vec<T>)]
// ============================================================

#[batch_impl(<T> Vec<T>)]
trait Collection {}

fn check_collection<T: Collection>(_: &T) {}

fn test_generic() {
    check_collection(&vec![1, 2, 3]);
    check_collection(&vec!["a", "b"]);
    println!("  2. generic: OK");
}

// ============================================================
// 3. Trait 自身带泛型：<T> TraitName<T> T
//    #[batch_impl(<T> FromValue<T> i32{...})]
// ============================================================

#[batch_impl(<T> FromValue<T> i32{
    fn wrap(_val: T) -> Self {
        0
    }
})]
trait FromValue<T> {
    fn wrap(val: T) -> Self;
}

fn test_trait_with_generic() {
    let x: i32 = FromValue::<String>::wrap(String::from("ignored"));
    assert_eq!(x, 0);
    println!("  3. trait with generic: OK");
}

// ============================================================
// 4. 自定义实现体：isize{...}
//    #[batch_impl(isize{...}, f64{...})]
// ============================================================

#[batch_impl(
    isize{
        fn is_zero(&self) -> bool { *self == 0 }
        fn describe(&self) -> String { format!("isize({})", self) }
    },
    f64{
        fn is_zero(&self) -> bool { *self == 0.0 }
        fn describe(&self) -> String { format!("f64({})", self) }
    }
)]
trait Inspect {
    fn is_zero(&self) -> bool;
    fn describe(&self) -> String;
}

fn test_custom_body() {
    assert!(0isize.is_zero());
    assert!(!42isize.is_zero());
    assert_eq!(42isize.describe(), "isize(42)");

    assert!(0.0f64.is_zero());
    assert!(!3.14f64.is_zero());
    assert_eq!(3.14f64.describe(), "f64(3.14)");
    println!("  4. custom body: OK");
}

// ============================================================
// 5. 泛型 + 并列列表：<T>[Vec<T>, HashMap<usize, T>]
//    #[batch_impl(<T> [Vec<T>, HashMap<usize, T>]{...})]
// ============================================================

#[batch_impl(<T> [Vec<T>, HashMap<usize, T>]{
    fn count(&self) -> usize { self.len() }
})]
trait Count {
    fn count(&self) -> usize;
}

fn test_generic_list() {
    let v: Vec<char> = vec!['a', 'b', 'c'];
    assert_eq!(v.count(), 3);

    let mut m = HashMap::<usize, bool>::new();
    m.insert(0, true);
    m.insert(1, false);
    assert_eq!(m.count(), 2);
    println!("  5. generic list: OK");
}

// ============================================================
// 6. 多个类型 + 自定义实现体：[usize, isize, f32]{...}
//    #[batch_impl([usize, isize, f32]{...})]
// ============================================================

#[batch_impl([usize, isize, f32]{
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged {
    fn tag(&self) -> &'static str;
}

fn test_multi_custom() {
    assert_eq!(0usize.tag(), "number");
    assert_eq!(0isize.tag(), "number");
    assert_eq!(0f32.tag(), "number");
    println!("  6. multi custom: OK");
}

// ============================================================
// 7. Trait 泛型 + 并列列表：<T>Pair<T>[Alpha, Beta, Gamma]
//    #[batch_impl(<T> Pair<T> [Alpha, Beta, Gamma]{...})]
// ============================================================

struct Alpha;
struct Beta;
struct Gamma;

#[batch_impl(<T> Pair<T> [Alpha, Beta, Gamma]{
    fn pair(self, other: T) -> (Self, T) where Self: Sized {
        (self, other)
    }
})]
trait Pair<T> {
    fn pair(self, other: T) -> (Self, T)
    where
        Self: Sized;
}

fn test_trait_generic_list() {
    let a = Alpha.pair(42i32);
    let b = Beta.pair("hello");
    let c = Gamma.pair(3.14f64);
    assert_eq!(a.1, 42i32);
    assert_eq!(b.1, "hello");
    assert_eq!(c.1, 3.14f64);
    println!("  7. trait generic list: OK");
}

// ============================================================
// 8. [isize] 无逗号 = 切片类型（非并列列表）
//    #[batch_impl([isize])]
// ============================================================

#[batch_impl([isize])]
trait SliceMarker {}

fn test_slice_type() {
    fn _check<T: SliceMarker + ?Sized>(_: &T) {}
    let arr: [isize; 3] = [1, 2, 3];
    _check(&arr[..]);
    println!("  8. slice type: OK");
}

// ============================================================
// 9. 嵌套：<T>Describe<T>[Vec<T>, <U> HashMap<T, U>]
//    Vec<T> 继承外层泛型 T
//    HashMap<T, U> 添加自己的泛型 U（合并为 T, U）
// ============================================================

#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>]{
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> {
    fn describe(&self) -> String;
}

fn test_nested() {
    let v: Vec<char> = vec!['a', 'b', 'c'];
    assert_eq!(v.describe(), "len=3");

    let mut m = HashMap::<char, bool>::new();
    m.insert('x', true);
    m.insert('y', false);
    assert_eq!(m.describe(), "len=2");
    println!("  9. nested: OK");
}

// ============================================================
// 10. 多个独立项各自带 body
//     #[batch_impl(usize{...}, String{...})]
// ============================================================

#[batch_impl(
    usize{
        fn id(&self) -> usize { *self }
    },
    String{
        fn id(&self) -> usize { self.len() }
    }
)]
trait Identifiable {
    fn id(&self) -> usize;
}

fn test_multi_specs_with_body() {
    assert_eq!(42usize.id(), 42);
    assert_eq!(String::from("hello").id(), 5);
    println!("  10. multi specs with body: OK");
}

// ============================================================
// 11. 复杂类型：元组、引用、智能指针（验证 token 透传）
// ============================================================

#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool
)]
trait ComplexMarker {}

fn test_complex_types() {
    // 仅验证 trait bound 正确生成，不创建实例
    fn _check<T: ComplexMarker>() {}
    _check::<(i32, String)>();
    _check::<&str>();
    _check::<Box<dyn std::fmt::Display>>();
    _check::<fn(i32) -> bool>();
    println!("  11. complex types: OK");
}

// ============================================================
// 12. 嵌套尖括号 Vec<Vec<T>>（验证 >> 不破坏 <> 深度跟踪）
// ============================================================

#[batch_impl(<T> Vec<Vec<T>>)]
trait NestedGeneric {}

fn test_nested_angle_brackets() {
    fn _check<T: NestedGeneric>() {}
    _check::<Vec<Vec<i32>>>();
    _check::<Vec<Vec<String>>>();
    println!("  12. nested angle brackets (Vec<Vec<T>>): OK");
}

// ============================================================
// 13. 完全路径类型 std::collections::HashMap<K,V>
//     + 多个 impl 泛型 <K, V>
// ============================================================

#[batch_impl(<K, V> std::collections::HashMap<K, V>)]
trait PathType {}

fn test_path_type() {
    fn _check<T: PathType>() {}
    _check::<std::collections::HashMap<i32, String>>();
    println!("  13. path type (std::collections::HashMap<K,V>): OK");
}

// ============================================================
// 14. Trait 名作为参数但无尖括号 → 视为目标类型，不混淆为 trait_params
//     验证：参数中出现的标识符即使与 trait 同名，只要无 <> 就不会被消费
//     注：Rust 不允许 struct 和 trait 同名，所以这里用实际证明：
//     解析器只匹配 "trait_name + <" 模式，纯标识符总是作为目标类型
// ============================================================

struct SameNameStruct;

#[batch_impl(SameNameStruct)]
trait SameNameTrait {}

fn test_trait_name_as_target() {
    fn _check<T: SameNameTrait>() {}
    _check::<SameNameStruct>();
    println!("  14. plain ident never confused as trait_params: OK");
}

// ============================================================
// 15. 多个 trait 泛型参数 <T, U> PairTrait<T, U> (T, U)
// ============================================================

#[batch_impl(<T, U> MultiParam<T, U> (T, U){
    fn first(&self) -> &T { &self.0 }
    fn second(&self) -> &U { &self.1 }
})]
trait MultiParam<T, U> {
    fn first(&self) -> &T;
    fn second(&self) -> &U;
}

fn test_multi_trait_params() {
    let pair: (i32, String) = (42, String::from("hi"));
    assert_eq!(pair.first(), &42i32);
    assert_eq!(pair.second(), &String::from("hi"));
    println!("  15. multi trait params <T,U> Trait<T,U> (T,U): OK");
}

// ============================================================
// 16. 嵌套并列列表：[[u8, u16], [i32, i64]]
//    外层列表 2 项，每项又是列表，展开为 4 个 impl
// ============================================================

#[batch_impl([[u8, u16], [i32, i64]]{
    fn label(&self) -> &'static str { "integer" }
})]
trait IntLabel {
    fn label(&self) -> &'static str;
}

fn test_nested_bracket_list() {
    assert_eq!(0u8.label(), "integer");
    assert_eq!(0u16.label(), "integer");
    assert_eq!(0i32.label(), "integer");
    assert_eq!(0i64.label(), "integer");
    println!("  16. nested list [[u8,u16],[i32,i64]]: OK");
}

// ============================================================
// 17. 切片类型 [u8] + 自定义 body
//    （[u8] 无逗号 = 切片，非并列）
// ============================================================

#[batch_impl(
    [u8]{
        fn len_bytes(&self) -> usize { self.len() }
    }
)]
trait ByteSlice {
    fn len_bytes(&self) -> usize;
}

fn test_slice_with_body() {
    let arr: [u8; 4] = [1, 2, 3, 4];
    assert_eq!(arr.len_bytes(), 4);
    println!("  17. slice [u8] with custom body: OK");
}

// ============================================================
// 18. 参数中标识符与 trait 名相同但无 <> → 不混淆为 trait_params
//     struct 和 trait 不同名，验证纯标识符只作为目标类型
// ============================================================

struct PlainStruct;

#[batch_impl(PlainStruct)]
trait PlainTrait {}

fn test_plain_ident() {
    fn _check<T: PlainTrait>() {}
    _check::<PlainStruct>();
    println!("  18. plain ident as target (no confusion): OK");
}

// ============================================================
// 19. dyn trait 对象（含 + 号）
// ============================================================

#[batch_impl(dyn std::fmt::Display + Send + Sync)]
trait DynMarker {}

fn test_dyn_trait_object() {
    fn _check<T: DynMarker + ?Sized>() {}
    _check::<dyn std::fmt::Display + Send + Sync>();
    println!("  19. dyn trait object (with +): OK");
}

// ============================================================
// 20. Box<T> + trait 泛型 + 自定义 body
// ============================================================

#[batch_impl(<T> UnwrapBox<T> Box<T>{
    fn unwrap_box(self) -> T { *self }
})]
trait UnwrapBox<T> {
    fn unwrap_box(self) -> T;
}

fn test_box_generic() {
    let b: Box<i32> = Box::new(42);
    assert_eq!(b.unwrap_box(), 42);
    println!("  20. Box<T> with trait generic and body: OK");
}

// ============================================================
// 21. 复杂 trait 泛型参数 Vec<T>（含嵌套尖括号 >>）
//     #[batch_impl(<T> MyTrait<Vec<T>> Vec<T>)]
//     验证 parse_balanced_args 正确处理嵌套 <> 和 >>
// ============================================================

#[batch_impl(<T> WrapperTrait<Vec<T>> Vec<T>{
    fn inner(self) -> Vec<T> { self }
})]
trait WrapperTrait<C> {
    fn inner(self) -> C;
}

fn test_complex_trait_generic() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.inner(), vec![1, 2, 3]);
    println!("  21. complex trait generic (Vec<T>): OK");
}

// ============================================================
// 22. 多个复杂 trait 泛型参数
//     #[batch_impl(<K, V> MyTrait<Vec<K>, Vec<V>> HashMap<K,V>)]
// ============================================================

#[batch_impl(<K, V> MapTrait<Vec<K>, Vec<V>> HashMap<K, V>)]
trait MapTrait<A, B> {}

fn test_multi_complex_trait_generic() {
    fn _check<T: MapTrait<Vec<i32>, Vec<String>>>() {}
    _check::<HashMap<i32, String>>();
    println!("  22. multi complex trait generic: OK");
}

// ============================================================
// 23. impl 泛型带类型标注：<T: Clone + std::fmt::Debug>
// ============================================================

#[batch_impl(<T: Clone + std::fmt::Debug> DescribeClone<T> Vec<T>{
    fn describe_clone(&self) -> Vec<T> where T: Clone {
        self.clone()
    }
})]
trait DescribeClone<T> {
    fn describe_clone(&self) -> Vec<T>;
}

fn test_type_bound() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.describe_clone(), vec![1, 2, 3]);
    println!("  23. type bound <T: Clone + Debug>: OK");
}

// ============================================================
// 24. Const 泛型：<const N: usize> [i32; N]
// ============================================================

#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N]{
    fn len_const(&self) -> usize { N }
    fn first(&self) -> i32 { self[0] }
})]
trait ConstGeneric<const N: usize> {
    fn len_const(&self) -> usize;
    fn first(&self) -> i32;
}

fn test_const_generic() {
    let arr: [i32; 5] = [10, 20, 30, 40, 50];
    assert_eq!(arr.len_const(), 5);
    assert_eq!(arr.first(), 10);
    println!("  24. const generic <const N: usize>: OK");
}

// ============================================================
// 25. 类型标注 + const 泛型混合：<T: Clone, const N: usize>
// ============================================================

#[batch_impl(<T: Clone, const N: usize> MixedGeneric<T, N> [T; N]{
    fn repeat_inner(&self) -> Vec<T> {
        std::iter::repeat(self[0].clone()).take(N).collect()
    }
})]
trait MixedGeneric<T, const N: usize> {
    fn repeat_inner(&self) -> Vec<T>;
}

fn test_mixed_generics() {
    let arr: [String; 3] = [String::from("hi"), String::from("hi"), String::from("hi")];
    assert_eq!(arr.repeat_inner().len(), 3);
    println!("  25. mixed <T: Clone, const N: usize>: OK");
}

// ============================================================
// 26. 生命周期泛型：<'a, T: 'a> &'a T
//    验证 parse_balanced_args 正确处理 'a 和 T: 'a
// ============================================================

#[batch_impl(<'a, T: 'a> LifetimeTrait<'a, T> &'a T)]
trait LifetimeTrait<'a, T> {}

fn test_lifetime_generic() {
    fn _check<'a, T: 'a>()
    where
        &'a T: LifetimeTrait<'a, T>,
    {
    }
    _check::<'static, i32>();
    println!("  26. lifetime generic <'a, T: 'a>: OK");
}

// ============================================================
// 27. ^ 运算符：self^T = T, &^T = &T, &mut^T = &mut T
// ============================================================

#[batch_impl(self^u32, &^u32)]
trait RefMarker {}

fn test_caret_basic() {
    fn _check<T: RefMarker>() {}
    _check::<u32>();
    _check::<&u32>();
    println!("  27. caret: self^T, &^T: OK");
}

// ============================================================
// 28. ^ 运算符：容器前缀 Box^u32 = Box<u32>
// ============================================================

#[batch_impl(Box^u32, Vec^isize)]
trait ContainerMarker {}

fn test_caret_container() {
    fn _check<T: ContainerMarker>() {}
    _check::<Box<u32>>();
    _check::<Vec<isize>>();
    println!("  28. caret: Box^u32, Vec^isize: OK");
}

// ============================================================
// 29. ^ 运算符：前缀列表 [&, self]^u32 → &u32, u32
// ============================================================

#[batch_impl([&, self]^u32)]
trait RefOrOwned {}

fn test_caret_prefix_list() {
    fn _check<T: RefOrOwned>() {}
    _check::<u32>();
    _check::<&u32>();
    println!("  29. caret: [&, self]^T: OK");
}

// ============================================================
// 30. ^ 运算符：笛卡尔积 [&, &mut]^[i32, i64]
//     注意：&mut 实现需要单独验证
// ============================================================

#[batch_impl(&^i32, &^i64)]
trait RefOnce {}

#[batch_impl(&mut^i32, &mut^i64)]
trait RefMutOnce {}

fn test_caret_cartesian() {
    fn _check_ref<T: RefOnce>() {}
    fn _check_mut<T: RefMutOnce>() {}
    _check_ref::<&i32>();
    _check_ref::<&i64>();
    _check_mut::<&mut i32>();
    _check_mut::<&mut i64>();
    println!("  30. caret: &^[i32,i64] + &mut^[i32,i64]: OK");
}

// ============================================================
// 31. ^ 运算符：多参容器 HashMap^<K,V> = HashMap<K,V>
// ============================================================

use std::collections::HashMap as MapType;

#[batch_impl(MapType^<u32, i32>)]
trait MapMarker {}

fn test_caret_multi_arg() {
    fn _check<T: MapMarker>() {}
    _check::<MapType<u32, i32>>();
    println!("  31. caret: HashMap^<K,V>: OK");
}

// ============================================================
// 32. ^ 运算符：多参容器 + 多个独立 ^ 表达式
// ============================================================

#[batch_impl(MapType^<u32, i32>, MapType^<u64, i64>)]
trait MultiMap {}

fn test_caret_multi_map_list() {
    fn _check<T: MultiMap>() {}
    _check::<MapType<u32, i32>>();
    _check::<MapType<u64, i64>>();
    println!("  32. caret: HashMap^<K,V> × 2: OK");
}

// ============================================================
// 33. ^ 运算符：综合 + 嵌套列表（全部具体类型，避免泛型悬空）
// ============================================================

#[batch_impl([[&, self]^u32, [Vec, Box]^u32])]
trait Comprehensive {}

fn test_caret_comprehensive() {
    fn _check<T: Comprehensive>() {}
    _check::<u32>();
    _check::<&u32>();
    _check::<Vec<u32>>();
    _check::<Box<u32>>();
    println!("  33. caret: comprehensive: OK");
}

// ============================================================
// 34. 嵌套 ^ 运算符：Box^Box^isize = Box<Box<isize>>
// ============================================================

#[batch_impl(Box^Box^isize)]
trait NestedBox {}

fn test_nested_caret() {
    fn _check<T: NestedBox>() {}
    _check::<Box<Box<isize>>>();
    println!("  34. nested caret Box^Box^T: OK");
}

// ============================================================
// 35. ^ 穿透 []：Box^[Box^isize] = Box<[Box<isize>]>
// ============================================================

#[batch_impl(Box^[Box^isize])]
trait NestedSlice {}

fn test_caret_through_bracket() {
    fn _check<T: NestedSlice>() {}
    _check::<Box<[Box<isize>]>>();
    println!("  35. caret through [] Box^[Box^T]: OK");
}

// ============================================================
// 36. 元组生成：()^3 = (), (A,), (A,B)
// ============================================================

#[batch_impl(()^3)]
trait GenTuple {}

fn test_tuple_gen() {
    fn _t<T: GenTuple>() {}
    _t::<()>();
    _t::<(i32,)>();
    _t::<(i32, String)>();
    println!("  36. tuple gen ()^3: OK");
}

// ============================================================
// 37. 元组重复类型：(u32,)^3 = (), (u32,), (u32,u32)
// ============================================================

#[batch_impl((u32,)^3)]
trait RepTuple {}

fn test_tuple_repeat() {
    fn _t<T: RepTuple>() {}
    _t::<()>();
    _t::<(u32,)>();
    _t::<(u32, u32)>();
    println!("  37. tuple repeat (u32,)^3: OK");
}

// ============================================================
// 38. 元组范围：()^1..3 = (A,), (A,B)
// ============================================================

#[batch_impl(()^1 .. 3)]
trait RangeTuple {}

fn test_tuple_range() {
    fn _t<T: RangeTuple>() {}
    _t::<(i32,)>();
    _t::<(i32, String)>();
    println!("  38. tuple range ()^1..3: OK");
}

// ============================================================
// 39. 元组闭区间：()^1..=3 = (A,), (A,B), (A,B,C)
// ============================================================

#[batch_impl(()^1 ..= 3)]
trait RangeIncTuple {}

fn test_tuple_range_inclusive() {
    fn _t<T: RangeIncTuple>() {}
    _t::<(i32,)>();
    _t::<(i32, String)>();
    _t::<(i32, String, u8)>();
    println!("  39. tuple range inc ()^1..=3: OK");
}

// ============================================================
// 40. 元组 bound：(<Clone>)^3 = (A:Clone,), (A:Clone,B:Clone)
// ============================================================

#[batch_impl((<Clone>)^3)]
trait BoundTuple {}

fn test_tuple_bound() {
    fn _t<T: BoundTuple>() {}
    _t::<(i32,)>();
    _t::<(i32, String)>();
    println!("  40. tuple bound (<Clone>)^3: OK");
}

// ============================================================
// 41. 元组追加：()^u32 = u32, (i32,)^u32 = (i32, u32)
// ============================================================

#[batch_impl(()^u32)]
trait AppendTupleEmpty {}

fn test_tuple_append_empty() {
    fn _t<T: AppendTupleEmpty>() {}
    _t::<(u32,)>();
    println!("  41. tuple append ()^u32 = (u32,): OK");
}

#[batch_impl((i32,)^u32)]
trait AppendTupleOne {}

fn test_tuple_append_one() {
    fn _t<T: AppendTupleOne>() {}
    _t::<(i32, u32)>();
    println!("  42. tuple append (i32,)^u32 = (i32,u32): OK");
}

// ============================================================
// 43. 元组追加 + 容器：()^[Box,Vec]^u32 = (Box<u32>,), (Vec<u32>,)
// ============================================================

#[batch_impl((i32,)^Box^u32)]
trait AppendTupleBox {}

fn test_tuple_append_box() {
    fn _t<T: AppendTupleBox>() {}
    _t::<(i32, Box<u32>)>();
    println!("  43. tuple append (i32,)^Box^u32 = (i32,Box<u32>): OK");
}

// ============================================================
// 44. 复杂嵌套：[&,Vec]^[[(),(isize)]^[Box,Rc,Arc]^[u8,i8,f32,f64,i16,u16]]
// ============================================================

#[batch_impl([&,Vec]^[[(),(isize,)]^[Box,Rc,Arc]^[u8,i8,f32,f64,i16,u16]])]
trait ComplexNest {}

fn test_complex_nest() {
    fn _check<T: ComplexNest + ?Sized>() {}
    // ()^Box<u8> = (Box<u8>,), then & and Vec wrap
    _check::<&(Box<u8>,)>();
    _check::<&(Arc<f64>,)>();
    _check::<Vec<(Box<u8>,)>>();
    _check::<Vec<(Arc<f64>,)>>();
    // (isize,)^Box<u8> = (isize, Box<u8>,), then & and Vec wrap
    _check::<&(isize, Box<u8>)>();
    _check::<&(isize, Arc<f64>)>();
    _check::<Vec<(isize, Box<u8>)>>();
    _check::<Vec<(isize, Arc<f64>)>>();
    println!("  44. complex nest: OK");
}

// ============================================================
// 44. 笛卡尔积：(u32,i32)^3 = (), (u32,), (i32,), (u32,u32), (u32,i32), (i32,u32), (i32,i32)
// ============================================================

#[batch_impl((u32,i32)^1..4)]
trait CartesianSimple {}

fn test_cartesian_simple() {
    fn _t<T: CartesianSimple>() {}
    _t::<(u32,)>();
    _t::<(i32,)>();
    _t::<(u32, u32)>();
    _t::<(u32, i32)>();
    _t::<(i32, u32)>();
    _t::<(i32, i32)>();
    _t::<(u32, u32, u32)>();
    _t::<(u32, u32, i32)>();
    _t::<(u32, i32, u32)>();
    _t::<(u32, i32, i32)>();
    _t::<(i32, u32, u32)>();
    _t::<(i32, u32, i32)>();
    _t::<(i32, i32, u32)>();
    _t::<(i32, i32, i32)>();
    println!("  44. cartesian (u32,i32)^1..4: OK");
}

// ============================================================
// 45. 笛卡尔积 + bound：(<Clone>,String)^2 = 所有 2 长度组合
// ============================================================

#[batch_impl((<Clone>,String)^1..3)]
trait CartesianBound {}

fn test_cartesian_bound() {
    fn _t<T: CartesianBound>() {}
    _t::<(String,)>();
    _t::<(i32,)>();
    _t::<(String, String)>();
    _t::<(i32, String)>();
    _t::<(String, i32)>();
    _t::<(i32, i32)>();
    println!("  45. cartesian bound (<Clone>,String)^1..3: OK");
}

// ============================================================
// 46. - 运算符：()-[usize,isize]-[u32,i32] = 左结合笛卡尔积
// ============================================================

#[batch_impl(()-[usize,isize]-[u32,i32])]
trait DashSimple {}

fn test_dash_simple() {
    fn _t<T: DashSimple>() {}
    _t::<(usize, u32)>();
    _t::<(usize, i32)>();
    _t::<(isize, u32)>();
    _t::<(isize, i32)>();
    println!("  46. dash ()-[usize,isize]-[u32,i32]: OK");
}

// ============================================================
// 47. - 运算符三维度：()-[A,B]-[C,D]-[E,F]
// ============================================================

#[batch_impl(()-[usize,isize]-[u32,i32]-[u64,i64])]
trait Dash3D {}

fn test_dash_3d() {
    fn _t<T: Dash3D>() {}
    _t::<(usize, u32, u64)>();
    _t::<(isize, i32, i64)>();
    _t::<(usize, i32, u64)>();
    println!("  47. dash 3D: OK");
}

// ============================================================
// 48. - 运算符完整测试
// ============================================================

#[batch_impl(()-[usize,isize]-[u32,i32]-[u64,i64])]
trait DashFull {}

fn test_dash_full() {
    fn _t<T: DashFull>() {}
    _t::<(usize, u32, u64)>();
    _t::<(isize, i32, i64)>();
    _t::<(usize, i32, u64)>();
    _t::<(isize, u32, i64)>();
    println!("  48. dash full: OK");
}

// ============================================================
// 49. - 链中 ^ 展开：()-[Box^u32, Vec^isize]
// ============================================================

#[batch_impl(()-[Box^u32, Vec^isize])]
trait DashCaret {}

fn test_dash_caret() {
    fn _t<T: DashCaret>() {}
    _t::<(Box<u32>,)>();
    _t::<(Vec<isize>,)>();
    println!("  49. dash with ^ expansion: OK");
}

// ============================================================
// 50. unsafe^T — 单个 spec 标记为 unsafe impl
// ============================================================

#[batch_impl(unsafe^usize, unsafe^Box<u32>, isize)]
unsafe trait UnsafePartial {}

fn test_unsafe_partial() {
    fn _t<T: UnsafePartial>() {}
    _t::<usize>();
    _t::<Box<u32>>();
    _t::<isize>();
    println!("  50. unsafe^T per-spec: OK");
}

// ============================================================
// 51. unsafe trait — 所有 impl 自动 unsafe
// ============================================================

#[batch_impl(usize, Box<u32>)]
unsafe trait UnsafeAll {
    fn do_something(&self) {}
}

fn test_unsafe_all() {
    42usize.do_something();
    Box::new(42u32).do_something();
    println!("  51. unsafe trait auto-detect: OK");
}

// ============================================================
// 52. batch_trait! with unsafe
// ============================================================

unsafe trait UnsafeBatch {
    fn batch_fn(&self) {}
}

batch_trait!(
    unsafe UnsafeBatch: usize, isize
);

fn test_unsafe_batch() {
    42usize.batch_fn();
    0isize.batch_fn();
    println!("  52. batch_trait! unsafe: OK");
}

// ============================================================
// 对比测试：auto_impl vs batch_trait 等价性
// ============================================================

// --- 基础类型 ---
trait CmpBase {}
#[batch_impl(usize)]
trait CmpAttrBase {}
batch_trait!(CmpBase: usize);

fn test_cmp_basic() {
    fn _a<T: CmpAttrBase>() {}
    fn _b<T: CmpBase>() {}
    _a::<usize>();
    _b::<usize>();
    println!("  cmp 1. basic: OK");
}

// --- 泛型 ---
trait CmpGeneric {}
#[batch_impl(<T> Vec<T>)]
trait CmpAttrGeneric {}
batch_trait!(CmpGeneric: <T> Vec<T>);

fn test_cmp_generic() {
    fn _a<T: CmpAttrGeneric>() {}
    fn _b<T: CmpGeneric>() {}
    _a::<Vec<i32>>();
    _b::<Vec<i32>>();
    println!("  cmp 2. generic: OK");
}

// --- trait 泛型 + 自定义 body ---
trait CmpTraitGen<T> {
    fn wrap(val: T) -> Self;
}
#[batch_impl(<T> CmpAttrTraitGen<T> i32 { fn wrap(_val: T) -> Self { 0 } })]
trait CmpAttrTraitGen<T> {
    fn wrap(val: T) -> Self;
}
batch_trait!(CmpTraitGen: <T> CmpTraitGen<T> i32 { fn wrap(_val: T) -> Self { 0 } });

fn test_cmp_trait_generic() {
    let _: i32 = CmpAttrTraitGen::<String>::wrap(String::new());
    let _: i32 = CmpTraitGen::<String>::wrap(String::new());
    println!("  cmp 3. trait generic + body: OK");
}

// --- 并列列表 ---
trait CmpList {
    fn tag(&self) -> &'static str;
}
#[batch_impl([u8, u16] { fn tag(&self) -> &'static str { "cmp" } })]
trait CmpAttrList {
    fn tag(&self) -> &'static str;
}
batch_trait!(CmpList: [u8, u16] { fn tag(&self) -> &'static str { "cmp" } });

fn test_cmp_list() {
    assert_eq!(CmpAttrList::tag(&0u8), "cmp");
    assert_eq!(CmpList::tag(&0u16), "cmp");
    println!("  cmp 4. list: OK");
}

// --- ^ 运算符 ---
trait CmpCaret {}
#[batch_impl(&^u32)]
trait CmpAttrCaret {}
batch_trait!(CmpCaret: &^u32);

fn test_cmp_caret() {
    fn _a<T: CmpAttrCaret>() {}
    fn _b<T: CmpCaret>() {}
    _a::<&u32>();
    _b::<&u32>();
    println!("  cmp 5. caret: OK");
}

// --- 嵌套 ^ ---
trait CmpNestedCaret {}
#[batch_impl(Box^Box^isize)]
trait CmpAttrNestedCaret {}
batch_trait!(CmpNestedCaret: Box^Box^isize);

fn test_cmp_nested_caret() {
    fn _a<T: CmpAttrNestedCaret>() {}
    fn _b<T: CmpNestedCaret>() {}
    _a::<Box<Box<isize>>>();
    _b::<Box<Box<isize>>>();
    println!("  cmp 6. nested caret: OK");
}

// --- ^ 穿透 [] ---
trait CmpCaretBracket {}
#[batch_impl(Box^[Box^isize])]
trait CmpAttrCaretBracket {}
batch_trait!(CmpCaretBracket: Box^[Box^isize]);

fn test_cmp_caret_bracket() {
    fn _a<T: CmpAttrCaretBracket>() {}
    fn _b<T: CmpCaretBracket>() {}
    _a::<Box<[Box<isize>]>>();
    _b::<Box<[Box<isize>]>>();
    println!("  cmp 7. caret through []: OK");
}

// --- 类型标注 + const 泛型 ---
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

fn test_cmp_const() {
    let _a = <[i32; 5] as CmpAttrConst<5>>::val();
    let _b = <[i32; 5] as CmpConst<5>>::val();
    assert_eq!(_a, 5);
    assert_eq!(_b, 5);
    println!("  cmp 8. const generic: OK");
}

// --- 生命周期 ---
#[allow(dead_code)]
trait CmpLifetime<'a, T> {}
#[batch_impl(<'a, T: 'a> CmpAttrLifetime<'a, T> &'a T)]
#[allow(dead_code)]
trait CmpAttrLifetime<'a, T> {}
batch_trait!(CmpLifetime: <'a, T: 'a> CmpLifetime<'a, T> &'a T);

fn test_cmp_lifetime() {
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
    println!("  cmp 9. lifetime: OK");
}

// --- 路径 trait ---
mod cmp_mod {
    pub trait PathTrait {}
}
trait CmpPath {}
#[batch_impl(u32)]
trait CmpAttrPath {}
batch_trait!(CmpPath: u32; cmp_mod::PathTrait: u32);

fn test_cmp_path() {
    fn _a<T: CmpAttrPath>() {}
    fn _b<T: CmpPath>() {}
    fn _c<T: cmp_mod::PathTrait>() {}
    _a::<u32>();
    _b::<u32>();
    _c::<u32>();
    println!("  cmp 10. path trait: OK");
}

// ============================================================
// 34. auto_impls! 宏：对已声明的 trait 生成 impl（IDE 友好）
// ============================================================

trait PlainA {}
trait PlainB {}
trait GenericC<T> {}
mod foo {
    pub trait Bar {}
}

batch_trait!(
    PlainA: usize, isize;
    PlainB: f32;
    GenericC: <T> GenericC<T> Vec<T>;
    foo::Bar: u32
);

fn test_auto_impls() {
    fn _check_a<T: PlainA>() {}
    fn _check_b<T: PlainB>() {}
    fn _check_c<T: GenericC<i32>>() {}
    fn _check_path<T: foo::Bar>() {}
    _check_a::<usize>();
    _check_a::<isize>();
    _check_b::<f32>();
    _check_c::<Vec<i32>>();
    _check_path::<u32>();
    println!("  38. auto_impls! macro (with path): OK");
}

// ============================================================

// ============================================================
// 53. unsafe^ 嵌套 ^：unsafe^Box^u32
// ============================================================

#[batch_impl(unsafe^Box^u32)]
unsafe trait UnsafeNestedCaret {}

fn test_unsafe_nested_caret() {
    fn _t<T: UnsafeNestedCaret>() {}
    _t::<Box<u32>>();
    println!("  53. unsafe^Box^u32: OK");
}

// ============================================================
// 54. unsafe^ 列表：[unsafe^usize, unsafe^isize, u32]
// ============================================================

#[batch_impl([unsafe^usize, unsafe^isize, u32])]
unsafe trait UnsafeList {}

fn test_unsafe_list() {
    fn _t<T: UnsafeList>() {}
    _t::<usize>();
    _t::<isize>();
    _t::<u32>();
    println!("  54. unsafe list: OK");
}

// ============================================================
// 55. unsafe^ 与 & 组合：unsafe^&^u32
// ============================================================

#[batch_impl(unsafe^&^u32)]
unsafe trait UnsafeRef {}

fn test_unsafe_ref() {
    fn _t<T: UnsafeRef>() {}
    _t::<&u32>();
    println!("  55. unsafe^&^u32: OK");
}

// ============================================================
// 56. unsafe^ 与 &mut 组合：unsafe^&mut^u32
// ============================================================

#[batch_impl(unsafe^&mut^u32)]
unsafe trait UnsafeRefMut {}

fn test_unsafe_ref_mut() {
    fn _t<T: UnsafeRefMut>() {}
    _t::<&mut u32>();
    println!("  56. unsafe^&mut^u32: OK");
}

// ============================================================
// 57. batch_trait! 多 trait 部分 unsafe
// ============================================================

trait BatchA {}
unsafe trait BatchB {}
trait BatchC {}

batch_trait!(
    BatchA: usize, isize;
    unsafe BatchB: u32, u64;
    BatchC: f32
);

fn test_batch_partial_unsafe() {
    fn _a<T: BatchA>() {}
    fn _b<T: BatchB>() {}
    fn _c<T: BatchC>() {}
    _a::<usize>();
    _b::<u32>();
    _c::<f32>();
    println!("  57. batch_trait! partial unsafe: OK");
}

// ============================================================
// 58. unsafe^ 与 vec/box 容器组合
// ============================================================

#[batch_impl(unsafe^[Box, Vec]^u32)]
unsafe trait UnsafeContainerList {}

fn test_unsafe_container_list() {
    fn _t<T: UnsafeContainerList>() {}
    _t::<Box<u32>>();
    _t::<Vec<u32>>();
    println!("  58. unsafe^[Box,Vec]^u32: OK");
}

// ============================================================
// 59. unsafe^ 与元组生成：unsafe^()^3
// ============================================================

#[batch_impl(unsafe^()^3)]
unsafe trait UnsafeTupleGen {}

fn test_unsafe_tuple_gen() {
    fn _t<T: UnsafeTupleGen>() {}
    _t::<()>();
    _t::<(i32,)>();
    _t::<(i32, String)>();
    println!("  59. unsafe^()^3: OK");
}

// ============================================================
// 60. unsafe^ 与元组重复类型：unsafe^(u32,)^3
// ============================================================

#[batch_impl(unsafe^(u32,)^3)]
unsafe trait UnsafeTupleRepeat {}

fn test_unsafe_tuple_repeat() {
    fn _t<T: UnsafeTupleRepeat>() {}
    _t::<()>();
    _t::<(u32,)>();
    _t::<(u32, u32)>();
    println!("  60. unsafe^(u32,)^3: OK");
}

// ============================================================
// 61. unsafe^ 与元组追加：unsafe^()^u32
// ============================================================

#[batch_impl(unsafe^()^u32)]
unsafe trait UnsafeTupleAppend {}

fn test_unsafe_tuple_append() {
    fn _t<T: UnsafeTupleAppend>() {}
    _t::<(u32,)>();
    println!("  61. unsafe^()^u32: OK");
}

// ============================================================
// 62. unsafe^ 与元组 bound：unsafe^(<Clone>)^3
// ============================================================

#[batch_impl(unsafe^(<Clone>)^3)]
unsafe trait UnsafeTupleBound {}

fn test_unsafe_tuple_bound() {
    fn _t<T: UnsafeTupleBound>() {}
    _t::<(i32,)>();
    _t::<(i32, String)>();
    println!("  62. unsafe^(<Clone>)^3: OK");
}

// ============================================================
// 63. unsafe^ 与 - 运算符组合
// ============================================================

#[batch_impl(unsafe^()-[usize, isize]-[u32, i32])]
unsafe trait UnsafeDash {}

fn test_unsafe_dash() {
    fn _t<T: UnsafeDash>() {}
    _t::<(usize, u32)>();
    _t::<(usize, i32)>();
    _t::<(isize, u32)>();
    _t::<(isize, i32)>();
    println!("  63. unsafe^()-[A,B]-[C,D]: OK");
}

// ============================================================
// 64. unsafe^ 与泛型：<T> unsafe^Vec<T>
// ============================================================

#[batch_impl(<T> unsafe^Vec<T>)]
unsafe trait UnsafeGenericVec {}

fn test_unsafe_generic_vec() {
    fn _t<T: UnsafeGenericVec>() {}
    _t::<Vec<i32>>();
    _t::<Vec<String>>();
    println!("  64. <T> unsafe^Vec<T>: OK");
}

// ============================================================
// 65. unsafe^ 与 trait 泛型：<T> UnsafeTrait<T> unsafe^i32
// ============================================================

#[batch_impl(<T> UnsafeTraitGen<T> unsafe^i32 {
    fn wrap(_val: T) -> Self { 0 }
})]
unsafe trait UnsafeTraitGen<T> {
    fn wrap(val: T) -> Self;
}

fn test_unsafe_trait_gen() {
    let x: i32 = UnsafeTraitGen::<String>::wrap(String::from("hi"));
    assert_eq!(x, 0);
    println!("  65. <T> Trait<T> unsafe^i32: OK");
}

// ============================================================
// 特殊类型测试
// ============================================================

// 函数类型 Fn(A, B) -> C
#[batch_impl(fn(i32) -> bool, fn(String, &str) -> usize)]
trait FnMarker {}

fn test_fn_types() {
    fn _check<T: FnMarker>() {}
    _check::<fn(i32) -> bool>();
    _check::<fn(String, &str) -> usize>();
    println!("  special 1. fn types: OK");
}

// dyn trait 对象
#[batch_impl(
    dyn std::fmt::Display,
    dyn std::fmt::Debug + Send,
    dyn std::fmt::Display + Send + Sync
)]
trait DynMarker2 {}

fn test_dyn_types() {
    fn _check<T: DynMarker2 + ?Sized>() {}
    _check::<dyn std::fmt::Display>();
    _check::<dyn std::fmt::Debug + Send>();
    _check::<dyn std::fmt::Display + Send + Sync>();
    println!("  special 2. dyn types: OK");
}

// 元组类型
#[batch_impl(
    (i32,),
    (i32, String),
    (i32, String, bool),
    ()
)]
trait TupleMarker {}

fn test_tuple_types() {
    fn _check<T: TupleMarker>() {}
    _check::<(i32,)>();
    _check::<(i32, String)>();
    _check::<(i32, String, bool)>();
    _check::<()>();
    println!("  special 3. tuple types: OK");
}

// 引用类型
#[batch_impl(&i32, &mut String, &str, &[u8])]
trait RefMarker2 {}

fn test_ref_types() {
    fn _check<T: RefMarker2>() {}
    _check::<&i32>();
    _check::<&mut String>();
    _check::<&str>();
    _check::<&[u8]>();
    println!("  special 4. ref types: OK");
}

// 裸指针类型
#[batch_impl(*const i32, *mut u8)]
trait PtrMarker {}

fn test_pointer_types() {
    fn _check<T: PtrMarker>() {}
    _check::<*const i32>();
    _check::<*mut u8>();
    println!("  special 5. pointer types: OK");
}

// 嵌套特殊类型
#[batch_impl(
    Box<dyn std::fmt::Display>,
    Vec<fn(i32) -> bool>,
    &(dyn std::fmt::Debug + Send),
    Option<&mut String>,
    *const Vec<u8>,
    fn(&str) -> Vec<i32>,
    (i32, &dyn std::fmt::Display),
    Box<Vec<&str>>
)]
trait NestedSpecialMarker {}

fn test_nested_special_types() {
    fn _check<T: NestedSpecialMarker>() {}
    _check::<Box<dyn std::fmt::Display>>();
    _check::<Vec<fn(i32) -> bool>>();
    _check::<&(dyn std::fmt::Debug + Send)>();
    _check::<Option<&mut String>>();
    _check::<*const Vec<u8>>();
    _check::<fn(&str) -> Vec<i32>>();
    _check::<(i32, &dyn std::fmt::Display)>();
    _check::<Box<Vec<&str>>>();
    println!("  special 6. nested special types: OK");
}

// ============================================================

fn main() {
    println!("=== auto_impl macro tests ===");
    println!("=== auto_impl macro tests ===");
    test_basic();
    test_generic();
    test_trait_with_generic();
    test_custom_body();
    test_generic_list();
    test_multi_custom();
    test_trait_generic_list();
    test_slice_type();
    test_nested();
    test_multi_specs_with_body();
    test_complex_types();
    test_nested_angle_brackets();
    test_path_type();
    test_trait_name_as_target();
    test_multi_trait_params();
    test_nested_bracket_list();
    test_slice_with_body();
    test_plain_ident();
    test_dyn_trait_object();
    test_box_generic();
    test_complex_trait_generic();
    test_multi_complex_trait_generic();
    test_type_bound();
    test_const_generic();
    test_mixed_generics();
    test_lifetime_generic();
    test_caret_basic();
    test_caret_container();
    test_caret_prefix_list();
    test_caret_cartesian();
    test_caret_multi_arg();
    test_caret_multi_map_list();
    test_caret_comprehensive();
    test_nested_caret();
    test_caret_through_bracket();
    test_tuple_gen();
    test_tuple_repeat();
    test_tuple_range();
    test_tuple_range_inclusive();
    test_tuple_bound();
    test_tuple_append_empty();
    test_tuple_append_one();
    test_tuple_append_box();
    test_complex_nest();
    test_cartesian_simple();
    test_cartesian_bound();
    test_dash_simple();
    test_dash_3d();
    test_dash_full();
    test_dash_caret();
    test_unsafe_partial();
    test_unsafe_all();
    test_unsafe_batch();
    test_auto_impls();
    test_unsafe_nested_caret();
    test_unsafe_list();
    test_unsafe_ref();
    test_unsafe_ref_mut();
    test_batch_partial_unsafe();
    test_unsafe_container_list();
    test_unsafe_tuple_gen();
    test_unsafe_tuple_repeat();
    test_unsafe_tuple_append();
    test_unsafe_tuple_bound();
    test_unsafe_dash();
    test_unsafe_generic_vec();
    test_unsafe_trait_gen();
    test_fn_types();
    test_dyn_types();
    test_tuple_types();
    test_ref_types();
    test_pointer_types();
    test_nested_special_types();
    println!("\n--- comparison tests ---");
    test_cmp_basic();
    test_cmp_generic();
    test_cmp_trait_generic();
    test_cmp_list();
    test_cmp_caret();
    test_cmp_nested_caret();
    test_cmp_caret_bracket();
    test_cmp_const();
    test_cmp_lifetime();
    test_cmp_path();
    println!("\nAll tests passed!");
}
