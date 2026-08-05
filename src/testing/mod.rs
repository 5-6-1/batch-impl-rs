//! 测试基建（仅 `cfg(test)` 编译）：proptest 随机 token 喂给真实宏入口，
//! 承诺"不因用户输入 panic"。

pub(crate) mod fuzz;
