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
- 发布 commit：`chore: release 0.9.8 (diagnostics and docs)`，正文附版本要点

## 2. 质量门（每个改动必跑，全绿才提交）

```bash
cargo fmt --check                      # 先跑 cargo fmt（历史教训：漏跑导致 CI fmt job 红）
cargo check --all-targets
cargo clippy --all-targets -- -D warnings   # clippy 零警告
cargo test                             # 所有 target：lib + dsl + UI 快照 + no_panic + doc_consistency + doctest
cargo test --doc                       # 单独列出：doctest 的失败形态与其它 target 不同
cargo doc --no-deps                    # 零警告
```

快照更新：`BLESS=1 cargo test --lib golden` 重写黄金快照（渲染层有意变更时）。

**UI 快照在 CI 里只在 Linux stable 跑。** `tests/ui/*.stderr` 锁的是 rustc 诊断措辞的原文，而它会随工具链与平台漂移：只有 `test-stable`（ubuntu + stable）跑 `--test ui`，MSRV 与 Windows 两个 job 跑的是 `cargo test --lib --test dsl --test no_panic --test doc_consistency`（刻意不点 UI target）。因此其他平台上的过期 `.stderr` **不是**回归信号——在 Linux stable 上用 `TRYBUILD=overwrite cargo test --test ui` 重新生成，新 fixture 也在那里加。这道网是真的，但作用域限定在平台上；套件里没有别的东西依赖它。

## 3. 发布流程（先 GitHub + CI 通过，再 crates.io）

1. **Unreleased 占位**：开发期间在四处 changelog 顶部维护 `## Unreleased`
   （`CHANGELOG.md`、`docs/zh-CN/CHANGELOG.md`、`docs/dev-changelog.md`、
   `docs/zh-CN/dev-changelog.md`）。每个改动完成即记入，不攒到发布时补。
2. **发布时**：
   - `Cargo.toml` 版本号递增；
   - 头部版本行更新（`README.md`、`docs/tutorial.md`、`docs/reference.md`、`docs/architecture.md`
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
  自由函数保持 `pub(crate)` 供 fuzz 直调（fuzz 按设计绕过链）。状态按**不变量**
  命名，不按 pass 命名；只有建立新不变量的转换才配一个状态位。
