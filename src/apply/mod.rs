//! 运算层：`Apply` trait 与各 `Ty` 变体的运算符语义实现。

pub(crate) mod apply_tuple;

// [`Apply`] trait 定义二元运算 `A.apply(B)`：`^`（右结合）/ `-`（左结合）。
// 各 `Ty` 变体分别实现 [`Apply::apply_help`]，承担其对应的组合语义——容器
// 追加参数、引用包裹、并列列表笛卡尔积、元组长度展开（`()^N`、`(<Bound>)^N`）、
// 关联参数生成等。**右操作数"结构上下文"的提前分发**（Array 分发 / Group 透明 /
// WithCode、WithWhere 应用透传 / WithType 泛型外提 / Range 展开 / Error 透传）
// 由 [`Apply::apply`] 的默认实现承担——所有 `Apply` 实现自动获得，无需重复。
// 两阶段分工：
//
// 1. [`Apply::apply`]（默认）：先看**右操作数**的结构——右操作数是"上下文"，
//    必须无条件先处理；
// 2. [`Apply::apply_help`]：右操作数是普通类型时才看**左操作数**的语义。
//    默认 `apply` 保证 `apply_help` 的 `o` 恒为普通类型。
//
// v0.4.2 中由 `Type` 更名为 `Apply`；v0.5.3 将提前分发下沉为默认方法，使
// "右操作数结构分发"成为 trait 契约（此前仅 `impl Apply for Ty` 隐式承担，
// 各变体实现隐式依赖它预先分发）。

use quote::quote;

use crate::apply::apply_tuple::map_range;
use crate::ast::*;

