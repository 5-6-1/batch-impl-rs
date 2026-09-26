# Changelog（用户）

> 用户可见的功能与行为变化；内部实现细节见 `docs/dev-changelog.md`。

## Unreleased

> 目标版本：**0.10.0**，已发布 0.9.7 的下一版本。继续开发，发布日期未定。

- 开始分阶段实现 Pack 重设计，先建立内部应用内核与可执行规格。
  公开 `*` 语法仍使用现有 splat 规则，新模型尚未接入宏入口。
  修复 splat 成员及独立参数/trait 实参节点的遍历遗漏，避免漏收内部
  诊断、fresh 声明和展开质量。
  零长度的约束/笛卡尔生成器不再占用 fresh 组号，后续真正生成参数时
  可以按预期从 `@0_0` 引用。

- 修复开放扩展示例的返回类型与 `macro_rules!` 接收模板，使完整 trait、
  `Vec<u8>` 等多 token 目标及前置 body 协议都有实际验证；移除相关
  `ignore`。基础教程补上可停下的阅读点与同一 `Describe` 的维护案例。
  本地 rustdoc 增加当前版本导航，仓库链接明确其源码/版本上下文；六个
  文档占位宏在摘要首句标明不可调用，并更正参考扩展宏的用途说明。

- 补齐首次修改路线：首例旁区分 `u8`、`[u8,]` 与切片 `[u8]`，说明局部与
  共享 body 合并而非覆盖；将已有普通 impl 的批量复用列为常用入口，
  无需外部 trait 签名镜像。双语教程沿同一个 `Describe` 逐步加类型、
  方法、特殊实现、泛型约束和包装转发，任务导航可点击。预览补齐 trait、
  `batch_impl_only` 与 impl 三入口示例，修正诊断数量保证及两处失实说明。

- 将计划版本从 0.9.8 调整为 **0.10.0**，让下列刻意的破坏性语法改动
  位于 Cargo 的 `"0.9.7"` 兼容范围之外。
- 重写双语入门路线：完整程序可直接复制，给出真实输出，再沿同一个 trait
  扩展能力。未发布源码使用本地路径依赖，发布版用户引导至 0.9.7 文档。
  前置基本成员指令，把预览作为错误恢复入口，说明外部 trait 的签名镜像
  成本，并使语言/文档导航同时适用于 Markdown 与 rustdoc。

- `#delegate` body 新增 `receiver.#call`：沿当前方法的改名映射生成完整
  调用并自动转发参数，无需追加 `()`。出现可识别的标记时，整个内容成为
  方法体，支持异构枚举分支、接收者表达式和结果链；没有标记时保持原来的
  目标表达式形式。宏 token、属性与内嵌 item 保留自身边界，不自动等待
  异步调用。
- 两种委托形式都显式转发方法的类型与 const 参数，覆盖无法由普通实参
  推断的参数；生命周期保持推断。更正参考手册的旧说法：排除名即使不在
  选中集合里，也必须是 trait 的现有成员。

- **破坏性变更：**类型族区间与 Rust 的端点规则统一：`@u8..u32` 选择
  `u8, u16`，`@u8..=u32` 还包含 `u32`。既有两端指定或省略左端的区间
  若需保留原来的包含末端行为，补上 `=`；省略右端仍到该族最大宽度，
  空区间和反向类型区间仍报错。单元素区间选择该类型，不再误读为切片。
- 指令名字列表允许空选择和尾逗号，包含嵌套列表与排除项。
  `#fill(){...}`、`#delegate(){...}`、`#blanket(){...}` 均不选择任何项，
  生成的 impl 仍须满足 Rust 的 trait 要求。所有项名先校验再排除，
  即使选择最终为空，拼错的名字也会报错。
- 修复 `#name`、`#fill`、`#blanket` 中带约束的关联类型和 GAT：保留泛型
  声明与 where 子句，impl 定义移除 trait 侧结果类型的 bounds，GAT 投影
  只传参数名字。
- 修复 `#delegate` 选择解析吞掉逗号或改名前其他名字的问题；找不到项及
  选中非方法的诊断现在按正确顺序显示项名和 trait 名。

- **破坏性变更：**移除已废弃的 `@all_fresh` 别名，既有用法改为 `@0..`；
  `@?` 保持支持。
- 新增 `@Self`：当前 impl 属性收到的自身类型，沿已有常量阶段用于模板、
  矩阵、类型实参与 where 谓词。堆叠属性各读本层输入，普通 Rust `Self`
  保持原义。**破坏性变更：**`Self` 成为保留常量名；既有 `batch_trait!`
  自定义 `@Self=...;` 需要改名。
- 修复形状替换：同名不能既要求保留字面又要求替换为不同内容；trait 入口
  已选定的矩阵叶子不再被二次替换。合法的同时映射保持支持。
- 泛型实参可匹配已声明的 const 参数（`Wrap<N>` 对 `Wrap<2>`）；函数指针
  可递归匹配参数与返回类型（`fn(A) -> B`）。const 声明及其引用同步处理，
  拒绝类型/const 跨类匹配；函数安全性、ABI 与生命周期契约保持，类型位置
  替换保留参数标签。
- 修复 `#blanket` 对异步方法与方法类型/const 泛型的转发，覆盖静态方法。
  生命周期保持推断，无需新增语法或运行时依赖。
- blanket 调用限定当前 trait，消除 supertrait 同名歧义；方法泛型约束中的
  裸 `Self` 给出诊断，关联投影、`Self: Sized` 与 outlives 门槛保持支持。
- 更正已有语法的说明：空格 apply 可用括号嵌套（`Box (Vec u8)`）；裸
  `where` 有谓词时可以不带 body；impl 入口的形状形式匹配冒号前的显式模板。
  同时更正 blanket 文档：关联项也转发时，关联类型投影可用于参数和返回值。

- 更正参考手册的入口分隔符：trait 属性使用逗号，impl 属性使用分号，
  `batch_trait!` 的 trait 段之间使用分号、每段内部使用逗号。
- 更正独立 splat 目标的说明：`*(u8, u16)` 生成两个合法 impl，目标重叠才会
  导致 E0119。两种语言都加入可编译的分隔符示例和 splat/元组对照示例。
  宏行为不变。

> 本开发周期此前积累的改动：`@N..M` 在所有位置都不含末尾，修正原先与文档矛盾的位置；`where` 谓词在定型后校验、splat 在参数位置列表里展开、误导性诊断修正，文档拆成教程与参考手册。

- **参考手册按"规则系统"组织，教程改为指向它。** apply / splat / `@` / `#` / `where` / `impl{...}` 各章分别给出规则、边界情形与交叉；impl 入口文法、堆叠属性的阶段语义与完整的形状模板绑定表都住在这里（§10 还逐字给出每条诊断措辞），而教程保留渐进路径、可运行示例与任务优先索引。

- **第三次评审——这次是把文档拿去和源码对读——更正了四处假声明并补上两个守卫缺口。** `#blanket` 的示例展示了宏根本不会生成的 impl（`Box<u32>`、`Cow<'_, str>`；实际产物是一个泛型 impl：`impl<P0> NumOps for Box<P0> where P0: NumOps`），而 `&` 包装被画成用 `(*self)` 委托——两段之下就是写着 `&` 与 `Box` 都用 `(**self)` 的规则。fresh 显示名撞名时用的是**电子表格式字母后缀**（`P0A`、`P0B`…`P0AA`），不是下划线。`examples/simplify.rs` 是 **30 个 impl** 而非 29：它自己的清单漏掉了 `main()` 断言其效果的 `#fill(name, kind)` 那个 impl。`README` 的 `().4` 示例改为 `P0…P3`（与全库一致），教程 §13.2 的引用改指 §7（`#name` / `#fill`）而不是 §6.3。两个守卫封住这类问题：每个 fixture 名在每版目录里必须**恰好出现一次**、两版行多重集必须相等（此前重复的一行在所有 `contains` 式检查下都绿）；以及紧挨指令标记的 `§` 引用必须指向关于该标记的章节——两处错引用指的都是**真实存在**的章节，这正是"只验存在"的守卫一直绿的原因。
- **前导 `-` 会被诊断，而不是静默生成 0 个 impl**——已退休算子的消息此前只覆盖 `Box - u8`，而 `#[batch_impl(-usize)]` 与 spec 列表里的 `-` 元素（`Vec<u8>, -u16`）**既不生成 impl 也不报错**，与"任何输入都不会静默产出零个 impl"的文档承诺矛盾。三种位置现在都报退休消息。
- **目录声称完整、实则没有 fixture 锁定的七条可达诊断已补齐**——`#blanket :0`、`#blanket` 包装列表里的空元素、未闭合的 `<`、作左操作数的 range、一个 spec 里第二个顶层 `{! ...}` 块、没有驱动的重复块、以及固有 impl 上的 `@trait`。新增守卫扫描源码里每一条 `batch-impl: …` 消息，要求它要么被 UI 快照渲染、要么带理由列入清单——这一类问题不会再静默回来。

- **教程又过了一轮评审**——§0 的任务索引加上 README 的**层级**列；§6、§7 与 §8.4/§8.5 明确标为可跳过的进阶层，于是核心路径（§1–§5、§8.1–§8.3、§9–§13）读起来是一条线；§9 定义了幂后缀与四种元数拼写（`()N`、`*()N`、`(A,B,)N`、`().1..=M`）并配实测例子，其中更正了"`(T,)N` 生成 1..N 元元组"这一错误说法（`(u8,)3` 是三重**积**，元数区间是 `().1..=M`）；§12 只留六个真正常撞的错误并指向参考手册 §10 的逐字目录；§4.6/§5.7 里查阅型的清单收缩成一条规则加一个指针；§1 在讲结合性之前先给出回报（`examples/simplify.rs`：约 15 行出 30 个 impl）。
- **五个此前只有中文教程才有的示例，现在两种语言都有而且会被编译**——其中两个是**坏的**（它们从未被编译过）：`#[repr(C)] u8` 在 trait impl 上不合法，`#fill(@all_methods, -name)` 的排除把参数集清空了。第三个潜在缺陷是隐藏的 `trait A<T>` 与 `batch_trait!` 示例实现的 `A` 不一致，也已修好。

- **外部评审后的教程更正**——教程不再声称 trait 参数改名会报错（§5.5/§8.3）：继承是**位置式**的，因此 `trait Store<T> where T: Clone` 配 `<X> Store<X> usize` 得到 `impl<X: Clone> Store<X> for usize`。§3 共享 body 示例的注释现在与宏真正拼接进每个 impl 的 body 一致；英文教程代码注释里的全角标点改为 ASCII；§0 标明自定义 `@name=...` 段**仅 `batch_trait!` 支持**；顶部新增按任务索引；§8.4 给出缺少开关声明时的实测诊断。

- **已退役的 `^` 幂现在会自己说话**——`(u8, u16)^2` 此前只报通用的 "unexpected `^` after the type"；现在直接说明 `^` 不再是类型算子，并给出可用的 `.N` 拼写（元组写 `(u8, u16).2`，生成器写 `T.*().2`）。同一条消息覆盖 caret 能出现的每个位置——spec 链、角度块、splat 组、`dyn` 尾巴、声明块，以及**bound 位置**（那里此前会被静默丢弃：`<T: Tr^u8>` 渲染成 `<T: Tr>`）。

