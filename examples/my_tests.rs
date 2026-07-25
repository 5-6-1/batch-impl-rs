use batch_impl::*;

// ============================================================
// 分类 1：基础指令测试
// ============================================================

// 1. #method{body} — 单方法简写
#[batch_impl(usize #t1a{"directive"})]
trait D01Method { fn t1a(&self) -> &str; }

fn test_01_method() {
    assert_eq!(42usize.t1a(), "directive");
    println!("  1. #method{{body}}: OK");
}

// 2. #fill(m1,m2){body} — 多方法填充同一 body
#[batch_impl(usize #fill(t2a, t2b){"filled"})]
trait D02Fill { fn t2a(&self) -> &str; fn t2b(&self) -> &str; }

fn test_02_fill() {
    assert_eq!(42usize.t2a(), "filled");
    assert_eq!(42usize.t2b(), "filled");
    println!("  2. #fill(m1,m2){{body}}: OK");
}

// 3. #fill(#all){body} — 填充全部方法
#[batch_impl(usize #fill(#all){"all_same"})]
trait D03FillAll { fn t3a(&self) -> &str; fn t3b(&self) -> &str; }

fn test_03_fill_all() {
    assert_eq!(42usize.t3a(), "all_same");
    assert_eq!(42usize.t3b(), "all_same");
    println!("  3. #fill(#all){{body}}: OK");
}

// ============================================================
// 分类 2：指令 + 运算符（^ / -）
// ============================================================

// 4. #method + ^ 运算符
#[batch_impl(Box^u32 #t4a{"boxed_caret"})]
trait D04Caret { fn t4a(&self) -> &str; }

fn test_04_caret() {
    let b: Box<u32> = Box::new(42);
    assert_eq!(b.t4a(), "boxed_caret");
    println!("  4. Box^T #method: OK");
}

// 5. #method + - 运算符
#[batch_impl(()-usize #t5a{"dash_method"})]
trait D05Dash { fn t5a(&self) -> &str; }

fn test_05_dash() {
    let t: (usize,) = (42,);
    assert_eq!(t.t5a(), "dash_method");
    println!("  5. ()-T #method: OK");
}

// 6. #fill + ^ + bracket 列表
#[batch_impl([Box, Vec]^u32 #fill(t6a, t6b){"fill_caret"})]
trait D06FillCaret { fn t6a(&self) -> &str; fn t6b(&self) -> &str; }

fn test_06_fill_caret() {
    let b: Box<u32> = Box::new(42);
    assert_eq!(b.t6a(), "fill_caret");
    assert_eq!(b.t6b(), "fill_caret");
    let v: Vec<u32> = vec![1, 2, 3];
    assert_eq!(v.t6a(), "fill_caret");
    assert_eq!(v.t6b(), "fill_caret");
    println!("  6. [Box,Vec]^T #fill: OK");
}

// 7. #method + - 运算符 + bracket
#[batch_impl(()-[Box^u32, Vec^isize] #t7a{"dash_bracket"})]
trait D07DashBracket { fn t7a(&self) -> &str; }

fn test_07_dash_bracket() {
    let t1 = (Box::new(42u32),);
    assert_eq!(t1.t7a(), "dash_bracket");
    let t2 = (vec![1isize, 2],);
    assert_eq!(t2.t7a(), "dash_bracket");
    println!("  7. ()-[Box^u32,Vec^isize] #method: OK");
}

// ============================================================
// 分类 3：指令 + 修饰符
// ============================================================

// 8. #method + unsafe
#[batch_impl(unsafe^usize #t8a{"unsafe_dir"})]
unsafe trait D08Unsafe { fn t8a(&self) -> &str; }

fn test_08_unsafe() {
    assert_eq!(42usize.t8a(), "unsafe_dir");
    println!("  8. unsafe^T #method: OK");
}

// 9. #method + impl 泛型
#[batch_impl(<T> Vec<T> #t9a{"generic_method"})]
trait D09Generic { fn t9a(&self) -> &str; }

fn test_09_generic() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.t9a(), "generic_method");
    println!("  9. <T> Vec<T> #method: OK");
}

// 10. #method + trait 泛型参数
#[batch_impl(<T> D10Trait<T> Vec<T> #t10a{42})]
trait D10Trait<T> { fn t10a(&self) -> usize; }

fn test_10_trait_generic() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.t10a(), 42);
    println!("  10. <T> D10Trait<T> Vec<T> #method: OK");
}

// 11. #method + const 泛型
#[batch_impl(<const N: usize> [u32; N] #t11a{N})]
trait D11Const { fn t11a(&self) -> usize; }

fn test_11_const() {
    let arr: [u32; 5] = [1, 2, 3, 4, 5];
    assert_eq!(arr.t11a(), 5);
    println!("  11. <const N:usize> [u32;N] #method: OK");
}

