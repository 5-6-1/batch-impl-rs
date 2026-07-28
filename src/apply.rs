//! 运算符语义。
//!
//! [`Apply`] trait 定义二元运算 `A.apply(B)`：`^`（右结合）/ `-`（左结合）。
//! 各 `Ty` 变体分别实现 `Apply`，承担其对应的组合语义——容器追加参数、
//! 引用包裹、并列列表笛卡尔积、元组长度展开（`()^N`、`(<Bound>)^N`）、
//! 关联参数生成等。`impl Apply for Ty` 兜底做数组分发与
//! `Group` 透明展开。
//!
//! v0.4.2 中由 `Type` 更名为 `Apply`，以避免与 stdlib `core::any::Any`
//! 等通用名称混淆，并强调"运算符语义"而非"类型身份"。

use quote::quote;

use crate::apply_tuple::map_range;
use crate::types::*;

/// 用消息生成包含 `compile_error!` 的 `Ty::Error`
pub(crate) fn err_ty(msg: &str) -> Ty {
    Ty::Error(TyError(quote! { compile_error!(#msg); }))
}

/// 类型表达式上的二元运算：`A^B` / `A-B` 中，`A.apply(B)` 产出组合后的 `Ty`。
///
/// 每个 `Ty` 变体各自实现 `apply` 的语义（容器追加参数、引用包裹、列表笛卡尔积等）；
/// `impl Apply for Ty` 统一做数组分发（`[A,B]^C => [A^C, B^C]`）和 `Group` 透明展开后委托。
pub(crate) trait Apply {
    fn apply(self, o: Ty) -> Ty;
}

impl Apply for Ty {
    fn apply(self, o: Ty) -> Ty {
        // 数组分发：左操作数 apply 到右数组的每个元素
        match o {
            Ty::Array(arr) => TyArray(
                arr.0.into_iter().map(|e| self.clone().apply(e)).collect(),
            )
            .into(),
            Ty::Group(g) => self.apply(*g.0),
            Ty::WithCode(wc) => {
                TyWithCode(self.apply(*wc.0).into(), wc.1).into()
            },
            Ty::Error(e) => e.into(),
            Ty::Range(TyRange {
                start,
                end,
                inclusive,
            }) => map_range(start, end, inclusive, |n| {
                self.clone().apply(TyNum(n).into())
            }),
            _ => match self {
                Ty::Prefix(p) => p.apply(o),
                Ty::Modified(m) => m.apply(o),
                Ty::Unsafe(u) => u.apply(o),
                Ty::Primitive(p) => p.apply(o),
                Ty::Generic(g) => g.apply(o),
                Ty::Trait(t) => t.apply(o),
                Ty::Array(a) => a.apply(o),
                Ty::Tuple(t) => t.apply(o),
                Ty::Group(g) => g.apply(o),
                Ty::Fn(f) => f.apply(o),
                Ty::CodeBlock(b) => b.apply(o),
                Ty::Attr(a) => a.apply(o),
                Ty::WithAttr(w) => w.apply(o),
                Ty::WithTrait(wt) => wt.apply(o),
                Ty::WithType(wt) => wt.apply(o),
                Ty::WithCode(wc) => wc.apply(o),
                Ty::TypeParam(t) => t.apply(o),
                Ty::Num(n) => n.apply(o),
                Ty::Range(r) => r.apply(o),
                Ty::Slice(s) => s.apply(o),
                Ty::FixedArray(f) => f.apply(o),
                Ty::Error(e) => e.into(),
            },
        }
    }
}

impl Apply for TyPrefix {
    /// `&^T` => `&T`；`*const^T` => `*const T`；`self^T` => `T`；`fn^(A,B)` => `fn(A,B)`；`unsafe^T` => `unsafe T`
    fn apply(self, o: Ty) -> Ty {
        match self {
            // &^T=>&T
            TyPrefix::Ref
            | TyPrefix::RefMut
            | TyPrefix::PtrConst
            | TyPrefix::PtrMut => TyModified(self, o.into()).into(),
            // self^T=>self
            TyPrefix::SelfType => o,
            // fn^(...,)=>fn(...)
            TyPrefix::Fn => match o {
                Ty::Tuple(t) => TyFn(t.0, None).into(),
                Ty::Group(t) => TyFn(vec![*t.0], None).into(),
                _ => err_ty(
                    "batch-impl: `fn` 前缀右侧必须是元组类型，如 fn^(i32, u32)",
                ),
            },
            // unsafe^T=unsafe下T
            TyPrefix::Unsafe => TyUnsafe(o.into()).into(),
        }
    }
}

impl Apply for TyModified {
    /// `&T^U` => `&(T^U)`（修饰符透传到内部类型）
    fn apply(self, o: Ty) -> Ty {
        // &T^U=>&(T^U)
        TyModified(self.0, self.1.apply(o).into()).into()
    }
}

impl Apply for TyUnsafe {
    /// `unsafe T^U` => `unsafe (T^U)`（unsafe 修饰透传到内部类型）
    fn apply(self, o: Ty) -> Ty {
        // unsafe传递
        TyUnsafe(self.0.apply(o).into()).into()
    }
}

impl Apply for TyPrimitive {
    /// `T^U` => `T<U>`，`T^<A,B>` => `T<A,B>`
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(tp) => TyGeneric(self.into(), tp).into(),
            _ => TyGeneric(self.into(), TyTypeParam::single(&o)).into(),
        }
    }
}

impl Apply for TyGeneric {
    /// `T<A>^B` => `T<A,B>`；`T<A>^<B,C>` => `T<A,B,C>`
    fn apply(self, o: Ty) -> Ty {
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
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                TyTrait(self.0, tp).into()
            },
            _ => TyWithTrait(self, o.into()).into(),
        }
    }
}

impl Apply for TyArray {
    /// `[A,B]^C` => `[A^C, B^C]`；`[A,B]^[C,D]` => `[A^C, A^D, B^C, B^D]`（笛卡尔积）
    fn apply(self, o: Ty) -> Ty {
        match o {
            Ty::Array(right) => {
                let mut result = vec![];
                for left in self.0 {
                    for right_elem in &right.0 {
                        result
                            .push(left.clone().apply(right_elem.clone()));
                    }
                }
                TyArray(result).into()
            },
            _ => {
                let result = self
                    .0
                    .into_iter()
                    .map(|e| e.apply(o.clone()))
                    .collect();
                TyArray(result).into()
            },
        }
    }
}