- **排他 `@N..M` 范围现在在所有位置都排除端点**——`@0..2` 处处覆盖 `P0, P1`（目标类型、`<>` 实参、where 谓词），符合文档化的 "normalized to inclusive" 协议；此前在类型位置覆盖 `P0, P1, P2`（与 where 谓词路径不一致）。空排他范围（`@2..1`）报定向错误。冻结语法面注：`@N..M`（排他）与 `@N..=M`（含端点）分别等价于 `@N..=M-1` 与 `@N..=M`。
- **no-panic 修复**——`#delegate(=foo)`（重命名缺左侧）、`#blanket` 方法参数组内 `Self`（`(Self, u8)`——现在像裸 `Self` 一样被拦截并引导）、repeat 块内极端 `@N` 游标字面量都不再 panic；各自报定向错误。
- **不再存在 panic 路径**——宏的生产代码不含 `unwrap` / `expect` / `panic!` / `unreachable!` / `debug_assert!` / `assert!`（proc macro 里的断言/panic 就是编译器 ICE）。内部不变量检查改为报定向错误：变长段残留检查（`mark_template`，已用穷举输入扫描证明不可达）与 range 长度检查。`Cursor` 的位置不变量（`bump` / `advance` 夹取到末尾）让解析层的切片在结构上免于 panic。该承诺不再只靠评审：`lib.rs` 的 clippy deny 家族加上源码级守卫测试（`tests/no_panic/main.rs`，同时拒绝用 `#[allow]` 静默该家族）会在 panic 构造重新出现时让构建失败。
- **仅*名为* `constant` 的类型参数永不被当 const 参数**——形状判定（`const` + ident 双 token）现在同时覆盖重复声明合并（`<constant: Clone> <constant: Copy>`）与"声明 vs 实参"的判定：`Vec.<constant>` 渲染为 `Vec<constant>`（此前会把 `<constant>` 当泛型声明 hoist 出去并以 E0107 失败）。重复 bound 仍按文档并成 where 谓词。
- **越界开区间 fresh 范围处处无操作**——`where{@5..: Clone}` 在 2-fresh impl 上贡献零谓词而不是 panic（where 谓词路径此前有与类型路径同款的索引越界缺口）。
- **无效 fresh-binding 开关报错**——`impl{@2..1}` / `impl{@2..=1}`（覆盖零个 fresh 的范围）报 "invalid fresh-binding switch"，不再静默重开或落入形状模板通道。该消息是**唯一**的错误：诊断替换整个 impl，不再附带 `expected {}, found ;` 等解析噪声与未约束参数错误。
- **超限 bound 生成器分发报错**——bound 数组笛卡尔积超过展开上限时报 "expands to N impls (limit ...)"，不再把非法的 `T: [A, B, ...]` bound 交给 rustc 报晦涩错误。诊断替换整段展开（乘积在任何分配前就被检查），且它针对的是**乘积**：每条范围单独都在上限内（三条 31 元范围 → 29791）正是它存在的场景。
- **`impl` 条目多模板合并**——一个矩阵上带多个形状模板（`impl{...} impl{...}`）的 `impl` 条目现在保留并合并*每一个*模板，而不是静默只留最后一个；所有模板合并出的槽位在体内都可绑定（attribute 条目本就合并——impl 条目现在走同一条路径）。
- **裸 `impl <trait-object>` 目标报定向错误**——spec 里的 `impl Fn() -> u8` / `impl dyn Fn() -> u8` / `impl Iterator + Clone` 不是形状模板（0.9.5 前的目标拼写从未渲染出合法 Rust，且自裸 impl 收集起静默产出空目标类型）。错误指引可用的拼写：trait 对象写 `dyn Fn() -> u8`，模板写 `impl{...}`。
- **impl 条目容忍 impl 块上的槽位同名泛型**——`impl<T> Mk for Wrapper<T>` 配模板槽位 `T` 不再产出 rustc E0207：冗余参数被剥除，其 bound 转为 where 谓词（`impl<T: Clone>` → `where u8: Clone`）。
- **无 body 的裸 `where` 不再吞掉下一个 spec**——`#[batch_impl(u8 where u8: Copy, isize)]` 恢复为两个 spec（`,` 是 spec 列表分隔符）；`where A: Clone, B: Copy` 仍然继续扫描（两个块都是谓词）。
- **where 谓词中的空排他范围报错**——`where{@2..2: Clone}` 与类型位置一样报 "empty exclusive range"，不再把裸 `@..` 泄漏进渲染出的子句。
- **超限错误说明它量的是什么**——`.N` / 范围 / 笛卡尔积超限仍报 "expands to N impls (limit 1024)"，而组合数组×range 链（曾经把编译器拖死的那条路径）的内部守卫现在报 "reaches an expansion mass of N nodes (limit 1024)"，不再把并非 impl 数的数字套上 impl 的说法。
- **impl 条目块自身的属性现在会传递到生成的 impl**——写在 `#[batch_impl(…)]` 所在的 `impl` 块上的 `#[cfg]` / `#[allow]` / `#[doc]` 此前被静默丢弃，因此那里的 `#[cfg(feature = …)]` 会生成无条件存在的 impl。现在它们会被发射在每个生成的 impl 之前，与一直继承 spec 附加属性的 attr 条目对齐。
- **spec 里现在可以写限定类型**——`<T as Tr>::Assoc`、`Foo<T>::Assoc`、`Foo::<u8>::Assoc`（turbofish）都能解析并渲染：作目标、作泛型实参、位于 bound 内，以及嵌套（`<<T as Tr>::Assoc as Tr>::Assoc`）。此前它们报 "unexpected `:` after the type"，因为角括号配对那一趟已经把 `<...>` 变成自洽的组，`::` 尾巴无处可挂。纯加法：此前能解析的拼写渲染出的 token 完全不变（有测试锁定）。
- **bound 位置现在接受关联类型绑定**——`<T: Iterator<Item = u8>>`、`dyn Iterator<Item = u8>`、`Box<dyn Iterator<Item = u8>>`、`for<'a> Iterator<Item = u8>`、`dyn (Iterator<Item = u8>)` 都是合法 Rust，此前却一律报具体类型那条错误（"binding args (`Item = u32`) are only valid on a trait path …"），因为"这个头是不是 trait"只看被实现 trait 的名字。普通类型的实参仍是普通类型列表——`Vec<Item = u8>` 保留定向错误，措辞现在也点明 bound 这个合法位置；并且接受范围止于 bound 元素的**头部**：它内部的子类型位置（`T: fn(Vec<Item = u8>)`、`T: (Vec<Item = u8>,)`、`T: &'static Vec<Item = u8>`）仍报同一条错误。无逗号的 `(...)` 是刻意的例外——Rust 把它读作带括号的 bound（`dyn (Iterator<Item = u8>)` 可编译）。
- **全局路径（`::std::vec::Vec<u8>`）可解析**——开头的 `::` 会开启一个块，因此可作 spec 起始、可嵌在实参列表里（`Box<::std::vec::Vec<u8>>`）、也可接 `::` 尾巴。此前顶层无法解析，而在实参列表里会**静默为空**（`Box<::std::vec::Vec<u8>>` 渲染成 `Box<>`）。`::` 后面不是路径段标识符时报定向错误。
- **`fn(...)` 类型的具名参数**——`fn(x: u8) -> u8`、`fn(u8, y: u8)`、`fn(_: u8)` 都是合法 Rust，此前报 "unexpected `:` after the type"。名字原样保留、`:` 之后的类型照常解析，因此具名参数内仍可用 DSL 算子（`fn(v: Box<u8>) -> u8`）。`Fn(x: u8)` 仍然报错——这是 rustc 对 `Trait(...)` 语法的规则（"does not support named parameters"）——现在给出直说这一点的消息；只有名字没有类型也会被报出，而不是渲染成 `x:`。
- **返回类型里多一个 `#` 不再挂死编译器**——`extern "C" fn` 透传块的返回表达式由一个 token 折叠循环消费，它信任"这个 token 能开启一个块"；而 `#` 后面不是 `[...]` 时（`extern "C" fn(u8) -> u8 #(x)`，本是属性/指令的笔误）块解析器判定它开不了块却**没有消费它**，折叠于是在原地空转。它不分配任何内存，因此 fuzz 套件的分配守卫抓不到：编译器永远跑不完（实测——把 `#(x)` 换成 `-> u8` 同一 spec 冷编译 47 s 完成，带 `#(x)` 的那份从未跑完）。现在它报 "unexpected `#` in a type position" 并消费该 token，与前两个折叠循环一致。
- **`impl` 块上的 `#[batch_impl]` 属性堆叠是同一次派生的多个 stage**——rustc 先展开最外层属性并把其余属性交给它，本入口再把它们发射到自己派生的 impl 上，编译器随后在**那些 impl 上**展开下一步：各步按**源码顺序**作用于**累积中的块**，前一步留在原地的槽位由后一步绑定。各步合成为笛卡尔积——`#[batch_impl(A : [u8, u16])] #[batch_impl(B : [u32, u64])]` 作用在 `impl Tag for Pair<A, B>` 上得到 `Pair<u8,u32> … Pair<u16,u64>` 四个 impl——于是 **shape family**（头部形状各不相同的容器：`Vec<T>`、`[T; 4]`、`Box<[T]>`、`&[T]`）可以用两步表达，而不必每族写一个 prototype。**空** stage 是恒等元（可用来关掉某一步）；写在两步之间的普通属性属于它所在的**展开层级**——该层的 `#[cfg]` 会裁掉该层派生的 impl **以及它下面的所有 stage**（实测：中层 `#[cfg(any())]` 时下层那个必然报错的 stage 根本没运行）。该顺序由 `tests/features/impl_entry_chain.rs` 锁定，其中包含把两条属性对调后 shape-family 用例报 `E0425` 的反例。
- **角度实参里的 `+` 不再被当作 bound 链**——裸区域收集器跑在**尖括号配对之前**，面对扁平 `<...>` 分不清"实参"与"顶层 bound"：`impl Box<dyn Fn() + Send> { … }` 被报成 `impl <trait-object>` 目标，而括号拼写 `impl{Box<dyn Fn() + Send>}` 能通过——两种拼写本是同一个模板。同一根因还让扁平角列表里的 `{…}`（const 泛型实参，`impl W<{ 1 }> { … }`）被当作 impl body。现在两种拼写一致。
- **写在 trait path 实参里的槽位名会被替换**——`#[batch_impl(Wrapper<T> : [Box, Rc].u8)] impl<T> PartialEq<T> for Wrapper<T>` 把 `T` 作为槽位参数剥除，于是留在 `PartialEq<T>` 里的名字到达编译器时无法解析（`cannot find type T`）。现在只映射 path 的**角度实参**，path 自身的 ident 不动——槽位名与 trait 同名也不会改坏 trait 名。
- **attr 的泛型声明与块的声明对账**——`#[batch_impl(<T> Box<T>)] impl<T> MkD for Box<T>` 此前会重复声明 `T`（`E0403: the name T is already used for a generic parameter`）。现在块自己的声明优先，被丢弃的那份把它的 bound 转成 where 谓词——`#[batch_impl(<T: Clone> …)] impl<T> …` 仍保留 `T: Clone`。
- **impl 入口上空 spec 列表是无操作**——挂在 `impl` 块上的 `#[batch_impl]` / `#[batch_impl()]`（以及只有分隔符的 `#[batch_impl(;)]`）此前会**吞掉整个块**：impl 入口按设计扣下原块（属性用从它派生出的 impl 顶替它），于是"什么都没派生"就什么都不剩，该项在**毫无诊断**的情况下消失。现在它原样发射原块——这正是派生的恒等元，也是"宏生成或意外清空的属性"最安全的失败方式。attr 入口本就是这个行为（空列表保留 trait、不加任何 impl），两个入口现在一致。
- **生命周期永远不是形状槽位**——当槽位名与某个生命周期同名时（`#[batch_impl(Box<a> : [Box<u8>, Box<u16>])] impl<'a> L<'a> for Box<a>`），替换会钻进生命周期**内部**改掉那个 ident，并丢掉 `<'a>` 声明，于是 impl 以 `E0261: use of undeclared lifetime name 'u8` 失败。形状匹配中具名生命周期是逐字比较、从不绑定，所以 `'a` 现在原样通过并保留声明，而类型位置照常替换。
- **`::` 后面什么都没有时会直说**——`#[batch_impl(A::)]` 此前报 "a `::`-tail segment must be an identifier — DSL tokens (`@…` / `#…`) are not allowed in a `::`-tail"，描述的却是另一种错误。现在尾部直接结束时报 "`::` must be followed by a path segment (write `Foo::Assoc`)"；尾部里真的写了 DSL token 则沿用原措辞。
- **`dyn … + Marker<>` 尾巴（以及目标类型任何位置）里的 `<>` 同步标记会填上本 spec 的实参，而不是消失**——trait 对象的尾巴此前是一个同步看不见的 token 袋，于是 `#[batch_impl(<T> … Box<dyn Marker<> + Send>)]` 生成 `Box<dyn Marker + Send>`：标记静默消失（空实参表渲染成裸名）。现在空尖括号出现在 impl 类型结构的任何位置都会被填充——where 谓词、`impl{...}` 模板、impl 泛型 bound 与目标类型。
- **写在*目标*上的 `X<>` 不再污染 trait 的实参**——声明简写 `A<>`（声明 trait 的形参并应用实参）属于 spec 头部；写在目标上时它吐出的声明块会被解析并进 trait 自己的实参，生成 `impl<T> Trait<T, T> for Holder<T>`（没人能编译的 impl）。现在只有头部会展开，目标上的标记走上面那条同步规则。
- **`dyn` 列表里没有后继 bound 的 `+` 现在有诊断**——`dyn Send +` 此前把裸 `+` 交给 rustc 抱怨；现在 DSL 直接报 "a `+` in a `dyn` bound list needs a bound after it"。
- **`impl{...}` 模板里的 DSL 算子直接报出**——形状核此前把模板解析失败包成 "template cannot destructure the target type (…)"，把真正的消息埋了；现在模板在 `X<>` 同步之后立即解析一次，诊断直接读作 "the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside)"。
- **`for<…>` binder 现在有校验**：它装的是生命周期（`for<'a>`），所以 `for<u8>`——类型参数属于 impl 声明而不是 binder——由 DSL 报出并指向出错元素，不再以 "expected lifetime" 的形式丢给 rustc。
- **畸形 `@` 引用无论在什么位置都报自己的错**——角度实参里的空排他区间（`#[batch_impl(Box<@2..1>)]`）、非整数位置引用（`Box<@1.5>`、`Box<@1u8>`）、range 端点不是数字，此前都以类型位置的 `compile_error!(…);` 到达 rustc，于是错误显示成 `` expected one of `,` or `>`, found `;` `` 而不是宏自己的消息——而且还额外吐出一个半成品 impl。现在它们报 DSL 自己的错误，且空区间消息会带上数字（``empty exclusive range `@2..1` ``），不再打印字面 `@{}..{}`。
- **`<>` 声明块里的关联类型 binding 报错，并给出可用写法**——声明块声明的是**参数**，因此 `<Item = u8> Held` 现在报 "an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target`"（嵌套写法 `(<Item = u8> Held,)` 此前会静默丢掉 binding，最外层写法则会"照办"——两者都不是 DSL 建模的拼写）。请把 binding 写在 trait 应用上：`#[batch_impl(AssocOnTrait<Item = u8> Held)]` → `impl AssocOnTrait for Held { type Item = u8; }`，并被提升进 body——Rust 里 `impl Trait<Item = u8> for X` 是 `E0229`。
- **生成的新鲜泛型不再抢占写在 bound 里的名字**——impl 泛型的内联 bound 与从 trait 定义继承来的谓词，现在都在生成显示名（`P0`、`P1`…）必须避让的冲突集里，因此 `<T: BoundTr<P0>> …` 里的 `P0` 仍指你自己那个类型（新鲜泛型改叫 `P0A`），不再被静默遮蔽。