// ============================================================
// 分类 4：指令 + 列表 & body 交互
// ============================================================

// 12. bracket 内各自带 #fill(#all)，不同 body 值
#[batch_impl([usize #fill(#all){"usize_body"}, isize #fill(#all){"isize_body"}])]
trait D12Bracket { fn t12a(&self) -> &str; fn t12b(&self) -> &str; }

fn test_12_bracket() {
    assert_eq!(42usize.t12a(), "usize_body");
    assert_eq!(42usize.t12b(), "usize_body");
    assert_eq!(42isize.t12a(), "isize_body");
    assert_eq!(42isize.t12b(), "isize_body");
    println!("  12. [T #fill(#all), T #fill(#all)]: OK");
}

// 13. 指令 + {body} 连续附着
#[batch_impl(usize #t13a{"directive_body"} { fn t13b(&self) -> i32 { 42 } })]
trait D13ContAttach { fn t13a(&self) -> &str; fn t13b(&self) -> i32; }

fn test_13_continuous_attach() {
    assert_eq!(42usize.t13a(), "directive_body");
    assert_eq!(42usize.t13b(), 42);
    println!("  13. #method + {{body}} continuous attach: OK");
}

// 14. 同一 spec 上多个指令
#[batch_impl(usize #t14a{"first"} #t14b{"second"})]
trait D14Multi { fn t14a(&self) -> &str; fn t14b(&self) -> &str; }

fn test_14_multi_directive() {
    assert_eq!(42usize.t14a(), "first");
    assert_eq!(42usize.t14b(), "second");
    println!("  14. multiple #directives on one spec: OK");
}

// ============================================================
// 分类 5：指令 + 复杂 DSL 组合
// ============================================================

// 15. #fill(#all) + 元组展开 ()^N
#[batch_impl(()^3 #fill(#all){"tuple_fill"})]
trait D15Tuple { fn t15a(&self) -> &str; }

fn test_15_tuple_fill() {
    let t = (42u32, "hello", false);
    assert_eq!(t.t15a(), "tuple_fill");
    println!("  15. ()^N #fill(#all): OK");
}

// 16. 嵌套列表内嵌指令
#[batch_impl([
    [usize #fill(t16a, t16b){"nested"},],
    isize { fn t16a(&self) -> &str { "isize_a" } fn t16b(&self) -> &str { "isize_b" } }
])]
trait D16Nested { fn t16a(&self) -> &str; fn t16b(&self) -> &str; }

fn test_16_nested_directive() {
    assert_eq!(42usize.t16a(), "nested");
    assert_eq!(42usize.t16b(), "nested");
    assert_eq!(42isize.t16a(), "isize_a");
    assert_eq!(42isize.t16b(), "isize_b");
    println!("  16. nested list [[T #fill,], T {{body}}]: OK");
}

// 17. 泛型 + unsafe + ^ + bracket + 指令全组合
#[batch_impl(<T: Clone> D17All<T> unsafe^[Box, Vec]^T #t17a{"all_combo"})]
unsafe trait D17All<T> { fn t17a(&self) -> &str; }

fn test_17_all_combo() {
    let b: Box<i32> = Box::new(42);
    assert_eq!(b.t17a(), "all_combo");
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.t17a(), "all_combo");
    println!("  17. generics + unsafe + ^ + bracket + #method: OK");
}

// ============================================================
// 分类 6：边界 / 参数传递
// ============================================================

// 18. #method 对应带参数的方法
#[batch_impl(usize #t18a{*self + other})]
trait D18Param { fn t18a(&self, other: usize) -> usize; }

fn test_18_param_method() {
    assert_eq!(42usize.t18a(8), 50);
    println!("  18. #method with parameter: OK");
}

// 19. #method 对应多参数方法
#[batch_impl(usize #t19a{*self + a + b})]
trait D19MultiParam { fn t19a(&self, a: usize, b: usize) -> usize; }

fn test_19_multi_param() {
    assert_eq!(10usize.t19a(20, 30), 60);
    println!("  19. #method with multiple params: OK");
}

// ============================================================
// 补充：更复杂的交叉场景
// ============================================================

// 20. #fill + 长 dash 链
#[batch_impl(()-usize-isize #fill(#all){"dash_chain_fill"})]
trait D20DashChain { fn t20a(&self) -> &str; }

fn test_20_dash_chain() {
    let t = (42usize, 0isize);
    assert_eq!(t.t20a(), "dash_chain_fill");
    println!("  20. ()-usize-isize #fill(#all): OK");
}

// 21. #fill(#all) + unsafe trait + 泛型
#[batch_impl(<T: Clone> D21Unsafe<T> unsafe^Vec<T> #fill(#all){"unsafe_fill"})]
unsafe trait D21Unsafe<T> { fn t21a(&self) -> &str; }