- **生产代码零 panic 构造**：`unwrap` / `expect` / `panic!` / `unreachable!` /
  `debug_assert!` / `assert!` 不得出现在 `#[cfg(test)]` 模块与测试专用文件
  （`testing/`、`*_tests.rs`、`*_worker.rs`）之外——proc macro 里的 panic 就是
  编译器 ICE，no-panic 承诺是无条件的。用 `.get()` 系访问器、`let else`、
  扫描派生索引，以及 `Cursor` 的**位置不变量**（`pos <= len`——`bump` /
  `advance` 都夹取，故 `slice_since` / `take_segment` / `take_rest` 自身无需
  边界守卫）。内部不变量检查一律**报诊断**而非断言：变长段残留后置条件
  （`preprocess/varseg.rs`）与 range 长度检查（`apply/apply_tuple.rs`）在违反时
  返回错误。回归守卫：`varseg::tests::postcondition_canary_never_fires`
  （≤6 token 穷举 + 随机长序列）、`scan::tests::cursor_position_never_exceeds_len`。
  **强制而非仅承诺**（两条腿，均可证伪——临时违规必须让它失败，已实测）：
  （1）`src/lib.rs` 带 `#![cfg_attr(not(test), deny(clippy::unwrap_used,
  clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo,
  clippy::unimplemented))]`——`not(test)` 作用域刻意给 crate 自身的
  `#[cfg(test)]` 模块与集成测试 crate 保留 `unwrap`/`assert!`；（2）
  `tests/no_panic/main.rs` 是源码级的那条腿：用 `syn` 走遍 `src/**/*.rs`（注释与
  字符串永不误报），报 `assert!` / `debug_assert*!`（clippy 无对应 lint）、
  `.unwrap()` / `.expect(…)`、以及**宏 token 流内部**铸出的 panic 构造
  （`quote!(x.unwrap())`——clippy 的 HIR 趟看不到）、**限定形式**
  （`Option::unwrap(o)`，clippy 的 `unwrap_used` 不抓）、以及**任何位置**静默
  该家族的 `#[allow(…)]`（item / impl item / 语句 / 表达式 / 文件自身的内层
  属性 / 嵌在 `#[cfg_attr(…)]` 里的臂 / **宏体内**的属性）与任何一刀切静默
  （`clippy::all`、`clippy::restriction`——整个家族所属的组——或 `warnings`）
  ——例外必须是那个守卫文件上的一次编辑，评审看得见。`#[cfg(test)]` 闸门是精确
  的：只跳过裸谓词，因此 `#[cfg(not(test))]` 的代码与其他生产代码一样被扫描。
  clippy 那条腿在 `clippy` job 跑；源码守卫搭在每个会执行测试的 job 上（全量 stable job，
  以及 MSRV 与 Windows 功能 job），并带"走查或跳过集坏掉就失败"的下限。
  **索引/切片棘轮：已完成，并收编为一行。** 迁移是逐文件进行的（生产代码 **208 → 0**——在棘轮前修订上重测为 207 个 `indexing_slicing` + 1 个 `string_slice`，跨 35 个文件；迁移当时的自计数为 203），每个文件临时带一个文件级 `deny(clippy::indexing_slicing)`；完成时把那些属性（记录该数字时为 24 个）全部替换为 `src/lib.rs` 里的**一行 crate 级 deny**（`cfg_attr(not(test), deny(clippy::indexing_slicing, clippy::string_slice))`，由 `tests/no_panic/main.rs::the_crate_denies_the_panic_and_indexing_families` 断言——重构删掉它就会一次性重开所有落点）。两个 lint 都要点名：它们是**分开**的，只写索引那个时 `&s[..n]`（字符串切片）会漏过去。真正干活的是两个应当复用的转换模式：在循环顶部一次性绑定当前 token（`while let Some(cur) = tokens.get(i)`，随后 `match cur` / `cur.span()` / `cur.clone()`）取代重复索引；把该 token 下传给辅助函数（`expand_group` / `expand_at` 新增 `cur` 参数）取代再读 `tokens[i]`。反复出现的切片惯用法住在 `util/scan.rs`——`span_at` / `slice_from` / `slice_upto` / `slice_window` / `slice_between`（`span_at` 吃 token 切片，其余四个对元素类型泛型），其中钳制的那几个**刻意钳制**（`get(..n)` 不是钳制：`n > len` 时返回 `None`）。该家族的*局部* `#[allow]` 仍属例外：必须登记到 `tests/no_panic/main.rs` 的 `INDEXING_ALLOW_EXCEPTIONS`，评审看得见；绝不使用模块级整体 allow。
- **单权威哲学**：每个跨模块判定收编到一处——`util/punct_ops.rs::read_op`
  （运算符形状）、`util/diagnostic.rs::compile_error_str`（错误构造）、
  `util/scan.rs::is_impl_template`（`impl{...}` 判别）、
  `entry/impl_entry.rs::chunks_to_streams`（where 块切分）、
  `ast/fresh_protocol.rs::is_carrier_at`（载体识别）、
  `ast/fresh_protocol.rs::AtRefError`（`@` 位置引用的诊断——每种情形一处拼写，
  由调用方选择渲染进类型通道还是 item 通道：`into_ty()` / `into_stream()`）、
  `ast/param_kind.rs::ParamKind`（参数种类——类型 / const / 生命周期——
  以及随之的 `const` 关键字剥离）。发现重复判定 → 收编，不新开副本。
- **语法冻结（0.7.2 起）**：既有 token 语义 final，新版本只做**加法**
  （新指令/常量/工具）、诊断精化、文档。任何语义变更 = 刻意的破坏性发布。
- **诊断 span**：指向用户可见 token（`err_ty_at` 水位），不用裸 `Span::call_site`
  （impl entry / shape 诊断已收编，`syn::Error::span()` / leaf token span 可用处必用）。

## 5. 文档纪律（双语六处 + doctest）