- **`where{…}` 谓词在定型后校验**——宏现在在所有阶段跑完之后（`X<>` 填充、`@` 解析、`impl{…}` 形状模板槽替换）解析谓词，因此 `where{ A B }`（漏 `:`）由 DSL 报出并给出修法，不再以"整个属性解析失败"的形式出现。
- **`impl{…}` 模板槽现在也替换进 `where{…}` 谓词**——文档承诺的替换（"模板的名字会重写目标、谓词与 body"）实际只作用到一份**不参与输出**的谓词副本上：`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` 生成 `where Vec<T>: Clone`（impl 上没有 `T`，E0425），而不是 `where Vec<i16>: Clone`。
- **谓词内的 splat 由 DSL 报错，不再丢给 rustc**——splat 是"参数位置列表"，没有任何阶段会展开谓词里的它（`(*(A,B)): Trait`、`X: Trait<*(A,B)>`），因此由谓词终检报错。裸 `*(A,B): Trait` 保留自己的消息，且不再推荐一个根本不工作的写法。

- **文档现在是"教程 + 参考手册"两份**——新增 `docs/reference.md`：位置 × 构造合法性矩阵、完整诊断目录（每一类 + 锁定其措辞的 fixture）、上限与保证。教程保留渐进路径并指向它，一条事实只有一个家；两份都进 crate 的 rustdoc。
- **绝对路径目标用 `.` 隔开**——`<...>`（跟在 ident 后）与 `::` 续接当前路径，所以 `@trait<u8> ::std::string::String` 会粘成一条路径、把 trait 放进类型位置（E0782）。写 `@trait<u8> . ::std::string::String` → `impl Tr<u8> for ::std::string::String`；有 trait 头时 `.` 与空格等价。
- **参考手册的诊断目录列出全部锁定消息**——104 个 `compile_fail` fixture 的触发与精确措辞（外加 3 个 `pass`），每行标明是 DSL、rustc、`batch_trait!` 前端还是预览通道写的。有四行如实记录"该有定向诊断、目前是 rustc 的消息"。

- **教程末尾新增实战章**——仓库那三个示例（`quickstart`、`simplify` 约 15 行 DSL 出 30 个 impl、`typeclass` 层级 + 36 个实例）连同它们组合的机制与运行方式一起讲清楚。

- **splat 现在在*每一个*参数位置列表里都展开**——callable 的参数表（`fn(*(u8, u16))`，`Fn(*(A,B)) -> C` 是同一张表）、`<>` 声明块（`<*(A,B)>` → `<A, B>`）与内联 bound（`<T: Tr<*(u8, u16)>>` → `<T: Tr<u8, u16>>`）此前把 splat 原样交给 rustc（raw pointer 错，或 `expected type, found @`）。唯一仍然拒绝 splat 的位置是 `where` 谓词——那里由 DSL 报出。
- **三条误导性诊断修好**——`#[batch_impl(1.5)]` 与 `#[batch_impl(1..x)]` 此前都报 "space-application chain exceeds 129 levels"（字面量块的错误路径没有消费游标，链把同一个 token 折到深度上限）；现在分别报"类型位置的字面量必须是整数"与"range 端点必须是整数"。`<>` 声明块里的 fresh 生成器（`<*().3>`）此前报 rustc 的 `expected type, found @`，现在给出定向消息与可用写法（`T.*().2`）。
- **声明位置的报错现在给出可用拼写**——它的示例曾是 `T^()^2`，但 `^` 不是算子：解析器直接拒绝（"unexpected `^` after the type"），这个拼写和经典的幂形式都一样被拒。现在示例是 `T.*().2`（`.N` 载体形式——`Box.*().2` → `Box<P0, P1>`）；另一种可用形式是并列的 `T<()2`（`Box<()2>` → `Box<(P0, P1,)>`）。

- **第五轮评审（对文档的冷读）**——独立评审把每份文档通读一遍，发现 architecture 的测试矩阵从未被测量过：它写着 104 个 UI fixture、9 份 golden、299 个 feature 测试、`examples/simplify.rs` 29 个 impl，而树里是 **112**、**10**、**300**、**30**。新增守卫从树里推导这些数字（并核对两种语言的镜像），因此不会再漂移。同一轮还修了 README 里不带文档名的 `§` 引用（两份文档都有那些编号，指向不同内容）、把教程指向的修饰符表补进参考手册、把六入口对照表移回教程（它属于那里），并让三份 `src/doc` 页面不再自称 "documentation marker only"——它们文档化的都是真宏。

## 0.9.7 (2026-08-29)

> 评审修复发布：黄金快照测试层（最后一块覆盖空白）、实测展开开销、打包卫生、Windows CI。无 DSL 语法变化。

- **黄金展开快照**——代表性 spec（矩阵 / splat / 嵌套空格应用 / where / 指令 / 两个 impl entry 形状）的**最终渲染输出**与 `tests/golden/*.golden` 锁定（`BLESS=1 cargo test --lib golden` 重写）。渲染管线漂移现在以一行 diff 失败，不再藏在"仍能编译"后面。
- **展开开销实测**——README 新增 **展开开销** 小节，由可复现的 perf 测试支撑（`cargo test --lib perf`）：1024 个 impl 上限（`(u8, u16, u32, u64).5`）约 0.2 ms/impl；典型 4 个 impl 约 2.5 ms（proc-macro2 层，不含 rustc 类型检查）。
- **打包卫生**——无关的本地笔记文件 `rust-2024-feature.md` 此前被打进每个 `.crate` 下载（`cargo package --list` 实测确认）；已从仓库移除（本地保留）并排除打包。
- **Windows（MSVC）CI job**——`windows-latest` 上跑功能/回归 + doctest，正是 `linker_messages` 抑制所要服务的平台。
- **impl entry 精确诊断 span**——形状模板解析错误用 `syn::Error::span()`，`match_shape` 失败携带 leaf/模板 span，`@{N}` 缺开关错误指向载体 token（此前全部 `Span::call_site`）。
- **纯文档占位宏不再静默**——误调用 `batch_impl_delegate!` / `fill` / `blanket` / `name` / `open` / `consts` 现在报 "documentation-only entry point"，不再展开为空；`batch_preprocess_test!` 文档化为其真实身份：可用的参考实现。
- **入口单次解析**——`#[batch_impl]` 扫描 item 首个语义 token（`impl` vs trait）再解析，不再每次尝试两次 `syn::parse`。
- **README 首页重构**——版本横幅压成一行并链接 CHANGELOG、"为什么要用它" + 最小示例置顶、特性表加核心/进阶层级标注、MSRV 原因写入文档（刻意选择：`Cell::update` 与 match 臂 if-let guard）。

## 0.9.6 (2026-08-27)

> ItemImpl 入口追上 attr 入口：`#[batch_impl(spec)] impl ...` 现在共享完整 DSL——`@` 内置常量、生成器 + `@N..` where 选择器、`fresh!(...)` body 标记、块模型（每容器 `impl{...}` 模板、`where{...}` 任意位置）、非匹配模板的文本替换、变长段模板。内部统一了多字符运算符识别。

- **ItemImpl 入口共享 attr 入口的 DSL**——形状形式的矩阵与直接形式的 for-type 现在按完整 DSL 解析：
  - **`@` 内置常量**（`@u*` / `@f*` / `@num` / range 族）可用于矩阵源；`@trait` 展开为 impl 自己的 trait 路径（固有 impl 报错）
  - **生成器 + `@N` 选择器**——`GenA<()0..=12>` 把 fresh 泛型 hoist 上 impl（`impl<P0..P12> GenA<(P0, ..., P12)>`）；`where @0..: SomeTrait` 约束它们
  - **`fresh!(...)` body 标记**——`type MyTuple = (fresh!(@(@T,)..))` 展开为 `(P0, P1, P2)`（body 里的 repeat 块 / fresh 引用，用合法宏调用拼写包装）；标记完全展开——输出永不含 `fresh!` 调用
  - **块模型**——矩阵的每个元素把自己的 `impl{...}` 模板与容器配对（`[[Box,Rc]impl{A<(T@..)>}, Vec impl{Vec<(T@..)>}].().2..=3`）；`where{...}` 任意位置组合（模板区 + 矩阵区，多附件逗号合并）
  - **文本替换**——impl 的 for-Type 不必逐 ident 镜像模板（非匹配模式）
  - **变长段模板**（`A<(T@..)>`）驱动 `fresh!` 段引用
- **运算符字典**（内部）——`read_op` 成为多字符运算符形状（`..` / `..=` / `->` / `::`）的唯一权威；散落的 `Spacing::Joint` 守卫（含 `scan_stop` 的三个）统一收敛，重复的载体提取 join 去重。

## 0.9.5 (2026-08-27)

> 直接拼接收尾 + DSL 人体工学版：0.9.4 载体重建遗留的段拼接 TODO 落地（repeat 块直接发射绑定叶子，`$( ... )*` 语义），impl 模板家族获得裸写/相邻拼写与逗号并列开关，`@{N}` body 槽规则收紧（"用必先声明"——breaking 收紧），repeat 块内 fresh 名称引用拼写统一为 `@{N}`（原 `@@N`）并新增逐轮游标形态 `@{@N}`。注意两个 **breaking** 行为变化：`@@N` → `@{N}`、`@{N}` body 槽必须声明——针对 0.9.4 写的 spec 需要相应修改。

