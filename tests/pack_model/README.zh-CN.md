# 参数包提案模型

[English](README.md) | 简体中文

**这里只保存设计模型，batch-impl 公开宏仍使用既有语义。**
本目录在正式接入前，把已经审阅的 Pack v2 提案保存为可执行测试。
检查通过不表示公开宏已经支持新写法，也不表示已经可以发布。
整个 `tests/` 已从发布的 crate 排除。

先读[教程](tutorial.zh-CN.md)，再查[语义契约](contract.zh-CN.md)，两者均有英文镜像。
教程先讲空格加括号，点号右结合是熟悉之后可用的简写。

## 运行

需要 Python 3.10 或更新版本，以及配好本机链接器的 Rust。Python 只使用标准库。
生成的 Rust 使用 edition 2024，项目最低 Rust 1.95 即可。在仓库根目录运行：

```text
python tests/pack_model/run.py
```

统一入口依次执行 17 组单元测试、有限结构穷举、两种语言教程的标记示例、
生成 Rust 类型族和实际消费调用，并检查预期的 E0119/E0207 负例。
每个输出族使用同一个 trait，不用不同 trait 名掩盖 impl 重叠；完整保留
fresh 声明，包括未受约束的参数，也不静默去重目标。

只想求值一条提案表达式时：

```text
python tests/pack_model/run.py --eval "(*((),) (*(self,Vec) *().3),)"
```

路径按脚本位置解析，因此在其他目录使用 `run.py` 的绝对路径也可以运行。
入口使用当前 `rustc`、链接器与环境，不设置任何工作站专用路径。
Windows 上请使用已配置 MSVC 与 SDK 库的开发环境；模型不搜索或硬编码
Visual Studio 安装位置。

## 源文件与输出

| 文件 | 职责 |
|---|---|
| `semantics.py` | 结构化包、候选、声明作用域、apply 与物化 |
| `syntax.py` | 提案支持子集的严格解析器 |
| `examples.py` | 手工指定的教学预期输出 |
| `test_model.py` | 具体回归、类型族、fresh 作用域与上限 |
| `exhaustive.py` | 独立的有限结构枚举与性质检查 |
| `validate.py` | 双语教程校验和保留声明的 Rust 验证 |
| `paths.py`、`run.py` | 产物路径与可移植统一入口 |

所有生成文件均写入 `target/pack-model/`：各阶段日志、`validation.json`、
`exhaustive_results.json`、生成的 Rust 源文件、元数据与可执行文件。
统一入口禁用 Python 字节码缓存。这些输出不提交；单独调试某脚本时也请用
`python -B`。

CI 的 `pack-model` job 在 Linux、Python 3.12 与 stable Rust 上运行同一个入口。
它与 `cargo test` 独立，普通 Rust 测试套件不会自动执行这个 Python 模型。

## 接入边界

不支持的输入明确拒绝，不会静默丢弃。模型未覆盖完整声明块、where、属性、
指令/body 语法，以及部分已有前缀 apply。模型拒绝某个旧写法，不等于提案
要求删除该写法。

这些检查验证模型和它生成的普通 Rust 类型，没有验证生产解析器、卫生性、
诊断跨度或完整宏管线。有限穷举是所列范围内的证据，不是任意输入的正确性证明。
