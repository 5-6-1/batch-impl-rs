// trybuild UI 测试：锁定错误信息的中文措辞与行为。
//
// 运行：`cargo test --test ui`
// 重新生成快照：`TRYBUILD=overwrite cargo test --test ui`

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // README "错误提示" 表中的核心诊断
    t.compile_fail("tests/ui/only_semicolon.rs");

    t.compile_fail("tests/ui/missing_colon.rs");

    t.compile_fail("tests/ui/trait_path_no_ident.rs");
    t.compile_fail("tests/ui/path_prefix_mismatch.rs");

    // DSL 语义错误
    t.compile_fail("tests/ui/num_as_left_operand.rs");

    // 指令系统错误
    t.compile_fail("tests/ui/directive_bad_follow.rs");
    t.compile_fail("tests/ui/fill_empty_args.rs");
    t.compile_fail("tests/ui/fill_bad_comma.rs");
    t.compile_fail("tests/ui/single_name_not_found.rs");
    t.compile_fail("tests/ui/delegate_on_non_fn.rs");

    // #delegate 解构模式参数无法转发
    t.compile_fail("tests/ui/delegate_pattern_arg.rs");

    // DSL 语义错误
    t.compile_fail("tests/ui/empty_range.rs");

    // 尾随运算符（`-`/`^` 后缺操作数）
    t.compile_fail("tests/ui/dangling_operator.rs");

    // 运算符/分隔符左空（`-A`/`^A`/`,A`/`A,,B`）
    t.compile_fail("tests/ui/leading_operator.rs");
    t.compile_fail("tests/ui/leading_comma.rs");

    // `unsafe` 并列非 fn 类型（应为 unsafe^T 或 unsafe fn(...)）
    t.compile_fail("tests/ui/unsafe_non_fn.rs");

    // 指令参数列表减法：`-` 缺目标 / 排除全部后为空
    t.compile_fail("tests/ui/minus_bad_target.rs");
    t.compile_fail("tests/ui/minus_empty.rs");

    // 泛型自动继承只认同名：改名 / bound 引用未声明形参
    t.compile_fail("tests/ui/rename_bound.rs");
    t.compile_fail("tests/ui/rename_ref.rs");

    // where 谓词继承：改名 / 复合谓词引用未声明形参
    t.compile_fail("tests/ui/rename_where.rs");
    t.compile_fail("tests/ui/where_const_ref.rs");

    // 组合展开数量超上限
    t.compile_fail("tests/ui/expand_limit.rs");

    // 裸 where 新语法缺少代码块
    t.compile_fail("tests/ui/where_missing_body.rs");

    // @ 常量系统：未知常量 / 范围端点错误 / 引用可见性（循环 / 前向）
    t.compile_fail("tests/ui/const_unknown.rs");
    t.compile_fail("tests/ui/const_range_bad.rs");
    t.compile_fail("tests/ui/const_cycle.rs");
    t.compile_fail("tests/ui/const_forward.rs");

    // #blanket：非 Deref 包装 / `:N` 非法
    t.compile_fail("tests/ui/blanket_ptr.rs");
    t.compile_fail("tests/ui/blanket_bad_depth.rs");

    // 一个 pass 路径，确保正常用例不被破坏
    t.pass("tests/ui/pass/basic.rs");
}