- **body 侧段引用改为直接拼接**——repeat 块内 `@A` 现在把段的第 i 个**绑定元素**直接拼进该轮输出（与 Rust 声明宏 `$( ... )*` 同一语义）：`impl{(A@..)}` 匹配 `(u8, u16, u32)`、body 写 `(@(@A::from(self.@0)),..)`，一步展开为 `(u8::from(self.0), u16::from(self.1), u32::from(self.2))`。中间的 `@{A_pos}` 载体拼写删除——展开与最终 impl 之间不存在任何可见的 token 机制；手写 `@{...}` 若承载的不是 fresh 位置引用则报错并给出指导。要按名字引用某个特定元素，就在段旁写一个普通固定槽——`impl{(A0, @A..,)}` 绑定 `A0 := ` 叶子[0]，body 裸写 `A0`；`@A..` 本身不衍生任何名字（不会声明 `A1`/`A2`）。repeat 块驱动段、`@N` 游标、轮间分隔符、嵌套轮次、纯游标块均不变。
- **组合展开链封顶**——数组×range 组合（`([T,T].0..3).0..3...`）此前每层嵌套无检查地倍增展开规模，可能耗尽内存（表现为编译器挂起/OOM 而非诊断）。现在每个增长点都执行既定的展开上限（1024），超限的 spec 报出惯用的 "expands to N impls (limit 1024)" 错误。
- **AsyncFn / AsyncFnMut / AsyncFnOnce bound**——async 闭包 trait 可作为可调用类型在 bound 中解析（`F: AsyncFn(u8)`）；结构与既有 `Fn` 家族一致。
- **lifetime 在 spec 中一等公民**——`<'a>` 声明、`'a` trait 实参、`+ 'a` bound 元素端到端可用；作为操作数误用报错并给指导。
- **where 继承按位置替换**——改名后的实参也能继承 trait bound 与 where 谓词：`<X, Y> Store<X, Y>` 将 `HashMap<T, K>: Send` 替换为 `HashMap<X, Y>: Send`；具体实参退化为普通谓词（`<K> Store<u32, K>` → `where u32: Clone`）。路径段（`A::B`）永不替换。
- **固有 impl**——`#[batch_impl(spec)] impl Type { … }` 接受与 impl trait 入口相同的 spec 语法（形状形式 / 直接形式），生成普通 `impl Type` 块；该形态下 `@trait` 不可用。
- **repeat 块输出预算**——嵌套 repeat 块的轮数相乘（笛卡尔积语义）；展开现在携带输出 token 预算（每 body 65536），嵌套乘积超限时报 `repeat-block expansion produces N tokens (limit 65536)`，而非无界输出。
- **裸 `impl` 拼写与裸 `where` 同款收集**——`impl (A@..) {body}` ≡ `impl{(A@..)} {body}`；相邻裸 `impl` 区像相邻 `where` 区一样拆分（`impl A<B> impl @{}` ≡ `impl A<B>, @{}`），各自收集成独立 `impl{...}` 模板合并进同一槽映射。
- **`impl{...}` 附件可逗号并列**——一个 `impl{...}` 块可同时携带多个开关/模板（`impl{(A@..,), @0.., @{}}`），按 depth-0 逗号逐段独立分类——与拆开的多个 `impl{...}` 块等价。
- **`@{N}` 需要声明的 body 槽**——body 里的 `@{N}` fresh 引用必须声明 `impl{@{}}`（或 fresh 绑定开关 `impl{@0..}`，其轮次消费 `@{N}`）——"用必先声明"规则；未声明时报错并给指导。宏注入的载体（blanket 投影、替换的 fresh 范围占位）按形状豁免。
- **`@@N` 统一为 `@{N}`**——repeat 块内外的 fresh 名称引用拼写一致：`@@0` → `@{0}`、`@@1` → `@{1}`（单 `@` 消费）。
- **`@{@N}`——逐轮 fresh 名称引用**——repeat 块内 `@{@N}` 命名该轮自己的 fresh（`@N` 是游标）：三个 fresh 上 `(@(@{@N}::foo()),..)` 展开为 `(P0::foo(), P1::foo(), P2::foo())`——一个 fresh 一个名字，由 `impl{@0..}` 驱动。

## 0.9.4 (2026-08-25)

> blanket 委托与 `#delegate` 改名工作——由用户对 batch-impl 与 `auto_impl` / `delegate` / `impl-trait-for-tuples` / `fortuples` / `trait-gen` 的手动并排实测驱动（逐行核对双方 cargo expand 完整展开）：auto_impl 的 GAT + 关联类型转发、delegate 的 `#[call(...)]` 改名是缺失的两个能力，现已补齐。在此基础上宏元层内部重建：结构化载体替换全部保留名，fresh 显示名撞名按电子表格式字母后缀逃逸（`P0A`、`P0B`、……）——编号从不跳过，`@N` 对应关系保持稳定。

- **`#blanket` GAT 投影**——trait 里的泛型关联类型（`trait Iterable { type Iter<'a> where Self: 'a; }`）现在委托为 `type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;`——GAT 自身参数穿过投影（此前裸 `<T as Iterable>::Iter` 报 E0107 "missing lifetime argument"）。普通关联类型/常量保持既有投影（`type Item = <T as Trait>::Item;`）
- **`#blanket` 裸 `Self` 参数/返回诊断**——方法带**裸 `Self`** 参数或返回（`fn new() -> Self`、`fn cmp(&self, other: Self)`）无法 blanket 委托：转发产生内部类型，与包装类型的 `Self` 不匹配。此前生成的 impl 报 rustc 通用 E0308/E0614；现在宏定向报错并给指导（该 wrapper 改用 `#name{...}`）。`Self::Assoc` 投影返回（`fn iter(&self) -> Self::Iter`）合法放行——内部 `T` 携带同一关联类型
- **`#blanket` `@?` 后缀——非 Sized 包装**——wrapper 元素以 `@?` 结尾（`Box@?`、`Box<Rc@?>`……后缀随链到达最内层）给该 spec 的 where 子句加 `T: ?Sized`，fresh 泛型可为非 Sized 目标（`#blanket(@all){Box@?}` + `Box<dyn Trait>` 现在可用；不加时 `T: Trait` 隐含 `Sized`，dyn 目标失败）
- **`#delegate` 改名——`foo = call_foo`**——元素 `size = len` 把 trait 的 `size` 方法委托给目标的 `len` 方法（delegate crate 的 `#[call(...)]` 机制，用 DSL 的 `=` 绑定拼写）：签名保留 `size`，只有调用用 `len`。绑定语义：每个被选方法绑定一个目标——默认同名，改名则绑定 `=` 右侧。改名的左侧**尚未选中**时把该方法加入选择集（`#delegate(size=len)` 单独选中 `size`）；与选中集重叠时合并（`#delegate(@all, size=len)`——`size` 转发给 `len`，其余同名——不产生重复定义）；同一方法改名两次（`#delegate(size=len, size=other)`）编译报错
- **可读 fresh 泛型——`P0, P1, ...`**——生成的 impl 不再暴露内部保留名：每个 fresh 泛型显示为 `P0, P1, ...`（P = Param，索引与 `@N` 一致，教程早已使用的拼写）。impl 内撞名（用户泛型或类型叫 `P0`）把该 fresh 逃逸为 `P0A`、`P0B`、……（电子表格式字母后缀，双射 base-26）——编号从不跳过，`@N` 对应关系保持稳定
- **生成的诊断卫生化**——DSL 错误在生成代码里发出的 `compile_error!` 拼写为 `::core::compile_error!`（绝对路径），用户自己的 `compile_error` 宏或模块无法遮蔽
- **`X<>` 在 `+` 连接的 bound 列表内同步**——开关模板（`impl{Tr<>}`）激活时，`+` 链里的空尖括号 bound（`<T: A<> + B + C>`）与任何 `X<>` 一样同步（结构化 bound 列表逐元素同步）
- **fresh 范围在 impl body 内重新展开**——`#map` 复制的签名按字面替换了 trait 泛型实参，可能把 `(@0..)` 占位符带进 body；body 后处理现在也在此重新展开 fresh 范围占位符（`(@0..)` → 对照本 impl 的 fresh 列表展开为 `(@0, @1, ...)`）
- **repeat 块轮间分隔符**——repeat 块 `@(...)..` 现在接受显式**轮间分隔符**：`@(A,)..` 逐元素重复 `A,`（`A, A, A`）——块内尾逗号即分隔符、随之重复，并排生成的元素正确连接
- **fresh 数驱动 cursor-only repeat 块**——cursor-only 块（`@(args.@0,)..`，无模板变长段驱动）现在**每个 impl fresh 泛型重复一次**——fresh 数即重复次数——**fresh 绑定开关**（`impl{@0..}`，形状模板的 fresh 范围形态）显式声明作用域并启用 `@@N` 名称引用（被绑 fresh 的名字本身，如 `@@0` → `P0`——写名字而非位置）
- **空元组折叠精确化**——范围占位符仅在元组**顶层**含范围占位符时折叠成真正的 1 元组（`(@0..)` → `(P0,)`）；普通 `(expr,)` 元组按字面保留尾逗号（`(a, b,)` 保持 `(a, b,)`）——折叠绝不改写非范围元组

## 0.9.3 (2026-08-22)

- **生成式 Fn 类型：Fn/FnMut/FnOnce 结构化**——Fn 家族不再是 passthrough：`Fn` / `FnMut` / `FnOnce`（以及裸 `fn`）带参数列表结构化解析，生成器可以在内部运行（`Fn()2` → `Fn(P0,P1)`；`Fn()0..4 R` → 每个元数一个 `Fn(P0..Pn) -> R` 形态）。空格形态更优：`Fn()N` ≡ `Fn.().N`，`.` 可省略。`dyn` / `for<'a>` 包装也结构化，生成器可穿透（`dyn Fn()2 + Send` → `dyn Fn(P0,P1) + Send`）与嵌套包装（`Box<dyn Fn()2>`）；fresh 参数乘到 impl 泛型
- **bound 生成器驱动 spec 级展开**——**impl 泛型 bound** 内的生成器（`<R, T: Fn()0..4 R> Tr<T> (@0..)`）每个元数生成一个 impl、bound 钉死在该元数（`T: Fn(P0,P1) -> R`），目标的 `@0..` 对照该 impl 自己的 fresh 列表重新展开——"Fn 元数 × 元组元素是同一批泛型"，一个 spec 覆盖所有元数
- **`(@0..)` 无逗号范围元组**——括号里范围占位符的尾逗号可选（`(@0..)` ≡ `(@0..,)`）：无逗号形态对元数 1 仍渲染真正的 1 元组 `(P0,)`（绝非组 `(P0)`）
- **裸 `where` 无需代码块**——spec 末尾的 `where A: Clone` ≡ `where A: Clone {}`（谓词区在 spec 末尾结束；既有 `impl{...}` / `where` / `;` 边界不变）。尾随 `{}` 不再必须
- **空格形态生成器拼写**——容器/修饰符与矩阵源之间的 `.` 可选：`()N` ≡ `().N`、`(A,)N` ≡ `(A,).N`、`*()N` ≡ `*().N`、`Box @u*` ≡ `Box.@u*`、`[Box, Rc] u32` ≡ `[Box, Rc].u32`（空格是首选拼写，除真正嵌套如 `Box.Box.u8` 之外）
- **`@all_fresh` 弃用**——等价于 `@0..`；文档标注弃用并推荐 `@N..` 家族（实现保留以兼容）
- **`@Cow` 文档化为 `#blanket` 专属**——包装常量是 `#blanket` 包装列表的内置（绝非自定义常量）；文档已说明，`@` 记法表格展示各常量展开结果

## 0.9.2 (2026-08-21)

- **`@N..` 范围引用随处可用**——开放（`@1..`）或闭合（`@0..=1`）fresh 范围现在可以出现在**任何单个 `@N` 能出现的位置**：where 谓词（`@1..::Output: Clone`——范围后的尾部逐 fresh 复制，关联类型路径随之携带）、`<>` 泛型实参（`Wrapper<@0..>`——一个占位位置重新展开为多个）、元组目标，以及 **impl 泛型声明位置**（`<@0..>` 把范围覆盖的每个 fresh 声明为 impl 参数——生成器放在 trait 实参，如 `<@0..> GenConv<*().2> T`）。**组内范围** `@L_N..` / `@L_N..M` / `@L_N..=M` 在**单个生成器组内**切片（`@g_i` 的组内对应物，跨数组分发稳定）。范围在 parse 时折叠成单个占位标识符、codegen 时对照 impl 的 fresh 列表重新展开；旧的"范围引用仅限 where 谓词主体"限制已移除
- **变长段不再需要尾随逗号**——`impl{(A@..)}`（此前需写 `impl{(A@..,)}`）现在可用：元组模板末尾的段自动补逗号，模板仍解析为元组
- **历史日志运算符还原**——CHANGELOG / dev-changelog 中 0.8.x 及更早的条目恢复使用当时的 `^` 运算符（0.9.0 发布时被机械改写成 `.`）；0.9 之前的文档不再误述旧语法

## 0.9.1 (2026-08-21)

- **类型起始运算符定向诊断**——`+A` 位于 spec 开头时此前会**静默生成 0 个 impl**、无任何诊断；现在报 "`+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`)"。`!`（never）块不再吞掉尾随 `{...}` body：`fn(A) -> ! { body }` 的 body 归属 impl 而非丢失
- **文档化 `self` 恒等前缀**——`self.T` = `T`；在矩阵中作**裸类型占位**（`[Box, self] u8` 同时生成 `Box<u8>` 与裸 `u8`）。0.9.0 文档未提及；§10 修饰符大全现已有
- **文档稳定性修订**——zh-CN 教程修正：§4.5 splat 幂示例不再泄露内部 `_Param_*_BatchGen_` 名（现为 `impl<P0, P1>`）、§4.3 补 `Frac<*(*@u*).2>` 36 impl 示例、§10 补 `!` 作 fn 返回类型、§11 `batch_trait!` 行不再声称支持 `#` 指令（实际拒绝）；`# path::to::Trait:` 外部路径前缀与 `:N` deref 深度也补入英文教程

## 0.9.0 (2026-08-21)