- **双语同步**：EN 是发布产物，zh-CN 开发时先写。六处 × 双语：
  `README`、`tutorial`、`reference`、`architecture`、`dev-changelog`、`CHANGELOG`。
  改一处必须同步另一语言，发布前检查；两种语言必须保持**相同的章节编号**——
  `tests/doc_consistency.rs` 比对教程、参考手册与本指南的编号骨架，以及
  `README` / `CHANGELOG` / `dev-changelog` / 本指南的标题骨架。同一套测试还会从树里
  推导 architecture **测试矩阵**所写的数字（UI fixture、golden、feature 模块/测试、
  `simplify.rs` 的 impl 数），因此那张表的计数不会静默过期。
- **教程代码块 = doctest**：`docs/tutorial.md`、`docs/reference.md` 与 `README.md` 的 ```rust
  块被 lib.rs 的 `#![doc = include_str!]` 编译——改了必须能编译。**zh-CN 镜像不在这套编译里**
  （`lib.rs` 只 include 英文文档，而 zh 文件被排除出发布包，所以 doctest-only 模块引用它们会让
  包上的 `cargo doc` 失败）。结构性检查是"镜像代码块计数"守卫；改动 zh 代码块时要手工复核——
  把两份 zh 文档当 crate doc 放进临时 crate 跑 `cargo test --doc`，2026-09-14 实测 58 块通过 + 1 块 ignore（共 59 个 ```rust 块）。
- **docs.rs 首屏**：README 是 lib.rs 文档的一部分，首页重构须保持
  "为什么用它 + 最小示例"置顶、版本横幅一行链接 CHANGELOG。
- **文档示例必须真实**：读者/评测员会逐条核对（splat 27 示例、`Box.Box u8`
  结合性都曾被实测抓错）。写进文档的展开结果先实测验证。
- **诊断消息要么被锁定、要么被登记**：`src/**` 里每个 `batch-impl: ` 字面量都必须出现在某个
  `tests/ui/**/*.stderr` 快照里，或列在 `UNLOCKED_DIAGNOSTICS`（`tests/doc_consistency.rs`）——
  那张表是**已登记的债**（2026-09-14 为 59 条）：列在其中意味着消息可达但未被快照锁定，
  新增诊断应当配 fixture，而不是往表里加。

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
  （按功能域分模块，每模块 ≤350 行，由 `tests/dsl.rs` 挂载）。
- UI 快照（trybuild）在 `tests/ui/`；黄金展开快照在 `tests/golden/`。
- fuzz（`src/testing/fuzz.rs`）直调单 pass、容忍乱序输入——它是类型态链外
  的第二层防线；词表要覆盖每个 pass 的入口关键词（历史教训：词表缺 `impl`
  时 `impl{...}` 模板上的变长段标记 pass 从未被随机覆盖），内部不变量检查
  与穷举回归测试配套，改动 pass 时保持。

## 9. 打包卫生

- 发布前 `cargo package --list` 人工检查：无关文件（本地笔记、探针）不进包。
- **包里到底装什么、为什么**（`Cargo.toml` 的 `exclude` 写了同样的理由）：构建只通过 `include_str!` 读 `README.md`、`docs/tutorial.md`、`docs/reference.md` 与 `src/doc/*.md`。`docs/zh-CN/`、`docs/dev-changelog.md` 与 `tests/` 是开发产物，一律排除——**必须一起排，不能只排一半**：`tests/doc_consistency.rs` 会读 zh 版 architecture，并解析当前态文档点到的每个路径（含 `docs/dev-changelog.md`），所以"带了 tests 却没带这些文档"的包会内含一个必然失败的测试套件，而 `cargo package` **只 build**、抓不到这一点。
- 不提交探针文件（`tests/_iso/` 是临时区，历史教训：曾两次误提交）。

## 10. 边界（不要做的事）

- 不用 PowerShell 做文本替换/写入/批量处理（见 batch-ops：字面量用 edit/write，
  正则/批量用 rust 工具）。
- 不把"流程纪律"只写进注释——能收编成类型/断言的，收编；
  能收编成文档的，收编到本文档。
