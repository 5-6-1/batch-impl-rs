# 开发规范

> 本项目的开发约定单权威。轮换的 AI 评审者与未来 contributor 从本文档接手，
> 不必从历史 commit / changelog 反推习惯。与通用 Rust 约定（工具链、依赖纪律、
> 错误处理、测试布局）见 rust-conventions 技能；批量/文本处理边界见 batch-ops 技能。

## 0. 项目性质与协作模式

- **AI 轮换开发**：大部分代码由 AI 编写 + AI 评审，作者少量插手，会更换 AI。
  因此**代码即文档**是最高原则——隐式知识必须显式化，结构契约尽量交给
  编译器而非散文（见 §4 类型态管线）。
- **中文交流**：与作者交流用中文；代码注释、doc 注释、commit message 用英文。
- **真实用户验证**：alga2 是真实用户（~900 impl 由本库生成），设计以其为校验场。

## 1. 提交规范

轻量 Conventional Commits：

```
<type>: <subject>            # 英文祈使句，≤50 字符
```

- type 限定：`feat` / `fix` / `refactor` / `perf` / `test` / `docs` / `chore` / `build`
- 单 crate 不写 scope
- 示例：`feat: typestate preprocessing pipeline (Stream states enforce pass order)`
- 发布 commit：`chore: release 0.9.7 (version + docs sync)`，正文附版本要点

## 2. 质量门（每个改动必跑，全绿才提交）

```bash
cargo fmt --check                      # 先跑 cargo fmt（历史教训：漏跑导致 CI fmt job 红）
cargo check --all-targets
cargo clippy --all-targets -- -D warnings   # clippy 零警告
cargo test                             # lib + dsl + UI 快照 + doctest
cargo test --doc
cargo doc --no-deps                    # 零警告
```

快照更新：`BLESS=1 cargo test --lib golden` 重写黄金快照（渲染层有意变更时）。

## 3. 发布流程（先 GitHub + CI 通过，再 crates.io）

1. **Unreleased 占位**：开发期间在四处 changelog 顶部维护 `## Unreleased`
   （`CHANGELOG.md`、`docs/zh-CN/CHANGELOG.md`、`docs/dev-changelog.md`、
   `docs/zh-CN/dev-changelog.md`）。每个改动完成即记入，不攒到发布时补。
2. **发布时**：
   - `Cargo.toml` 版本号递增；
   - 头部版本行更新（`README.md`、`docs/tutorial.md`、`docs/architecture.md`
     及其 zh-CN 对应——EN 替换为 `**vX.Y.Z** (date) — 摘要`；zh-CN architecture
     按版本堆叠是**新增一行**，不替换）；
   - README 依赖示例版本（`batch-impl = "X.Y.Z"`）同步；
   - 四处 `## Unreleased` → `## X.Y.Z (date)`，保留摘要行；
   - `cargo package --list` 检查清单（无关文件不进包，历史教训：
     `rust-2024-feature.md` 曾被打进每个 `.crate`）；
   - commit `chore: release X.Y.Z (...)` → `git tag vX.Y.Z` →
     `git push origin main --tags`；
   - **CI 全绿后**（fmt / clippy / test-stable / test-MSRV / test-Windows / doc）
     才 `cargo publish`；若 push 后有修正 commit，tag 用 `-f` 移到最新 commit 再强推。
3. **发布后**：创建 GitHub Release（`gh release create vX.Y.Z --notes "..."`）。

## 4. 架构契约（改代码前必读）

- **类型态管线**（`src/preprocess/stream.rs`）：预处理顺序由类型系统强制，
  不是注释。新增 pass 必须在 `Stream<S>` 状态链内改——改到 `Paired` 之前的
  中间态，或尾部分叉（`expand_tokens` / `reject_directives` / `where_process`）。
  自由函数保持 `pub(crate)` 供 fuzz 直调（fuzz 按设计绕过链），
  `expand_consts` 入口的金丝雀 `debug_assert!` 不许删。状态按**不变量**命名，
  不按 pass 命名；只有建立新不变量的转换才配一个状态位。