- **Breaking：apply 运算符重命名**——`.` 现在是右结合 apply 运算符（旧 `^` 的语义，矩阵写法 `[Box, Rc].u8` 不变），**空格应用取代 `-`** 作为左结合组合（`Box u8` = `Box<u8>`、`HashMap K V` = `HashMap<K, V>`、`<T: Clone> Vec<T>` = 声明应用到类型）。`-` 前缀只保留**指令域**的排除语义（`#fill(@all,-foo)`——指令实参，绝非类型运算符）；类型域里的裸 `-` 现在定向报错而不是被静默误解析
- **块模型**——DSL 现在是**块的任意组合**：声明、指令块、代码块、类型任意顺序出现，链用 `apply` 折叠（无位置要求——`{...}` 块不必在末尾、声明不必在开头；`<T> #tag{"ab"} Box T` 任意顺序得到同一 impl）。空格应用 = 块的任意组合（`<T> #foo{} Box T` = `<T>.apply({...}).apply(Box).apply(T)`，无硬性顺序）
- **同名泛型声明合并**——`<T: Clone><T: Copy> X` 不再报"重复 T"：名字只声明一次（裸名），所有 bound 移入 where 谓词（`impl<T> ... where T: Clone, T: Copy`）；单次声明保留内联 bound
- **形状模板 `_` 通配**——`impl{B<_>}` / `impl{[A; _]}`：下划线是**永不替换**的占位符（绑定位置推断，`_` 保持 `_`），支持部分固定的模板
- **`X<>` 同步为本 spec trait 应用**——where 谓词、`impl{...}` 模板、impl 泛型 bound（`<T: Semiring<>>`）或——通过**开关模板** `impl{Tr<>}`（不参与 Self 匹配，只声明同步）——body（`<Self as Semiring<>>::Assoc`）里的同名空尖括号 trait（`Semiring<>`）展开为本 spec 的 trait 应用（`Semiring<Additive, Multiplicative>`），trait 实参只写一次（spec trait 部分）而无需在每个谓词重复；`@trait<>` 等价（`@trait` 先展开为 trait 路径，顺带消掉长外部路径）。任何非本 spec trait 的 `X<>` 报错；无泛型参数的 trait 同步为裸名（`Tr<>` → `Tr`）。开关模板支持路径限定（`impl{mod::Tr<>}`——`@trait` 展开为完整路径，含 `batch_impl_only` 外部路径）

## 0.8.3 (2026-08-19)

- **移除内置指令拼写守卫**——`check_builtin_typo`（Levenshtein 距离检查：名字接近 `fill`/`delegate`/`blanket` 就用 `compile_error!` 报"did you mean"）整体删除。它还会误伤**单指令 `#name{body}`**：trait 方法恰巧叫 `fill`、`delegate`、`blanket`（或近似名如 `delegate_to`）会被直接拒绝。过程宏没有警告通道，`compile_error!` 不是约束名字的正确方式——开放扩展拼写错误现在正常展开、由 rustc 自己的"macro not found"暴露，trait item 撞名则按字面工作

## 0.8.2 (2026-08-19)

- **where 谓词 `@N` 值引用与 `@N..` 开放范围**——`@N` 现在也解析尖括号组内（`Module<..., Scalar = @0::Scalar>`——谓词 tail 与每个组都会被扫描，与 `resolve_at_refs` 同形），关联类型绑定可以引用另一个 fresh 的关联类型（alga2 元组 `Module` 标量相等约束）；`@N..` 开放范围覆盖"从 N 到最后一个 fresh"，N 越界时**为空**（arity 1 的 impl 不产生"从第二分量起"的谓词——不报错）；空谓词（开放范围无可发射项、尾逗号空段）从 where 子句中丢弃，不再输出悬空逗号
- **变长段与重复块（shape template）**——`impl{...}` 模板可用 `ident@..` 声明**变长段**：覆盖从自身位置起的所有剩余元组位置，名字**对齐叶子位置**（`(u8, A@..,)` 匹配 `(u8, u16, u32)` → `A1`、`A2`；`(A@..,)` 任意 arity → `A0..An`），同层多段均分剩余位置（无法均分 / 段名前缀重复报错），段可递归进嵌套元组。body 用 `@(...)..` 重复：`@ident` 为第 i 个元素的槽名（随后由槽映射改写成绑定值），`@N` 为索引游标（展开为 `N + i`，路径前缀由用户书写），块按驱动段的公共长度逐轮输出（引用的段必须等长；块的长度有三个来源：块内 `@ident` 引用、**前置段声明** `@A(...)..`（`@` 后直接写段名，支持纯游标块）、或纯游标块用模板**唯一段**），嵌套块独立轮次（笛卡尔积），块体尾部 `,` 为每轮分隔符（并列块之间不写逗号）。一条 spec 覆盖形状的所有元组 arity——alga2 风格 `().1..=4 where{@all_fresh: Magma} impl{(A@..,)} #combine{( @(@A::combine(&self.@0, &rhs.@0),).. )}` 为 n = 1..4 生成 `impl<A0..An> Magma for (A0, ..., An) where A0: Magma, ...`

## 0.8.1 (2026-08-18)

- **修复：`where{...}` 谓词组配对尖括号**——`where{...}` 块内的两参数 bound（`@all_fresh: Semiring<Additive, Multiplicative>`）此前被深度 0 逗号分裂成坏谓词，因为 Brace 组透传、`<>` 保持扁平。`angle_collect` 现在进入 `where{...}` 组并配对组内 `<...>`（代码体仍透传）；`render_angles` 还原。真实使用中发现（alga2）；DSL 端到端回归测试锁定

## 0.8.0 (2026-08-18)

- **shape-match 增强（impl entry / shape template）**——`impl{...}` / ItemImpl 模板匹配覆盖全部 `syn::Type` 形态（切片、任意元数元组、定长数组、引用/指针、多段路径）；定长数组长度写成裸 const 参数名时绑定叶子长度（`[A; N]` → `N := 3`），`'_'` 匿名生命周期为通配匹配任意叶子生命周期——由此支撑**原型实现模式**：为代表性叶子写一个正确实现，"相同→保留、不同→绑定"规则自动适配整个矩阵（`[Box,Rc].@num impl{Box<u8>} #max{Box::new(u8::MAX)}` → 28 个 impl；`Cow<'_, @num> impl{Cow<'_, u8>}` 覆盖含生命周期的 Cow 族；多族可在一条属性内组合）。fn 指针 / trait 对象模板与跨类实参（生命周期/const vs 类型）保持逐字并定向诊断
- **impl entry：`#[batch_impl]` ItemImpl 入口**——属性宏同样接受 `impl` 块并批量实例化：DSL 描述形状模板 × 矩阵源（`A<B> : [Box,Rc].[usize,isize]`），每个矩阵叶子产出一个 impl，槽映射（共享 shape-match 内核：相同 ident 保留、不同则绑定）重写 for-Type / where 谓词 / body；原始 impl（for-Type 含占位槽名）被 withhold。`@trait`（→ impl 的 trait path）允许在泛型声明 bound 与 where 谓词中；自定义 `@` 常量 / `@N` 引用 / `#` 指令在本入口拒绝；`;` 分隔多个 spec（单 spec 为常见形态）；裸 where 谓词区域新增深度 0 `;` 与（仅 ItemImpl）流末尾终止
- **shape template：`impl{...}` Self-part 形状模板**——trait 入口新增第三种尾随附件（`T impl{...} where{...} {body}`，任意顺序）：块内是标准 Rust 类型模板，与矩阵叶子目标类型**逐位匹配**——与目标同位置的 ident **相同** → 字面保留；**不同** → 绑定槽，替换进目标/where/body（`Box<u32> impl{Rc<T>}` → `Rc := Box, T := u32`；`[Box, Rc].u32 impl{W<T>}` → 每叶子一个 impl，`W` 绑定叶子 base）。多个 `impl{...}` 合并为单一映射（同形冗余合法、异形冲突报错）；模板内 DSL 算子、形状不匹配、附件深度超限均定向报错
- **回退：属性宏自定义 `@` 常量**——移除 0.7.2 误加的特性（`#[batch_impl]` / `#[batch_impl_only]` 的前导 `@name=value;` 段）：自定义常量段仅 `batch_trait!` 可用（属性宏中出现定义段报"custom constants are not supported"）；属性宏矩阵直接用 `.` / `-` / `*` 书写（属性宏的未知常量消息不再带"定义须先于引用"后缀）
- **风格打底**：`rustfmt.toml` 移除宽度上限（`max_width` / `fn_call_width` / `struct_lit_width` / `struct_variant_width`），回归固定四行配置；全库按新配置重新格式化
- **文档**：`examples/simplify.rs` 注释由中文译为英文（DSL 内容不动）；`examples/quickstart.rs` 注释同步英文化；`docs/architecture.md` 测试矩阵数字更新（`tests/features/` 拆分后 dsl 167、ui 74 compile_fail）
- **去 panic 承诺加固**：生产代码不再有任何可能 panic 的路径——畸形或对抗性 DSL 输入一律产出诊断或展开（修复单 token `#blanket` wrapper 在 debug 构建下的 underflow；笛卡尔积展开改为分配前检查规模，超大矩阵报上限而非耗尽内存）
- **扁平链深度护栏**：`.`/`-` 算子链、尾部 `{...}`/`where{...}` 附件链、链式类型段（`<T><U>...X`）统一 128 层上限——数百个链式单元此前会使编译器栈溢出（STATUS_STACK_OVERFLOW）；解析层现在输出定向诊断（fuzz 词表新增 `@`/`.`/`'`/`+`/`?` 及常量系统词，覆盖常量/range/生命周期路径）

## 0.7.2 (2026-08-14)

- 诊断语言用户化：`@N`/`@g_i` 越界或悬空引用不再泄露 `_Param_*_BatchGen_` 保留名——where 谓词与目标类型/trait 实参位置统一按用户语言定向报错（此前类型位置的悬空引用会以 rustc E0412 裸错暴露内部命名）
- **`batch_preview!`**：DSL 感知的展开预览——把 `#[batch_impl(...)] trait` 原样包进去，通过诊断通道逐 impl 展示真实展开（走真实管线）；附带预览独有提示：已知一元容器渲染出 2+ 实参（`Box<Vec, u32>`）即 `.`/`-` 结合性误写形状（`A^B-C` = `A-B-C`），提示嵌套改写（`Box^Vec^u32`）；编译器路径从不猜测
- **trait 实参中的生成器 splat 声明提升**：`Conv<*().2> X` 现在生成 `impl<P0,P1> Conv<P0,P1> for X`（此前声明被丢弃 → rustc E0412 裸错暴露 fresh 名），与泛型实参位置同一规则；泛型声明位置的生成器（`<*().3>` / `<*(().3)>`）改为定向报错（fresh 声明无载体），不再渲染 `impl <<P0,..> *(P0,..)>` 垃圾代码
- **`#blanket` 按值接收者修复 + doc 提示**：`fn consume(self)` 的委托体 deref 少一层——按值 `self` 本身就是包装，此前统一 `**self` 多解引用了一层内部类型（E0614）；现在 `(*self).consume()` 对 `Box` 等可移动包装正常通过（`&`/`Rc` 等共享包装移出仍不可过，生成的 impl 携带 `#[doc]` 提示：建议 `@all_ref_methods` 或 `#name{...}`；proc macro 无稳定 warning 通道 E0658，doc 通道零编译风险）
- **开放扩展协议收敛**：顶层 `{! m!{...}}` 为唯一推荐形态，内嵌 `T {m!{...}}`（无 `!`，输出关联项）标注弃用、保留兼容——proc macro 无 warning 通道，收敛落在文档层（tutorial §7.5 / crate 文档 / architecture）
- **语法面冻结承诺**：全部既有记号语义视为 final（README 新增承诺节 + architecture 扩展准则 + tutorial §6.4 power-user tier 标注）——后续只做加法、诊断精化与文档，改动既有语义即刻意破坏性发布
- **属性宏自定义 `@` 常量**（**0.8.0 已回退**）：`#[batch_impl]` / `#[batch_impl_only]` 支持与 `batch_trait!` 相同的前导 `@name=value;` 常量段（懒展开、链式引用、循环/前向引用拒绝）；定义不在前导位置时定向报错

## 0.7.1 (2026-08-13)

- 诊断加固：类型位置的 `;`/`=`/`@`/`#` 残留、相邻类型缺少操作符（`A B`）、binding/bound 缺值、非整数类型字面量、range 端点非整数、数组长度畸形、类型起始 `+`/`?`/`.`、fn 参数列表后残留、blanket 返回 `Self`/`Self::Assoc` 的方法、未知指令拼写建议——全部定向报错而非渲染非法 Rust 或抛 rustc 裸错

## 0.7.0 (2026-08-10)

### splat 展开延迟到 codegen（bug 修复 + 语义明确）

splat（`*(...)` / `*[...]`）现在在 parse/apply 全程保持整体，只在生成 impl 时摊平成元素。常规场景行为不变（`T^*(A,B)` → `T<A,B>`、`[a, *[b,c]]` = `[a,b,c]`），两个此前损坏的组合现在正常工作：