fn test_21_unsafe_fill() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.t21a(), "unsafe_fill");
    println!("  21. <T> D21Unsafe<T> unsafe^Vec<T> #fill(#all): OK");
}

// 22. #fill(#all) + 范围元组 ()^N..M
#[batch_impl(()^2..4 #fill(#all){"range_fill"})]
trait D22Range { fn t22a(&self) -> &str; }

fn test_22_range() {
    let t2 = (42u32, "hello");
    assert_eq!(t2.t22a(), "range_fill");
    let t3 = (42u32, "hello", true);
    assert_eq!(t3.t22a(), "range_fill");
    println!("  22. ()^N..M #fill(#all): OK");
}

// 23. 引用类型 + #method
#[batch_impl(&^u32 #t23a{"ref_dir"})]
trait D23Ref { fn t23a(&self) -> &str; }

fn test_23_ref() {
    let x: u32 = 42;
    assert_eq!((&x).t23a(), "ref_dir");
    println!("  23. &^T #method: OK");
}

// 24. fn 类型 + 指令
#[batch_impl(fn^(u32, i32) #t24a{"fn_dir"} { fn t24b(&self) -> i32 { 0 } })]
trait D24Fn { fn t24a(&self) -> &str; fn t24b(&self) -> i32; }

fn test_24_fn() {
    // fn 指针类型不能直接创建值来调用，但 trait bound 验证了生成正确
    fn _check<T: D24Fn>() {}
    _check::<fn(u32, i32)>();
    println!("  24. fn^(A,B) #method + {{body}}: OK");
}

// 25. 关联类型绑定 + 指令
#[batch_impl(<T: Clone> D25Assoc<Item=T> Vec<T> #t25a{42})]
trait D25Assoc { type Item; fn t25a(&self) -> usize; }

fn test_25_assoc() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.t25a(), 42);
    println!("  25. assoc type binding + #method: OK");
}

fn main() {
    println!("=== my_tests: 指令系统测试 ===\n");
    test_01_method();
    test_02_fill();
    test_03_fill_all();
    test_04_caret();
    test_05_dash();
    test_06_fill_caret();
    test_07_dash_bracket();
    test_08_unsafe();
    test_09_generic();
    test_10_trait_generic();
    test_11_const();
    test_12_bracket();
    test_13_continuous_attach();
    test_14_multi_directive();
    test_15_tuple_fill();
    test_16_nested_directive();
    test_17_all_combo();
    test_18_param_method();
    test_19_multi_param();
    test_20_dash_chain();
    test_21_unsafe_fill();
    test_22_range();
    test_23_ref();
    test_24_fn();
    test_25_assoc();
    println!("\n=== delegate 测试 ===");
    test_26_delegate();
    test_27_delegate_tuple();
    test_28_delegate_mismatch();
    test_29_delegate_all();
    test_30_blanket_impl();
    test_31_arc_delegate();
    test_32_tuple_multi_del();
    test_33_rc_delegate();
    test_34_trait_generic_del();
    test_35_multi_spec_del();
    test_36_unsafe_delegate();
    println!("\nAll 36 directive tests passed!");
}

// ============================================================
// #delegate 测试
// ============================================================

// 26. #delegate(d26_len){**self} — Box<Vec<u32>> 委托到 Vec::len()
#[batch_impl(Vec<u32>#d26_len{self.len()},Box^Vec^u32 #delegate(d26_len){**self})]
trait D26Delegate { fn d26_len(&self) -> usize; }

fn test_26_delegate() {
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.d26_len(), 3);
    println!("  26. #delegate(d26_len){{**self}} on Box<Vec<u32>>: OK");
}

// 27. #delegate(d27_len){self.0} — 元组字段委托
#[batch_impl(Vec<u32>#d27_len{self.len()},()-Box^Vec^u32 #delegate(d27_len){self.0})]
trait D27DelTupleMove { fn d27_len(&self) -> usize; }

fn test_27_delegate_tuple() {
    let t = (Box::new(vec![1u32, 2, 3]),);
    assert_eq!(t.d27_len(), 3);
    println!("  27. #delegate(d27_len){{self.0}} on tuple: OK");
}

// 28. #method + #delegate — Vec<u32> 用 #method，Box<Vec<u32>> 用 #delegate
#[batch_impl(Vec^u32 #d28_len{self.len()}, Box^Vec^u32 #delegate(d28_len){**self})]
trait D28DelNameMismatch { fn d28_len(&self) -> usize; }

fn test_28_delegate_mismatch() {
    let v: Vec<u32> = vec![1, 2, 3];
    assert_eq!(v.d28_len(), 3);
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.d28_len(), 3);
    println!("  28. #method + #delegate(d28_len){{**self}}: OK");
}