/// 用消息生成包含 `compile_error!` 的 `Ty::Error`
pub(crate) fn err_ty(msg: &str) -> Ty {
    TyError(quote! { compile_error!(#msg); }).into()
}

/// 展开产物数量上限校验：`len` 超过 [`MAX_EXPAND`] 时返回 `compile_error!` 信号。
/// 用于 `^N` / 笛卡尔积 / 范围批量等可能指数级膨胀的展开点。
pub(crate) fn check_expand_limit(what: &str, len: usize) -> Option<Ty> {
    (len > MAX_EXPAND).then(|| {
        err_ty(&format!(
            "batch-impl: `{}` 的展开产物数量 {} 超过上限 {}，可能是指数/范围/笛卡尔积误写",
            what, len, MAX_EXPAND
        ))
    })
}

/// 类型表达式上的二元运算：`A^B` / `A-B` 中，`A.apply(B)` 产出组合后的 `Ty`。
///
/// 需要 `Clone`（默认的数组分发 / 范围展开要复用左操作数 `self`）与
/// `Into<Ty> + Into<Box<Ty>> + Into<Option<Box<Ty>>>`（把左操作数装回类型 /
/// 目标类型——裸代码块、裸 where 作为右操作数时，以及 `TyPrimitive` 等变体
/// 把 `self` 转为泛型基座时使用）。
pub(crate) trait Apply:
    Clone + Into<Ty> + Into<Box<Ty>> + Into<Option<Box<Ty>>>
{
    /// 右操作数"结构上下文"提前分发（默认实现，所有 `Apply` 实现自动获得）。
    ///
    /// `o` 为 Array/Group/WithCode/WithWhere/WithType/Range/Error 时在此处理
    /// （数组分发 / Group 透明 / 应用透传 / 泛型外提 / 范围展开 / 错误透传），
    /// 否则委托给 [`Apply::apply_help`]——因此 `apply_help` 的右操作数
    /// **恒为普通类型**。
    fn apply(self, o: Ty) -> Ty {
        match o {
            // 数组分发：左操作数 apply 到右数组的每个元素。
            // 数组-数组链式（`[A,B]^[C,D]^[E,F]`）的产物按**叶子数**校验上限——
            // 中间数组每个都小，但叶子数随 `^` 链指数增长。
            Ty::Array(arr) => {
                let result: Vec<Ty> =
                    arr.0.into_iter().map(|e| self.clone().apply(e)).collect();
                if let Some(e) = check_expand_limit(
                    "并列列表链式展开",
                    result.iter().map(count_leaves).sum(),
                ) {
                    return e;
                }
                TyArray(result).into()
            }
            Ty::Group(g) => self.apply(*g.0),
            Ty::WithCode(wc) => match wc.0 {
                Some(inner) => TyWithCode(self.apply(*inner).into(), wc.1).into(),
                None => TyWithCode(self.into(), wc.1).into(),
            },
            Ty::WithWhere(ww) => match ww.0 {
                Some(inner) => TyWithWhere(self.apply(*inner).into(), ww.1).into(),
                None => TyWithWhere(self.into(), ww.1).into(),
            },
            // 右操作数为 `WithType`（如 `()^N` 的 fresh 泛型元组）时，
            // 把泛型声明外提到外层：`T^<A>X` => `<A>(T^X)`，
            // 避免 `T<<A>X>` 在类型中泄漏泛型声明。
            Ty::WithType(wt) => TyWithType(wt.0, self.apply(*wt.1).into()).into(),
            Ty::Error(e) => e.into(),
            Ty::Range(TyRange { start, end, inclusive }) => {
                map_range(start, end, inclusive, |n| {
                    self.clone().apply(TyNum(n).into())
                })
            }
            _ => self.apply_help(o),
        }
    }

    /// 左操作数"语义"：各变体实现自己的组合规则。
    /// 由默认 [`Apply::apply`] 保证 `o` 为普通类型（非结构上下文）。
    fn apply_help(self, o: Ty) -> Ty;
}

impl Apply for Ty {
    fn apply_help(self, o: Ty) -> Ty {
        match self {
            Ty::WithPrefix(wp) => wp.apply_help(o),
            Ty::Primitive(p) => p.apply_help(o),
            Ty::Generic(g) => g.apply_help(o),
            Ty::Trait(t) => t.apply_help(o),
            Ty::Array(a) => a.apply_help(o),
            Ty::Tuple(t) => t.apply_help(o),
            Ty::Group(g) => g.apply_help(o),
            Ty::Fn(f) => f.apply_help(o),
            Ty::WithAttr(w) => w.apply_help(o),
            Ty::WithTrait(wt) => wt.apply_help(o),
            Ty::WithType(wt) => wt.apply_help(o),
            Ty::WithCode(wc) => wc.apply_help(o),
            Ty::WithWhere(ww) => ww.apply_help(o),
            Ty::TypeParam(t) => t.apply_help(o),
            Ty::Num(n) => n.apply_help(o),
            Ty::Range(r) => r.apply_help(o),
            Ty::PrimitiveArray(pa) => pa.apply_help(o),
            Ty::Error(e) => e.into(),
        }
    }
}

impl Apply for TyWithPrefix {
    /// `&^T` => `&T`；`*const^T` => `*const T`；`self^T` => `T`；`unsafe^T` => `unsafe T`（unsafe impl 标记）
    ///
    /// `&T^U` => `&(T^U)`、`unsafe T^U` => `unsafe (T^U)`：修饰符透传到内部类型。
    fn apply_help(self, o: Ty) -> Ty {
        match self.0 {
            // &^T=>&T / unsafe^T=>unsafe T
            TyPrefix::Ref
            | TyPrefix::RefMut
            | TyPrefix::PtrConst
            | TyPrefix::PtrMut
            | TyPrefix::Unsafe => {
                let inner = match self.1 {
                    Some(t) => t.apply(o),
                    None => o,
                };
                TyWithPrefix(self.0, inner.into()).into()
            }
            // self^T=>T
            TyPrefix::SelfType => o,
        }
    }
}

impl Apply for TyPrimitive {
    /// `T^U` => `T<U>`，`T^<A,B>` => `T<A,B>`
    fn apply_help(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(tp) => TyGeneric(self.into(), tp).into(),
            _ => TyGeneric(self.into(), TyTypeParam::single(&o)).into(),
        }
    }
}

impl Apply for TyGeneric {
    /// `T<A>^B` => `T<A,B>`；`T<A>^<B,C>` => `T<A,B,C>`
    fn apply_help(self, o: Ty) -> Ty {
        let mut tp = self.1;
        match o {
            Ty::TypeParam(rhs) => tp.extend(rhs),
            _ => tp.push_arg(&o),
        }
        TyGeneric(self.0, tp).into()
    }
}

impl Apply for TyTrait {
    /// `Trait<T>^U` => `WithTrait(Trait<T>, U)`（trait 泛型应用到目标类型上）
    fn apply_help(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                TyTrait(self.0, tp).into()
            }
            _ => TyWithTrait(self, o.into()).into(),
        }
    }
}

impl Apply for TyArray {
    /// `[A,B]^C` => `[A^C, B^C]`（右操作数为普通类型；`[A,B]^[C,D]` 的笛卡尔积
    /// 由默认 `apply` 的 Array 分支逐层分发 + `expand` 摊平）
    fn apply_help(self, o: Ty) -> Ty {
        let result = self.0.into_iter().map(|e| e.apply(o.clone())).collect();
        TyArray(result).into()
    }
}