- `Conv<bool> Pair^*(A, B)` → `impl Conv<bool> for Pair<A, B>`（原误解析成 `Pair<A<B>>`）；
- `Pair^*(A, B) #method{...}`（右 splat 后跟指令 body）→ `Pair<A, B>`（原产出 `Pair<*const (A, B)>`）。

### splat 存续：数组元素保持 splat 到消费

数组/列表元素若是 splat，不再 parse 时摊平——splat 活到右操作数 apply 或 codegen 摊平。这让 splat 幂驱动重复位置：

```rust
#[batch_impl(Pair^[*(SplatA),*(SplatB)]^2)]
// → impl Pair<SplatA, SplatA> + impl Pair<SplatB, SplatB>
```

有右操作数时，保持的 splat 走自身语义：`[A,B,C]^D` = `[A^D, B^D, C^D]`（裸列表分发）、`[A,*(B,C)]^D` = `[A^D, *(B,C,D)]` = `[A^D, B, C, D]`（元组 splat 追加）、`[A,*[B,C]]^D` = `[A^D, *[B^D,C^D]]` = `[A^D, B^D, C^D]`（数组 splat 分发）。无右操作数时，数组目标照常摊平（`[a, *[b,c]]` = `[a,b,c]`）。要纯分发请写裸列表（`[A,B,C]^D`）。

### trait 泛型实参：具体实参替换进指令 body

spec 级 trait 段带具体实参时，现在会把 trait 的泛型参数替换进指令抄写的 body：

```rust
#[batch_impl_only(
    From<bool>
    Frac.*(*@u*).2
    #from{
        Frac { positive: true, num: value.into(), denom: true.into() }
    }
)]
pub trait From<T>: Sized { fn from(value: T) -> Self; }
```

生成 `impl From<bool> for Frac<u8, u8> { fn from(value: bool) { ... } }` × 36——抄写的签名（以及你 body 代码里）的 `T` 会被替换成 `bool`。

### 泛型实参内的 splat 幂

`Frac<*(*@u*)^2>` 现在把幂的笛卡尔结果分发成逐对 impl——36 个，与右 splat 链 `Frac^*(*@u*)^2` 等价。数组实参分发只有一个权威位置：字面（`T<[A,B]>`）、常量（`T<@u*>`）与幂结果全部进 params 成 `TyArray`，在 `expand` 统一分发。

### 具体类型实参拒绝 binding/bound

binding（`Item = u32`）与 bound（`T: Clone`）只属 trait 路径（`Conv<Item = u32> X`）与泛型声明（`<T: Clone> Foo`）。具体类型的实参（`struct Assoc<T>` 配 `Assoc<Item = u32>`）现在报定向错误——此前 bound 被静默丢弃、struct binding 渲染非法代码。

### splat：`*` 前缀展开

- `*` 把容器/生成器**展开拼入**（只出现在 `[]`/`()` 之前）：
  - 列表内拼入：`[a, *[d,e,f]]` = `[a,d,e,f]`；`^`/`-` 右操作数扁平追加：`(a,b,c)^*(d,e,f)` = `(a,b,c,d,e,f)`（拼接）；`Vec^*(a,b)` = `Vec<a,b>`（多实参）
  - 生成器 splat：`(*(()^3))` = `(A,B,C)`（组变元组 + fresh 声明提升）
- 嵌套幂等、空 splat 无操作；**左操作数按来源括号分语义**：`*[...]^T` 分配（`*[A^T,B^T]`——集合，对标 `TyArray`）、`*(...)^T` 追加（`*(A,B,...,T)`——列表，对标 `TyTuple`）；**`*(A,B)^N` 幂把每个笛卡尔组合包回 splat**——`*(A,B)^2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]`；右 splat 链把组合摊平进容器——`A^*(*@u*)^2` = `A<u8,u8>`/`A<u8,u16>`/...（`A<@u*,@u*>` 的重复列表简写）；`*(A,B)^2` 单独作目标会摊平成重复（E0119）——元组 impl 用 `(A,B)^2`；**splat 只展开一层**——元组保持（`*((a,b),)` = 一个 `(a,b)` impl）、数组/嵌套 splat/生成器摊平；`*()^N` 保持 splat 形态供载体——`T^*()^2` = `<A,B>T<A,B>`；`#fill` 单元素推荐——写 `#name{body}` 而非 `#fill(name){body}`；`*const`/`*mut` 指针不受影响

### 分发传播与生成器修复

- 数组（分发列表）在嵌套位置（元组元素/泛型实参/pow_cartesian 组合）按笛卡尔积分发——`(u8, [u16, u32])` → `(u8,u16)`/`(u8,u32)`；`Vec<[u8,u16]>` → `Vec<u8>`/`Vec<u16>`
- 修复：`(T,)^N` 克隆含生成器的 T（如 `(()^3,)^3`）时同名 fresh 声明重复 hoist → E0403；现同名 fresh 只声明一次（共享语义）

## 0.6.7 (2026-08-08)

### `@N` 位置引用：每 impl 独立编号 + 目标类型支持

- **破坏性变更**：fresh 泛型编号现为**每 impl 独立**——每个生成的 impl
  把自身 fresh 参数按文档序重编号为 `_Param_0..N_BatchGen_`，`@N` 恒指
  *本 impl* 的第 N 个 fresh。这修复了单元漂移：`@0` 现可在跨 spec 与
  range 生成的 impl 中使用（此前计数器跨单元延续，后续单元 `@0` 报错）。
  组合场景（如 `()^3-()^3`）下 `@0` 是生成类型中第一个出现的 fresh
  （此前按声明顺序，与文档序不一致）；
- `@N` 现可直接用于目标类型（`Box<@0>`）；blanket 包装的位置标记
  （`(u32, @0)`）走同一通道。

### 开放扩展改为顶层（`#cmd` / `{! ...}`）

- **破坏性变更**：开放扩展 `#cmd(args){body}` 现为**顶层**——宏调用收到
  `{spec}(args){body}trait`（4 段，spec 主体在最前）并生成任意 item，
  通常是自己完整的 impl；batch-impl 不再为它生成 impl。同一协议也可通过
  给 spec 附加 `{! m!{...}}` 使用（用户手写宏输入）。内嵌形态
  `T {m!{...}}`（无 `!`）仍把宏调用留在 impl body（关联项，用户手写完整
  输入含 trait）。

### `@all_fresh` / `@N..M` 批量 where 引用

- `@all_fresh: Bound` 约束全部 fresh 泛型；`@N..M` / `@N..=M` 约束连续
  fresh 段（`@0..=2: Clone`）；两者展开为逗号分隔的多谓词，越界/超大
  展开报错。

### 错误聚合

- 多个 spec 的错误一次全部报出，不再停在第一个错误。


## 0.6.6 (2026-08-07)

### `(T)^N` 分组剥离语义 + 数字渲染无后缀

- **破坏性变更**：`(T)^N` 此前（0.2.0 起）生成长度 N 的重复元组 `(T, T, ...)`；
  现改为剥离分组等价 `T^N`（普通类型 `^N` 是 const 泛型实参：`(W).2 = W<2>`，
  其中 `W` 为带 const 泛型的类型）。依赖 `(T)^N` 生成元组的升级用户须改用
  `(T,)^N`；
- `(<T>)` 是错误语法（`(` 后 `<` 不是合法类型）；
- 数字/范围渲染不带 `usize` 后缀（`W<2>` 而非 `W<2usize>`、`[u8; 3]` 而非
  `[u8; 3usize]`）。

### 输入校验护栏

- `expand_consts` 深度守卫 128 层（超深 `[[[` 嵌套不再栈溢出）；
- `#blanket` `:N` 上限 128（`Box:999999` 不再栈溢出）；
- batch_trait! 常量定义拦截 `@all_*` 保留名（定义处报错）；
- `#blanket` `Box:`（冒号后空）在 DSL 层报错。

### `#delegate` 支持参数模式

- 可作表达式的参数模式（如 `(a, b)`）保留签名，委托调用直接以模式 token
  重建转发；`ref x`、守卫、`_` 及其嵌套形式（`(ref x, ref y)`）自动命名
  （`arg0`…）转发。

### 输入校验补全

- batch_trait! 常量定义拦截裸 `@all`；
- 常量值引用校验（check_value_refs）补 128 层深度守卫；
- 深度守卫前移（Group 递归前拦截）+ 类型注解模式（`x: u32`）委托回退命名；
- `#blanket` 包装支持 `@0` 位置标记：带 `@0` 时 T 可放任意位置
  （`(u32, @0)` → `(u32, T)`），不带则 `部分.T` 末尾附加；
- 新增 6 个空占位宏（batch_impl_delegate / fill / blanket / name / open /
  consts）作为指令文档入口——纯 doc 符号，展开为空，误调用无害。

## 0.6.5 (2026-08-06)


### 指令参数方括号写法：`#cmd[args]{body}`

- 指令参数支持 `(args)` 或 `[args]` 等价写法（如 `#fill[@all_methods]{0}`）——
  方括号在参数本身含括号时更清晰；错误消息与教程同步更新。

### 修复：宏调用 passthrough 洞

- `ident!(...)` / `foo!()` 的 `()` 参数组此前无条件进入递归——内部 `@` 常量被
  替换、`<` 被错误配对成角度组；此前只有 `[]` 组有 `!`/`#` passthrough 守卫。
  现在 `()` 组共享守卫（宏调用原样透传；`#name(...)` 指令参数与 DSL 元组仍进入）。

### 行为收紧：裸范围端点引用在定义处报错

- `@a=@u8`（无 `..` 的端点）此前通过 `check_value_refs`、使用处才炸；现在
  定义处直接报错（ui fixture `const_bare_endpoint` 锁定）。

### blanket `@0` / `@N` 统一到 codegen 解析

- blanket 包装 where 的 `@0`/`@N` 原样保留进 spec，由 `resolve_where_at` 与
  普通 where 谓词统一解析（blanket 的 fresh 泛型是唯一 fresh，`@0` 索引到它）；
  预处理只替换 `@trait`。行为等价、架构统一——"`@N` 是唯一 codegen 记号"对
  blanket 包装 where 也成立。

## 0.6.4 (2026-08-05)

### Apply trait 恢复：`apply` 右分发默认实现（span 兼容）

- span 改造时 `trait Apply` 只剩 `apply_help`（右分发被挪到 `TyKind::apply`
  普通方法）——trait 名与主方法名不一致；恢复之前设计：
  - `trait Apply: Clone + Into<TyKind>`——`apply(self, o, span)` 默认实现
    （右操作数结构分发，从 `TyKind::apply` 平移）+ `apply_help` 抽象钩子；
  - `impl Apply for TyKind`（覆写 `is_type_param` + 转发子类型）；
    子类型 `apply_help` 改普通方法（`impl X`，`pub(crate)`）——不再实现
    trait（默认 apply 的 `Ty::new(span, self)` 需要 Self: Into<TyKind>，
    子类型不满足）；
  - `is_type_param()` 默认方法（TyKind 覆写）替代 `matches!(self, ...)`
    ——泛型 Self 无法 match TyKind 变体；
- span 贯穿不变：`Ty::apply` 取 span → `kind.apply(o, span)`（trait 默认，
  每个构造 `Ty::new(span, ...)` 用左操作数 span，`o.span` 仅 fallthrough）；
- 测试全绿（分离声明顺序、数组/范围/泛型外提均回归）。

### `@trait` 提前展开（常量阶段/段级），`@N` 成为唯一 codegen 记号

- 问题：`where{...}` 是 Brace 组，`expand_consts` 原先不进入（body 的 `@` 是
  pattern 语法）——where 谓词里的 `@trait`/`@N` 都残留到 codegen 的
  `resolve_where_at`；`@trait` 不该留到 codegen（只有 `@N` 需要
  impl 泛型列表）；
- 修复三处：
  - `expand_consts` 识别 `where` Ident + Brace 组（DSL 结构非 body）→ 进入展开
    `@trait`（batch_impl 用 trait 路径）；`@N`（`@` + Literal）在
    `try_expand_at` 返回 None 保留（不再误报"must be followed by a name"）；
  - `replace_segment_trait`（batch_trait! 段级）递归进组——where{...} 谓词里的
    `@trait` 也能段级替换；
  - `resolve_where_at` 删除 `@trait` 分支——现在只处理 `@N`（签名去掉
    trait_name 参数），架构上"`@N` 是唯一 codegen 解析的记号"成立；
- 验证：batch_impl `where{T: @trait<T>}`（B1）、batch_trait! 段级 where 组内
  `@trait`（探针）都提前展开；`where{@0: Clone}` 纯 fresh 场景回归全绿。

### `@N` 位置引用语义修正：只索引 fresh 泛型

- `@N` 现在指 where 谓词内**第 N 个宏生成的 fresh 泛型**（`_Param_{N}_BatchGen_`
  形式）——用户泛型（`<T>` 等）**不参与 @N 索引**，直接写名字（`where{T: Default}`）；