- **单权威哲学**：每个跨模块判定收编到一处——`util/punct_ops.rs::read_op`
  （运算符形状）、`util/diagnostic.rs::compile_error_str`（错误构造）、
  `util/scan.rs::is_impl_template`（`impl{...}` 判别）、
  `entry/impl_spec.rs::chunks_to_streams`（where 块切分）、
  `ast/fresh.rs::is_carrier_at`（载体识别）。发现重复判定 → 收编，不新开副本。
- **语法冻结（0.7.2 起）**：既有 token 语义 final，新版本只做**加法**
  （新指令/常量/工具）、诊断精化、文档。任何语义变更 = 刻意的破坏性发布。
- **诊断 span**：指向用户可见 token（`err_ty_at` 水位），不用裸 `Span::call_site`
  （impl entry / shape 诊断已收编，`syn::Error::span()` / leaf token span 可用处必用）。

## 5. 文档纪律（双语五处 + doctest）

- **双语同步**：EN 是发布产物，zh-CN 开发时先写。五处 × 双语：
  `README`、`tutorial`、`architecture`、`dev-changelog`、`CHANGELOG`。
  改一处必须同步另一语言，发布前检查。
- **教程代码块 = doctest**：`docs/tutorial.md` 与 `README.md` 的 ```rust
  块被 lib.rs 的 `#![doc = include_str!]` 编译——改了必须能编译。
- **docs.rs 首屏**：README 是 lib.rs 文档的一部分，首页重构须保持
  "为什么用它 + 最小示例"置顶、版本横幅一行链接 CHANGELOG。
- **文档示例必须真实**：读者/评测员会逐条核对（splat 27 示例、`Box.Box u8`
  结合性都曾被实测抓错）。写进文档的展开结果先实测验证。

## 6. 依赖与工具链

- **绝不主动添加 crate**：需要新依赖时先解释用途、征得同意（例外：用户明确要求）。
- 工具链最新 stable、edition 2024、MSRV 1.95（**刻意**：`Cell::update` +
  match 臂 if-let guard，1.87/1.88 稳定，1.95 留 stable 余量；改 MSRV 需实测 +
  更新 CI `test-msrv` job）。
- 错误处理：thiserror 优先，简单场景手写枚举；不主动引入 anyhow。
- 异步：默认 tokio；不主动引入异步运行时。

## 7. 语法风格偏好（作者专属，勿改）

作者有明确且**专门处理过**的语法偏好，评审时按此把关：

- **链式胜过包裹**：优先 `val.into()` 与组合子链，而非 `Some(val)`、
  `Box::new(val)` 这类显式包裹——除非显式类型本身有信息价值。
- **推导胜过手写**：`let x = 1` 优于 `let x = 1u32` / `let x: u32 = 1`；
  类型标注优先放调用处（turbofish）：`(1..100).collect::<u32>()` 优于
  `let x: u32 = (1..100).collect()`；构造用公知简洁形式（`vec![]` 优于
  `Vec::new()`，该用 `format!` / `Default::default()` 时就用）。
- **简洁胜过复杂**：能短则短，避免无谓的中间变量与重复结构；不以牺牲
  可读性为代价。
- 完整条目见 rust-conventions 技能「风格三原则」。

## 8. 测试布局

- 默认内联 `#[cfg(test)]`；数量大或整体性强时迁入 `tests/features/`
  （按功能域分模块，每模块 <350 行，由 `tests/dsl.rs` 挂载）。
- UI 快照（trybuild）在 `tests/ui/`；黄金展开快照在 `tests/golden/`。
- fuzz（`src/testing/fuzz.rs`）直调单 pass、容忍乱序输入——它是类型态链外
  的第二层防线，金丝雀断言与之配套，改动 pass 时保持。

## 9. 打包卫生

- 发布前 `cargo package --list` 人工检查：无关文件（本地笔记、探针）不进包。
- 不提交探针文件（`tests/_iso/` 是临时区，历史教训：曾两次误提交）。

## 10. 边界（不要做的事）

- 不用 PowerShell 做文本替换/写入/批量处理（见 batch-ops：字面量用 edit/write，
  正则/批量用 rust 工具）。
- 不把"流程纪律"只写进注释——能收编成类型/断言的，收编；
  能收编成文档的，收编到本文档。
