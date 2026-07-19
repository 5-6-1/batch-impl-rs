// ===========================================================================
// ds-test: mimo 的 auto_impl 审查测试
// 仅测试原始代码，不修改 auto-impl
// ===========================================================================
use batch_impl::batch_impl;

fn main() {
    println!("=== ds-test: mimo 代码审查 ===\n");
    test_boundary_cases();
    test_caret_nesting();
    test_cartesian();
    test_append_tuple();
    test_dash();
    println!("\nAll ds-tests passed!");
}

// ============================================================
// 1. 边界测试：^ 与 bracket 的交互
// ============================================================

fn test_boundary_cases() {
    // 1a: Box^[Box^isize] 展开为 Box<[Box<isize>]>（非 Box<Box<isize>>）
    #[batch_impl(Box^[Box^isize])]
    trait S1a {}
    fn _1a<T: S1a>() {}
    _1a::<Box<[Box<isize>]>>();
    println!("  1a. Box^[Box^isize] = Box<[Box<isize>]>: OK");

    // 1b: ^ 右侧 bracket 同时含逗号和 ^
    #[batch_impl([Box, Vec]^[&^u32, u64])]
    trait S1b {}
    fn _1b<T: S1b>() {}
    _1b::<Box<&u32>>();
    _1b::<Box<u64>>();
    _1b::<Vec<&u32>>();
    _1b::<Vec<u64>>();
    println!("  1b. [Box,Vec]^[&^u32, u64] = 4 impls: OK");
}

// ============================================================
// 2. ^ 运算符深度嵌套
// ============================================================

fn test_caret_nesting() {
    // 2a: 三层嵌套 ^
    #[batch_impl(Box^Box^Box^u32)]
    trait S2a {}
    fn _2a<T: S2a>() {}
    _2a::<Box<Box<Box<u32>>>>();
    println!("  2a. Box^Box^Box^u32: OK");

    // 2b: 两侧都是 bracket（笛卡尔积）
    #[batch_impl([Box, Vec]^[u32, i64])]
    trait S2b {}
    fn _2b<T: S2b>() {}
    _2b::<Box<u32>>();
    _2b::<Box<i64>>();
    _2b::<Vec<u32>>();
    _2b::<Vec<i64>>();
    println!("  2b. [Box,Vec]^[u32,i64] = 4 impls: OK");

    // 2c: 左侧含 self
    #[batch_impl([&, self]^[u32, i64])]
    trait S2c {}
    fn _2c<T: S2c>() {}
    _2c::<&u32>();
    _2c::<&i64>();
    _2c::<u32>();
    _2c::<i64>();
    println!("  2c. [&,self]^[u32,i64] = 4 impls: OK");

    // 2d: ^ 右侧多参
    #[batch_impl(HashMap^<u32, String>)]
    trait S2d {}
    fn _2d<T: S2d>() {}
    _2d::<HashMap<u32, String>>();
    println!("  2d. HashMap^<u32,String>: OK");

    // 2e: Vec^[u32, i64]
    #[batch_impl(Vec^[u32, i64])]
    trait S2e {}
    fn _2e<T: S2e>() {}
    _2e::<Vec<u32>>();
    _2e::<Vec<i64>>();
    println!("  2e. Vec^[u32,i64] = 2 impls: OK");
}

// ============================================================
// 3. 笛卡尔积元组
// ============================================================

fn test_cartesian() {
    // 3a: 双类型笛卡尔积 — 使用 (u32,i32)^3
    #[batch_impl((u32, i32)^3)]
    trait S3a {}
    fn _3a<T: S3a>() {}
    // (u32,i32)^3 生成长度为3的所有组合
    _3a::<(u32, u32, u32)>();
    _3a::<(u32, u32, i32)>();
    _3a::<(u32, i32, u32)>();
    _3a::<(u32, i32, i32)>();
    _3a::<(i32, u32, u32)>();
    _3a::<(i32, u32, i32)>();
    _3a::<(i32, i32, u32)>();
    _3a::<(i32, i32, i32)>();
    println!("  3a. (u32,i32)^3 = 8 impls: OK");

    // 3b: 单类型重复 — (u32,)^3（注意逗号）
    #[batch_impl((u32,)^3)]
    trait S3b {}
    fn _3b<T: S3b>() {}
    // (u32,)^3 生成 (u32,u32,u32)
    _3b::<(u32, u32, u32)>();
    println!("  3b. (u32,)^3 = (u32,u32,u32): OK");

    // 3c: 泛型生成 ()^3
    #[batch_impl(()^3)]
    trait S3c {}
    fn _3c<T: S3c>() {}
    // ()^3 生成带3个泛型参数的元组
    _3c::<(i32, i32, i32)>();
    println!("  3c. ()^3 = (A,B,C): OK");
}

// ============================================================
// 4. 元组追加
// ============================================================

fn test_append_tuple() {
    // 4a: ()^u32 = (u32,)
    #[batch_impl(()^u32)]
    trait S4a {}
    fn _4a<T: S4a>() {}
    _4a::<(u32,)>();
    println!("  4a. ()^u32 = (u32,): OK");

    // 4b: (i32,)^u32 = (i32,u32)
    #[batch_impl((i32,)^u32)]
    trait S4b {}
    fn _4b<T: S4b>() {}
    _4b::<(i32, u32)>();
    println!("  4b. (i32,)^u32 = (i32,u32): OK");

    // 4c: ()^Box^u32 = (Box<u32>,)
    #[batch_impl(()^Box^u32)]
    trait S4c {}
    fn _4c<T: S4c>() {}
    _4c::<(Box<u32>,)>();
    println!("  4c. ()^Box^u32 = (Box<u32>,): OK");
}

// ============================================================
// 5. - 运算符
// ============================================================

fn test_dash() {
    // 5a: 基础 - 元组构建
    #[batch_impl(()-[usize, isize]-[u32, i32])]
    trait S5a {}
    fn _5a<T: S5a>() {}
    _5a::<(usize, u32)>();
    _5a::<(usize, i32)>();
    _5a::<(isize, u32)>();
    _5a::<(isize, i32)>();
    println!("  5a. ()-[usize,isize]-[u32,i32] = 4 impls: OK");

    // 5b: - 链中的 ^ 展开（mimo 新增修复）
    #[batch_impl(()-[Box^u32, Vec^isize]-[i64, i32])]
    trait S5b {}
    fn _5b<T: S5b>() {}
    _5b::<(Box<u32>, i64)>();
    _5b::<(Box<u32>, i32)>();
    _5b::<(Vec<isize>, i64)>();
    _5b::<(Vec<isize>, i32)>();
    println!("  5b. ()-[Box^u32,Vec^isize]-[i64,i32] = 4 impls: OK");
}

// ============================================================
// 辅助
// ============================================================
use std::collections::HashMap;