- 与 blanket 包装谓词的 `@0`（= 目标泛型 fresh T）自然统一：blanket 只有一个
  fresh，`@0` 恰好是"第 0 个 fresh"，不再是特例规则；
- 破坏点：`<T> ... where{@0: Default}` 曾指用户泛型 T——改为 `where{T: Default}`
  （更自然）；越界报错更新（"impl has N fresh generics"）；
- 初衷：`@N` 本意就是 `_Param_N_BatchGen_` 的直接映射——fresh 编号是全局
  计数器、与最终位置无关（多 fresh 源/用户泛型混排时错位），故用"第 N 个 fresh"
  加固：位置可数、与编号无关、含用户泛型场景的纯粹性。

### 泛型参数族：`@all_type_params` / `@all_const_params` / `@all_lifetimes`

- 泛型声明照抄 trait 形参：类型参数只名字（`@all_type_params` → `<T, U>`）、
  const 完整声明（`@all_const_params` → `<const N: usize>`）、生命周期原样
  （`@all_lifetimes` → `<'a>`）；bound 由既有同名继承自动补；
- 用法：`#[batch_impl(@all_type_params GenT<T> Vec<T>)]`——声明与 trait 同步，
  改 trait 形参不必改宏；
- 组合（如 `@all_lifetimes @all_type_params`）保持生命周期在前——顺带修复
  了 DSL 分离泛型声明的顺序 bug（`<'a> <T> X` 曾生成 `<T, 'a>`）；
- batch_impl/batch_impl_only 专属（需要 trait_def）；trait 无该类参数时报错。

### `@` 常量名字族改名：`@uint`/`@int`/`@float` → `@u*`/`@i*`/`@f*`

- 名字族符号与范围族统一：`u`/`i`/`f` = 族、`*` = 通配全集——`@u*` 与
  `@u8..u128` 讲的是同一族（原 `uint` 与 `u` 符号不一致是概念裂缝）；
- 语义不变：`@u*` = `[u8, u16, u32, u64, u128, usize]`（含 usize），
  `@i*` = `[i8..isize]`，`@f*` = `[f32, f64]`；`@num`/`@scalar` 不变
  （`@num` = `@u* + @i* + @f*`）；
- **破坏性变更**：`@uint`/`@int`/`@float` 已删除（错误消息提示新名）；
- 实现：`builtin_named` 表 `u*`/`i*`/`f*` 通配（Ident + `*`，消费 3 token）；
  `check_value_refs` 同步识别通配（值内 `@u*` 引用）；ui 快照重生成。
## 0.6.3 (2026-08-05)

### 修正文档

- README（中文 + 英文）头部示例：`()^4` 的展开注释错误——`()^N` 是**单个** N 元组
  （`()^4` → 单个 `(A, B, C, D)`），原注释误写为 4 个不同长度的 impl；长度范围
  应使用 `()^1..=4`。仅修正注释，无行为变化。
## 0.6.2 (2026-08-05)

### 基于 span 的诊断

- 每个 `Ty` 节点携带源 `Span`（`enum Ty` → `struct Ty { span, kind: TyKind }`）；
  `Ty::apply` 取节点自身的 span 并在组合子输出中贯穿——`apply` 内产生的错误
  指向左操作数的位置；
- `compile_error_str` / `compile_err_at!` 接受显式 span；parse、常量、指令、
  blanket、apply 的错误全部接到肇事 token 的 span（`.` 缺操作数现在指向 `.`
  本身，而非整个宏调用）；
- 平台限制（rustc 行为）：属性宏输入的顶层 token 携带精确 span，但组内 token
  退化为 call-site span，且以 `Err` 返回的错误总是显示在宏调用行——真正显示
  精确位置的是 `Ty::Error`（Ok 输出）路径上的 parse/apply 错误；
- `compile_error!` 只把关键字标识符盖上目标 span、其余保持 call-site——若全
  token 都带 span，rustc 会把错误当作 item 位置的用户代码
  （"macros that expand to items must be delimited..."）。

### `#blanket` 静态方法委托

- `#blanket` 现在把无 receiver 的方法（静态方法 / `@all_static_methods` /
  `@all_methods`）经 blanket 泛型 `t` 转发——`fn make() -> u8 { t::make() }`，
  而非 deref 链委托体 `(**self).make()`（静态方法没有 `self`，E0424）；
- 直接调用、嵌套包装（`Box<Box<u8>>`）、参数转发都能经 `t: Trait` bound 到达
  底层 impl——与 assoc item 投影同一转发语义；
- 哲学统一：实例方法经 deref 转发、静态方法经 bound 转发，都是转发，不特判。

### 按 receiver 种类的 `@all` 过滤

- 新增 `@all` 族标记按 receiver 种类过滤 trait 方法：
  `@all_ref_methods`（`&self` / `&mut self`）、`@all_value_methods`
  （`self`，含 typed receiver）、`@all_static_methods`（关联函数）；
- 典型用法：`#blanket(@all_ref_methods){Box}` 只委托引用 receiver 的方法，
  绕开 by-value 委托对包装类型的语义模糊（by-value 方法回落到 trait 默认实现）；
- 与其余 `@all` 族一样被 `#fill` / `#delegate` / `#blanket` 与 `-` 排除共享；
  `batch_trait!` 报错（需要 trait_def）。

### 注释、错误消息与文档全英文化

- **注释与错误消息全部改为英文**（源码、测试、ui fixture）——受众更广；消息中的
  DSL 记号（`` `@uint` ``、`` `#fill` ``、`` `@0` ``）保持不变；
- **文档语言策略确立**：开发期以中文 doc（`docs/zh-CN/`）为主文档记录改动，
  发布前翻译为英文放入英文 doc（`README.md`、`CHANGELOG.md`、`docs/tutorial.md`、
  `docs/architecture.md`、`docs/dev-changelog.md`）；0.6.2 已完成英文版初译，
  中文版继续作为开发态主文档演进；
- 文档中的代码示例不变（兼作 doctest——46 个全过）；
- 修复了 tutorial 中段级 `@trait` 示例的损坏围栏（`` `ust `` → `` ```rust ``），
  顺带纳入 doctest 覆盖。
## 0.6.1 (2026-08-05)

### 新特性：`@all_required*` / `@all_default*` 范围标记

- 指令范围按 trait item 的**默认实现状态**过滤（fn 带默认体 / const 带默认值 /
  type 带默认类型 = default；无默认 = required，impl 必须提供）：
  - `@all_required_methods` / `@all_required_constants` / `@all_required_types` / `@all_required`；
  - `@all_default_methods` / `@all_default_constants` / `@all_default_types` / `@all_default`；
- `#fill` / `#delegate` / `#blanket` 三指令与 `-` 排除通用；
- 典型用法：`#fill(@all_required_methods){...}` = 只实现必须的、默认方法保留
  trait 默认实现（此前需 `@all` + 逐个 `-name` 排除）；`@all_required*` 与
  `@all_default*` 组合可分别填充两类（required ∪ default = all）。

### 修复：`@` 常量先于 `<>` 配对（`@ <> # where` 预处理顺序）

- 此前管线为 `<> @ # where`：`batch_trait!` 里 `Vec<@inner>` 这类
  常量值含 `<...>` 的写法，`@inner` 被配对进尖括号组后不再展开，
  残留到输出报 `found '@'`（`@map = HashMap<u32, String>` 直接值
  恰好被定义处配对兜底，嵌套/引用场景暴露）；
- 修正为宏元层最外：`@` 展开先于 `<>` 配对，展开产物（含扁平
  `<...>`）统一由 angle_collect 配对；
- `batch_impl`/`batch_impl_only` 支持内置 `@` + `<>` + `#` + where；
  `batch_trait!` 支持自定义 `@` + `<>` + where（`#` 需 trait 定义，函数式
  宏不可用）。

### 宏元层完整化：`@` 是唯一宏元记号

- **`#all` 系删除，全部迁移为 `@all` 系**（`@all` / `@all_methods` /
  `@all_constants` / `@all_types` / `@all_required*` / `@all_default*`）：
  `#` 只剩指令名格式，范围选择归宏元层——`#fill(#all)` 写作
  `#fill(@all)`，减法不变（`#fill(@all, -foo)`）；
- `@all` 展开为 `[item, ...]` 列表；指令参数支持手写 `[a, b]` 与
  `-[a, b]` 排除；
- **trait 感知常量**（`#[batch_impl]` / `#[batch_impl_only]` 专属；
  `batch_trait!` 无 trait 定义、遇之报错）：`@trait`（本地 trait 名）、
  `@Cow`（`Cow<'_>` + 固有约束打包）；
- **blanket 包装约束谓词**：`{Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}}`
  ——解决 deref target ≠ T 的包装（`Cow` 的 deref target 是 `T::Owned`），
  `@0` 指目标泛型；普通 where 谓词中 `@N` 为通用位置引用（元组\n  `()^2 where{@0: Clone}` 等）；「`<>` 只留名字、约束全进 where」后约束合并 =
  并列谓词（零分析）；普通 impl 的 `<T: Clone>` 写法保持兼容。

### 文档修正：`batch_trait!` 的指令边界明确化


- 此前 `lib.rs` 文档与 `docs/tutorial.md` 声称 `batch_trait!` 的 spec 语法
  "与 `#[batch_impl]` 相同"——实际 `batch_trait!` **不支持 `#` 指令**
  （`#fill`/`#delegate`/`#blanket`/开放扩展），遇 `#` 直接报错；
- 原因：指令需要 trait 定义作签名真相源，`batch_trait!` 是函数式宏、拿不到
  定义。需要指令请用 `#[batch_impl]` / `#[batch_impl_only]`（与 `A<>` 照抄、
  泛型 bound 继承的既有限制同源）。
- 无行为变化：`batch_trait!` 支持 `@` 常量与全部类型 DSL，仅文档如实声明
  指令边界。

## 0.6.0 (2026-08-04)

### 新特性：`@` 常量系统（类型矩阵命名复用）

`@` 常量在预处理阶段展开为字面列表，与手写逐 token 等价：

- **内置名字族**：`@uint` / `@int` / `@float` / `@num` / `@scalar`
  （如 `#[batch_impl(@scalar)]` 一行生成 16 个 impl：u8..char）；
- **内置范围族**：`@u8..u128` / `@i8..i128` / `@f32..f64`（**含端点**，
  宽度校验；`@u8..u128` = `[u8, u16, u32, u64, u128]`）；
- **用户自定义**（仅 `batch_trait!`）：前导 `@name=值;` 段，后续段落跨
  trait 复用。值是**任意 token**（**懒展开**——原样入库，引用处拼接后递归
  展开），可直接写 DSL 运算（`@wrapped=[Box,Rc]^@num`）或链式引用其他常量
  （`@chain=@wrapped`）；循环引用（`@a=@a`）与前向引用（`@a=@b` 定义在后）
  在定义处报错；
- 未知 `@xxx`、范围端点非法、自定义与内置重名均 `compile_error!`。

### 新特性：`#blanket(methods){包装列表}` — 覆盖式委托

`#blanket(#all){&,Box,Rc}` 为每个包装类型生成一段完整委托 spec——免写包装
矩阵与委托体。先给内部类型实现 trait，再 blanket 覆盖包装
（`impl<T: Trait> Trait for Box<T>` 等）。

- **包装元素为任意类型表达式**：`&`/`&mut`/`Box`/`Rc`/`Arc`/自定义智能指针/
  嵌套（`Box^Arc:2` → `Box<Arc<T>>`）/预填（`Cow<'_>` → `Cow<'_, T>`）；
- **`:N` 深度标注**：委托体 `*` 数量 = N + 1（`Box^Arc:2` → `***self`），
  默认 1——宏不猜包装内部 Deref 层数，嵌套须显式标注；
- **泛型 trait 支持**（`trait Foo<X: Clone>`）：trait 形参照抄为 impl 泛型 +
  实参填参数名 + trait 级 where 谓词透传（`impl<X: Clone, T: Foo<X>>
  Foo<X> for 包装<T> where ...`）；
- **assoc type / const 委托**：`#all` 含 const/type 项时生成投影
  `type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`——
  带必需关联类型的 trait 也能 blanket 覆盖；
- `*const`/`*mut`、`self`、空元素/非法 `:N` 报错，引导手写 `#delegate`；
  by-value receiver 方法委托语义依赖包装的 Deref/move 能力，维持全放行 +
  rustc 兜底（文档警示）。

### 行为变化

- 指令展开协议改为 `Vec<TokenTree>`（内部）：既有指令产物不变（单 `{...}`
  组），`#blanket` 等多产物指令成为可能。用户无感知。

### 文档

- README 精简为推销版（为什么要用它、心智模型、快速开始、特性一览），
  完整教程独立到 `docs/tutorial.md`，开发者文档独立到 `docs/architecture.md`
