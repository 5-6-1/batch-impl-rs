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
    t.compile_fail("tests/ui/single_name_not_found.rs");
    t.compile_fail("tests/ui/delegate_on_non_fn.rs");

    // 裸 where 新语法缺少代码块
    t.compile_fail("tests/ui/where_missing_body.rs");

    // 一个 pass 路径，确保正常用例不被破坏
    t.pass("tests/ui/pass/basic.rs");
}