// 29. #delegate(#all){self} — 委托全部方法
#[batch_impl(String #d29_len{self.len()}#d29_is_empty{self.is_empty()},Box^String #delegate(#all){**self})]
trait D29DelAll {
    fn d29_len(&self) -> usize;
    fn d29_is_empty(&self) -> bool;
}

fn test_29_delegate_all() {
    let b: Box<String> = Box::new(String::from("hello"));
    assert_eq!(b.d29_len(), 5);
    assert!(!b.d29_is_empty());
    let empty: Box<String> = Box::new(String::new());
    assert!(empty.d29_is_empty());
    println!("  29. #delegate(#all){{**self}}: OK");
}

// ============================================================
// delegate 常用模式
// ============================================================

// 30. 经典 blanket impl 模式 — 具体类型 + 引用委托
//     i32 用 #method，&T (T: D30ToI32) 用 #delegate
#[batch_impl(i32 #to_i32{*self}, <T: D30ToI32> &T #delegate(to_i32){**self})]
trait D30ToI32 { fn to_i32(&self) -> i32; }

fn test_30_blanket_impl() {
    assert_eq!(42i32.to_i32(), 42);
    assert_eq!((&42i32).to_i32(), 42);
    assert_eq!((&100i32).to_i32(), 100);
    println!("  30. blanket impl #method + #delegate on &T: OK");
}

// 31. Arc 智能指针委托
use std::sync::Arc;

#[batch_impl(String #d31_len{self.len()},Arc^String #delegate(d31_len){**self})]
trait D31ArcDelegate { fn d31_len(&self) -> usize; }

fn test_31_arc_delegate() {
    let a: Arc<String> = Arc::new(String::from("hello world"));
    assert_eq!(a.d31_len(), 11);
    println!("  31. Arc<T> #delegate: OK");
}

// 32. 元组 + 多方法委托 — 委托 d32_len, d32_is_empty 到 self.0
#[batch_impl(Vec<u32>#d32_len{self.len()}#d32_is_empty{self.is_empty()},()-Box^Vec^u32 #delegate(#all){self.0})]
trait D32TupleMulti {
    fn d32_len(&self) -> usize;
    fn d32_is_empty(&self) -> bool;
}

fn test_32_tuple_multi_del() {
    let t = (Box::new(vec![1u32, 2, 3]),);
    assert_eq!(t.d32_len(), 3);
    assert!(!t.d32_is_empty());
    let empty = (Box::new(vec![]),);
    assert!(empty.d32_is_empty());
    println!("  32. ()-Box^Vec^T #delegate(m1,m2){{self.0}}: OK");
}

// 33. Rc 智能指针 + 带参数委托
use std::rc::Rc;

#[batch_impl(String #d33_contains{self.contains(pat)},Rc^String #delegate(d33_contains){**self})]
trait D33RcDelegate { fn d33_contains(&self, pat: &str) -> bool; }

fn test_33_rc_delegate() {
    let r: Rc<String> = Rc::new(String::from("hello world"));
    assert!(r.d33_contains("world"));
    assert!(!r.d33_contains("rust"));
    println!("  33. Rc<T> #delegate with param: OK");
}

// 34. delegate + trait 泛型 — 泛型容器的 len 委托
#[batch_impl(<T>D34Map<T>Vec<T>#d34_len{self.len()},<T> D34Map<T> Box^Vec^T #delegate(d34_len){**self})]
trait D34Map<T> { fn d34_len(&self) -> usize; }

fn test_34_trait_generic_del() {
    let b: Box<Vec<i32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.d34_len(), 3);
    let b2: Box<Vec<String>> = Box::new(vec![String::from("a"), String::from("b")]);
    assert_eq!(b2.d34_len(), 2);
    println!("  34. <T> D34Map<T> Box^Vec^T #delegate: OK");
}

// 35. 多个 spec 各自用 delegate 到不同类型
#[batch_impl(
    [Vec^u32,String]#d35_len{self.len()},
    Box^Vec^u32 #delegate(d35_len){**self},
    Box^String #delegate(d35_len){**self}
)]
trait D35MultiDel { fn d35_len(&self) -> usize; }

fn test_35_multi_spec_del() {
    let v: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(v.d35_len(), 3);
    let s: Box<String> = Box::new(String::from("hi"));
    assert_eq!(s.d35_len(), 2);
    println!("  35. Box^Vec<T> + Box^String both #delegate: OK");
}

// 36. unsafe + delegate
#[batch_impl(Vec<u32>#d36_len{self.len()},Box^Vec^u32 #delegate(d36_len){**self})]
unsafe trait D36UnsafeDel { fn d36_len(&self) -> usize; }

fn test_36_unsafe_delegate() {
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.d36_len(), 3);
    println!("  36. unsafe^Box^Vec^T #delegate: OK");
}