- CHANGELOG 拆分为本文件（用户可见变更）与 `docs/dev-changelog.md`（开发者笔记）
- 教程新增 `@` 常量与 `#blanket` 章节；架构文档新增「语法域隔离」与
  「附着语义」章节

## 0.5.7 (2026-08-03)

### 新特性：trait 级 where 子句继承（自动生效，无需改代码）

`trait Foo<T> where T: Clone` 的谓词**全形态**继承到生成的 impl：

- **单一形参谓词**（`T: Clone`）合并进泛型 bound——`<T> Foo<T>` →
  `impl<T: Clone>`，与内联 bound（`trait Foo<T: Clone>`）同一条继承链路
  （同名继承 / 改名报错 / 引用检查全部复用）；
- **其余谓词原样透传**到 impl 的 where 子句：`T::Item: Clone`、`Vec<T>: ...`、
  生命周期谓词（`'a: 'b`）等全部覆盖，`<T>` 与 `A<>` 两种写法同效。

### 行为变化

- 此前复合谓词（`T::Item: Clone` 等）被静默丢弃，生成缺约束的 impl 导致
  rustc E0277（且定位模糊）；现在自动附加到 impl where，与手写等价。
  此前因此报错的代码升级后直接可用。
- 新增错误消息：`继承的 where 谓词 ... 引用形参 ...，请声明或手写 where`
  （改名场景引导）。
- 无破坏性变化；`batch_trait!` 无 trait 定义，不受影响。

## 0.5.6 (2026-08-03)

### 行为变化：孤立 `<` / `>` 报错

- 未配对的 `<`（缺少匹配 `>`）与多余的 `>`（缺少匹配 `<`）此前透传为垃圾
  token，现在报 `compile_error!`（非法输入）。

## 0.5.5 (2026-08-03)

### 新特性：`A<>` trait 泛型照抄

- `A<>`：空实参列表表示"实参与 bound 全部来自 trait 定义"——
  `trait Foo<T: Clone>` + `#[batch_impl(Foo<> ())]` 展开为
  `impl<T: Clone> Foo<T> for ()`，一行都不用写泛型；
- `A<绑定们>` 同款照抄：`Foo<Item=T>` 照抄位置实参 + 绑定原样保留；
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 可用（需要 trait 定义）；
  `batch_trait!` 无 trait 定义，`A<>` 原样透传。

### 行为变化：改名 = 明确报错，绝不静默

- 实参 `X` 对应形参 `T`（有 bound）但名字不同、或继承的 bound 引用
  `'a`/`U` 等形参名而 impl 未声明同名——均报 `compile_error!` 引导
  （请改名或手写 bound）。此前改名场景静默退化为不继承，生成缺 bound 的
  impl 报 E0277。

## 0.5.4 (2026-08-03)

### 新特性：trait 泛型 bound 自动继承

`trait Foo<T: Clone>` 时，spec 中**未写 bound** 的 impl 泛型参数按名继承 trait
的同名参数内联 bound——`#[batch_impl(<T> Foo<T> Vec<T>)]` 直接生成
`impl<T: Clone> Foo<T> for Vec<T>`，无需手写（此前生成的 impl 缺 bound 报 E0277）。

- 写了 bound = 用户负责，宏不干预（sub trait 蕴含交由 rustc 验证）；
- 继承 `T: Clone` / `T: 'a` 等内联 bound；trait 级 where 子句不继承（0.5.7 起支持）；
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持；`batch_trait!` 不继承。

### 新特性：指令参数列表减法 `-name`（取代 `#except`）

`#fill`/`#delegate` 参数新增 `-` 前缀排除项：保留列表减去排除列表，排除优先。
`#except(保留){排除}` 的双括号形式被取代并移除：

- `#fill(#all,-foo){body}` = 所有 item 除 `foo`
- `#fill(#all,-#all_methods)` = 仅 const + type 项
- `-` 后缺目标、排除后为空报 `compile_error!`

## 0.5.3 (2026-08-02)

### 新特性

- **`unsafe fn(...)` 类型**：`unsafe` 紧跟 `fn` 时修饰 fn 类型本身
  （`unsafe fn(u32)->u32`、`unsafe fn^(A,B)-C`）；`unsafe X`（X 非 fn，并列）
  报错（忘写 `.` 的笔误）；裸 `unsafe` 后跟 `.`/`-` 仍是 unsafe impl 标记。
- **开放扩展机制修复**：不认识的 `#name(args){body}` 展开为函数式宏调用
  `name!{(args){body} trait ...}`——把方法名列表、body 与整个 trait 交给
  用户的同名宏（"用户自定义的 `#fill`"，此前属性委托写法必然编译失败）。
- **指令减法 `#except(保留){排除}`**（0.5.4 被 `-name` 取代并移除）。

### 修复

- **`#delegate` 参数转发加固**：解构模式参数（`(a, b)` / `_`）无法委托转发，
  此前被静默丢弃生成错误调用，现报 `compile_error!`（含 trait 名与方法名）。
- **空范围诊断**：`().3..2` 等空范围此前静默生成零个 impl，现报错。
- **尾随运算符静默吞段修复**：`A.`、`f32 Vec^-` 等尾随运算符此前整段静默
  消失（下游 E0599 定位模糊），现报 `compile_error!`。
- **空操作数严格化**：`-A`（左空静默吞段）、`.A`（生成垃圾类型）、`,A`、
  `A,,B` 均报错；尾随逗号（`A,`）与 `()`/`[]` 真实 token 不受影响。
- **指令参数逗号严格化**：`#fill(a,,b)` 等前导/尾随/连续逗号报错（此前静默跳过）。

### 行为约束：组合展开数量上限

`^N` / 笛卡尔积 / 范围批量等展开超过 1024 产物（如 `().100000`、
`[A,B]^[C,D]^[E,F]`）报 `compile_error!`，防止误写挂死编译。

## 0.5.2 (2026-08-01)

### 新特性：数组/切片 builder

- `[]^T` → `[T]`（空基座包出切片）
- `[T]^N` → `[T; N]`（定长数组；`N` 可为数字字面量、const 泛型标识符、范围或列表）
- `<const N: usize> []-X-N` → `[X; N]`：`[]` 作 `-` 累加链基座，把整个类型矩阵
  包进 const 泛型定长数组
- `()^N` 的 fresh 泛型元组作为泛型实参/数组元素时自动外提
  （修复 `Box^()^N` 与矩阵嵌入的既有 bug）

## 0.5.1 (2026-07-31)

### 新特性：`where{...}` 后缀

- `where{...}` 跟在目标类型之后，为生成的 impl 添加 where 子句；多个会合并。
- 裸写 `where 谓词 {代码块}` 新语法（三个接口通用）：谓词区逗号不被 spec
  切分，`ident!{...}` 宏体不计入边界，多个 `where` 段可依次书写。

## 0.5.0 (2026-07-28)

### 新特性：`#[batch_impl_only]` 外部 trait 路径前缀

`#[batch_impl_only(#ext::mod::TraitName: usize, isize)]` 为外部模块中定义的
trait 生成 impl（路径末尾标识符必须与本地 dummy trait 名一致；
`#[batch_impl]` 不支持此前缀）。

## 0.4.2 (2026-07-27)

### 新特性

- **`#name{body}` 支持 const / type 项**：`#CONST{value}` → `const ... = value;`、
  `#Type{def}` → `type ... = def;`，不再局限于 fn。
- **`#fill` 扩展与 `#all` 标记**：`#fill` 可用于 fn + const + type；
  `#all` 变为所有 item；新增 `#all_methods` / `#all_constants` / `#all_types`。
- `#delegate` 仍仅支持 Fn，传入非 Fn 项报 `compile_error!`。

## 0.4.1 (2026-07-25)

- 修复自定义（开放扩展）宏未携带 trait_def 的问题。

## 0.4.0 (2026-07-25)

### 新特性：指令系统

| 指令   | 语法                      | 效果                                      |
|--------|---------------------------|-------------------------------------------|
| 单方法 | `#method{body}`           | `{fn method(签名) { body }}`              |
| 填充   | `#fill(args){body}`       | `{fn m1(sig){body} fn m2(sig){body} ...}` |
| 委托   | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}`     |

- `#fill(#all){body}` 表示 trait 的所有方法
- 指令与 DSL 运算符、`{body}` 连续附着、泛型、unsafe 等特性自由组合
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持

### 新特性：`#[batch_impl_only]` 与 `{body}` 连续附着

- `#[batch_impl_only]`：丢弃 trait 定义、只输出 impl 块（trait 已在别处定义时用）
- `T{body1}{body2}` 正确递归附着

## 0.3.0 (2026-07-24)

### 完全重写

v0.3.0 是从零开始的完全重写。公开 API 和 DSL 语法与 v0.2.x 保持一致。
功能清单：

- `#[batch_impl]` 属性宏 + `batch_trait!` 函数式宏
- `.`（右结合）/ `-`（左结合）运算符：泛型应用、类型组合
- `[A, B, C]` 并列列表 + `{ body }` 独立/共享实现体合并
- `<T: Clone, Item=V>` 泛型参数与关联类型绑定
- `()^N` 元组生成 + `(<Bound>)^N` 带约束元组 + `(T1,T2)^N` 笛卡尔积 + 范围语法
- `&` / `&mut` / `*const` / `*mut` / `fn` / `self` / `unsafe` / `#[attr]` 前缀修饰符
- `fn(A,B)->C` 函数类型
- `HashMap<K>^V` 预填泛型追加
- `unsafe^T` 单条 unsafe + `unsafe trait` 自动 unsafe
- `compile_error!` 错误输出（不 panic、不 ICE）

### 修复（相对于 v0.2.x）

- `batch_trait!` 中 `fn(i32) -> bool` 等含 `->` 的 spec 不再误断段落边界
- `()^0` 正确生成空元组 `()`

## 0.2.2 (2026-07-20)

### 修复

- `fn^i32` 正确生成 `fn(i32)` 而非 `fn i32`
- 所有工具函数统一排除 `->` 中的 `>`（`HashMap^<u32>-String` 等含 `->` 的
  类型不再误判尖括号）

## 0.2.1 (2026-07-20)

### 修复

- **优先级**：`HashMap^K-V` 现在正确解析为 `HashMap<K, V>`（此前被解析为
  `HashMap<K<V>>`）。注意：`Box^Vec-u32` 仍是错误写法，应写 `Box^Vec^u32`
- `HashMap^<u32>-String` 中 `-String` 不再被静默丢弃
- `unsafe^#[attr]^T` 不再报"属性 . 的内部错误"
- `fn^(u32,i32)-usize` 正确生成 `fn(u32,i32)->usize`（此前返回类型被当参数追加）
- 嵌套 `fn^(u32,i32)^i64-usize` 不再丢失 `Fn` 前缀

## 0.2.0 (2026-07-19)

### 新功能

- **关联类型简洁写法**：`TraitName<AssocType=value>`（支持多绑定与复杂类型，
  可与 `.`/`-`/unsafe 组合）
- **独立/共享 body 合并**：`[A{bodyA}, B{bodyB}]{shared}`（支持多层嵌套）
- **元组生成规则修改**：`()^N` 生成带 N 个泛型参数的元组；`(T)^N` 生成长度
  N 的重复元组；`(T1,T2)^N` 笛卡尔积；范围语法 `()^M..N` / `()^M..=N`
- **`*const`/`*mut` 指针**：`*const^T` → `*const T`，支持链式
- **引用修饰符特殊行为**：`&^A^B` → `&A<B>`（先绑定再应用）
- **fn 关键字**：`fn^(A,B)` 创建、`fn(A,B)^T` 追加返回类型、`fn-(A,B)^N` 组合
- **`#[...]` 属性**：`#[attr]^T` 在 impl 块前添加属性

## 0.1.1 (2026-07-19)

### 新功能：预填泛型追加

- `A<B>^C` → `A<B, C>`（容器带预填泛型时 `.` 追加参数而非生成 `A<B><C>`）
- `[Box, Cow<'_>]^T` → `Box<T>, Cow<'_, T>`（列表支持）
- `-` 运算符自动受益：`HashMap-u32-String` → `HashMap<u32, String>`

## 0.1.0 (2026-07-19)

### 初始发布

- `#[batch_impl(...)]` 属性宏 + `batch_trait!(...)` 函数式宏
- `.`（右结合）/ `-`（左结合）运算符：泛型应用
- 元组生成：`()^N`、`(<Bound>)^N`、`(T1,T2)^N` 笛卡尔积、`()^M..N` 范围
- 泛型支持：impl 泛型（含 const）、trait 泛型、生命周期、泛型继承
- `unsafe^T` / `unsafe trait` / `batch_trait!(unsafe ...)` 
- 中文错误提示，`compile_error!` 而非 panic
